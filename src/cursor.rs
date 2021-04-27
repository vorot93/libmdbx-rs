use crate::{
    database::Database,
    error::{
        mdbx_result,
        Error,
        Result,
    },
    flags::*,
    mdbx_try_optional,
    transaction::{
        TransactionKind,
        RW,
    },
    util::freeze_bytes,
    RO,
};
use ffi::{
    MDBX_cursor_op,
    MDBX_FIRST,
    MDBX_FIRST_DUP,
    MDBX_GET_BOTH,
    MDBX_GET_BOTH_RANGE,
    MDBX_GET_CURRENT,
    MDBX_GET_MULTIPLE,
    MDBX_LAST,
    MDBX_LAST_DUP,
    MDBX_NEXT,
    MDBX_NEXT_DUP,
    MDBX_NEXT_MULTIPLE,
    MDBX_NEXT_NODUP,
    MDBX_PREV,
    MDBX_PREV_DUP,
    MDBX_PREV_MULTIPLE,
    MDBX_PREV_NODUP,
    MDBX_SET,
    MDBX_SET_KEY,
    MDBX_SET_LOWERBOUND,
    MDBX_SET_RANGE,
};
use libc::{
    c_uint,
    c_void,
};
use lifetimed_bytes::Bytes;
use std::{
    fmt,
    marker::PhantomData,
    mem,
    ptr,
    result,
};

/// A cursor for navigating the items within a database.
pub struct Cursor<'txn, K>
where
    K: TransactionKind,
{
    cursor: *mut ffi::MDBX_cursor,
    _marker: PhantomData<fn(&'txn (), K)>,
}

impl<'txn, K> Cursor<'txn, K>
where
    K: TransactionKind,
{
    pub(crate) fn new(db: &Database<'txn, K>) -> Result<Self> {
        let mut cursor: *mut ffi::MDBX_cursor = ptr::null_mut();
        unsafe {
            mdbx_result(ffi::mdbx_cursor_open(db.txn(), db.dbi(), &mut cursor))?;
        }
        Ok(Self {
            cursor,
            _marker: PhantomData,
        })
    }

    pub(crate) fn new_at_position(other: &Self) -> Result<Self> {
        unsafe {
            let cursor = ffi::mdbx_cursor_create(ptr::null_mut());

            let res = ffi::mdbx_cursor_copy(other.cursor(), cursor);

            let s = Self {
                cursor,
                _marker: PhantomData,
            };

            mdbx_result(res)?;

            Ok(s)
        }
    }

    /// Returns a raw pointer to the underlying MDBX cursor.
    ///
    /// The caller **must** ensure that the pointer is not used after the
    /// lifetime of the cursor.
    pub fn cursor(&self) -> *mut ffi::MDBX_cursor {
        self.cursor
    }

    /// Retrieves a key/data pair from the cursor. Depending on the cursor op,
    /// the current key may be returned.
    fn get(
        &self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: MDBX_cursor_op,
    ) -> Result<(Option<Bytes<'txn>>, Bytes<'txn>, bool)> {
        unsafe {
            let mut key_val = slice_to_val(key);
            let mut data_val = slice_to_val(data);
            let key_ptr = key_val.iov_base;
            let data_ptr = data_val.iov_base;
            let v = mdbx_result(ffi::mdbx_cursor_get(self.cursor(), &mut key_val, &mut data_val, op))?;
            assert_ne!(data_ptr, data_val.iov_base);
            let txn = ffi::mdbx_cursor_txn(self.cursor());
            let key_out = {
                // MDBX wrote in new key
                if key_ptr != key_val.iov_base {
                    Some(freeze_bytes::<K>(txn, &key_val)?)
                } else {
                    None
                }
            };
            let data_out = freeze_bytes::<K>(txn, &data_val)?;

            Ok((key_out, data_out, v))
        }
    }

    fn get_value(
        &mut self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: MDBX_cursor_op,
    ) -> Result<Option<Bytes<'txn>>> {
        let (_, v, _) = mdbx_try_optional!(self.get(key, data, op));

        Ok(Some(v))
    }

    fn get_full(
        &mut self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: MDBX_cursor_op,
    ) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        let (k, v, _) = mdbx_try_optional!(self.get(key, data, op));

        Ok(Some((k.unwrap(), v)))
    }

    /// Position at first key/data item.
    pub fn first(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_FIRST)
    }

    /// [DatabaseFlags::DUP_SORT]-only: Position at first data item of current key.
    pub fn first_dup(&mut self) -> Result<Option<Bytes<'txn>>> {
        self.get_value(None, None, MDBX_FIRST_DUP)
    }

    /// [DatabaseFlags::DUP_SORT]-only: Position at key/data pair.
    pub fn get_both(&mut self, k: impl AsRef<[u8]>, v: impl AsRef<[u8]>) -> Result<Option<Bytes<'txn>>> {
        self.get_value(Some(k.as_ref()), Some(v.as_ref()), MDBX_GET_BOTH)
    }

    /// [DatabaseFlags::DUP_SORT]-only: Position at given key and at first data greater than or equal to specified data.
    pub fn get_both_range(&mut self, k: impl AsRef<[u8]>, v: impl AsRef<[u8]>) -> Result<Option<Bytes<'txn>>> {
        self.get_value(Some(k.as_ref()), Some(v.as_ref()), MDBX_GET_BOTH_RANGE)
    }

    /// Return key/data at current cursor position.
    pub fn get_current(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_GET_CURRENT)
    }

    /// DupFixed-only: Return up to a page of duplicate data items from current cursor position.
    /// Move cursor to prepare for [Self::next_multiple()].
    pub fn get_multiple(&mut self) -> Result<Option<Bytes<'txn>>> {
        self.get_value(None, None, MDBX_GET_MULTIPLE)
    }

    /// Position at last key/data item.
    pub fn last(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_LAST)
    }

    /// DupSort-only: Position at last data item of current key.
    pub fn last_dup(&mut self) -> Result<Option<Bytes<'txn>>> {
        self.get_value(None, None, MDBX_LAST_DUP)
    }

    /// Position at next data item
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_NEXT)
    }

    /// [DatabaseFlags::DUP_SORT]-only: Position at next data item of current key.
    pub fn next_dup(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_NEXT_DUP)
    }

    /// [DatabaseFlags::DUP_FIXED]-only: Return up to a page of duplicate data items from next cursor position. Move cursor to prepare for MDBX_NEXT_MULTIPLE.
    pub fn next_multiple(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_NEXT_MULTIPLE)
    }

    /// Position at first data item of next key.
    pub fn next_nodup(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_NEXT_NODUP)
    }

    /// Position at previous data item.
    pub fn prev(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_PREV)
    }

    /// [DatabaseFlags::DUP_SORT]-only: Position at previous data item of current key.
    pub fn prev_dup(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_PREV_DUP)
    }

    /// Position at last data item of previous key.
    pub fn prev_nodup(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_PREV_NODUP)
    }

    /// Position at specified key.
    pub fn set(&mut self, key: impl AsRef<[u8]>) -> Result<Option<Bytes<'txn>>> {
        self.get_value(Some(key.as_ref()), None, MDBX_SET)
    }

    /// Position at specified key, return both key and data.
    pub fn set_key(&mut self, key: impl AsRef<[u8]>) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(Some(key.as_ref()), None, MDBX_SET_KEY)
    }

    /// Position at first key greater than or equal to specified key.
    pub fn set_range(&mut self, key: impl AsRef<[u8]>) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(Some(key.as_ref()), None, MDBX_SET_RANGE)
    }

    /// [DatabaseFlags::DUP_FIXED]-only: Position at previous page and return up to a page of duplicate data items.
    pub fn prev_multiple(&mut self) -> Result<Option<(Bytes<'txn>, Bytes<'txn>)>> {
        self.get_full(None, None, MDBX_PREV_MULTIPLE)
    }

    /// Position at first key-value pair greater than or equal to specified, return both key and data, and the return code depends on a exact match.
    ///
    /// For non DupSort-ed collections this works the same as [Self::set_range()], but returns [false] if key found exactly and [true] if greater key was found.
    ///
    /// For DupSort-ed a data value is taken into account for duplicates, i.e. for a pairs/tuples of a key and an each data value of duplicates.
    /// Returns [false] if key-value pair found exactly and [true] if the next pair was returned.
    pub fn set_lowerbound(&mut self, key: impl AsRef<[u8]>) -> Result<Option<(bool, Bytes<'txn>, Bytes<'txn>)>> {
        let (k, v, found) = mdbx_try_optional!(self.get(Some(key.as_ref()), None, MDBX_SET_LOWERBOUND));

        Ok(Some((found, k.unwrap(), v)))
    }

    /// Iterate over database items. The iterator will begin with item next
    /// after the cursor, and continue until the end of the database. For new
    /// cursors, the iterator will begin with the first item in the database.
    ///
    /// For databases with duplicate data items ([DatabaseFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn iter(&mut self) -> Iter<'txn, '_, K>
    where
        Self: Sized,
    {
        Iter::new(self, ffi::MDBX_NEXT, ffi::MDBX_NEXT)
    }

    /// Iterate over database items starting from the beginning of the database.
    ///
    /// For databases with duplicate data items ([DatabaseFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn iter_start(&mut self) -> Iter<'txn, '_, K>
    where
        Self: Sized,
    {
        Iter::new(self, ffi::MDBX_FIRST, ffi::MDBX_NEXT)
    }

    /// Iterate over database items starting from the given key.
    ///
    /// For databases with duplicate data items ([DatabaseFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn iter_from(&mut self, key: impl AsRef<[u8]>) -> Iter<'txn, '_, K> {
        match self.set_range(key) {
            Ok(_) => (),
            Err(error) => return Iter::Err(error),
        };
        Iter::new(self, ffi::MDBX_GET_CURRENT, ffi::MDBX_NEXT)
    }

    /// Iterate over duplicate database items. The iterator will begin with the
    /// item next after the cursor, and continue until the end of the database.
    /// Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup(&mut self) -> IterDup<'txn, '_, K> {
        IterDup::new(self, ffi::MDBX_NEXT)
    }

    /// Iterate over duplicate database items starting from the beginning of the
    /// database. Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup_start(&mut self) -> IterDup<'txn, '_, K> {
        IterDup::new(self, ffi::MDBX_FIRST)
    }

    /// Iterate over duplicate items in the database starting from the given
    /// key. Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup_from(&mut self, key: impl AsRef<[u8]>) -> IterDup<'txn, '_, K> {
        match self.set_range(key) {
            Ok(_) => (),
            Err(error) => return IterDup::Err(error),
        };
        IterDup::new(self, ffi::MDBX_GET_CURRENT)
    }

    /// Iterate over the duplicates of the item in the database with the given key.
    pub fn iter_dup_of(&mut self, key: impl AsRef<[u8]>) -> Iter<'txn, '_, K> {
        match self.set(key) {
            Ok(Some(_)) => (),
            Ok(None) => {
                self.last().ok();
                return Iter::new(self, ffi::MDBX_NEXT, ffi::MDBX_NEXT);
            },
            Err(error) => return Iter::Err(error),
        };
        Iter::new(self, ffi::MDBX_GET_CURRENT, ffi::MDBX_NEXT_DUP)
    }
}

impl<'txn> Cursor<'txn, RW> {
    /// Puts a key/data pair into the database. The cursor will be positioned at
    /// the new data item, or on failure usually near it.
    pub fn put(&mut self, key: impl AsRef<[u8]>, data: impl AsRef<[u8]>, flags: WriteFlags) -> Result<()> {
        let key = key.as_ref();
        let data = data.as_ref();
        let key_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: key.len(),
            iov_base: key.as_ptr() as *mut c_void,
        };
        let mut data_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: data.len(),
            iov_base: data.as_ptr() as *mut c_void,
        };
        mdbx_result(unsafe { ffi::mdbx_cursor_put(self.cursor(), &key_val, &mut data_val, flags.bits()) })?;

        Ok(())
    }

    /// Deletes the current key/data pair.
    ///
    /// ### Flags
    ///
    /// [WriteFlags::NO_DUP_DATA] may be used to delete all data items for the
    /// current key, if the database was opened with [DatabaseFlags::DUP_SORT].
    pub fn del(&mut self, flags: WriteFlags) -> Result<()> {
        mdbx_result(unsafe { ffi::mdbx_cursor_del(self.cursor(), flags.bits()) })?;

        Ok(())
    }
}

impl<'txn, K> Clone for Cursor<'txn, K>
where
    K: TransactionKind,
{
    fn clone(&self) -> Self {
        Self::new_at_position(self).unwrap()
    }
}

impl<'txn, K> fmt::Debug for Cursor<'txn, K>
where
    K: TransactionKind,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("Cursor").finish()
    }
}

impl<'txn, K> Drop for Cursor<'txn, K>
where
    K: TransactionKind,
{
    fn drop(&mut self) {
        unsafe { ffi::mdbx_cursor_close(self.cursor) }
    }
}

unsafe fn slice_to_val(slice: Option<&[u8]>) -> ffi::MDBX_val {
    match slice {
        Some(slice) => ffi::MDBX_val {
            iov_len: slice.len(),
            iov_base: slice.as_ptr() as *mut c_void,
        },
        None => ffi::MDBX_val {
            iov_len: 0,
            iov_base: ptr::null_mut(),
        },
    }
}

unsafe impl<'txn> Send for Cursor<'txn, RO> {}

impl<'txn, K> IntoIterator for Cursor<'txn, K>
where
    K: TransactionKind,
{
    type Item = Result<(Bytes<'txn>, Bytes<'txn>)>;
    type IntoIter = IntoIter<'txn, K>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self, MDBX_NEXT, MDBX_NEXT)
    }
}

/// An iterator over the key/value pairs in an MDBX database.
#[derive(Debug)]
pub enum IntoIter<'txn, K>
where
    K: TransactionKind,
{
    /// An iterator that returns an error on every call to [Iter::next()].
    /// Cursor.iter*() creates an Iter of this type when MDBX returns an error
    /// on retrieval of a cursor.  Using this variant instead of returning
    /// an error makes Cursor.iter()* methods infallible, so consumers only
    /// need to check the result of Iter.next().
    Err(Error),

    /// An iterator that returns an Item on calls to [Iter::next()].
    /// The Item is a [Result], so this variant
    /// might still return an error, if retrieval of the key/value pair
    /// fails for some reason.
    Ok {
        /// The MDBX cursor with which to iterate.
        cursor: Cursor<'txn, K>,

        /// The first operation to perform when the consumer calls [Iter::next()].
        op: ffi::MDBX_cursor_op,

        /// The next and subsequent operations to perform.
        next_op: ffi::MDBX_cursor_op,

        _marker: PhantomData<fn(&'txn (), K)>,
    },
}

impl<'txn, K> IntoIter<'txn, K>
where
    K: TransactionKind,
{
    /// Creates a new iterator backed by the given cursor.
    fn new(cursor: Cursor<'txn, K>, op: ffi::MDBX_cursor_op, next_op: ffi::MDBX_cursor_op) -> Self {
        IntoIter::Ok {
            cursor,
            op,
            next_op,
            _marker: PhantomData,
        }
    }
}

impl<'txn, K> Iterator for IntoIter<'txn, K>
where
    K: TransactionKind,
{
    type Item = Result<(Bytes<'txn>, Bytes<'txn>)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ok {
                cursor,
                op,
                next_op,
                _marker,
            } => {
                let mut key = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let mut data = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let op = mem::replace(op, *next_op);
                unsafe {
                    match ffi::mdbx_cursor_get(cursor.cursor(), &mut key, &mut data, op) {
                        ffi::MDBX_SUCCESS => {
                            let txn = ffi::mdbx_cursor_txn(cursor.cursor());
                            let key = match freeze_bytes::<K>(txn, &key) {
                                Ok(v) => v,
                                Err(e) => return Some(Err(e)),
                            };
                            let data = match freeze_bytes::<K>(txn, &data) {
                                Ok(v) => v,
                                Err(e) => return Some(Err(e)),
                            };
                            Some(Ok((key, data)))
                        },
                        // EINVAL can occur when the cursor was previously seeked to a non-existent value,
                        // e.g. iter_from with a key greater than all values in the database.
                        ffi::MDBX_NOTFOUND | libc::ENODATA => None,
                        error => Some(Err(Error::from_err_code(error))),
                    }
                }
            },
            Self::Err(err) => Some(Err(*err)),
        }
    }
}

/// An iterator over the key/value pairs in an MDBX database.
#[derive(Debug)]
pub enum Iter<'txn, 'cur, K>
where
    K: TransactionKind,
{
    /// An iterator that returns an error on every call to [Iter::next()].
    /// Cursor.iter*() creates an Iter of this type when MDBX returns an error
    /// on retrieval of a cursor.  Using this variant instead of returning
    /// an error makes Cursor.iter()* methods infallible, so consumers only
    /// need to check the result of Iter.next().
    Err(Error),

    /// An iterator that returns an Item on calls to [Iter::next()].
    /// The Item is a [Result], so this variant
    /// might still return an error, if retrieval of the key/value pair
    /// fails for some reason.
    Ok {
        /// The MDBX cursor with which to iterate.
        cursor: &'cur mut Cursor<'txn, K>,

        /// The first operation to perform when the consumer calls [Iter::next()].
        op: ffi::MDBX_cursor_op,

        /// The next and subsequent operations to perform.
        next_op: ffi::MDBX_cursor_op,
    },
}

impl<'txn, 'cur, K> Iter<'txn, 'cur, K>
where
    K: TransactionKind,
{
    /// Creates a new iterator backed by the given cursor.
    fn new(cursor: &'cur mut Cursor<'txn, K>, op: ffi::MDBX_cursor_op, next_op: ffi::MDBX_cursor_op) -> Self {
        Iter::Ok {
            cursor,
            op,
            next_op,
        }
    }
}

impl<'txn, 'cur, K> Iterator for Iter<'txn, 'cur, K>
where
    K: TransactionKind,
{
    type Item = Result<(Bytes<'txn>, Bytes<'txn>)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Ok {
                cursor,
                op,
                next_op,
            } => {
                let mut key = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let mut data = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let op = mem::replace(op, *next_op);
                unsafe {
                    match ffi::mdbx_cursor_get(cursor.cursor(), &mut key, &mut data, op) {
                        ffi::MDBX_SUCCESS => {
                            let txn = ffi::mdbx_cursor_txn(cursor.cursor());
                            let key = match freeze_bytes::<K>(txn, &key) {
                                Ok(v) => v,
                                Err(e) => return Some(Err(e)),
                            };
                            let data = match freeze_bytes::<K>(txn, &data) {
                                Ok(v) => v,
                                Err(e) => return Some(Err(e)),
                            };
                            Some(Ok((key, data)))
                        },
                        // EINVAL can occur when the cursor was previously seeked to a non-existent value,
                        // e.g. iter_from with a key greater than all values in the database.
                        ffi::MDBX_NOTFOUND | libc::ENODATA => None,
                        error => Some(Err(Error::from_err_code(error))),
                    }
                }
            },
            &mut Iter::Err(err) => Some(Err(err)),
        }
    }
}

/// An iterator over the keys and duplicate values in an MDBX database.
///
/// The yielded items of the iterator are themselves iterators over the duplicate values for a
/// specific key.
pub enum IterDup<'txn, 'cur, K>
where
    K: TransactionKind,
{
    /// An iterator that returns an error on every call to Iter.next().
    /// Cursor.iter*() creates an Iter of this type when MDBX returns an error
    /// on retrieval of a cursor.  Using this variant instead of returning
    /// an error makes Cursor.iter()* methods infallible, so consumers only
    /// need to check the result of Iter.next().
    Err(Error),

    /// An iterator that returns an Item on calls to Iter.next().
    /// The Item is a Result<(&'txn [u8], &'txn [u8])>, so this variant
    /// might still return an error, if retrieval of the key/value pair
    /// fails for some reason.
    Ok {
        /// The MDBX cursor with which to iterate.
        cursor: &'cur mut Cursor<'txn, K>,

        /// The first operation to perform when the consumer calls Iter.next().
        op: c_uint,
    },
}

impl<'txn, 'cur, K> IterDup<'txn, 'cur, K>
where
    K: TransactionKind,
{
    /// Creates a new iterator backed by the given cursor.
    fn new(cursor: &'cur mut Cursor<'txn, K>, op: c_uint) -> Self {
        IterDup::Ok {
            cursor,
            op,
        }
    }
}

impl<'txn, 'cur, K> fmt::Debug for IterDup<'txn, 'cur, K>
where
    K: TransactionKind,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("IterDup").finish()
    }
}

impl<'txn, 'cur, K> Iterator for IterDup<'txn, 'cur, K>
where
    K: TransactionKind,
{
    type Item = IntoIter<'txn, K>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IterDup::Ok {
                cursor,
                op,
            } => {
                let mut key = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let mut data = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let op = mem::replace(op, ffi::MDBX_NEXT_NODUP);
                let err_code = unsafe { ffi::mdbx_cursor_get(cursor.cursor(), &mut key, &mut data, op) };

                if err_code == ffi::MDBX_SUCCESS {
                    Some(IntoIter::new(
                        Cursor::new_at_position(&**cursor).unwrap(),
                        ffi::MDBX_GET_CURRENT,
                        ffi::MDBX_NEXT_DUP,
                    ))
                } else {
                    None
                }
            },
            IterDup::Err(err) => Some(IntoIter::Err(*err)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::environment::*;
    use tempfile::tempdir;

    #[test]
    fn test_get() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();

        assert_eq!(None, db.cursor().unwrap().first().unwrap());

        db.put(b"key1", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key3", b"val3", WriteFlags::empty()).unwrap();

        let mut cursor = db.cursor().unwrap();
        assert_eq!(cursor.first().unwrap().unwrap(), (b"key1".into(), b"val1".into()));
        assert_eq!(cursor.get_current().unwrap().unwrap(), (b"key1".into(), b"val1".into()));
        assert_eq!(cursor.next().unwrap().unwrap(), (b"key2".into(), b"val2".into()));
        assert_eq!(cursor.prev().unwrap().unwrap(), (b"key1".into(), b"val1".into()));
        assert_eq!(cursor.last().unwrap().unwrap(), (b"key3".into(), b"val3".into()));
        assert_eq!(cursor.set(b"key1").unwrap().unwrap(), b"val1");
        // assert_eq!((Some(b"key3".into()), b"val3".into()), cursor.get(Some(b"key3"), None, MDBX_SET_KEY).unwrap());
        assert_eq!(cursor.set_range(b"key2\0").unwrap().unwrap(), (b"key3".into(), b"val3".into()));
    }

    #[test]
    fn test_get_dup() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.create_db(None, DatabaseFlags::DUP_SORT).unwrap();
        db.put(b"key1", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key1", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key1", b"val3", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val3", WriteFlags::empty()).unwrap();

        let mut cursor = db.cursor().unwrap();
        assert_eq!(cursor.first().unwrap().unwrap(), (b"key1".into(), b"val1".into()));
        assert_eq!(cursor.first_dup().unwrap().unwrap(), b"val1");
        assert_eq!(cursor.get_current().unwrap().unwrap(), (b"key1".into(), b"val1".into()));
        assert_eq!(cursor.next_nodup().unwrap().unwrap(), (b"key2".into(), b"val1".into()));
        assert_eq!(cursor.next_dup().unwrap().unwrap(), (b"key2".into(), b"val2".into()));
        assert_eq!(cursor.next_dup().unwrap().unwrap(), (b"key2".into(), b"val3".into()));
        assert_eq!(cursor.next_dup().unwrap(), None);
        assert_eq!(cursor.prev_dup().unwrap().unwrap(), (b"key2".into(), b"val2".into()));
        assert_eq!(cursor.last_dup().unwrap().unwrap(), b"val3");
        assert_eq!(cursor.prev_nodup().unwrap().unwrap(), (b"key1".into(), b"val3".into()));
        assert_eq!(cursor.set(b"key1").unwrap().unwrap(), b"val1");
        assert_eq!(cursor.set(b"key2").unwrap().unwrap(), b"val1");
        assert_eq!(cursor.set_range(b"key1\0").unwrap().unwrap(), (b"key2".into(), b"val1".into()));
        assert_eq!(cursor.get_both(b"key1", b"val3").unwrap().unwrap(), b"val3");
        assert_eq!(cursor.get_both_range(b"key2", b"val").unwrap().unwrap(), b"val1");
    }

    #[test]
    fn test_get_dupfixed() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.create_db(None, DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED).unwrap();
        db.put(b"key1", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key1", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key1", b"val3", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val4", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val5", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val6", WriteFlags::empty()).unwrap();

        let mut cursor = db.cursor().unwrap();
        assert_eq!(cursor.first().unwrap().unwrap(), (b"key1".into(), b"val1".into()));
        assert_eq!(cursor.get_multiple().unwrap().unwrap(), b"val1val2val3");
        assert_eq!(cursor.next_multiple().unwrap(), None);
    }

    #[test]
    fn test_iter() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let items: Vec<(Bytes, Bytes)> = vec![
            (b"key1".into(), b"val1".into()),
            (b"key2".into(), b"val2".into()),
            (b"key3".into(), b"val3".into()),
            (b"key5".into(), b"val5".into()),
        ];

        {
            let txn = env.begin_rw_txn().unwrap();
            let db = txn.open_db(None).unwrap();
            for (key, data) in &items {
                db.put(key, data, WriteFlags::empty()).unwrap();
            }
            assert!(!txn.commit().unwrap());
        }

        let txn = env.begin_ro_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        let mut cursor = db.cursor().unwrap();

        // Because Result implements FromIterator, we can collect the iterator
        // of items of type Result<_, E> into a Result<Vec<_, E>> by specifying
        // the collection type via the turbofish syntax.
        assert_eq!(items, cursor.iter().collect::<Result<Vec<_>>>().unwrap());

        // Alternately, we can collect it into an appropriately typed variable.
        let retr: Result<Vec<_>> = cursor.iter_start().collect();
        assert_eq!(items, retr.unwrap());

        cursor.set(b"key2").unwrap();
        assert_eq!(
            items.clone().into_iter().skip(2).collect::<Vec<_>>(),
            cursor.iter().collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(items, cursor.iter_start().collect::<Result<Vec<_>>>().unwrap());

        assert_eq!(
            items.clone().into_iter().skip(1).collect::<Vec<_>>(),
            cursor.iter_from(b"key2").collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(
            items.clone().into_iter().skip(3).collect::<Vec<_>>(),
            cursor.iter_from(b"key4").collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(
            vec!().into_iter().collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_from(b"key6").collect::<Result<Vec<_>>>().unwrap()
        );
    }

    #[test]
    fn test_iter_empty_database() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let txn = env.begin_ro_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        let mut cursor = db.cursor().unwrap();

        assert_eq!(None, cursor.iter().next());
        assert_eq!(None, cursor.iter_start().next());
        assert_eq!(None, cursor.iter_from(b"foo").next());
    }

    #[test]
    fn test_iter_empty_dup_database() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        txn.create_db(None, DatabaseFlags::DUP_SORT).unwrap();
        txn.commit().unwrap();

        let txn = env.begin_ro_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        let mut cursor = db.cursor().unwrap();

        assert_eq!(None, cursor.iter().next());
        assert_eq!(None, cursor.iter_start().next());
        assert_eq!(None, cursor.iter_from(b"foo").next());
        assert_eq!(None, cursor.iter_from(b"foo").next());
        assert_eq!(None, cursor.iter_dup().flatten().next());
        assert_eq!(None, cursor.iter_dup_start().flatten().next());
        assert_eq!(None, cursor.iter_dup_from(b"foo").flatten().next());
        assert_eq!(None, cursor.iter_dup_of(b"foo").next());
    }

    #[test]
    fn test_iter_dup() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        txn.create_db(None, DatabaseFlags::DUP_SORT).unwrap();
        txn.commit().unwrap();

        let items: Vec<(Bytes, Bytes)> = vec![
            (b"a".into(), b"1".into()),
            (b"a".into(), b"2".into()),
            (b"a".into(), b"3".into()),
            (b"b".into(), b"1".into()),
            (b"b".into(), b"2".into()),
            (b"b".into(), b"3".into()),
            (b"c".into(), b"1".into()),
            (b"c".into(), b"2".into()),
            (b"c".into(), b"3".into()),
            (b"e".into(), b"1".into()),
            (b"e".into(), b"2".into()),
            (b"e".into(), b"3".into()),
        ];

        {
            let txn = env.begin_rw_txn().unwrap();
            let db = txn.open_db(None).unwrap();
            for (key, data) in &items {
                db.put(key, data, WriteFlags::empty()).unwrap();
            }
            txn.commit().unwrap();
        }

        let txn = env.begin_ro_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        let mut cursor = db.cursor().unwrap();
        assert_eq!(items, cursor.iter_dup().flatten().collect::<Result<Vec<_>>>().unwrap());

        cursor.set(b"b").unwrap();
        assert_eq!(
            items.clone().into_iter().skip(4).collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_dup().flatten().collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(items, cursor.iter_dup_start().flatten().collect::<Result<Vec<(Bytes, Bytes)>>>().unwrap());

        assert_eq!(
            items.clone().into_iter().skip(3).collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_dup_from(b"b").flatten().collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(
            items.clone().into_iter().skip(3).collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_dup_from(b"ab").flatten().collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(
            items.clone().into_iter().skip(9).collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_dup_from(b"d").flatten().collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(
            vec!().into_iter().collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_dup_from(b"f").flatten().collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(
            items.clone().into_iter().skip(3).take(3).collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_dup_of(b"b").collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(0, cursor.iter_dup_of(b"foo").count());
    }

    #[test]
    fn test_iter_del_get() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let items: Vec<(Bytes, Bytes)> = vec![(b"a".into(), b"1".into()), (b"b".into(), b"2".into())];
        let r: Vec<(_, _)> = Vec::new();
        {
            let txn = env.begin_rw_txn().unwrap();
            let db = txn.create_db(None, DatabaseFlags::DUP_SORT).unwrap();
            assert_eq!(r, db.cursor().unwrap().iter_dup_of(b"a").collect::<Result<Vec<_>>>().unwrap());
            txn.commit().unwrap();
        }

        {
            let txn = env.begin_rw_txn().unwrap();
            let db = txn.open_db(None).unwrap();
            for (key, data) in &items {
                db.put(key, data, WriteFlags::empty()).unwrap();
            }
            txn.commit().unwrap();
        }

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        let mut cursor = db.cursor().unwrap();
        assert_eq!(items, cursor.iter_dup().flatten().collect::<Result<Vec<_>>>().unwrap());

        assert_eq!(
            items.clone().into_iter().take(1).collect::<Vec<(Bytes, Bytes)>>(),
            cursor.iter_dup_of(b"a").collect::<Result<Vec<_>>>().unwrap()
        );

        assert_eq!(cursor.set(b"a").unwrap().unwrap(), b"1");

        cursor.del(WriteFlags::empty()).unwrap();

        assert_eq!(r, cursor.iter_dup_of(b"a").collect::<Result<Vec<_>>>().unwrap());
    }

    #[test]
    fn test_put_del() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        let mut cursor = db.cursor().unwrap();

        cursor.put(b"key1", b"val1", WriteFlags::empty()).unwrap();
        cursor.put(b"key2", b"val2", WriteFlags::empty()).unwrap();
        cursor.put(b"key3", b"val3", WriteFlags::empty()).unwrap();

        assert_eq!(cursor.get_current().unwrap().unwrap(), (b"key3".into(), b"val3".into()));

        cursor.del(WriteFlags::empty()).unwrap();
        assert_eq!(cursor.last().unwrap().unwrap(), (b"key2".into(), b"val2".into()));
    }
}
