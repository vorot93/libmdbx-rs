use derive_more::*;

#[derive(Debug, Display)]
pub enum DecodeError<E>
where
    E: std::error::Error,
{
    Internal(crate::Error),
    Decode(E),
}
