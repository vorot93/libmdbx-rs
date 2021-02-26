use crate::{
    cursor::{
        RoCursor,
        RwCursor,
    },
    database::Database,
    environment::{
        Environment,
        Stat,
    },
    error::{
        mdbx_result,
        Error,
        Result,
    },
    flags::{
        DatabaseFlags,
        WriteFlags,
    },
};
use libc::{
    c_uint,
    c_void,
};
use mem::size_of;
use std::{
    fmt,
    marker::PhantomData,
    mem,
    ptr,
    result,
    slice,
};

mod private {
    use super::*;

    pub trait Sealed {}

    impl<'env> Sealed for RoTransaction<'env> {}
    impl<'env> Sealed for RwTransaction<'env> {}
}

/// An LMDB transaction.
///
/// All database operations require a transaction.
pub trait Transaction<'env>: Sized + private::Sealed {
    /// Returns a raw pointer to the underlying MDBX transaction.
    ///
    /// The caller **must** ensure that the pointer is not used after the
    /// lifetime of the transaction.
    fn txn(&self) -> *mut ffi::MDBX_txn;

    /// Returns a raw pointer to the MDBX environment.
    fn env(&self) -> *mut ffi::MDBX_env {
        unsafe { ffi::mdbx_txn_env(self.txn()) }
    }

    /// Commits the transaction.
    ///
    /// Any pending operations will be saved.
    fn commit(self) -> Result<bool> {
        let result = mdbx_result(unsafe { ffi::mdbx_txn_commit_ex(self.txn(), ptr::null_mut()) });
        mem::forget(self);
        result
    }

    /// Opens a database in the transaction.
    ///
    /// If `name` is `None`, then the default database will be opened, otherwise
    /// a named database will be opened. The database handle will be private to
    /// the transaction until the transaction is successfully committed. If the
    /// transaction is aborted the returned database handle should no longer be
    /// used.
    ///
    /// Prefer using `Environment::open_db`.
    ///
    /// ## Safety
    ///
    /// This function (as well as `Environment::open_db`,
    /// `Environment::create_db`, and `Database::create`) **must not** be called
    /// from multiple concurrent transactions in the same environment. A
    /// transaction which uses this function must finish (either commit or
    /// abort) before any other transaction may use this function.
    fn open_db(&self, name: Option<&str>) -> Result<Database<'env>> {
        Database::new(self, name, 0)
    }

    /// Gets an item from a database.
    ///
    /// This function retrieves the data associated with the given key in the
    /// database. If the database supports duplicate keys
    /// (`DatabaseFlags::DUP_SORT`) then the first data item for the key will be
    /// returned. Retrieval of other items requires the use of
    /// `Transaction::cursor_get`. If the item is not in the database, then
    /// `Error::NotFound` will be returned.
    fn get<'txn, K>(&'txn self, database: Database<'env>, key: &K) -> Result<&'txn [u8]>
    where
        K: AsRef<[u8]>,
    {
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
            match ffi::mdbx_get(self.txn(), database.dbi(), &key_val, &mut data_val) {
                ffi::MDBX_SUCCESS => Ok(slice::from_raw_parts(data_val.iov_base as *const u8, data_val.iov_len)),
                err_code => Err(Error::from_err_code(err_code)),
            }
        }
    }

    /// Open a new read-only cursor on the given database.
    fn open_ro_cursor(&self, db: Database<'env>) -> Result<RoCursor<'_>> {
        RoCursor::new(self, db)
    }

    /// Gets the option flags for the given database in the transaction.
    fn db_flags(&self, db: Database<'env>) -> Result<DatabaseFlags> {
        let mut flags: c_uint = 0;
        unsafe {
            mdbx_result(ffi::mdbx_dbi_flags_ex(self.txn(), db.dbi(), &mut flags, ptr::null_mut()))?;
        }
        Ok(DatabaseFlags::from_bits_truncate(flags))
    }

    /// Retrieves database statistics.
    fn stat(&self, db: Database<'env>) -> Result<Stat> {
        unsafe {
            let mut stat = Stat::new();
            lmdb_try!(ffi::mdbx_dbi_stat(self.txn(), db.dbi(), stat.mdb_stat(), size_of::<Stat>()));
            Ok(stat)
        }
    }
}

/// An LMDB read-only transaction.
pub struct RoTransaction<'env> {
    txn: *mut ffi::MDBX_txn,
    _marker: PhantomData<&'env ()>,
}

impl<'env> fmt::Debug for RoTransaction<'env> {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("RoTransaction").finish()
    }
}

impl<'env> Drop for RoTransaction<'env> {
    fn drop(&mut self) {
        unsafe {
            ffi::mdbx_txn_abort(self.txn);
        }
    }
}

impl<'env> RoTransaction<'env> {
    /// Creates a new read-only transaction in the given environment. Prefer
    /// using `Environment::begin_ro_txn`.
    pub(crate) fn new(env: &'env Environment) -> Result<RoTransaction<'env>> {
        let mut txn: *mut ffi::MDBX_txn = ptr::null_mut();
        unsafe {
            mdbx_result(ffi::mdbx_txn_begin_ex(
                env.env(),
                ptr::null_mut(),
                ffi::MDBX_RDONLY,
                &mut txn,
                ptr::null_mut(),
            ))?;
            Ok(RoTransaction {
                txn,
                _marker: PhantomData,
            })
        }
    }

    /// Resets the read-only transaction.
    ///
    /// Abort the transaction like `Transaction::abort`, but keep the
    /// transaction handle.  `InactiveTransaction::renew` may reuse the handle.
    /// This saves allocation overhead if the process will start a new read-only
    /// transaction soon, and also locking overhead if
    /// `EnvironmentFlags::NO_TLS` is in use. The reader table lock is released,
    /// but the table slot stays tied to its thread or transaction. Reader locks
    /// generally don't interfere with writers, but they keep old versions of
    /// database pages allocated. Thus they prevent the old pages from being
    /// reused when writers commit new data, and so under heavy load the
    /// database size may grow much more rapidly than otherwise.
    pub fn reset(self) -> InactiveTransaction<'env> {
        let txn = self.txn;
        unsafe {
            mem::forget(self);
            ffi::mdbx_txn_reset(txn)
        };
        InactiveTransaction {
            txn,
            _marker: PhantomData,
        }
    }
}

impl<'env> Transaction<'env> for RoTransaction<'env> {
    fn txn(&self) -> *mut ffi::MDBX_txn {
        self.txn
    }
}

unsafe impl<'env> Send for RoTransaction<'env> {}
unsafe impl<'env> Sync for RoTransaction<'env> {}

/// An inactive read-only transaction.
pub struct InactiveTransaction<'env> {
    txn: *mut ffi::MDBX_txn,
    _marker: PhantomData<&'env ()>,
}

impl<'env> fmt::Debug for InactiveTransaction<'env> {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("InactiveTransaction").finish()
    }
}

impl<'env> Drop for InactiveTransaction<'env> {
    fn drop(&mut self) {
        unsafe {
            ffi::mdbx_txn_abort(self.txn);
        }
    }
}

impl<'env> InactiveTransaction<'env> {
    /// Renews the inactive transaction, returning an active read-only
    /// transaction.
    ///
    /// This acquires a new reader lock for a transaction handle that had been
    /// released by `RoTransaction::reset`.
    pub fn renew(self) -> Result<RoTransaction<'env>> {
        let txn = self.txn;
        unsafe {
            mem::forget(self);
            mdbx_result(ffi::mdbx_txn_renew(txn))?
        };
        Ok(RoTransaction {
            txn,
            _marker: PhantomData,
        })
    }
}

/// An LMDB read-write transaction.
pub struct RwTransaction<'env> {
    txn: *mut ffi::MDBX_txn,
    _marker: PhantomData<&'env ()>,
}

impl<'env> fmt::Debug for RwTransaction<'env> {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("RwTransaction").finish()
    }
}

impl<'env> Drop for RwTransaction<'env> {
    fn drop(&mut self) {
        unsafe {
            ffi::mdbx_txn_abort(self.txn);
        }
    }
}

// unsafe impl<'env> Sync for RwTransaction<'env> {}

impl<'env> RwTransaction<'env> {
    /// Creates a new read-write transaction in the given environment. Prefer
    /// using `Environment::begin_ro_txn`.
    pub(crate) fn new(env: &'env Environment) -> Result<RwTransaction<'env>> {
        let mut txn: *mut ffi::MDBX_txn = ptr::null_mut();
        unsafe {
            mdbx_result(ffi::mdbx_txn_begin_ex(
                env.env(),
                ptr::null_mut(),
                ffi::MDBX_TXN_READWRITE,
                &mut txn,
                ptr::null_mut(),
            ))?;
            Ok(RwTransaction {
                txn,
                _marker: PhantomData,
            })
        }
    }

    /// Opens a database in the provided transaction, creating it if necessary.
    ///
    /// If `name` is `None`, then the default database will be opened, otherwise
    /// a named database will be opened. The database handle will be private to
    /// the transaction until the transaction is successfully committed. If the
    /// transaction is aborted the returned database handle should no longer be
    /// used.
    ///
    /// Prefer using `Environment::create_db`.
    ///
    /// ## Safety
    ///
    /// This function (as well as `Environment::open_db`,
    /// `Environment::create_db`, and `Database::open`) **must not** be called
    /// from multiple concurrent transactions in the same environment. A
    /// transaction which uses this function must finish (either commit or
    /// abort) before any other transaction may use this function.
    pub fn create_db<'txn>(&'txn self, name: Option<&str>, flags: DatabaseFlags) -> Result<Database<'env>> {
        Database::new(self, name, flags.bits() | ffi::MDBX_CREATE)
    }

    /// Opens a new read-write cursor on the given database and transaction.
    pub fn open_rw_cursor<'txn>(&'txn mut self, db: Database<'env>) -> Result<RwCursor<'txn>> {
        RwCursor::new(self, db)
    }

    /// Stores an item into a database.
    ///
    /// This function stores key/data pairs in the database. The default
    /// behavior is to enter the new key/data pair, replacing any previously
    /// existing key if duplicates are disallowed, or adding a duplicate data
    /// item if duplicates are allowed (`DatabaseFlags::DUP_SORT`).
    pub fn put<K, D>(&mut self, database: Database, key: &K, data: &D, flags: WriteFlags) -> Result<()>
    where
        K: AsRef<[u8]>,
        D: AsRef<[u8]>,
    {
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
        mdbx_result(unsafe { ffi::mdbx_put(self.txn(), database.dbi(), &key_val, &mut data_val, flags.bits()) })?;

        Ok(())
    }

    /// Returns a buffer which can be used to write a value into the item at the
    /// given key and with the given length. The buffer must be completely
    /// filled by the caller.
    pub fn reserve<'txn, K>(
        &'txn mut self,
        database: Database,
        key: &K,
        len: usize,
        flags: WriteFlags,
    ) -> Result<&'txn mut [u8]>
    where
        K: AsRef<[u8]>,
    {
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
                self.txn(),
                database.dbi(),
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
    /// Otherwise, if data parameter is `None`, any/all value(s) for specified key will be deleted.
    pub fn del<K>(&mut self, database: Database, key: &K, data: Option<&[u8]>) -> Result<()>
    where
        K: AsRef<[u8]>,
    {
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
                unsafe { ffi::mdbx_del(self.txn(), database.dbi(), &key_val, &d) }
            } else {
                unsafe { ffi::mdbx_del(self.txn(), database.dbi(), &key_val, ptr::null()) }
            }
        })?;

        Ok(())
    }

    /// Empties the given database. All items will be removed.
    pub fn clear_db(&mut self, db: Database) -> Result<()> {
        mdbx_result(unsafe { ffi::mdbx_drop(self.txn(), db.dbi(), false) })?;

        Ok(())
    }

    /// Drops the database from the environment.
    pub fn drop_db(&mut self, db: Database) -> Result<()> {
        mdbx_result(unsafe { ffi::mdbx_drop(self.txn(), db.dbi(), true) })?;

        Ok(())
    }

    /// Begins a new nested transaction inside of this transaction.
    pub fn begin_nested_txn(&mut self) -> Result<RwTransaction<'_>> {
        let mut nested: *mut ffi::MDBX_txn = ptr::null_mut();
        unsafe {
            let env: *mut ffi::MDBX_env = ffi::mdbx_txn_env(self.txn());
            ffi::mdbx_txn_begin_ex(env, self.txn(), 0, &mut nested, ptr::null_mut());
        }
        Ok(RwTransaction {
            txn: nested,
            _marker: PhantomData,
        })
    }
}

impl<'env> Transaction<'env> for RwTransaction<'env> {
    fn txn(&self) -> *mut ffi::MDBX_txn {
        self.txn
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        cursor::Cursor,
        error::*,
        flags::*,
    };
    use std::{
        io::Write,
        sync::{
            Arc,
            Barrier,
        },
        thread::{
            self,
            JoinHandle,
        },
    };
    use tempfile::tempdir;

    #[test]
    fn test_put_get_del() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.open_db(None).unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        txn.put(db, b"key1", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        assert_eq!(b"val1", txn.get(db, b"key1").unwrap());
        assert_eq!(b"val2", txn.get(db, b"key2").unwrap());
        assert_eq!(b"val3", txn.get(db, b"key3").unwrap());
        assert_eq!(txn.get(db, b"key"), Err(Error::NotFound));

        txn.del(db, b"key1", None).unwrap();
        assert_eq!(txn.get(db, b"key1"), Err(Error::NotFound));
    }

    #[test]
    fn test_put_get_del_multi() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.create_db(None, DatabaseFlags::DUP_SORT).unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        txn.put(db, b"key1", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key1", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key1", b"val3", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val3", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        let txn = env.begin_rw_txn().unwrap();
        {
            let mut cur = txn.open_ro_cursor(db).unwrap();
            let iter = cur.iter_dup_of(b"key1");
            let vals = iter.map(|x| x.unwrap()).map(|(_, x)| x).collect::<Vec<_>>();
            assert_eq!(vals, vec![b"val1", b"val2", b"val3"]);
        }
        txn.commit().unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        txn.del(db, b"key1", Some(b"val2")).unwrap();
        txn.del(db, b"key2", None).unwrap();
        txn.commit().unwrap();

        let txn = env.begin_rw_txn().unwrap();
        {
            let mut cur = txn.open_ro_cursor(db).unwrap();
            let iter = cur.iter_dup_of(b"key1");
            let vals = iter.map(|x| x.unwrap()).map(|(_, x)| x).collect::<Vec<_>>();
            assert_eq!(vals, vec![b"val1", b"val3"]);

            let iter = cur.iter_dup_of(b"key2");
            assert_eq!(0, iter.count());
        }
        txn.commit().unwrap();
    }

    #[test]
    fn test_reserve() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.open_db(None).unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        {
            let mut writer = txn.reserve(db, b"key1", 4, WriteFlags::empty()).unwrap();
            writer.write_all(b"val1").unwrap();
        }
        txn.commit().unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        assert_eq!(b"val1", txn.get(db, b"key1").unwrap());
        assert_eq!(txn.get(db, b"key"), Err(Error::NotFound));

        txn.del(db, b"key1", None).unwrap();
        assert_eq!(txn.get(db, b"key1"), Err(Error::NotFound));
    }

    #[test]
    fn test_inactive_txn() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.open_db(None).unwrap();

        {
            let mut txn = env.begin_rw_txn().unwrap();
            txn.put(db, b"key", b"val", WriteFlags::empty()).unwrap();
            txn.commit().unwrap();
        }

        let txn = env.begin_ro_txn().unwrap();
        let inactive = txn.reset();
        let active = inactive.renew().unwrap();
        assert!(active.get(db, b"key").is_ok());
    }

    #[test]
    fn test_nested_txn() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.open_db(None).unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        txn.put(db, b"key1", b"val1", WriteFlags::empty()).unwrap();

        {
            let mut nested = txn.begin_nested_txn().unwrap();
            nested.put(db, b"key2", b"val2", WriteFlags::empty()).unwrap();
            assert_eq!(nested.get(db, b"key1").unwrap(), b"val1");
            assert_eq!(nested.get(db, b"key2").unwrap(), b"val2");
        }

        assert_eq!(txn.get(db, b"key1").unwrap(), b"val1");
        assert_eq!(txn.get(db, b"key2"), Err(Error::NotFound));
    }

    #[test]
    fn test_clear_db() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.open_db(None).unwrap();

        {
            let mut txn = env.begin_rw_txn().unwrap();
            txn.put(db, b"key", b"val", WriteFlags::empty()).unwrap();
            txn.commit().unwrap();
        }

        {
            let mut txn = env.begin_rw_txn().unwrap();
            txn.clear_db(db).unwrap();
            txn.commit().unwrap();
        }

        let txn = env.begin_ro_txn().unwrap();
        assert_eq!(txn.get(db, b"key"), Err(Error::NotFound));
    }

    #[test]
    fn test_drop_db() {
        let dir = tempdir().unwrap();
        let env = Environment::new().set_max_dbs(2).open(dir.path()).unwrap();
        let db = env.create_db(Some("test"), DatabaseFlags::empty()).unwrap();

        {
            let mut txn = env.begin_rw_txn().unwrap();
            txn.put(db, b"key", b"val", WriteFlags::empty()).unwrap();
            txn.commit().unwrap();
        }
        {
            let mut txn = env.begin_rw_txn().unwrap();
            txn.drop_db(db).unwrap();
            txn.commit().unwrap();
        }

        assert_eq!(env.open_db(Some("test")), Err(Error::NotFound));
    }

    #[test]
    fn test_concurrent_readers_single_writer() {
        let dir = tempdir().unwrap();
        let env: Arc<Environment> = Arc::new(Environment::new().open(dir.path()).unwrap());

        let n = 10usize; // Number of concurrent readers
        let barrier = Arc::new(Barrier::new(n + 1));
        let mut threads: Vec<JoinHandle<bool>> = Vec::with_capacity(n);

        let key = b"key";
        let val = b"val";

        for _ in 0..n {
            let reader_env = env.clone();
            let reader_barrier = barrier.clone();

            threads.push(thread::spawn(move || {
                let db = reader_env.open_db(None).unwrap();
                {
                    let txn = reader_env.begin_ro_txn().unwrap();
                    assert_eq!(txn.get(db, key), Err(Error::NotFound));
                }
                reader_barrier.wait();
                reader_barrier.wait();
                {
                    let txn = reader_env.begin_ro_txn().unwrap();
                    txn.get(db, key).unwrap() == val
                }
            }));
        }

        let db = env.open_db(None).unwrap();
        let mut txn = env.begin_rw_txn().unwrap();
        barrier.wait();
        txn.put(db, key, val, WriteFlags::empty()).unwrap();
        txn.commit().unwrap();
        barrier.wait();

        assert!(threads.into_iter().all(|b| b.join().unwrap()))
    }

    #[test]
    fn test_concurrent_writers() {
        let dir = tempdir().unwrap();
        let env = Arc::new(Environment::new().open(dir.path()).unwrap());

        let n = 10usize; // Number of concurrent writers
        let mut threads: Vec<JoinHandle<bool>> = Vec::with_capacity(n);

        let key = "key";
        let val = "val";

        for i in 0..n {
            let writer_env = env.clone();

            threads.push(thread::spawn(move || {
                let db = writer_env.open_db(None).unwrap();
                let mut txn = writer_env.begin_rw_txn().unwrap();
                txn.put(db, &format!("{}{}", key, i), &format!("{}{}", val, i), WriteFlags::empty()).unwrap();
                txn.commit().is_ok()
            }));
        }
        assert!(threads.into_iter().all(|b| b.join().unwrap()));

        let db = env.open_db(None).unwrap();
        let txn = env.begin_ro_txn().unwrap();

        for i in 0..n {
            assert_eq!(format!("{}{}", val, i).as_bytes(), txn.get(db, &format!("{}{}", key, i)).unwrap());
        }
    }

    #[test]
    fn test_stat() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.create_db(None, DatabaseFlags::empty()).unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        txn.put(db, b"key1", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.stat(db).unwrap();
            assert_eq!(stat.entries(), 3);
        }

        let mut txn = env.begin_rw_txn().unwrap();
        txn.del(db, b"key1", None).unwrap();
        txn.del(db, b"key2", None).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.stat(db).unwrap();
            assert_eq!(stat.entries(), 1);
        }

        let mut txn = env.begin_rw_txn().unwrap();
        txn.put(db, b"key4", b"val4", WriteFlags::empty()).unwrap();
        txn.put(db, b"key5", b"val5", WriteFlags::empty()).unwrap();
        txn.put(db, b"key6", b"val6", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.stat(db).unwrap();
            assert_eq!(stat.entries(), 4);
        }
    }

    #[test]
    fn test_stat_dupsort() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();
        let db = env.create_db(None, DatabaseFlags::DUP_SORT).unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        txn.put(db, b"key1", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key1", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key1", b"val3", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key2", b"val3", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.stat(db).unwrap();
            assert_eq!(stat.entries(), 9);
        }

        let mut txn = env.begin_rw_txn().unwrap();
        txn.del(db, b"key1", Some(b"val2")).unwrap();
        txn.del(db, b"key2", None).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.stat(db).unwrap();
            assert_eq!(stat.entries(), 5);
        }

        let mut txn = env.begin_rw_txn().unwrap();
        txn.put(db, b"key4", b"val1", WriteFlags::empty()).unwrap();
        txn.put(db, b"key4", b"val2", WriteFlags::empty()).unwrap();
        txn.put(db, b"key4", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.stat(db).unwrap();
            assert_eq!(stat.entries(), 8);
        }
    }
}
