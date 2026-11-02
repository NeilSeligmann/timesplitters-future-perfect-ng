"""Attach bounded, invocation-specific stage evidence to prebuilt manifests."""

from __future__ import annotations

import json
import math
import os
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from tempfile import TemporaryDirectory

from tools.build_stages import Cursor, cursor

ENV = "TSFP_BUILD_STAGE_RECEIPT"
MAX_BYTES = 262144
MAX_ROWS = 64
STAGES = {"configure", "host", "cargo", "bundle"}
KINDS = {"lifted_c", "native_c", "rust_archive", "link_or_archive", "other"}


def number(value: object) -> bool:
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def stage_row(value: dict) -> dict:
    """Keep timing metadata only; parser errors and arbitrary payloads stay private."""
    if (
        type(value.get("schema")) is not int
        or value["schema"] != 1
        or value.get("stage") not in STAGES
        or not number(value.get("wall_seconds"))
        or type(value.get("exit")) is not int
    ):
        raise ValueError("invalid stage row")
    row = {key: value[key] for key in ("stage", "wall_seconds", "exit")}
    if "ninja" in value:
        ninja = value["ninja"]
        row["ninja"] = {"available": False}
        if ninja.get("available") is True:
            groups = ninja["groups"]
            clean = {}
            for kind, group in groups.items():
                if kind not in KINDS or not all(
                    number(group.get(key))
                    for key in ("outputs", "output_seconds", "first_ms", "last_ms")
                ):
                    raise ValueError("invalid Ninja summary")
                if type(group["outputs"]) is not int or group["last_ms"] < group["first_ms"]:
                    raise ValueError("invalid Ninja interval")
                clean[kind] = {
                    key: group[key] for key in ("outputs", "output_seconds", "first_ms", "last_ms")
                }
            row["ninja"] = {"available": True, "groups": clean}
    return row


def read(path: Path, before: Cursor | None) -> dict:
    try:
        with path.open("rb") as stream:
            stat = os.fstat(stream.fileno())
            if before is not None:
                if stat.st_ino != before.inode or stat.st_size < before.offset:
                    raise ValueError("receipt replaced")
                stream.seek(max(0, before.offset - 256))
                if stream.read(len(before.anchor)) != before.anchor:
                    raise ValueError("receipt rewritten")
                stream.seek(before.offset)
            raw = stream.read(MAX_BYTES + 1)
        if not raw:
            return {"available": False, "reason": "no stage rows from this invocation"}
        lines = raw.splitlines()
        if len(raw) > MAX_BYTES or len(lines) > MAX_ROWS or not raw.endswith(b"\n"):
            raise ValueError("receipt outside bounds")
        rows = [stage_row(json.loads(line)) for line in lines]
        return {
            "available": True,
            "rows": rows,
            "scope": "this build invocation; Cargo includes link; Ninja output durations overlap",
        }
    except (
        OSError,
        ValueError,
        TypeError,
        KeyError,
        AttributeError,
        OverflowError,
        RecursionError,
    ):
        return {"available": False, "reason": "missing or invalid stage receipt"}


@contextmanager
def recording(root: Path, env: dict[str, str]) -> Iterator[dict]:
    """Retain explicit receipts, remove owned scratch, and never recount older rows."""
    temporary_root = root / "tmp"
    temporary_root.mkdir(exist_ok=True)
    original = env.get(ENV)
    with TemporaryDirectory(prefix="prebuilt-stages-", dir=temporary_root) as directory:
        path = Path(original) if original else Path(directory) / "stages.jsonl"
        if not path.is_absolute():
            path = root / path
        path = path.resolve()
        before = cursor(path)
        env[ENV] = str(path)
        evidence: dict = {}
        try:
            yield evidence
        finally:
            evidence.update(read(path, before))
            if original is None:
                env.pop(ENV, None)
            else:
                env[ENV] = original
