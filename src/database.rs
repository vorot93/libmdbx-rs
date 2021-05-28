use crate::{
    environment::EnvironmentKind,
    error::{
        mdbx_result,
        Result,
    },
    flags::{
        DatabaseFlags,
        WriteFlags,
    },
    transaction::{
        TransactionKind,
        RW,
    },
    util::freeze_bytes,
    Cursor,
    Error,
    Stat,
    Transaction,
};
use libc::{
    c_uint,
    c_void,
};
use lifetimed_bytes::Bytes;
use std::{
    ffi::CString,
    marker::PhantomData,
    mem::size_of,
    ptr,
    slice,
};

/// A handle to an individual database in an environment.
///
/// A database handle denotes the name and parameters of a database in an environment.
#[derive(Debug)]
pub struct Database<'txn, K>
where
    K: TransactionKind,
{
    dbi: ffi::MDBX_dbi,
    txn: *mut ffi::MDBX_txn,
    _marker: PhantomData<&'txn K>,
}

impl<'txn, K> Database<'txn, K>
where
    K: TransactionKind,
{
    /// Opens a new database handle in the given transaction.
    ///
    /// Prefer using `Environment::open_db`, `Environment::create_db`, `TransactionExt::open_db`,
    /// or `RwTransaction::create_db`.
    pub(crate) fn new<'env, E: EnvironmentKind>(
        txn: &'txn Transaction<'env, K, E>,
        name: Option<&str>,
        flags: c_uint,
    ) -> Result<Self> {
        let c_name = name.map(|n| CString::new(n).unwrap());
        let name_ptr = if let Some(c_name) = &c_name {
            c_name.as_ptr()
        } else {
            ptr::null()
        };
        let mut dbi: ffi::MDBX_dbi = 0;
        let txn = txn.txn();
        mdbx_result(unsafe { ffi::mdbx_dbi_open(txn, name_ptr, flags, &mut dbi) })?;
        Ok(Database {
            dbi,
            txn,
            _marker: PhantomData,
        })
    }

    pub(crate) fn freelist_db<'env, E: EnvironmentKind>(txn: &'txn Transaction<'env, K, E>) -> Self {
        Database {
            dbi: 0,
            txn: txn.txn(),
            _marker: PhantomData,
        }
    }

    /// Returns the underlying MDBX database handle.
    ///
    /// The caller **must** ensure that the handle is not used after the lifetime of the
    /// environment, or after the database has been closed.
    pub fn dbi(&self) -> ffi::MDBX_dbi {
        self.dbi
    }

    pub(crate) fn txn(&self) -> *mut ffi::MDBX_txn {
        self.txn
    }

    /// Gets an item from a database.
    ///
    /// This function retrieves the data associated with the given key in the
    /// database. If the database supports duplicate keys
    /// ([DatabaseFlags::DUP_SORT]) then the first data item for the key will be
    /// returned. Retrieval of other items requires the use of
    /// [Cursor]. If the item is not in the database, then
    /// [None] will be returned.
    pub fn get(&self, key: impl AsRef<[u8]>) -> Result<Option<Bytes<'txn>>> {
        let key = key.as_ref();
        let key_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: key.len(),
            iov_base: key.as_ptr() as *mut c_void,
        };
        let mut data_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: 0,
            iov_base: ptr::null_mut(),
        };
        unsafe {
            match ffi::mdbx_get(self.txn, self.dbi(), &key_val, &mut data_val) {
                ffi::MDBX_SUCCESS => freeze_bytes::<K>(self.txn, &data_val).map(Some),
                ffi::MDBX_NOTFOUND => Ok(None),
                err_code => Err(Error::from_err_code(err_code)),
            }
        }
    }

    /// Open a new cursor on the given database.
    pub fn cursor(&self) -> Result<Cursor<'txn, K>> {
        Cursor::new(self)
    }

    /// Gets the option flags for the given database in the transaction.
    pub fn db_flags(&self) -> Result<DatabaseFlags> {
        let mut flags: c_uint = 0;
        unsafe {
            mdbx_result(ffi::mdbx_dbi_flags_ex(self.txn, self.dbi(), &mut flags, ptr::null_mut()))?;
        }
        Ok(DatabaseFlags::from_bits_truncate(flags))
    }

    /// Retrieves database statistics.
    pub fn stat(&self) -> Result<Stat> {
        unsafe {
            let mut stat = Stat::new();
            mdbx_result(ffi::mdbx_dbi_stat(self.txn, self.dbi(), stat.mdb_stat(), size_of::<Stat>()))?;
            Ok(stat)
        }
    }
}

impl<'txn> Database<'txn, RW> {
    /// Stores an item into a database.
    ///
    /// This function stores key/data pairs in the database. The default
    /// behavior is to enter the new key/data pair, replacing any previously
    /// existing key if duplicates are disallowed, or adding a duplicate data
    /// item if duplicates are allowed ([DatabaseFlags::DUP_SORT]).
    pub fn put(&self, key: impl AsRef<[u8]>, data: impl AsRef<[u8]>, flags: WriteFlags) -> Result<()> {
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
        mdbx_result(unsafe { ffi::mdbx_put(self.txn, self.dbi(), &key_val, &mut data_val, flags.bits()) })?;

        Ok(())
    }

    /// Returns a buffer which can be used to write a value into the item at the
    /// given key and with the given length. The buffer must be completely
    /// filled by the caller.
    pub fn reserve(&self, key: impl AsRef<[u8]>, len: usize, flags: WriteFlags) -> Result<&'txn mut [u8]> {
        let key = key.as_ref();
        let key_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: key.len(),
            iov_base: key.as_ptr() as *mut c_void,
        };
        let mut data_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: len,
            iov_base: ptr::null_mut::<c_void>(),
        };
        unsafe {
            mdbx_result(ffi::mdbx_put(
                self.txn,
                self.dbi(),
                &key_val,
                &mut data_val,
                flags.bits() | ffi::MDBX_RESERVE,
            ))?;
            Ok(slice::from_raw_parts_mut(data_val.iov_base as *mut u8, data_val.iov_len))
        }
    }

    /// Delete items from a database.
    /// This function removes key/data pairs from the database.
    ///
    /// The data parameter is NOT ignored regardless the database does support sorted duplicate data items or not.
    /// If the data parameter is non-NULL only the matching data item will be deleted.
    /// Otherwise, if data parameter is [None], any/all value(s) for specified key will be deleted.
    pub fn del(&self, key: impl AsRef<[u8]>, data: Option<&[u8]>) -> Result<()> {
        let key = key.as_ref();
        let key_val: ffi::MDBX_val = ffi::MDBX_val {
            iov_len: key.len(),
            iov_base: key.as_ptr() as *mut c_void,
        };
        let data_val: Option<ffi::MDBX_val> = data.map(|data| ffi::MDBX_val {
            iov_len: data.len(),
            iov_base: data.as_ptr() as *mut c_void,
        });

        mdbx_result({
            if let Some(d) = data_val {
                unsafe { ffi::mdbx_del(self.txn, self.dbi(), &key_val, &d) }
            } else {
                unsafe { ffi::mdbx_del(self.txn, self.dbi(), &key_val, ptr::null()) }
            }
        })?;

        Ok(())
    }

    /// Empties the given database. All items will be removed.
    pub fn clear_db(&self) -> Result<()> {
        mdbx_result(unsafe { ffi::mdbx_drop(self.txn, self.dbi(), false) })?;

        Ok(())
    }

    /// Drops the database from the environment.
    ///
    /// # Safety
    /// Caller must close ALL other [Database] and [Cursor] instances pointing to the same dbi BEFORE calling this function.
    pub unsafe fn drop_db(self) -> Result<()> {
        mdbx_result(ffi::mdbx_drop(self.txn, self.dbi(), true))?;

        Ok(())
    }
}

unsafe impl<'txn, K> Send for Database<'txn, K> where K: TransactionKind {}
