# SPDX-License-Identifier: GPL-3.0-or-later
"""python3 -m tools.t2capture.run: run --va/--all and summary (T3743)."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

from tools import datadir
from tools.t2capture import runner
from tools.t2corpus.store import Corpus


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python3 -m tools.t2capture.run", description=__doc__)
    parser.add_argument("--receipt-root", type=Path, help="checkout holding tmp/wt-cache/receipts")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("run", help="compare corpus entries against the Rust ports")
    which = p.add_mutually_exclusive_group(required=True)
    which.add_argument("--va", action="append", help="function 0x.. (repeatable, T3784)")
    which.add_argument("--all", action="store_true", help="every corpus VA by call weight")
    p.add_argument("--host", type=Path, help="host executable (default $TSFP_T2_HOST)")
    p.add_argument("--store", type=Path, help="corpus dir (default: data_dir('t2corpus'))")
    p.add_argument("--lifted-tree", help="lifted tree hash (default: newest ingested)")
    p.add_argument("--jobs", type=int, default=1)
    p.add_argument("--min-samples", type=int, default=0)
    p.add_argument("--timeout", type=float, default=runner.DEFAULT_TIMEOUT)
    p.add_argument("--build-input-hash", help="host build input hash (default: HEAD's)")
    p = sub.add_parser("summary", help="per-status counts and top divergences")
    p.add_argument("--json", action="store_true")
    p.add_argument("--path", type=Path, default=runner.SUMMARY_PATH)
    p.add_argument("--no-write", action="store_true")
    return parser


def _cmd_run(args: argparse.Namespace) -> int:
    host = args.host or (Path(os.environ["TSFP_T2_HOST"]) if "TSFP_T2_HOST" in os.environ else None)
    if host is None:
        print("no host: pass --host or set TSFP_T2_HOST", file=sys.stderr)
        return 2
    try:
        runner.preflight(host)
    except runner.HostRefused as exc:
        print(f"t2capture: {exc}", file=sys.stderr)
        return 2
    store = args.store or datadir.data_dir("t2corpus")
    root = runner.receipt_root(args.receipt_root)
    corpus = Corpus(store)
    try:
        tree = args.lifted_tree or corpus.newest_tree() or ""
    finally:
        corpus.close()
    options = {
        "timeout": args.timeout,
        "build_input_hash": args.build_input_hash or runner.default_build_input_hash(),
    }
    if args.all:
        receipts = runner.run_all(
            store, tree, host, root, jobs=args.jobs, min_samples=args.min_samples, **options
        )
    else:
        receipts = runner.run_vas(
            store, tree, [int(va, 16) for va in args.va], host, root,
            jobs=args.jobs, min_samples=args.min_samples, **options,
        )  # fmt: skip
    for receipt in receipts:
        print(
            f"{receipt['va']} {receipt['status']} {json.dumps(receipt['counts'].get('fidelity'))}"
        )
    return 0


def _cmd_summary(args: argparse.Namespace) -> int:
    root = runner.receipt_root(args.receipt_root)
    summary = runner.build_summary(root, runner.exec_weights())
    text = json.dumps(summary, indent=1, sort_keys=True) + "\n"
    if not args.no_write:
        args.path.write_text(text)
    if args.json:
        print(text, end="")
    else:
        print("status counts:", json.dumps(summary["status_counts"]))
        for fn in summary["top_divergences"]:
            print(f"  {fn['va']} calls={fn['calls']} class={fn['class']} {fn['port_module']}")
    return 0


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    return _cmd_run(args) if args.cmd == "run" else _cmd_summary(args)


if __name__ == "__main__":
    sys.exit(main())
