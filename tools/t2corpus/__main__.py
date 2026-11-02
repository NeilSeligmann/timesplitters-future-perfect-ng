# SPDX-License-Identifier: GPL-3.0-or-later
"""python3 -m tools.t2corpus: ingest, stats, coverage, summary, entries, export (T3741)."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from tools import datadir
from tools.t2capture import path_count
from tools.t2corpus import compact as compaction
from tools.t2corpus import coverage as cov
from tools.t2corpus.spool import SpoolError, flag_names, read_spool, segment_paths
from tools.t2corpus.store import Corpus

SUMMARY_PATH = Path("docs/data/t2corpus-summary.json")
WEIGHTS_PATH = Path("docs/data/t1884-exec-weights.csv")
DISPOSITIONS_PATH = Path("tools/data/system_dispositions.json")
ROUTES_PATH = Path("docs/data/t3702-route-coverage.json")


def build_summary(corpus: Corpus, tree: str) -> dict:
    stats = corpus.function_stats(tree)
    return {
        "schema": 1,
        "task": "T3741",
        "lifted_tree_hash": tree,
        "note": "counts and hashes only; raw captured memory lives in the derived-data store",
        "functions": {f"0x{va:08x}": stat for va, stat in sorted(stats.items())},
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python3 -m tools.t2corpus", description=__doc__)
    parser.add_argument("--store", type=Path, help="corpus dir (default: data_dir('t2corpus'))")
    parser.add_argument("--lifted-tree", help="lifted tree hash to query (default: newest)")
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("ingest", help="add spools (append-only)").add_argument("spools", nargs="+")
    p = sub.add_parser(
        "compact", help="ingest finished segments of a spool and delete them (T3745)"
    )
    p.add_argument("spool", type=Path)
    p.add_argument("--final", action="store_true", help="also take a .gz.part (host has exited)")
    sub.add_parser("stats", help="entries, shapes and recordings per function")
    p = sub.add_parser("bundle", help="write every entry as spools for another store (T3740)")
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--tag", required=True, help="unique id of this bundle (the remote run)")
    p = sub.add_parser("coverage", help="executed port functions lacking corpus shapes")
    p.add_argument(
        "--min-shapes", type=int, default=3, help="cap; per function min(this, CFG paths)"
    )
    p.add_argument("--flat", action="store_true", help="ignore CFG path counts (pre-T3795 query)")
    p.add_argument("--json", action="store_true")
    p.add_argument("--exec-weights", type=Path, default=WEIGHTS_PATH)
    p.add_argument("--dispositions", type=Path, default=DISPOSITIONS_PATH)
    p.add_argument("--route-coverage", type=Path, default=ROUTES_PATH)
    p = sub.add_parser("summary", help="committed per-function summary")
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    p.add_argument("--path", type=Path, default=SUMMARY_PATH)
    p = sub.add_parser("entries", help="provenance of one function's entries (no raw bytes)")
    p.add_argument("--va", required=True)
    p = sub.add_parser("export", help="raw records of one function as a spool")
    p.add_argument("--va", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--skip-flagged", action="store_true", help="drop records with nonzero flags")
    return parser


def _cmd_ingest(corpus: Corpus, args: argparse.Namespace) -> int:
    status = 0
    names: list[str] = []
    for name in args.spools:  # T3745: a segmented spool's base path stands for its segments
        segments = [] if Path(name).exists() else segment_paths(Path(name), include_open=True)
        if segments:
            names.extend(str(segment) for segment in segments)
        else:
            names.append(name)
    for name in names:
        try:
            rep = corpus.ingest(read_spool(Path(name)))
        except (SpoolError, OSError) as exc:
            print(f"{name}: unreadable spool: {exc}", file=sys.stderr)
            status = 1
            continue
        if rep.skipped_recording:
            print(f"{name}: recording already ingested for this tree, no-op")
            continue
        print(
            f"{name}: records={rep.records} kept={rep.kept} duplicate={rep.duplicates} "
            f"capped_shape={rep.capped_shape} capped_function={rep.capped_function} "
            f"new_shape_over_function_cap={rep.new_shape_over_function_cap} "
            f"over_hard_ceiling={rep.over_hard_ceiling} malformed={rep.malformed} "
            f"truncated={rep.truncated}"
        )
    return status


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    corpus = Corpus(args.store or datadir.data_dir("t2corpus"))
    try:
        if args.cmd == "ingest":
            return _cmd_ingest(corpus, args)
        if args.cmd == "bundle":
            for path in corpus.bundle(args.out, args.tag):
                print(path)
            return 0
        if args.cmd == "compact":
            ledger = compaction.compact(args.spool, corpus.root, final=args.final)
            print(json.dumps(compaction.asdict(ledger), sort_keys=True))
            return 0
        tree = args.lifted_tree or corpus.newest_tree() or ""
        if args.cmd == "stats":
            for va, stat in sorted(corpus.function_stats(tree).items()):
                print(f"0x{va:08x} entries={stat['entries']} shapes={stat['shapes']} "
                      f"recordings={stat['recordings']} flags={stat['flag_counts']}")  # fmt: skip
        elif args.cmd == "coverage":
            from tools.live_census import store as live_store

            live = live_store.union_vas()  # T3824: executed = replay weights union live sessions
            live_systems = None
            if live:
                from tools.t2reach.audit import port_rows

                live_systems = {
                    int(str(r["va"]), 16): str(r.get("subsystem") or "") for r in port_rows()
                }
            executed = cov.executed_port_functions(
                args.exec_weights, args.dispositions, live, live_systems
            )
            gaps = cov.coverage_gaps(
                executed,
                corpus.function_stats(tree),
                cov.route_index(args.route_coverage),
                args.min_shapes,
                None if args.flat else path_count.load(tree),
            )
            print(json.dumps(gaps, indent=1) if args.json else cov.render_table(gaps))
        elif args.cmd == "summary":
            text = json.dumps(build_summary(corpus, tree), indent=1, sort_keys=True) + "\n"
            if args.write:
                args.path.write_text(text)
            elif not tree:
                print("no corpus recordings here, nothing to check")
            elif not args.path.exists() or args.path.read_text() != text:
                print(f"{args.path} is stale: run summary --write", file=sys.stderr)
                return 1
        elif args.cmd == "entries":
            for row in corpus.entry_rows(tree, int(args.va, 16)):
                print(f"recording={row['recording_sha256']} poll={row['poll']} "
                      f"thread={row['guest_thread']} caller=0x{row['caller_va']:08x} "
                      f"ret=0x{row['return_address']:08x} shape={row['shape'][:12]} "
                      f"digest={row['digest'][:12]} flags={flag_names(row['flags'])}")  # fmt: skip
        elif args.cmd == "export":
            count = corpus.export(int(args.va, 16), tree, args.out, args.skip_flagged)
            print(f"exported {count} records to {args.out}")
    finally:
        corpus.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
