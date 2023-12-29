use crate::{
    error::{mdbx_result, Error, Result},
    table::Table,
    transaction::{RO, RW},
    Mode, ReadWriteOptions, SyncMode, Transaction, TransactionKind,
};
use byteorder::{ByteOrder, NativeEndian};
use libc::c_uint;
use mem::size_of;
use sealed::sealed;
use std::{
    ffi::CString,
    fmt,
    fmt::Debug,
    marker::PhantomData,
    mem,
    os::unix::ffi::OsStrExt,
    path::Path,
    ptr, result,
    sync::mpsc::{sync_channel, SyncSender},
    thread::sleep,
    time::Duration,
};

#[sealed]
pub trait DatabaseKind: Debug + 'static {
    const EXTRA_FLAGS: ffi::MDBX_env_flags_t;
}

#[derive(Debug)]
pub struct NoWriteMap;
#[derive(Debug)]
pub struct WriteMap;

#[sealed]
impl DatabaseKind for NoWriteMap {
    const EXTRA_FLAGS: ffi::MDBX_env_flags_t = ffi::MDBX_ENV_DEFAULTS;
}
#[sealed]
impl DatabaseKind for WriteMap {
    const EXTRA_FLAGS: ffi::MDBX_env_flags_t = ffi::MDBX_WRITEMAP;
}

#[derive(Copy, Clone, Debug)]
pub struct TxnPtr(pub *mut ffi::MDBX_txn);
unsafe impl Send for TxnPtr {}

#[derive(Copy, Clone, Debug)]
pub struct DbPtr(pub *mut ffi::MDBX_env);
unsafe impl Send for DbPtr {}
unsafe impl Sync for DbPtr {}

pub(crate) enum TxnManagerMessage {
    Begin {
        parent: TxnPtr,
        flags: ffi::MDBX_txn_flags_t,
        sender: SyncSender<Result<TxnPtr>>,
    },
    Abort {
        tx: TxnPtr,
        sender: SyncSender<Result<bool>>,
    },
    Commit {
        tx: TxnPtr,
        sender: SyncSender<Result<bool>>,
    },
}

/// Supports multiple tables, all residing in the same shared-memory map.
pub struct Database<E>
where
    E: DatabaseKind,
{
    inner: DbPtr,
    pub(crate) txn_manager: Option<SyncSender<TxnManagerMessage>>,
    _marker: PhantomData<E>,
}

#[derive(Clone, Default)]
pub struct DatabaseOptions {
    pub permissions: Option<ffi::mdbx_mode_t>,
    pub max_readers: Option<c_uint>,
    pub max_tables: Option<u64>,
    pub rp_augment_limit: Option<u64>,
    pub loose_limit: Option<u64>,
    pub dp_reserve_limit: Option<u64>,
    pub txn_dp_limit: Option<u64>,
    pub spill_max_denominator: Option<u64>,
    pub spill_min_denominator: Option<u64>,
    pub page_size: Option<PageSize>,
    pub no_sub_dir: bool,
    pub exclusive: bool,
    pub accede: bool,
    pub mode: Mode,
    pub no_rdahead: bool,
    pub no_meminit: bool,
    pub coalesce: bool,
    pub liforeclaim: bool,
}

impl DatabaseOptions {
    pub(crate) fn make_flags(&self) -> ffi::MDBX_env_flags_t {
        let mut flags = 0;

        if self.no_sub_dir {
            flags |= ffi::MDBX_NOSUBDIR;
        }

        if self.exclusive {
            flags |= ffi::MDBX_EXCLUSIVE;
        }

        if self.accede {
            flags |= ffi::MDBX_ACCEDE;
        }

        match self.mode {
            Mode::ReadOnly => {
                flags |= ffi::MDBX_RDONLY;
            }
            Mode::ReadWrite(ReadWriteOptions { sync_mode, .. }) => {
                flags |= match sync_mode {
                    SyncMode::Durable => ffi::MDBX_SYNC_DURABLE,
                    SyncMode::NoMetaSync => ffi::MDBX_NOMETASYNC,
                    SyncMode::SafeNoSync => ffi::MDBX_SAFE_NOSYNC,
                    SyncMode::UtterlyNoSync => ffi::MDBX_UTTERLY_NOSYNC,
                };
            }
        }

        if self.no_rdahead {
            flags |= ffi::MDBX_NORDAHEAD;
        }

        if self.no_meminit {
            flags |= ffi::MDBX_NOMEMINIT;
        }

        if self.coalesce {
            flags |= ffi::MDBX_COALESCE;
        }

        if self.liforeclaim {
            flags |= ffi::MDBX_LIFORECLAIM;
        }

        flags |= ffi::MDBX_NOTLS;

        flags
    }
}

impl<E> Database<E>
where
    E: DatabaseKind,
{
    /// Open a database.
    pub fn open(path: impl AsRef<Path>) -> Result<Database<E>> {
        Self::open_with_options(path, Default::default())
    }

    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: DatabaseOptions,
    ) -> Result<Database<E>> {
        let mut db: *mut ffi::MDBX_env = ptr::null_mut();
        unsafe {
            mdbx_result(ffi::mdbx_env_create(&mut db))?;
            if let Err(e) = (|| {
                if let Mode::ReadWrite(ReadWriteOptions {
                    min_size,
                    max_size,
                    growth_step,
                    shrink_threshold,
                    ..
                }) = options.mode
                {
                    mdbx_result(ffi::mdbx_env_set_geometry(
                        db,
                        min_size.unwrap_or(-1),
                        -1,
                        max_size.unwrap_or(-1),
                        growth_step.unwrap_or(-1),
                        shrink_threshold.unwrap_or(-1),
                        match options.page_size {
                            None => -1,
                            Some(PageSize::MinimalAcceptable) => 0,
                            Some(PageSize::Set(size)) => size as isize,
                        },
                    ))?;
                }
                for (opt, v) in [
                    (ffi::MDBX_opt_max_db, options.max_tables),
                    (ffi::MDBX_opt_rp_augment_limit, options.rp_augment_limit),
                    (ffi::MDBX_opt_loose_limit, options.loose_limit),
                    (ffi::MDBX_opt_dp_reserve_limit, options.dp_reserve_limit),
                    (ffi::MDBX_opt_txn_dp_limit, options.txn_dp_limit),
                    (
                        ffi::MDBX_opt_spill_max_denominator,
                        options.spill_max_denominator,
                    ),
                    (
                        ffi::MDBX_opt_spill_min_denominator,
                        options.spill_min_denominator,
                    ),
                ] {
                    if let Some(v) = v {
                        mdbx_result(ffi::mdbx_env_set_option(db, opt, v))?;
                    }
                }

                let path = match CString::new(path.as_ref().as_os_str().as_bytes()) {
                    Ok(path) => path,
                    Err(..) => return Err(crate::Error::Invalid),
                };
                mdbx_result(ffi::mdbx_env_open(
                    db,
                    path.as_ptr(),
                    options.make_flags() | E::EXTRA_FLAGS,
                    options.permissions.unwrap_or(0o644),
                ))?;

                Ok(())
            })() {
                ffi::mdbx_env_close_ex(db, false);

                return Err(e);
            }
        }

        let mut db = Database {
            inner: DbPtr(db),
            txn_manager: None,
            _marker: PhantomData,
        };

        #[allow(clippy::redundant_locals)]
        if let Mode::ReadWrite { .. } = options.mode {
            let (tx, rx) = std::sync::mpsc::sync_channel(0);
            let e = db.inner;
            std::thread::spawn(move || loop {
                match rx.recv() {
                    Ok(msg) => match msg {
                        TxnManagerMessage::Begin {
                            parent,
                            flags,
                            sender,
                        } => {
                            let e = e;
                            let mut txn: *mut ffi::MDBX_txn = ptr::null_mut();
                            sender
                                .send(
                                    mdbx_result(unsafe {
                                        ffi::mdbx_txn_begin_ex(
                                            e.0,
                                            parent.0,
                                            flags,
                                            &mut txn,
                                            ptr::null_mut(),
                                        )
                                    })
                                    .map(|_| TxnPtr(txn)),
                                )
                                .unwrap()
                        }
                        TxnManagerMessage::Abort { tx, sender } => {
                            sender
                                .send(mdbx_result(unsafe { ffi::mdbx_txn_abort(tx.0) }))
                                .unwrap();
                        }
                        TxnManagerMessage::Commit { tx, sender } => {
                            sender
                                .send(mdbx_result(unsafe {
                                    ffi::mdbx_txn_commit_ex(tx.0, ptr::null_mut())
                                }))
                                .unwrap();
                        }
                    },
                    Err(_) => return,
                }
            });

            db.txn_manager = Some(tx);
        }

        Ok(db)
    }

    /// Returns a raw pointer to the underlying MDBX database.
    ///
    /// The caller **must** ensure that the pointer is not dereferenced after the lifetime of the
    /// database.
    pub fn ptr(&self) -> DbPtr {
        self.inner
    }

    /// Create a read-only transaction for use with the database.
    pub fn begin_ro_txn(&self) -> Result<Transaction<'_, RO, E>> {
        Transaction::new(self)
    }

    /// Create a read-write transaction for use with the database. This method will block while
    /// there are any other read-write transactions open on the database.
    pub fn begin_rw_txn(&self) -> Result<Transaction<'_, RW, E>> {
        let sender = self.txn_manager.as_ref().ok_or(Error::Access)?;
        let txn = loop {
            let (tx, rx) = sync_channel(0);
            sender
                .send(TxnManagerMessage::Begin {
                    parent: TxnPtr(ptr::null_mut()),
                    flags: RW::OPEN_FLAGS,
                    sender: tx,
                })
                .unwrap();
            let res = rx.recv().unwrap();
            if let Err(Error::Busy) = &res {
                sleep(Duration::from_millis(250));
                continue;
            }

            break res;
        }?;
        Ok(Transaction::new_from_ptr(self, txn.0))
    }

    /// Flush the database data buffers to disk.
    pub fn sync(&self, force: bool) -> Result<bool> {
        mdbx_result(unsafe { ffi::mdbx_env_sync_ex(self.ptr().0, force, false) })
    }

    /// Retrieves statistics about this database.
    pub fn stat(&self) -> Result<Stat> {
        unsafe {
            let mut stat = Stat::new();
            mdbx_result(ffi::mdbx_env_stat_ex(
                self.ptr().0,
                ptr::null(),
                stat.mdb_stat(),
                size_of::<Stat>(),
            ))?;
            Ok(stat)
        }
    }

    /// Retrieves info about this database.
    pub fn info(&self) -> Result<Info> {
        unsafe {
            let mut info = Info(mem::zeroed());
            mdbx_result(ffi::mdbx_env_info_ex(
                self.ptr().0,
                ptr::null(),
                &mut info.0,
                size_of::<Info>(),
            ))?;
            Ok(info)
        }
    }

    /// Retrieves the total number of pages on the freelist.
    ///
    /// Along with [Database::info()], this can be used to calculate the exact number
    /// of used pages as well as free pages in this database.
    ///
    /// ```
    /// # use libmdbx::Database;
    /// # use libmdbx::NoWriteMap;
    /// let dir = tempfile::tempdir().unwrap();
    /// let db = Database::<NoWriteMap>::open(&dir).unwrap();
    /// let info = db.info().unwrap();
    /// let stat = db.stat().unwrap();
    /// let freelist = db.freelist().unwrap();
    /// let last_pgno = info.last_pgno() + 1; // pgno is 0 based.
    /// let total_pgs = info.map_size() / stat.page_size() as usize;
    /// let pgs_in_use = last_pgno - freelist;
    /// let pgs_free = total_pgs - pgs_in_use;
    /// ```
    ///
    /// Note:
    ///
    /// * MDBX stores all the freelists in the designated table 0 in each database,
    ///   and the freelist count is stored at the beginning of the value as `libc::c_uint`
    ///   (32-bit) in the native byte order.
    ///
    /// * It will create a read transaction to traverse the freelist table.
    pub fn freelist(&self) -> Result<usize> {
        let mut freelist: usize = 0;
        let txn = self.begin_ro_txn()?;
        let table = Table::freelist_table();
        let cursor = txn.cursor(&table)?;

        for result in cursor {
            let (_key, value) = result?;
            if value.len() < mem::size_of::<u32>() {
                return Err(Error::Corrupted);
            }

            freelist += NativeEndian::read_u32(&value) as usize;
        }

        Ok(freelist)
    }
}

/// Database statistics.
///
/// Contains information about the size and layout of an MDBX database or table.
#[repr(transparent)]
pub struct Stat(ffi::MDBX_stat);

impl Stat {
    /// Create a new Stat with zero'd inner struct `ffi::MDB_stat`.
    pub(crate) fn new() -> Stat {
        unsafe { Stat(mem::zeroed()) }
    }

    /// Returns a mut pointer to `ffi::MDB_stat`.
    pub(crate) fn mdb_stat(&mut self) -> *mut ffi::MDBX_stat {
        &mut self.0
    }
}

impl Stat {
    /// Size of a table page. This is the same for all tables in the database.
    #[inline]
    pub const fn page_size(&self) -> u32 {
        self.0.ms_psize
    }

    /// Depth (height) of the B-tree.
    #[inline]
    pub const fn depth(&self) -> u32 {
        self.0.ms_depth
    }

    /// Number of internal (non-leaf) pages.
    #[inline]
    pub const fn branch_pages(&self) -> usize {
        self.0.ms_branch_pages as usize
    }

    /// Number of leaf pages.
    #[inline]
    pub const fn leaf_pages(&self) -> usize {
        self.0.ms_leaf_pages as usize
    }

    /// Number of overflow pages.
    #[inline]
    pub const fn overflow_pages(&self) -> usize {
        self.0.ms_overflow_pages as usize
    }

    /// Number of data items.
    #[inline]
    pub const fn entries(&self) -> usize {
        self.0.ms_entries as usize
    }

    /// Total size in bytes.
    #[inline]
    pub const fn total_size(&self) -> u64 {
        (self.leaf_pages() + self.branch_pages() + self.overflow_pages()) as u64
            * self.page_size() as u64
    }
}

#[repr(transparent)]
pub struct GeometryInfo(ffi::MDBX_envinfo__bindgen_ty_1);

impl GeometryInfo {
    pub fn min(&self) -> u64 {
        self.0.lower
    }
}

/// Database information.
///
/// Contains database information about the map size, readers, last txn id etc.
#[repr(transparent)]
pub struct Info(ffi::MDBX_envinfo);

impl Info {
    pub fn geometry(&self) -> GeometryInfo {
        GeometryInfo(self.0.mi_geo)
    }

    /// Size of memory map.
    #[inline]
    pub fn map_size(&self) -> usize {
        self.0.mi_mapsize as usize
    }

    /// Last used page number
    #[inline]
    pub fn last_pgno(&self) -> usize {
        self.0.mi_last_pgno as usize
    }

    /// Last transaction ID
    #[inline]
    pub fn last_txnid(&self) -> usize {
        self.0.mi_recent_txnid as usize
    }

    /// Max reader slots in the database
    #[inline]
    pub fn max_readers(&self) -> usize {
        self.0.mi_maxreaders as usize
    }

    /// Max reader slots used in the database
    #[inline]
    pub fn num_readers(&self) -> usize {
        self.0.mi_numreaders as usize
    }
}

impl<E> fmt::Debug for Database<E>
where
    E: DatabaseKind,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        f.debug_struct("Database").finish()
    }
}

impl<E> Drop for Database<E>
where
    E: DatabaseKind,
{
    fn drop(&mut self) {
        unsafe {
            ffi::mdbx_env_close_ex(self.inner.0, false);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageSize {
    MinimalAcceptable,
    Set(usize),
}
