use crate::{
    error::{
        mdbx_result,
        Result,
    },
    Transaction,
};
use libc::c_uint;
use std::{
    ffi::CString,
    marker::PhantomData,
    ptr,
};

/// A handle to an individual database in an environment.
///
/// A database handle denotes the name and parameters of a database in an environment.
#[derive(Debug, Eq, PartialEq)]
pub struct Database<'env> {
    dbi: ffi::MDBX_dbi,
    _marker: PhantomData<&'env ()>,
}

impl<'env> Database<'env> {
    /// Opens a new database handle in the given transaction.
    ///
    /// Prefer using `Environment::open_db`, `Environment::create_db`, `TransactionExt::open_db`,
    /// or `RwTransaction::create_db`.
    pub(crate) fn new<Txn: Transaction<'env>>(txn: &Txn, name: Option<&str>, flags: c_uint) -> Result<Self> {
        let c_name = name.map(|n| CString::new(n).unwrap());
        let name_ptr = if let Some(ref c_name) = c_name {
            c_name.as_ptr()
        } else {
            ptr::null()
        };
        let mut dbi: ffi::MDBX_dbi = 0;
        mdbx_result(unsafe { ffi::mdbx_dbi_open(txn.txn(), name_ptr, flags, &mut dbi) })?;
        Ok(Database {
            dbi,
            _marker: PhantomData,
        })
    }

    pub(crate) fn freelist_db() -> Self {
        Database {
            dbi: 0,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying MDBX database handle.
    ///
    /// The caller **must** ensure that the handle is not used after the lifetime of the
    /// environment, or after the database has been closed.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn dbi(&self) -> ffi::MDBX_dbi {
        self.dbi
    }
}

unsafe impl<'env> Sync for Database<'env> {}
unsafe impl<'env> Send for Database<'env> {}
