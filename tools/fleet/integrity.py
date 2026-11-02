"""Host binary integrity guard (T3697): hash manifest of the interpreters and the Rust toolchain.

Why: on 2026-10-24 the owner PC's `python3.15` had two flipped bits (storage or RAM fault) and every
local pytest died with signal 11 until the binary was replaced (docs/incidents/I-T3697-1.md).

`record` hashes the project interpreters (`.uv-python/cpython-*/bin/python3.N`) and the pinned Rust
toolchain (`bin/*`, `lib/*.so` of the channel in `rust-toolchain.toml`) into a manifest from a
known-good state. `check` rehashes and reports changed and missing files. The manifest is never
overwritten by a check or by a repeated `record`: a changed file stays reported until
`record --accept <path>` (optionally `--expect <sha256>` from another host's copy). That is the
point: auto-accepting a changed hash would hide the flip.

Per host: the manifest and status live under `<root>/tmp/fleet/` of that host's checkout; the
controller runs `check` on remotes over ssh (job `integrity`) and `detect_integrity_changed`
(tools/fleet/detectors.py) turns the status files into one HEALTH line per host and file.

`python3 -m tools.fleet.integrity {record,check}`; stdlib only so remote hosts run it with any
python3.
"""

from __future__ import annotations

import argparse
import glob
import hashlib
import json
import os
import re
import socket
import sys
import time
from collections.abc import Sequence
from pathlib import Path

MANIFEST = "tmp/fleet/integrity-manifest.json"
STATUS = "tmp/fleet/integrity-status.json"
CHECK_EVERY_S = 3600
FALLBACK_CHANNEL = "1.99.1"


def toolchain_channel(root: Path) -> str:
    try:
        text = (root / "rust-toolchain.toml").read_text()
    except OSError:
        return FALLBACK_CHANNEL
    match = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.MULTILINE)
    return match.group(1) if match else FALLBACK_CHANNEL


def targets(root: Path) -> list[str]:
    """Root-relative paths of the files to guard that exist on this host (sorted, resolved)."""
    channel = toolchain_channel(root)
    patterns = (
        ".uv-python/cpython-*/bin/python3.*[0-9]",
        f"tmp/rust/rustup/toolchains/{channel}-*/bin/*",
        f"tmp/rust/rustup/toolchains/{channel}-*/lib/*.so",
    )
    found: set[str] = set()
    for pattern in patterns:
        for path in glob.glob(str(root / pattern)):
            if os.path.isfile(path):
                found.add(os.path.relpath(path, root))
    return sorted(found)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def hash_files(root: Path, relatives: Sequence[str]) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for relative in relatives:
        try:
            hashes[relative] = sha256(root / relative)
        except OSError:
            continue
    return hashes


def load_manifest(root: Path) -> dict[str, str] | None:
    try:
        data = json.loads((root / MANIFEST).read_text())
    except (OSError, ValueError):
        return None
    files = data.get("files") if isinstance(data, dict) else None
    return files if isinstance(files, dict) else None


def write_json(path: Path, data: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    scratch = path.with_name(path.name + f".{os.getpid()}.tmp")
    scratch.write_text(json.dumps(data, indent=1, sort_keys=True) + "\n")
    os.replace(scratch, path)


def record(root: Path, *, now: float | None = None) -> tuple[dict[str, str], list[str]]:
    """Baseline from the current state. Creates the manifest, or only ADDS files it has not seen;
    an existing entry is never touched (use `accept`). Returns (manifest, added paths)."""
    files = load_manifest(root) or {}
    added = []
    for relative, digest in hash_files(root, targets(root)).items():
        if relative not in files:
            files[relative] = digest
            added.append(relative)
    write_json(
        root / MANIFEST, {"files": files, "updated": now if now is not None else time.time()}
    )
    return files, added


def accept(root: Path, relative: str, *, expect: str | None = None) -> str:
    """Take the current hash of one file as the new baseline, after the owner verified it against
    another host's copy (`expect` = that copy's sha256; a mismatch refuses)."""
    files = load_manifest(root)
    if files is None or relative not in files:
        raise ValueError(f"{relative} is not in the manifest")
    digest = sha256(root / relative)
    if expect is not None and digest != expect:
        raise ValueError(f"{relative} has sha256 {digest}, not the expected {expect}")
    files[relative] = digest
    write_json(root / MANIFEST, {"files": files, "updated": time.time()})
    return digest


def check(root: Path, *, now: float | None = None, host: str | None = None) -> dict:
    """Rehash the manifest files. `changed` and `missing` stay reported until accepted. Writes the
    status file. `state` is `no-manifest`, `ok` or `changed`."""
    stamp = now if now is not None else time.time()
    name = host or socket.gethostname()
    files = load_manifest(root)
    if files is None:
        status = {
            "host": name,
            "checked": stamp,
            "state": "no-manifest",
            "changed": [],
            "missing": [],
        }
    else:
        now_hashes = hash_files(root, sorted(files))
        changed = sorted(p for p, h in now_hashes.items() if h != files[p])
        missing = sorted(p for p in files if p not in now_hashes)
        status = {
            "host": name,
            "checked": stamp,
            "state": "changed" if changed or missing else "ok",
            "changed": changed,
            "missing": missing,
            "files": len(files),
        }
    write_json(root / STATUS, status)
    return status


def cached_status(root: Path, now: float, max_age: float = CHECK_EVERY_S) -> dict:
    """The local status, rehashing only when the last check is older than `max_age`."""
    try:
        status = json.loads((root / STATUS).read_text())
        if now - float(status["checked"]) < max_age:
            return status
    except (OSError, ValueError, KeyError, TypeError):
        pass
    return check(root, now=now)


def format_findings(status: dict) -> list[tuple[str, str]]:
    """(file, detail) per changed or missing file of one status."""
    host = status.get("host", "?")
    out = [(p, f"{host} {p} sha256 differs from the manifest") for p in status.get("changed", [])]
    out += [(p, f"{host} {p} is missing") for p in status.get("missing", [])]
    return out


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.fleet.integrity", description=__doc__)
    parser.add_argument("--root", default=".", help="checkout root (default: cwd)")
    sub = parser.add_subparsers(dest="command", required=True)
    rec = sub.add_parser("record", help="baseline from a known-good state (adds new files only)")
    rec.add_argument("--accept", metavar="PATH", help="accept the current hash of one changed file")
    rec.add_argument(
        "--expect", metavar="SHA256", help="with --accept: hash of another host's copy"
    )
    chk = sub.add_parser("check", help="rehash and report; exit 1 when a file changed")
    chk.add_argument("--json", action="store_true", help="print the status as JSON")
    chk.add_argument("--init", action="store_true", help="record first when no manifest exists")
    chk.add_argument("--host", help="host name in the status (default: hostname)")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()
    if args.command == "record":
        if args.accept:
            try:
                digest = accept(root, args.accept, expect=args.expect)
            except ValueError as error:
                print(f"integrity: refused: {error}", file=sys.stderr)
                return 2
            print(f"integrity: accepted {args.accept} {digest}")
            return 0
        files, added = record(root)
        print(f"integrity: manifest has {len(files)} files, {len(added)} added")
        return 0
    if args.init and load_manifest(root) is None:
        record(root)
    status = check(root, host=args.host)
    if args.json:
        print(json.dumps(status))
    else:
        for _, detail in format_findings(status):
            print(f"integrity: {detail}")
        print(f"integrity: {status['state']}")
    return 1 if status["state"] == "changed" else 0


if __name__ == "__main__":
    sys.exit(main())
