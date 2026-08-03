# ThreatFlux Cache

[![Crates.io](https://img.shields.io/crates/v/threatflux-cache.svg)](https://crates.io/crates/threatflux-cache)
[![docs.rs](https://docs.rs/threatflux-cache/badge.svg)](https://docs.rs/threatflux-cache)
[![CI](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/ci.yml/badge.svg)](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/ci.yml)
[![Security](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/security.yml/badge.svg)](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/security.yml)
[![CodeQL](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/github-code-scanning/codeql)
[![MSRV](https://img.shields.io/badge/MSRV-1.95.0-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An async, typed cache for Rust applications that need pluggable storage, bounded
key histories, metadata, and simple entry queries.

ThreatFlux Cache keeps its working set in memory. Applications can use the
built-in memory backend, enable filesystem snapshots, or implement
`StorageBackend` for another persistence mechanism.

## Highlights

- Generic, serializable keys, values, and metadata
- `put` semantics for one current value per key
- `add_entry` semantics for bounded per-key history
- LRU, LFU, FIFO, TTL, and manual eviction strategies
- Timestamp, access-count, key-pattern, and metadata-category filters
- Bounded, versioned JSON filesystem snapshots
- An async API built on Tokio with no unsafe code in the crate

## Install

The default features enable the filesystem backend and JSON serialization. The
default cache type still uses the in-memory backend unless you construct a
`FilesystemBackend` explicitly.

```toml
[dependencies]
threatflux-cache = "0.2.0"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

For a memory-only build:

```toml
[dependencies]
threatflux-cache = { version = "0.2.0", default-features = false }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

ThreatFlux Cache requires Rust 1.95.0 or newer.

## Quick start

```rust
use serde::{Deserialize, Serialize};
use threatflux_cache::{AsyncCache, Cache, CacheConfig};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, User> = Cache::with_config(CacheConfig::default()).await?;
    let key = "user:1".to_owned();

    cache
        .put(
            key.clone(),
            User {
                id: 1,
                name: "Alice".to_owned(),
            },
        )
        .await?;

    assert_eq!(
        cache.get(&key).await?.map(|user| user.name),
        Some("Alice".to_owned())
    );
    Ok(())
}
```

`put` replaces the history for its key. Use `CacheEntry` and `add_entry` when
you want to retain multiple versions:

```rust
use threatflux_cache::{Cache, CacheConfig, CacheEntry};

# async fn example() -> threatflux_cache::Result<()> {
let cache: Cache<String, String> = Cache::with_config(
    CacheConfig::default().with_max_entries_per_key(3),
)
.await?;

cache
    .add_entry(CacheEntry::new("report".to_owned(), "version 1".to_owned()))
    .await?;
cache
    .add_entry(CacheEntry::new("report".to_owned(), "version 2".to_owned()))
    .await?;

assert_eq!(cache.get_entries(&"report".to_owned()).await.unwrap().len(), 2);
# Ok(())
# }
```

See [`examples/basic_usage.rs`](examples/basic_usage.rs) and
[`examples/custom_entry.rs`](examples/custom_entry.rs) for complete programs.

## Filesystem snapshots

Filesystem persistence requires the `filesystem-backend` feature. The backend
path is authoritative; configure persistence separately so the cache loads and
saves snapshots.

```rust
use threatflux_cache::{
    AsyncCache, Cache, CacheConfig, FilesystemBackend, PersistenceConfig,
};

# async fn example() -> threatflux_cache::Result<()> {
let path = std::path::PathBuf::from("./cache-data");
let backend = FilesystemBackend::<String, String>::new(&path).await?;
let config = CacheConfig::default()
    .with_persistence(PersistenceConfig::enabled());
let cache = Cache::new(config, backend).await?;
cache.put("greeting".to_owned(), "hello".to_owned()).await?;
cache.flush().await?;
# Ok(())
# }
```

`flush` waits for the backend to finish writing the current state. Snapshots are
not a transactional database or substitute for a system of record. Read
[`docs/PERSISTENCE.md`](docs/PERSISTENCE.md) before relying on restart recovery
or sharing a directory between processes.

## Feature flags

| Feature              | Default | Surface enabled                                                               |
| -------------------- | :-----: | ----------------------------------------------------------------------------- |
| `filesystem-backend` |   yes   | `FilesystemBackend`; also enables Tokio filesystem I/O and JSON serialization |
| `full`               |   no    | Alias for every supported optional feature                                    |

For tested feature combinations and current limitations, see
[`docs/FEATURES.md`](docs/FEATURES.md).

## Behavioral boundaries

- `get` returns the newest entry and records an access; `get_entries` records an
  access for every returned version.
- Search patterns are case-sensitive substrings of the key's `Display` output;
  values are not full-text searched.
- `get`, `get_entries`, `contains`, `len`, and search exclude expired entries;
  search can opt into expired results.
- LRU, LFU, and FIFO eviction remove one entire key and its history when the
  global limit is crossed.
- `default_ttl` applies to entries that do not already have an explicit expiry.
- Zero entry limits and a zero persistence sync interval are rejected; with
  eviction disabled, an insertion that would grow a full cache returns
  `CapacityExceeded`.

The complete contract is documented in
[`docs/BEHAVIOR.md`](docs/BEHAVIOR.md). This crate is pre-1.0; minor releases may
refine APIs and on-disk representation.

## Extending the cache

Implement [`StorageBackend`](https://docs.rs/threatflux-cache/latest/threatflux_cache/storage/trait.StorageBackend.html)
to provide a different snapshot store. Implement `EntryMetadata` to attach
domain-specific metadata and expose a category to the built-in search filters.

## Development and security

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — contribution workflow
- [`DEVELOPMENT.md`](DEVELOPMENT.md) — local setup and commands
- [`TESTING.md`](TESTING.md) — validation matrix
- [`SECURITY.md`](SECURITY.md) — private vulnerability reporting
- [`CHANGELOG.md`](CHANGELOG.md) — release history

## License

Licensed under the [MIT License](LICENSE).
