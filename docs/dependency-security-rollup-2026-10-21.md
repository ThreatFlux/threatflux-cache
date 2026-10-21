# Stable dependency and security rollup

Reviewed 2026-10-21 against fetched main `5c3c124689e7eb1141ca87adc2f4f4f210713e2d`.

Rust builds and workflows now use stable **1.99.1**, verified against the [official stable channel manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml). The consumer MSRV remains **1.95.0** and has a separate compatibility check. Cargo.lock was refreshed within upstream compatibility requirements. No prerelease or yanked release was selected.

All direct and development crates were independently checked against the [crates.io sparse index](https://index.crates.io/config.json):

| Crate | Latest stable | Published MSRV |
| --- | --- | --- |
| async-trait | 0.1.92 | 1.71 |
| chrono | 0.4.45 | 1.62.0 |
| serde | 1.0.229 | 1.56 |
| serde_json | 1.0.154 | 1.71 |
| tempfile | 3.27.0 | 1.63 |
| thiserror | 2.0.21 | 1.77 |
| tokio | 1.54.0 | 1.85 |

All referenced Actions pins were checked against their authoritative GitHub tags; taiki-e/install-action advances to v2.87.29. Cargo tools were also checked against crates.io: cargo-semver-checks advances to 0.51.0 and cargo-llvm-cov to 0.9.1. Other Actions and tools already reference current stable releases. The project is a Rust library with no shipped Docker/native runtime; no container migration is applicable.

The paginated GitHub open-PR and open-code-scanning-alert inventories both returned zero entries. There are no findings to classify, fix or dismiss, and no dependency PRs to supersede. An empty alert inventory is not proof that all possible security analysis has run. Cargo audit/deny and the complete repository feature/test/MSRV matrix remain release gates.

Validation is recorded in the replacement PR. No Jira ticket, merge, deployment, CVE ignore or advisory suppression is introduced.
