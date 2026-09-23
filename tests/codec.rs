//! Round trips through the optional byte-container codecs.

#![cfg(any(feature = "bytes", feature = "lifetimed-bytes"))]

use libmdbx::*;
use tempfile::tempdir;

type Database = libmdbx::Database<NoWriteMap>;

fn db_with_value(dir: &tempfile::TempDir) -> Database {
    let db = Database::open(dir).unwrap();
    let txn = db.begin_rw_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    txn.put(&table, b"key", b"value", WriteFlags::empty())
        .unwrap();
    txn.commit().unwrap();
    db
}

#[cfg(feature = "bytes")]
#[test]
fn test_bytes_roundtrip() {
    let dir = tempdir().unwrap();
    let db = db_with_value(&dir);
    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();

    let value: bytes::Bytes = txn.get(&table, b"key").unwrap().unwrap();
    assert_eq!(value, &b"value"[..]);
}

#[cfg(feature = "lifetimed-bytes")]
#[test]
fn test_lifetimed_bytes_roundtrip() {
    let dir = tempdir().unwrap();
    let db = db_with_value(&dir);
    for rw in [false, true] {
        let value = if rw {
            let txn = db.begin_rw_txn().unwrap();
            let table = txn.open_table(None).unwrap();
            txn.get::<lifetimed_bytes::Bytes>(&table, b"key")
                .unwrap()
                .unwrap()
                .to_vec()
        } else {
            let txn = db.begin_ro_txn().unwrap();
            let table = txn.open_table(None).unwrap();
            txn.get::<lifetimed_bytes::Bytes>(&table, b"key")
                .unwrap()
                .unwrap()
                .to_vec()
        };
        assert_eq!(value, b"value");
    }
}

#[cfg(all(feature = "bytes", feature = "orm"))]
#[test]
fn test_orm_bytes_roundtrip() {
    use libmdbx::orm::{DatabaseChart, table, table_info};

    table!((Blobs) bytes::Bytes => bytes::Bytes);
    let chart: DatabaseChart = [table_info!(Blobs)].into_iter().collect();
    let db: orm::Database = orm::Database::create(None, &chart).unwrap();

    let tx = db.begin_readwrite().unwrap();
    tx.upsert::<Blobs>("k".into(), "v".into()).unwrap();
    assert_eq!(
        tx.get::<Blobs>("k".into()).unwrap(),
        Some(bytes::Bytes::from("v"))
    );
}
