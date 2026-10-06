# Releasing

This checklist is for ThreatFlux Cache maintainers. Releases should be produced
from a clean, protected `main` branch through the repository's release workflow.

## Prepare

1. Decide the version from the public API, behavior, MSRV, feature, and snapshot
   compatibility changes.
2. Update README installation snippets and any migration guide. Move the
   release notes from `Unreleased` into a dated, versioned section in
   `CHANGELOG.md` and update its comparison links. Do not bump the `Cargo.toml`
   version by hand: `Auto Release` bumps it from the latest release.
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

Releases are cut by the `Auto Release` workflow
(`.github/workflows/auto-release.yml`) and published by the `Release` workflow
(`.github/workflows/release.yml`).

1. Merge the release change to `main` with a conventional commit subject and
   wait for the `CI` and `Security` workflows to pass on the merge commit.
   `fix:` commits produce a patch release, `feat:` commits a minor release, and
   breaking changes a major release; `chore:`, `ci:`, `docs:`, `build:`, and
   `test:` commits do not release.
2. `Auto Release` bumps `Cargo.toml` and `Cargo.lock`, commits
   `chore: release vX.Y.Z` to `main`, pushes the annotated `vX.Y.Z` tag, and
   creates the GitHub release. It authenticates as the ThreatFlux automation
   GitHub App, so the tag push starts `Release` directly.
3. `Release` checks that the tag is annotated, points at a commit on `main`,
   and matches the `Cargo.toml` version. It then tests and builds the crate on
   every supported target, packages it, generates a CycloneDX SBOM, publishes
   to crates.io, and attaches the `.crate` file and the SBOM to the GitHub
   release, each with a `.sha256` checksum file and a signed build provenance
   attestation. The attached `.crate` must be identical to the package
   crates.io serves, or the release fails. A version that is already on
   crates.io is skipped, so a rerun is safe.
4. Verify that the release notes accurately reflect the curated changelog and
   edit the GitHub release when important compatibility or migration context is
   missing. A `## [X.Y.Z]` section in `CHANGELOG.md` replaces the generated
   notes.

To rehearse a release without tagging or publishing anything, run either
workflow manually with `dry_run` enabled:

```bash
gh workflow run auto-release.yml -f version_bump=auto -f dry_run=true
gh workflow run release.yml --ref main -f dry_run=true
```

A `Release` dry run tests and builds every target, packages the crate,
generates the SBOM, and runs `cargo publish --dry-run`.

crates.io publishing uses
[trusted publishing](https://crates.io/docs/trusted-publishing): the `publish`
job runs in the `crates-io` environment and exchanges its GitHub OIDC token for
a short-lived crates.io token. No registry token is stored in the repository or
its secrets. Never place a long-lived token in repository files or logs.

## Verify

- Confirm the version and owners on crates.io.
- Build the published crate in a fresh project using default and no-default
  features.
- Confirm docs.rs built the public documentation.
- Verify the GitHub release assets, their checksums, and their provenance:

  ```bash
  gh release download vX.Y.Z --repo ThreatFlux/threatflux-cache --dir dist
  (cd dist && sha256sum --check ./*.sha256)
  gh attestation verify dist/threatflux-cache-X.Y.Z.crate --repo ThreatFlux/threatflux-cache
  ```

- Confirm the changelog comparison link points at the new tag.

If publication fails after crates.io accepts a version, do not delete or reuse
that version. Fix forward with a new patch release and document the incident.
