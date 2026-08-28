# libmdbx-rs

Rust bindings for [libmdbx](https://libmdbx.dqdkfa.ru).

## Build requirements

Building this crate compiles the vendored libmdbx C sources and generates FFI
bindings, so you need:

- A C compiler (any toolchain supported by the
  [cc](https://crates.io/crates/cc) crate).
- [libclang](https://rust-lang.github.io/rust-bindgen/requirements.html) for
  [bindgen](https://crates.io/crates/bindgen) (on Windows, point `LIBCLANG_PATH`
  at your LLVM installation).

## Usage

```rust,no_run
use libmdbx::{Database, DatabaseOptions, NoWriteMap, WriteFlags};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Slots for named tables must be reserved when opening the database.
    let options = DatabaseOptions {
        max_tables: Some(1),
        ..Default::default()
    };
    let path = std::env::temp_dir().join("libmdbx-demo");
    let db = Database::<NoWriteMap>::open_with_options(&path, options)?;

    // Write
    let txn = db.begin_rw_txn()?;
    let table = txn.create_table(Some("pets"), Default::default())?;
    txn.put(&table, b"cat", b"meow", WriteFlags::UPSERT)?;
    txn.commit()?;

    // Read
    let txn = db.begin_ro_txn()?;
    let table = txn.open_table(Some("pets"))?;
    let value = txn.get::<Vec<u8>>(&table, b"cat")?.unwrap();
    assert_eq!(value, b"meow");

    Ok(())
}
```

## Features

| Feature          | Description                                                                       |
|------------------|-----------------------------------------------------------------------------------|
| `orm`            | Typed table mapping: declare tables with the `table!` macro and use typed keys/values (`libmdbx::orm`). |
| `cbor`           | Use any serde type as an ORM key/value via CBOR (`cbor_table_object!`, backed by ciborium). Implies `orm`. |
| `bytes`          | ORM codec implementations for [`bytes::Bytes`](https://docs.rs/bytes).             |
| `lifetimed-bytes`| Zero-copy lifetime-carrying `Bytes<'tx>` handles for core transaction reads.       |

## License

The entire code within this repository is licensed under the [Apache License 2.0](./LICENSE).
