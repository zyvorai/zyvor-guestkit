#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""Check that install snippets in the docs name the version in Cargo.toml.

Scans README.md, SECURITY.md, deploy/README.md, integration/README.md and every Markdown
file under docs/ for places that mean "the current release":

  * GitHub Release URLs        zyvor-guestkit/releases/download/vX.Y.Z/...  and  .../releases/tag/vX.Y.Z
  * release tarball names      guestkit-X.Y.Z-linux-amd64[-musl].tar.gz
  * GHCR image tags            ghcr.io/zyvorai/<image>:vX.Y.Z  (and guestkit-mcp:X.Y.Z)
  * pinned tags in env vars    TAG=vX.Y.Z / IMAGE_TAG=vX.Y.Z

Changelogs and the roadmap are historical and skipped. This does not change any manifest: it
only reports docs that lag behind Cargo.toml (compare scripts/check-mcp-registry.sh, which keeps
Cargo.toml, server.json and the tag in step).

Usage: scripts/check-doc-versions.py          (exit 1 when a doc names another version)
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKIP = {
    "CHANGELOG.md",
    "docs/development/CHANGELOG.md",
    "docs/development/roadmap.md",
}
V = r"v?(\d+\.\d+\.\d+)"
PATTERNS = [
    re.compile(r"zyvor-guestkit/releases/download/v(\d+\.\d+\.\d+)/"),
    re.compile(r"zyvor-guestkit/releases/tag/v(\d+\.\d+\.\d+)"),
    re.compile(r"guestkit-(\d+\.\d+\.\d+)-linux-amd64"),
    re.compile(r"ghcr\.io/zyvorai/[a-z-]+:" + V),
    re.compile(r"\b(?:IMAGE_)?TAG=v(\d+\.\d+\.\d+)(?!-enterprise)"),
]


def cargo_version():
    for line in (ROOT / "Cargo.toml").read_text().splitlines():
        m = re.match(r'version\s*=\s*"([^"]+)"', line)
        if m:
            return m.group(1)
    sys.exit("check-doc-versions: no version in Cargo.toml")


def files():
    out = [ROOT / n for n in ("README.md", "SECURITY.md", "deploy/README.md", "integration/README.md")]
    out += sorted((ROOT / "docs").rglob("*.md"))
    return [f for f in out if f.is_file() and str(f.relative_to(ROOT)) not in SKIP]


def main():
    want = cargo_version()
    bad = 0
    for f in files():
        for n, line in enumerate(f.read_text(errors="ignore").splitlines(), 1):
            for pat in PATTERNS:
                for m in pat.finditer(line):
                    if m.group(1) != want:
                        print(f"{f.relative_to(ROOT)}:{n}: names {m.group(1)}, Cargo.toml is {want}")
                        bad += 1
    if bad:
        print(f"check-doc-versions: {bad} stale reference(s)", file=sys.stderr)
        return 1
    print(f"check-doc-versions: OK ({want})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
