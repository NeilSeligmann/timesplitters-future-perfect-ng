"""T2319 native PC save location at the launcher boundary (XDG Linux contract)."""

from __future__ import annotations

import os
from collections.abc import Mapping
from pathlib import Path


def standard_path(env: Mapping[str, str] | None = None) -> Path:
    """Match tsfp_engine_fileio::local_storage::standard_path without filesystem effects."""
    env = os.environ if env is None else env
    xdg = Path(env.get("XDG_DATA_HOME", ""))
    if xdg.is_absolute():
        return xdg / "tsfp" / "saves"
    home = Path(env.get("HOME", ""))
    if not home.is_absolute():
        raise ValueError("no absolute HOME or XDG_DATA_HOME for PC saves")
    return home / ".local" / "share" / "tsfp" / "saves"
