use libmdbx::*;
use std::path::Path;

pub type Database = libmdbx::Database<NoWriteMap>;

pub const TEST_TABLE: &str = "test";

pub fn test_db(path: impl AsRef<Path>) -> Database {
    test_db_with_options(path, |_| {}, TableFlags::empty())
}

pub fn test_db_with_table_options(path: impl AsRef<Path>, table_options: TableFlags) -> Database {
    test_db_with_options(path, |_| {}, table_options)
}

pub fn test_db_with_db_options(
    path: impl AsRef<Path>,
    db_options: impl FnOnce(&mut DatabaseOptions),
) -> Database {
    test_db_with_options(path, db_options, TableFlags::empty())
}

pub fn test_db_with_options(
    path: impl AsRef<Path>,
    db_options: impl FnOnce(&mut DatabaseOptions),
    table_options: TableFlags,
) -> Database {
    let mut options = DatabaseOptions {
        max_tables: Some(1),
        ..DatabaseOptions::default()
    };
    db_options(&mut options);
    let db = Database::open_with_options(path, options).unwrap();
    let txn = db.begin_rw_txn().unwrap();
    txn.create_table(TEST_TABLE, table_options).unwrap();
    txn.commit().unwrap();
    db
}
