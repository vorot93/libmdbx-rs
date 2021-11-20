use ffi::MDBX_error_t;
use std::{ffi::CStr, fmt, result, str};
/// An MDBX error kind.
#[derive(Debug)]
pub enum Error {
    KeyExist,
    NotFound,
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
    DecodeError(Box<dyn std::error::Error + Send + Sync + 'static>),
    Other(MDBX_error_t),
}

impl Error {
    /// Converts a raw error code to an [Error].
    pub fn from_err_code(err_code: MDBX_error_t) -> Error {
        match err_code {
            MDBX_error_t::MDBX_KEYEXIST => Error::KeyExist,
            MDBX_error_t::MDBX_NOTFOUND => Error::NotFound,
            MDBX_error_t::MDBX_PAGE_NOTFOUND => Error::PageNotFound,
            MDBX_error_t::MDBX_CORRUPTED => Error::Corrupted,
            MDBX_error_t::MDBX_PANIC => Error::Panic,
            MDBX_error_t::MDBX_VERSION_MISMATCH => Error::VersionMismatch,
            MDBX_error_t::MDBX_INVALID => Error::Invalid,
            MDBX_error_t::MDBX_MAP_FULL => Error::MapFull,
            MDBX_error_t::MDBX_DBS_FULL => Error::DbsFull,
            MDBX_error_t::MDBX_READERS_FULL => Error::ReadersFull,
            MDBX_error_t::MDBX_TXN_FULL => Error::TxnFull,
            MDBX_error_t::MDBX_CURSOR_FULL => Error::CursorFull,
            MDBX_error_t::MDBX_PAGE_FULL => Error::PageFull,
            MDBX_error_t::MDBX_UNABLE_EXTEND_MAPSIZE => Error::UnableExtendMapsize,
            MDBX_error_t::MDBX_INCOMPATIBLE => Error::Incompatible,
            MDBX_error_t::MDBX_BAD_RSLOT => Error::BadRslot,
            MDBX_error_t::MDBX_BAD_TXN => Error::BadTxn,
            MDBX_error_t::MDBX_BAD_VALSIZE => Error::BadValSize,
            MDBX_error_t::MDBX_BAD_DBI => Error::BadDbi,
            MDBX_error_t::MDBX_PROBLEM => Error::Problem,
            MDBX_error_t::MDBX_BUSY => Error::Busy,
            MDBX_error_t::MDBX_EMULTIVAL => Error::Multival,
            MDBX_error_t::MDBX_WANNA_RECOVERY => Error::WannaRecovery,
            MDBX_error_t::MDBX_EKEYMISMATCH => Error::KeyMismatch,
            MDBX_error_t::MDBX_EINVAL => Error::InvalidValue,
            MDBX_error_t::MDBX_EACCESS => Error::Access,
            MDBX_error_t::MDBX_TOO_LARGE => Error::TooLarge,
            other => Error::Other(other),
        }
    }

    /// Converts an [Error] to the raw error code.
    fn to_err_code(&self) -> MDBX_error_t {
        match self {
            Error::KeyExist => MDBX_error_t::MDBX_KEYEXIST,
            Error::NotFound => MDBX_error_t::MDBX_NOTFOUND,
            Error::PageNotFound => MDBX_error_t::MDBX_PAGE_NOTFOUND,
            Error::Corrupted => MDBX_error_t::MDBX_CORRUPTED,
            Error::Panic => MDBX_error_t::MDBX_PANIC,
            Error::VersionMismatch => MDBX_error_t::MDBX_VERSION_MISMATCH,
            Error::Invalid => MDBX_error_t::MDBX_INVALID,
            Error::MapFull => MDBX_error_t::MDBX_MAP_FULL,
            Error::DbsFull => MDBX_error_t::MDBX_DBS_FULL,
            Error::ReadersFull => MDBX_error_t::MDBX_READERS_FULL,
            Error::TxnFull => MDBX_error_t::MDBX_TXN_FULL,
            Error::CursorFull => MDBX_error_t::MDBX_CURSOR_FULL,
            Error::PageFull => MDBX_error_t::MDBX_PAGE_FULL,
            Error::UnableExtendMapsize => MDBX_error_t::MDBX_UNABLE_EXTEND_MAPSIZE,
            Error::Incompatible => MDBX_error_t::MDBX_INCOMPATIBLE,
            Error::BadRslot => MDBX_error_t::MDBX_BAD_RSLOT,
            Error::BadTxn => MDBX_error_t::MDBX_BAD_TXN,
            Error::BadValSize => MDBX_error_t::MDBX_BAD_VALSIZE,
            Error::BadDbi => MDBX_error_t::MDBX_BAD_DBI,
            Error::Problem => MDBX_error_t::MDBX_PROBLEM,
            Error::Busy => MDBX_error_t::MDBX_BUSY,
            Error::Multival => MDBX_error_t::MDBX_EMULTIVAL,
            Error::WannaRecovery => MDBX_error_t::MDBX_WANNA_RECOVERY,
            Error::KeyMismatch => MDBX_error_t::MDBX_EKEYMISMATCH,
            Error::InvalidValue => MDBX_error_t::MDBX_EINVAL,
            Error::Access => MDBX_error_t::MDBX_EACCESS,
            Error::TooLarge => MDBX_error_t::MDBX_TOO_LARGE,
            Error::Other(err_code) => *err_code,
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DecodeError(reason) => write!(fmt, "{}", reason),
            other => {
                write!(fmt, "{}", unsafe {
                    let err = ffi::mdbx_strerror(other.to_err_code().0);
                    str::from_utf8_unchecked(CStr::from_ptr(err).to_bytes())
                })
            }
        }
    }
}

impl std::error::Error for Error {}

/// An MDBX result.
pub type Result<T> = result::Result<T, Error>;

pub fn _mdbx_result(err_code: MDBX_error_t) -> Result<bool> {
    match err_code {
        MDBX_error_t::MDBX_SUCCESS => Ok(false),
        MDBX_error_t::MDBX_RESULT_TRUE => Ok(true),
        other => Err(Error::from_err_code(other)),
    }
}

#[macro_export]
macro_rules! mdbx_result {
    ($expr:expr) => {
        crate::error::_mdbx_result(ffi::MDBX_error_t($expr))
    };
}

#[macro_export]
macro_rules! mdbx_try_optional {
    ($expr:expr) => {{
        match $expr {
            Err(Error::NotFound) => return Ok(None),
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
        assert_eq!("Permission denied", Error::from_err_code(13).to_string());
        assert_eq!(
            "MDBX_error_t::MDBX_INVALID: File is not an MDBX file",
            Error::Invalid.to_string()
        );
    }
}
