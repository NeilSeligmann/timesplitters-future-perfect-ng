# SPDX-License-Identifier: GPL-3.0-or-later
"""Entry-to-exit CFG path counts of lifted functions, capped at 3 (T3795, rule 43).

    python3 -m tools.t2capture.path_count --lift <tmp/private-host/lift-...> [--write]

The corpus coverage query asks for min(3, path count) shapes per function: a branch-free body
has one shape whatever the recording, so a flat 3 asks for recordings that cannot exist. The
count is derived from the lifted C (`gen/recomp_*.c`), never hand-listed (rule 45):
- blocks start at `loc_<VA>:` labels; edges are `if (...) goto`, `goto`, fallthrough, and
  `return;` (conditional when the line starts with `if (`) to the exit;
- a block ending in `RECOMP_NORETURN_RETURNED` (a call to a noreturn function, the early-exit
  assert form) is a dead end, not a path;
- a cycle on any entry-to-exit route counts as CAP (loops vary the callee sequence and read set);
- anything else that transfers control (jump tables `_jt`, a multi-line `if`, `switch`, a goto
  out of the function) makes the body underivable: absent from the table, the query uses CAP.
The table is `generated/t2capture/path-count.json` (lift-derived, untracked), keyed by the lift
tree name. A missing table or another tree means CAP for every function (fail closed).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLE_PATH = Path("generated/t2capture/path-count.json")
CAP = 3
FUNCTION = re.compile(r"^void sub_([0-9A-F]{8})\(void\)$")
LABEL = re.compile(r"^(loc_[0-9A-F]{8}):")
COND_GOTO = re.compile(r"^\s*if \(.*\) goto (loc_[0-9A-F]{8});")
GOTO = re.compile(r"^\s*goto (loc_[0-9A-F]{8});")
NORETURN = "RECOMP_NORETURN_RETURNED"
# Control flow the parser does not model: its presence makes the body underivable.
UNMODELLED = re.compile(r"_jt\b|\bswitch\s*\(|^\s*if \(|\bgoto\b|\blongjmp\b")
EXIT = "<exit>"
DEAD = "<dead>"


def _blocks(body: list[str]) -> dict[str, list[str]] | None:
    """{block label: successor labels} in body order (EXIT, DEAD are sinks); None = underivable."""
    order: list[str] = []
    succ: dict[str, list[str]] = {}
    current: str | None = None
    open_block = False  # control can still fall through the current block
    for line in body:
        text = line.split("/*", 1)[0].rstrip()
        label = LABEL.match(text)
        if label:
            name = label.group(1)
            if current is not None and open_block:
                succ[current].append(name)
            current, open_block = name, True
            order.append(name)
            succ[name] = []
            continue
        if current is None or not open_block or not text.strip():
            continue  # prologue, or unreachable text after an unconditional transfer
        if NORETURN in text:
            succ[current].append(DEAD)
            open_block = False
        elif "return;" in text:
            succ[current].append(EXIT)
            if text.lstrip().startswith("if ("):
                continue  # conditional return: also falls through
            open_block = False
        elif match := COND_GOTO.match(text):
            succ[current].append(match.group(1))
        elif match := GOTO.match(text):
            succ[current].append(match.group(1))
            open_block = False
        elif UNMODELLED.search(text):
            return None
    if not order:
        return None
    if current is not None and open_block:
        return None  # falls off the end of the body: not a modelled exit
    known = set(succ) | {EXIT, DEAD}
    if any(target not in known for targets in succ.values() for target in targets):
        return None  # a goto out of the function
    return {name: succ[name] for name in order}


def count_paths(body: list[str]) -> int | None:
    """Distinct entry-to-exit paths capped at CAP; None = underivable (caller uses CAP)."""
    graph = _blocks(body)
    if graph is None:
        return None
    entry = next(iter(graph))
    memo: dict[str, int] = {}

    def walk(node: str, stack: frozenset[str]) -> int:
        if node == EXIT:
            return 1
        if node == DEAD:
            return 0
        if node in stack:
            return CAP  # a cycle: loops vary the shape, never fewer than CAP
        if node in memo:
            return memo[node]
        total = 0
        for target in graph[node]:
            total = min(CAP, total + walk(target, stack | {node}))
        memo[node] = total
        return total

    found = walk(entry, frozenset())
    return found if found > 0 else None  # no reachable exit: underivable


def scan(gen_dir: Path) -> dict[int, int]:
    """{VA: path count} for every derivable lifted function (underivable ones are absent)."""
    out: dict[int, int] = {}
    for source in sorted(gen_dir.glob("recomp_*.c")):
        lines = source.read_text(errors="replace").splitlines()
        starts = [(i, m.group(1)) for i, line in enumerate(lines) if (m := FUNCTION.match(line))]
        for pos, (index, va) in enumerate(starts):
            end = starts[pos + 1][0] if pos + 1 < len(starts) else len(lines)
            body = lines[index + 1 : end]
            closing = next((i for i, text in enumerate(body) if text == "}"), None)
            if closing is None:
                continue
            found = count_paths(body[:closing])
            if found is not None:
                out[int(va, 16)] = found
    return out


def table_path() -> Path:
    local = ROOT / TABLE_PATH
    if local.exists():
        return local
    from tools.trace_store import main_root

    return main_root() / TABLE_PATH


def load(tree: str, path: Path | None = None) -> dict[int, int]:
    """{VA: path count} for lift tree `tree`; empty (CAP for all) when missing or another tree."""
    target = path or table_path()
    if not target.exists():
        print(
            f"path_count: no table at {target}, every function needs {CAP} shapes", file=sys.stderr
        )
        return {}
    table = json.loads(target.read_text())
    if not tree or table.get("lift_tree") != tree:
        print(f"path_count: table is for {table.get('lift_tree')!r}, not {tree!r}: {CAP} shapes",
              file=sys.stderr)  # fmt: skip
        return {}
    return {int(va, 16): int(n) for va, n in table.get("paths", {}).items()}


def required_shapes(va: int, paths: dict[int, int]) -> int:
    """min(CAP, path count); an unknown count fails closed at CAP."""
    return min(CAP, paths.get(va, CAP))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2capture.path_count", description=__doc__
    )
    parser.add_argument("--lift", type=Path, required=True, help="lift dir (holds gen/)")
    parser.add_argument("--write", action="store_true", help=f"write {TABLE_PATH} (main checkout)")
    args = parser.parse_args(argv)
    gen = args.lift / "gen"
    if not gen.is_dir():
        print(f"path_count: no {gen}", file=sys.stderr)
        return 2
    paths = scan(gen)
    table = {
        "schema": 1,
        "task": "T3795",
        "lift_tree": args.lift.name,
        "cap": CAP,
        "paths": {f"0x{va:08X}": n for va, n in sorted(paths.items())},
    }
    if args.write:
        from tools.trace_store import main_root

        target = main_root() / TABLE_PATH
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(table, indent=1) + "\n")
        print(f"wrote {target}")
    by_count = {n: sum(1 for v in paths.values() if v == n) for n in range(1, CAP + 1)}
    print(f"lift_tree={args.lift.name} derivable={len(paths)} by_count={by_count}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
