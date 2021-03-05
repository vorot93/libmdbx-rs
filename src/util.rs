use crate::Error;
use lifetimed_bytes::Bytes;
use std::slice;

pub unsafe fn freeze_bytes<'b>(txn: *const ffi::MDBX_txn, data_val: &ffi::MDBX_val) -> Result<Bytes<'b>, Error> {
    let is_dirty = match ffi::mdbx_is_dirty(txn, data_val.iov_base) {
        ffi::MDBX_RESULT_TRUE => true,
        ffi::MDBX_RESULT_FALSE => false,
        other => return Err(Error::from_err_code(other)),
    };

    let s = slice::from_raw_parts(data_val.iov_base as *const u8, data_val.iov_len);

    Ok(if is_dirty {
        Bytes::from(s.to_vec())
    } else {
        Bytes::from(s)
    })
}
