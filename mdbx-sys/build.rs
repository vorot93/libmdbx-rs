use bindgen::{
    Formatter,
    callbacks::{IntKind, ParseCallbacks},
};
use std::{env, path::PathBuf};

#[derive(Debug)]
struct Callbacks;

impl ParseCallbacks for Callbacks {
    fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
        match name {
            "MDBX_SUCCESS"
            | "MDBX_KEYEXIST"
            | "MDBX_NOTFOUND"
            | "MDBX_PAGE_NOTFOUND"
            | "MDBX_CORRUPTED"
            | "MDBX_PANIC"
            | "MDBX_VERSION_MISMATCH"
            | "MDBX_INVALID"
            | "MDBX_MAP_FULL"
            | "MDBX_DBS_FULL"
            | "MDBX_READERS_FULL"
            | "MDBX_TLS_FULL"
            | "MDBX_TXN_FULL"
            | "MDBX_CURSOR_FULL"
            | "MDBX_PAGE_FULL"
            | "MDBX_MAP_RESIZED"
            | "MDBX_INCOMPATIBLE"
            | "MDBX_BAD_RSLOT"
            | "MDBX_BAD_TXN"
            | "MDBX_BAD_VALSIZE"
            | "MDBX_BAD_DBI"
            | "MDBX_LOG_DONTCHANGE"
            | "MDBX_DBG_DONTCHANGE"
            | "MDBX_RESULT_TRUE"
            | "MDBX_UNABLE_EXTEND_MAPSIZE"
            | "MDBX_PROBLEM"
            | "MDBX_LAST_LMDB_ERRCODE"
            | "MDBX_BUSY"
            | "MDBX_EMULTIVAL"
            | "MDBX_EBADSIGN"
            | "MDBX_WANNA_RECOVERY"
            | "MDBX_EKEYMISMATCH"
            | "MDBX_TOO_LARGE"
            | "MDBX_THREAD_MISMATCH"
            | "MDBX_TXN_OVERLAPPING"
            | "MDBX_BACKLOG_DEPLETED"
            | "MDBX_DUPLICATED_CLK"
            | "MDBX_DANGLING_DBI"
            | "MDBX_OUSTED"
            | "MDBX_MVCC_RETARDED"
            | "MDBX_LAST_ERRCODE" => Some(IntKind::Int),
            _ => Some(IntKind::UInt),
        }
    }
}

fn main() {
    let mut mdbx = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    mdbx.push("libmdbx");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let bindings = bindgen::Builder::default()
        .header(mdbx.join("mdbx.h").to_string_lossy())
        .allowlist_var("^(MDBX|mdbx)_.*")
        .allowlist_type("^(MDBX|mdbx)_.*")
        .allowlist_function("^(MDBX|mdbx)_.*")
        .size_t_is_usize(true)
        .ctypes_prefix("::libc")
        .parse_callbacks(Box::new(Callbacks))
        .layout_tests(false)
        .prepend_enum_name(false)
        .generate_comments(false)
        .disable_header_comment()
        .formatter(Formatter::None)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    let mut mdbx = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    mdbx.push("libmdbx");

    let mut cc_builder = cc::Build::new();
    cc_builder
        .flag_if_supported("-Wall")
        .flag_if_supported("-Werror")
        .flag_if_supported("-ffunction-sections")
        .flag_if_supported("-fvisibility=hidden")
        .flag_if_supported("-Wno-error=attributes");

    let flags = format!(
        "\"-NDEBUG={} {}\"",
        u8::from(!cfg!(debug_assertions)),
        cc_builder
            .get_compiler()
            .cflags_env()
            .to_str()
            .unwrap()
            .trim()
    );

    cc_builder
        .define("MDBX_BUILD_FLAGS", flags.as_str())
        .define("MDBX_TXN_CHECKOWNER", "0");

    // __cpu_model is not available in musl
    if env::var("TARGET").unwrap().ends_with("-musl") {
        cc_builder.define("MDBX_HAVE_BUILTIN_CPU_SUPPORTS", "0");
    }

    if cfg!(windows) {
        println!(r"cargo:rustc-link-lib=dylib=ntdll");
        println!(r"cargo:rustc-link-lib=dylib=user32");
    }

    cc_builder.file(mdbx.join("mdbx.c")).compile("libmdbx.a");
}
