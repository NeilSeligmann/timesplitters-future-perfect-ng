# SPDX-License-Identifier: GPL-3.0-or-later
"""Preserve codewrite lifted originals in a private build directory (T1818)."""

from __future__ import annotations

import argparse
from pathlib import Path

from tools.play.rust_originals import preserve

ROOTS = (0x3C97AB, 0x40F720, 0x38AEB0, 0x38C010, 0x3B9BC0, 0x3BFA40, 0x3AA1D0)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gen-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    preserve(args.gen_dir, args.out, ROOTS, "tsfp_codewrite_original_")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
