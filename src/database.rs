use crate::{
    environment::EnvironmentKind,
    error::{
        mdbx_result,
        Result,
    },
    flags::DatabaseFlags,
    transaction::{
        txn_execute,
        TransactionKind,
        RW,
    },
    Cursor,
    Stat,
    Transaction,
};
use libc::c_uint;
use parking_lot::Mutex;
use std::{
    ffi::CString,
    marker::PhantomData,
    mem::size_of,
    ptr,
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
    txn: &'txn Mutex<*mut ffi::MDBX_txn>,
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
        mdbx_result(txn_execute(txn.txn_mutex(), |txn| unsafe { ffi::mdbx_dbi_open(txn, name_ptr, flags, &mut dbi) }))?;
        Ok(Database {
            dbi,
            txn: txn.txn_mutex(),
            _marker: PhantomData,
        })
    }

    pub(crate) fn freelist_db<'env, E: EnvironmentKind>(txn: &'txn Transaction<'env, K, E>) -> Self {
        Database {
            dbi: 0,
            txn: txn.txn_mutex(),
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

    pub(crate) fn txn(&self) -> &'txn Mutex<*mut ffi::MDBX_txn> {
        self.txn
    }

    /// Open a new cursor on the given database.
    pub fn cursor(&self) -> Result<Cursor<'txn, K>> {
        Cursor::new(self)
    }

    /// Gets the option flags for the given database in the transaction.
    pub fn db_flags(&self) -> Result<DatabaseFlags> {
        let mut flags: c_uint = 0;
        unsafe {
            mdbx_result(txn_execute(self.txn, |txn| {
                ffi::mdbx_dbi_flags_ex(txn, self.dbi(), &mut flags, ptr::null_mut())
            }))?;
        }
        Ok(DatabaseFlags::from_bits_truncate(flags))
    }

    /// Retrieves database statistics.
    pub fn stat(&self) -> Result<Stat> {
        unsafe {
            let mut stat = Stat::new();
            mdbx_result(txn_execute(self.txn, |txn| {
                ffi::mdbx_dbi_stat(txn, self.dbi(), stat.mdb_stat(), size_of::<Stat>())
            }))?;
            Ok(stat)
        }
    }
}

impl<'txn> Database<'txn, RW> {
    /// Empties the given database. All items will be removed.
    pub fn clear_db(&self) -> Result<()> {
        mdbx_result(txn_execute(self.txn, |txn| unsafe { ffi::mdbx_drop(txn, self.dbi(), false) }))?;

        Ok(())
    }

    /// Drops the database from the environment.
    ///
    /// # Safety
    /// Caller must close ALL other [Database] and [Cursor] instances pointing to the same dbi BEFORE calling this function.
    pub unsafe fn drop_db(self) -> Result<()> {
        mdbx_result(txn_execute(self.txn, |txn| ffi::mdbx_drop(txn, self.dbi(), true)))?;

        Ok(())
    }
}

unsafe impl<'txn, K> Send for Database<'txn, K> where K: TransactionKind {}
unsafe impl<'txn, K> Sync for Database<'txn, K> where K: TransactionKind {}
