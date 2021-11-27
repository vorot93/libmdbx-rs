# libmdbx-rs

This repo is a fork of [mozilla/lmdb-rs](https://github.com/mozilla/lmdb-rs)
with patches to make it work with [libmdbx](https://github.com/erthink/libmdbx).

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
