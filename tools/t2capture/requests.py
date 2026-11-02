"""T3877 prerequisite work consumed by live captures and the continuous checker.

One atomic file per VA/action avoids queue producer/consumer lost updates. Refresh
requests name the old receipt digest: a newly written receipt satisfies the job,
even if its verdict still needs a fix. Dispatch validates that new receipt again.
"""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
from pathlib import Path

from tools import datadir


def request_dir() -> Path:
    return datadir.data_dir("t2-prerequisites")


def digest(path: Path) -> str:
    if not path.is_file():
        return "missing"
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def atomic_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=".request-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(value, handle, sort_keys=True)
            handle.write("\n")
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def submit(
    action: str, va: int, item: str, reason: str, receipt: Path, directory: Path | None = None
) -> None:
    if action not in {"capture", "refresh"}:
        raise ValueError(action)
    atomic_json(
        (directory or request_dir()) / action / f"{va:08X}.json",
        {
            "va": f"0x{va:08X}",
            "item": item,
            "reason": reason,
            "receipt": str(receipt),
            "receipt_digest": digest(receipt),
        },
    )


def pending(action: str, directory: Path | None = None) -> set[int]:
    out = set()
    for path in sorted(((directory or request_dir()) / action).glob("*.json")):
        data = json.loads(path.read_text())
        if action == "refresh" and digest(Path(data["receipt"])) != data["receipt_digest"]:
            continue
        out.add(int(data["va"], 16))
    return out
