# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-08-03

### Added

- Explicit behavior, feature, persistence, migration, testing, security, and
  release documentation.

### Changed

- Prepared the 0.2.0 API and package metadata around the crate's supported cache
  and JSON snapshot functionality.
- Replaced per-key persistence files with a bounded, versioned JSON snapshot and
  atomic replacement on supported filesystems.
- Made startup and scheduled persistence errors visible, added explicit async
  `flush`, applied default TTLs, and consistently excluded expired reads.
- Added configuration validation and made capacity exhaustion explicit when
  eviction is disabled.
- Hardened CI, documentation, security, and release automation with pinned
  actions, least-privilege permissions, and reproducible package checks.
- Raised the minimum supported Rust version to 1.95.0.

### Removed

- Removed the nonfunctional `compression`, `openapi`, `metrics`, and `tracing`
  feature surfaces.
- Removed the standalone `json-serialization` feature; `filesystem-backend` now
  enables its JSON implementation directly.
- Removed Bincode snapshot support. Existing Bincode files are not readable by
  0.2.0; migrate or repopulate them before upgrading.
- Removed the 0.1 per-key JSON layout. Version 0.2 requires a clean directory or
  application-level export and repopulation.
- Removed `PersistenceConfig` paths and unreliable drop-time saving; backend
  construction selects the path and `Cache::flush` defines graceful shutdown.
- Removed the single-format `SerializationFormat` wrapper and unused
  `StorageStats`; custom backends now implement `size_bytes` explicitly.
- Removed the unused `EntryStatistics`, `CacheKeySer`, and `CacheValueSer`
  surfaces and the blanket `From<serde_json::Error> for CacheError` conversion.
- Removed unimplemented extended-search and relevance-scoring placeholders.
- Removed obsolete repository bootstrap scripts and agent-specific notes.

See [`docs/MIGRATING_TO_0.2.md`](docs/MIGRATING_TO_0.2.md) for upgrade guidance.

## [0.1.8] - 2025-08-14

### Changed

- Standardized CI, documentation, examples, dependency policy, and repository
  metadata.
- Declared the Rust version supported by the published package.
- Changed the package license to MIT.

### Fixed

- Corrected feature combinations and generic type usage in examples and tests.
- Improved filesystem filename sanitization and test coverage.

[Unreleased]: https://github.com/ThreatFlux/threatflux-cache/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ThreatFlux/threatflux-cache/compare/v0.1.8...v0.2.0
[0.1.8]: https://github.com/ThreatFlux/threatflux-cache/releases/tag/v0.1.8
