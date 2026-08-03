# Persistence and durability

`FilesystemBackend` provides restart-oriented snapshots for an in-process
cache. It is not a database, write-ahead log, distributed cache, encrypted
store, or cross-process coordination service.

## Enabling persistence

Construct the backend with its directory and enable persistence in the cache
configuration:

```rust
use threatflux_cache::{Cache, CacheConfig, FilesystemBackend, PersistenceConfig};

# async fn example() -> threatflux_cache::Result<()> {
let backend = FilesystemBackend::<String, String>::new("./cache-data").await?;
let config = CacheConfig::default()
    .with_persistence(PersistenceConfig::enabled());
let cache = Cache::new(config, backend).await?;
# let _ = cache;
# Ok(())
# }
```

The path belongs to `FilesystemBackend`; `PersistenceConfig` controls when the
cache loads and saves. With `load_on_startup`, construction loads `cache.json`
and returns an error if the snapshot is invalid or unsupported.

`sync_interval` defaults to 100. Successful `put` and `add_entry` operations, and
a `remove` that finds a key, advance this counter. The operation that reaches the
interval waits for a full snapshot write and reports a write failure; earlier
`put` and `add_entry` calls update only the in-memory working set. Set the
interval to match the application's acceptable recovery window; zero is invalid
when persistence is enabled.

`remove` and `clear` also invoke their backend operations immediately instead of
waiting only for the interval. All cache mutations update memory first. If an
immediate or scheduled backend operation fails, the in-memory mutation remains
applied and the method returns the error. After correcting the storage failure,
call `flush` to reconcile the backend with the current in-memory state.

Call `cache.flush().await?` after an important batch and during controlled
shutdown. It serializes with cache mutations, writes the current state, and waits
for the backend to finish. There is no implicit drop-time save.

## Snapshot format and limits

Version 0.2 stores the full cache in one JSON file named `cache.json`. Its
envelope has `"version": 1` and an `entries` array. Each item in `entries` is a
two-element `[key, entry_history]` pair, allowing any supported serializable key
type. A replacement snapshot removes keys that are no longer present.

The filesystem backend rejects:

- snapshots larger than 64 MiB by default (configurable with
  `with_max_snapshot_bytes`);
- more than 100,000 keys or 1,000,000 entries;
- duplicate keys or entries whose embedded key does not match its map key;
- unknown format versions, malformed JSON, symlinks, and non-regular snapshot
  files; and
- the 0.1 per-key JSON and Bincode layouts.

The byte limit includes the JSON envelope. A zero-byte limit therefore rejects
every snapshot write, including a logically empty cache.

`FilesystemBackend::load` validates and returns the decoded snapshot. During
`Cache` construction, startup loading additionally removes expired entries and
trims histories and total entries to the configured cache limits. Serialization
and snapshot replacement require temporary memory in addition to the live cache;
choose lower application limits for large keys or values.

## Durability boundary

On Unix, writes use a new mode-`0600` temporary file, flush and sync it, rename
it over the old snapshot, and sync the directory. The rename gives readers an
atomic old-or-new file on filesystems that implement the usual rename semantics.
On Windows, replacing an existing file requires a remove-and-rename fallback and
is not fully atomic.

Even after a successful `flush`, durability ultimately depends on the operating
system, filesystem, storage hardware, and deployment environment. A process
crash before the next scheduled save loses newer in-memory changes. A mutation
whose backend operation fails remains applied in memory and returns the storage
error; retry `flush` after addressing the failure.

Applications that cannot reconstruct cached values must store them in a durable
system of record. Design a cache-miss path that can safely repopulate snapshots.

## Compatibility

The snapshot has an explicit format version, but its typed JSON representation
can also become incompatible when application key, value, or metadata types
change. Keep one directory per cache and format version. Do not place unrelated
JSON files in the directory or edit `cache.json` in place.

Neither the 0.1 per-key JSON layout nor 0.1 Bincode files are readable by 0.2;
both are rejected as unsupported persistence formats. `FilesystemBackend::clear`
also refuses to delete legacy or unrelated `.json` and `.bin` files; it only
removes the current `cache.json` snapshot after validating the directory. Export,
archive, or remove legacy files explicitly before upgrading; see
[`MIGRATING_TO_0.2.md`](MIGRATING_TO_0.2.md).

## Trust and security

Use a real directory owned by the application account with least-privilege
permissions. The backend rejects a directory that is a symlink when it is
constructed and rejects a snapshot that is not a regular file. Snapshot
contents are neither encrypted nor authenticated: anyone who can read the
directory can read cached values, and anyone who can modify it may supply data
on the next load.

Do not persist secrets unless the application separately provides appropriate
encryption and key management. Do not use attacker-controlled shared
directories. Treat data from custom backends as untrusted at the application
boundary.

## Concurrency and operations

One backend instance serializes its own filesystem operations. Use a persistence
directory from only one backend instance and one process; there is no
cross-instance locking or transactional isolation.

Operational guidance:

- monitor snapshot size and propagated filesystem errors;
- exclude the directory from source control and container image layers;
- call `flush` before a graceful shutdown or planned restart;
- exercise cold-start, restart, full-disk, and corrupt-snapshot recovery; and
- migrate or clear snapshots before incompatible application type changes.
