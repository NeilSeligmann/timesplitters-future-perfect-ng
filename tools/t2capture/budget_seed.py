# SPDX-License-Identifier: GPL-3.0-or-later
"""Seed table of the T3803 per-VA capture budget (src/host/t2_capture.c, rule 43).

    python3 -m tools.t2capture.budget_seed --out <file> [--store <dir>] [--lifted-tree <hash>]

The capture runtime stops recording a VA once it has K kept entries (TSFP_T2_CAPTURE_BUDGET, default
32) and min(3, CFG path count) shapes. This file tells it what the corpus already holds, so a
re-capture only pays for VAs that still need entries. One line per VA:
`<va hex> <corpus entries> <corpus shapes> <min(3, paths)>`; a 0 shape minimum means unknown (3).
Entries and shapes come from `tools.t2corpus` function_stats of the store for one lifted tree, the
shape minimum from tools/t2capture/path_count.py (T3795, read through its owner). The runtime reads
the file named by TSFP_T2_CAPTURE_BUDGET_SEED (`rust_trace capture --t2-budget-seed`). The file
holds VAs and counts only, no guest bytes.

T3836 targeted recapture: `--only <va,...|@file>` writes a seed of just those VAs (0 entries each)
with `#! allow-only 1`, so the runtime records nothing else, and `--max-set-bytes` /
`--max-callees` add `#! <name> <n>` directives that raise the size caps for the run. The runtime
skips `#` lines; tools.t2capture.runtime_env turns the directives into the TSFP_T2_CAPTURE_*
variables on the host (the seed file is what reaches a remote host).
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from tools.t2capture import path_count

DEFAULT_STORE = "t2corpus-f2"
ENV = "TSFP_T2_CAPTURE_BUDGET_SEED"
# T3836: seed directives (`#! <name> <value>`) and the runtime variable each one sets.
DIRECTIVES = {
    "allow-only": "TSFP_T2_CAPTURE_ALLOW_ONLY",
    "max-set-bytes": "TSFP_T2_CAPTURE_MAX_SET_BYTES",
    "max-callees": "TSFP_T2_CAPTURE_MAX_CALLEES",
}


def render(seed: dict[int, tuple[int, int, int]], directives: dict[str, int] | None = None) -> str:
    lines = ["# T3803 capture budget seed: va entries shapes need (0 = unknown, 3)"]
    for name, value in sorted((directives or {}).items()):
        if name not in DIRECTIVES:
            raise ValueError(f"budget_seed: unknown directive {name}")
        lines.append(f"#! {name} {int(value)}")
    lines += [f"{va:08x} {e} {s} {n}" for va, (e, s, n) in sorted(seed.items())]
    return "\n".join(lines) + "\n"


def directives_env(text: str) -> dict[str, str]:
    """The TSFP_T2_CAPTURE_* variables the `#!` directives of a seed file ask for (T3836).
    An unknown directive or a non-integer value is an error, never ignored."""
    env: dict[str, str] = {}
    for line in text.splitlines():
        if not line.startswith("#!"):
            continue
        parts = line[2:].split()
        if len(parts) != 2 or parts[0] not in DIRECTIVES or not parts[1].isdigit():
            raise ValueError(f"budget_seed: bad directive {line!r}")
        env[DIRECTIVES[parts[0]]] = parts[1]
    return env


def parse_vas(text: str) -> list[int]:
    """`0x1234,5678` or `@file` (one VA per line or comma separated, `#` comments)."""
    if text.startswith("@"):
        text = Path(text[1:]).read_text()
    items = [item.split("#")[0].strip() for line in text.splitlines() for item in line.split(",")]
    return sorted({int(item, 16) for item in items if item})


def write_only(out: Path, vas: list[int], max_set_bytes: int = 0, max_callees: int = 0) -> int:
    """T3836: a seed that records only `vas` (fresh budget each), optional raised caps."""
    if not vas:
        raise ValueError("budget_seed: --only names no VA")
    directives = {"allow-only": 1}
    if max_set_bytes:
        directives["max-set-bytes"] = max_set_bytes
    if max_callees:
        directives["max-callees"] = max_callees
    out.parent.mkdir(parents=True, exist_ok=True)
    temp = out.with_name(out.name + ".tmp")
    temp.write_text(render({va: (0, 0, 0) for va in vas}, directives))
    temp.replace(out)
    return len(vas)


def build(stats: dict[int, dict], paths: dict[int, int]) -> dict[int, tuple[int, int, int]]:
    """{va: (entries, shapes, need)}: every VA with entries, plus VAs whose path count is below CAP
    (their minimum is met sooner). VAs absent from the path table carry need 0 (unknown)."""
    seed: dict[int, tuple[int, int, int]] = {}
    for va, stat in stats.items():
        need = path_count.required_shapes(va, paths) if va in paths else 0
        seed[va] = (int(stat.get("entries", 0)), int(stat.get("shapes", 0)), need)
    for va, count in paths.items():
        if va not in seed and count < path_count.CAP:
            seed[va] = (0, 0, path_count.required_shapes(va, paths))
    return seed


def write(out: Path, store: Path, tree: str | None = None) -> tuple[str, int]:
    """Write the seed for `store` (lifted tree `tree`, default its newest); (tree, VA count)."""
    from tools.t2corpus.store import Corpus

    corpus = Corpus(store)
    try:
        tree = tree or corpus.newest_tree() or ""
        if not tree:
            raise ValueError(f"budget_seed: corpus {store} holds no recording")
        stats = corpus.function_stats(tree)
    finally:
        corpus.close()
    seed = build(stats, path_count.load(tree))
    out.parent.mkdir(parents=True, exist_ok=True)
    temp = out.with_name(out.name + ".tmp")
    temp.write_text(render(seed))
    temp.replace(out)
    return tree, len(seed)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2capture.budget_seed", description=__doc__
    )
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--store", type=Path, help=f"corpus store (default the data dir {DEFAULT_STORE})"
    )
    parser.add_argument(
        "--lifted-tree", help="lift tree of the entries (default the store's newest)"
    )
    parser.add_argument("--only", help="T3836: record only these VAs (comma list or @file)")
    parser.add_argument("--max-set-bytes", type=int, default=0, help="T3836: read/write set cap")
    parser.add_argument("--max-callees", type=int, default=0, help="T3836: callee cap")
    args = parser.parse_args(argv)
    if args.only:
        try:
            count = write_only(args.out, parse_vas(args.only), args.max_set_bytes, args.max_callees)
        except (OSError, ValueError) as exc:
            print(exc, file=sys.stderr)
            return 2
        print(f"budget_seed: allow-only {count} VAs, {args.out}")
        return 0
    if args.max_set_bytes or args.max_callees:
        print("budget_seed: --max-set-bytes / --max-callees need --only", file=sys.stderr)
        return 2
    from tools import datadir

    store = args.store or datadir.data_dir(DEFAULT_STORE)
    try:
        tree, count = write(args.out, store, args.lifted_tree)
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 2
    print(f"budget_seed: {count} VAs, tree {tree}, {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
