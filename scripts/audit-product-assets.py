#!/usr/bin/env python3
"""Check the distributed product for imported branding and unreviewed assets."""

import hashlib
import json
from pathlib import Path
import re
import sys
from zipfile import ZipFile

root = Path(__file__).resolve().parent.parent
brand = re.compile(r"anthropic|claude", re.IGNORECASE)
# These are actual provider protocol, catalog, authentication, and regression
# identifiers. They are not application names or frontend artwork.
provider_files = {
    "crates/quixi-engine/src/providers.rs",
    "crates/quixi-engine/src/provider_tool_anthropic.rs",
    "crates/quixi-netstack/src/lib.rs",
}
provider_patterns = {
    "crates/quixi-engine/src/provider_tool_responses.rs": re.compile(r"anthropic_payload"),
    "crates/quixi-engine/src/mcp_integration_tests.rs": re.compile(r'"anthropic"|anthropic_'),
    "crates/quixi-engine/src/lib.rs": re.compile(
        r"claude-sonnet-4-6|anthropic-version"
    ),
    "crates/quixi-ipc/src/lib.rs": re.compile(
        r'"provider": "anthropic"|"signInWithAnthropicApi"|claude-(sonnet|opus)-'
    ),
    "resources/ui/app.js": re.compile(r'"anthropic", "Messages API"'),
}
errors = []
checked = 0
for directory in ("resources", "crates", "bins"):
    for path in (root / directory).rglob("*"):
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        checked += 1
        if path.suffix.lower() in {".ttf", ".otf", ".woff", ".woff2"}:
            errors.append(
                f"{relative}: redistribute no private font bundle; use system fonts"
            )
        if brand.search(relative) and relative not in provider_files:
            errors.append(f"{relative}: branded product filename")
        if path.suffix == ".skill":
            with ZipFile(path) as archive:
                for entry in archive.namelist():
                    if brand.search(
                        archive.read(entry).decode("utf-8", errors="replace")
                    ):
                        errors.append(f"{relative}:{entry}: unexpected skill branding")
            continue
        if relative in provider_files:
            continue
        raw = path.read_bytes()
        if b"\0" in raw:
            continue
        for number, line in enumerate(
            raw.decode("utf-8", errors="replace").splitlines(), 1
        ):
            if brand.search(line) and not provider_patterns.get(
                relative, re.compile(r"(?!)")
            ).search(line):
                errors.append(f"{relative}:{number}: unexpected product branding")
for name in ("ion-dist", "renderer", "preload", "fonts"):
    if (root / "resources" / name).exists():
        errors.append(f"resources/{name}: removed application bundle must not return")
libraries = root / "resources/artifact-libs"
for name, record in json.loads((libraries / "PROVENANCE.json").read_text()).items():
    if (
        record["license"] != "MIT"
        or not record.get("integrity")
        or not record.get("files")
    ):
        errors.append(f"{name}: incomplete upstream redistribution record")
    for name, expected in record["files"].items():
        path = libraries / name
        if (
            not path.is_file()
            or hashlib.sha256(path.read_bytes()).hexdigest() != expected
        ):
            errors.append(f"{path}: upstream file or license changed")
if errors:
    print("\n".join(errors), file=sys.stderr)
    raise SystemExit(1)
print(
    f"Product asset audit passed: {checked} files; remaining names are confined to provider integration."
)
