#![cfg(feature = "orm")]

use libmdbx::orm::{DatabaseChart, Decodable, Encodable, table, table_info};
use std::sync::Arc;

table! { ( Numbers ) u64 => u64 }

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
