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

/// Utilities for testing. Not public API.
#[doc(hidden)]
pub mod test_utils {
    use super::*;
    use std::path::Path;
    #[cfg(test)]
    use tempfile::tempdir;

    type Database = crate::Database<NoWriteMap>;

    pub const TEST_TABLE: &str = "test";

    pub fn test_db(path: impl AsRef<Path>) -> Database {
        test_db_with_options(path, |_| {}, |_| {})
    }

    pub fn test_db_with_options(
        path: impl AsRef<Path>,
        db_options: impl FnOnce(&mut DatabaseOptions),
        table_options: impl FnOnce(&mut TableFlags),
    ) -> Database {
        let mut options = DatabaseOptions::default();
        db_options(&mut options);
        if options.max_tables.unwrap_or(0) == 0 {
            options.max_tables = Some(1);
        }
        let db = Database::open_with_options(path, options).unwrap();
        let txn = db.begin_rw_txn().unwrap();
        let mut table_flags = TableFlags::empty();
        table_options(&mut table_flags);
        txn.create_table(TEST_TABLE, table_flags).unwrap();
        txn.commit().unwrap();
        db
    }

    /// Regression test for https://github.com/danburkert/lmdb-rs/issues/21.
    /// This test reliably segfaults when run against lmbdb compiled with opt level -O3 and newer
    /// GCC compilers.
    #[test]
    fn issue_21_regression() {
        const HEIGHT_KEY: [u8; 1] = [0];

        let dir = tempdir().unwrap();

        let db = Database::open_with_options(
            &dir,
            DatabaseOptions {
                max_tables: Some(2),
                ..Default::default()
            },
        )
        .unwrap();

        for height in 0..1000_u64 {
            let value = height.to_le_bytes();
            let tx = db.begin_rw_txn().unwrap();
            let index = tx.create_table(TEST_TABLE, TableFlags::DUP_SORT).unwrap();
            tx.put(&index, HEIGHT_KEY, value, WriteFlags::empty())
                .unwrap();
            tx.commit().unwrap();
        }
    }
}
