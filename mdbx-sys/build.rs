#[cfg(feature = "bindgen")]
#[path = "bindgen.rs"]
mod generate;

use std::{env, path::PathBuf};

fn main() {
    #[cfg(feature = "bindgen")]
    generate::generate();

    let mut mdbx = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    mdbx.push("libmdbx");

    let mut builder = cc::Build::new();

    builder
        .file(mdbx.join("mdbx.c"))
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wbad-function-cast")
        .flag_if_supported("-Wuninitialized");

    if env::var("CARGO_FEATURE_WITH_ASAN").is_ok() {
        builder.flag("-fsanitize=address");
    }

    if env::var("CARGO_FEATURE_WITH_FUZZER").is_ok() {
        builder.flag("-fsanitize=fuzzer");
    } else if env::var("CARGO_FEATURE_WITH_FUZZER_NO_LINK").is_ok() {
        builder.flag("-fsanitize=fuzzer-no-link");
    }

    let flags = format!("{:?}", builder.get_compiler().cflags_env());
    builder.define("MDBX_BUILD_FLAGS", flags.as_str());

    builder.compile("libmdbx.a")
}
