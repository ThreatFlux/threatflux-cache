# Testing

ThreatFlux Cache supports its default feature set, a memory-only build, every
declared feature combination, and Rust 1.95.0 or newer.

## Fast local loop

Run these while developing:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

## Pull-request validation

Before opening a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo clippy --all-targets --no-default-features --locked -- -D warnings
cargo test --locked
cargo test --no-default-features --locked
cargo test --all-features --locked
cargo test --doc --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked
cargo build --examples --all-features --locked
```

Validate the declared minimum Rust version separately:

```bash
rustup toolchain install 1.95.0 --profile minimal
cargo +1.95.0 check --all-targets --all-features --locked
```

For conditional-code changes, install `cargo-hack` and check the feature
powerset:

```bash
cargo hack check --feature-powerset --depth 2 --locked
```

## Security and dependency checks

```bash
cargo audit --deny warnings
cargo deny check
```

These tools use current advisory and index data, so a result can change without a
source change. Investigate new failures rather than adding broad ignores.

## What to test

| Area            | Required coverage                                                                      |
| --------------- | -------------------------------------------------------------------------------------- |
| Core operations | Put/get replacement, history insertion, removal, clear, and statistics                 |
| Capacity        | Per-key and global boundaries, every eviction policy, and zero limits                  |
| Expiration      | Boundary timestamps and each read/search path                                          |
| Concurrency     | Concurrent readers/writers, persistence scheduling, and shutdown                       |
| Persistence     | Round trips, malformed/truncated data, stale files, collisions, and interrupted writes |
| Features        | Default, no-default, each independent feature, and all features                        |
| Custom types    | Non-string keys, metadata, and custom backend errors                                   |

Filesystem tests must use isolated temporary directories. Tests should not rely
on network services, wall-clock sleeps, execution order, or a developer's home
directory.

## Release validation

Release candidates also require the checklist in
[`docs/RELEASING.md`](docs/RELEASING.md), including package inspection and
semantic-version analysis.
