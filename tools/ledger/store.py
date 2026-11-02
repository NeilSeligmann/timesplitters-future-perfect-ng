# SPDX-License-Identifier: GPL-3.0-or-later
"""Paginated, deterministic ledger snapshot: index.json plus ledger-NNNN.jsonl pages."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from pathlib import Path

from tools.ledger.model import pages

SCHEMA = 1


def dumps(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def write_snapshot(
    out_dir: Path, rows: Sequence[dict[str, object]], *, page_size: int, pins: Mapping[str, str]
) -> dict[str, object]:
    out_dir.mkdir(parents=True, exist_ok=True)
    for stale in out_dir.glob("ledger-*.jsonl"):
        stale.unlink()
    listing = []
    for number, chunk in enumerate(pages(rows, page_size), start=1):
        text = "".join(dumps(row) + "\n" for row in chunk)
        name = f"ledger-{number:04d}.jsonl"
        (out_dir / name).write_text(text, encoding="utf-8")
        listing.append(
            {
                "file": name,
                "first": chunk[0]["va"],
                "last": chunk[-1]["va"],
                "functions": len(chunk),
                "sha256": hashlib.sha256(text.encode()).hexdigest(),
            }
        )
    index = {
        "schema": SCHEMA,
        "functions": len(rows),
        "page_size": page_size,
        "pages": listing,
        "pins": dict(pins),
    }
    (out_dir / "index.json").write_text(
        json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return index


def read_snapshot(path: Path) -> dict[str, dict[str, object]]:
    """va -> row. `path` is a snapshot directory, an index.json, or one .jsonl file."""
    files: list[Path]
    if path.is_dir():
        path = path / "index.json"
    if path.name == "index.json":
        index = json.loads(path.read_text(encoding="utf-8"))
        files = [path.parent / page["file"] for page in index["pages"]]
        for page, file in zip(index["pages"], files, strict=True):
            digest = hashlib.sha256(file.read_bytes()).hexdigest()
            if digest != page["sha256"]:
                raise ValueError(f"{file}: sha256 does not match index")
    else:
        files = [path]
    rows: dict[str, dict[str, object]] = {}
    for file in files:
        for line in file.read_text(encoding="utf-8").splitlines():
            if line:
                row = json.loads(line)
                rows[str(row["va"])] = row
    return rows
