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
- In-process writers queue on `WriterGate` (a mutex + condvar in `Database`),
  held by each top-level write `Transaction` until after its commit/abort.
  Needed because the manager thread begins every write transaction, so
  libmdbx sees its writer lock held by the *same* thread and fails a second
  begin with `MDBX_BUSY` rather than blocking. The previous `MDBX_BUSY` retry
  loop (backoff to 800 ms) delayed handoff by up to 800 ms. Cross-process
  writers still block inside `mdbx_txn_begin` on the manager thread.

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
  deadlock, not UB. The buffer is zero-filled through the raw pointer before
  the `&mut [u8]` is formed: libmdbx may return stale or (with `no_meminit`)
  uninitialized memory, and a `&mut [MaybeUninit<u8>]` API was rejected as
  unergonomic for a memset that costs far less than the write itself.
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

- `Iter`/`IntoIter` share one engine, `Range` (`src/cursor.rs`): a front
  cursor plus a back cursor copied from the front on the first `next_back`.
  Each end stops when it reaches a position the other end already yielded,
  compared with `mdbx_cursor_compare` — the table's own key *and duplicate*
  order. This gives the full `DoubleEndedIterator` + `FusedIterator`
  contract under any interleaving. Rejected alternatives: a single shared
  cursor (the ends trample each other's position: items skipped, repeated,
  or yielded after `None`), and byte-wise key bounds (wrong for
  `INTEGER_KEY`/`REVERSE_KEY` tables and blind to duplicates).
- The back end is bounded by the *front's position*, never by a stored key:
  before the front has yielded anything it is positioned (state `Pending`),
  so `iter_from(k).rev()` stops at `k` and `into_iter_back_from(k).rev()`
  stops at the largest key `<= k`. Seek-based constructors seek eagerly; a
  seek that finds nothing yields an empty iterator.
- Fetch, position comparison and decoding happen under one transaction-lock
  hold, so a writer sharing the transaction cannot move the page between them.
  A libmdbx error ends iteration; a decode error is yielded and iteration
  continues.
- `IterDup` yields `Result<IntoIter, Error>` per key: anything except
  `MDBX_NOTFOUND | MDBX_ENODATA` is an error, never a silent end-of-iteration.
- `Cursor::try_clone` closes a half-built raw cursor directly: dropping a
  `Cursor` while holding the transaction mutex self-deadlocks (the mutex is
  not reentrant).
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
- `table!`/`dupsort!` call each other as `$crate::table!`/`$crate::dupsort!`
  (unqualified calls resolve at the call site and break path invocation) and
  accept doc-less declarations (the doc matcher is `*`,
  not `+`).

## Error taxonomy

One `Error` enum for the whole crate (core and ORM): every MDBX-specific code
has a named variant; anything else is preserved in `Other(c_int)`.
`DecodeError`/`EncodeError`/`IoError` carry boxed `std::error::Error` sources.
`Display` formats via thread-safe `mdbx_strerror_r` with lossy UTF-8
(`mdbx_strerror` is documented non-thread-safe, and locale output is not
guaranteed UTF-8).

## Logging

libmdbx logs to stderr by default (level NOTICE, in release builds too), which
is unacceptable for a library. `logging::install` (called once per process
before the first `mdbx_env_create`) swaps in `mdbx_setup_debug_nofmt` with a
forwarder to the `log` crate, leaving libmdbx's level unchanged so `log`
filters decide. The `nofmt` variant avoids C varargs; libmdbx formats into a
leaked 1 KiB buffer under its own debug lock, and reports the *untruncated*
`vsnprintf` length, so the forwarder clamps it to the buffer.

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
