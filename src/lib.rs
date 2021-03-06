//! Idiomatic and safe APIs for interacting with the
//! [Lightning Memory-mapped Database (LMDB)](https://symas.com/lmdb).

#![doc(html_root_url = "https://docs.rs/mdbx/0.1.0")]

pub use crate::{
    cursor::{
        Cursor,
        Iter,
        IterDup,
        RoCursor,
        RwCursor,
    },
    database::Database,
    environment::{
        Environment,
        EnvironmentBuilder,
        Geometry,
        Info,
        Stat,
    },
    error::{
        Error,
        Result,
    },
    flags::*,
    transaction::{
        RoTransaction,
        RwTransaction,
        Transaction,
    },
};

macro_rules! lmdb_try {
    ($expr:expr) => {{
        match $expr {
            ::ffi::MDBX_SUCCESS => false,
            ::ffi::MDBX_RESULT_TRUE => true,
            err_code => return Err(crate::Error::from_err_code(err_code)),
        }
    }};
}

mod cursor;
mod database;
mod environment;
mod error;
mod flags;
mod transaction;
mod util;

#[cfg(test)]
mod test_utils {

    use byteorder::{
        ByteOrder,
        LittleEndian,
    };
    use tempfile::tempdir;

    use super::*;

    /// Regression test for https://github.com/danburkert/lmdb-rs/issues/21.
    /// This test reliably segfaults when run against lmbdb compiled with opt level -O3 and newer
    /// GCC compilers.
    #[test]
    fn issue_21_regression() {
        const HEIGHT_KEY: [u8; 1] = [0];

        let dir = tempdir().unwrap();

        let env = {
            let mut builder = Environment::new();
            builder.set_max_dbs(2);
            builder.set_geometry(Geometry {
                size: Some(1_000_000..1_000_000),
                ..Default::default()
            });
            builder.open(dir.path()).expect("open lmdb env")
        };

        for height in 0..1000 {
            let mut value = [0u8; 8];
            LittleEndian::write_u64(&mut value, height);
            let tx = env.begin_rw_txn().expect("begin_rw_txn");
            let index = tx.create_db(None, DatabaseFlags::DUP_SORT).expect("open index db");
            index.put(&HEIGHT_KEY, &value, WriteFlags::empty()).expect("tx.put");
            tx.commit().expect("tx.commit");
        }
    }
}
