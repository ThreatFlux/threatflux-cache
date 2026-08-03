# Feature flags

ThreatFlux Cache keeps optional functionality explicit so applications can
minimize their dependency graph.

| Feature              | Included by default | Enables                                            |
| -------------------- | :-----------------: | -------------------------------------------------- |
| `filesystem-backend` |         Yes         | Filesystem snapshot backend and JSON serialization |
| `full`               |         No          | Alias for all supported optional functionality     |

`default-features = false` provides the core cache and memory backend without a
filesystem serializer.

The filesystem feature owns its JSON implementation directly. The former
standalone `json-serialization` flag no longer exists because it did not provide
a usable snapshot backend by itself.

The 0.2 release also removes the former `bincode-serialization`, `compression`,
`openapi`, `metrics`, and `tracing` flags. They either exposed an unmaintained
snapshot format or did not affect cache operations. The `full` alias remains for
forward compatibility and currently selects `filesystem-backend`. See
[`MIGRATING_TO_0.2.md`](MIGRATING_TO_0.2.md).

## Supported combinations

```bash
# Memory-only core
cargo check --no-default-features --locked

# Default filesystem + JSON build
cargo check --locked

# Explicit filesystem + JSON build
cargo check --no-default-features --features filesystem-backend --locked
```

Every declared combination is expected to compile and pass its applicable
tests. The default generic `Cache<K, V>` uses `MemoryBackend`; enabling
`filesystem-backend` only makes `FilesystemBackend` available.

## Adding a feature

A new feature should:

- gate meaningful behavior, not only a dependency;
- work with `default-features = false` when practical;
- include tests for its independent and combined configurations;
- document its public API, operational cost, and compatibility contract; and
- avoid silently changing the behavior or format of an existing configuration.
