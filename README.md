# ThreatFlux Cache
[![Crates.io](https://img.shields.io/crates/v/threatflux-cache.svg)](https://crates.io/crates/threatflux-cache)
[![Documentation](https://docs.rs/threatflux-cache/badge.svg)](https://docs.rs/threatflux-cache)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/ci.yml/badge.svg)](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/ci.yml)
[![Release](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/release.yml/badge.svg)](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/release.yml)
[![Security](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/security.yml/badge.svg)](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/security.yml)
[![Documentation](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/docs.yml/badge.svg)](https://github.com/ThreatFlux/threatflux-cache/actions/workflows/docs.yml)
[![codecov](https://codecov.io/gh/ThreatFlux/threatflux-cache/graph/badge.svg?token=DlAEOagOhp)](https://codecov.io/gh/ThreatFlux/threatflux-cache)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/8f07242350d84597af1af64edbc505fa)](https://app.codacy.com/gh/ThreatFlux/threatflux-cache/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)



A flexible, async-first cache library for Rust with pluggable backends, multiple eviction policies, and advanced search capabilities.

## Features

- **Async-first design**: Built on tokio for high-performance async operations
- **Generic key-value storage**: Works with any serializable types
- **Multiple backends**:
  - In-memory storage (default)
  - Filesystem persistence
  - Easy to add custom backends
- **Eviction policies**:
  - LRU (Least Recently Used)
  - LFU (Least Frequently Used)
  - FIFO (First In First Out)
  - TTL (Time To Live)
  - Manual only
- **Advanced features**:
  - Entry metadata and custom attributes
  - Search and query capabilities
  - Compression support
  - Metrics integration
  - Automatic persistence
  - Entry statistics and access tracking

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
threatflux-cache = "0.1.0"
```

### Feature Flags

- `default`: Enables filesystem backend and JSON serialization
- `filesystem-backend`: Filesystem storage support
- `json-serialization`: JSON format support
- `bincode-serialization`: Bincode format support
- `compression`: Compression support for stored values
- `openapi`: OpenAPI schema generation
- `metrics`: Prometheus metrics integration
- `tracing`: Tracing support
- `full`: All features enabled

## Quick Start

### Basic Usage

```rust
use threatflux_cache::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a cache with default configuration
    let cache: Cache<String, User> = Cache::with_config(CacheConfig::default()).await?;
    
    // Store a value
    let user = User { id: 1, name: "Alice".to_string() };
    cache.put("user:1".to_string(), user).await?;
    
    // Retrieve a value
    if let Some(user) = cache.get(&"user:1".to_string()).await? {
        println!("Found user: {}", user.name);
    }
    
    Ok(())
}
```

### With Filesystem Persistence

```rust
use threatflux_cache::prelude::*;

let config = CacheConfig::default()
    .with_persistence(PersistenceConfig::with_path("/tmp/my-cache"))
    .with_eviction_policy(EvictionPolicy::Lru);

let backend = FilesystemBackend::new("/tmp/my-cache").await?;
let cache: Cache<String, String> = Cache::new(config, backend).await?;
```

### Custom Metadata

```rust
use threatflux_cache::{CacheEntry, BasicMetadata};

let metadata = BasicMetadata {
    execution_time_ms: Some(100),
    size_bytes: Some(1024),
    category: Some("api-response".to_string()),
    tags: vec!["user".to_string(), "profile".to_string()],
};

let entry = CacheEntry::with_metadata(
    "key".to_string(),
    "value".to_string(),
    metadata,
);

cache.add_entry(entry).await?;
```

### Search Capabilities

```rust
use threatflux_cache::SearchQuery;

// Search by pattern and category
let query = SearchQuery::new()
    .with_pattern("user")
    .with_category("api-response")
    .with_access_count_range(Some(5), None);

let results = cache.search(&query).await;
for entry in results {
    println!("Found: {:?}", entry.value);
}
```

## Migration from file-scanner

If you're migrating from file-scanner's built-in cache, see the `examples/file_scanner_migration.rs` for a complete migration guide. The library provides an adapter pattern to maintain API compatibility while gaining the benefits of the new cache system.

## Configuration Options

```rust
let config = CacheConfig::default()
    // Capacity settings
    .with_max_entries_per_key(100)
    .with_max_total_entries(10_000)
    
    // Eviction policy
    .with_eviction_policy(EvictionPolicy::Lru)
    
    // Persistence
    .with_persistence(PersistenceConfig {
        enabled: true,
        path: Some("/var/cache/myapp".into()),
        sync_interval: 100,
        save_on_drop: true,
        load_on_startup: true,
    })
    
    // TTL for all entries
    .with_default_ttl(Duration::from_secs(3600))
    
    // Enable compression
    .with_compression(CompressionConfig {
        algorithm: CompressionAlgorithm::Gzip,
        level: 6,
        min_size: 1024,
    });
```

## Custom Storage Backend

Implement the `StorageBackend` trait to create custom storage solutions:

```rust
use async_trait::async_trait;
use threatflux_cache::{StorageBackend, CacheEntry, Result};

pub struct MyCustomBackend;

#[async_trait]
impl StorageBackend for MyCustomBackend {
    type Key = String;
    type Value = String;
    type Metadata = ();
    
    async fn save(&self, entries: &HashMap<Self::Key, Vec<CacheEntry<Self::Key, Self::Value, Self::Metadata>>>) -> Result<()> {
        // Implementation
        Ok(())
    }
    
    async fn load(&self) -> Result<HashMap<Self::Key, Vec<CacheEntry<Self::Key, Self::Value, Self::Metadata>>>> {
        // Implementation
        Ok(HashMap::new())
    }
    
    // ... other required methods
}
```

## Performance Considerations

- The cache uses `Arc<RwLock<HashMap>>` for thread-safe concurrent access
- Batch operations are preferred for bulk updates
- Filesystem backend saves are throttled using a semaphore
- Consider compression for large values to reduce I/O

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
