# SPDX-License-Identifier: GPL-3.0-or-later
"""Live targeted capture (T3833): `sh tools/dev/play.sh --capture-targets [auto|FILE]`.

The owner plays the normal game on a capture host (`TSFP_BUILD_CAPTURE=1 TSFP_CAPTURE_KIND=play`).
Only the VAs of an ALLOW list (TSFP_T2_CAPTURE_TARGETS, src/host/t2_capture.c) serialize frames, a
target Rust root runs its lifted original (src/host/rust_ports.c), everything else runs the default
ports and records nothing. The T3803 budget still applies to the targets.

    python3 -m tools.t2capture.live_targets targets [--out FILE] [--json]
        the auto list and its count per source
    python3 -m tools.t2capture.live_targets prepare --run-dir D --targets auto|FILE
        --build-sha SHA --ports SPEC
        write D/capture-targets.txt and D/capture-budget-seed.txt, print the host environment
        as `export` lines (play.sh evals them)
    python3 -m tools.t2capture.live_targets check-host HOST
        exit 2 with the reason when HOST is not a usable capture host
    python3 -m tools.t2capture.live_targets ingest --run-dir D [--store DIR]
        ingest D/capture.bin into tmp/t2corpus-f2 with live provenance,
        ledger D/t2corpus-ingest.json

The auto list is the union of
  coverage   `tools.t2corpus coverage` gaps of f2: executed port functions never captured or
             lacking shapes (min(3, CFG paths), the entry floor);
  floor      recording receipts refused only for the minimum sample count (the 3-call floor,
             tools.t2harness.mutation_run.floor_vas);
  reach      tools.t2reach class c (reachable by owner play) and e functions;
  live       `tools.live_census report` ran-live-uncaptured: a live session ran them, no replay did.
A source whose inputs are missing is reported on stderr and contributes nothing (rule 45).

Provenance: the spool header's recording_sha256 is `live:<run id>` (the run dir name), its
build_input_hash `live build=<sha> ports=<spec>+capture`. Each run is one ingest unit; the corpus
change detection of t2-continuous then picks the new entries up. A live entry is a real original
call of what the owner did, so no replay desync is involved. Run data stays owner-private.
"""

from __future__ import annotations

import argparse
import csv
import json
import shlex
import sys
from collections.abc import Callable, Mapping
from pathlib import Path

TARGETS_ENV = "TSFP_T2_CAPTURE_TARGETS"
TARGETS_NAME = "capture-targets.txt"
SEED_NAME = "capture-budget-seed.txt"
LEDGER_NAME = "t2corpus-ingest.json"
STORE = "t2corpus-f2"
LIVE_PREFIX = "live:"
SOURCES = ("coverage", "floor", "reach", "live", "queue")
REACH_CLASSES = frozenset({"c", "e"})


def _warn(message: str) -> None:
    print(f"live_targets: {message}", file=sys.stderr)


def coverage_vas() -> set[int]:
    from tools import datadir
    from tools.live_census import store as live_store
    from tools.t2capture import path_count
    from tools.t2corpus import __main__ as t2corpus_cli
    from tools.t2corpus import coverage as cov
    from tools.t2corpus.store import Corpus

    store = datadir.data_dir(STORE)
    if not store.is_dir():
        raise FileNotFoundError(f"corpus {store}")
    live = live_store.union_vas()
    live_systems = None
    if live:
        from tools.t2reach.audit import port_rows

        live_systems = {int(str(r["va"]), 16): str(r.get("subsystem") or "") for r in port_rows()}
    executed = cov.executed_port_functions(
        t2corpus_cli.WEIGHTS_PATH, t2corpus_cli.DISPOSITIONS_PATH, live, live_systems
    )
    corpus = Corpus(store)
    try:
        tree = corpus.newest_tree() or ""
        stats = corpus.function_stats(tree)
    finally:
        corpus.close()
    gaps = cov.coverage_gaps(
        executed, stats, cov.route_index(t2corpus_cli.ROUTES_PATH), 3, path_count.load(tree)
    )
    return {int(gap["va"], 16) for gap in gaps}


def floor_vas() -> set[int]:
    from tools.t2capture import runner
    from tools.t2harness import mutation_run

    receipts = runner.receipt_path(runner.receipt_root(), 0).parent
    if not receipts.is_dir():
        raise FileNotFoundError(f"receipt dir {receipts}")
    return set(mutation_run.floor_vas(receipts))


def reach_vas(csv_path: Path | None = None) -> set[int]:
    from tools.t2reach import audit

    path = csv_path or audit.csv_path()
    with Path(path).open(newline="") as handle:
        return {int(r["va"], 16) for r in csv.DictReader(handle) if r["class"] in REACH_CLASSES}


def live_vas() -> set[int]:
    from tools.live_census import store as live_store
    from tools.t2reach.audit import execution_evidence, port_rows

    collected = live_store.collect()
    if not collected.runs:
        return set()
    rows = port_rows()
    replay, _where, _counts = execution_evidence(rows, include_live=False)
    ports = {int(str(r["va"]), 16) for r in rows}
    return (set(live_store.union(collected.runs)) & ports) - set(replay)


def queue_vas() -> set[int]:
    from tools.t2capture import requests

    return requests.pending("capture")


LOADERS: dict[str, Callable[[], set[int]]] = {
    "coverage": coverage_vas,
    "floor": floor_vas,
    "reach": reach_vas,
    "live": live_vas,
    "queue": queue_vas,
}


def derive(loaders: Mapping[str, Callable[[], set[int]]] | None = None) -> dict[int, list[str]]:
    """{va: sources}: the auto ALLOW list. A failing source is named on stderr and skipped."""
    out: dict[int, list[str]] = {}
    for name, load in (loaders or LOADERS).items():
        try:
            vas = load()
        except (OSError, ValueError, KeyError, SystemExit) as exc:
            _warn(f"source {name} unavailable ({exc}); it adds no targets")
            continue
        for va in vas:
            if va:
                out.setdefault(va, []).append(name)
    return out


def render(targets: Mapping[int, list[str]]) -> str:
    lines = ["# T3833 capture ALLOW list: va [sources]"]
    lines += [f"{va:08x} {','.join(sorted(targets[va]))}" for va in sorted(targets)]
    return "\n".join(lines) + "\n"


def parse(text: str) -> set[int]:
    """VAs of a targets file: the first hex word of each line, `#` comments (0x optional)."""
    vas = set()
    for line in text.splitlines():
        word = line.split("#", 1)[0].split()
        if word:
            va = int(word[0], 16)
            if va:
                vas.add(va)
    return vas


def counts(targets: Mapping[int, list[str]]) -> dict[str, int]:
    found = {name: 0 for name in SOURCES}
    for names in targets.values():
        for name in names:
            found[name] = found.get(name, 0) + 1
    return {**found, "total": len(targets)}


def provenance(run_dir: Path, build_sha: str, ports: str) -> dict[str, str]:
    """Spool header fields of a live run (run id, build sha, port set)."""
    spec = ports.removeprefix("--rust-ports=")
    return {
        "TSFP_T2_CAPTURE_RECORDING_SHA256": f"{LIVE_PREFIX}{Path(run_dir).name}",
        "TSFP_T2_CAPTURE_BUILD_INPUT_HASH": (
            f"live build={build_sha or 'unknown'} ports={spec}+capture"
        ),
    }


def prepare(
    run_dir: Path,
    choice: str,
    build_sha: str,
    ports: str,
    loaders: Mapping[str, Callable[[], set[int]]] | None = None,
) -> tuple[dict[str, str], dict[str, int]]:
    """Write the run's ALLOW list and budget seed; (host environment, counts)."""
    from tools import datadir, datadisk
    from tools.t2capture import budget_seed
    from tools.t2capture.runtime_env import SPOOL_NAME

    run_dir = Path(run_dir)
    run_dir.mkdir(parents=True, exist_ok=True)
    if choice == "auto":
        targets = derive(loaders)
    else:
        source = Path(choice)
        if not source.is_file():
            raise ValueError(f"--capture-targets: {source} not found (auto or a file of hex VAs)")
        targets = {va: ["file"] for va in parse(source.read_text())}
    if not targets:
        raise ValueError("--capture-targets: the ALLOW list is empty, nothing would be captured")
    (run_dir / TARGETS_NAME).write_text(render(targets))
    env = {
        **datadisk.capture_env(),
        "TSFP_T2_CAPTURE": str((run_dir / SPOOL_NAME).resolve()),
        "TSFP_T2_CAPTURE_SEGMENT_BYTES": "0",  # one plain capture.bin, flushed per record
        TARGETS_ENV: str((run_dir / TARGETS_NAME).resolve()),
        **provenance(run_dir, build_sha, ports),
    }
    store = datadir.data_dir(STORE)
    try:
        budget_seed.write(run_dir / SEED_NAME, store)
        env[budget_seed.ENV] = str((run_dir / SEED_NAME).resolve())
    except (ValueError, OSError) as exc:  # no corpus yet: every target starts at 0
        _warn(f"no budget seed ({exc}); the budget counts this run only")
    return env, counts(targets)


def ingest(run_dir: Path, store: Path | None = None) -> dict[str, object]:
    """Ingest RUN/capture.bin (live provenance in its header); the ledger goes beside it."""
    from tools import datadir
    from tools.t2capture.runtime_env import SPOOL_NAME
    from tools.t2corpus.spool import read_spool
    from tools.t2corpus.store import Corpus

    spool = Path(run_dir) / SPOOL_NAME
    if not spool.is_file():
        raise FileNotFoundError(f"{spool}: the host wrote no capture (see host.err)")
    read = read_spool(spool)
    recording = str(read.header.get("recording_sha256", ""))
    if not recording.startswith(LIVE_PREFIX):
        raise ValueError(
            f"{spool}: recording {recording!r} is not a live run ({LIVE_PREFIX}<run id>)"
        )
    corpus = Corpus(store or datadir.data_dir(STORE))
    try:
        report = corpus.ingest(read)
    finally:
        corpus.close()
    ledger: dict[str, object] = {
        "recording_sha256": recording,
        "build_input_hash": read.header.get("build_input_hash"),
        "lifted_tree_hash": read.header.get("lifted_tree_hash"),
        "records": report.records,
        "kept": report.kept,
        "duplicates": report.duplicates,
        "functions": len(report.by_function),
        "truncated": report.truncated,
        "skipped_recording": report.skipped_recording,
    }
    (Path(run_dir) / LEDGER_NAME).write_text(json.dumps(ledger, indent=1, sort_keys=True) + "\n")
    return ledger


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2capture.live_targets", description=__doc__.splitlines()[0]
    )
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("targets", help="the auto ALLOW list")
    p.add_argument("--out", type=Path)
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("prepare", help="write a run's ALLOW list, print its host environment")
    p.add_argument("--run-dir", type=Path, required=True)
    p.add_argument("--targets", default="auto")
    p.add_argument("--build-sha", default="")
    p.add_argument("--ports", default="")
    p = sub.add_parser("check-host", help="refuse a non-capture host")
    p.add_argument("host", type=Path)
    p = sub.add_parser("ingest", help="ingest a run's capture.bin into the corpus")
    p.add_argument("--run-dir", type=Path, required=True)
    p.add_argument("--store", type=Path)
    args = parser.parse_args(argv)

    if args.cmd == "targets":
        targets = derive()
        if args.out:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(render(targets))
        print(
            json.dumps(counts(targets)) if args.json else render(targets),
            end="" if not args.json else "\n",
        )
        return 0
    if args.cmd == "check-host":
        from tools.t2capture.runtime_env import capture_host_problem

        problem = capture_host_problem(args.host)
        if problem:
            print(f"play.sh --capture-targets: {problem}", file=sys.stderr)
            return 2
        return 0
    if args.cmd == "prepare":
        try:
            env, found = prepare(args.run_dir, args.targets, args.build_sha, args.ports)
        except ValueError as exc:
            print(exc, file=sys.stderr)
            return 2
        for key, value in sorted(env.items()):
            print(f"export {key}={shlex.quote(value)}")
        summary = ", ".join(
            f"{name} {found[name]}" for name in (*SOURCES, "file") if found.get(name)
        )
        total = found["total"]
        print(f"echo {shlex.quote(f'capture targets: {total} VAs ({summary})')}")
        return 0
    try:
        ledger = ingest(args.run_dir, args.store)
    except (OSError, ValueError) as exc:
        print(f"live_targets: ingest failed: {exc}", file=sys.stderr)
        return 1
    print(
        f"capture targets: ingested {ledger['kept']} of {ledger['records']} records over "
        f"{ledger['functions']} functions into {STORE} ({ledger['recording_sha256']})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
