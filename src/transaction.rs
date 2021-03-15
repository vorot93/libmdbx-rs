use crate::{
    database::Database,
    environment::Environment,
    error::{
        mdbx_result,
        Error,
        Result,
    },
    flags::DatabaseFlags,
};
use std::{
    fmt,
    ptr,
    result,
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
pub trait Transaction: Sized + private::Sealed {
    #[doc(hidden)]
    const ONLY_CLEAN: bool;

    /// Returns a raw pointer to the underlying MDBX transaction.
    ///
    /// The caller **must** ensure that the pointer is not used after the
    /// lifetime of the transaction.
    fn txn(&self) -> *mut ffi::MDBX_txn;

    /// Returns a raw pointer to the MDBX environment.
    fn env(&self) -> &Environment;

    /// Returns the transaction id.
    fn id(&self) -> u64 {
        unsafe { ffi::mdbx_txn_id(self.txn()) }
    }

    #[doc(hidden)]
    fn note_committed(&mut self);

    #[doc(hidden)]
    unsafe fn is_dirty(&self, ptr: *const libc::c_void) -> Result<bool> {
        match ffi::mdbx_is_dirty(self.txn(), ptr) {
            ffi::MDBX_RESULT_TRUE => Ok(true),
            ffi::MDBX_RESULT_FALSE => Ok(false),
            other => Err(Error::from_err_code(other)),
        }
    }

    /// Commits the transaction.
    ///
    /// Any pending operations will be saved.
    fn commit(mut self) -> Result<bool> {
        let result = mdbx_result(unsafe { ffi::mdbx_txn_commit_ex(self.txn(), ptr::null_mut()) });
        self.note_committed();
        result
    }

    /// Opens a handle to an MDBX database.
    ///
    /// If `name` is `None`, then the returned handle will be for the default database.
    ///
    /// If `name` is not `None`, then the returned handle will be for a named database. In this
    /// case the environment must be configured to allow named databases through
    /// `EnvironmentBuilder::set_max_dbs`.
    ///
    /// The returned database handle may be shared among any transaction in the environment.
    ///
    /// The database name may not contain the null character.
    fn open_db<'txn>(&'txn self, name: Option<&str>) -> Result<Database<'txn, Self>> {
        Database::new(self, name, 0)
    }
}

/// An LMDB read-only transaction.
pub struct RoTransaction<'env> {
    txn: *mut ffi::MDBX_txn,
    committed: bool,
    env: &'env Environment,
}

impl<'env> fmt::Debug for RoTransaction<'env> {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("RoTransaction").finish()
    }
}

impl<'env> Drop for RoTransaction<'env> {
    fn drop(&mut self) {
        if !self.committed {
            unsafe {
                ffi::mdbx_txn_abort(self.txn);
            }
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
                committed: false,
                env,
            })
        }
    }
}

impl<'env> Transaction for RoTransaction<'env> {
    const ONLY_CLEAN: bool = true;

    fn txn(&self) -> *mut ffi::MDBX_txn {
        self.txn
    }

    fn note_committed(&mut self) {
        self.committed = true;
    }

    fn env(&self) -> &Environment {
        self.env
    }
}

unsafe impl<'env> Send for RoTransaction<'env> {}
unsafe impl<'env> Sync for RoTransaction<'env> {}

/// An LMDB read-write transaction.
pub struct RwTransaction<'env> {
    txn: *mut ffi::MDBX_txn,
    committed: bool,
    env: &'env Environment,
}

impl<'env> fmt::Debug for RwTransaction<'env> {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("RwTransaction").finish()
    }
}

impl<'env> Drop for RwTransaction<'env> {
    fn drop(&mut self) {
        if !self.committed {
            unsafe {
                ffi::mdbx_txn_abort(self.txn);
            }
        }
    }
}

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
                committed: false,
                env,
            })
        }
    }

    fn open_db_with_flags<'txn>(&'txn self, name: Option<&str>, flags: DatabaseFlags) -> Result<Database<'txn, Self>> {
        Database::new(self, name, flags.bits())
    }

    /// Opens a handle to an MDBX database, creating the database if necessary.
    ///
    /// If the database is already created, the given option flags will be added to it.
    ///
    /// If `name` is `None`, then the returned handle will be for the default database.
    ///
    /// If `name` is not `None`, then the returned handle will be for a named database. In this
    /// case the environment must be configured to allow named databases through
    /// `EnvironmentBuilder::set_max_dbs`.
    ///
    /// This function will fail with `Error::BadRslot` if called by a thread with an open
    /// transaction.
    pub fn create_db<'txn>(&'txn self, name: Option<&str>, flags: DatabaseFlags) -> Result<Database<'txn, Self>> {
        self.open_db_with_flags(name, flags | DatabaseFlags::CREATE)
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
            committed: false,
            env: self.env,
        })
    }
}

impl<'env> Transaction for RwTransaction<'env> {
    const ONLY_CLEAN: bool = false;

    fn txn(&self) -> *mut ffi::MDBX_txn {
        self.txn
    }

    fn note_committed(&mut self) {
        self.committed = true;
    }

    fn open_db<'txn>(&'txn self, name: Option<&str>) -> Result<Database<'txn, Self>> {
        self.open_db_with_flags(name, DatabaseFlags::default())
    }

    fn env(&self) -> &Environment {
        self.env
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        error::*,
        flags::*,
    };
    use lifetimed_bytes::Bytes;
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

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        db.put(b"key1", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        assert_eq!(b"val1", &*db.get(b"key1").unwrap());
        assert_eq!(b"val2", &*db.get(b"key2").unwrap());
        assert_eq!(b"val3", &*db.get(b"key3").unwrap());
        assert_eq!(db.get(b"key").unwrap_err(), Error::NotFound);

        db.del(b"key1", None).unwrap();
        assert_eq!(db.get(b"key1").unwrap_err(), Error::NotFound);
    }

    #[test]
    fn test_put_get_del_multi() {
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
        db.put(b"key3", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key3", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        {
            let mut cur = db.cursor().unwrap();
            let iter = cur.iter_dup_of(b"key1");
            let vals = iter.map(|x| x.unwrap()).map(|(_, x)| x).collect::<Vec<_>>();
            assert_eq!(vals, vec![b"val1".into(), b"val2".into(), b"val3".into()] as Vec<Bytes>);
        }
        txn.commit().unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        db.del(b"key1", Some(b"val2")).unwrap();
        db.del(b"key2", None).unwrap();
        txn.commit().unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        {
            let mut cur = db.cursor().unwrap();
            let iter = cur.iter_dup_of(b"key1");
            let vals = iter.map(|x| x.unwrap()).map(|(_, x)| x).collect::<Vec<_>>();
            assert_eq!(vals, vec![b"val1".into(), b"val3".into()] as Vec<Bytes>);

            let iter = cur.iter_dup_of(b"key2");
            assert_eq!(0, iter.count());
        }
        txn.commit().unwrap();
    }

    #[test]
    fn test_reserve() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        {
            let mut writer = db.reserve(b"key1", 4, WriteFlags::empty()).unwrap();
            writer.write_all(b"val1").unwrap();
        }
        txn.commit().unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        assert_eq!(Bytes::from(b"val1"), db.get(b"key1").unwrap());
        assert_eq!(db.get(b"key"), Err(Error::NotFound));

        db.del(b"key1", None).unwrap();
        assert_eq!(db.get(b"key1"), Err(Error::NotFound));
    }

    #[test]
    fn test_nested_txn() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let mut txn = env.begin_rw_txn().unwrap();
        txn.open_db(None).unwrap().put(b"key1", b"val1", WriteFlags::empty()).unwrap();

        {
            let nested = txn.begin_nested_txn().unwrap();
            let db = nested.open_db(None).unwrap();
            db.put(b"key2", b"val2", WriteFlags::empty()).unwrap();
            assert_eq!(db.get(b"key1").unwrap(), Bytes::from(b"val1"));
            assert_eq!(db.get(b"key2").unwrap(), Bytes::from(b"val2"));
        }

        let db = txn.open_db(None).unwrap();
        assert_eq!(db.get(b"key1").unwrap(), Bytes::from(b"val1"));
        assert_eq!(db.get(b"key2"), Err(Error::NotFound));
    }

    #[test]
    fn test_clear_db() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        {
            let txn = env.begin_rw_txn().unwrap();
            txn.open_db(None).unwrap().put(b"key", b"val", WriteFlags::empty()).unwrap();
            assert!(!txn.commit().unwrap());
        }

        {
            let txn = env.begin_rw_txn().unwrap();
            txn.open_db(None).unwrap().clear_db().unwrap();
            assert!(!txn.commit().unwrap());
        }

        let txn = env.begin_ro_txn().unwrap();
        assert_eq!(txn.open_db(None).unwrap().get(b"key").unwrap_err(), Error::NotFound);
    }

    #[test]
    fn test_drop_db() {
        let dir = tempdir().unwrap();
        {
            let env = Environment::new().set_max_dbs(2).open(dir.path()).unwrap();

            {
                let txn = env.begin_rw_txn().unwrap();
                txn.create_db(Some("test"), DatabaseFlags::empty())
                    .unwrap()
                    .put(b"key", b"val", WriteFlags::empty())
                    .unwrap();
                // Workaround for MDBX dbi drop issue
                txn.create_db(Some("canary"), DatabaseFlags::empty()).unwrap();
                assert!(!txn.commit().unwrap());
            }
            {
                let txn = env.begin_rw_txn().unwrap();
                let db = txn.open_db(Some("test")).unwrap();
                unsafe {
                    db.drop_db().unwrap();
                }
                assert_eq!(txn.open_db(Some("test")).unwrap_err(), Error::NotFound);
                assert!(!txn.commit().unwrap());
            }
        }

        let env = Environment::new().set_max_dbs(2).open(dir.path()).unwrap();

        let txn = env.begin_ro_txn().unwrap();
        txn.open_db(Some("canary")).unwrap();
        assert_eq!(txn.open_db(Some("test")).unwrap_err(), Error::NotFound);
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
                {
                    let txn = reader_env.begin_ro_txn().unwrap();
                    let db = txn.open_db(None).unwrap();
                    assert_eq!(db.get(key), Err(Error::NotFound));
                }
                reader_barrier.wait();
                reader_barrier.wait();
                {
                    let txn = reader_env.begin_ro_txn().unwrap();
                    let db = txn.open_db(None).unwrap();
                    db.get(key).unwrap() == val
                }
            }));
        }

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        println!("wait2");
        barrier.wait();
        db.put(key, val, WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        println!("wait1");
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
                let txn = writer_env.begin_rw_txn().unwrap();
                let db = txn.open_db(None).unwrap();
                db.put(&format!("{}{}", key, i), &format!("{}{}", val, i), WriteFlags::empty()).unwrap();
                txn.commit().is_ok()
            }));
        }
        assert!(threads.into_iter().all(|b| b.join().unwrap()));

        let txn = env.begin_ro_txn().unwrap();
        let db = txn.open_db(None).unwrap();

        for i in 0..n {
            assert_eq!(format!("{}{}", val, i).as_bytes(), db.get(&format!("{}{}", key, i)).unwrap());
        }
    }

    #[test]
    fn test_stat() {
        let dir = tempdir().unwrap();
        let env = Environment::new().open(dir.path()).unwrap();

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.create_db(None, DatabaseFlags::empty()).unwrap();
        db.put(b"key1", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key2", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let db = txn.open_db(None).unwrap();
            let stat = db.stat().unwrap();
            assert_eq!(stat.entries(), 3);
        }

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        db.del(b"key1", None).unwrap();
        db.del(b"key2", None).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let db = txn.open_db(None).unwrap();
            let stat = db.stat().unwrap();
            assert_eq!(stat.entries(), 1);
        }

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        db.put(b"key4", b"val4", WriteFlags::empty()).unwrap();
        db.put(b"key5", b"val5", WriteFlags::empty()).unwrap();
        db.put(b"key6", b"val6", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let db = txn.open_db(None).unwrap();
            let stat = db.stat().unwrap();
            assert_eq!(stat.entries(), 4);
        }
    }

    #[test]
    fn test_stat_dupsort() {
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
        db.put(b"key3", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key3", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key3", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.open_db(None).unwrap().stat().unwrap();
            assert_eq!(stat.entries(), 9);
        }

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        db.del(b"key1", Some(b"val2")).unwrap();
        db.del(b"key2", None).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.open_db(None).unwrap().stat().unwrap();
            assert_eq!(stat.entries(), 5);
        }

        let txn = env.begin_rw_txn().unwrap();
        let db = txn.open_db(None).unwrap();
        db.put(b"key4", b"val1", WriteFlags::empty()).unwrap();
        db.put(b"key4", b"val2", WriteFlags::empty()).unwrap();
        db.put(b"key4", b"val3", WriteFlags::empty()).unwrap();
        txn.commit().unwrap();

        {
            let txn = env.begin_ro_txn().unwrap();
            let stat = txn.open_db(None).unwrap().stat().unwrap();
            assert_eq!(stat.entries(), 8);
        }
    }
}
