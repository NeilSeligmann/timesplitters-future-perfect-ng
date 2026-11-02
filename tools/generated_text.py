# SPDX-License-Identifier: GPL-3.0-or-later
"""LF-only writers for generated CSV and text.

Open CSV files with newline="" (the csv module owns record separators). These
factories force LF even when a caller supplies a dialect with CRLF defaults.
For generated plain text, use write_lf_text to disable platform translation.
"""

import csv
from pathlib import Path
from typing import Any, TextIO


def lf_csv_writer(stream: TextIO, **kwargs: Any) -> Any:
    kwargs["lineterminator"] = "\n"
    return csv.writer(stream, **kwargs)


def lf_csv_dict_writer(stream: TextIO, **kwargs: Any) -> csv.DictWriter:
    kwargs["lineterminator"] = "\n"
    return csv.DictWriter(stream, **kwargs)


def write_lf_text(path: Path, text: str) -> None:
    path.write_text(text.replace("\r\n", "\n"), encoding="utf-8", newline="\n")


def publish_if_changed(staged: Path, destination: Path) -> bool:
    """Publish a staged generated file only when its bytes differ.

    Returns True when the destination was replaced. On an unchanged result,
    the staged file is removed and the destination's inode and mtime survive.
    The caller owns the staging directory and is responsible for creating it.
    """
    if destination.exists() and staged.read_bytes() == destination.read_bytes():
        staged.unlink()
        return False
    destination.parent.mkdir(parents=True, exist_ok=True)
    staged.replace(destination)
    return True
