extern crate cc;
extern crate pkg_config;

use std::env;
use std::path::PathBuf;

#[cfg(feature = "mdb_idl_logn_8")]
const MDB_IDL_LOGN: u8 = 8;
#[cfg(feature = "mdb_idl_logn_9")]
const MDB_IDL_LOGN: u8 = 9;
#[cfg(feature = "mdb_idl_logn_10")]
const MDB_IDL_LOGN: u8 = 10;
#[cfg(feature = "mdb_idl_logn_11")]
const MDB_IDL_LOGN: u8 = 11;
#[cfg(feature = "mdb_idl_logn_12")]
const MDB_IDL_LOGN: u8 = 12;
#[cfg(feature = "mdb_idl_logn_13")]
const MDB_IDL_LOGN: u8 = 13;
#[cfg(feature = "mdb_idl_logn_14")]
const MDB_IDL_LOGN: u8 = 14;
#[cfg(feature = "mdb_idl_logn_15")]
const MDB_IDL_LOGN: u8 = 15;
#[cfg(not(any(
    feature = "mdb_idl_logn_8",
    feature = "mdb_idl_logn_9",
    feature = "mdb_idl_logn_10",
    feature = "mdb_idl_logn_11",
    feature = "mdb_idl_logn_12",
    feature = "mdb_idl_logn_13",
    feature = "mdb_idl_logn_14",
    feature = "mdb_idl_logn_15",
)))]
const MDB_IDL_LOGN: u8 = 16;

fn main() {
    let mut lmdb = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    lmdb.push("lmdb");
    lmdb.push("libraries");
    lmdb.push("liblmdb");

    if !pkg_config::find_library("liblmdb").is_ok() {
        cc::Build::new()
            .define("MDB_IDL_LOGN", Some(MDB_IDL_LOGN.to_string().as_str()))
            .file(lmdb.join("mdb.c"))
            .file(lmdb.join("midl.c"))
            // https://github.com/mozilla/lmdb/blob/b7df2cac50fb41e8bd16aab4cc5fd167be9e032a/libraries/liblmdb/Makefile#L23
            .flag_if_supported("-Wno-unused-parameter")
            .flag_if_supported("-Wbad-function-cast")
            .flag_if_supported("-Wuninitialized")
            .compile("liblmdb.a")
    }
}
