# SPDX-License-Identifier: GPL-3.0-or-later
"""Run the ORIGINAL lifted code on fabricated inputs and keep its capture records as reference
entries (T3796).

Per function: write the generator's inputs as a spool, run `<capture host> --t2-construct <inputs>
--va <va> --out <results.jsonl>` with `TSFP_T2_CAPTURE=<spool>`, and pair every capture record of
the root call with its input through the return-address tag. An input is REFUSED, never used to
judge a port, when the original:

- faulted or panicked (`fault`), killed the host (`host_death`) or ran past the timeout (`timeout`);
- produced a record with a capture_incomplete flag (`flag:<name>`, e.g. mmio or self_modifying);
- produced no root record at all (`no_record`; when no input of a function has one, the host reaches
  that VA through hand-written host code, not a lifted body with a capture frame);
- read or wrote outside the guest range the ports accept (`outside_guest`: `anim_lane::guest_ok`,
  below 64 KiB or from 2 GiB up). The arena answered such an access with poison, the game would have
  faulted (null or wild pointer), and the port's lane guard refuses it: never part of the
  constructed domain.

Global pages: a first round runs with stack, heap and TIB only. Pages of the XBE image any root
record read anyway (it got the arena's 0xCD poison; refused records count too) are then provided
(`generate.global_page`) and the round is repeated, at most `MAX_ROUNDS` times. Image bytes go only
to the uncommitted store (`tools.datadir`), never to git.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

from tools.t2corpus import keys
from tools.t2corpus.spool import (
    FLAG_BITS,
    decode_record,
    effective_flags,
    flag_names,
    read_spool,
    segment_paths,
    write_spool,
)
from tools.t2harness import generate as gen

MAX_ROUNDS = 4
MIN_ENTRIES = 8  # distinct accepted inputs wanted per function (brief: at least 8 entries)
EXTRA_BATCHES = 3
EXTRA_VARIANTS = 4
# The host's per-input watchdog (`--input-ms`): a hang is a SIGALRM fault, refused as timeout.
INPUT_MS = 2000
SIGALRM = 14
HOST_SECONDS_BASE = 30.0  # backstop for a host that hangs outside a guarded run
# The runtime's sampler would drop inputs of one shape after 8; constructed runs keep every call.
CAPTURE_ENV = {
    "TSFP_T2_CAPTURE_SEGMENT_BYTES": "0",
    "TSFP_T2_CAPTURE_SHAPE_CAP": "4096",
    "TSFP_T2_CAPTURE_FUNC_CAP": "4096",
    "TSFP_T2_CAPTURE_HARD": "4096",
    "TSFP_T2_CAPTURE_MIN_FREE_GB": "1",
}
REFUSAL_REASONS = ("fault", "host_death", "timeout", "no_record", "flag", "outside_guest")
GUEST_OK_SOURCE = Path("crates/tsfp-port-engine-anim-adapter/src/anim_lane.rs")
GUEST_OK = re.compile(
    r"fn guest_ok\(address: u32\) -> bool \{\s*\((0x[0-9a-fA-F_]+)\.\.(0x[0-9a-fA-F_]+)\)\.contains"
)


def guest_range(root: Path) -> tuple[int, int]:
    """[lo, hi) of guest addresses the port lanes accept, read from its owner (rule 45).

    Fails loudly when the owner changed shape."""
    text = (root / GUEST_OK_SOURCE).read_text()
    found = GUEST_OK.search(text)
    if found is None:
        raise HarnessError(f"guest_ok range not found in {GUEST_OK_SOURCE}")
    return int(found[1].replace("_", ""), 16), int(found[2].replace("_", ""), 16)


def outside(raw: bytes, lo: int, hi: int) -> int | None:
    """The first address the record read or wrote outside [lo, hi), else None."""
    rec = decode_record(raw)
    for addr, data in [*rec.read_runs, *rec.write_runs]:
        if addr < lo or addr + len(data) > hi:
            return addr if addr < lo or addr >= hi else hi
    return None


class HarnessError(RuntimeError):
    """The harness itself failed (bad host, unreadable output): no verdict for the function."""


@dataclass
class Outcome:
    index: int
    accepted: bool
    reason: str = ""
    raw: bytes = b""
    extra_reads: int = 0


@dataclass
class FunctionResult:
    va: int
    contract: gen.Contract
    outcomes: dict[int, Outcome] = field(default_factory=dict)
    rounds: int = 0
    global_pages: int = 0
    header: dict[str, object] = field(default_factory=dict)

    @property
    def accepted(self) -> list[Outcome]:
        return [o for o in self.outcomes.values() if o.accepted]

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
    """The synthetic `recording_sha256` of a constructed spool.

    One ingest unit per function and seed."""
    text = f"constructed-call:{gen.GENERATOR}/{gen.GENERATOR_VERSION}:{seed}:{va:#010x}"
    return hashlib.sha256(text.encode()).hexdigest()


def parse_results(text: str) -> tuple[set[int], dict[int, dict[str, object]]]:
    """(started positions, end line by position) of a `--t2-construct` results file; a cut last line
    is ignored."""
    started: set[int] = set()
    ended: dict[int, dict[str, object]] = {}
    for line in text.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        position = int(row["index"])
        if row.get("event") == "start":
            started.add(position)
        elif row.get("event") == "end":
            ended[position] = row
    return started, ended


def root_records(raw_records: list[bytes], va: int, tags: dict[int, int]) -> dict[int, bytes]:
    """Candidate index -> the root record of its call (its own frame with the input's tag)."""
    found: dict[int, bytes] = {}
    for raw in raw_records:
        try:
            rec = decode_record(raw)
        except Exception:  # noqa: BLE001 - a malformed record pairs with nothing and the input is no_record
            continue
        if rec.va == va and rec.return_address in tags:
            found.setdefault(tags[rec.return_address], raw)
    return found


def classify(
    positions: list[int],
    started: set[int],
    ended: dict[int, dict[str, object]],
    records: dict[int, bytes],
    returncode: int | None,
    domain: tuple[int, int] = (0, 1 << 32),
) -> tuple[dict[int, Outcome], list[int]]:
    """Outcomes of one host run over candidates `positions` (in input order) and the candidates left
    to rerun.

    returncode None means the host was killed at the timeout."""
    outcomes: dict[int, Outcome] = {}
    rerun: list[int] = []
    for position, index in enumerate(positions):
        end = ended.get(position)
        if end is None:
            if position in started:
                reason = "timeout" if returncode is None else f"host_death:rc={returncode}"
                outcomes[index] = Outcome(index, False, reason)
            else:
                rerun.append(index)
            continue
        extra = int(str(end.get("extra_reads", 0)))
        if end.get("end") != "returned":
            detail = end.get("signal", end.get("detail", ""))
            kind = "signal" if end.get("end") == "fault" else str(end.get("end"))
            reason = "timeout:watchdog" if detail == SIGALRM else f"fault:{kind}={detail}"
            outcomes[index] = Outcome(index, False, reason, extra_reads=extra)
            continue
        raw = records.get(index)
        if raw is None:
            # The host flushes the spool at exit: a returned input of a killed host has no record
            # yet.
            if returncode in (0, 4):
                outcomes[index] = Outcome(index, False, "no_record", extra_reads=extra)
            else:
                rerun.append(index)
            continue
        flags = effective_flags(decode_record(raw).flags) & ~FLAG_BITS["old_format"]
        if flags:
            outcomes[index] = Outcome(
                index, False, "flag:" + "+".join(flag_names(flags)), raw, extra
            )
            continue
        wild = outside(raw, *domain)
        if wild is not None:
            outcomes[index] = Outcome(index, False, f"outside_guest:{wild:#x}", raw, extra)
            continue
        outcomes[index] = Outcome(index, True, raw=raw, extra_reads=extra)
    return outcomes, rerun


def unprovided_pages(raw: bytes, provided: set[int]) -> set[int]:
    """Pages the original read that the input did not provide (it read arena poison there)."""
    rec = decode_record(raw)
    pages: set[int] = set()
    for addr, data in rec.read_runs:
        for page in range(addr // gen.PAGE, (addr + max(len(data), 1) - 1) // gen.PAGE + 1):
            if page * gen.PAGE not in provided:
                pages.add(page * gen.PAGE)
    return pages


def provided_pages(cand: gen.Candidate, global_pages: dict[int, tuple[bytes, bool]]) -> set[int]:
    pages = set(global_pages)
    for addr, data in cand.runs:
        pages.update(range(addr // gen.PAGE * gen.PAGE, addr + len(data), gen.PAGE))
    return pages


def read_capture(spool: Path) -> tuple[dict[str, object], list[bytes]]:
    paths = [spool] if spool.is_file() else segment_paths(spool, include_open=True)
    header: dict[str, object] = {}
    raws: list[bytes] = []
    for path in paths:
        read = read_spool(path)
        header = header or read.header
        raws += read.records
    return header, raws


Runner = Callable[[list[str], dict[str, str], float], int | None]


def run_host(argv: list[str], env: dict[str, str], timeout: float) -> int | None:
    """Run the host; its return code, or None when it was killed at the timeout."""
    try:
        done = subprocess.run(
            argv,
            env={**os.environ, **env},
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return None
    if done.returncode == 2:
        raise HarnessError(f"host refused the run: {done.stderr.decode(errors='replace')[-400:]}")
    return done.returncode


def run_round(
    host: Path,
    contract: gen.Contract,
    cands: list[gen.Candidate],
    global_pages: dict[int, tuple[bytes, bool]],
    workdir: Path,
    seed: int,
    domain: tuple[int, int],
    runner: Runner = run_host,
    encode: Callable[..., bytes] = gen.encode_input,
) -> tuple[dict[int, Outcome], dict[str, object]]:
    """Every candidate once through the original (restarting after a fault, death or timeout).
    `encode(va, candidate, global_pages)` writes one input record (T3822 mutants have their own)."""
    by_index = {c.index: c for c in cands}
    pending = [c.index for c in cands]
    outcomes: dict[int, Outcome] = {}
    header: dict[str, object] = {}
    attempt = 0
    while pending:
        attempt += 1
        if attempt > len(cands) + 1:
            raise HarnessError(f"{contract.va:#x}: no progress after {attempt} host runs")
        stem = workdir / f"a{attempt:03d}"
        inputs = stem.with_suffix(".in.t2cap")
        spool = stem.with_suffix(".cap.t2cap")
        results = stem.with_suffix(".jsonl")
        for path in (spool, results):
            path.unlink(missing_ok=True)
        records = [encode(contract.va, by_index[i], global_pages) for i in pending]
        write_spool(inputs, construct_header(contract.va, seed), records)
        env = {
            **CAPTURE_ENV,
            "TSFP_T2_CAPTURE": str(spool),
            "TSFP_T2_CAPTURE_RECORDING_SHA256": recording_id(contract.va, seed),
        }
        argv = [
            str(host),
            "--t2-construct",
            str(inputs),
            "--va",
            f"{contract.va:#x}",
            "--out",
            str(results),
        ]
        argv += ["--input-ms", str(INPUT_MS)]
        timeout = HOST_SECONDS_BASE + (INPUT_MS / 1000 + 0.5) * len(pending)
        returncode = runner(argv, env, timeout)
        started, ended = parse_results(results.read_text() if results.is_file() else "")
        found_header, raws = (
            read_capture(spool) if spool.exists() or segment_paths(spool, True) else ({}, [])
        )
        header = header or found_header
        tags = {by_index[i].return_address: i for i in pending}
        found = root_records(raws, contract.va, tags)
        done, rerun = classify(pending, started, ended, found, returncode, domain)
        outcomes.update(done)
        if returncode not in (0, 4, None) and not started:
            raise HarnessError(f"{contract.va:#x}: host exit {returncode} before any input")
        pending = rerun
    return outcomes, header


def construct_header(va: int, seed: int) -> dict[str, object]:
    return {
        "recording_sha256": recording_id(va, seed),
        "build_input_hash": "",
        "lifted_tree_hash": "",
        "created": "",
        "tool_version": f"{gen.GENERATOR}/{gen.GENERATOR_VERSION}",
        "tier2_source": "constructed-call",
    }


def _rounds(
    host: Path,
    contract: gen.Contract,
    cands: list[gen.Candidate],
    global_pages: dict[int, tuple[bytes, bool]],
    workdir: Path,
    image_page: Callable[[int], tuple[bytes, bool] | None],
    domain: tuple[int, int],
    seed: int,
    runner: Runner,
) -> tuple[dict[int, Outcome], dict[str, object], int]:
    """Discovery rounds for one batch of candidates; grows `global_pages` in place. Last round's
    outcomes."""
    by_index = {c.index: c for c in cands}
    outcomes: dict[int, Outcome] = {}
    header: dict[str, object] = {}
    round_number = 0
    for round_number in range(1, MAX_ROUNDS + 1):
        round_dir = workdir / f"round{round_number}"
        round_dir.mkdir(parents=True, exist_ok=True)
        outcomes, header = run_round(
            host, contract, cands, global_pages, round_dir, seed, domain, runner
        )
        wanted: dict[int, tuple[bytes, bool]] = {}
        # Every root record counts, refused ones too: a global read as poison may be what sent it
        # outside.
        for outcome in outcomes.values():
            if not outcome.raw:
                continue
            provided = provided_pages(by_index[outcome.index], global_pages)
            for page in unprovided_pages(outcome.raw, provided):
                data = image_page(page)
                if data is not None:
                    wanted[page] = data
        if not wanted:
            break
        global_pages.update(wanted)
    return outcomes, header, round_number


INPUT_DEPENDENT = ("outside_guest", "flag")


def worth_more(result: FunctionResult) -> bool:
    """More seeded variants can help only when some input was accepted or refused for an
    input-dependent reason; a function whose every input hung, died, faulted or left no frame gets
    the same from more seeds."""
    return bool(result.accepted) or any(
        o.reason.startswith(INPUT_DEPENDENT) for o in result.outcomes.values()
    )


def construct(
    host: Path,
    contract: gen.Contract,
    workdir: Path,
    image_page: Callable[[int], tuple[bytes, bool] | None],
    domain: tuple[int, int],
    seed: int = gen.DEFAULT_SEED,
    runner: Runner = run_host,
) -> FunctionResult:
    """All batches for one function. `image_page(page_va)` is (XBE image page, writable), None
    outside the image; `domain` the guest range of `guest_range`.

    The first batch is `generate.candidates`. While fewer than `MIN_ENTRIES` distinct inputs are
    accepted, up to `EXTRA_BATCHES` more batches add `EXTRA_VARIANTS` seeded variants of each
    strategy that produced an accepted input (of every seeded strategy when none did), unless
    `worth_more` says seeds cannot help."""
    workdir.mkdir(parents=True, exist_ok=True)
    cands = gen.candidates(contract, seed)
    batch = cands
    global_pages: dict[int, tuple[bytes, bool]] = {}
    result = FunctionResult(contract.va, contract)
    for number in range(EXTRA_BATCHES + 1):
        outcomes, header, rounds = _rounds(
            host,
            contract,
            batch,
            global_pages,
            workdir / f"batch{number}",
            image_page,
            domain,
            seed,
            runner,
        )
        result.outcomes.update(outcomes)
        result.header = result.header or header
        result.rounds = max(result.rounds, rounds)
        if len({keys.input_digest(decode_record(o.raw)) for o in result.accepted}) >= MIN_ENTRIES:
            break
        if number == EXTRA_BATCHES or not worth_more(result):
            break
        productive = sorted({cands[o.index].strategy for o in result.accepted} - {"zero"})
        strategies = productive or [name for name in gen.STRATEGIES if name != "zero"]
        first_variant = gen.VARIANTS + number * EXTRA_VARIANTS
        batch = [
            gen.candidate(contract, seed, strategy, variant, len(cands) + offset)
            for offset, (strategy, variant) in enumerate(
                (strategy, variant)
                for strategy in strategies
                for variant in range(first_variant, first_variant + EXTRA_VARIANTS)
            )
        ]
        cands += batch
    result.global_pages = len(global_pages)
    return result


def reference_spool(result: FunctionResult, seed: int, out: Path) -> int:
    """Write the accepted root records as an ingestable spool; returns the record count."""
    header = dict(result.header)
    header.update(
        recording_sha256=recording_id(result.va, seed),
        tier2_source="constructed-call",
        generator=gen.GENERATOR,
        generator_version=gen.GENERATOR_VERSION,
        seed=seed,
    )
    for key in ("build_input_hash", "lifted_tree_hash", "created", "tool_version"):
        header.setdefault(key, "")
    raws = [o.raw for o in sorted(result.accepted, key=lambda o: o.index)]
    write_spool(out, header, raws)
    return len(raws)
