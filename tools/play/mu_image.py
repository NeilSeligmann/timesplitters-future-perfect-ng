# SPDX-License-Identifier: GPL-3.0-or-later
"""Create a blank, writable Xbox memory-unit image for the host MU adapter.

The FATX layout matches ``src/input/mu_fatx.c``. The 8 MiB capacity and
on-disk format remain INFERRED; see ``docs/t1085-mu-fatx.md``.
"""

from __future__ import annotations

import argparse
import os
import struct
import sys
from pathlib import Path

from tools.datadir import output_path

IMAGE_BYTES = 8 * 1024 * 1024
SECTOR_BYTES = 512
SUPERBLOCK_BYTES = 4096
CLUSTER_SECTORS = 32
CLUSTER_BYTES = SECTOR_BYTES * CLUSTER_SECTORS
VOLUME_ID = 0x4D550000
DEFAULT_IMAGE = output_path("tmp/mu/blank-mu.img")


def format_image(path: Path) -> Path:
    """Create an 8 MiB FATX16 image; refuse to overwrite any existing path."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as image:
            image.truncate(IMAGE_BYTES)
            header = bytearray(SUPERBLOCK_BYTES)
            struct.pack_into("<IIII", header, 0, 0x58544146, VOLUME_ID, CLUSTER_SECTORS, 1)
            header[16] = 1  # one FAT
            image.write(header)

            # Match mu_fatx.c geometry and its reserved FAT entries.
            clusters = (IMAGE_BYTES - SUPERBLOCK_BYTES) // CLUSTER_BYTES
            fat_bytes = ((clusters + 1) * 2 + SUPERBLOCK_BYTES - 1) // SUPERBLOCK_BYTES
            fat_bytes *= SUPERBLOCK_BYTES
            image.seek(SUPERBLOCK_BYTES)
            image.write(b"\xf8\xff\xff\xff")  # FAT16 entries 0 (media/EOC), 1 (EOC)

            # The first data cluster is the root directory; 0xff marks end entries.
            image.seek(SUPERBLOCK_BYTES + fat_bytes)
            image.write(b"\xff" * CLUSTER_BYTES)
            image.flush()
            os.fsync(image.fileno())
    except BaseException:
        try:
            path.unlink()
        except OSError:
            pass
        raise
    return path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("create", help="create a blank 8 MiB FATX MU image")
    create.add_argument("path", nargs="?", type=Path, default=DEFAULT_IMAGE)
    args = parser.parse_args(argv)
    try:
        path = format_image(args.path)
    except FileExistsError:
        parser.error(f"refusing to overwrite existing image: {args.path}")
    except OSError as error:
        parser.error(f"cannot create image {args.path}: {error}")
    print(f"created {path} ({IMAGE_BYTES} bytes, FATX16, volume id 0x{VOLUME_ID:08x})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
