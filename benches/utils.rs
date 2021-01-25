use mdbx::{
    Environment,
    Transaction,
    WriteFlags,
};
use tempdir::TempDir;

pub fn get_key(n: u32) -> String {
    format!("key{}", n)
}

pub fn get_data(n: u32) -> String {
    format!("data{}", n)
}

pub fn setup_bench_db(num_rows: u32) -> (TempDir, Environment) {
    let dir = TempDir::new("test").unwrap();
    let env = Environment::new().open(dir.path()).unwrap();

    {
        let db = env.open_db(None).unwrap();
        let mut txn = env.begin_rw_txn().unwrap();
        for i in 0..num_rows {
            txn.put(db, &get_key(i), &get_data(i), WriteFlags::empty()).unwrap();
        }
        txn.commit().unwrap();
    }
    (dir, env)
}
