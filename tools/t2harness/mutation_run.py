# SPDX-License-Identifier: GPL-3.0-or-later
"""Recorded-call mutation runs (T3822): targets, seeds, the original as oracle, labelled receipts.

    python3 -m tools.t2harness mutate --host <capture tsfp-bevy> --target floor|single-shape|class-c
        [--top N | --va 0x..] [--seed 3822] [--jobs 3] [--out docs/data/t2-mutation-harness]

Targets (read through their owners, rule 45):

- `floor`: recording receipts (`tools.t2capture.runner.receipt_root()`) whose only promote refusal
  (`tools.t2capture_promote.refusals`, receipt-echoing currents so only content counts) is the
  minimum sample count;
- `single-shape`: corpus functions with one shape whose CFG has more paths
  (`tools.t2capture.path_count`), heaviest executed-call weight first; mutations retry for other
  branches (no branch-condition solving: steering is by more mutants while shapes < min(3, paths));
- `class-c`: T3793 class c functions (never executed) with a static caller that has corpus entries;
  the caller's entry state (registers, read-set) is the seed (INFERRED call-site state).

Per function: up to `MAX_SEEDS` unflagged entries of the newest tree, `PER_BATCH` mutants per batch,
up to `BATCHES` batches until `construct.MIN_ENTRIES` distinct inputs and the shape target are
accepted. The accepted root records go into a run-local corpus judged by the unchanged
`python3 -m tools.t2capture.run`; receipts get `tier2_source: constructed-call` and `inputs: {kind:
fabricated, generator: tools.t2harness.mutate, seed, generator_version, mutated_from: [seed entry
digests]}`.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

from tools.t2corpus import keys
from tools.t2corpus.spool import decode_record, write_spool
from tools.t2harness import construct, mutate, receipts
from tools.t2harness import generate as gen

ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = Path("docs/data/t2-mutation-harness")
TARGETS = ("floor", "single-shape", "class-c")
MAX_SEEDS = 8
PER_BATCH = 24
BATCHES = 3
FILL_ROUNDS = 2  # T3826: refill rounds per batch for unsupplied_read refusals (--fill caller)
FILL_ENTRIES = 64  # caller entries whose read-sets form the fill memory
READ_FILL_LABEL = "caller-entries+xbe-image"  # receipts inputs.read_fill (INFERRED state)


@dataclass
class Job:
    va: int
    target: str
    seeds: list[mutate.Seed]
    stack_args: int
    shape_target: int
    seed_va: int  # the VA whose entries seed it (the caller for class-c)
    fill: mutate.FillSource | None = None  # T3826: unsupplied reads refilled from here


@dataclass
class Result:
    job: Job
    items: dict[int, mutate.Mutant] = field(default_factory=dict)
    outcomes: dict[int, construct.Outcome] = field(default_factory=dict)
    header: dict[str, object] = field(default_factory=dict)
    error: str = ""
    filled: set[int] = field(default_factory=set)  # input indices built by `mutate.filled`

    @property
    def accepted(self) -> list[construct.Outcome]:
        seen: set[str] = set()
        out = []
        for outcome in sorted(self.outcomes.values(), key=lambda o: o.index):
            if not outcome.accepted:
                continue
            digest = keys.input_digest(decode_record(outcome.raw))
            if digest not in seen:
                seen.add(digest)
                out.append(outcome)
        return out

    def refused(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for outcome in self.outcomes.values():
            if not outcome.accepted:
                key = outcome.reason.split(":", 1)[0]
                counts[key] = counts.get(key, 0) + 1
        return dict(sorted(counts.items()))

    def shapes(self) -> int:
        return len({keys.shape_key(decode_record(o.raw)) for o in self.accepted})


def recording_id(va: int, seed: int) -> str:
    text = f"constructed-call:{mutate.GENERATOR}/{mutate.GENERATOR_VERSION}:{seed}:{va:#010x}"
    return hashlib.sha256(text.encode()).hexdigest()


def refuse_unsupplied(
    items: dict[int, mutate.Mutant], outcomes: dict[int, construct.Outcome]
) -> None:
    """Accepted outcomes whose record read a byte the mutant did not supply become refusals."""
    for index, outcome in outcomes.items():
        if outcome.accepted:
            wild = mutate.unsupplied_read(items[index], outcome.raw)
            if wild is not None:
                outcome.accepted = False
                outcome.reason = f"unsupplied_read:{wild:#x}"


def run_job(
    host: Path,
    job: Job,
    workdir: Path,
    domain: tuple[int, int],
    seed: int,
    runner: construct.Runner = construct.run_host,
) -> Result:
    result = Result(job)
    contract = gen.Contract(va=job.va, stack_args=job.stack_args)
    per_seed = -(-PER_BATCH // len(job.seeds))
    for number in range(BATCHES):
        batch = mutate.mutants(
            job.seeds, job.va, job.stack_args, seed, PER_BATCH,
            first_number=number * per_seed, first_index=len(result.items),
        )  # fmt: skip
        result.items.update({item.index: item for item in batch})
        (workdir / f"batch{number}").mkdir(parents=True, exist_ok=True)
        try:
            outcomes, header = construct.run_round(
                host, contract, batch, {}, workdir / f"batch{number}", seed, domain, runner,
                encode=mutate.encode_input,
            )  # fmt: skip
        except construct.HarnessError as exc:
            result.error = str(exc)[-300:]
            break
        refuse_unsupplied(result.items, outcomes)
        result.outcomes.update(outcomes)
        result.header = result.header or header
        if job.fill is not None:
            outcomes = {
                **outcomes,
                **fill_rounds(
                    host, job, result, outcomes, workdir / f"batch{number}", seed, domain, runner
                ),
            }
        if len(result.accepted) >= construct.MIN_ENTRIES and result.shapes() >= job.shape_target:
            break
        if not any(o.accepted or o.reason.startswith("unsupplied") for o in outcomes.values()):
            break  # every mutant faulted, hung or left no frame: more of the same will too
    return result


def fill_rounds(
    host: Path,
    job: Job,
    result: Result,
    outcomes: dict[int, construct.Outcome],
    workdir: Path,
    seed: int,
    domain: tuple[int, int],
    runner: construct.Runner,
) -> dict[int, construct.Outcome]:
    """Rerun `unsupplied_read` refusals with the missing bytes from `job.fill` (T3826), up to
    FILL_ROUNDS times (a filled input may read further unsupplied bytes). The original decides
    again; an input with a byte the fill source lacks stays refused."""
    assert job.fill is not None
    contract = gen.Contract(va=job.va, stack_args=job.stack_args)
    latest = outcomes
    done: dict[int, construct.Outcome] = {}
    for number in range(FILL_ROUNDS):
        batch = []
        for index, outcome in sorted(latest.items()):
            if outcome.accepted or not outcome.reason.startswith("unsupplied_read"):
                continue
            item = mutate.filled(
                result.items[index], outcome.raw, job.fill, len(result.items) + len(batch)
            )
            if item is not None:
                batch.append(item)
        if not batch:
            break
        result.items.update({item.index: item for item in batch})
        directory = workdir / f"fill{number}"
        directory.mkdir(parents=True, exist_ok=True)
        try:
            latest, _header = construct.run_round(
                host, contract, batch, {}, directory, seed, domain, runner,
                encode=mutate.encode_input,
            )  # fmt: skip
        except construct.HarnessError as exc:
            result.error = str(exc)[-300:]
            break
        refuse_unsupplied(result.items, latest)
        result.outcomes.update(latest)
        result.filled |= {item.index for item in batch}
        done.update(latest)
    return done


def reference_spool(result: Result, seed: int, out: Path) -> int:
    header = dict(result.header)
    header.update(
        recording_sha256=recording_id(result.job.va, seed),
        tier2_source=receipts.TIER2_SOURCE,
        generator=mutate.GENERATOR,
        generator_version=mutate.GENERATOR_VERSION,
        seed=seed,
    )
    for key in ("build_input_hash", "lifted_tree_hash", "created", "tool_version"):
        header.setdefault(key, "")
    raws = [o.raw for o in result.accepted]
    write_spool(out, header, raws)
    return len(raws)


def label(receipt: dict[str, object], seed: int, mutated_from: list[str]) -> dict[str, object]:
    """The runner receipt with the constructed-call label and the seed entries it came from."""
    if not mutated_from:
        raise ValueError("a mutation receipt needs at least one mutated_from digest")
    out = receipts.label(receipt, seed, generator=mutate.GENERATOR)
    out["inputs"] = {
        **dict(out["inputs"]),  # type: ignore[arg-type]
        "generator_version": mutate.GENERATOR_VERSION,
        "mutated_from": sorted(set(mutated_from)),
    }
    return out


# Targets -------------------------------------------------------------------------------------


def _echo_currents(receipt: dict) -> object:
    from tools.t2capture_promote import Currents

    return Currents(
        compare_logic=lambda: receipt.get("compare_logic"),
        lifted_tree_hash=lambda: receipt.get("lifted_tree_hash"),
        port_source_commit=lambda _module: receipt.get("port_source_commit"),
    )


def floor_vas(receipt_dir: Path) -> list[int]:
    """Recording receipts refused for the minimum sample count only."""
    from tools.t2capture_promote import refusals

    out = []
    for path in sorted(receipt_dir.glob("0x*.json")):
        receipt = json.loads(path.read_text())
        if receipt.get("tier2_source") not in (None, "capture"):
            continue
        found = refusals(receipt, _echo_currents(receipt))
        if found and all("minimum sample count" in reason for reason in found):
            out.append(int(path.stem, 16))
    return out


def single_shape_vas(corpus: object, tree: str, receipt_dir: Path) -> list[int]:
    """One corpus shape, more CFG paths, an equal recording receipt (the port runs)."""
    from tools.t2capture import path_count

    paths = path_count.load(tree)
    out = []
    for va, stat in corpus.function_stats(tree).items():  # type: ignore[attr-defined]
        if stat["shapes"] != 1 or paths.get(va, path_count.CAP) < 2:
            continue
        path = receipt_dir / f"0x{va:08X}.json"
        if path.is_file() and json.loads(path.read_text()).get("status") == "equal":
            out.append(va)
    return out


def class_c_callers(corpus: object, tree: str) -> dict[int, int]:
    """Class c VA -> a static caller with corpus entries (the most entries)."""
    from tools.t2reach import audit

    with audit.csv_path().open(newline="") as handle:
        rows = [int(r["va"], 16) for r in csv.DictReader(handle) if r["class"] == "c"]
    graph, _reach, _pins = audit.load_graph(audit.find_xbe(None))
    callers = graph.callers()
    stats = corpus.function_stats(tree)  # type: ignore[attr-defined]
    out = {}
    for va in rows:
        live = [c for c in callers.get(va, ()) if c in stats]
        if live:
            out[va] = max(live, key=lambda c: (stats[c]["entries"], -c))
    return out


def load_seeds(corpus: object, tree: str, va: int, limit: int = MAX_SEEDS) -> list[mutate.Seed]:
    rows = [r for r in corpus.entry_rows(tree, va) if not r["flags"]]  # type: ignore[attr-defined]
    rows.sort(key=lambda r: r["digest"])
    return [
        mutate.Seed(r["digest"], decode_record(corpus.raw_record(r)))  # type: ignore[attr-defined]
        for r in rows[:limit]
    ]


def jobs_for(args: argparse.Namespace, corpus: object, tree: str) -> list[Job]:
    from tools.t2capture import path_count, runner
    from tools.t2harness import targets

    receipt_dir = args.receipts or runner.receipt_path(runner.receipt_root(), 0).parent
    weights = runner.exec_weights()
    paths = path_count.load(tree)
    callers: dict[int, int] = {}
    if args.va:
        vas = [int(va, 16) for va in args.va]
    elif args.target == "floor":
        vas = floor_vas(receipt_dir)
    elif args.target == "single-shape":
        vas = single_shape_vas(corpus, tree, receipt_dir)
    else:
        callers = class_c_callers(corpus, tree)
        vas = sorted(callers)
    if args.target == "class-c" and args.va:
        callers = {va: c for va, c in class_c_callers(corpus, tree).items() if va in vas}
        vas = sorted(callers)
    weight = (lambda va: weights.get(callers.get(va, va), 0)) if callers else weights.get
    vas.sort(key=lambda va: (-(weight(va) or 0), va))
    gen_dir = targets.gen_dir(ROOT)
    image = targets.image_reader() if getattr(args, "fill", None) == "caller" and callers else None
    out = []
    for va in vas:
        seed_va = callers.get(va, va)
        seeds = load_seeds(corpus, tree, seed_va)
        if not seeds:
            continue
        try:
            stack_args = targets.contract(gen_dir, va).stack_args
        except SystemExit:
            continue
        shape_target = min(3, paths.get(va, path_count.CAP))
        fill = None
        if image is not None:
            memory = mutate.caller_memory(load_seeds(corpus, tree, seed_va, FILL_ENTRIES))
            fill = mutate.FillSource(memory, image)
        out.append(Job(va, args.target, seeds, stack_args, shape_target, seed_va, fill))
        if args.top and len(out) >= args.top:
            break
    return out


# Command -------------------------------------------------------------------------------------


def facts(result: Result) -> dict[str, object]:
    job = result.job
    accepted = result.accepted
    out: dict[str, object] = {
        "target": job.target,
        "seed_va": f"0x{job.seed_va:08X}",
        "seeds": len(job.seeds),
        "stack_args": job.stack_args,
        "shape_target": job.shape_target,
        "inputs": len(result.outcomes),
        "accepted": len(accepted),
        "refused": result.refused(),
        "shapes": result.shapes() if accepted else 0,
        "status": "constructed" if accepted else "all_inputs_refused",
    }
    if result.job.fill is not None:
        out["filled_accepted"] = sum(1 for o in accepted if o.index in result.filled)
    if result.error:
        out.update(status="harness_error", detail=result.error)
    return out


def cmd_mutate(args: argparse.Namespace) -> int:
    from tools import datadir, datadisk
    from tools.t2capture import runner
    from tools.t2capture.continuous import DEFAULT_STORE
    from tools.t2corpus.spool import read_spool
    from tools.t2corpus.store import Corpus
    from tools.t2harness.__main__ import judge

    datadisk.no_core_dumps()
    shared = Corpus(args.corpus or datadir.data_dir(DEFAULT_STORE))
    tree = shared.newest_tree() or ""
    try:
        chosen = jobs_for(args, shared, tree)
    finally:
        shared.close()
    work = args.work or datadir.data_dir("t2harness") / time.strftime("mutate-%Y%m%d-%H%M%S")
    domain = construct.guest_range(ROOT)
    print(f"t2harness mutate: {len(chosen)} {args.target} functions, work {work}", flush=True)
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        results = list(
            pool.map(
                lambda job: run_job(
                    args.host, job, work / runner.va_text(job.va), domain, args.seed
                ),
                chosen,
            )
        )
    store = work / "corpus"
    corpus = Corpus(store)
    summary_facts: dict[str, dict[str, object]] = {}
    judged: dict[int, Result] = {}
    try:
        for result in results:
            key = runner.va_text(result.job.va)
            summary_facts[key] = facts(result)
            if result.accepted:
                spool = work / key / "reference.t2cap"
                reference_spool(result, args.seed, spool)
                corpus.ingest(read_spool(spool))
                judged[result.job.va] = result
    finally:
        corpus.close()
    judge(args.host, store, work, sorted(judged), args.jobs)
    written = 0
    for va, result in judged.items():
        key = runner.va_text(va)
        path = runner.receipt_path(work, va)
        if not path.is_file():
            summary_facts[key].update(status="no_receipt")
            continue
        origins = [result.items[o.index].seed_digest for o in result.accepted]
        receipt = label(json.loads(path.read_text()), args.seed, origins)
        if any(o.index in result.filled for o in result.accepted):
            receipt["inputs"] = {**dict(receipt["inputs"]), "read_fill": READ_FILL_LABEL}  # type: ignore[arg-type]
        receipts.write(args.out, receipt)
        written += 1
        counts = dict(receipt.get("counts", {}))
        summary_facts[key].update(
            status=receipt.get("status"),
            compared=counts.get("compared", 0),
            equal=counts.get("equal", 0),
            diverged=counts.get("diverged", 0),
            mutated_from=len(set(origins)),
        )
    summary = receipts.merge_summary(
        args.out, summary_facts, task="T3822", generator=mutate.GENERATOR,
        generator_version=mutate.GENERATOR_VERSION,
    )  # fmt: skip
    print(f"t2harness mutate: {len(chosen)} functions, {written} receipts")
    print("status:", json.dumps(summary["status_counts"]))
    print("refused inputs:", json.dumps(summary["refused_inputs"]))
    print("totals:", json.dumps(summary["totals"]))
    return 0


def add_parser(sub: argparse._SubParsersAction) -> None:
    run = sub.add_parser("mutate", help="recorded-call mutation (T3822)")
    run.add_argument("--host", type=Path, required=True, help="TSFP_BUILD_CAPTURE=1 tsfp-bevy")
    run.add_argument("--target", choices=TARGETS, required=True)
    run.add_argument("--top", type=int, help="the N heaviest by executed-call weight")
    run.add_argument("--va", action="append", help="function 0x.. (repeatable)")
    run.add_argument("--seed", type=int, default=mutate.DEFAULT_SEED)
    run.add_argument("--jobs", type=int, default=3)
    run.add_argument("--out", type=Path, default=ROOT / OUT_DIR)
    run.add_argument("--corpus", type=Path, help="seed corpus (default: the full-pass store)")
    run.add_argument("--receipts", type=Path, help="recording receipts (default: runner root)")
    run.add_argument("--work", type=Path)
    run.add_argument(
        "--fill",
        choices=("caller",),
        help="class-c: rerun unsupplied_read refusals with bytes from the caller's recorded "
        "read-sets and the XBE image (T3826, INFERRED call-site state)",
    )
    run.set_defaults(func=cmd_mutate)
