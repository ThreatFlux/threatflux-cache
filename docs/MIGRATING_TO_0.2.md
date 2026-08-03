# Migrating to 0.2

ThreatFlux Cache 0.2 narrows the crate to behavior it actively implements and
hardens filesystem snapshot handling. Review this guide before changing the
dependency version.

## Toolchain

Rust 1.95.0 is the minimum supported version. Upgrade the build toolchain before
upgrading the crate.

## Feature flags

Remove these features from `Cargo.toml`:

- `bincode-serialization`
- `json-serialization`
- `compression`
- `openapi`
- `metrics`
- `tracing`

The filesystem backend now enables its JSON dependency directly; the standalone
`json-serialization` flag did not provide a usable backend by itself. The
`compression`, `openapi`, `metrics`, and `tracing` flags did not affect cache
operations in 0.1. Applications that used their dependencies or configuration
types should depend on the relevant crate directly and implement that behavior
at the application boundary.

The supported optional persistence surface is now filesystem snapshots encoded
as JSON. `default-features = false` remains the memory-only configuration.
The `full` alias remains valid and currently enables `filesystem-backend`.

## Existing snapshots

Version 0.2 uses a single versioned `cache.json` snapshot. It does not read
`.bin` snapshots created with `bincode-serialization` or the 0.1 per-key JSON
layout. Cache data should be reconstructible, so the preferred migration is:

1. Deploy a 0.1 build that can read the existing snapshot directory.
2. Export the logical entries to the application's source of truth. If needed,
   use an application-specific exporter while the 0.1 types are still available.
3. Stop every process using the old cache directory.
4. Upgrade to 0.2 with a new, empty directory and allow the cache to repopulate.
5. Remove the old files only after validating the new deployment.

Do not point 0.2 at a 0.1 directory and assume it was loaded. Legacy JSON and
Bincode files are reported as `UnsupportedPersistenceFormat`.
`FilesystemBackend::clear` intentionally refuses to delete them, so it is not a
migration tool. Archive or remove the old files explicitly only after completing
the export and validation steps above.

## Persistence API

The backend now owns the filesystem path, and shutdown persistence is explicit.

```rust
// 0.1
let persistence = PersistenceConfig::with_path(path);

// 0.2
let backend = FilesystemBackend::new(path).await?;
let persistence = PersistenceConfig::enabled();
let cache = Cache::new(
    CacheConfig::default().with_persistence(persistence),
    backend,
)
.await?;

// Before graceful shutdown:
cache.flush().await?;
```

`PersistenceConfig::path`, `with_path`, and `save_on_drop` were removed.
`Cache::flush` is the supported way to wait for the current state to reach the
backend.

## Other API changes

- `CacheError` is now non-exhaustive and adds snapshot-size and unsupported-
  format errors. Add a wildcard arm to exhaustive matches.
- Removed compression configuration/error types and the manual metrics module.
- Removed `SerializationFormat` and `FilesystemBackend::with_format`; the
  filesystem backend has one versioned JSON format. Custom backends should use
  their serializer directly.
- `StorageBackend::size_bytes` is now required instead of silently defaulting to
  zero, and the unused `StorageStats` container was removed.
- Removed the unused `SearchQuery::custom_predicates` field.
- Removed the unimplemented `ExtendedSearch` trait and disconnected
  `SearchResult` scoring container; `Cache::search` continues to return matching
  entries directly.
- Removed the placeholder `CacheStats::memory_usage_bytes` field.
- Removed the unused `EntryStatistics` container. `CacheStats` does not aggregate
  metadata execution-time or size accessors.
- Removed the unused `CacheKeySer` and `CacheValueSer` marker traits. Use the
  concrete `StorageBackend` bounds when implementing a backend.
- Removed the blanket `From<serde_json::Error> for CacheError` implementation.
  Map application serialization and deserialization failures explicitly to the
  appropriate `CacheError` variant or to the application's own error type.
- A zero entry limit or zero enabled-persistence sync interval now returns
  `InvalidConfiguration`.
- `default_ttl` is now applied to `put` and to entries without an explicit TTL.
- Direct reads exclude expired entries, and inserts that would grow a full cache
  return `CapacityExceeded` when eviction is disabled.

## Persistence expectations

Read [`PERSISTENCE.md`](PERSISTENCE.md). In particular:

- backend snapshots remain a cache recovery mechanism, not a durable system of
  record;
- `flush` is required at a controlled shutdown;
- a cache directory must not be shared by concurrent backend instances; and
- on-disk data must be treated as untrusted unless directory integrity is
  controlled.

## Validation

Test the application with the same feature selection used in production:

```bash
cargo update -p threatflux-cache
cargo test
cargo tree -e features -i threatflux-cache
```

Exercise a cold start, a restart with a valid JSON snapshot, and recovery from a
missing or invalid snapshot. Confirm that cache misses repopulate safely.
