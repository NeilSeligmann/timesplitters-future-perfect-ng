# SPDX-License-Identifier: GPL-3.0-or-later
"""Preserve pilot lifted originals in an ignored build directory for runtime A/B.

Only the definition name changes. Direct/indirect callers retain the public
adapter symbol, so enabling Rust cannot bypass accounting inside a chunk.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

from tools.replace.rust_lane import ALL_LANE, INV_HOST_ROOTS, INV_ROOTS, ROOTS

# The 26 four-register roots (24 selectable plus the two harness-only wave 2 candidates) and the
# T1854 lane roots, which T1858 made selectable in the host; T2407 adds its actor append seam.
HOST_ROOTS = tuple(va for va in ROOTS if va not in ALL_LANE)
# T1874: the 129 inventory and pose/control lane roots are host-selectable too, including the 11
# that the harness caller audit refused (not in ROOTS, the harness replacement set). The default
# preserves every host-selectable root (ORIGINALS).
# the roots of T1815 to T1854 and T1835 wave 2b (the T1858 stub-callee native A/B set)
BASE_ROOTS = tuple(va for va in ROOTS if va not in INV_ROOTS)
ORIGINALS = (*ROOTS, *(va for va in INV_HOST_ROOTS if va not in ROOTS))


def preserve(
    gen: Path, output: Path, roots: tuple[int, ...] = ORIGINALS, prefix: str = "tsfp_original_"
) -> None:
    bodies: dict[int, tuple[str, str]] = {}
    wanted = set(roots)
    # Scan each lifted unit once. Running a DOTALL regex for every root on each
    # unit rescans the whole lift 1,308 times during a hybrid-host build.
    definition = re.compile(
        r"^void sub_([0-9A-F]{8})\(void\)\s*\n\{.*?^\}", re.MULTILINE | re.DOTALL
    )
    for path in sorted(gen.glob("recomp_*.c")):
        text = path.read_text()
        for match in definition.finditer(text):
            va = int(match.group(1), 16)
            if va not in wanted:
                continue
            if va in bodies:
                raise ValueError(f"duplicate lifted definition {va:08X}")
            bodies[va] = (path.name, match.group())
    if set(bodies) != set(roots):
        raise ValueError(
            "missing lifted pilot roots: "
            + ",".join(f"{va:08X}" for va in roots if va not in bodies)
        )
    source = '#define RECOMP_GENERATED_CODE\n#include "recomp_funcs.h"\n#include <math.h>\n'
    records = []
    for va in roots:
        name, body = bodies[va]
        source += body.replace(f"void sub_{va:08X}(void)", f"void {prefix}{va:08x}(void)", 1) + "\n"
        records.append(
            {
                "va": f"0x{va:08X}",
                "chunk": name,
                "body_sha256": hashlib.sha256(body.encode()).hexdigest(),
            }
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(source)
    output.with_suffix(".json").write_text(
        json.dumps(
            {
                "schema": 1,
                "roots": records,
                "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
            },
            indent=2,
        )
        + "\n"
    )


def preserve_chunks(
    gen: Path,
    output_dir: Path,
    stem: str,
    roots: tuple[int, ...] = ORIGINALS,
    prefix: str = "tsfp_original_",
    chunk_count: int = 4,
) -> list[Path]:
    """Write root-order-preserving source chunks and a stable aggregate receipt."""
    if chunk_count < 1:
        raise ValueError("chunk_count must be positive")
    bodies: dict[int, tuple[str, str]] = {}
    wanted = set(roots)
    definition = re.compile(
        r"^void sub_([0-9A-F]{8})\(void\)\s*\n\{.*?^\}", re.MULTILINE | re.DOTALL
    )
    for path in sorted(gen.glob("recomp_*.c")):
        text = path.read_text()
        for match in definition.finditer(text):
            va = int(match.group(1), 16)
            if va in wanted:
                if va in bodies:
                    raise ValueError(f"duplicate lifted definition {va:08X}")
                bodies[va] = (path.name, match.group())
    if set(bodies) != wanted:
        missing = ",".join(f"{va:08X}" for va in roots if va not in bodies)
        raise ValueError("missing lifted pilot roots: " + missing)
    output_dir.mkdir(parents=True, exist_ok=True)
    count = min(chunk_count, len(roots))
    groups = [
        roots[(i * len(roots)) // count : ((i + 1) * len(roots)) // count] for i in range(count)
    ]
    paths: list[Path] = []
    all_records = []
    header = '#define RECOMP_GENERATED_CODE\n#include "recomp_funcs.h"\n#include <math.h>\n'
    aggregate_source = header
    for index, group in enumerate(groups):
        source = header
        records = []
        for va in group:
            name, body = bodies[va]
            renamed = (
                body.replace(f"void sub_{va:08X}(void)", f"void {prefix}{va:08x}(void)", 1) + "\n"
            )
            source += renamed
            aggregate_source += renamed
            records.append(
                {
                    "va": f"0x{va:08X}",
                    "chunk": name,
                    "body_sha256": hashlib.sha256(body.encode()).hexdigest(),
                }
            )
        path = output_dir / f"{stem}-{index}.c"
        path.write_text(source)
        paths.append(path)
        all_records.extend(records)
    aggregate = {
        "schema": 1,
        "roots": all_records,
        "source_sha256": hashlib.sha256(aggregate_source.encode()).hexdigest(),
    }
    (output_dir / f"{stem}.json").write_text(json.dumps(aggregate, indent=2) + "\n")
    return paths


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gen-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    parser.add_argument(
        "--chunk-dir", type=Path, help="write four independently compilable C chunks"
    )
    parser.add_argument("--chunk-stem", default="rust-originals")
    parser.add_argument(
        "--harness-roots",
        action="store_true",
        help="only the roots of T1815 to T1854 and T1835 wave 2b (the T1858 stub-callee A/B set)",
    )
    parser.add_argument(
        "--roots",
        help="comma-separated hex VAs to preserve instead of all retained roots (ext roots)",
    )
    args = parser.parse_args()
    if args.harness_roots and args.roots:
        parser.error("--harness-roots and --roots are exclusive")
    if args.roots:
        roots = tuple(int(token, 16) for token in args.roots.split(","))
    else:
        roots = BASE_ROOTS if args.harness_roots else ORIGINALS
    if args.chunk_dir:
        preserve_chunks(args.gen_dir, args.chunk_dir, args.chunk_stem, roots)
    else:
        if args.out is None:
            parser.error("--out is required unless --chunk-dir is used")
        preserve(args.gen_dir, args.out, roots)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
