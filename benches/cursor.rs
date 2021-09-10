mod utils;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ffi::*;
use mdbx::{Cursor, Result, TransactionKind};
use std::ptr;
use utils::*;

/// Benchmark of iterator sequential read performance.
fn bench_get_seq_iter(c: &mut Criterion) {
    let n = 100;
    let (_dir, env) = setup_bench_db(n);
    let txn = env.begin_ro_txn().unwrap();
    let db = txn.open_db(None).unwrap();

    c.bench_function("bench_get_seq_iter", |b| {
        b.iter(|| {
            let mut cursor = txn.cursor(&db).unwrap();
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

            fn iterate<K: TransactionKind>(cursor: &mut Cursor<'_, K>) -> Result<()> {
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
        })
    });
}

/// Benchmark of cursor sequential read performance.
fn bench_get_seq_cursor(c: &mut Criterion) {
    let n = 100;
    let (_dir, env) = setup_bench_db(n);
    let txn = env.begin_ro_txn().unwrap();
    let db = txn.open_db(None).unwrap();

    c.bench_function("bench_get_seq_cursor", |b| {
        b.iter(|| {
            let (i, count) = txn
                .cursor(&db)
                .unwrap()
                .into_iter()
                .map(Result::unwrap)
                .fold((0, 0), |(i, count), (key, val)| {
                    (i + key.len() + val.len(), count + 1)
                });

            black_box(i);
            assert_eq!(count, n);
        })
    });
}

/// Benchmark of raw MDBX sequential read performance (control).
fn bench_get_seq_raw(c: &mut Criterion) {
    let n = 100;
    let (_dir, env) = setup_bench_db(n);

    let dbi = env.begin_ro_txn().unwrap().open_db(None).unwrap().dbi();
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

    c.bench_function("bench_get_seq_raw", |b| {
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
        })
    });
}

criterion_group!(
    benches,
    bench_get_seq_iter,
    bench_get_seq_cursor,
    bench_get_seq_raw
);
criterion_main!(benches);
