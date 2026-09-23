use std::fmt::Debug;

/// Converts a value into the bytes stored in the database.
pub trait Encodable: Send + Sync + Sized {
    /// The encoded bytes.
    type Encoded: AsRef<[u8]> + Send + Sync;

    /// Encodes the value.
    fn encode(self) -> Result<Self::Encoded, crate::Error>;
}

/// Converts stored bytes back into a value.
pub trait Decodable: Send + Sync + Sized {
    /// Decodes a value.
    fn decode(b: &[u8]) -> Result<Self, crate::Error>;
}

/// A type that can be both stored and loaded.
pub trait TableObject: Encodable + Decodable {}

impl<T> TableObject for T where T: Encodable + Decodable {}

/// A typed table; declare one with [table!](crate::table).
pub trait Table: Send + Sync + Debug + 'static {
    /// The table's name in the database.
    const NAME: &'static str;

    /// The key type.
    type Key: Encodable;
    /// The value type.
    type Value: TableObject;
    /// The type used to seek to the closest key.
    type SeekKey: Encodable;
}
/// A `DUP_SORT` table; declare one with [dupsort!](crate::dupsort).
pub trait DupSort: Table {
    /// The type used to seek to the closest duplicate.
    type SeekValue: Encodable;
}
