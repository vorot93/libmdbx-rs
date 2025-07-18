mod utils;

use libmdbx::*;
use tempfile::tempdir;
use utils::{Database, *};

#[test]
fn test_open() {
    let dir = tempdir().unwrap();

    // opening non-existent database with read-only should fail
    assert!(
        Database::open_with_options(
            &dir,
            DatabaseOptions {
                mode: Mode::ReadOnly,
                ..Default::default()
            }
        )
        .is_err()
    );

    // opening non-existent database should succeed
    assert!(Database::open(&dir).is_ok());

    // opening database with read-only should succeed
    assert!(
        Database::open_with_options(
            &dir,
            DatabaseOptions {
                mode: Mode::ReadOnly,
                ..Default::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn test_begin_txn() {
    let dir = tempdir().unwrap();

    {
        // writable database
        let db = Database::open(&dir).unwrap();

        assert!(db.begin_rw_txn().is_ok());
        assert!(db.begin_ro_txn().is_ok());
    }

    {
        // read-only database
        let db = Database::open_with_options(
            &dir,
            DatabaseOptions {
                mode: Mode::ReadOnly,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(db.begin_rw_txn().is_err());
        assert!(db.begin_ro_txn().is_ok());
    }
}

#[test]
fn test_open_table() {
    let dir = tempdir().unwrap();
    let db = test_db(&dir);

    let txn = db.begin_ro_txn().unwrap();
    assert!(txn.open_table(TEST_TABLE).is_ok());
    assert!(
        txn.open_table(&TEST_TABLE.chars().chain("_abc".chars()).collect::<String>())
            .is_err()
    );
}

#[test]
fn test_create_table() {
    let dir = tempdir().unwrap();
    let db = Database::open_with_options(
        &dir,
        DatabaseOptions {
            max_tables: Some(1),
            ..Default::default()
        },
    )
    .unwrap();

    let txn = db.begin_rw_txn().unwrap();
    assert!(txn.open_table(TEST_TABLE).is_err());
    assert!(txn.create_table(TEST_TABLE, TableFlags::empty()).is_ok());
    assert!(txn.open_table(TEST_TABLE).is_ok())
}

#[test]
fn test_close_table() {
    let dir = tempdir().unwrap();
    let db = Database::open_with_options(
        &dir,
        DatabaseOptions {
            max_tables: Some(1),
            ..Default::default()
        },
    )
    .unwrap();

    let txn = db.begin_rw_txn().unwrap();
    txn.create_table(TEST_TABLE, TableFlags::empty()).unwrap();
    txn.open_table(TEST_TABLE).unwrap();
}

#[test]
fn test_sync() {
    let dir = tempdir().unwrap();
    {
        let db = Database::open(&dir).unwrap();
        db.sync(true).unwrap();
    }
    {
        let db = Database::open_with_options(
            &dir,
            DatabaseOptions {
                mode: Mode::ReadOnly,
                ..Default::default()
            },
        )
        .unwrap();
        db.sync(true).unwrap_err();
    }
}

#[test]
fn test_stat() {
    let dir = tempdir().unwrap();
    let db = test_db(&dir);

    // Stats should be empty initially.
    let stat = db.stat().unwrap();
    assert_eq!(stat.depth(), 1);
    assert_eq!(stat.branch_pages(), 0);
    assert_eq!(stat.leaf_pages(), 1);
    assert_eq!(stat.overflow_pages(), 0);
    assert_eq!(stat.entries(), 1);

    // Write a few small values.
    for i in 0..64_u64 {
        let value = i.to_le_bytes();
        let tx = db.begin_rw_txn().unwrap();
        tx.put(
            &tx.open_table(TEST_TABLE).unwrap(),
            value,
            value,
            WriteFlags::default(),
        )
        .unwrap();
        tx.commit().unwrap();
    }

    // Stats should now reflect inserted values.
    let stat = db.stat().unwrap();
    assert_eq!(stat.depth(), 2);
    assert_eq!(stat.branch_pages(), 0);
    assert_eq!(stat.leaf_pages(), 2);
    assert_eq!(stat.overflow_pages(), 0);
    assert_eq!(stat.entries(), 65);
}

#[test]
fn test_info() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let info = db.info().unwrap();
    // assert_eq!(info.geometry().min(), map_size as u64);
    // assert_eq!(info.last_pgno(), 1);
    // assert_eq!(info.last_txnid(), 0);
    assert_eq!(info.num_readers(), 0);
}

#[test]
fn test_freelist() {
    let dir = tempdir().unwrap();
    let db = test_db(&dir);

    let mut freelist = db.freelist().unwrap();
    assert_eq!(freelist, 0);

    // Write a few small values.
    for i in 0..64_u64 {
        let value = i.to_le_bytes();
        let tx = db.begin_rw_txn().unwrap();
        tx.put(
            &tx.open_table(TEST_TABLE).unwrap(),
            value,
            value,
            WriteFlags::default(),
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let tx = db.begin_rw_txn().unwrap();
    tx.clear_table(&tx.open_table(TEST_TABLE).unwrap()).unwrap();
    tx.commit().unwrap();

    // Freelist should not be empty after clear_table.
    freelist = db.freelist().unwrap();
    assert!(freelist > 0);
}
