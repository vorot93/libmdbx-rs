# Design

Cross-cutting design decisions and the alternatives that were rejected. Facts
local to a function live in its rustdoc; this file is for the things a reader of
any single file cannot reconstruct. Working conventions are in `AGENTS.md`.

## Threading and transaction model

- Every FFI call that touches a transaction goes through an
  `Arc<Mutex<TxnPtr>>` (`txn_execute`), and read-write begin/commit/abort are
  funneled to a single manager thread owned by `Database` (created for
  `Mode::ReadWrite`). This is what makes `Transaction` `Send + Sync` safe.
- `MDBX_NOSTICKYTHREADS` is forced on at env open and this is load-bearing:
  without it libmdbx pins transactions to OS threads and every cross-thread
  use above is a `MDBX_THREAD_MISMATCH`. Do not remove the flag.
- If the manager thread is gone (environment dropped while a transaction was
  still live), commit paths return `Error::Panic` and `Drop` deliberately leaks
  the handle instead of panicking — the environment is being destroyed either
  way.
- `begin_rw_txn` retries `MDBX_BUSY` with exponential backoff (25 ms doubling,
  capped at 800 ms).

## Memory-safety policy

- **Reads**: read-only transactions get zero-copy borrowed views (`Cow::Borrowed`)
  — MVCC snapshots pin pages for the transaction's lifetime. Read-write
  transactions always copy: libmdbx documents that values are valid "only until
  a subsequent update operation", and the old `mdbx_is_dirty` check only covered
  already-dirtied pages, not clean pages invalidated by later writes. The
  `Decodable` trait doc carries the zero-copy contract for custom impls.
- **`put_with`** (MDBX's reserve operation): the reserved buffer is handed to
  a caller closure and never escapes it; the `mdbx_put(RESERVE)` call and the
  closure both run inside `txn_execute`, so concurrent writers cannot relocate
  the page under a live `&mut`. A `&'txn mut self` signature was rejected:
  `Table<'txn>` borrows the transaction, so `&mut self` + `&Table<'txn>`
  cannot be called (E0502). The closure must not use the same transaction —
  deadlock, not UB.
- **Write flags**: `MDBX_RESERVE` and `MDBX_MULTIPLE` change what libmdbx
  does with the data argument (`MULTIPLE` reads *and writes* `data[1]`, i.e.
  past a single `MDBX_val`), so they are not `WriteFlags` members, and every
  put/del path masks to the declared flags (`WriteFlags::ffi_bits`) because
  `from_bits_retain` can still smuggle raw bits in. They are only set by
  `put_with` and `put_multiple`, which build the matching data argument;
  `put_multiple` copies misaligned input into an 8-aligned buffer because
  libmdbx requires aligned `INTEGER_DUP` elements.
- **Empty values**: every `MDBX_val` → slice conversion guards
  `iov_len == 0` before `slice::from_raw_parts`; a NULL base with length 0 is
  instant UB by Rust's rules regardless of what libmdbx happens to return.
- **Cursor lifetimes**: cursors and iterators carry
  `PhantomData<fn() -> (&'txn (), ..)>`, which is *covariant* in `'txn`: the
  lifetime can only shrink, so borrow checking orders cursor-drop (and the end
  of every zero-copy borrow) before commit/abort. Never put `'txn` in argument
  position (`fn(&'txn ())`): that is contravariant, lets `'txn` be *extended*
  to `'static`, and lets a cursor or a borrowed value outlive its snapshot —
  this was a real use-after-free. `compile_fail` doctests on `Cursor` and
  `IntoIter` guard it.

## Iterator semantics

- `Iter`/`IntoIter` are backed by a single MDBX cursor. Mixed
  `next()`/`next_back()` past the point where the two directions meet can yield
  middle items twice — an accepted property of single-cursor DEI (documented on
  the types), not fixable without key-comparison machinery.
- `into_iter_back_from(bound)` stores the bound and stops both directions once
  the back direction crosses it (a `done` flag); without this, out-of-domain
  keys leaked from the front direction after back exhaustion.
- `IterDup` yields `Result<IntoIter, Error>` per key: anything except
  `MDBX_NOTFOUND | MDBX_ENODATA` is an error, never a silent end-of-iteration.
- libmdbx op trap: `MDBX_SET_UPPERBOUND` is an *exclusive* range-end op
  (positions at the first key strictly greater). `Cursor::set_upperbound`
  wraps `MDBX_TO_KEY_LESSER_OR_EQUAL` to deliver its documented
  "largest key <= X" semantics.

## ORM layer

- `orm::Database` defaults to `WriteMap`: libmdbx modifies the database
  directly in mapped memory and flushes with a single syscall — significantly
  faster for writes than the double-buffered default. It is safe here because
  the wrapper's safe surface exposes no stray pointers into the map (see
  memory-safety policy). `Database<NoWriteMap>` remains available.
- The ORM is generic over `DatabaseKind` but not over the core transaction
  kinds; all ORM errors are the crate's typed `Error` (anyhow was dropped:
  callers could not match error kinds without downcasting). `Encodable::encode`
  is fallible because CBOR serialization of user types can fail.
- `CutStart<T>` encodes fixed-width big-endian values with *trailing* zeros
  removed: the result is a byte prefix that compares `<=` the full encoding,
  so `SET_RANGE`/`seek_closest` never skips the value. The previous
  leading-zero cut produced seek keys greater than their values.
- `table!`/`dupsort!` accept doc-less declarations (the doc matcher is `*`,
  not `+`).

## Error taxonomy

One `Error` enum for the whole crate (core and ORM): every MDBX-specific code
has a named variant; anything else is preserved in `Other(c_int)`.
`DecodeError`/`EncodeError`/`IoError` carry boxed `std::error::Error` sources.
`Display` formats via thread-safe `mdbx_strerror_r` with lossy UTF-8
(`mdbx_strerror` is documented non-thread-safe, and locale output is not
guaranteed UTF-8).

## mdbx-sys

- Bindings are generated at build time into `OUT_DIR` (no pregenerated
  bindings to go stale); error-code macros are typed as signed `c_int` via the
  bindgen callback.
- The build script must read the *target* from cargo's env
  (`CARGO_CFG_TARGET_OS`, `DEBUG`) — `cfg!` in build.rs reflects the host and
  build-script profile, breaking cross-compilation and custom profiles.
- `-Werror` is deliberately not passed to the vendored C sources: a future
  compiler warning in frozen upstream code would otherwise break every
  consumer's build.
