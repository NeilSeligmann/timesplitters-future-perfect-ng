# SPDX-License-Identifier: GPL-3.0-or-later
"""python3 -m tools.t2harness: constructed-call Tier 2 for class d functions (T3796).

    python3 -m tools.t2harness run --host <capture tsfp-bevy> (--top N | --va 0x.. ... | --all)
        [--seed 3796] [--jobs 4] [--out docs/data/t3793-harness] [--work DIR]
    python3 -m tools.t2harness recheck (--changed | --va 0x..) [--host <capture host>] [--dry-run]
        [--out DIR] [--into DIR] [--limit N] [--skip-key FILE]   (T3826: mutation receipts too)
    python3 -m tools.t2harness check [--out docs/data/t3793-harness]
    python3 -m tools.t2harness mutate --host <capture tsfp-bevy> --target floor|single-shape|class-c
        [--top N | --va 0x..] [--seed 3822] [--out docs/data/t2-mutation-harness]   (T3822)

`run`: class d rows (tools.t2reach) minus recording-credited VAs, heaviest static fan-in first; per
function fabricate inputs, run the original in the capture host (`--t2-construct`), ingest the
accepted reference entries into a run-local corpus, judge the ports with the unchanged `python3 -m
tools.t2capture.run`, and write labelled receipts to --out plus `<out>-summary.json`. `check`: every
receipt in --out carries the label.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

from tools.t2harness import construct, receipts, targets
from tools.t2harness import generate as gen

ROOT = Path(__file__).resolve().parents[2]
RUNNER_SECONDS_PER_VA = 60
# The runner's own per-function limit. The outer timeout must cover it, or a slow function kills
# the whole judge instead of being recorded as timed out (I-T3865-13: 2 floor VAs, outer 240 s
# < inner 300 s, the harness stage failed every pass).
RUNNER_FUNCTION_TIMEOUT_S = 300


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python3 -m tools.t2harness", description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)
    run = sub.add_parser("run", help="construct, judge and label class d functions")
    which = run.add_mutually_exclusive_group(required=True)
    which.add_argument("--top", type=int, help="the N heaviest by static fan-in")
    which.add_argument("--va", action="append", help="function 0x.. (repeatable)")
    which.add_argument("--all", action="store_true")
    run.add_argument(
        "--skip-done", action="store_true", help="leave out VAs already in the summary"
    )
    run.add_argument(
        "--host",
        type=Path,
        required=True,
        help="TSFP_BUILD_CAPTURE=1 tsfp-bevy with --t2-construct",
    )
    run.add_argument("--seed", type=int, default=gen.DEFAULT_SEED)
    run.add_argument("--jobs", type=int, default=4)
    run.add_argument("--out", type=Path, default=ROOT / receipts.OUT_DIR)
    run.add_argument(
        "--work", type=Path, help="scratch dir (default: data_dir('t2harness')/run-<time>)"
    )
    run.add_argument(
        "--csv", type=Path, help="class d CSV (default: tools.t2reach.audit.csv_path())"
    )
    recheck = sub.add_parser(
        "recheck", help="rerun functions whose receipt is stale (port, compare logic or lift)"
    )
    pick = recheck.add_mutually_exclusive_group(required=True)
    pick.add_argument("--changed", action="store_true", help="every receipt in --out gone stale")
    pick.add_argument("--va", action="append", help="function 0x.. (repeatable)")
    recheck.add_argument("--host", type=Path, help="capture host (needed unless --dry-run)")
    recheck.add_argument("--dry-run", action="store_true", help="list the stale VAs only")
    recheck.add_argument("--seed", type=int, default=gen.DEFAULT_SEED)
    recheck.add_argument("--jobs", type=int, default=4)
    recheck.add_argument("--out", type=Path, default=ROOT / receipts.OUT_DIR)
    recheck.add_argument("--work", type=Path)
    recheck.add_argument("--csv", type=Path)
    recheck.add_argument(
        "--into", type=Path, help="write the fresh receipts and summary here (default --out)"
    )
    recheck.add_argument("--limit", type=int, help="rerun at most N stale functions (T3826)")
    recheck.add_argument(
        "--skip-key",
        type=Path,
        help="JSON {va: stale key} of reruns already tried at that staleness: skipped (T3826)",
    )
    check = sub.add_parser("check", help="every receipt carries the constructed-call label")
    check.add_argument("--out", type=Path, default=ROOT / receipts.OUT_DIR)
    from tools.t2harness import mutation_run

    mutation_run.add_parser(sub)
    return parser


def select(args: argparse.Namespace) -> list[targets.Target]:
    rows = targets.class_d_rows(args.csv)
    skip = targets.recording_credit(ROOT)
    if args.skip_done and receipts.summary_path(args.out).is_file():
        done = json.loads(receipts.summary_path(args.out).read_text()).get("functions", {})
        skip |= {int(va, 16) for va in done}
    if args.va:
        wanted = {int(va, 16) for va in args.va}
        rows = [row for row in rows if int(row["va"], 16) in wanted]
        missing = wanted - {int(row["va"], 16) for row in rows}
        if missing:
            raise SystemExit(
                f"t2harness: not class d: {', '.join(hex(va) for va in sorted(missing))}"
            )
    vas = [int(row["va"], 16) for row in rows]
    ordered = targets.ordered(rows, targets.fan_in(vas), skip)
    return ordered[: args.top] if args.top else ordered


def status_without_receipt(result: construct.FunctionResult) -> str:
    """No accepted input: `no_capture_frame` when no input produced a root record (the host reaches
    the VA through hand-written host code, there is no lifted body to take a reference from), else
    `all_inputs_refused`."""
    if result.outcomes and all(o.reason == "no_record" for o in result.outcomes.values()):
        return "no_capture_frame"
    return "all_inputs_refused"


def judge_timeout(count: int, jobs: int) -> float:
    """Whole-runner limit: every batch of `jobs` functions may run to the per-function limit."""
    batches = -(-count // max(1, jobs))
    return 120 + max(RUNNER_SECONDS_PER_VA, RUNNER_FUNCTION_TIMEOUT_S) * batches


def judge(host: Path, store: Path, root: Path, vas: list[int], jobs: int) -> None:
    """The unchanged rule 43 runner over the run-local corpus."""
    if not vas:
        return
    argv = [sys.executable, "-m", "tools.t2capture.run", "--receipt-root", str(root), "run"]
    for va in vas:
        argv += ["--va", f"{va:#010x}"]
    argv += ["--host", str(host), "--store", str(store), "--jobs", str(jobs)]
    argv += ["--timeout", str(RUNNER_FUNCTION_TIMEOUT_S)]
    timeout = judge_timeout(len(vas), jobs)
    done = subprocess.run(argv, cwd=ROOT, capture_output=True, text=True, timeout=timeout)
    if done.returncode:
        raise SystemExit(f"t2harness: runner rc {done.returncode}: {done.stderr[-800:]}")


def cmd_run(args: argparse.Namespace) -> int:
    from tools import datadir, datadisk

    # Fabricated inputs kill the host now and then (I-T3796-1): no core to the host's root disk.
    datadisk.no_core_dumps()
    from tools.t2capture import runner
    from tools.t2corpus.spool import read_spool
    from tools.t2corpus.store import Corpus

    chosen = select(args)
    work = args.work or datadir.data_dir("t2harness") / time.strftime("run-%Y%m%d-%H%M%S")
    store = work / "corpus"
    gen_dir = targets.gen_dir(ROOT)
    image_page = targets.image_reader()
    domain = construct.guest_range(ROOT)
    facts: dict[str, dict[str, object]] = {}
    judged: list[int] = []
    corpus = Corpus(store)
    try:
        for target in chosen:
            key = runner.va_text(target.va)
            base = {"fan_in": target.fan_in, "subsystem": target.subsystem}
            try:
                contract = targets.contract(gen_dir, target.va)
            except SystemExit as exc:
                facts[key] = {**base, "status": "no_lifted_body", "detail": str(exc)}
                continue
            try:
                result = construct.construct(
                    args.host, contract, work / key, image_page, domain, args.seed
                )
            except construct.HarnessError as exc:
                facts[key] = {**base, "status": "harness_error", "detail": str(exc)[-300:]}
                continue
            accepted = len(result.accepted)
            facts[key] = {
                **base,
                "stack_args": contract.stack_args,
                "paths_hint": contract.paths_hint,
                "shape_target": contract.shape_target,
                "inputs": len(result.outcomes),
                "accepted": accepted,
                "refused": result.refused(),
                "shapes": result.shapes() if accepted else 0,
                "rounds": result.rounds,
                "global_pages": result.global_pages,
                "status": status_without_receipt(result) if not accepted else "constructed",
            }
            if accepted:
                spool = work / key / "reference.t2cap"
                construct.reference_spool(result, args.seed, spool)
                corpus.ingest(read_spool(spool))
                judged.append(target.va)
    finally:
        corpus.close()
    judge(args.host, store, work, judged, args.jobs)
    written = 0
    for va in judged:
        key = runner.va_text(va)
        path = runner.receipt_path(work, va)
        if not path.is_file():
            facts[key].update(status="no_receipt")
            continue
        receipt = receipts.label(json.loads(path.read_text()), args.seed)
        receipts.write(args.out, receipt)
        written += 1
        counts = dict(receipt.get("counts", {}))
        facts[key].update(
            status=receipt.get("status"),
            compared=counts.get("compared", 0),
            equal=counts.get("equal", 0),
            diverged=counts.get("diverged", 0),
            capture_unfaithful=dict(counts.get("fidelity", {})).get("capture_unfaithful", 0),
        )
    summary = receipts.merge_summary(args.out, facts)
    print(f"t2harness: {len(chosen)} functions, {written} receipts, work {work}")
    print(
        "status:",
        json.dumps(summary["status_counts"]),
        "refused inputs:",
        json.dumps(summary["refused_inputs"]),
    )
    print("totals:", json.dumps(summary["totals"]))
    return 0


def cmd_check(args: argparse.Namespace) -> int:
    found = receipts.check(args.out)
    for line in found:
        print(f"t2harness check: {line}", file=sys.stderr)
    count = len(list(args.out.glob("0x*.json")))
    print(f"t2harness check: {count} receipts, {len(found)} problems")
    return 1 if found else 0


def stale_receipts(out: Path, currents: object) -> dict[str, list[str]]:
    """VA -> why its receipt in `out` no longer describes the tree (stale ones only)."""
    stale = {}
    for path in sorted(out.glob("0x*.json")):
        reasons = receipts.stale(json.loads(path.read_text()), currents)  # type: ignore[arg-type]
        if reasons:
            stale[path.stem] = reasons
    return stale


def mutation_targets(out: Path, vas: list[str]) -> dict[str, list[str]]:
    """target -> VAs for the T3822 mutation receipts among `vas` (from the receipt's generator
    and the summary's `target`), so a recheck reruns each the way it was credited."""
    from tools.t2harness import mutate

    facts = {}
    if receipts.summary_path(out).is_file():
        facts = json.loads(receipts.summary_path(out).read_text()).get("functions", {})
    grouped: dict[str, list[str]] = {}
    for va in vas:
        path = out / f"{va}.json"
        inputs = json.loads(path.read_text()).get("inputs", {}) if path.is_file() else {}
        if dict(inputs).get("generator") != mutate.GENERATOR:
            continue
        target = str(dict(facts.get(va, {})).get("target") or "floor")
        grouped.setdefault(target, []).append(va)
    return grouped


TRIED_SCHEMA = 2  # I-T3839-1: the flat file recorded keys BEFORE the run, so a failed run hid them


def load_tried(path: Path | None) -> dict[str, str]:
    """VA -> stale key whose rerun FINISHED without a current receipt (refused). A legacy flat
    file (keys recorded before the run, kept by failed runs) is ignored: every key is retried."""
    if path is None or not path.is_file():
        return {}
    try:
        doc = json.loads(path.read_text())
    except ValueError:
        return {}
    if not isinstance(doc, dict) or doc.get("schema") != TRIED_SCHEMA:
        return {}
    return {str(va): str(key) for va, key in dict(doc.get("tried") or {}).items()}


def save_tried(path: Path, tried: dict[str, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    doc = {"schema": TRIED_SCHEMA, "tried": dict(sorted(tried.items()))}
    path.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n")


def refused_after(into: Path, keys: dict[str, str], currents: object) -> dict[str, str]:
    """The VAs of a finished rerun that still have no current receipt in `into` (refused)."""
    left = {}
    for va, key in keys.items():
        path = into / f"{va}.json"
        receipt = json.loads(path.read_text()) if path.is_file() else {}
        if not receipt or receipts.stale(receipt, currents):  # type: ignore[arg-type]
            left[va] = key
    return left


def cmd_recheck(args: argparse.Namespace) -> int:
    """Re-run harness credit after a port change (the harness owns this, not t2-continuous):
    stale = the port's source file, the compare logic or the lift moved since the receipt.
    T3796 receipts rerun through `run`, T3822 mutation receipts through `mutate` with their
    recorded target. A rerun that yields no receipt leaves the old one (and its credit).
    `--skip-key`: a stale key is skipped only after a rerun of it FINISHED without a current
    receipt; a failed or killed run records nothing (I-T3839-1)."""
    from tools.t2capture_promote import real_currents

    keys: dict[str, str] = {}
    currents = real_currents(ROOT)
    refused = 0
    if args.changed:
        stale = stale_receipts(args.out, currents)
        tried = load_tried(args.skip_key)
        vas = []
        for va, reasons in stale.items():
            keys[va] = "; ".join(reasons)
            if tried.get(va) == keys[va]:
                refused += 1
                continue
            vas.append(va)
            print(f"stale {va}: {keys[va]}")
    else:
        vas = [f"0x{int(va, 16):08X}" for va in args.va]
    if args.limit is not None:
        vas = vas[: max(0, args.limit)]
    print(f"t2harness recheck: {len(vas)} function(s) to rerun")
    print(f"t2harness recheck: {refused} stale function(s) refused at their key (credit kept)")
    if args.dry_run or not vas:
        return 0
    if args.host is None:
        raise SystemExit("t2harness recheck: --host is required unless --dry-run")
    into = args.into or args.out
    grouped = mutation_targets(args.out, vas)
    mutated = {va for group in grouped.values() for va in group}
    status = 0
    from tools.t2harness import mutation_run

    for target, group in sorted(grouped.items()):
        status |= mutation_run.cmd_mutate(
            argparse.Namespace(
                host=args.host, target=target, top=None, va=group,
                seed=mutation_run.mutate.DEFAULT_SEED,
                jobs=args.jobs, out=into, corpus=None, receipts=None, fill=None,
                work=args.work / f"mutate-{target}" if args.work else None,
            )
        )  # fmt: skip
    rest = [va for va in vas if va not in mutated]
    # I-T3839-1: a VA that left class d since its receipt (recording corpus, a new row class)
    # made `select` refuse the whole batch; it is not this harness's to rerun (refused below)
    class_d = {int(row["va"], 16) for row in targets.class_d_rows(args.csv)}
    gone = [va for va in rest if int(va, 16) not in class_d]
    if gone:
        print(f"t2harness recheck: {len(gone)} stale function(s) no longer class d: {gone[:8]}")
    rest = [va for va in rest if int(va, 16) in class_d]
    if rest:
        run_args = argparse.Namespace(
            top=None, all=False, va=rest, skip_done=False, host=args.host, seed=args.seed,
            jobs=args.jobs, out=into, work=args.work, csv=args.csv,
        )  # fmt: skip
        status |= cmd_run(run_args)
    if args.skip_key is not None and status == 0:  # finished: its refusals are not retried
        tried = load_tried(args.skip_key)
        tried.update(refused_after(into, {va: keys[va] for va in vas if va in keys}, currents))
        save_tried(args.skip_key, tried)
    return status


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    commands = {"run": cmd_run, "recheck": cmd_recheck, "check": cmd_check}
    return commands[args.cmd](args) if args.cmd in commands else args.func(args)


if __name__ == "__main__":
    sys.exit(main())
