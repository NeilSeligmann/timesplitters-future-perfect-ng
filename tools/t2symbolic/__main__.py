# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871 CLI: `python3 -m tools.t2symbolic {inventory,select,run,pilot}`.

    inventory            measured counts of never-executed port functions, leaves, loop-free leaves
    select --count 50    the deterministic pilot list (sha256 of `T3871:<va>` over eligible leaves)
    run --va 0x42690     symbolic equivalence (Kani/CBMC) of one VA, optionally `--fuzz SECONDS`
    pilot --count 50     `run` over the pilot list, JSON lines to --out
    sweep                `run` over every loop-free leaf (T3875), JSON lines to --out
    receipts             `symbolic` Tier 2 receipts for the proved results (T3875)

Inputs are found through their owners: the newest complete lift (`tools.private_host`) and the
reachability CSV (`tools.t2reach.audit.csv_path`). `--lift` and `--reach` override them.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from tools.t2symbolic import inventory, runner
from tools.t2symbolic import report as summary_report

ROOT = inventory.ROOT


def default_gen_dir() -> Path:
    from tools import private_host

    main = private_host.main_checkout(ROOT, None)
    for lift in private_host.newest_lifts(main):
        if private_host.complete_lift(lift) and (lift / "gen" / "recomp_types.h").is_file():
            return lift / "gen"
    raise SystemExit(
        "t2symbolic: no complete lift under tmp/private-host (build one or pass --lift)"
    )


def default_reach_csv() -> Path:
    from tools.t2reach import audit

    return audit.csv_path()


def gen_dir_of(args: argparse.Namespace) -> Path:
    return args.lift / "gen" if args.lift else default_gen_dir()


def reach_of(args: argparse.Namespace) -> Path:
    return args.reach if args.reach else default_reach_csv()


def parse_va(text: str) -> int:
    return int(text, 16) if text.lower().startswith("0x") else int(text, 16)


def write_receipts(args: argparse.Namespace, rows: list[inventory.Row], gen_dir: Path) -> int:
    from tools import t2capture_promote
    from tools.t2symbolic import receipts

    modules = {row.va: row.rust.module for row in rows if row.rust is not None}
    results = [
        json.loads(line)
        for path in args.results
        for line in path.read_text().splitlines()
        if line.strip()
    ]
    written = receipts.write_all(
        results,
        args.out,
        module_of=modules.get,
        commit_of=lambda module: t2capture_promote.git_last_commit(ROOT, module),
        lifted_tree_hash=gen_dir.parent.name,
        tool_version=runner.kani_version(),
        unwind=args.unwind,
    )
    print(f"{len(written)} receipt(s) for {len(results)} result(s) in {args.out}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2symbolic", description=__doc__.splitlines()[0]
    )
    parser.add_argument(
        "--lift", type=Path, help="lift dir holding gen/ (default: newest complete lift)"
    )
    parser.add_argument(
        "--reach", type=Path, help="tier2_reachability.csv (default: tools.t2reach)"
    )
    sub = parser.add_subparsers(dest="command", required=True)

    inv = sub.add_parser(
        "inventory", help="measured leaf inventory of never-executed port functions"
    )
    inv.add_argument("--rows", type=Path, help="also write one JSON line per function here")

    sel = sub.add_parser("select", help="deterministic pilot list")
    sel.add_argument("--count", type=int, default=50)

    run = sub.add_parser("run", help="check one VA")
    run.add_argument("--va", type=parse_va, required=True)
    runner.add_run_options(run)

    pilot = sub.add_parser("pilot", help="check the pilot list")
    pilot.add_argument("--count", type=int, default=50)
    pilot.add_argument(
        "--fuzz-every", type=int, default=0, help="also fuzz every n-th pilot function (0: none)"
    )
    pilot.add_argument(
        "--out", type=Path, required=True, help="JSON lines, one verdict per function"
    )
    pilot.add_argument(
        "--shard", default="0/1", help="i/n: only pilot entries whose index is i mod n"
    )
    runner.add_run_options(pilot)

    sweep = sub.add_parser("sweep", help="check every loop-free leaf (T3875)")
    sweep.add_argument("--out", type=Path, required=True, help="JSON lines, resumable")
    sweep.add_argument("--shard", default="0/1", help="i/n: only entries whose index is i mod n")
    runner.add_run_options(sweep)

    receipts = sub.add_parser("receipts", help="receipts for proved_equal results")
    receipts.add_argument("--results", type=Path, nargs="+", required=True)
    receipts.add_argument("--out", type=Path, required=True)
    receipts.add_argument("--unwind", type=int, default=runner.DEFAULT_UNWIND)

    report = sub.add_parser("report", help="summarise pilot JSON lines (reclassified from logs)")
    report.add_argument("files", type=Path, nargs="+")

    args = parser.parse_args(argv)
    if args.command == "report":
        records = summary_report.load(args.files)
        print(summary_report.render(records))
        return 0
    gen_dir = gen_dir_of(args)
    rows = inventory.build(ROOT, gen_dir, reach_of(args))
    if args.command == "inventory":
        print(json.dumps(inventory.summary(rows), indent=1))
        if args.rows:
            args.rows.parent.mkdir(parents=True, exist_ok=True)
            args.rows.write_text("".join(json.dumps(row.as_json()) + "\n" for row in rows))
        return 0
    if args.command == "select":
        for row in inventory.select(rows, args.count):
            assert row.rust is not None
            print(f"{row.va:#x}\t{row.subsystem}\t{row.rust.crate}\t{row.name}")
        return 0
    by_va = {row.va: row for row in rows}
    if args.command == "run":
        row = by_va.get(args.va)
        if row is None:
            print(
                f"t2symbolic: {args.va:#x} is not a never-executed port function", file=sys.stderr
            )
            return 2
        result = runner.check(row, gen_dir, args, args.fuzz)
        print(json.dumps(result))
        return 0 if result["symbolic"]["verdict"] in runner.CLEAN_VERDICTS else 1
    if args.command == "receipts":
        return write_receipts(args, rows, gen_dir)
    if args.command == "sweep":
        selected = sorted((row for row in rows if row.loop_free), key=lambda row: row.va)
    else:
        selected = inventory.select(rows, args.count)
    shard, shards = (int(part) for part in args.shard.split("/"))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    # Resumable: VAs already in --out are kept and skipped (a stopped run continues).
    done = set()
    if args.out.exists():
        done = {json.loads(line)["va"] for line in args.out.read_text().splitlines() if line}
    with args.out.open("a") as handle:
        for index, row in enumerate(selected):
            if index % shards != shard or f"{row.va:#x}" in done:
                continue
            fuzz_every = getattr(args, "fuzz_every", 0)
            fuzz = args.fuzz if fuzz_every > 0 and index % fuzz_every == 0 else 0
            if not row.eligible:
                result = runner.ineligible(row)
            else:
                result = runner.check(row, gen_dir, args, fuzz)
            result["pilot_index"] = index
            handle.write(json.dumps(result) + "\n")
            handle.flush()
            print(
                f"{row.va:#x} {result['symbolic']['verdict']} {result['symbolic']['seconds']:.1f}s",
                flush=True,
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
