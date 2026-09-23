#![cfg(feature = "orm")]

use libmdbx::orm::{
    CutStart, DatabaseChart, DatabaseOptions, Decodable, Encodable, table, table_info,
};
use std::sync::Arc;

table! { ( Numbers ) u64 [ CutStart<u64> ] => u64 }

// Table NOT part of the chart used by most tests.
table! { ( Extra ) String => Vec<u8> }

fn chart() -> Arc<DatabaseChart> {
    Arc::new([table_info!(Numbers)].into_iter().collect())
}

#[test]
fn test_orm_upsert_get_delete() {
    let db: libmdbx::orm::Database = libmdbx::orm::Database::create(None, &chart()).unwrap();
    let tx = db.begin_readwrite().unwrap();

    tx.upsert::<Numbers>(1, 1).unwrap();
    tx.upsert::<Numbers>(2, 4).unwrap();
    assert_eq!(tx.get::<Numbers>(2).unwrap(), Some(4));

    let mut got = vec![];
    for kv in tx.cursor::<Numbers>().unwrap().walk(None) {
        got.push(kv.unwrap());
    }
    assert_eq!(got, vec![(1, 1), (2, 4)]);

    assert!(tx.delete::<Numbers>(1, None).unwrap());
    assert_eq!(tx.get::<Numbers>(1).unwrap(), None);
    tx.commit().unwrap();

    let tx = db.begin_read().unwrap();
    assert_eq!(tx.get::<Numbers>(2).unwrap(), Some(4));
    assert_eq!(tx.get::<Numbers>(3).unwrap(), None);
}

#[test]
fn test_user_max_tables_respected() {
    let options = DatabaseOptions {
        max_tables: Some(5),
        ..Default::default()
    };
    let chart = Arc::new([table_info!(Numbers)].into_iter().collect());
    let db: libmdbx::orm::Database =
        libmdbx::orm::Database::create_with_options(None, options, &chart).unwrap();

    // `Extra` is not in the chart; create it via the core API (Deref) and
    // use it through the ORM. Requires the user-provided max_tables = 5 to
    // survive: the chart alone only reserves one slot.
    let tx = db.begin_rw_txn().unwrap();
    tx.create_table(Some("Extra"), libmdbx::TableFlags::default())
        .unwrap();
    tx.commit().unwrap();

    let tx = db.begin_readwrite().unwrap();
    let mut cur = tx.cursor::<Extra>().unwrap();
    cur.upsert("k".to_string(), b"v".to_vec()).unwrap();
    drop(cur);
    tx.commit().unwrap();

    let tx = db.begin_read().unwrap();
    assert_eq!(
        tx.get::<Extra>("k".to_string()).unwrap(),
        Some(b"v".to_vec())
    );
}

#[test]
fn test_database_mode_generic() {
    let db: libmdbx::orm::Database<libmdbx::NoWriteMap> =
        libmdbx::orm::Database::create(None, &chart()).unwrap();
    let tx = db.begin_readwrite().unwrap();
    tx.upsert::<Numbers>(1, 1).unwrap();
    tx.commit().unwrap();

    let tx = db.begin_read().unwrap();
    assert_eq!(tx.get::<Numbers>(1).unwrap(), Some(1));
}

table! { ( Small ) u8 => u16 }

#[test]
fn test_u8_u16_keys() {
    let chart = Arc::new([table_info!(Small)].into_iter().collect());
    let db: libmdbx::orm::Database = libmdbx::orm::Database::create(None, &chart).unwrap();
    let tx = db.begin_readwrite().unwrap();
    tx.upsert::<Small>(2, 700).unwrap();
    tx.upsert::<Small>(0, 1).unwrap();
    tx.upsert::<Small>(255, u16::MAX).unwrap();
    tx.commit().unwrap();

    let tx = db.begin_read().unwrap();
    let items: Vec<_> = tx
        .cursor::<Small>()
        .unwrap()
        .walk_back(None)
        .map(|kv| kv.unwrap())
        .collect();
    assert_eq!(items, vec![(255, u16::MAX), (2, 700), (0, 1)]);
}

#[derive(Debug)]
pub struct FailingDecode;

impl Encodable for FailingDecode {
    type Encoded = Vec<u8>;

    fn encode(self) -> Result<Self::Encoded, libmdbx::Error> {
        Ok(vec![])
    }
}

impl Decodable for FailingDecode {
    fn decode(_: &[u8]) -> Result<Self, libmdbx::Error> {
        Err(libmdbx::Error::DecodeError("bad".into()))
    }
}

table! { ( FailingTable ) u64 => FailingDecode }

#[test]
fn test_orm_error_type_is_typed() {
    let db: libmdbx::orm::Database = libmdbx::orm::Database::create(
        None,
        &Arc::new([table_info!(FailingTable)].into_iter().collect()),
    )
    .unwrap();

    let tx = db.begin_readwrite().unwrap();
    tx.upsert::<FailingTable>(1, FailingDecode).unwrap();

    assert!(matches!(
        tx.get::<FailingTable>(1).unwrap_err(),
        libmdbx::Error::DecodeError(_)
    ));
}

#[test]
fn test_cutstart_encode_cuts_trailing_zeros() {
    let enc = CutStart(0x0100u64).encode().unwrap();
    assert_eq!(enc.as_ref(), &[0, 0, 0, 0, 0, 0, 1][..]);
    let enc = CutStart(5u64).encode().unwrap();
    assert_eq!(enc.as_ref(), &[0, 0, 0, 0, 0, 0, 0, 5][..]);
    let enc = CutStart(0u64).encode().unwrap();
    assert_eq!(enc.as_ref(), &[][..]);
}

#[test]
fn test_cutstart_roundtrip() {
    for v in [0u64, 1, 5, 0x100, u64::MAX - 1] {
        let enc = CutStart(v).encode().unwrap();
        let back = <CutStart<u64> as Decodable>::decode(enc.as_ref()).unwrap();
        assert_eq!(back.0, v);
    }
}

#[test]
fn test_cutstart_seek_finds_value() {
    let db: libmdbx::orm::Database = libmdbx::orm::Database::create(None, &chart()).unwrap();
    let tx = db.begin_readwrite().unwrap();
    for i in 1..=1000u64 {
        tx.upsert::<Numbers>(i, i).unwrap();
    }
    tx.commit().unwrap();

    let tx = db.begin_read().unwrap();
    let mut cursor = tx.cursor::<Numbers>().unwrap();
    let (k, _) = cursor
        .seek_closest(CutStart(5u64))
        .unwrap()
        .expect("5 must be found");
    assert_eq!(k, 5);

    let first = tx
        .cursor::<Numbers>()
        .unwrap()
        .walk(Some(CutStart(7u64)))
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(first, (7, 7));
}

#[test]
fn test_walk_back_bounds() {
    let db: libmdbx::orm::Database = libmdbx::orm::Database::create(None, &chart()).unwrap();
    let tx = db.begin_readwrite().unwrap();
    for i in 1..=10u64 {
        tx.upsert::<Numbers>(i, i).unwrap();
    }
    tx.commit().unwrap();

    let tx = db.begin_read().unwrap();
    // present upper bound: inclusive, descending
    let items: Vec<_> = tx
        .cursor::<Numbers>()
        .unwrap()
        .walk_back(Some(CutStart(5u64)))
        .map(|kv| kv.unwrap())
        .collect();
    assert_eq!(items, vec![(5, 5), (4, 4), (3, 3), (2, 2), (1, 1)]);

    // absent upper bound: keys > bound must NOT appear. No u64 exists
    // between 6 and 7, so delete key 6 and seek 6 instead.
    let tx = db.begin_readwrite().unwrap();
    tx.delete::<Numbers>(6, None).unwrap();
    tx.commit().unwrap();
    let tx = db.begin_read().unwrap();
    let items: Vec<_> = tx
        .cursor::<Numbers>()
        .unwrap()
        .walk_back(Some(CutStart(6u64)))
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(items, vec![5, 4, 3, 2, 1]); // 6 absent, 7..10 excluded

    // None: whole table descending
    let items: Vec<_> = tx
        .cursor::<Numbers>()
        .unwrap()
        .walk_back(None)
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(items, vec![10, 9, 8, 7, 5, 4, 3, 2, 1]);
}

#[cfg(feature = "cbor")]
mod cbor {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct UserInfo {
        age: u8,
        first_name: String,
    }

    libmdbx::cbor_table_object!(UserInfo);

    table! { ( Users ) String => UserInfo }

    #[test]
    fn test_cbor_roundtrip() {
        let db: libmdbx::orm::Database = libmdbx::orm::Database::create(
            None,
            &Arc::new([table_info!(Users)].into_iter().collect()),
        )
        .unwrap();

        let tx = db.begin_readwrite().unwrap();
        let user = UserInfo {
            age: 42,
            first_name: "Leet".to_string(),
        };
        tx.upsert::<Users>("l33tc0der".to_string(), user.clone())
            .unwrap();
        assert_eq!(
            tx.get::<Users>("l33tc0der".to_string()).unwrap(),
            Some(user)
        );
        tx.commit().unwrap();
    }
}

#[test]
fn test_database_defaults_to_writemap() {
    fn expect_writemap(_: &libmdbx::orm::Database<libmdbx::WriteMap>) {}

    let db: libmdbx::orm::Database = libmdbx::orm::Database::create(None, &chart()).unwrap();
    expect_writemap(&db);
    let tx = db.begin_readwrite().unwrap();
    tx.upsert::<Numbers>(1, 1).unwrap();
    tx.commit().unwrap();
    let tx = db.begin_read().unwrap();
    assert_eq!(tx.get::<Numbers>(1).unwrap(), Some(1));
}

/// `table_sizes` must not close table handles: they are shared by every
/// transaction in the environment, so closing one invalidates handles other
/// transactions (possibly on other threads) are using.
#[test]
fn test_table_sizes_keeps_shared_handles_open() {
    let db: libmdbx::orm::Database = libmdbx::orm::Database::create(None, &chart()).unwrap();
    let tx = db.begin_readwrite().unwrap();
    tx.upsert::<Numbers>(1, 1).unwrap();
    tx.commit().unwrap();

    let raw = db.begin_ro_txn().unwrap();
    let table = raw.open_table(Some("Numbers")).unwrap();

    let sizes = db.begin_read().unwrap().table_sizes().unwrap();
    assert!(sizes.contains_key("Numbers"));

    assert_eq!(
        raw.get::<Vec<u8>>(&table, 1u64.to_be_bytes()).unwrap(),
        Some(1u64.to_be_bytes().to_vec())
    );
}
