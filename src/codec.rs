use crate::{Error, TransactionKind};
use derive_more::{Deref, DerefMut, Display};
use std::{borrow::Cow, slice};
use thiserror::Error;

/// Implement this to be able to decode data values.
///
/// # Zero-copy contract
///
/// The default [`Decodable::decode_val`] receives a slice pointing directly
/// into the database's memory map. That memory is owned by MDBX:
///
/// * It must never be written through.
/// * In a read-only transaction the bytes are stable for the duration of the
///   transaction (MVCC snapshot). Returning borrowed views (e.g.
///   [`Cow::Borrowed`]) is sound there.
/// * In a read-write transaction MDBX may relocate or overwrite pages on any
///   subsequent write operation. Implementations MUST copy (like this crate's
///   [`Cow`] impl does) unless they override `decode_val` and take full
///   responsibility for invalidation.
pub trait Decodable<'tx> {
    fn decode(data_val: &[u8]) -> Result<Self, Error>
    where
        Self: Sized;

    #[doc(hidden)]
    unsafe fn decode_val<K: TransactionKind>(
        _: *const ffi::MDBX_txn,
        data_val: &ffi::MDBX_val,
    ) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let s: &[u8] = unsafe {
            if data_val.iov_len == 0 {
                &[]
            } else {
                slice::from_raw_parts(data_val.iov_base as *const u8, data_val.iov_len)
            }
        };

        Decodable::decode(s)
    }
}

impl<'tx> Decodable<'tx> for Cow<'tx, [u8]> {
    fn decode(data_val: &[u8]) -> Result<Self, Error> {
        Ok(Cow::Owned(data_val.to_vec()))
    }

    #[doc(hidden)]
    unsafe fn decode_val<K: TransactionKind>(
        _: *const ffi::MDBX_txn,
        data_val: &ffi::MDBX_val,
    ) -> Result<Self, Error> {
        let s: &[u8] = unsafe {
            if data_val.iov_len == 0 {
                &[]
            } else {
                slice::from_raw_parts(data_val.iov_base as *const u8, data_val.iov_len)
            }
        };

        // RW transactions may relocate pages on subsequent writes; copy for safety.
        Ok(if K::ONLY_CLEAN {
            Cow::Borrowed(s)
        } else {
            Cow::Owned(s.to_vec())
        })
    }
}

#[cfg(feature = "lifetimed-bytes")]
impl<'tx> Decodable<'tx> for lifetimed_bytes::Bytes<'tx> {
    fn decode(data_val: &[u8]) -> Result<Self, Error> {
        Ok(Self::from(data_val.to_vec()))
    }

    #[doc(hidden)]
    unsafe fn decode_val<K: TransactionKind>(
        txn: *const ffi::MDBX_txn,
        data_val: &ffi::MDBX_val,
    ) -> Result<Self, Error> {
        unsafe { Cow::<'tx, [u8]>::decode_val::<K>(txn, data_val).map(From::from) }
    }
}

#[cfg(feature = "bytes")]
impl Decodable<'_> for bytes::Bytes {
    fn decode(data_val: &[u8]) -> Result<Self, Error> {
        Ok(Self::copy_from_slice(data_val))
    }
}

impl Decodable<'_> for Vec<u8> {
    fn decode(data_val: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(data_val.to_vec())
    }
}

impl Decodable<'_> for () {
    fn decode(_: &[u8]) -> Result<Self, Error> {
        Ok(())
    }

    unsafe fn decode_val<K: TransactionKind>(
        _: *const ffi::MDBX_txn,
        _: &ffi::MDBX_val,
    ) -> Result<Self, Error> {
        Ok(())
    }
}

/// If you don't need the data itself, just its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deref, DerefMut)]
pub struct ObjectLength(pub usize);

impl Decodable<'_> for ObjectLength {
    fn decode(data_val: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(Self(data_val.len()))
    }
}

impl<const LEN: usize> Decodable<'_> for [u8; LEN] {
    fn decode(data_val: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        #[derive(Clone, Debug, Display, Error)]
        struct InvalidSize<const LEN: usize> {
            got: usize,
        }

        if data_val.len() != LEN {
            return Err(Error::DecodeError(Box::new(InvalidSize::<LEN> {
                got: data_val.len(),
            })));
        }
        let mut a = [0; LEN];
        a[..].copy_from_slice(data_val);
        Ok(a)
    }
}
