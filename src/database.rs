use crate::{
    Mode, ReadWriteOptions, SyncMode, Transaction, TransactionKind,
    error::{Error, Result, mdbx_result},
    table::Table,
    transaction::{RO, RW},
};
use libc::c_uint;
use mem::size_of;
use parking_lot::{Condvar, Mutex};
use sealed::sealed;
use std::{
    fmt,
    fmt::Debug,
    marker::PhantomData,
    mem,
    ops::Deref,
    path::Path,
    ptr, result,
    sync::mpsc::{SyncSender, sync_channel},
};

/// Opens `env` at `path`, handing libmdbx the path in the platform's native
/// encoding.
///
/// On Windows this must be `mdbx_env_openW` with UTF-16: `mdbx_env_open`
/// decodes its `char*` with the ANSI code page, not UTF-8.
unsafe fn env_open(
    env: *mut ffi::MDBX_env,
    path: &Path,
    flags: ffi::MDBX_env_flags_t,
    mode: ffi::mdbx_mode_t,
) -> Result<()> {
    const NUL_IN_PATH: Error = Error::InvalidArgument("database path contains a NUL byte");
    #[cfg(unix)]
    let rc = {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| NUL_IN_PATH)?;
        unsafe { ffi::mdbx_env_open(env, path.as_ptr(), flags, mode) }
    };
    #[cfg(windows)]
    let rc = {
        use std::os::windows::ffi::OsStrExt;
        let mut path: Vec<u16> = path.as_os_str().encode_wide().collect();
        if path.contains(&0) {
            return Err(NUL_IN_PATH);
        }
        path.push(0);
        unsafe { ffi::mdbx_env_openW(env, path.as_ptr().cast(), flags, mode) }
    };
    mdbx_result(rc).map(drop)
}

/// How a [Database] writes: through the memory map ([WriteMap]) or through
/// file I/O ([NoWriteMap]). Sealed.
#[sealed]
pub trait DatabaseKind: Debug + 'static {
    #[doc(hidden)]
    const EXTRA_FLAGS: ffi::MDBX_env_flags_t;
}

/// [DatabaseKind] that buffers modified pages and writes them with file I/O.
#[derive(Debug)]
pub struct NoWriteMap;
/// [DatabaseKind] that writes directly through the memory map (`MDBX_WRITEMAP`);
/// typically faster, but nested transactions are unavailable.
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

/// A raw libmdbx transaction handle, for direct FFI use; see
/// [Transaction::txn](crate::Transaction::txn).
#[derive(Copy, Clone, Debug)]
pub struct TxnPtr(pub *mut ffi::MDBX_txn);
unsafe impl Send for TxnPtr {}

/// A raw libmdbx environment handle, for direct FFI use; see [Database::ptr].
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
    writer_gate: WriterGate,
    _marker: PhantomData<E>,
}

/// Admits one write transaction at a time within this process.
///
/// Every write transaction is begun by the manager thread, so libmdbx sees the
/// writer lock as already held by the calling thread and fails a second
/// in-process begin with `MDBX_BUSY` instead of blocking. Waiting here rather
/// than retrying wakes the next writer as soon as the current one ends.
#[derive(Default)]
struct WriterGate {
    busy: Mutex<bool>,
    freed: Condvar,
}

impl WriterGate {
    fn acquire(&self) -> WriteSlot<'_> {
        let mut busy = self.busy.lock();
        while *busy {
            self.freed.wait(&mut busy);
        }
        *busy = true;
        WriteSlot(self)
    }
}

/// The right to run the environment's write transaction, released on drop.
///
/// A top-level write [Transaction] holds it until after its commit or abort.
pub(crate) struct WriteSlot<'db>(&'db WriterGate);

impl Drop for WriteSlot<'_> {
    fn drop(&mut self) {
        *self.0.busy.lock() = false;
        self.0.freed.notify_one();
    }
}

/// Options for [Database::open_with_options]. `None` and `false` keep
/// libmdbx's defaults.
#[derive(Clone, Default)]
pub struct DatabaseOptions {
    /// Unix permission bits for newly created files (default `0o644`).
    pub permissions: Option<ffi::mdbx_mode_t>,
    /// Maximum number of simultaneous read transactions (reader slots).
    pub max_readers: Option<c_uint>,
    /// Maximum number of named tables. The default is 0: named tables
    /// cannot be opened unless this is set.
    pub max_tables: Option<u64>,
    /// Limit on GC records read to find contiguous free pages
    /// (`MDBX_opt_rp_augment_limit`).
    pub rp_augment_limit: Option<u64>,
    /// Maximum number of loose pages a write transaction keeps for reuse
    /// (`MDBX_opt_loose_limit`).
    pub loose_limit: Option<u64>,
    /// Maximum number of freed dirty-page buffers kept for reuse
    /// (`MDBX_opt_dp_reserve_limit`).
    pub dp_reserve_limit: Option<u64>,
    /// Maximum number of dirty pages a write transaction holds before
    /// spilling (`MDBX_opt_txn_dp_limit`).
    pub txn_dp_limit: Option<u64>,
    /// Largest fraction (1/N) of dirty pages spilled at once
    /// (`MDBX_opt_spill_max_denominator`).
    pub spill_max_denominator: Option<u64>,
    /// Smallest fraction (1/N) of dirty pages spilled at once
    /// (`MDBX_opt_spill_min_denominator`).
    pub spill_min_denominator: Option<u64>,
    /// Page size for a newly created database; ignored in [Mode::ReadOnly].
    pub page_size: Option<PageSize>,
    /// The path names the data file itself (the lock file gets a `-lck`
    /// suffix) instead of a directory (`MDBX_NOSUBDIR`).
    pub no_sub_dir: bool,
    /// Open exclusively: no other process may use the database
    /// (`MDBX_EXCLUSIVE`).
    pub exclusive: bool,
    /// Adopt the mode flags of an environment already open in another
    /// process instead of failing on a mismatch (`MDBX_ACCEDE`).
    pub accede: bool,
    /// Open mode. When this is [Mode::ReadOnly], the geometry settings of
    /// [ReadWriteOptions] are ignored: MDBX only allows setting geometry for
    /// read-write environments.
    pub mode: Mode,
    /// Disable OS readahead on the memory map (`MDBX_NORDAHEAD`).
    pub no_rdahead: bool,
    /// Skip zero-initializing page buffers before writing them
    /// (`MDBX_NOMEMINIT`): faster, but unused page space may hold stale
    /// memory contents.
    pub no_meminit: bool,
    /// Reuse freed pages last-in first-out (`MDBX_LIFORECLAIM`).
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

        if self.liforeclaim {
            flags |= ffi::MDBX_LIFORECLAIM;
        }

        // MDBX_NOSTICKYTHREADS is load-bearing: this crate's soundness relies on
        // transactions being usable from any thread (see Transaction's Sync impl
        // and the RW-txn manager thread).
        flags |= ffi::MDBX_NOSTICKYTHREADS;

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

    /// Open a database with the given options, creating it if needed (unless
    /// [Mode::ReadOnly]).
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: DatabaseOptions,
    ) -> Result<Database<E>> {
        crate::logging::install();
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
                    (
                        ffi::MDBX_opt_max_readers,
                        options.max_readers.map(u64::from),
                    ),
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

                env_open(
                    db,
                    path.as_ref(),
                    options.make_flags() | E::EXTRA_FLAGS,
                    options.permissions.unwrap_or(0o644),
                )?;

                Ok(())
            })() {
                ffi::mdbx_env_close_ex(db, false);

                return Err(e);
            }
        }

        let mut db = Database {
            inner: DbPtr(db),
            txn_manager: None,
            writer_gate: WriterGate::default(),
            _marker: PhantomData,
        };

        #[allow(clippy::redundant_locals)]
        if let Mode::ReadWrite { .. } = options.mode {
            let (tx, rx) = std::sync::mpsc::sync_channel(0);
            let e = db.inner;
            std::thread::spawn(move || {
                loop {
                    match rx.recv() {
                        Ok(msg) => match msg {
                            TxnManagerMessage::Begin {
                                parent,
                                flags,
                                sender,
                            } => {
                                let e = e;
                                let mut txn: *mut ffi::MDBX_txn = ptr::null_mut();
                                let _ = sender.send(
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
                                );
                            }
                            TxnManagerMessage::Abort { tx, sender } => {
                                let _ = sender.send(mdbx_result(unsafe {
                                    ffi::mdbx_txn_abort_ex(tx.0, ptr::null_mut())
                                }));
                            }
                            TxnManagerMessage::Commit { tx, sender } => {
                                let _ = sender.send(mdbx_result(unsafe {
                                    ffi::mdbx_txn_commit_ex(tx.0, ptr::null_mut())
                                }));
                            }
                        },
                        Err(_) => return,
                    }
                }
            });

            db.txn_manager = Some(tx);
        }

        Ok(db)
    }

    /// Returns the raw libmdbx environment handle.
    ///
    /// The caller must not use it after the database is dropped.
    pub fn ptr(&self) -> DbPtr {
        self.inner
    }

    /// Create a read-only transaction for use with the database.
    pub fn begin_ro_txn(&self) -> Result<Transaction<'_, RO, E>> {
        Transaction::new(self)
    }

    /// Create a read-write transaction for use with the database.
    ///
    /// Blocks while another read-write transaction is open on the database,
    /// in this process or another; waiting in-process writers start as soon as
    /// the current one commits or aborts. Calling this while the same thread
    /// holds a write transaction on this database therefore never returns.
    pub fn begin_rw_txn(&self) -> Result<Transaction<'_, RW, E>> {
        let manager = self.txn_manager.as_ref().ok_or(Error::Access)?;
        let slot = self.writer_gate.acquire();
        let (sender, rx) = sync_channel(0);
        manager
            .send(TxnManagerMessage::Begin {
                parent: TxnPtr(ptr::null_mut()),
                flags: RW::OPEN_FLAGS,
                sender,
            })
            .map_err(|_| Error::Panic)?;
        let txn = rx.recv().map_err(|_| Error::Panic)??;
        Ok(Transaction::new_from_ptr(self, txn.0, Some(slot)))
    }

    /// Flush the database data buffers to disk.
    ///
    /// Returns `true` if there was no data pending for flush to disk.
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
    ///   and the freelist count is stored at the beginning of the value as 32-bit integer
    ///   in the native byte order.
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

            freelist +=
                u32::from_ne_bytes(value.deref()[..mem::size_of::<u32>()].try_into().unwrap())
                    as usize;
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

/// The database file's size limits and growth policy, in bytes; see
/// [ReadWriteOptions](crate::ReadWriteOptions).
#[repr(transparent)]
pub struct GeometryInfo(ffi::MDBX_envinfo__bindgen_ty_1);

impl GeometryInfo {
    /// Lower bound of the file size.
    pub fn min_size(&self) -> u64 {
        self.0.lower
    }

    /// Upper bound of the file size.
    pub fn max_size(&self) -> u64 {
        self.0.upper
    }

    /// Current file size.
    pub fn current_size(&self) -> u64 {
        self.0.current
    }

    /// Step by which the file grows.
    pub fn growth_step(&self) -> u64 {
        self.0.grow
    }

    /// Free space at the end of the file above which it is shrunk.
    pub fn shrink_threshold(&self) -> u64 {
        self.0.shrink
    }
}

/// Database information.
///
/// Contains database information about the map size, readers, last txn id etc.
#[repr(transparent)]
pub struct Info(ffi::MDBX_envinfo);

impl Info {
    /// Size limits and growth policy of the database file.
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
    pub fn last_txnid(&self) -> u64 {
        self.0.mi_recent_txnid
    }

    /// Max reader slots in the database
    #[inline]
    pub fn max_readers(&self) -> usize {
        self.0.mi_maxreaders as usize
    }

    /// Reader slots currently in use
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

/// Page size of a newly created database; see [DatabaseOptions::page_size].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageSize {
    /// The smallest page size libmdbx accepts.
    MinimalAcceptable,
    /// This size in bytes: a power of two in libmdbx's supported range.
    Set(usize),
}
