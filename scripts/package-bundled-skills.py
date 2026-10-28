#!/usr/bin/env python3
"""Package the checked-in QuixiDesk skill sources without external app assets."""

import json
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

resources = Path(__file__).resolve().parent.parent / "resources"
packages = resources / "bundled-skills"
manifest = json.loads((packages / "manifest.json").read_text())
for skill in manifest["skills"]:
    name = skill["name"]
    source = resources / "bundled-skills-extracted" / name
    with ZipFile(packages / f"{name}.skill", "w", compression=ZIP_DEFLATED) as archive:
        for path in sorted(source.rglob("*")):
            if path.is_file():
                info = ZipInfo(
                    f"{name}/{path.relative_to(source).as_posix()}",
                    (2026, 1, 1, 0, 0, 0),
                )
                info.compress_type = ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, path.read_bytes())
