use crate::{
    DatabaseKind, Decodable, Transaction,
    database::TxnPtr,
    error::{Error, Result, mdbx_result},
    flags::*,
    mdbx_try_optional,
    table::Table,
    transaction::{RW, TransactionKind, put_multiple_with, txn_execute},
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
    borrow::{Borrow, Cow},
    cmp::Ordering,
    fmt,
    iter::FusedIterator,
    marker::PhantomData,
    mem, ptr, result,
    sync::Arc,
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

    /// Returns a new cursor on the same table, at the same position.
    pub fn try_clone(&self) -> Result<Self> {
        let cursor = txn_execute(&self.txn, |_| unsafe {
            let cursor = ffi::mdbx_cursor_create(ptr::null_mut());
            if cursor.is_null() {
                return Err(Error::from_err_code(ffi::MDBX_ENOMEM));
            }
            let rc = ffi::mdbx_cursor_copy(self.cursor.0, cursor);
            if rc != ffi::MDBX_SUCCESS {
                // Close the raw handle here: dropping a `Cursor` would
                // re-lock the (non-reentrant) transaction mutex we hold.
                ffi::mdbx_cursor_close(cursor);
                return Err(Error::from_err_code(rc));
            }
            Ok(cursor)
        })?;
        Ok(Self {
            txn: self.txn.clone(),
            cursor: CursorPtr(cursor),
            _marker: PhantomData,
        })
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
        let (k, v, inexact) = mdbx_try_optional!(self.get(Some(key), value, MDBX_SET_LOWERBOUND));

        Ok(Some((inexact, k, v)))
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
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        Iter::new(Range::new(self, EndState::Unstarted, Ops::ASCENDING))
    }

    /// Iterate over table items starting from the beginning of the table.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn iter_start<Key, Value>(&mut self) -> Iter<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        Iter::new(Range::new(
            self,
            EndState::Unstarted,
            Ops::ASCENDING.starting_with(MDBX_FIRST),
        ))
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
        IntoIter::new(Range::new(
            self,
            EndState::Unstarted,
            Ops::ASCENDING.starting_with(MDBX_FIRST),
        ))
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
        let found = self.seek(key, MDBX_SET_RANGE);
        Iter::new(Range::seeked(self, found, Ops::ASCENDING))
    }

    /// Iterate over table items starting from the given key.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the next key.
    pub fn into_iter_from<Key, Value>(self, key: &[u8]) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let found = self.seek(key, MDBX_SET_RANGE);
        IntoIter::new(Range::seeked(self, found, Ops::ASCENDING))
    }

    /// Iterate over table items backwards, starting from the largest key
    /// less than or equal to the given key, down to the first key.
    ///
    /// For tables with duplicate data items ([TableFlags::DUP_SORT]), the
    /// duplicate data items of each key will be returned before moving on to
    /// the previous key.
    ///
    /// Reversing the iterator (e.g. via [.rev()](Iterator::rev)) yields the
    /// same items in ascending order, i.e. the keys up to the given one.
    pub fn into_iter_back_from<Key, Value>(self, key: &[u8]) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let found = self.seek(key, MDBX_TO_KEY_LESSER_OR_EQUAL);
        IntoIter::new(Range::seeked(self, found, Ops::DESCENDING))
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
        IntoIter::new(Range::new(self, EndState::Unstarted, Ops::DESCENDING))
    }

    /// Iterate over duplicate table items. The iterator will begin with the
    /// item next after the cursor, and continue until the end of the table.
    /// Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup<Key, Value>(&mut self) -> IterDup<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        IterDup::new(self, MDBX_NEXT, Ok(true))
    }

    /// Iterate over duplicate table items starting from the beginning of the
    /// table. Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup_start<Key, Value>(&mut self) -> IterDup<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        IterDup::new(self, MDBX_FIRST, Ok(true))
    }

    /// Iterate over duplicate items in the table starting from the given
    /// key. Each item will be returned as an iterator of its duplicates.
    pub fn iter_dup_from<Key, Value>(&mut self, key: &[u8]) -> IterDup<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let found = self.seek(key, MDBX_SET_RANGE);
        IterDup::new(self, MDBX_GET_CURRENT, found)
    }

    /// Iterate over the duplicates of the item in the table with the given key.
    pub fn iter_dup_of<Key, Value>(&mut self, key: &[u8]) -> Iter<'txn, '_, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let found = self.seek(key, MDBX_SET);
        Iter::new(Range::seeked(self, found, Ops::DUPS))
    }

    /// Iterate over the duplicates of the item in the table with the given key.
    pub fn into_iter_dup_of<Key, Value>(self, key: &[u8]) -> IntoIter<'txn, K, Key, Value>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        let found = self.seek(key, MDBX_SET);
        IntoIter::new(Range::seeked(self, found, Ops::DUPS))
    }

    /// Positions the cursor with a key-seeking `op`, without decoding.
    /// `Ok(false)` means no item matched.
    fn seek(&self, key: &[u8], op: MDBX_cursor_op) -> Result<bool> {
        let mut key_val = unsafe { slice_to_val(Some(key)) };
        let mut data_val = unsafe { slice_to_val(None) };
        txn_execute(&self.txn, |_| unsafe {
            cursor_get_raw(self.cursor.0, &mut key_val, &mut data_val, op)
        })
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
                ffi::mdbx_cursor_put(
                    self.cursor.0,
                    &key_val,
                    &mut data_val,
                    c_enum(flags.ffi_bits()),
                )
            })
        })?;

        Ok(())
    }

    /// [TableFlags::DUP_FIXED]-only: stores `values`, a concatenation of
    /// `value_len`-byte elements, as duplicates of `key` in one operation
    /// (MDBX's `MDBX_MULTIPLE`). The cursor is left at the last stored item.
    ///
    /// Returns the number of elements stored. Fails with
    /// [Error::InvalidArgument] if `value_len` is zero or does not divide
    /// `values.len()`.
    pub fn put_multiple(
        &mut self,
        key: &[u8],
        values: &[u8],
        value_len: usize,
        flags: WriteFlags,
    ) -> Result<usize> {
        let key_val = ffi::MDBX_val {
            iov_len: key.len(),
            iov_base: key.as_ptr() as *mut c_void,
        };
        put_multiple_with(values, value_len, |data| {
            txn_execute(&self.txn, |_| unsafe {
                ffi::mdbx_cursor_put(
                    self.cursor.0,
                    &key_val,
                    data.as_mut_ptr(),
                    c_enum(flags.ffi_bits() | ffi::MDBX_MULTIPLE as u32),
                )
            })
        })
    }

    /// Deletes the current key/data pair.
    ///
    /// ### Flags
    ///
    /// [WriteFlags::ALLDUPS] deletes all data items for the current key, if
    /// the table was opened with [TableFlags::DUP_SORT].
    pub fn del(&mut self, flags: WriteFlags) -> Result<()> {
        mdbx_result(unsafe {
            txn_execute(&self.txn, |_| {
                ffi::mdbx_cursor_del(self.cursor.0, c_enum(flags.ffi_bits()))
            })
        })?;

        Ok(())
    }
}

impl<K> Clone for Cursor<'_, K>
where
    K: TransactionKind,
{
    /// Clones the cursor at its current position.
    ///
    /// # Panics
    ///
    /// If libmdbx fails to copy the cursor (out of memory); see
    /// [Cursor::try_clone] for the fallible version.
    fn clone(&self) -> Self {
        self.try_clone().expect("failed to copy MDBX cursor")
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

/// Runs a cursor get `op`, leaving the result in `key`/`data`.
///
/// Returns `Ok(false)` at the end of the data (`MDBX_NOTFOUND`, or
/// `MDBX_ENODATA` after a failed seek). Must run under the transaction lock.
unsafe fn cursor_get_raw(
    cursor: *mut ffi::MDBX_cursor,
    key: &mut ffi::MDBX_val,
    data: &mut ffi::MDBX_val,
    op: MDBX_cursor_op,
) -> Result<bool> {
    match unsafe { ffi::mdbx_cursor_get(cursor, key, data, op) } {
        ffi::MDBX_SUCCESS | ffi::MDBX_RESULT_TRUE => Ok(true),
        ffi::MDBX_NOTFOUND | ffi::MDBX_ENODATA => Ok(false),
        error => Err(Error::from_err_code(error)),
    }
}

/// Compares the positions of two positioned cursors on the same table, in
/// iteration order. Must run under the transaction lock.
fn position_cmp<K: TransactionKind>(
    a: &Cursor<'_, K>,
    b: &Cursor<'_, K>,
    descending: bool,
) -> Ordering {
    let table_order = unsafe { ffi::mdbx_cursor_compare(a.cursor.0, b.cursor.0, false) }.cmp(&0);
    if descending {
        table_order.reverse()
    } else {
        table_order
    }
}

/// How one fetch in an iteration ended.
enum Step<T> {
    /// Yield this item; `true` if it was the last one left in the domain.
    Item(T, bool),
    /// The domain is exhausted.
    End,
    /// libmdbx failed; iteration stops after yielding the error.
    Failed(Error),
}

/// Runs `op` on `cursor` and decodes the item it lands on, unless `admit`
/// rejects the new position (`None`: the domain is exhausted; `Some(last)`:
/// yield it). Fetch, comparison and decoding share one transaction lock
/// hold, so a concurrent writer cannot move the page in between.
fn fetch<'txn, K, Key, Value>(
    cursor: &Cursor<'txn, K>,
    op: MDBX_cursor_op,
    admit: impl FnOnce(&Cursor<'txn, K>) -> Option<bool>,
) -> Step<Result<(Key, Value)>>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    let mut key = unsafe { slice_to_val(None) };
    let mut data = unsafe { slice_to_val(None) };
    txn_execute(&cursor.txn, |txn| unsafe {
        match cursor_get_raw(cursor.cursor.0, &mut key, &mut data, op) {
            Ok(true) => {}
            Ok(false) => return Step::End,
            Err(error) => return Step::Failed(error),
        }
        let Some(last) = admit(cursor) else {
            return Step::End;
        };
        let item = Key::decode_val::<K>(txn, &key)
            .and_then(|key| Ok((key, Value::decode_val::<K>(txn, &data)?)));
        Step::Item(item, last)
    })
}

/// Progress of the front end of a [Range].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndState {
    /// Not positioned yet: the first fetch runs [Ops::front_first].
    Unstarted,
    /// Positioned at an item that has not been yielded yet.
    Pending,
    /// Positioned at the item it yielded last.
    Yielded,
}

/// The cursor ops that walk one iteration domain from both ends.
#[derive(Clone, Copy, Debug)]
struct Ops {
    /// Positions an [EndState::Unstarted] front at the domain's first item.
    front_first: MDBX_cursor_op,
    /// Advances the front.
    front_next: MDBX_cursor_op,
    /// Positions the back at the domain's last item, starting from a copy of
    /// the positioned front (so relative ops such as `MDBX_LAST_DUP` work).
    back_first: MDBX_cursor_op,
    /// Advances the back.
    back_next: MDBX_cursor_op,
    /// Whether the front walks in descending table order.
    descending: bool,
}

impl Ops {
    /// Ascending from the item after the cursor to the last item.
    const ASCENDING: Self = Self {
        front_first: MDBX_NEXT,
        front_next: MDBX_NEXT,
        back_first: MDBX_LAST,
        back_next: MDBX_PREV,
        descending: false,
    };

    /// Descending from the last item to the first.
    const DESCENDING: Self = Self {
        front_first: MDBX_LAST,
        front_next: MDBX_PREV,
        back_first: MDBX_FIRST,
        back_next: MDBX_NEXT,
        descending: true,
    };

    /// The duplicates of the key the front is positioned at.
    const DUPS: Self = Self {
        front_first: MDBX_FIRST_DUP,
        front_next: MDBX_NEXT_DUP,
        back_first: MDBX_LAST_DUP,
        back_next: MDBX_PREV_DUP,
        descending: false,
    };

    const fn starting_with(self, front_first: MDBX_cursor_op) -> Self {
        Self {
            front_first,
            ..self
        }
    }
}

/// Double-ended iteration over a contiguous domain of a table.
///
/// The front cursor walks the domain in [Ops] order. The back cursor is a
/// copy of the front, created on the first `next_back`, that walks it in
/// reverse. Each end stops as soon as it reaches a position the other end has
/// already yielded. Positions are compared with `mdbx_cursor_compare`, i.e.
/// in the table's own key and duplicate order (so `INTEGER_KEY`,
/// `REVERSE_KEY` and friends work), which makes every item come out exactly
/// once however `next` and `next_back` are interleaved.
struct Range<'txn, K, C>
where
    K: TransactionKind,
{
    front: C,
    /// Created by the first `next_back`. While the range is not done it is
    /// positioned at the item it yielded last.
    back: Option<Cursor<'txn, K>>,
    front_state: EndState,
    ops: Ops,
    /// An error from setting up the iterator, yielded once.
    error: Option<Error>,
    done: bool,
}

impl<'txn, K, C> Range<'txn, K, C>
where
    K: TransactionKind,
    C: Borrow<Cursor<'txn, K>>,
{
    fn new(front: C, front_state: EndState, ops: Ops) -> Self {
        Self {
            front,
            back: None,
            front_state,
            ops,
            error: None,
            done: false,
        }
    }

    /// A range whose front was positioned by a seek with outcome `found`.
    fn seeked(front: C, found: Result<bool>, ops: Ops) -> Self {
        let mut range = Self::new(front, EndState::Pending, ops);
        match found {
            Ok(true) => {}
            Ok(false) => range.done = true,
            Err(error) => range.error = Some(error),
        }
        range
    }

    /// Yields the setup error once, then reports whether iteration is over.
    fn take_error(&mut self) -> Option<Error> {
        let error = self.error.take();
        self.done |= error.is_some();
        error
    }

    /// Applies the outcome of a fetch to the iteration state.
    fn settle<T>(&mut self, step: Step<Result<T>>) -> Option<Result<T>> {
        match step {
            Step::Item(item, last) => {
                self.done |= last;
                Some(item)
            }
            Step::End => {
                self.done = true;
                None
            }
            Step::Failed(error) => {
                self.done = true;
                Some(Err(error))
            }
        }
    }

    fn next<Key, Value>(&mut self) -> Option<Result<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        if let Some(error) = self.take_error() {
            return Some(Err(error));
        }
        if self.done {
            return None;
        }
        let op = match self.front_state {
            EndState::Unstarted => self.ops.front_first,
            EndState::Pending => MDBX_GET_CURRENT,
            EndState::Yielded => self.ops.front_next,
        };
        let back = self.back.as_ref();
        let descending = self.ops.descending;
        let step = fetch(self.front.borrow(), op, |front| match back {
            // The back has yielded its position and everything beyond it.
            Some(back) => position_cmp(front, back, descending)
                .is_lt()
                .then_some(false),
            None => Some(false),
        });
        if let Step::Item(..) = step {
            self.front_state = EndState::Yielded;
        }
        self.settle(step)
    }

    fn next_back<Key, Value>(&mut self) -> Option<Result<(Key, Value)>>
    where
        Key: Decodable<'txn>,
        Value: Decodable<'txn>,
    {
        if let Some(error) = self.take_error() {
            return Some(Err(error));
        }
        if self.done {
            return None;
        }
        // The back is bounded by the front, so the front must be positioned.
        if self.front_state == EndState::Unstarted {
            match fetch::<K, (), ()>(self.front.borrow(), self.ops.front_first, |_| Some(false)) {
                Step::Item(..) => self.front_state = EndState::Pending,
                Step::End => {
                    self.done = true;
                    return None;
                }
                Step::Failed(error) => return self.settle(Step::Failed(error)),
            }
        }
        let op = if self.back.is_some() {
            self.ops.back_next
        } else {
            match self.front.borrow().try_clone() {
                Ok(back) => self.back = Some(back),
                Err(error) => return self.settle(Step::Failed(error)),
            }
            self.ops.back_first
        };
        let front = self.front.borrow();
        let front_pending = self.front_state == EndState::Pending;
        let descending = self.ops.descending;
        let back = self.back.as_ref().expect("the back cursor exists by now");
        let step = fetch(back, op, |back| {
            match position_cmp(back, front, descending) {
                // Stay strictly beyond what the front has yielded; the front's
                // pending item may be taken, as the last one left.
                Ordering::Greater => Some(false),
                Ordering::Equal if front_pending => Some(true),
                _ => None,
            }
        });
        self.settle(step)
    }
}

/// An iterator over the key/value pairs in an MDBX table, owning its cursor.
///
/// Double-ended: [DoubleEndedIterator::next_back()] walks the same items from
/// the other end (on a second cursor), and however the two ends are
/// interleaved, every item is yielded exactly once. Iteration stops after a
/// libmdbx error; a decoding error is yielded and iteration continues.
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
pub struct IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
{
    range: Range<'txn, K, Cursor<'txn, K>>,
    _marker: PhantomData<fn() -> (Key, Value)>,
}

impl<'txn, K, Key, Value> IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
{
    fn new(range: Range<'txn, K, Cursor<'txn, K>>) -> Self {
        Self {
            range,
            _marker: PhantomData,
        }
    }

    /// An iterator that yields `error` once.
    pub(crate) fn failed(cursor: Cursor<'txn, K>, error: Error) -> Self {
        Self::new(Range::seeked(cursor, Err(error), Ops::ASCENDING))
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
        self.range.next()
    }
}

impl<'txn, K, Key, Value> DoubleEndedIterator for IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.range.next_back()
    }
}

impl<'txn, K, Key, Value> FusedIterator for IntoIter<'txn, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
}

impl<K, Key, Value> fmt::Debug for IntoIter<'_, K, Key, Value>
where
    K: TransactionKind,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("IntoIter").finish_non_exhaustive()
    }
}

impl<'txn, K> IntoIterator for Cursor<'txn, K>
where
    K: TransactionKind,
{
    type Item = Result<(Cow<'txn, [u8]>, Cow<'txn, [u8]>)>;
    type IntoIter = IntoIter<'txn, K, Cow<'txn, [u8]>, Cow<'txn, [u8]>>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(Range::new(self, EndState::Unstarted, Ops::ASCENDING))
    }
}

/// An iterator over the key/value pairs in an MDBX table, borrowing its
/// cursor.
///
/// Double-ended with the same guarantees as [IntoIter]. The borrowed cursor
/// is left wherever the front end stopped.
pub struct Iter<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
{
    range: Range<'txn, K, &'cur mut Cursor<'txn, K>>,
    _marker: PhantomData<fn() -> (Key, Value)>,
}

impl<'txn, 'cur, K, Key, Value> Iter<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
{
    fn new(range: Range<'txn, K, &'cur mut Cursor<'txn, K>>) -> Self {
        Self {
            range,
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
        self.range.next()
    }
}

impl<'txn, K, Key, Value> DoubleEndedIterator for Iter<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.range.next_back()
    }
}

impl<'txn, K, Key, Value> FusedIterator for Iter<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
}

impl<K, Key, Value> fmt::Debug for Iter<'_, '_, K, Key, Value>
where
    K: TransactionKind,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("Iter").finish_non_exhaustive()
    }
}

/// An iterator over the keys and duplicate values in an MDBX table.
///
/// The yielded items of the iterator are themselves iterators over the
/// duplicate values for a specific key. Iteration stops after an error.
pub struct IterDup<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
{
    cursor: &'cur mut Cursor<'txn, K>,
    /// The op that moves to the next key's first duplicate.
    op: MDBX_cursor_op,
    /// An error from setting up the iterator, yielded once.
    error: Option<Error>,
    done: bool,
    _marker: PhantomData<fn() -> (Key, Value)>,
}

impl<'txn, 'cur, K, Key, Value> IterDup<'txn, 'cur, K, Key, Value>
where
    K: TransactionKind,
{
    /// Iterates from the key `op` moves to; `found` is the outcome of any
    /// seek that positioned the cursor first.
    fn new(cursor: &'cur mut Cursor<'txn, K>, op: MDBX_cursor_op, found: Result<bool>) -> Self {
        let (done, error) = match found {
            Ok(found) => (!found, None),
            Err(error) => (false, Some(error)),
        };
        Self {
            cursor,
            op,
            error,
            done,
            _marker: PhantomData,
        }
    }
}

impl<K, Key, Value> fmt::Debug for IterDup<'_, '_, K, Key, Value>
where
    K: TransactionKind,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("IterDup").finish_non_exhaustive()
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
        if let Some(error) = self.error.take() {
            self.done = true;
            return Some(Err(error));
        }
        if self.done {
            return None;
        }
        let op = mem::replace(&mut self.op, MDBX_NEXT_NODUP);
        match fetch::<K, (), ()>(self.cursor, op, |_| Some(false)) {
            Step::Item(..) => {}
            Step::End => {
                self.done = true;
                return None;
            }
            Step::Failed(error) => {
                self.done = true;
                return Some(Err(error));
            }
        }
        let dups = self.cursor.try_clone();
        self.done = dups.is_err();
        Some(dups.map(|dups| IntoIter::new(Range::new(dups, EndState::Pending, Ops::DUPS))))
    }
}

impl<'txn, K, Key, Value> FusedIterator for IterDup<'txn, '_, K, Key, Value>
where
    K: TransactionKind,
    Key: Decodable<'txn>,
    Value: Decodable<'txn>,
{
}
