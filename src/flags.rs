use bitflags::bitflags;
use ffi::*;
use libc::c_uint;

#[derive(Clone, Copy, Debug)]
pub enum SyncMode {
    Durable,
    NoMetaSync,
    SafeNoSync,
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
    ReadWrite {
        sync_mode: SyncMode,
    },
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
            },
            Mode::ReadWrite {
                sync_mode,
            } => {
                flags |= match sync_mode {
                    SyncMode::Durable => ffi::MDBX_SYNC_DURABLE,
                    SyncMode::NoMetaSync => ffi::MDBX_NOMETASYNC,
                    SyncMode::SafeNoSync => ffi::MDBX_SAFE_NOSYNC,
                    SyncMode::UtterlyNoSync => ffi::MDBX_UTTERLY_NOSYNC,
                };
            },
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
        const UPSERT = MDBX_UPSERT;
        const NO_OVERWRITE = MDBX_NOOVERWRITE;
        const NO_DUP_DATA = MDBX_NODUPDATA;
        const CURRENT = MDBX_CURRENT;
        const ALLDUPS = MDBX_ALLDUPS;
        const RESERVE = MDBX_RESERVE;
        const APPEND = MDBX_APPEND;
        const APPEND_DUP = MDBX_APPENDDUP;
        const MULTIPLE = MDBX_MULTIPLE;
    }
}
