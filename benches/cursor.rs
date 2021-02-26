#![feature(test)]
extern crate test;

mod utils;

use ffi::*;
use mdbx::{
    Cursor,
    Result,
    RoCursor,
    Transaction,
};
use std::ptr;
use test::{
    black_box,
    Bencher,
};
use utils::*;

/// Benchmark of iterator sequential read performance.
#[bench]
fn bench_get_seq_iter(b: &mut Bencher) {
    let n = 100;
    let (_dir, env) = setup_bench_db(n);
    let db = env.open_db(None).unwrap();
    let txn = env.begin_ro_txn().unwrap();

    b.iter(|| {
        let mut cursor = txn.open_ro_cursor(&db).unwrap();
        let mut i = 0;
        let mut count = 0u32;

        for (key, data) in cursor.iter().map(Result::unwrap) {
            i = i + key.len() + data.len();
            count += 1;
        }
        for (key, data) in cursor.iter().filter_map(Result::ok) {
            i = i + key.len() + data.len();
            count += 1;
        }

        fn iterate(cursor: &mut RoCursor) -> Result<()> {
            let mut i = 0;
            for result in cursor.iter() {
                let (key, data) = result?;
                i = i + key.len() + data.len();
            }
            Ok(())
        }
        iterate(&mut cursor).unwrap();

        black_box(i);
        assert_eq!(count, n);
    });
}

/// Benchmark of cursor sequential read performance.
#[bench]
fn bench_get_seq_cursor(b: &mut Bencher) {
    let n = 100;
    let (_dir, env) = setup_bench_db(n);
    let db = env.open_db(None).unwrap();
    let txn = env.begin_ro_txn().unwrap();

    b.iter(|| {
        let cursor = txn.open_ro_cursor(&db).unwrap();
        let mut i = 0;
        let mut count = 0u32;

        while let Ok((key_opt, val)) = cursor.get(None, None, MDBX_NEXT) {
            i += key_opt.map(|key| key.len()).unwrap_or(0) + val.len();
            count += 1;
        }

        black_box(i);
        assert_eq!(count, n);
    });
}

/// Benchmark of raw LMDB sequential read performance (control).
#[bench]
fn bench_get_seq_raw(b: &mut Bencher) {
    let n = 100;
    let (_dir, env) = setup_bench_db(n);
    let db = env.open_db(None).unwrap();

    let dbi: MDBX_dbi = db.dbi();
    let _txn = env.begin_ro_txn().unwrap();
    let txn = _txn.txn();

    let mut key = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    let mut data = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    let mut cursor: *mut MDBX_cursor = ptr::null_mut();

    b.iter(|| unsafe {
        mdbx_cursor_open(txn, dbi, &mut cursor);
        let mut i = 0;
        let mut count = 0u32;

        while mdbx_cursor_get(cursor, &mut key, &mut data, MDBX_NEXT) == 0 {
            i += key.iov_len + data.iov_len;
            count += 1;
        }

        black_box(i);
        assert_eq!(count, n);
        mdbx_cursor_close(cursor);
    });
}
