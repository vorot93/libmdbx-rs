#![feature(test)]
extern crate test;

mod utils;

use ffi::*;
use libc::size_t;
use mdbx::{
    Transaction,
    WriteFlags,
};
use rand::{
    prelude::SliceRandom,
    SeedableRng,
};
use rand_xorshift::XorShiftRng;
use std::ptr;
use test::{
    black_box,
    Bencher,
};
use utils::*;

#[bench]
fn bench_get_rand(b: &mut Bencher) {
    let n = 100u32;
    let (_dir, env) = setup_bench_db(n);
    let db = env.open_db(None).unwrap();
    let txn = env.begin_ro_txn().unwrap();

    let mut keys: Vec<String> = (0..n).map(get_key).collect();
    keys.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    b.iter(|| {
        let mut i = 0usize;
        for key in &keys {
            i += txn.get(db, key).unwrap().len();
        }
        black_box(i);
    });
}

#[bench]
fn bench_get_rand_raw(b: &mut Bencher) {
    let n = 100u32;
    let (_dir, env) = setup_bench_db(n);
    let db = env.open_db(None).unwrap();
    let _txn = env.begin_ro_txn().unwrap();

    let mut keys: Vec<String> = (0..n).map(get_key).collect();
    keys.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    let dbi = db.dbi();
    let txn = _txn.txn();

    let mut key_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    let mut data_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };

    b.iter(|| unsafe {
        let mut i: size_t = 0;
        for key in &keys {
            key_val.iov_len = key.len() as size_t;
            key_val.iov_base = key.as_bytes().as_ptr() as *mut _;

            mdbx_get(txn, dbi, &key_val, &mut data_val);

            i += key_val.iov_len;
        }
        black_box(i);
    });
}

#[bench]
fn bench_put_rand(b: &mut Bencher) {
    let n = 100u32;
    let (_dir, env) = setup_bench_db(0);
    let db = env.open_db(None).unwrap();

    let mut items: Vec<(String, String)> = (0..n).map(|n| (get_key(n), get_data(n))).collect();
    items.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    b.iter(|| {
        let mut txn = env.begin_rw_txn().unwrap();
        for &(ref key, ref data) in items.iter() {
            txn.put(db, key, data, WriteFlags::empty()).unwrap();
        }
    });
}

#[bench]
fn bench_put_rand_raw(b: &mut Bencher) {
    let n = 100u32;
    let (_dir, _env) = setup_bench_db(0);
    let db = _env.open_db(None).unwrap();

    let mut items: Vec<(String, String)> = (0..n).map(|n| (get_key(n), get_data(n))).collect();
    items.shuffle(&mut XorShiftRng::from_seed(Default::default()));

    let dbi = db.dbi();
    let env = _env.env();

    let mut key_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };
    let mut data_val: MDBX_val = MDBX_val {
        iov_len: 0,
        iov_base: ptr::null_mut(),
    };

    b.iter(|| unsafe {
        let mut txn: *mut MDBX_txn = ptr::null_mut();
        mdbx_txn_begin_ex(env, ptr::null_mut(), 0, &mut txn, ptr::null_mut());

        let mut i: ::libc::c_int = 0;
        for &(ref key, ref data) in items.iter() {
            key_val.iov_len = key.len() as size_t;
            key_val.iov_base = key.as_bytes().as_ptr() as *mut _;
            data_val.iov_len = data.len() as size_t;
            data_val.iov_base = data.as_bytes().as_ptr() as *mut _;

            i += mdbx_put(txn, dbi, &key_val, &mut data_val, 0);
        }
        assert_eq!(0, i);
        mdbx_txn_abort(txn);
    });
}
