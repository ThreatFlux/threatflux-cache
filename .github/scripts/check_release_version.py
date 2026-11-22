"""Reject an untagged manifest version too small for the commits it will ship."""

import re
import subprocess
import tomllib
from pathlib import Path

STABLE_VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
SUBJECT = re.compile(r"^([A-Za-z][A-Za-z0-9-]*)(?:\([^)]+\))?(!)?:\s?.+$")
BREAKING_FOOTER = re.compile(r"^BREAKING[- ]CHANGE\b", re.MULTILINE)


def git(*args):
    return subprocess.check_output(("git", *args), text=True, encoding="utf-8")


def version_tuple(version):
    match = STABLE_VERSION.fullmatch(version)
    if match is None:
        raise ValueError(f"Expected a stable release version, got {version!r}")
    return tuple(map(int, match.groups()))


def minimum_version(released, messages):
    major, minor, patch = released
    required = released
    for message in messages:
        subject = SUBJECT.match(message.partition("\n")[0].strip())
        if (subject and subject[2]) or BREAKING_FOOTER.search(message):
            return major + 1, 0, 0
        if subject and subject[1] == "feat":
            required = max(required, (major, minor + 1, 0))
        if subject and subject[1] == "fix":
            required = max(required, (major, minor, patch + 1))
    return required


def main():
    manifest = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
    current = version_tuple(manifest["package"]["version"])
    tags = git("tag", "--list", "v*").splitlines()
    tag = max(
        (tag for tag in tags if STABLE_VERSION.fullmatch(tag[1:])),
        key=lambda tag: version_tuple(tag[1:]),
    )
    released = version_tuple(tag[1:])
    if current <= released:
        return

    # Match the pinned release action's commit classification, excluding merge
    # commits while including the commits they brought into the release.
    messages = git("log", "--no-merges", "-z", "--format=%B", f"{tag}..HEAD").split("\0")
    required = minimum_version(released, messages)
    if current < required:
        version = ".".join(map(str, required))
        raise SystemExit(
            f"Refusing to tag {manifest['package']['version']}: commits since {tag} "
            f"require at least {version}. Merge a corrected version bump first."
        )


if __name__ == "__main__":
    main()
