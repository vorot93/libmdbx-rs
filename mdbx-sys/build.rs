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

    let flags = format!("{:?}", builder.get_compiler().cflags_env());
    builder.define("MDBX_BUILD_FLAGS", flags.as_str());

    builder.compile("libmdbx.a")
}
