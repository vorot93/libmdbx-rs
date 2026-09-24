# AGENTS.md

Working conventions for libmdbx-rs.

## Build & test

- Full test suite: `cargo test --all-features`
- ORM tests only: `cargo test --features orm,cbor --test orm`
- Clippy (must be clean): `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Format (note the non-default import granularity): `cargo fmt --all -- --config=imports_granularity=Crate`
- Rust policy: target latest stable; no MSRV is declared or guaranteed
  (edition 2024 implies a compiler >= 1.85 regardless).

CI (`.github/workflows/main.yml`) runs the fmt check above plus
`cargo hack clippy --workspace --feature-powerset --depth 2 --all-targets -- -D warnings`,
`cargo hack test --workspace --feature-powerset --depth 2` and
`cargo test --workspace --all-features` on Linux, macOS, and Windows, and
`cargo test --workspace --all-features --target x86_64-unknown-linux-musl` (needs
`musl-tools`). Any change must hold under feature-powerset depth 2, not just
all-features — feature-gated code paths can break in single-feature builds that
all-features hides (e.g. a helper only the ORM uses is dead code, hence a
clippy error, without `orm`). Run both `cargo hack` commands locally before
pushing feature-gated changes.

Benches: `autobenches = false` because `benches/utils.rs` is a module shared by
the declared benches; auto-discovery made it a bogus `utils` bench target.

To check which defines reach the vendored C build (e.g. `NDEBUG` vs
`MDBX_FORCE_ASSERTIONS`), run `CC_ENABLE_DEBUG_OUTPUT=1 cargo build -vv -p mdbx-sys`
after touching `mdbx-sys/build.rs`; the `cc` crate hides compiler commands otherwise.

## Gotchas

- Platform-gated code (`#[cfg(unix)]` / `#[cfg(windows)]`) is only linted on that
  platform, and a Windows clippy run can't be done from Linux without a MinGW
  toolchain (the C builds need `x86_64-w64-mingw32-gcc`). Keep imports used only by one
  platform inside its cfg'd block, or Windows CI fails with `unused_imports`.
- Named tables need slots reserved at open: `DatabaseOptions { max_tables: Some(n), .. }`.
  The default is 0, and `open_table`/`create_table` on a named table then fails with `DbsFull`.
- `README.md` is `include_str!`-ed as the crate-level doc (`src/lib.rs`), so its code
  blocks are doctests: examples that touch disk must be marked `no_run`, and they must
  compile (and ideally be run once manually) when the API changes.
- libmdbx cursor-op naming trap: `MDBX_SET_UPPERBOUND` positions at the first key
  *strictly greater* (it is an exclusive range-end op). For "largest key <= X" use
  `MDBX_TO_KEY_LESSER_OR_EQUAL` (what `Cursor::set_upperbound` wraps). Verify op
  semantics against `mdbx-sys/libmdbx/mdbx.h` before wiring new ones.

## Vendored upstream code

`mdbx-sys/libmdbx/` is a vendored upstream libmdbx source subtree. **Never edit
it.** Fixes belong in the Rust wrapper; genuine upstream bugs should be reported
and fixed upstream, then re-vendored.

## Version coupling

The root crate pins the FFI crate exactly:
`ffi = { package = "mdbx-sys", version = "=<version>", path = "./mdbx-sys" }`.
When releasing, bump `mdbx-sys/Cargo.toml` and the pin in the root `Cargo.toml`
together so they always match (the `=` pin guarantees a mismatch fails to
resolve rather than silently mixing versions).

## API stability

The crate is pre-1.0: breaking API changes are acceptable when warranted — bump
the minor version and call them out in `CHANGELOG.md`. Cross-cutting design
rationale lives in `DESIGN.md`.
