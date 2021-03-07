use crate::{
    database::Database,
    environment::Environment,
    error::{
        mdbx_result,
        Error,
        Result,
    },
    flags::DatabaseFlags,
    Transaction,
    TransactionKind,
    RW,
};
use parking_lot::Mutex;
use std::{
    collections::HashSet,
    fmt,
    ptr,
    result,
    sync::Arc,
};

/// Transaction chart. It should contain information about internal structure.
pub unsafe trait TxnChart<K: TransactionKind>: Sized {
    const MIN_DBI_NUM: u64;

    fn init(txn: *mut ffi::MDBX_txn) -> Result<Self>;
    fn txn(&self) -> *mut ffi::MDBX_txn;
}

/// Dynamic transaction chart which allows manual database creation.
pub struct DynamicTxnChart<K: TransactionKind> {
    txn: *mut ffi::MDBX_txn,
    opened_dbs: Arc<Mutex<HashSet<ffi::MDBX_dbi>>>,
}

unsafe impl<K: TransactionKind> TxnChart<K> for DynamicTxnChart<K> {
    const MIN_DBI_NUM: u64 = 0;

    fn init(txn: *mut ffi::MDBX_txn) -> Result<Self> {
        Ok(Self {
            txn,
            opened_dbs: Default::default(),
        })
    }

    fn txn(&self) -> *mut ffi::MDBX_txn {
        self.txn
    }
}

impl<K> DynamicTxnChart<K> {
    pub fn open(&self, name: &str) -> Result<Database<K>> {
        Database::new(self.txn, Some(name), 0)
    }
}

impl DynamicTxnChart<RW> {
    pub fn create(&self, name: &str) -> Result<Database<RW>> {
        Database::new(self, name, DatabaseFlags::CREATE.bits())
    }
}
