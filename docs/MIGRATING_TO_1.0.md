# Migrate to ThreatFlux Cache 1.0

Cache entry timestamps and search bounds use `threatflux_cache::Timestamp`.
Per-entry TTLs and `CacheEntry::age` use `threatflux_cache::SignedDuration`
from the `time` crate. Default TTLs still take `std::time::Duration`.

```rust
use threatflux_cache::{CacheEntry, SearchQuery, SignedDuration, Timestamp};

let start: Timestamp = "2026-01-01T00:00:00.123456789Z".parse()?;
let recent = SearchQuery::new().with_timestamp_range(Some(start), None);
let entry = CacheEntry::<_, _, ()>::new("key", "value")
    .with_ttl(SignedDuration::minutes(5));
# Ok::<(), threatflux_cache::ParseTimestampError>(())
```

To convert an existing Chrono value, parse its UTC or offset timestamp string.
This route preserves leap seconds and nanosecond precision. For ordinary Unix
timestamps, use `Timestamp::from_unix_timestamp_nanos`. Use
`SignedDuration::new(seconds, nanoseconds)` for a signed duration with
subsecond precision.

Version 1 filesystem snapshots retain their UTC timestamp strings and can be
loaded directly. See [persistence compatibility](PERSISTENCE.md#compatibility).
