extern crate cc;
extern crate pkg_config;

#[cfg(feature = "bindgen")]
extern crate bindgen;

#[cfg(feature = "bindgen")]
#[path = "bindgen.rs"]
mod generate;

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    #[cfg(feature = "bindgen")]
    generate::generate();

    let mut mdbx = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    mdbx.push("libmdbx");

    if !pkg_config::find_library("libmdbx").is_ok() {
        Command::new("make")
            .args(&["-C", &mdbx.display().to_string()])
            .status()
            .expect("failed to make!");
    }
}
