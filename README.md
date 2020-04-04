# mdbx-sys

This repo is a fork of [mozilla/lmdb-rs](https://github.com/mozilla/lmdb-rs)
with patches \to make it work with [erthink/libmdbx](https://github.com/erthink/libmdbx).

## Building from Source

```bash
git clone --recursive git@github.com:Kerollmops/mdbx-rs.git
cd mdbx-rs
cargo build
```

## Publishing to crates.io

To publish the mdbx-sys crate to crates.io:

```bash
git clone --recursive git@github.com:Kerollmops/mdbx-rs.git
cd mdbx-rs/mdbx-sys
# Update the version string in mdbx-sys/Cargo.toml and mdbx-sys/src/lib.rs.
# We are forced to precompile the library, this is because of mdbx
# needing the GIT_DIR and cargo publish that doesn't copy it with the sources.
cargo clean && cargo check
cargo publish --allow-dirty
git tag mdbx-sys-$VERSION # where $VERSION is the updated version string
git push git@github.com:Kerollmops/mdbx-rs.git --tags
```

## Updating the libmdbx Version

To update the libmdbx version you must clone it and copy the `dist/` folder in `mdbx-sys/`.
Make sure to follow the [building steps](https://github.com/erthink/libmdbx#building).

```bash
# clone libmmdbx to a repository outside at specific tag
git clone https://github.com/erthink/libmdbx.git ../libmdbx --branch v0.7.0
make -C ../libmdbx dist

# copy the `libmdbx/dist/` folder just created into `mdbx-sys/libmdbx`
rm -rf mdbx-sys/libmdbx
cp -R ../libmdbx/dist mdbx-sys/libmdbx

# add the changes to the next commit you will make
git add mdbx-sys/libmdbx
```
