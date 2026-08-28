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
`cargo hack clippy --workspace --feature-powerset --depth 2 -- -D warnings` and
`cargo hack test --workspace --feature-powerset --depth 2` on Linux, macOS, and
Windows. Any change must hold under feature-powerset depth 2, not just
all-features — feature-gated code paths can break in single-feature builds that
all-features hides.

## Vendored upstream code

`mdbx-sys/libmdbx/` is a vendored upstream libmdbx source subtree. **Never edit
it.** Fixes belong in the Rust wrapper; genuine upstream bugs should be reported
and fixed upstream, then re-vendored.

## Version coupling

The root crate pins the FFI crate exactly:
`ffi = { package = "mdbx-sys", version = "=14.3.1", path = "./mdbx-sys" }`.
When releasing, bump `mdbx-sys/Cargo.toml` and the pin in the root `Cargo.toml`
together so they always match (the `=` pin guarantees a mismatch fails to
resolve rather than silently mixing versions).

## API stability

The crate is pre-1.0: breaking API changes are acceptable when warranted — bump
the minor version and call them out in the changelog/release notes.
