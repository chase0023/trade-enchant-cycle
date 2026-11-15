#!/usr/bin/env python3
"""Check dist/trade_enchant_cycle-<version>.hnmod and build the source ZIP.

The `.hnmod` is not written here. It is written by the registry's own tool, so
that the bytes we publish are the bytes the registry builds and hashes:

    python3 native/tools/native.py package trade_enchant_cycle

run from a clone of https://github.com/horizon-rust/horizon-mods, with this
repository copied to `native/trade_enchant_cycle/`. That tool writes

    horizon-native.json
    mixins/<id>/LICENSE
    mixins/<id>/horizon-mixin.toml
    mixins/<id>/*.rs

This script then proves the file really is made from `mixins/<id>/` here: the
manifest's id/version/pins must match, and every payload must be byte-identical
to the source next to it. Only then does it build the project ZIP, which is the
whole source tree with the `.hnmod` inside it. Nothing is downloaded.

    python scripts/package.py
"""

from __future__ import annotations

import hashlib
import json
import re
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ID = "trade_enchant_cycle"
BUNDLE = ROOT / "mixins" / ID
COMPAT = ROOT / "compatibility.json"
DIST = ROOT / "dist"
PROJECT_NAME = "trade-enchant-cycle"

VERSION = "1.1.0"
GAME = "26.3"
HORIZON = "9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0"
MIXINS = "0d6b103c70633d8fd9443d8d0acd3aadaacedb06"
HOOKS = 80

# `horizon-mixins` writes this next to the manifest when it plans a build; it is
# a cache of one run, not part of the source.
SKIP_FILES = {".plan.json.horizon-mixins-plan.lock"}


def manifest() -> dict:
    text = (BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8")
    found = {
        "id": re.search(r'^id\s*=\s*"([^"]+)"', text, re.MULTILINE),
        "version": re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE),
        "horizon": re.search(r'^horizon_revision\s*=\s*"([0-9a-f]{40})"', text, re.MULTILINE),
    }
    missing = [name for name, match in found.items() if match is None]
    if missing:
        raise SystemExit(f"mixins/{ID}/horizon-mixin.toml is missing {', '.join(missing)}")
    if found["id"].group(1) != ID or found["version"].group(1) != VERSION:
        raise SystemExit(
            f"package.py constants ({ID} {VERSION}) differ from the manifest "
            f"({found['id'].group(1)} {found['version'].group(1)})"
        )
    if found["horizon"].group(1) != HORIZON:
        raise SystemExit("package.py HORIZON differs from horizon-mixin.toml; update both")
    return {"id": ID, "version": VERSION, "game": GAME, "horizon": HORIZON, "mixins": MIXINS, "hooks": HOOKS}


def payloads() -> dict[str, bytes]:
    """Every file the bundle ships: its `.rs` and `.toml`, and its LICENSE."""
    files = {p.name: p.read_bytes() for p in sorted(BUNDLE.iterdir()) if p.is_file() and p.suffix in {".rs", ".toml"}}
    if "horizon-mixin.toml" not in files:
        raise SystemExit(f"mixins/{ID}/horizon-mixin.toml is missing")
    for name, raw in files.items():
        head = raw.decode("utf-8").lstrip()
        # horizon-mixins rejects a payload that starts with a file attribute or
        # an inner doc comment (`//!` counts as one).
        if head.startswith("#!") or head.startswith("//!"):
            raise SystemExit(f"{name} starts with a file attribute or `//!`")
    text = files["horizon-mixin.toml"].decode("utf-8")
    for name in re.findall(r'^(?:source|helper)\s*=\s*"([^"]+)"', text, re.MULTILINE):
        if name not in files:
            raise SystemExit(f"the manifest names {name}, which is not in mixins/{ID}/")
    licence = ROOT / "LICENSE"
    if licence.is_file():
        files["LICENSE"] = licence.read_bytes()
    return files


def verify_hnmod(meta: dict, files: dict[str, bytes]) -> Path:
    hnmod = DIST / f"{ID}-{VERSION}.hnmod"
    if not hnmod.is_file():
        raise SystemExit(
            f"{hnmod.relative_to(ROOT)} is missing: build it with the registry's tool "
            "(native/tools/native.py package) and copy it into dist/"
        )
    with zipfile.ZipFile(hnmod) as archive:
        shipped = {name: archive.read(name) for name in archive.namelist()}
    recorded = json.loads(shipped.pop("horizon-native.json"))
    expected = {"schema": 1, **meta}
    if recorded != expected:
        raise SystemExit(f"horizon-native.json says {recorded}, expected {expected}")
    if sorted(shipped) != sorted(f"mixins/{ID}/{name}" for name in files):
        extra = sorted(set(shipped) - {f"mixins/{ID}/{name}" for name in files})
        gone = sorted({f"mixins/{ID}/{name}" for name in files} - set(shipped))
        raise SystemExit(f"the .hnmod does not hold these files (extra {extra}, missing {gone})")
    for name, raw in files.items():
        if shipped[f"mixins/{ID}/{name}"] != raw:
            raise SystemExit(f"mixins/{ID}/{name} differs from the copy inside {hnmod.name}")
    return hnmod


def write_project_zip(hnmod: Path) -> Path:
    out = DIST / f"{PROJECT_NAME}-{VERSION}.zip"
    skip_dirs = {"target", ".git", "__pycache__"}
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for path in sorted(ROOT.rglob("*")):
            rel = path.relative_to(ROOT)
            if any(part in skip_dirs for part in rel.parts):
                continue
            if path.is_dir() or path == out or path.suffix == ".log" or path.name in SKIP_FILES:
                continue
            z.write(path, f"{PROJECT_NAME}/{rel.as_posix()}")
        entry = f"{PROJECT_NAME}/{hnmod.relative_to(ROOT).as_posix()}"
        if entry not in z.namelist():
            z.write(hnmod, entry)
    return out


def main() -> None:
    meta = manifest()
    files = payloads()
    hnmod = verify_hnmod(meta, files)
    project = write_project_zip(hnmod)
    digest = hashlib.sha256(hnmod.read_bytes()).hexdigest()
    print(f"hnmod:   {hnmod.relative_to(ROOT).as_posix()} ({hnmod.stat().st_size} bytes)")
    print(f"sha256:  {digest}")
    print(f"project: {project.relative_to(ROOT).as_posix()} ({project.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
