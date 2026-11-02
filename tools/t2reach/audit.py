# SPDX-License-Identifier: GPL-3.0-or-later
"""T3793 Tier 2 reachability audit: classify never-executed port-class functions.

    python3 -m tools.t2reach audit [--write] [--xbe PATH]
    python3 -m tools.t2reach check [--xbe PATH]

`audit` builds the static graph (`tools.t2reach.graph`) from the certified retail XBE and the
shared function table, checks it against every function with execution evidence (none may
come out unreachable, else it stops), classifies each port-class function without execution
evidence (`tools.t2reach.classify`) and prints the per-class, per-subsystem counts. `--write`
stores the per-function CSV `generated/t2reach/tier2_reachability.csv` (bulk per-address output,
never committed), the committed counts `tools/data/tier2_reachability_summary.json` and
`tools/data/tier2_exclusions.json` (the MEASURED-unreachable ones the ledger leaves out of the
Tier 2 denominator). `check` rebuilds the graph and refuses an exclusion whose function is
live or executed. Inputs are read through their owners (the ledger snapshot, the weights
loader, the recording census, the capture corpus); a missing input stops loudly.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter, defaultdict
from collections.abc import Mapping
from pathlib import Path

from tools.generated_text import lf_csv_writer
from tools.t2reach import classify as cls
from tools.t2reach import graph as gr

ROOT = Path(__file__).resolve().parents[2]
RULES = Path("tools/data/t2reach_rules.json")
OUT_CSV = Path("generated/t2reach/tier2_reachability.csv")
OUT_SUMMARY = Path("tools/data/tier2_reachability_summary.json")
OUT_EXCLUSIONS = Path("tools/data/tier2_exclusions.json")
FUNCTIONS = Path("generated/retail/functions.csv")
OVERRIDES = Path("tools/data/function_overrides.csv")
ADDITIONS = Path("tools/data/function_additions.csv")
CENSUS = Path("docs/data/t3702-recording-census.json")
CORPUS = "t2corpus-f2"
XBE_CACHE = Path("tmp/wt-cache/rea-xbe/default.xbe")
DOC = "docs/t3793-tier2-reachability-audit.md"
CSV_COLUMNS = ("va", "subsystem", "class", "confidence", "session", "evidence")


def _resolve(path: Path | str) -> Path:
    from tools.ledger.cli import resolve_snapshot

    return resolve_snapshot(str(path))


def _need(path: Path, what: str) -> Path:
    if not path.exists():
        raise SystemExit(f"t2reach: {what} not found at {path}")
    return path


def find_xbe(explicit: str | None) -> Path:
    from tools.coverage import XBE_SEARCH
    from tools.orphan_reach_report import CERTIFIED

    candidates = [Path(explicit)] if explicit else [*map(_resolve, XBE_SEARCH), _resolve(XBE_CACHE)]
    for path in candidates:
        if path.is_file():
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if digest != CERTIFIED:
                raise SystemExit(f"t2reach: {path} is not the certified retail XBE ({digest[:12]})")
            return path
    raise SystemExit(f"t2reach: retail XBE not found (tried {', '.join(map(str, candidates))})")


def load_graph(xbe: Path) -> tuple[gr.Graph, gr.Reach, dict[str, str]]:
    from tools.codediff.boundaries import load_function_table
    from tools.name_candidates import Image

    functions_csv = _need(_resolve(FUNCTIONS), "function table export")
    table = load_function_table(functions_csv, ROOT / OVERRIDES, ROOT / ADDITIONS)
    nodes = gr.partition(
        {
            f.entry_va: gr.Node(f.entry_va, max(f.body_max_va + 1, f.entry_va + f.size_bytes))
            for f in table
        }
    )
    image = Image(xbe)
    graph = gr.build(
        gr.regions_of(image),
        nodes,
        entry_point=image.xbe.entry_point,
        data_roots=gr.loader_roots(image),
        kernel_slots=gr.kernel_slots(image),
    )
    pins = {
        "xbe_sha256": hashlib.sha256(image.data).hexdigest(),
        "function_table": f"{FUNCTIONS} + {OVERRIDES} + {ADDITIONS}",
        "function_table_sha256": hashlib.sha256(
            b"".join(
                p.read_bytes()
                for p in (functions_csv, ROOT / OVERRIDES, ROOT / ADDITIONS)
                if p.is_file()
            )
        ).hexdigest(),
        "functions": str(len(nodes)),
    }
    return graph, gr.fixpoint(graph), pins


def section_bounds(xbe: Path) -> dict[str, tuple[int, int]]:
    from tools.xbe import parse_xbe

    return {
        s.name: (s.virtual_addr, s.virtual_addr + s.virtual_size)
        for s in parse_xbe(xbe.read_bytes()).sections
    }


def execution_evidence(
    rows: list[Mapping[str, object]],
    *,
    include_live: bool = True,
) -> tuple[set[int], dict[int, str], dict[str, int]]:
    """(VAs with any execution evidence, VA -> recordings that ran it, count per source).

    T3824: `live census` is the union of the owner's live play sessions (tools.live_census),
    empty until the first census build session; include_live=False is the replay-only view."""
    import os

    from tools import verification
    from tools.datadir import data_dir
    from tools.ledger import execution

    sources: dict[str, set[int]] = {}
    weights = execution.load(_need(_resolve(execution.DEFAULT_PATH), "execution weights"))
    sources["weights"] = {va for va, calls in (weights.calls if weights else {}).items() if calls}
    census = json.loads(_need(_resolve(CENSUS), "recording census").read_text(encoding="utf-8"))
    where: dict[int, list[str]] = defaultdict(list)
    for recording, item in sorted(census.get("recordings", {}).items()):
        for va_text, calls in item.get("port_calls", {}).items():
            if calls:
                where[int(va_text, 16)].append(recording)
    sources["recording census"] = set(where)
    corpus = data_dir(CORPUS)
    if not corpus.is_dir():
        raise SystemExit(f"t2reach: capture corpus {corpus} not found")
    sources["capture corpus"] = {
        int(name, 16) for name in os.listdir(corpus) if len(name) == 8 and (corpus / name).is_dir()
    }
    sources["tier 2 credit"] = {
        int(str(row["va"]), 16)
        for row in rows
        if verification.is_tier2(row.get("verification_tier"))
    }
    if include_live:
        from tools.live_census import store as live_store

        sources["live census"] = set(live_store.union_vas())
    executed = set().union(*sources.values())
    labels = {
        va: ", ".join(names[:2]) + (" ..." if len(names) > 2 else "") for va, names in where.items()
    }
    return executed, labels, {name: len(vas) for name, vas in sources.items()}


def port_rows() -> list[Mapping[str, object]]:
    from tools.ledger import dispositions
    from tools.ledger.cli import DEFAULT_OUT, resolve_snapshot, rows_of

    table = dispositions.load(ROOT / dispositions.DEFAULT_PATH)
    rows = rows_of(resolve_snapshot(str(DEFAULT_OUT)))
    return [
        r
        for r in rows
        if dispositions.disposition_of(str(r.get("subsystem") or "unknown"), table) == "port"
    ]


def run_audit(xbe: Path) -> dict[str, object]:
    rules = json.loads((ROOT / RULES).read_text(encoding="utf-8"))
    rows = port_rows()
    executed, where, source_counts = execution_evidence(rows)
    graph, reach, pins = load_graph(xbe)
    contradicted = sorted(va for va in executed if va in graph.functions and va not in reach.live)
    if contradicted:
        raise SystemExit(
            "t2reach: the static graph calls executed functions unreachable, so it is unsound: "
            + ", ".join(f"{va:#x}" for va in contradicted[:20])
        )
    facts = cls.Facts(
        nodes={va: (n.start, n.end) for va, n in graph.nodes.items()},
        edges=graph.edges,
        callers=graph.callers(),
        live=reach.live,
        functions=graph.functions,
        data_objects=graph.data_objects,
        imports=graph.imports,
        executed=frozenset(executed),
        sections=section_bounds(xbe),
    )
    names = {int(str(r["va"]), 16): str(r.get("name") or "") for r in rows}
    result: list[cls.Row] = []
    for row in rows:
        va = int(str(row["va"]), 16)
        if va in executed:
            continue
        result.append(
            cls.classify(
                va, str(row.get("subsystem") or ""), names.get(va, ""), facts, rules, names, where
            )
        )
    dead_nodes = sorted(set(graph.nodes) - reach.live)
    return {
        "rows": result,
        "pins": pins,
        "port_functions": len(rows),
        "executed_port": sum(1 for r in rows if int(str(r["va"]), 16) in executed),
        "executed_checked": len([va for va in executed if va in graph.functions]),
        "sources": source_counts,
        "graph": {
            "nodes": len(graph.nodes),
            "functions": len(graph.functions),
            "data_objects": len(graph.data_objects),
            "roots": len(graph.roots),
            "live_functions": len(graph.functions & reach.live),
            "dead_functions": len(reach.dead_functions()),
        },
        "dead_nodes": dead_nodes,
        "callers": facts.callers,
    }


def exclusions_document(audit: Mapping[str, object]) -> dict[str, object]:
    callers: Mapping[int, set[int]] = audit["callers"]  # type: ignore[assignment]
    functions = {}
    for row in audit["rows"]:  # type: ignore[attr-defined]
        if row.klass != "a" or row.confidence != "MEASURED":
            continue
        functions[f"{row.va:#x}"] = {
            "subsystem": row.subsystem,
            "name": row.name,
            "confidence": row.confidence,
            "callers": [f"{c:#x}" for c in sorted(callers.get(row.va, ()))],
            "evidence": row.evidence,
        }
    return {
        "schema": 1,
        "task": "T3793",
        "doc": DOC,
        "decision": "docs/decisions/tier2-reachability-audit.md",
        "method": (
            "tools.t2reach.graph static fixpoint from the XBE entry point and TLS directory over "
            "raw-byte and decoded references of every function, code gap and live data object; "
            "checked sound against every function with execution evidence"
        ),
        "rebuild": "python3 -m tools.t2reach audit --write",
        "pins": audit["pins"],
        "dead_nodes": [f"{va:#x}" for va in audit["dead_nodes"]],  # type: ignore[attr-defined]
        "functions": functions,
    }


def write_outputs(audit: Mapping[str, object]) -> None:
    rows: list[cls.Row] = sorted(audit["rows"], key=lambda r: r.va)  # type: ignore[arg-type]
    out_csv = csv_path()
    out_csv.parent.mkdir(parents=True, exist_ok=True)
    with out_csv.open("w", encoding="utf-8", newline="") as handle:
        writer = lf_csv_writer(handle)
        writer.writerow(CSV_COLUMNS)
        for row in rows:
            writer.writerow(
                (
                    f"{row.va:#010x}",
                    row.subsystem,
                    row.klass,
                    row.confidence,
                    row.session,
                    row.evidence,
                )
            )
    (ROOT / OUT_EXCLUSIONS).write_text(
        json.dumps(exclusions_document(audit), indent=1, sort_keys=True) + "\n", encoding="utf-8"
    )
    (ROOT / OUT_SUMMARY).write_text(
        json.dumps(summary_document(audit), indent=1, sort_keys=True) + "\n", encoding="utf-8"
    )


def csv_path() -> Path:
    """The per-function CSV under `generated/` (the main checkout's when a worktree has none)."""
    return _resolve(OUT_CSV.parts[0]).joinpath(*OUT_CSV.parts[1:])


def summary_document(audit: Mapping[str, object]) -> dict[str, object]:
    """Committed counts: per class and confidence, per subsystem, per session, class b kinds."""
    rows: list[cls.Row] = audit["rows"]  # type: ignore[assignment]
    by_system: dict[str, Counter[str]] = defaultdict(Counter)
    sessions: dict[str, Counter[str]] = defaultdict(Counter)
    for row in rows:
        by_system[row.subsystem][row.klass] += 1
        if row.klass == "c":
            sessions[row.session or "-"][row.confidence] += 1
    return {
        "schema": 1,
        "task": "T3793",
        "doc": DOC,
        "rebuild": "python3 -m tools.t2reach audit --write",
        "per_function_csv": str(OUT_CSV),
        "pins": audit["pins"],
        "port_functions": audit["port_functions"],
        "with_execution_evidence": audit["executed_port"],
        "without_execution_evidence": len(rows),
        "execution_sources": audit["sources"],
        "graph": audit["graph"],
        "soundness_executed_checked": audit["executed_checked"],
        "classes": {
            f"{klass} {confidence}": count
            for (klass, confidence), count in sorted(
                Counter((r.klass, r.confidence) for r in rows).items()
            )
        },
        "by_subsystem": {
            system: dict(sorted(c.items())) for system, c in sorted(by_system.items())
        },
        "sessions": {session: dict(sorted(c.items())) for session, c in sorted(sessions.items())},
        "class_b_kinds": dict(
            sorted(Counter(r.evidence.split(":")[0] for r in rows if r.klass == "b").items())
        ),
    }


def summary(audit: Mapping[str, object]) -> str:
    rows: list[cls.Row] = audit["rows"]  # type: ignore[assignment]
    lines = [
        f"port-class functions {audit['port_functions']}, with execution evidence "
        f"{audit['executed_port']}, without {len(rows)}",
        "execution sources (all functions): "
        + ", ".join(f"{k} {v}" for k, v in audit["sources"].items()),  # type: ignore[attr-defined]
        "graph: " + ", ".join(f"{k} {v}" for k, v in audit["graph"].items()),  # type: ignore[attr-defined]
        f"soundness: 0 of {audit['executed_checked']} executed functions unreachable",
        "",
        "| class | confidence | functions |",
        "|---|---|---:|",
    ]
    for (klass, confidence), count in sorted(
        Counter((r.klass, r.confidence) for r in rows).items()
    ):
        lines.append(f"| {klass} {cls.CLASS_TITLES[klass]} | {confidence} | {count} |")
    lines += [
        "",
        "| subsystem | a | b | c | d | e | total |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    by_system: dict[str, Counter[str]] = defaultdict(Counter)
    for row in rows:
        by_system[row.subsystem][row.klass] += 1
    for system, counts in sorted(by_system.items()):
        cells = " | ".join(str(counts.get(k, 0)) for k in cls.CLASSES)
        lines.append(f"| {system} | {cells} | {sum(counts.values())} |")
    lines += [
        "",
        "| session | functions (c MEASURED) | functions (c INFERRED) |",
        "|---|---:|---:|",
    ]
    sessions: dict[str, Counter[str]] = defaultdict(Counter)
    for row in rows:
        if row.klass == "c":
            sessions[row.session or "-"][row.confidence] += 1
    for session, counts in sorted(sessions.items(), key=lambda item: (len(item[0]), item[0])):
        lines.append(f"| {session} | {counts.get('MEASURED', 0)} | {counts.get('INFERRED', 0)} |")
    service = Counter(r.evidence.split(":")[0] for r in rows if r.klass == "b")
    lines += ["", "class b by kind: " + ", ".join(f"{k} {v}" for k, v in sorted(service.items()))]
    return "\n".join(lines)


def run_check(xbe: Path, path: Path) -> int:
    document = json.loads(path.read_text(encoding="utf-8"))
    rows = port_rows()
    executed, _, _ = execution_evidence(rows)
    graph, reach, _ = load_graph(xbe)
    refused = []
    for va_text in document.get("functions", {}):
        va = int(va_text, 16)
        if va in executed:
            refused.append(f"{va_text}: has execution evidence")
        elif va not in graph.functions:
            refused.append(f"{va_text}: not in the function table")
        elif va in reach.live:
            live = sorted(c for c in graph.callers().get(va, ()) if c in reach.live)
            refused.append(
                f"{va_text}: live, real callers {', '.join(f'{c:#x}' for c in live[:5])}"
            )
    for line in refused:
        print(f"REFUSED {line}", file=sys.stderr)
    print(f"t2reach check: {len(document.get('functions', {}))} exclusions, {len(refused)} refused")
    return 1 if refused else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2reach", description=__doc__.split("\n")[0]
    )
    sub = parser.add_subparsers(dest="command", required=True)
    audit = sub.add_parser("audit", help="classify never-executed port functions")
    audit.add_argument("--xbe")
    audit.add_argument("--write", action="store_true", help="write the CSV and exclusions file")
    check = sub.add_parser("check", help="refuse exclusions that are live or executed")
    check.add_argument("--xbe")
    check.add_argument("--exclusions", default=str(OUT_EXCLUSIONS))
    args = parser.parse_args(argv)
    xbe = find_xbe(args.xbe)
    if args.command == "check":
        return run_check(xbe, ROOT / args.exclusions)
    result = run_audit(xbe)
    print(summary(result))
    if args.write:
        write_outputs(result)
        print(f"wrote {csv_path()}, {OUT_SUMMARY} and {OUT_EXCLUSIONS}")
    return 0
