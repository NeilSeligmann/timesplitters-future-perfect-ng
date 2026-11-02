# SPDX-License-Identifier: GPL-3.0-or-later
"""input.rec format facts for the Python tools, read from their owner (T3820, rule 45).

The writer and reader live in `crates/tsfp-input/src/recording.rs`; the format versions come
from its `FORMAT_VERSION` and `DEVICE_ROWS_VERSION` constants, so a format bump needs no edit
here.
Format: docs/t2348-input-recording.md.
"""

from __future__ import annotations

import functools
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/tsfp-input/src/recording.rs"
MAGIC = "TSFP-INPUT-REC"
SIMULATION_HZ = "60"
# Header fields any version may carry after the positional ones (T2421 saves binding, T3820 build).
STAMP_KEYS = ("saves=", "saves-sha256=", "build=")
# Session options a header records as `name=1|0` and the host flag that applies each one on replay.
# A recorded option missing here cannot be applied, so `session_host_args` refuses it.
SESSION_OPTION_FLAGS = {"unlock-all": "--unlock-all"}


@functools.cache
def _constant(name: str) -> int:
    match = re.search(rf"pub const {name}: u32 = (\d+);", SOURCE.read_text())
    if not match:
        raise ValueError(f"{SOURCE} has no {name}: the recording format owner moved")
    return int(match[1])


def format_version() -> int:
    return _constant("FORMAT_VERSION")


def device_rows_version() -> int:
    return _constant("DEVICE_ROWS_VERSION")


def parse_header(header: str) -> tuple[int, bool]:
    """(version, unlock_all) of a header line; ValueError when it is not a supported recording."""
    fields = [f for f in header.rstrip("\r\n").split("\t") if not f.startswith(STAMP_KEYS)]
    if len(fields) < 3 or fields[0] != MAGIC or fields[2] != SIMULATION_HZ:
        raise ValueError(f"unsupported native input route header {header!r}")
    try:
        version = int(fields[1])
    except ValueError as error:
        raise ValueError(f"unsupported native input route header {header!r}") from error
    if not 1 <= version <= format_version():
        raise ValueError(f"unsupported native input route header {header!r}")
    if version <= 2 and len(fields) == 3:
        return version, False
    if version >= 3 and fields[3:] in (["unlock-all=0"], ["unlock-all=1"]):
        return version, fields[3] == "unlock-all=1"
    raise ValueError(f"unsupported native input route header {header!r}")


def session_host_args(header: str) -> list[str]:
    """Host flags that replay the session options a header records (rule 45).

    ValueError when the header records an option, or a value, that no replay host flag applies."""
    flags: list[str] = []
    for field in header.rstrip("\r\n").split("\t")[3:]:
        if field.startswith(STAMP_KEYS):
            continue
        name, _, value = field.partition("=")
        flag = SESSION_OPTION_FLAGS.get(name)
        if flag is None or value not in ("0", "1"):
            raise ValueError(f"recording header option {field!r} cannot be applied on replay")
        if value == "1":
            flags.append(flag)
    return flags


def row_fields(kind: str, version: int) -> int | None:
    """Field count of a row kind in `version`, None when the kind does not exist there."""
    device = device_rows_version()
    table = {
        "T": (1, 8),
        "M": (1, 5),
        "U": (2, 8 if version >= 4 else 7),
        "P": (5, 7),
        "G": (5, 8),
        "D": (device, 5),  # T3820 device events
        "S": (device, 5),  # T3819 state digests
    }
    since, count = table.get(kind, (None, None))
    return count if since is not None and version >= since else None
