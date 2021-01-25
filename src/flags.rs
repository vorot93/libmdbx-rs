use bitflags::bitflags;
use ffi::*;
use libc::c_uint;

bitflags! {
    #[doc="Environment options."]
    #[derive(Default)]
    pub struct EnvironmentFlags: c_uint {
        const NO_SUB_DIR = MDBX_NOSUBDIR;
        const READ_ONLY = MDBX_RDONLY;
        const EXCLUSIVE = MDBX_EXCLUSIVE;
        const ACCEDE = MDBX_ACCEDE;
        const WRITE_MAP = MDBX_WRITEMAP;
        const NO_TLS = MDBX_NOTLS;
        const NO_READAHEAD = MDBX_NORDAHEAD;
        const NO_MEM_INIT = MDBX_NOMEMINIT;
        const COALESCE = MDBX_COALESCE;
        const LIFORECLAIM = MDBX_LIFORECLAIM;
        const PAGEPERTURB = MDBX_PAGEPERTURB;
        const SYNC_DURABLE = MDBX_SYNC_DURABLE;
        const NO_META_SYNC = MDBX_NOMETASYNC;
        const SAFE_NO_SYNC = MDBX_SAFE_NOSYNC;
        const MAP_ASYNC = MDBX_MAPASYNC;
        const UTTERLY_NO_SYNC = MDBX_UTTERLY_NOSYNC;
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
