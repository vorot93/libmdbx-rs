use crate::{
    DatabaseKind, Decodable, Transaction,
    database::TxnPtr,
    error::{Error, Result, mdbx_result},
    flags::*,
    mdbx_try_optional,
    table::Table,
    transaction::{RW, TransactionKind, txn_execute},
};
use ffi::{
    MDBX_FIRST, MDBX_FIRST_DUP, MDBX_GET_BOTH, MDBX_GET_BOTH_RANGE, MDBX_GET_CURRENT,
    MDBX_GET_MULTIPLE, MDBX_LAST, MDBX_LAST_DUP, MDBX_NEXT, MDBX_NEXT_DUP, MDBX_NEXT_MULTIPLE,
    MDBX_NEXT_NODUP, MDBX_PREV, MDBX_PREV_DUP, MDBX_PREV_MULTIPLE, MDBX_PREV_NODUP, MDBX_SET,
    MDBX_SET_KEY, MDBX_SET_LOWERBOUND, MDBX_SET_RANGE, MDBX_TO_KEY_LESSER_OR_EQUAL, MDBX_cursor_op,
};
use libc::c_void;
use parking_lot::Mutex;
use std::{
    borrow::Cow, fmt, iter::FusedIterator, marker::PhantomData, mem, ptr, result, slice, sync::Arc,
};

#[derive(Copy, Clone, Debug)]
pub struct CursorPtr(pub *mut ffi::MDBX_cursor);
unsafe impl Send for CursorPtr {}

/// A cursor for navigating the items within a table.
///
/// A cursor, and any value it decodes without copying, cannot outlive the
/// transaction it was opened in:
///
/// ```compile_fail,E0597
/// # use libmdbx::{Database, NoWriteMap};
/// let db = Database::<NoWriteMap>::open("db").unwrap();
/// let cursor;
/// {
///     let txn = db.begin_ro_txn().unwrap();
///     let table = txn.open_table(None).unwrap();
///     cursor = txn.cursor(&table).unwrap();
/// }
/// drop(cursor);
/// ```
pub struct Cursor<'txn, K>
where
    K: TransactionKind,
{
    txn: Arc<Mutex<TxnPtr>>,
    cursor: CursorPtr,
    _marker: PhantomData<fn() -> (&'txn (), K)>,
}

impl<'txn, K> Cursor<'txn, K>
where
    K: TransactionKind,
{
    pub(crate) fn new<E: DatabaseKind>(
        txn: &'txn Transaction<K, E>,
        table: &Table<'txn>,
    ) -> Result<Self> {
        let mut cursor: *mut ffi::MDBX_cursor = ptr::null_mut();

        let txn = txn.txn_mutex();
        unsafe {
            mdbx_result(txn_execute(&txn, |txn| {
                ffi::mdbx_cursor_open(txn, table.dbi(), &mut cursor)
            }))?;
        }
        Ok(Self {
            txn,
            cursor: CursorPtr(cursor),
            _marker: PhantomData,
        })
    }

    fn new_at_position(other: &Self) -> Result<Self> {
        unsafe {
            let cursor = ffi::mdbx_cursor_create(ptr::null_mut());
            if cursor.is_null() {
                return Err(Error::Other(libc::ENOMEM));
            }
            let s = Self {
                txn: other.txn.clone(),
                cursor: CursorPtr(cursor),
                _marker: PhantomData,
            };
            mdbx_result(ffi::mdbx_cursor_copy(other.cursor().0, cursor))?;
            Ok(s)
        }
    }

    /// Returns a raw pointer to the underlying MDBX cursor.
    ///
    /// The caller **must** ensure that the pointer is not used after the
    /// lifetime of the cursor.
    pub fn cursor(&self) -> CursorPtr {
        self.cursor
    }

    /// Retrieves a key/data pair from the cursor, always decoding the key and
    /// value at the cursor position after the op.
    fn get<Key, Value>(
        &self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: MDBX_cursor_op,
    ) -> Result<(Key, Value, bool)>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        unsafe {
            let mut key_val = slice_to_val(key);
            let mut data_val = slice_to_val(data);
            txn_execute(&self.txn, |txn| {
                let v = mdbx_result(ffi::mdbx_cursor_get(
                    self.cursor.0,
                    &mut key_val,
                    &mut data_val,
                    op,
                ))?;
                let key = Key::decode_val::<K>(txn, &key_val)?;
                let value = Value::decode_val::<K>(txn, &data_val)?;
                Ok((key, value, v))
            })
        }
    }

    fn get_value<Value>(
        &mut self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: MDBX_cursor_op,
    ) -> Result<Option<Value>>
    where
        Value: Decodable<'txn>,
    {
        let (_, v, _) = mdbx_try_optional!(self.get::<(), Value>(key, data, op));

        Ok(Some(v))
    }

    fn get_full<Key, Value>(
        &mut self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: MDBX_cursor_op,
    ) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let (k, v, _) = mdbx_try_optional!(self.get(key, data, op));

        Ok(Some((k, v)))
    }

    /// Position at first key/data item.
    pub fn first<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_FIRST)
    }

    /// [TableFlags::DUP_SORT]-only: Position at first data item of current key.
    pub fn first_dup<Value>(&mut self) -> Result<Option<Value>>
    where
        Value: Decodable<'txn>,
    {
        self.get_value(None, None, MDBX_FIRST_DUP)
    }

    /// [TableFlags::DUP_SORT]-only: Position at key/data pair.
    pub fn get_both<Value>(&mut self, k: &[u8], v: &[u8]) -> Result<Option<Value>>
    where
        Value: Decodable<'txn>,
    {
        self.get_value(Some(k), Some(v), MDBX_GET_BOTH)
    }

    /// [TableFlags::DUP_SORT]-only: Position at given key and at first data greater than or equal to specified data.
    pub fn get_both_range<Value>(&mut self, k: &[u8], v: &[u8]) -> Result<Option<Value>>
    where
        Value: Decodable<'txn>,
    {
        self.get_value(Some(k), Some(v), MDBX_GET_BOTH_RANGE)
    }

    /// Return key/data at current cursor position.
    pub fn get_current<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_GET_CURRENT)
    }

    /// DupFixed-only: Return up to a page of duplicate data items from current cursor position.
    /// Move cursor to prepare for [Self::next_multiple()].
    pub fn get_multiple<Value>(&mut self) -> Result<Option<Value>>
    where
        Value: Decodable<'txn>,
    {
        self.get_value(None, None, MDBX_GET_MULTIPLE)
    }

    /// Position at last key/data item.
    pub fn last<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_LAST)
    }

    /// DupSort-only: Position at last data item of current key.
    pub fn last_dup<Value>(&mut self) -> Result<Option<Value>>
    where
        Value: Decodable<'txn>,
    {
        self.get_value(None, None, MDBX_LAST_DUP)
    }

    /// Position at next data item
    #[allow(clippy::should_implement_trait)]
    pub fn next<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_NEXT)
    }

    /// [TableFlags::DUP_SORT]-only: Position at next data item of current key.
    pub fn next_dup<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_NEXT_DUP)
    }

    /// [TableFlags::DUP_FIXED]-only: Return up to a page of duplicate data items from next cursor position. Move cursor to prepare for MDBX_NEXT_MULTIPLE.
    pub fn next_multiple<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_NEXT_MULTIPLE)
    }

    /// Position at first data item of next key.
    pub fn next_nodup<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_NEXT_NODUP)
    }

    /// Position at previous data item.
    pub fn prev<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_PREV)
    }

    /// [TableFlags::DUP_SORT]-only: Position at previous data item of current key.
    pub fn prev_dup<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_PREV_DUP)
    }

    /// Position at last data item of previous key.
    pub fn prev_nodup<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_PREV_NODUP)
    }

    /// Position at specified key.
    pub fn set<Value>(&mut self, key: &[u8]) -> Result<Option<Value>>
    where
        Value: Decodable<'txn>,
    {
        self.get_value(Some(key), None, MDBX_SET)
    }

    /// Position at specified key, return both key and data.
    pub fn set_key<Key, Value>(&mut self, key: &[u8]) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(Some(key), None, MDBX_SET_KEY)
    }

    /// Position at first key greater than or equal to specified key.
    pub fn set_range<Key, Value>(&mut self, key: &[u8]) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(Some(key), None, MDBX_SET_RANGE)
    }

    /// Position at the largest key less than or equal to the specified key.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]) the
    /// cursor is positioned at the key level, i.e. the returned pair may be
    /// any of the key's duplicates.
    ///
    /// Note that this is *not* a direct wrapper for the libmdbx
    /// `MDBX_SET_UPPERBOUND` op, which positions at the first key *greater*
    /// than the given one. This method uses `MDBX_TO_KEY_LESSER_OR_EQUAL`,
    /// whose semantics match "upper bound" in the `largest key <= given`
    /// sense.
    pub fn set_upperbound<Key, Value>(&mut self, key: &[u8]) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(Some(key), None, MDBX_TO_KEY_LESSER_OR_EQUAL)
    }

    /// [TableFlags::DUP_FIXED]-only: Position at previous page and return up to a page of duplicate data items.
    pub fn prev_multiple<Key, Value>(&mut self) -> Result<Option<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        self.get_full(None, None, MDBX_PREV_MULTIPLE)
    }

    /// Position at first key-value pair greater than or equal to specified, return both key and data, and the return code depends on a exact match.
    ///
    /// For non DupSort-ed collections this works the same as [Self::set_range()], but returns [false] if key found exactly and [true] if greater key was found.
    ///
    /// For DupSort-ed a data value is taken into account for duplicates, i.e. for a pairs/tuples of a key and an each data value of duplicates.
    /// Returns [false] if key-value pair found exactly and [true] if the next pair was returned.
    pub fn set_lowerbound<Key, Value>(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
    ) -> Result<Option<(bool, Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let (k, v, found) = mdbx_try_optional!(self.get(Some(key), value, MDBX_SET_LOWERBOUND));

        Ok(Some((found, k, v)))
    }

    /// Iterate over table items. The iterator will begin with item next
    /// after the cursor, and continue until the end of the table. For new
    /// cursors, the iterator will begin with the first item in the table.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn iter<Key, Value>(&mut self) -> Iter<'txn, '_, K, Key, Value>
    where
        Self: Sized,
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        Iter::new(self, ffi::MDBX_NEXT, ffi::MDBX_NEXT, MDBX_LAST, MDBX_PREV)
    }

    /// Iterate over table items starting from the beginning of the table.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn iter_start<Key, Value>(&mut self) -> Iter<'txn, '_, K, Key, Value>
    where
        Self: Sized,
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        Iter::new(self, ffi::MDBX_FIRST, ffi::MDBX_NEXT, MDBX_LAST, MDBX_PREV)
    }

    /// Iterate over table items starting from the beginning of the table.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn into_iter_start<Key, Value>(self) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        IntoIter::new(
            self,
            ffi::MDBX_FIRST,
            ffi::MDBX_NEXT,
            MDBX_LAST,
            MDBX_PREV,
            None,
        )
    }

    /// Iterate over table items starting from the given key.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn iter_from<Key, Value>(&mut self, key: &[u8]) -> Iter<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let res: Result<Option<((), ())>> = self.set_range(key);
        if let Err(error) = res {
            return Iter::Err(Some(error));
        };
        Iter::new(
            self,
            ffi::MDBX_GET_CURRENT,
            ffi::MDBX_NEXT,
            MDBX_LAST,
            MDBX_PREV,
        )
    }

    /// Iterate over table items starting from the given key.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn into_iter_from<Key, Value>(mut self, key: &[u8]) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let res: Result<Option<((), ())>> = self.set_range(key);
        if let Err(error) = res {
            return IntoIter::Err(Some(error));
        };
        IntoIter::new(
            self,
            ffi::MDBX_GET_CURRENT,
            ffi::MDBX_NEXT,
            MDBX_LAST,
            MDBX_PREV,
            None,
        )
    }

    /// Iterate over table items backwards, starting from the largest key
    /// less than or equal to the given key, down to the first key.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the previous key.
    ///
    /// The iteration domain is bounded by the given key, so reversing the
    /// iterator (e.g. via [.rev()](Iterator::rev)) yields an
    /// ascending iteration over the keys less than or equal to it.
    pub fn into_iter_back_from<Key, Value>(mut self, key: &[u8]) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let res: Result<Option<((), ())>> = self.set_upperbound(key);
        match res {
            Err(error) => return IntoIter::Err(Some(error)),
            // No key <= `key`: the iteration domain is empty. The failed seek
            // may leave the cursor on a valid item, so park it on the last
            // item where MDBX_NEXT is a stable NOTFOUND.
            Ok(None) => {
                let _: Result<Option<((), ())>> = self.last();
                return IntoIter::new(
                    self,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    None,
                );
            }
            Ok(Some(_)) => (),
        };
        IntoIter::new(
            self,
            ffi::MDBX_GET_CURRENT,
            ffi::MDBX_PREV,
            MDBX_FIRST,
            MDBX_NEXT,
            Some(key),
        )
    }

    /// Iterate over table items backwards, starting from the last key.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the previous key.
    pub fn into_iter_back_start<Key, Value>(self) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        IntoIter::new(self, MDBX_LAST, MDBX_PREV, MDBX_FIRST, MDBX_NEXT, None)
    }

    /// Iterate over duplicate table items. The iterator will begin with the
    /// item next after the cursor, and continue until the end of the table.
    /// Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup<Key, Value>(&mut self) -> IterDup<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        IterDup::new(self, ffi::MDBX_NEXT)
    }

    /// Iterate over duplicate table items starting from the beginning of the
    /// table. Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup_start<Key, Value>(&mut self) -> IterDup<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        IterDup::new(self, ffi::MDBX_FIRST)
    }

    /// Iterate over duplicate items in the table starting from the given
    /// key. Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup_from<Key, Value>(&mut self, key: &[u8]) -> IterDup<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let res: Result<Option<((), ())>> = self.set_range(key);
        if let Err(error) = res {
            return IterDup::Err(Some(error));
        };
        IterDup::new(self, ffi::MDBX_GET_CURRENT)
    }

    /// Iterate over the duplicates of the item in the table with the given key.
    pub fn iter_dup_of<Key, Value>(&mut self, key: &[u8]) -> Iter<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let res: Result<Option<()>> = self.set(key);
        match res {
            Ok(Some(_)) => (),
            Ok(None) => {
                let _: Result<Option<((), ())>> = self.last();
                return Iter::new(
                    self,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                );
            }
            Err(error) => return Iter::Err(Some(error)),
        };
        Iter::new(
            self,
            ffi::MDBX_GET_CURRENT,
            ffi::MDBX_NEXT_DUP,
            MDBX_LAST_DUP,
            ffi::MDBX_PREV_DUP,
        )
    }

    /// Iterate over the duplicates of the item in the table with the given key.
    pub fn into_iter_dup_of<Key, Value>(mut self, key: &[u8]) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let res: Result<Option<()>> = self.set(key);
        match res {
            Ok(Some(_)) => (),
            Ok(None) => {
                let _: Result<Option<((), ())>> = self.last();
                return IntoIter::new(
                    self,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    ffi::MDBX_NEXT,
                    None,
                );
            }
            Err(error) => return IntoIter::Err(Some(error)),
        };
        IntoIter::new(
            self,
            ffi::MDBX_GET_CURRENT,
            ffi::MDBX_NEXT_DUP,
            MDBX_LAST_DUP,
            ffi::MDBX_PREV_DUP,
            None,
        )
    }
}

impl Cursor<'_, RW> {
    /// Puts a key/data pair into the table. The cursor will be positioned at
    /// the new data item, or on failure usually near it.
    pub fn put(&mut self, key: &[u8], data: &[u8], flags: WriteFlags) -> Result<()> {
        let key_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: key.len(),
            iov_base: key.as_ptr() as *mut c_void,
        };
        let mut data_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: data.len(),
            iov_base: data.as_ptr() as *mut c_void,
        };
        mdbx_result(unsafe {
            txn_execute(&self.txn, |_| {
                ffi::mdbx_cursor_put(self.cursor.0, &key_val, &mut data_val, c_enum(flags.bits()))
            })
        })?;

        Ok(())
    }

    /// Deletes the current key/data pair.
    ///
    /// ### Flags
    ///
    /// [WriteFlags::NO_DUP_DATA] may be used to delete all data items for the
    /// current key, if the table was opened with [TableFlags::DUP_SORT].
    pub fn del(&mut self, flags: WriteFlags) -> Result<()> {
        mdbx_result(unsafe {
            txn_execute(&self.txn, |_| {
                ffi::mdbx_cursor_del(self.cursor.0, c_enum(flags.bits()))
            })
        })?;

        Ok(())
    }
}

impl<K> Clone for Cursor<'_, K>
where
    K: TransactionKind,
{
    fn clone(&self) -> Self {
        txn_execute(&self.txn, |_| Self::new_at_position(self).unwrap())
    }
}

impl<K> fmt::Debug for Cursor<'_, K>
where
    K: TransactionKind,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("Cursor").finish()
    }
}

impl<K> Drop for Cursor<'_, K>
where
    K: TransactionKind,
{
    fn drop(&mut self) {
        txn_execute(&self.txn, |_| unsafe {
            ffi::mdbx_cursor_close(self.cursor.0)
        })
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

/// Borrows the raw bytes of an `MDBX_val` filled in by libmdbx.
unsafe fn val_to_slice(val: &ffi::MDBX_val) -> &[u8] {
    if val.iov_base.is_null() {
        &[]
    } else {
        unsafe { slice::from_raw_parts(val.iov_base as *const u8, val.iov_len) }
    }
}

/// Outcome of one cursor fetch driving an iterator step.
///
/// `PastBound` is only produced when a `bound` was given and the fetched key
/// lies beyond it: the cursor is then parked outside the iterator's domain,
/// so the whole iterator must hard-stop rather than let either direction
/// fetch from the parked position.
enum Fetched<T> {
    /// Yield this item (possibly an error item).
    Item(T),
    /// End of iteration (`MDBX_NOTFOUND`/`MDBX_ENODATA`).
    Exhausted,
    /// The fetched key is beyond the inclusive bound.
    PastBound,
}

impl<T> Fetched<T> {
    /// The yielded item, if this step produced one. Both end states end
    /// iteration for the caller.
    fn item(self) -> Option<T> {
        match self {
            Fetched::Item(item) => Some(item),
            _ => None,
        }
    }
}

/// Runs a cursor get op and decodes the pair at the resulting position.
///
/// Maps `MDBX_NOTFOUND`/`MDBX_ENODATA` to [Fetched::Exhausted] (end of
/// iteration), any other failure to an error item, and, when `bound` is
/// given, a successfully fetched key strictly greater than `bound` to
/// [Fetched::PastBound], so the reversed element order of a bounded
/// iterator stays within its domain when driven from the back.
fn fetch_op<'txn, K, Key, Value>(
    cursor: &Cursor<'txn, K>,
    op: MDBX_cursor_op,
    bound: Option<&[u8]>,
) -> Fetched<Result<(Key, Value)>>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    let mut key = ffi::MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    let mut data = ffi::MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    unsafe {
        txn_execute(&cursor.txn, |txn| {
            match ffi::mdbx_cursor_get(cursor.cursor.0, &mut key, &mut data, op) {
                ffi::MDBX_SUCCESS => {
                    if bound.is_some_and(|b| val_to_slice(&key) > b) {
                        return Fetched::PastBound;
                    }
                    let key = match Key::decode_val::<K>(txn, &key) {
                        Ok(v) => v,
                        Err(e) => return Fetched::Item(Err(e)),
                    };
                    let data = match Value::decode_val::<K>(txn, &data) {
                        Ok(v) => v,
                        Err(e) => return Fetched::Item(Err(e)),
                    };
                    Fetched::Item(Ok((key, data)))
                }
                // MDBX_ENODATA can occur when the cursor was previously seeked to a non-existent value,
                // e.g. iter_from with a key greater than all values in the table.
                ffi::MDBX_NOTFOUND | ffi::MDBX_ENODATA => Fetched::Exhausted,
                error => Fetched::Item(Err(Error::from_err_code(error))),
            }
        })
    }
}

impl<'txn, K> IntoIterator for Cursor<'txn, K>
where
    K: TransactionKind,
{
    type Item = Result<(Cow<'txn, [u8]>, Cow<'txn, [u8]>)>;
    type IntoIter = IntoIter<'txn, K, Cow<'txn, [u8]>, Cow<'txn, [u8]>>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self, MDBX_NEXT, MDBX_NEXT, MDBX_LAST, MDBX_PREV, None)
    }
}

/// An iterator over the key/value pairs in an MDBX table.
///
/// This iterator implements [DoubleEndedIterator], with both directions
/// backed by a single MDBX cursor. Exhausting either direction is stable
/// (MDBX cursor ops don't wrap), but interleaving [Iterator::next()] and
/// [DoubleEndedIterator::next_back()] past the point where the two
/// directions meet can yield the middle items twice. Bounded iterators
/// (see [Cursor::into_iter_back_from()]) are the exception: once the back
/// direction hits the bound, the whole iterator stops, so keys beyond the
/// bound are never yielded from either direction.
///
/// Like its cursor, the iterator cannot outlive its transaction:
///
/// ```compile_fail,E0597
/// # use libmdbx::{Database, NoWriteMap};
/// let db = Database::<NoWriteMap>::open("db").unwrap();
/// let iter;
/// {
///     let txn = db.begin_ro_txn().unwrap();
///     let table = txn.open_table(None).unwrap();
///     iter = txn.cursor(&table).unwrap().into_iter();
/// }
/// drop(iter);
/// ```
#[derive(Debug)]
pub enum IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    /// An iterator that yields a single error item on the first call to
    /// [IntoIter::next()], then ends. Created when the initial seek (e.g.
    /// `into_iter_from`) failed; yields the error once.
    Err(Option<Error>),

    /// An iterator that returns an Item on calls to [IntoIter::next()].
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

        /// The first operation to perform when the consumer calls
        /// [DoubleEndedIterator::next_back()]. Subsequent calls use
        /// `back_next_op`.
        back_op: ffi::MDBX_cursor_op,

        /// The second and subsequent operations to perform when the consumer
        /// calls [DoubleEndedIterator::next_back()].
        back_next_op: ffi::MDBX_cursor_op,

        /// Inclusive upper bound on the iterated keys, enforced in the back
        /// direction. Only set by [Cursor::into_iter_back_from()].
        bound: Option<Vec<u8>>,

        /// Set when a fetch crossed `bound`: the cursor is then parked
        /// outside the iteration domain, so both directions stop yielding.
        done: bool,

        _marker: PhantomData<fn() -> (&'txn (), K, Key, Value)>,
    },
}

impl<'txn, K, Key, Value> IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    /// Creates a new iterator backed by the given cursor.
    ///
    /// `op`/`next_op` drive [Iterator::next()] and `back_op`/`back_next_op`
    /// drive [DoubleEndedIterator::next_back()]. `bound`, when set, keeps
    /// keys fetched from the back direction within `keys <= bound`.
    fn new(
        cursor: Cursor<'txn, K>,
        op: ffi::MDBX_cursor_op,
        next_op: ffi::MDBX_cursor_op,
        back_op: ffi::MDBX_cursor_op,
        back_next_op: ffi::MDBX_cursor_op,
        bound: Option<&[u8]>,
    ) -> Self {
        IntoIter::Ok {
            cursor,
            op,
            next_op,
            back_op,
            back_next_op,
            bound: bound.map(|b| b.to_vec()),
            done: false,
            _marker: PhantomData,
        }
    }
}

impl<'txn, K, Key, Value> Iterator for IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    type Item = Result<(Key, Value)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ok {
                cursor,
                op,
                next_op,
                done,
                ..
            } => {
                if *done {
                    return None;
                }
                match fetch_op(cursor, mem::replace(op, *next_op), None) {
                    Fetched::Item(item) => Some(item),
                    // Unreachable in the front direction (no bound is
                    // passed), but stop hard if it ever happens.
                    Fetched::PastBound => {
                        *done = true;
                        None
                    }
                    Fetched::Exhausted => None,
                }
            }
            Self::Err(err) => err.take().map(Err),
        }
    }
}

impl<'txn, K, Key, Value> DoubleEndedIterator for IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ok {
                cursor,
                back_op,
                back_next_op,
                bound,
                done,
                ..
            } => {
                if *done {
                    return None;
                }
                match fetch_op(
                    cursor,
                    mem::replace(back_op, *back_next_op),
                    bound.as_deref(),
                ) {
                    Fetched::Item(item) => Some(item),
                    // The cursor is parked beyond the bound; fetching from it
                    // in either direction could leak out-of-domain keys.
                    Fetched::PastBound => {
                        *done = true;
                        None
                    }
                    Fetched::Exhausted => None,
                }
            }
            Self::Err(err) => err.take().map(Err),
        }
    }
}

impl<'txn, K, Key, Value> FusedIterator for IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
}

/// An iterator over the key/value pairs in an MDBX table.
///
/// This iterator implements [DoubleEndedIterator], with both directions
/// backed by a single MDBX cursor. Exhausting either direction is stable
/// (MDBX cursor ops don't wrap), but interleaving [Iterator::next()] and
/// [DoubleEndedIterator::next_back()] past the point where the two
/// directions meet can yield the middle items twice.
#[derive(Debug)]
pub enum Iter<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    /// An iterator that yields a single error item on the first call to
    /// [Iter::next()], then ends. Created when the initial seek (e.g.
    /// `iter_from`) failed; yields the error once.
    Err(Option<Error>),

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

        /// The first operation to perform when the consumer calls
        /// [DoubleEndedIterator::next_back()]. Subsequent calls use
        /// `back_next_op`.
        back_op: ffi::MDBX_cursor_op,

        /// The second and subsequent operations to perform when the consumer
        /// calls [DoubleEndedIterator::next_back()].
        back_next_op: ffi::MDBX_cursor_op,

        _marker: PhantomData<fn() -> (&'txn (), Key, Value)>,
    },
}

impl<'txn, 'cur, K, Key, Value> Iter<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    /// Creates a new iterator backed by the given cursor.
    ///
    /// `op`/`next_op` drive [Iterator::next()] and `back_op`/`back_next_op`
    /// drive [DoubleEndedIterator::next_back()].
    fn new(
        cursor: &'cur mut Cursor<'txn, K>,
        op: ffi::MDBX_cursor_op,
        next_op: ffi::MDBX_cursor_op,
        back_op: ffi::MDBX_cursor_op,
        back_next_op: ffi::MDBX_cursor_op,
    ) -> Self {
        Iter::Ok {
            cursor,
            op,
            next_op,
            back_op,
            back_next_op,
            _marker: PhantomData,
        }
    }
}

impl<'txn, K, Key, Value> Iterator for Iter<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    type Item = Result<(Key, Value)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Ok {
                cursor,
                op,
                next_op,
                ..
            } => fetch_op(cursor, mem::replace(op, *next_op), None).item(),
            Iter::Err(err) => err.take().map(Err),
        }
    }
}

impl<'txn, K, Key, Value> DoubleEndedIterator for Iter<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Ok {
                cursor,
                back_op,
                back_next_op,
                ..
            } => fetch_op(cursor, mem::replace(back_op, *back_next_op), None).item(),
            Iter::Err(err) => err.take().map(Err),
        }
    }
}

impl<'txn, K, Key, Value> FusedIterator for Iter<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
}

/// An iterator over the keys and duplicate values in an MDBX table.
///
/// The yielded items of the iterator are themselves iterators over the duplicate values for a
/// specific key.
pub enum IterDup<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    /// An iterator that yields a single error item on the first call to
    /// Iter.next(), then ends. Created when the initial seek (e.g.
    /// `iter_dup_from`) failed; yields the error once.
    Err(Option<Error>),

    /// An iterator that yields a [Result] on each call to Iter.next(): an
    /// [IntoIter] over the duplicates of the next key, or an error if the
    /// retrieval failed.
    Ok {
        /// The MDBX cursor with which to iterate.
        cursor: &'cur mut Cursor<'txn, K>,

        /// The first operation to perform when the consumer calls Iter.next().
        op: ffi::MDBX_cursor_op,

        _marker: PhantomData<fn() -> (&'txn (), Key, Value)>,
    },
}

impl<'txn, 'cur, K, Key, Value> IterDup<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    /// Creates a new iterator backed by the given cursor.
    fn new(cursor: &'cur mut Cursor<'txn, K>, op: ffi::MDBX_cursor_op) -> Self {
        IterDup::Ok {
            cursor,
            op,
            _marker: PhantomData,
        }
    }
}

impl<'txn, K, Key, Value> fmt::Debug for IterDup<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("IterDup").finish()
    }
}

impl<'txn, K, Key, Value> Iterator for IterDup<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    type Item = Result<IntoIter<'txn, K, Key, Value>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IterDup::Ok { cursor, op, .. } => {
                let mut key = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let mut data = ffi::MDBX_val {
                    iov_len: 0,
                    iov_base: ptr::null_mut(),
                };
                let op = mem::replace(op, ffi::MDBX_NEXT_NODUP);
                txn_execute(&cursor.txn, |_| {
                    let err_code =
                        unsafe { ffi::mdbx_cursor_get(cursor.cursor().0, &mut key, &mut data, op) };
                    match err_code {
                        ffi::MDBX_SUCCESS => Some(Cursor::new_at_position(&**cursor).map(|c| {
                            IntoIter::new(
                                c,
                                ffi::MDBX_GET_CURRENT,
                                ffi::MDBX_NEXT_DUP,
                                ffi::MDBX_LAST_DUP,
                                ffi::MDBX_PREV_DUP,
                                None,
                            )
                        })),
                        ffi::MDBX_NOTFOUND | ffi::MDBX_ENODATA => None,
                        error => Some(Err(Error::from_err_code(error))),
                    }
                })
            }
            IterDup::Err(err) => err.take().map(Err),
        }
    }
}
