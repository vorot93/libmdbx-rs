#![allow(clippy::type_complexity, clippy::unnecessary_cast)]
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use crate::{
    codec::*,
    cursor::{Cursor, IntoIter, Iter, IterDup},
    database::{
        Database, DatabaseKind, DatabaseOptions, Info, NoWriteMap, PageSize, Stat, WriteMap,
    },
    error::{Error, Result},
    flags::*,
    table::Table,
    transaction::{RO, RW, Transaction, TransactionKind},
};

mod codec;
mod cursor;
mod database;
mod error;
mod flags;
mod table;
mod transaction;

/// Fully typed ORM for use with libmdbx.
#[cfg(feature = "orm")]
#[cfg_attr(docsrs, doc(cfg(feature = "orm")))]
pub mod orm;

#[cfg(feature = "orm")]
mod orm_uses {
    #[doc(hidden)]
    pub use arrayref;

    #[doc(hidden)]
    pub use impls;

    #[cfg(feature = "cbor")]
    #[doc(hidden)]
    pub use ciborium;
}

#[cfg(feature = "orm")]
pub use orm_uses::*;
