use std::fmt::Debug;

pub trait Encodable: Send + Sync + Sized {
    type Encoded: AsRef<[u8]> + Send + Sync;

    fn encode(self) -> Result<Self::Encoded, crate::Error>;
}

pub trait Decodable: Send + Sync + Sized {
    fn decode(b: &[u8]) -> Result<Self, crate::Error>;
}

pub trait TableObject: Encodable + Decodable {}

impl<T> TableObject for T where T: Encodable + Decodable {}

pub trait Table: Send + Sync + Debug + 'static {
    const NAME: &'static str;

    type Key: Encodable;
    type Value: TableObject;
    type SeekKey: Encodable;
}
pub trait DupSort: Table {
    type SeekValue: Encodable;
}
