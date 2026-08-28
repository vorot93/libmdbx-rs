use super::traits::*;
use arrayvec::ArrayVec;
use derive_more::*;
use std::fmt::Display;

pub(crate) fn dec<E: std::error::Error + Send + Sync + 'static>(e: E) -> crate::Error {
    crate::Error::DecodeError(Box::new(e))
}

#[derive(
    Clone,
    Copy,
    Debug,
    Deref,
    DerefMut,
    Default,
    Display,
    PartialEq,
    Eq,
    From,
    PartialOrd,
    Ord,
    Hash,
)]
pub struct CutStart<T>(pub T);

impl Encodable for () {
    type Encoded = [u8; 0];

    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        Ok([])
    }
}

impl Decodable for () {
    fn decode(b: &[u8]) -> Result<Self, crate::Error> {
        if !b.is_empty() {
            return Err(dec(TooLong::<0> { received: b.len() }));
        }

        Ok(())
    }
}

impl Encodable for Vec<u8> {
    type Encoded = Self;

    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        Ok(self)
    }
}

impl Decodable for Vec<u8> {
    fn decode(b: &[u8]) -> Result<Self, crate::Error> {
        Ok(b.to_vec())
    }
}

#[cfg(feature = "bytes")]
impl Encodable for bytes::Bytes {
    type Encoded = Self;

    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        Ok(self)
    }
}

#[cfg(feature = "bytes")]
impl Decodable for bytes::Bytes {
    fn decode(b: &[u8]) -> Result<Self, crate::Error> {
        Ok(b.to_vec().into())
    }
}

impl Encodable for String {
    type Encoded = Vec<u8>;

    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        Ok(self.into_bytes())
    }
}

impl Decodable for String {
    fn decode(b: &[u8]) -> Result<Self, crate::Error> {
        String::from_utf8(b.into()).map_err(dec)
    }
}

impl<const MAX_LEN: usize> Encodable for ArrayVec<u8, MAX_LEN> {
    type Encoded = Self;

    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        Ok(self)
    }
}

impl<const MAX_LEN: usize> Decodable for ArrayVec<u8, MAX_LEN> {
    fn decode(v: &[u8]) -> Result<Self, crate::Error> {
        let mut out = Self::default();
        out.try_extend_from_slice(v).map_err(dec)?;
        Ok(out)
    }
}

impl<const LEN: usize> Encodable for [u8; LEN] {
    type Encoded = Self;

    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        Ok(self)
    }
}

impl<const LEN: usize> Decodable for [u8; LEN] {
    fn decode(b: &[u8]) -> Result<Self, crate::Error> {
        if b.len() != LEN {
            return Err(dec(BadLength::<LEN> { received: b.len() }));
        }

        let mut l = [0; LEN];
        l.copy_from_slice(b);
        Ok(l)
    }
}

#[derive(Clone, Debug)]
pub struct BadLength<const EXPECTED: usize> {
    pub received: usize,
}

impl<const EXPECTED: usize> Display for BadLength<EXPECTED> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bad length: {EXPECTED} != {}", self.received)
    }
}

impl<const EXPECTED: usize> std::error::Error for BadLength<EXPECTED> {}

#[derive(Clone, Debug)]
pub struct TooLong<const MAXIMUM: usize> {
    pub received: usize,
}
impl<const MAXIMUM: usize> Display for TooLong<MAXIMUM> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Value too long: {} > {MAXIMUM}", self.received)
    }
}

impl<const MAXIMUM: usize> std::error::Error for TooLong<MAXIMUM> {}

#[derive(Clone, Debug)]
struct TupleBadLength {
    received: usize,
    a_len: usize,
    b_len: usize,
}

impl Display for TupleBadLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Bad length: {} != {} + {}",
            self.received, self.a_len, self.b_len
        )
    }
}

impl std::error::Error for TupleBadLength {}

#[macro_export]
macro_rules! table_integer {
    ($ty:ident => $real_ty:ident) => {
        impl $crate::orm::Encodable for $ty {
            type Encoded = [u8; $real_ty::BITS as usize / 8];

            fn encode(self) -> $crate::Result<Self::Encoded> {
                Ok(self.to_be_bytes())
            }
        }

        impl $crate::orm::Decodable for $ty {
            fn decode(b: &[u8]) -> $crate::Result<Self> {
                const EXPECTED: usize = $real_ty::BITS as usize / 8;

                match b.len() {
                    EXPECTED => Ok($real_ty::from_be_bytes(*$crate::arrayref::array_ref!(
                        &*b, 0, EXPECTED
                    ))
                    .into()),
                    other => Err($crate::Error::DecodeError(Box::new(
                        $crate::orm::BadLength::<EXPECTED> { received: other },
                    ))),
                }
            }
        }
    };
}

table_integer!(u32 => u32);
table_integer!(u64 => u64);
table_integer!(u128 => u128);

impl<T, const LEN: usize> Encodable for CutStart<T>
where
    T: Encodable<Encoded = [u8; LEN]>,
{
    type Encoded = ArrayVec<u8, LEN>;

    /// Encodes the value with trailing zero bytes removed. The result is a
    /// byte-wise prefix lower bound: it compares `<=` the full big-endian
    /// encoding, so `SET_RANGE`/`seek_closest` with it never skips the value.
    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        let arr = self.0.encode()?;

        let mut out = <Self::Encoded as Default>::default();
        let zeros = arr.iter().rev().take_while(|b| **b == 0).count();
        out.try_extend_from_slice(&arr[..LEN - zeros])
            .map_err(|e| crate::Error::EncodeError(Box::new(e)))?;
        Ok(out)
    }
}

impl<T, const LEN: usize> Decodable for CutStart<T>
where
    T: Encodable<Encoded = [u8; LEN]> + Decodable,
{
    fn decode(b: &[u8]) -> Result<Self, crate::Error> {
        if b.len() > LEN {
            return Err(dec(TooLong::<LEN> { received: b.len() }));
        }

        let mut array = [0; LEN];
        array[..b.len()].copy_from_slice(b);
        T::decode(&array).map(Self)
    }
}

/// Implements [`Encodable`](crate::orm::Encodable) and
/// [`Decodable`](crate::orm::Decodable) for a type using CBOR,
/// via `serde::Serialize` and `serde::Deserialize`.
///
/// Requires the `cbor` feature.
#[cfg(feature = "cbor")]
#[macro_export]
macro_rules! cbor_table_object {
    ($ty:ident) => {
        impl $crate::orm::Encodable for $ty {
            type Encoded = Vec<u8>;

            fn encode(self) -> $crate::Result<Self::Encoded> {
                let mut v = vec![];
                $crate::ciborium::ser::into_writer(&self, &mut v)
                    .map_err(|e| $crate::Error::EncodeError(Box::new(e)))?;
                Ok(v)
            }
        }

        impl $crate::orm::Decodable for $ty {
            fn decode(v: &[u8]) -> $crate::Result<Self> {
                $crate::ciborium::de::from_reader(v)
                    .map_err(|e| $crate::Error::DecodeError(Box::new(e)))
            }
        }
    };
}

impl<A, B, const A_LEN: usize, const B_LEN: usize> Encodable for (A, B)
where
    A: Encodable<Encoded = [u8; A_LEN]>,
    B: Encodable<Encoded = [u8; B_LEN]>,
{
    type Encoded = Vec<u8>;

    fn encode(self) -> Result<Self::Encoded, crate::Error> {
        let mut v = Vec::with_capacity(A_LEN + B_LEN);
        v.extend_from_slice(&self.0.encode()?);
        v.extend_from_slice(&self.1.encode()?);
        Ok(v)
    }
}

impl<A, B, const A_LEN: usize, const B_LEN: usize> Decodable for (A, B)
where
    A: TableObject<Encoded = [u8; A_LEN]>,
    B: TableObject<Encoded = [u8; B_LEN]>,
{
    fn decode(v: &[u8]) -> Result<Self, crate::Error> {
        if v.len() != A_LEN + B_LEN {
            return Err(dec(TupleBadLength {
                received: v.len(),
                a_len: A_LEN,
                b_len: B_LEN,
            }));
        }
        Ok((A::decode(&v[..A_LEN])?, B::decode(&v[A_LEN..])?))
    }
}
