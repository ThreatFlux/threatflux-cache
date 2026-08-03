# Contributing

Thank you for improving ThreatFlux Cache. Bug reports, documentation fixes,
tests, and focused implementation changes are welcome.

By participating, you agree to follow the
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). Report security issues privately as
described in [`SECURITY.md`](SECURITY.md).

## Before opening an issue

- Search existing issues and pull requests.
- Confirm the behavior on the latest release or `main`.
- Reduce bugs to a small reproducible example when possible.
- Include the Rust version, crate version, enabled features, backend, and
  platform.

Do not include secrets, private cache contents, or sensitive persisted files in
a public report.

## Development workflow

1. Fork the repository and create a branch from `main`.
2. Make one focused change with tests and documentation.
3. Run the checks in [`TESTING.md`](TESTING.md).
4. Review the diff for generated files, credentials, and unrelated edits.
5. Open a pull request explaining the problem, approach, compatibility impact,
   and validation performed.

Use clear commit messages written in the imperative mood. Keep commits small
enough to review independently; maintainers may squash them when merging.

## Pull-request checklist

- [ ] Public behavior and compatibility impact are documented.
- [ ] Tests cover new behavior and regressions.
- [ ] Default, no-default, and all-feature configurations pass.
- [ ] Formatting, Clippy, rustdoc, audit, and dependency-policy checks pass.
- [ ] Snapshot-format changes include migration guidance.
- [ ] No generated build output or sensitive cache data is included.

## API and persistence changes

This crate is pre-1.0, but compatibility still matters. Call out removed or
renamed public items, changed trait bounds, changed defaults, and altered error
behavior. Use `cargo-semver-checks` when a release baseline is available.

Any change to files written by `FilesystemBackend` needs:

- compatibility or explicit migration behavior;
- malformed-input and interrupted-write tests;
- documented resource and trust boundaries; and
- consideration of concurrent processes and filesystem permissions.

## Review

Maintainers may request changes for correctness, safety, API consistency,
documentation, tests, or release compatibility. A pull request may be closed if
it becomes inactive or diverges from project scope; useful work can always be
reopened in a smaller follow-up.
