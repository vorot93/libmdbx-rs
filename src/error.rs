use libc::c_int;
use std::{
    ffi::CStr,
    fmt,
    os::raw::c_char,
    result,
    str,
};

/// An MDBX error kind.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
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
    Incompatible,
    BadRslot,
    BadTxn,
    BadValSize,
    BadDbi,
    Other(c_int),
}

impl Error {
    /// Converts a raw error code to an `Error`.
    pub fn from_err_code(err_code: c_int) -> Error {
        match err_code {
            ffi::MDBX_KEYEXIST => Error::KeyExist,
            ffi::MDBX_NOTFOUND => Error::NotFound,
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
            ffi::MDBX_INCOMPATIBLE => Error::Incompatible,
            ffi::MDBX_BAD_RSLOT => Error::BadRslot,
            ffi::MDBX_BAD_TXN => Error::BadTxn,
            ffi::MDBX_BAD_VALSIZE => Error::BadValSize,
            ffi::MDBX_BAD_DBI => Error::BadDbi,
            other => Error::Other(other),
        }
    }

    /// Converts an `Error` to the raw error code.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn to_err_code(&self) -> c_int {
        match self {
            Error::KeyExist => ffi::MDBX_KEYEXIST,
            Error::NotFound => ffi::MDBX_NOTFOUND,
            Error::PageNotFound => ffi::MDBX_PAGE_NOTFOUND,
            Error::Corrupted => ffi::MDBX_CORRUPTED,
            Error::Panic => ffi::MDBX_PANIC,
            Error::VersionMismatch => ffi::MDBX_VERSION_MISMATCH,
            Error::Invalid => ffi::MDBX_INVALID,
            Error::MapFull => ffi::MDBX_MAP_FULL,
            Error::DbsFull => ffi::MDBX_DBS_FULL,
            Error::ReadersFull => ffi::MDBX_READERS_FULL,
            Error::TxnFull => ffi::MDBX_TXN_FULL,
            Error::CursorFull => ffi::MDBX_CURSOR_FULL,
            Error::PageFull => ffi::MDBX_PAGE_FULL,
            Error::Incompatible => ffi::MDBX_INCOMPATIBLE,
            Error::BadRslot => ffi::MDBX_BAD_RSLOT,
            Error::BadTxn => ffi::MDBX_BAD_TXN,
            Error::BadValSize => ffi::MDBX_BAD_VALSIZE,
            Error::BadDbi => ffi::MDBX_BAD_DBI,
            Error::Other(err_code) => *err_code,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", unsafe {
            // This is safe since the error messages returned from mdbx_strerror are static.
            let err: *const c_char = ffi::mdbx_strerror(self.to_err_code()) as *const c_char;
            str::from_utf8_unchecked(CStr::from_ptr(err).to_bytes())
        })
    }
}

/// An LMDB result.
pub type Result<T> = result::Result<T, Error>;

pub fn mdbx_result(err_code: c_int) -> Result<()> {
    if err_code == ffi::MDBX_SUCCESS as i32 {
        Ok(())
    } else {
        Err(Error::from_err_code(err_code))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_description() {
        assert_eq!("Permission denied", Error::from_err_code(13).to_string());
        assert_eq!("MDB_NOTFOUND: No matching key/data pair found", Error::NotFound.to_string());
    }
}
