# Security Policy

## Supported versions

Security fixes are provided for the latest published minor release. Users should
upgrade to the newest patch before reporting a problem.

| Version              | Supported |
| -------------------- | :-------: |
| Latest `0.x` release |    Yes    |
| Older releases       |    No     |

## Reporting a vulnerability

Do not open a public issue or discussion for a suspected vulnerability.

Use GitHub's
[private vulnerability reporting](https://github.com/ThreatFlux/threatflux-cache/security/advisories/new).
If that is unavailable, email `security@threatflux.ai` with the repository name
in the subject.

Include, when possible:

- affected versions, features, and backend;
- impact and realistic attack conditions;
- a minimal reproducer or malformed snapshot;
- suggested mitigations; and
- whether the issue is already public.

Remove credentials, personal information, and unrelated production data. Encrypt
especially sensitive material before sending it and ask for a preferred key or
transfer method.

We aim to acknowledge reports within three business days. Validation,
remediation, disclosure timing, and credit are coordinated privately. Please
allow a reasonable remediation window before public disclosure.

## Security model

ThreatFlux Cache is a library, not an authorization boundary or encrypted data
store. Applications remain responsible for:

- authenticating and authorizing cache callers;
- choosing safe filesystem locations and permissions;
- protecting the confidentiality and integrity of cached values;
- bounding untrusted keys, values, metadata, and custom-backend data;
- coordinating access if more than one process uses a persistence location; and
- defining recovery when a best-effort snapshot cannot be read.

See [`docs/PERSISTENCE.md`](docs/PERSISTENCE.md) for the detailed persistence
threat model.

## Dependency disclosures

Reports that only repeat a dependency advisory should explain whether the
vulnerable code is reachable in this crate. Automated scanner output is useful,
but reachability and impact help maintainers prioritize the fix.
