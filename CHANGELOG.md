# Changelog

## 0.8.0 - 2026-09-03

No API or behavior changes.

- Relicensed from MPL-2.0 to Apache-2.0 (both `libmdbx` and `mdbx-sys`); `NOTICE` and `CITATION.cff` added.
- Package metadata (license, authors, homepage, repository) is inherited from the workspace.

## 0.7.0 - 2026-08-28

### Breaking changes

- `Transaction::reserve` is now `Transaction::put_with`: it takes a closure that fills the reserved buffer in place; the buffer cannot escape the call, and the closure runs under the transaction lock (it must not use the same transaction — deadlock, not UB).
- `orm::Database` is parametrized over `DatabaseKind` (default `WriteMap`, the
  previous hardcoded kind); use `Database<NoWriteMap>` for the copy-on-write
  write path.
- ORM replaces `anyhow` with the crate's typed `Error`, and `Encodable::encode` is now fallible.
- `CutStart` cuts trailing zeros so prefix seeks no longer skip values shorter than the prefix.
- `walk_back` yields keys `<=` the given bound (via `MDBX_TO_KEY_LESSER_OR_EQUAL`; note `MDBX_SET_UPPERBOUND` positions at the first key *greater*).
- `IterDup::Item` is now `Result<IntoIter>`: iteration errors surface instead of silently truncating.
- Zero-copy `Cow` reads in RW transactions now copy, since RW page memory is not stable across writes.

### Fixes

- Cursor keys are always decoded, fixing a panic when a seek key's pointer compared equal to the returned key.
- `Transaction::table_flags` no longer always fails (`mdbx_dbi_flags_ex` state pointer was null); unknown bits are preserved.
- Error `Display` uses thread-safe `mdbx_strerror_r` with lossy UTF-8 conversion (was: non-thread-safe `mdbx_strerror` with unchecked UTF-8); error codes map totally to typed variants.
- Bounded `into_iter_back_from` hard-stops once the back direction crosses the bound, so out-of-domain keys can no longer leak from the front direction.
- `Iter`/`IntoIter` implement `DoubleEndedIterator` and `FusedIterator`; `set_upperbound` and reverse iteration added.
- `u8`/`u16` keys and values are supported.
- ORM respects the user's `max_tables` instead of overriding it.
- Empty `MDBX_val`s are guarded against null-pointer slice construction.
- WriteMap mode covered by tests; geometry-capped writes surface `Error::MapFull`.

## 14.3.1 - 2026-08-28

- mdbx-sys builds via target-aware env vars (fixes cross-compilation and non-Linux target-os builds); `-Werror` dropped.
- Packaging hygiene: `links` declaration, explicit feature list.
- Error-code constants are typed as `c_int` to match libmdbx return codes (fixes the misspelled `MDBX_DUPLICATED_LCK` entry that left it unsigned).
