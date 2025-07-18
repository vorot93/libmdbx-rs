mod utils;

use criterion::{Criterion, criterion_group, criterion_main};
use ffi::*;
use libc::size_t;
use libmdbx::{ObjectLength, WriteFlags};
use rand::{SeedableRng, prelude::SliceRandom};
use rand_xorshift::XorShiftRng;
use std::{hint::black_box, ptr};
use utils::*;

fn bench_get_rand(c: &mut Criterion) {
    let n = 100u32;
    let (_dir, db) = setup_bench_db(n);
    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(BENCH_TABLE).unwrap();

    let mut keys: Vec<String> = (0..n).map(get_key).collect();
    keys.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    c.bench_function("bench_get_rand", |b| {
        b.iter(|| {
            let mut i = 0usize;
            for key in &keys {
                i += *txn
                    .get::<ObjectLength>(&table, key.as_bytes())
                    .unwrap()
                    .unwrap();
            }
            black_box(i);
        })
    });
}

fn bench_get_rand_raw(c: &mut Criterion) {
    let n = 100u32;
    let (_dir, db) = setup_bench_db(n);
    let _txn = db.begin_ro_txn().unwrap();
    let table = _txn.open_table(BENCH_TABLE).unwrap();

    let mut keys: Vec<String> = (0..n).map(get_key).collect();
    keys.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    let dbi = table.dbi();
    let txn = _txn.txn();

    let mut key_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    let mut data_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };

    c.bench_function("bench_get_rand_raw", |b| {
        b.iter(|| unsafe {
            let mut i: size_t = 0;
            for key in &keys {
                key_val.iov_len = key.len() as size_t;
                key_val.iov_base = key.as_bytes().as_ptr() as *mut _;

                mdbx_get(txn.0, dbi, &key_val, &mut data_val);

                i += key_val.iov_len;
            }
            black_box(i);
        })
    });
}

fn bench_put_rand(c: &mut Criterion) {
    let n = 100u32;
    let (_dir, db) = setup_bench_db(0);

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(BENCH_TABLE).unwrap();
    txn.prime_for_permaopen(table);
    let table = txn.commit_and_rebind_open_dbs().unwrap().1.remove(0);

    let mut items: Vec<(String, String)> = (0..n).map(|n| (get_key(n), get_data(n))).collect();
    items.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    c.bench_function("bench_put_rand", |b| {
        b.iter(|| {
            let txn = db.begin_rw_txn().unwrap();
            for (key, data) in items.iter() {
                txn.put(&table, key, data, WriteFlags::empty()).unwrap();
            }
        })
    });
}

fn bench_put_rand_raw(c: &mut Criterion) {
    let n = 100u32;
    let (_dir, _db) = setup_bench_db(0);

    let mut items: Vec<(String, String)> = (0..n).map(|n| (get_key(n), get_data(n))).collect();
    items.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    let dbi = _db
        .begin_ro_txn()
        .unwrap()
        .open_table(BENCH_TABLE)
        .unwrap()
        .dbi();
    let env = _db.ptr();

    let mut key_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    let mut data_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };

    c.bench_function("bench_put_rand_raw", |b| {
        b.iter(|| unsafe {
            let mut txn: *mut MDBX_txn = ptr::null_mut();
            mdbx_txn_begin_ex(env.0, ptr::null_mut(), 0, &mut txn, ptr::null_mut());

            let mut i: ::libc::c_int = 0;
            for (key, data) in items.iter() {
                key_val.iov_len = key.len() as size_t;
                key_val.iov_base = key.as_bytes().as_ptr() as *mut _;
                data_val.iov_len = data.len() as size_t;
                data_val.iov_base = data.as_bytes().as_ptr() as *mut _;

                i += mdbx_put(txn, dbi, &key_val, &mut data_val, 0);
            }
            assert_eq!(0, i);
            mdbx_txn_abort(txn);
        })
    });
}

criterion_group!(
    benches,
    bench_get_rand,
    bench_get_rand_raw,
    bench_put_rand,
    bench_put_rand_raw
);
criterion_main!(benches);
