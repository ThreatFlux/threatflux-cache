# Cache behavior

This document defines the observable behavior of ThreatFlux Cache 0.2. It is a
guide to the implementation, not a substitute for the Rust API documentation.

## Data model

The cache stores a map from each key to a chronological vector of
`CacheEntry<K, V, M>` values. Each entry records creation time, optional expiry,
access count, last-access time, and application metadata.

- `AsyncCache::put` removes all existing versions for a key and inserts one new
  entry with default metadata.
- `Cache::add_entry` inserts a version in creation-timestamp order and retains
  the newest `max_entries_per_key` versions.
- `AsyncCache::get` returns the non-expired entry with the newest creation
  timestamp.
- `Cache::get_entries` returns every retained, non-expired version for a key.
- `AsyncCache::remove` removes the key and returns its newest retained value.
- `AsyncCache::len` counts non-expired entries, not unique keys.

Keys, values, and metadata are cloned when returned. Large objects therefore
have a cost beyond the map itself.

## Access tracking

`get`/`get_latest` increments the selected entry's access count.
`get_entries` increments every returned entry. `contains`, `search`, and
statistics do not record an access.

`Cache::get_stats` describes all entries still stored, including expired entries
that a read has not removed; `expired_count` identifies that subset and
`total_keys` reports stored unique keys. Statistics do not estimate heap usage or
aggregate the execution-time and size accessors provided by `EntryMetadata`.

## Capacity and eviction

`max_entries_per_key` is enforced when `add_entry` inserts a history item.
`put` replaces the key and then participates in global capacity enforcement.
Each insertion first reclaims all expired entries. `max_total_entries` then
selects the configured eviction strategy when an insertion crosses the limit.

LRU, LFU, and FIFO select a key and remove its complete history. TTL eviction
first removes expired entries and falls back to FIFO if the cache is still over
capacity. With `EvictionPolicy::None`, an insertion that would grow a full cache
returns `CacheError::CapacityExceeded`.

Both entry limits must be greater than zero. `Cache::new` rejects invalid
configuration instead of constructing a cache with ambiguous capacity behavior.

Selection among exact ties follows the map's internal iteration order and is not
a stable cross-process ordering. Applications that require deterministic victim
selection should provide distinct timestamps/access counts or a custom design.

## Expiration

An entry is expired when the current UTC time reaches its `expiry` field. Set
per-entry expiration with `CacheEntry::with_ttl`. `CacheConfig::default_ttl` is
applied by both `put` and `add_entry` when the new entry does not already have an
explicit expiry.

`get` and `get_entries` discard expired entries they encounter. Every insertion
also reclaims expired entries across the cache before enforcing capacity.
`contains` and `len` exclude expired entries without removing them. Search
excludes expired entries unless `SearchQuery::include_expired(true)` is set.
There is no background expiry sweeper.

## Search

The built-in `SearchQuery` combines all configured filters with logical AND:

- a case-sensitive substring of `Display(key)`;
- inclusive creation-timestamp bounds;
- inclusive access-count bounds;
- an exact, case-sensitive `EntryMetadata::category`; and
- expiration inclusion.

Search does not inspect value contents or metadata tags. Results are cloned and
their order follows the internal map iteration order; callers should sort when
stable ordering matters.

## Concurrency

Clones of a `Cache` share entries, backend, counters, and synchronization state.
Mutations and `flush` are serialized across those clones. Operations are safe to
call concurrently from Tokio tasks, but the cache does not provide
multi-operation transactions: a sequence such as `contains` followed by `get`
can observe intervening writes.

The process-level guarantees do not extend to multiple cache instances or
processes sharing a filesystem directory. See [`PERSISTENCE.md`](PERSISTENCE.md).

## Persistence boundary

With persistence enabled, each successful `put` or `add_entry`, plus a `remove`
that finds a key, advances the sync counter. Reaching `sync_interval` triggers an
awaited full backend save. `remove` and `clear` also invoke the corresponding
backend operations immediately. Every mutation is applied to memory first, so a
returned storage error does not roll the in-memory mutation back. `Cache::flush`
explicitly saves the current state, reconciles the backend after a prior failure,
and waits for the operation; call it during controlled shutdown.
