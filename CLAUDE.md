# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ThreatFlux Cache is an async-first Rust cache library with pluggable backends, multiple eviction policies, and advanced search capabilities. It provides a flexible caching solution for Rust applications with support for different storage backends (memory, filesystem) and serialization formats (JSON, bincode).

## Essential Commands

### Build and Development
```bash
# Quick build
cargo build                     # Debug build
cargo build --release          # Release build
cargo build --all-features     # Build with all features enabled

# Run using Makefile shortcuts
make build                     # Debug build
make release                   # Release build
make all                       # Format, lint, test, and build
```

### Code Quality and Testing
```bash
# Formatting and linting
cargo fmt                      # Format code
cargo fmt --check             # Check formatting without changes
cargo clippy -- -D warnings   # Run clippy with strict warnings
make fmt                      # Format via Makefile
make clippy                   # Lint via Makefile

# Testing
cargo test                    # Run all tests
cargo test --all-features    # Test with all features
cargo test --lib             # Run library tests only
cargo test --doc             # Run doc tests
cargo test <test_name>       # Run specific test
make test                    # Test via Makefile

# Documentation
cargo doc --no-deps --open   # Generate and open documentation
make doc                     # Generate docs via Makefile
```

### Security and Compliance
```bash
# Security checks (must be installed first via setup-dev-tools.sh)
cargo audit                   # Check for security vulnerabilities
cargo deny check             # Validate licenses and dependencies

# Pre-commit checks
make pre-commit              # Run fmt + clippy + test
./setup-dev-tools.sh         # Install all dev tools and pre-commit hooks
```

### Examples
```bash
# Run specific examples
cargo run --example basic_usage
cargo run --example custom_entry
cargo run --example file_scanner_migration
cargo run --example simple_test

# Run all examples
make examples
```

## Architecture Overview

### Core Components

1. **Cache Module** (`src/cache.rs`)
   - Main `Cache<K, V>` struct implementing `AsyncCache` trait
   - Thread-safe concurrent access using `Arc<RwLock<HashMap>>`
   - Manages entries, eviction, and persistence

2. **Storage Backends** (`src/backends/`)
   - `StorageBackend` trait defines the interface for all backends
   - `MemoryBackend`: In-memory storage (default)
   - `FilesystemBackend`: Persistent storage to disk (requires feature flag)
   - Custom backends can be implemented by users

3. **Eviction Policies** (`src/eviction.rs`)
   - LRU (Least Recently Used)
   - LFU (Least Frequently Used) 
   - FIFO (First In First Out)
   - TTL (Time To Live)
   - Manual only

4. **Entry System** (`src/entry.rs`)
   - `CacheEntry<K, V, M>` wraps values with metadata
   - `EntryMetadata` trait for custom metadata
   - Built-in tracking: access count, last access time, creation time

5. **Search Capabilities** (`src/search.rs`)
   - `SearchQuery` for building complex queries
   - `Searchable` trait for making entries searchable
   - Pattern matching, category filtering, access count ranges

6. **Configuration** (`src/config.rs`)
   - `CacheConfig` for cache behavior settings
   - `PersistenceConfig` for storage configuration
   - Compression settings (when feature enabled)

### Key Design Patterns

- **Async-First**: All operations are async using tokio
- **Generic Types**: Works with any `Serialize + DeserializeOwned` types
- **Feature Flags**: Optional functionality via Cargo features
- **Trait-Based**: Extensible through traits (StorageBackend, EntryMetadata, Searchable)

### Thread Safety

The cache uses `Arc<RwLock<HashMap>>` for thread-safe concurrent access:
- Multiple readers can access simultaneously
- Writers have exclusive access
- Batch operations minimize lock contention

### Persistence Strategy

When persistence is enabled:
- Periodic saves based on `sync_interval` 
- Save on drop if `save_on_drop` is true
- Load on startup if `load_on_startup` is true
- Throttled filesystem writes using semaphore

## Feature Flags

- `default`: filesystem-backend, json-serialization
- `filesystem-backend`: Filesystem storage support
- `json-serialization`: JSON format support  
- `bincode-serialization`: Bincode format support
- `compression`: Value compression support
- `openapi`: OpenAPI schema generation
- `metrics`: Prometheus metrics integration
- `tracing`: Tracing support
- `full`: All features enabled

## Testing Strategy

Tests are organized by module:
- Unit tests in each module file
- Integration tests for cross-module functionality
- Property-based tests using proptest for invariants
- Examples serve as integration tests

Run tests with different feature combinations to ensure compatibility.