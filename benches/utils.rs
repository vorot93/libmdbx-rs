use libmdbx::{Database, DatabaseOptions, NoWriteMap, TableFlags, WriteFlags};
use tempfile::{TempDir, tempdir};

pub fn get_key(n: u32) -> String {
    format!("key{n}")
}

pub fn get_data(n: u32) -> String {
    format!("data{n}")
}

pub const BENCH_TABLE: &str = "bench";

pub fn setup_bench_db(num_rows: u32) -> (TempDir, Database<NoWriteMap>) {
    let dir = tempdir().unwrap();
    let db = Database::open_with_options(
        &dir,
        DatabaseOptions {
            max_tables: Some(1),
            ..Default::default()
        },
    )
    .unwrap();

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.create_table(BENCH_TABLE, TableFlags::empty()).unwrap();
        for i in 0..num_rows {
            txn.put(&table, get_key(i), get_data(i), WriteFlags::empty())
                .unwrap();
        }
        txn.commit().unwrap();
    }
    (dir, db)
}
