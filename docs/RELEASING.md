# Releasing

This checklist is for ThreatFlux Cache maintainers. Releases should be produced
from a clean, protected `main` branch through the repository's release workflow.

## Prepare

1. Decide the version from the public API, behavior, MSRV, feature, and snapshot
   compatibility changes.
2. Update `Cargo.toml`, README installation snippets, and any migration guide.
   Move the release notes from `Unreleased` into a dated, versioned section in
   `CHANGELOG.md` and update its comparison links.
3. Confirm the package license, repository URL, description, categories, and
   include/exclude set.
4. Run the full matrix in [`../TESTING.md`](../TESTING.md).
5. Compare the public API with the previous release:

   ```bash
   cargo semver-checks check-release --all-features
   ```

6. Inspect exactly what crates.io will receive:

   ```bash
   cargo package --list --locked
   cargo package --locked
   ```

The package must not contain cache snapshots, credentials, coverage output,
generated documentation, or repository scratch files.

## Publish

1. Merge the release change to `main` and wait for required checks.
2. Create an annotated `vX.Y.Z` tag from the verified `main` commit.
3. Push the tag and let the protected release workflow validate and publish it.
   After crates.io publication succeeds, the workflow creates the GitHub release
   with GitHub-generated release notes.
4. Verify that the generated notes accurately reflect the curated changelog and
   edit the GitHub release when important compatibility or migration context is
   missing.

Publishing credentials should use crates.io trusted publishing or a short-lived
token scoped to this crate and a protected GitHub environment. Never place a
long-lived token in repository files or logs.

## Verify

- Confirm the version and owners on crates.io.
- Build the published crate in a fresh project using default and no-default
  features.
- Confirm docs.rs built the public documentation.
- Verify the GitHub release artifacts and checksums, if any.
- Confirm the changelog comparison link points at the new tag.

If publication fails after crates.io accepts a version, do not delete or reuse
that version. Fix forward with a new patch release and document the incident.
