extern crate bindgen;
extern crate cc;
extern crate pkg_config;

use bindgen::callbacks::IntKind;
use bindgen::callbacks::ParseCallbacks;
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

#[derive(Debug)]
struct Callbacks;

impl ParseCallbacks for Callbacks {
    fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
        match name {
            "MDB_SUCCESS"
            | "MDB_KEYEXIST"
            | "MDB_NOTFOUND"
            | "MDB_PAGE_NOTFOUND"
            | "MDB_CORRUPTED"
            | "MDB_PANIC"
            | "MDB_VERSION_MISMATCH"
            | "MDB_INVALID"
            | "MDB_MAP_FULL"
            | "MDB_DBS_FULL"
            | "MDB_READERS_FULL"
            | "MDB_TLS_FULL"
            | "MDB_TXN_FULL"
            | "MDB_CURSOR_FULL"
            | "MDB_PAGE_FULL"
            | "MDB_MAP_RESIZED"
            | "MDB_INCOMPATIBLE"
            | "MDB_BAD_RSLOT"
            | "MDB_BAD_TXN"
            | "MDB_BAD_VALSIZE"
            | "MDB_BAD_DBI"
            | "MDB_LAST_ERRCODE" => Some(IntKind::Int),
            _ => Some(IntKind::UInt),
        }
    }
}

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
            .flag("-Wno-unused-parameter")
            .compile("liblmdb.a")
    }

    let bindings = bindgen::Builder::default()
        .header(lmdb.join("lmdb.h").to_string_lossy())
        .whitelist_var("^(MDB|mdb)_.*")
        .whitelist_type("^(MDB|mdb)_.*")
        .whitelist_function("^(MDB|mdb)_.*")
        .ctypes_prefix("::libc")
        .blacklist_item("mode_t")
        .blacklist_item("^__.*")
        .parse_callbacks(Box::new(Callbacks {}))
        .layout_tests(false)
        .prepend_enum_name(false)
        .rustfmt_bindings(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
