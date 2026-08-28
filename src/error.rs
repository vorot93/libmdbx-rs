use libc::c_int;
use std::{ffi::CStr, fmt, result};

/// An MDBX error kind.
#[derive(Debug)]
pub enum Error {
    KeyExist,
    NotFound,
    NoData,
    PageNotFound,
    Corrupted,
    Panic,
    VersionMismatch,
    Invalid,
    MapFull,
    DbsFull,
    ReadersFull,
    TxnFull,
    CursorFull,
    PageFull,
    UnableExtendMapsize,
    Incompatible,
    BadRslot,
    BadTxn,
    BadValSize,
    BadDbi,
    Problem,
    Busy,
    Multival,
    WannaRecovery,
    KeyMismatch,
    InvalidValue,
    Access,
    TooLarge,
    BadSign,
    ThreadMismatch,
    TxnOverlapping,
    BacklogDepleted,
    DuplicatedLck,
    DanglingDbi,
    Ousted,
    MvccRetarded,
    LaggardReader,
    DecodeError(Box<dyn std::error::Error + Send + Sync + 'static>),
    EncodeError(Box<dyn std::error::Error + Send + Sync + 'static>),
    IoError(std::io::Error),
    Other(c_int),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(e)
    }
}

impl Error {
    /// Converts a raw error code to an [Error].
    pub fn from_err_code(err_code: c_int) -> Error {
        match err_code {
            ffi::MDBX_KEYEXIST => Error::KeyExist,
            ffi::MDBX_NOTFOUND => Error::NotFound,
            ffi::MDBX_ENODATA => Error::NoData,
            ffi::MDBX_PAGE_NOTFOUND => Error::PageNotFound,
            ffi::MDBX_CORRUPTED => Error::Corrupted,
            ffi::MDBX_PANIC => Error::Panic,
            ffi::MDBX_VERSION_MISMATCH => Error::VersionMismatch,
            ffi::MDBX_INVALID => Error::Invalid,
            ffi::MDBX_MAP_FULL => Error::MapFull,
            ffi::MDBX_DBS_FULL => Error::DbsFull,
            ffi::MDBX_READERS_FULL => Error::ReadersFull,
            ffi::MDBX_TXN_FULL => Error::TxnFull,
            ffi::MDBX_CURSOR_FULL => Error::CursorFull,
            ffi::MDBX_PAGE_FULL => Error::PageFull,
            ffi::MDBX_UNABLE_EXTEND_MAPSIZE => Error::UnableExtendMapsize,
            ffi::MDBX_INCOMPATIBLE => Error::Incompatible,
            ffi::MDBX_BAD_RSLOT => Error::BadRslot,
            ffi::MDBX_BAD_TXN => Error::BadTxn,
            ffi::MDBX_BAD_VALSIZE => Error::BadValSize,
            ffi::MDBX_BAD_DBI => Error::BadDbi,
            ffi::MDBX_PROBLEM => Error::Problem,
            ffi::MDBX_BUSY => Error::Busy,
            ffi::MDBX_EMULTIVAL => Error::Multival,
            ffi::MDBX_WANNA_RECOVERY => Error::WannaRecovery,
            ffi::MDBX_EKEYMISMATCH => Error::KeyMismatch,
            ffi::MDBX_EINVAL => Error::InvalidValue,
            ffi::MDBX_EACCESS => Error::Access,
            ffi::MDBX_TOO_LARGE => Error::TooLarge,
            ffi::MDBX_EBADSIGN => Error::BadSign,
            ffi::MDBX_THREAD_MISMATCH => Error::ThreadMismatch,
            ffi::MDBX_TXN_OVERLAPPING => Error::TxnOverlapping,
            ffi::MDBX_BACKLOG_DEPLETED => Error::BacklogDepleted,
            ffi::MDBX_DUPLICATED_LCK => Error::DuplicatedLck,
            ffi::MDBX_DANGLING_DBI => Error::DanglingDbi,
            ffi::MDBX_OUSTED => Error::Ousted,
            ffi::MDBX_MVCC_RETARDED => Error::MvccRetarded,
            ffi::MDBX_LAGGARD_READER => Error::LaggardReader,
            other => Error::Other(other),
        }
    }

    /// Converts an [Error] to the raw MDBX error code,
    /// or `None` for errors that don't originate from MDBX.
    fn mdbx_err_code(&self) -> Option<c_int> {
        match self {
            Error::KeyExist => Some(ffi::MDBX_KEYEXIST),
            Error::NotFound => Some(ffi::MDBX_NOTFOUND),
            Error::NoData => Some(ffi::MDBX_ENODATA),
            Error::PageNotFound => Some(ffi::MDBX_PAGE_NOTFOUND),
            Error::Corrupted => Some(ffi::MDBX_CORRUPTED),
            Error::Panic => Some(ffi::MDBX_PANIC),
            Error::VersionMismatch => Some(ffi::MDBX_VERSION_MISMATCH),
            Error::Invalid => Some(ffi::MDBX_INVALID),
            Error::MapFull => Some(ffi::MDBX_MAP_FULL),
            Error::DbsFull => Some(ffi::MDBX_DBS_FULL),
            Error::ReadersFull => Some(ffi::MDBX_READERS_FULL),
            Error::TxnFull => Some(ffi::MDBX_TXN_FULL),
            Error::CursorFull => Some(ffi::MDBX_CURSOR_FULL),
            Error::PageFull => Some(ffi::MDBX_PAGE_FULL),
            Error::UnableExtendMapsize => Some(ffi::MDBX_UNABLE_EXTEND_MAPSIZE),
            Error::Incompatible => Some(ffi::MDBX_INCOMPATIBLE),
            Error::BadRslot => Some(ffi::MDBX_BAD_RSLOT),
            Error::BadTxn => Some(ffi::MDBX_BAD_TXN),
            Error::BadValSize => Some(ffi::MDBX_BAD_VALSIZE),
            Error::BadDbi => Some(ffi::MDBX_BAD_DBI),
            Error::Problem => Some(ffi::MDBX_PROBLEM),
            Error::Busy => Some(ffi::MDBX_BUSY),
            Error::Multival => Some(ffi::MDBX_EMULTIVAL),
            Error::WannaRecovery => Some(ffi::MDBX_WANNA_RECOVERY),
            Error::KeyMismatch => Some(ffi::MDBX_EKEYMISMATCH),
            Error::InvalidValue => Some(ffi::MDBX_EINVAL),
            Error::Access => Some(ffi::MDBX_EACCESS),
            Error::TooLarge => Some(ffi::MDBX_TOO_LARGE),
            Error::BadSign => Some(ffi::MDBX_EBADSIGN),
            Error::ThreadMismatch => Some(ffi::MDBX_THREAD_MISMATCH),
            Error::TxnOverlapping => Some(ffi::MDBX_TXN_OVERLAPPING),
            Error::BacklogDepleted => Some(ffi::MDBX_BACKLOG_DEPLETED),
            Error::DuplicatedLck => Some(ffi::MDBX_DUPLICATED_LCK),
            Error::DanglingDbi => Some(ffi::MDBX_DANGLING_DBI),
            Error::Ousted => Some(ffi::MDBX_OUSTED),
            Error::MvccRetarded => Some(ffi::MDBX_MVCC_RETARDED),
            Error::LaggardReader => Some(ffi::MDBX_LAGGARD_READER),
            Error::Other(err_code) => Some(*err_code),
            Error::DecodeError(_) | Error::EncodeError(_) | Error::IoError(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DecodeError(reason) | Error::EncodeError(reason) => write!(fmt, "{reason}"),
            Error::IoError(reason) => write!(fmt, "{reason}"),
            other => match other.mdbx_err_code() {
                Some(code) => {
                    let mut buf = [0 as libc::c_char; 256];
                    let msg = unsafe {
                        let p = ffi::mdbx_strerror_r(code, buf.as_mut_ptr(), buf.len());
                        CStr::from_ptr(p)
                    };
                    write!(fmt, "{}", String::from_utf8_lossy(msg.to_bytes()))
                }
                None => write!(fmt, "{other:?}"),
            },
        }
    }
}

impl std::error::Error for Error {}

/// An MDBX result.
pub type Result<T> = result::Result<T, Error>;

pub fn mdbx_result(err_code: c_int) -> Result<bool> {
    match err_code {
        ffi::MDBX_SUCCESS => Ok(false),
        ffi::MDBX_RESULT_TRUE => Ok(true),
        other => Err(Error::from_err_code(other)),
    }
}

#[macro_export]
macro_rules! mdbx_try_optional {
    ($expr:expr) => {{
        match $expr {
            Err(Error::NotFound | Error::NoData) => return Ok(None),
            Err(e) => return Err(e),
            Ok(v) => v,
        }
    }};
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_description() {
        #[cfg(not(windows))]
        assert_eq!("Permission denied", Error::from_err_code(13).to_string());

        assert_eq!(
            "MDBX_INVALID: File is not an MDBX file",
            Error::Invalid.to_string()
        );
    }

    #[test]
    fn test_newer_mdbx_codes() {
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_EBADSIGN),
            Error::BadSign
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_THREAD_MISMATCH),
            Error::ThreadMismatch
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_TXN_OVERLAPPING),
            Error::TxnOverlapping
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_BACKLOG_DEPLETED),
            Error::BacklogDepleted
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_DUPLICATED_LCK),
            Error::DuplicatedLck
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_DANGLING_DBI),
            Error::DanglingDbi
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_OUSTED),
            Error::Ousted
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_MVCC_RETARDED),
            Error::MvccRetarded
        ));
        assert!(matches!(
            Error::from_err_code(ffi::MDBX_LAGGARD_READER),
            Error::LaggardReader
        ));
    }

    #[test]
    fn test_display_is_lossy_and_total() {
        // errno path goes through strerror_r; must produce non-empty lossy UTF-8
        let s = Error::Other(13).to_string();
        assert!(!s.is_empty());
        let s = Error::EncodeError("boom".into()).to_string();
        assert_eq!(s, "boom");
    }
}
