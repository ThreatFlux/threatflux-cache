# Development

This guide covers the local workflow for ThreatFlux Cache. See
[`CONTRIBUTING.md`](CONTRIBUTING.md) before opening a pull request and
[`TESTING.md`](TESTING.md) for the full validation matrix.

## Prerequisites

- Rust 1.95.0 or newer, installed with [rustup](https://rustup.rs/)
- `rustfmt` and Clippy
- Git

The crate has no required native system libraries.

```bash
rustup component add rustfmt clippy
cargo check --locked
```

Optional repository checks use these tools:

```bash
cargo install --locked cargo-audit
cargo install --locked cargo-deny
cargo install --locked cargo-hack
cargo install --locked cargo-semver-checks
```

## Useful commands

```bash
cargo fmt --all
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
cargo test --no-default-features --locked
cargo test --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked
cargo build --examples --all-features --locked
```

Run `make help` for the repository's command aliases. Direct Cargo commands are
the source of truth and work on every supported platform.

## Feature work

The default build and the no-default-features build are both supported. When a
change touches conditional code, validate the feature powerset:

```bash
cargo hack check --feature-powerset --depth 2 --locked
```

Keep feature-gated public documentation under matching `#[cfg]` attributes and
avoid making a core API depend on a feature unless that dependency is part of
the documented contract.

## Design expectations

- Keep cache semantics independent from backend implementation details.
- Do not perform blocking filesystem work on Tokio executor threads.
- Treat persisted bytes and custom-backend output as untrusted input.
- Bound allocations and arithmetic derived from persisted data.
- Preserve deterministic behavior for eviction and serialization where the API
  promises it.
- Document any compatibility change to the public API or snapshot format.

Relevant contracts live in [`docs/BEHAVIOR.md`](docs/BEHAVIOR.md) and
[`docs/PERSISTENCE.md`](docs/PERSISTENCE.md).

## Tests

Unit tests live beside their modules and integration tests live in `tests/`.
Examples must compile in every feature combination for which they are enabled.
Add regression tests for fixes, especially around capacity boundaries,
expiration, malformed snapshots, interrupted writes, and filesystem boundaries.

Avoid time-based sleeps where a timestamp can be set directly. Tests that use
the filesystem should use a unique temporary directory and must not write to a
shared `/tmp` path.

## Documentation

Public API changes require rustdoc updates and, when user-visible, corresponding
README or guide changes. Check relative links before submitting:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked
cargo test --doc --all-features --locked
```

## Security-sensitive changes

Do not open a public issue for a suspected vulnerability. Follow
[`SECURITY.md`](SECURITY.md). For dependency and supply-chain checks, run:

```bash
cargo audit --deny warnings
cargo deny check
```
