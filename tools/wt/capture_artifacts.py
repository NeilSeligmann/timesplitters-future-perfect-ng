# SPDX-License-Identifier: GPL-3.0-or-later
"""Resolve capture artifact paths inside the replay queue's result directory."""

from __future__ import annotations

from pathlib import Path, PurePosixPath

_CAPTURE_PREFIX = "@capture/"


def resolve_capture_env(value: str, output_dir: str | Path) -> tuple[str, str]:
    """Parse a TSFP env override and expand its stable capture-output marker.

    ``NAME=@capture/name.trace`` is a cache-stable way to ask the queue to put
    an artifact beside its receipt. Absolute paths and parent traversal are
    rejected so the remote and local capture paths have the same boundary.
    """
    name, separator, content = value.partition("=")
    if not separator or not name.startswith("TSFP_"):
        raise ValueError(f"invalid replay environment override: {value}")
    if content.startswith(_CAPTURE_PREFIX):
        relative = PurePosixPath(content[len(_CAPTURE_PREFIX) :])
        if (
            len(relative.parts) != 1
            or relative.is_absolute()
            or any(part in {"", ".", ".."} for part in relative.parts)
            or relative.suffix != ".trace"
        ):
            raise ValueError(f"invalid capture artifact path: {content}")
        content = str(Path(output_dir).joinpath(*relative.parts))
    return name, content
