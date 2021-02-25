#![deny(warnings)]
#![allow(non_camel_case_types, non_upper_case_globals)]
#![allow(clippy::all)]
#![doc(html_root_url = "https://docs.rs/mdbx-sys/0.9.3")]

extern crate libc;

mod bindings;
pub use bindings::*;
