# libmdbx-rs

Idiomatic, safe Rust bindings for [libmdbx](https://libmdbx.dqdkfa.ru),
an embedded key-value store.

The crate wraps the vendored C library (`mdbx-sys`) behind a `Send + Sync`
transaction API. An optional typed ORM lives under `libmdbx::orm`.

[Documentation](https://docs.rs/libmdbx) · [Repository](https://github.com/vorot93/libmdbx-rs)

The crate is pre-1.0: breaking changes are called out in `CHANGELOG.md`.

## Build requirements

Building compiles the vendored libmdbx C sources and generates FFI bindings:

- A C compiler (any toolchain supported by the [cc](https://crates.io/crates/cc) crate).
- [libclang](https://rust-lang.github.io/rust-bindgen/requirements.html) for
  [bindgen](https://crates.io/crates/bindgen) (on Windows, point `LIBCLANG_PATH`
  at your LLVM installation).

Rust: latest stable (edition 2024).

## Usage

The unnamed table needs no extra options:

```rust,no_run
use libmdbx::{Database, NoWriteMap, WriteFlags};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join("libmdbx-demo");
    let db = Database::<NoWriteMap>::open(&path)?;

    let txn = db.begin_rw_txn()?;
    let table = txn.create_table(None, Default::default())?;
    txn.put(&table, b"cat", b"meow", WriteFlags::UPSERT)?;
    txn.commit()?;

    let txn = db.begin_ro_txn()?;
    let table = txn.open_table(None)?;
    let value = txn.get::<Vec<u8>>(&table, b"cat")?.unwrap();
    assert_eq!(value, b"meow");

    Ok(())
}
```

Named tables need slots reserved at open (`max_tables` defaults to 0).
Without that, `open_table` / `create_table` fail with `Error::DbsFull`:

```rust,no_run
use libmdbx::{Database, DatabaseOptions, NoWriteMap};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::<NoWriteMap>::open_with_options(
        "mydb",
        DatabaseOptions {
            max_tables: Some(4),
            ..Default::default()
        },
    )?;
    let txn = db.begin_rw_txn()?;
    txn.create_table(Some("pets"), Default::default())?;
    txn.commit()?;
    Ok(())
}
```

Walk a table with [`Cursor`]. Reads in a read-only transaction can be
zero-copy (`Cow<[u8]>`); read-write transactions always copy, because libmdbx
may relocate pages on a later write.

`Database` is generic over the write path:

- [`NoWriteMap`] — modified pages are buffered and written through file I/O.
- [`WriteMap`] — writes go through the memory map (typically faster). The ORM
  defaults to this.

## ORM

Enable the `orm` feature and see `libmdbx::orm`.
Declare tables with `table!` / `dupsort!`, assemble a `DatabaseChart`, and use
typed `get` / `upsert` / cursors. `cbor` adds serde-via-CBOR table objects.

## Features

| Feature           | Description                                                                                          |
|-------------------|------------------------------------------------------------------------------------------------------|
| `orm`             | Typed table mapping (`libmdbx::orm`).                                                                |
| `cbor`            | Any serde type as an ORM key/value via CBOR (`cbor_table_object!`, ciborium). Implies `orm`.         |
| `bytes`           | ORM codec implementations for [`bytes::Bytes`](https://docs.rs/bytes).                               |
| `lifetimed-bytes` | Zero-copy lifetime-carrying `Bytes<'tx>` handles for core transaction reads.                         |

## FFI crate

[`mdbx-sys`](https://crates.io/crates/mdbx-sys) is the raw bindgen crate. This
workspace pins it exactly (`=`) so the wrapper and FFI crate always match.

## License

Apache License 2.0. See [LICENSE](./LICENSE).
