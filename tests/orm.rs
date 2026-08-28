#![cfg(feature = "orm")]

use libmdbx::orm::{CutStart, DatabaseChart, Decodable, Encodable, table, table_info};
use std::sync::Arc;

table! { ( Numbers ) u64 [ CutStart<u64> ] => u64 }

fn chart() -> Arc<DatabaseChart> {
    Arc::new([table_info!(Numbers)].into_iter().collect())
}

#[test]
fn test_orm_upsert_get_delete() {
    let db = libmdbx::orm::Database::create(None, &chart()).unwrap();
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
    let db = libmdbx::orm::Database::create(
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
    let db = libmdbx::orm::Database::create(None, &chart()).unwrap();
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
        let db = libmdbx::orm::Database::create(
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
