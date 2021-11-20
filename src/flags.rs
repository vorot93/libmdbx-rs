use ffi::MDBX_env_flags_t;

/// MDBX sync mode
#[derive(Clone, Copy, Debug)]
pub enum SyncMode {
    /// Default robust and durable sync mode.
    /// Metadata is written and flushed to disk after a data is written and flushed, which guarantees the integrity of the database in the event of a crash at any time.
    Durable,

    /// Don't sync the meta-page after commit.
    ///
    /// Flush system buffers to disk only once per transaction commit, omit the metadata flush.
    /// Defer that until the system flushes files to disk, or next non-read-only commit or [Environment::sync()](crate::Environment::sync).
    /// Depending on the platform and hardware, with [SyncMode::NoMetaSync] you may get a doubling of write performance.
    ///
    /// This trade-off maintains database integrity, but a system crash may undo the last committed transaction.
    /// I.e. it preserves the ACI (atomicity, consistency, isolation) but not D (durability) database property.
    NoMetaSync,

    /// Don't sync anything but keep previous steady commits.
    ///
    /// [SyncMode::UtterlyNoSync] the [SyncMode::SafeNoSync] flag disable similarly flush system buffers to disk when committing a transaction.
    /// But there is a huge difference in how are recycled the MVCC snapshots corresponding to previous "steady" transactions (see below).
    ///
    /// With [crate::WriteMap] the [SyncMode::SafeNoSync] instructs MDBX to use asynchronous mmap-flushes to disk.
    /// Asynchronous mmap-flushes means that actually all writes will scheduled and performed by operation system on it own manner, i.e. unordered.
    /// MDBX itself just notify operating system that it would be nice to write data to disk, but no more.
    ///
    /// Depending on the platform and hardware, with [SyncMode::SafeNoSync] you may get a multiple increase of write performance, even 10 times or more.
    ///
    /// In contrast to [SyncMode::UtterlyNoSync] mode, with [SyncMode::SafeNoSync] flag MDBX will keeps untouched pages within B-tree of the last transaction "steady" which was synced to disk completely.
    /// This has big implications for both data durability and (unfortunately) performance:
    ///
    /// A system crash can't corrupt the database, but you will lose the last transactions; because MDBX will rollback to last steady commit since it kept explicitly.
    /// The last steady transaction makes an effect similar to "long-lived" read transaction since prevents reuse of pages freed by newer write transactions, thus the any data changes will be placed in newly allocated pages.
    /// To avoid rapid database growth, the system will sync data and issue a steady commit-point to resume reuse pages, each time there is insufficient space and before increasing the size of the file on disk.
    /// In other words, with [SyncMode::SafeNoSync] flag MDBX protects you from the whole database corruption, at the cost increasing database size and/or number of disk IOPs.
    /// So, [SyncMode::SafeNoSync] flag could be used with [Environment::sync()](crate::Environment::sync) as alternatively for batch committing or nested transaction (in some cases).
    ///
    /// The number and volume of of disk IOPs with [SyncMode::SafeNoSync] flag will exactly the as without any no-sync flags.
    /// However, you should expect a larger process's work set and significantly worse a locality of reference, due to the more intensive allocation of previously unused pages and increase the size of the database.
    SafeNoSync,

    /// Don't sync anything and wipe previous steady commits.
    ///
    /// Don't flush system buffers to disk when committing a transaction.
    /// This optimization means a system crash can corrupt the database, if buffers are not yet flushed to disk.
    /// Depending on the platform and hardware, with [SyncMode::UtterlyNoSync] you may get a multiple increase of write performance, even 100 times or more.
    ///
    /// If the filesystem preserves write order (which is rare and never provided unless explicitly noted) and the [WriteMap](crate::WriteMap) and [EnvironmentFlags::liforeclaim] flags are not used,
    /// then a system crash can't corrupt the database, but you can lose the last transactions, if at least one buffer is not yet flushed to disk.
    /// The risk is governed by how often the system flushes dirty buffers to disk and how often [Environment::sync()](crate::Environment::sync) is called.
    /// So, transactions exhibit ACI (atomicity, consistency, isolation) properties and only lose D (durability).
    /// I.e. database integrity is maintained, but a system crash may undo the final transactions.
    ///
    /// Otherwise, if the filesystem not preserves write order (which is typically) or [WriteMap](crate::WriteMap) or [EnvironmentFlags::liforeclaim] flags are used, you should expect the corrupted database after a system crash.
    ///
    /// So, most important thing about [SyncMode::UtterlyNoSync]:
    ///
    /// A system crash immediately after commit the write transaction high likely lead to database corruption.
    /// Successful completion of [Environment::sync(force=true)](crate::Environment::sync) after one or more committed transactions guarantees consistency and durability.
    /// BUT by committing two or more transactions you back database into a weak state, in which a system crash may lead to database corruption!
    /// In case single transaction after [Environment::sync()](crate::Environment::sync), you may lose transaction itself, but not a whole database.
    /// Nevertheless, [SyncMode::UtterlyNoSync] provides "weak" durability in case of an application crash (but no durability on system failure),
    /// and therefore may be very useful in scenarios where data durability is not required over a system failure (e.g for short-lived data), or if you can take such risk.
    UtterlyNoSync,
}

impl Default for SyncMode {
    fn default() -> Self {
        Self::Durable
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    ReadOnly,
    ReadWrite { sync_mode: SyncMode },
}

impl Default for Mode {
    fn default() -> Self {
        Self::ReadWrite {
            sync_mode: SyncMode::default(),
        }
    }
}

impl From<Mode> for EnvironmentFlags {
    fn from(mode: Mode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EnvironmentFlags {
    pub no_sub_dir: bool,
    pub exclusive: bool,
    pub accede: bool,
    pub mode: Mode,
    pub no_rdahead: bool,
    pub no_meminit: bool,
    pub coalesce: bool,
    pub liforeclaim: bool,
}

impl EnvironmentFlags {
    pub(crate) fn make_flags(&self) -> MDBX_env_flags_t {
        let mut flags = MDBX_env_flags_t::MDBX_ENV_DEFAULTS;

        if self.no_sub_dir {
            flags |= MDBX_env_flags_t::MDBX_NOSUBDIR;
        }

        if self.exclusive {
            flags |= MDBX_env_flags_t::MDBX_EXCLUSIVE;
        }

        if self.accede {
            flags |= MDBX_env_flags_t::MDBX_ACCEDE;
        }

        match self.mode {
            Mode::ReadOnly => {
                flags |= MDBX_env_flags_t::MDBX_RDONLY;
            }
            Mode::ReadWrite { sync_mode } => {
                flags |= match sync_mode {
                    SyncMode::Durable => MDBX_env_flags_t::MDBX_SYNC_DURABLE,
                    SyncMode::NoMetaSync => MDBX_env_flags_t::MDBX_NOMETASYNC,
                    SyncMode::SafeNoSync => MDBX_env_flags_t::MDBX_SAFE_NOSYNC,
                    SyncMode::UtterlyNoSync => MDBX_env_flags_t::MDBX_UTTERLY_NOSYNC,
                };
            }
        }

        if self.no_rdahead {
            flags |= MDBX_env_flags_t::MDBX_NORDAHEAD;
        }

        if self.no_meminit {
            flags |= MDBX_env_flags_t::MDBX_NOMEMINIT;
        }

        if self.coalesce {
            flags |= MDBX_env_flags_t::MDBX_COALESCE;
        }

        if self.liforeclaim {
            flags |= MDBX_env_flags_t::MDBX_LIFORECLAIM;
        }

        flags |= MDBX_env_flags_t::MDBX_NOTLS;

        flags
    }
}

/*
bitflags! {
    #[doc="Database options."]
    #[derive(Default)]
    pub struct DatabaseFlags: c_uint {
        const REVERSE_KEY = MDBX_REVERSEKEY;
        const DUP_SORT = MDBX_DUPSORT;
        const INTEGER_KEY = MDBX_INTEGERKEY;
        const DUP_FIXED = MDBX_DUPFIXED;
        const INTEGER_DUP = MDBX_INTEGERDUP;
        const REVERSE_DUP = MDBX_REVERSEDUP;
        const CREATE = MDBX_CREATE;
        const ACCEDE = MDBX_DB_ACCEDE;
    }
}
bitflags! {
    #[doc="Write options."]
    #[derive(Default)]
    pub struct WriteFlags: c_uint {
        const UPSERT = ffi::MDBX_put_flags_t::MDBX_UPSERT;
        const NO_OVERWRITE = ffi::MDBX_put_flags_t::MDBX_NOOVERWRITE;
        const NO_DUP_DATA = ffi::MDBX_put_flags_t::MDBX_NODUPDATA;
        const CURRENT = ffi::MDBX_put_flags_t::MDBX_CURRENT;
        const ALLDUPS = ffi::MDBX_put_flags_t::MDBX_ALLDUPS;
        const RESERVE = ffi::MDBX_put_flags_t::MDBX_RESERVE;
        const APPEND = ffi::MDBX_put_flags_t::MDBX_APPEND;
        const APPEND_DUP = ffi::MDBX_put_flags_t::ffi::MDBX_put_flags_t::MDBX_APPENDDUP;
        const MULTIPLE = ffi::MDBX_put_flags_t::MDBX_MULTIPLE;
    }
}
*/
