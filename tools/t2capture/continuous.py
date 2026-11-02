# SPDX-License-Identifier: GPL-3.0-or-later
"""Continuous capture Tier 2 re-check and promote service (T3784, rule 43 check stage).

    python3 -m tools.t2capture.continuous once [--sha <40-char>]   # one idempotent pass
    python3 -m tools.t2capture.continuous loop                     # one pass per origin/main move

A pass re-checks only the port VAs that may have changed verdict since the last pass:
- VAs whose port source file changed (`git diff --name-only old..new`, reversed through
  docs/data/port-registry.json), plus every port VA in a crate with a changed file and in every
  crate depending on it, transitively (tools.t2capture.crate_graph, cargo metadata path deps),
- every corpus VA when the compare rule (runner.compare_logic_digest: COMPARE_LOGIC_VERSION and
  the fpo_leaf / ret_liveness / inlined-callee tables) or the lift key changed,
- (T3868) a selected VA whose content key (compare rule, lift, corpus digest, port source file
  hashes) equals that of its last landed check is skipped and keeps its credit, except forced
  ones; a time-based sweep re-checks the least recently checked VAs so each is re-checked within
  Config.sweep_window_s (24 h),
- VAs whose corpus entry set changed (Corpus.function_stats digest_set_hash),
- credited VAs whose evidence entry is stale (port_source_commit, compare_logic or lift differ),
- port VAs with corpus entries but no evidence row, once per corpus digest (state `uncredited`),
- VAs carried over from the previous pass: a selection over MAX_VAS_PER_PASS is ordered by
  executed-call weight (runner.exec_weights) and the remainder runs next pass.
It builds a TSFP_BUILD_CAPTURE=1 host for the new SHA (cached by build input hash, which names the
capture flag; a cached host that is not a capture build is discarded and rebuilt), runs
`tools.t2capture.run run --va ...` offline against the existing corpus (never captures), then lands
on FRESH origin/main: fetch, detach the checker at the current tip, re-run
`tools.t2capture_promote` over the pass's receipts there, commit and submit through merge_task;
a conflict or rejected landing retries from a new tip (LAND_ATTEMPTS), never a JSON hand-merge.

State, lock, heartbeat and the checker worktree live under data_dir("t2continuous").
docs/t3738-capture-design.md "Continuous capture Tier 2 (2026-10-26)".
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
import time
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from pathlib import Path

from tools import datadir
from tools.t2capture_promote import is_harness_entry
from tools.wt import relative_worktree

NAME = "t2-continuous"
STATE_NAME = "t2continuous"
EVIDENCE = "docs/data/tier2-capture-evidence.json"
REGISTRY = "docs/data/port-registry.json"
DEFAULT_STORE = "t2corpus-f2"  # data_dir name of the corpus the full passes used
POLL_S = 300
MAX_PASS_S = 3 * 3600
STALE_S = 1800  # HEALTH t2-continuous-stale: no heartbeat for 30 min (beats every minute)
BACKOFF_BASE_S = 600
BACKOFF_MAX_S = 6 * 3600
JOBS = 4
NICE = 15
VA_TIMEOUT_S = 120
STALE_CREDIT_S = 24 * 3600  # HEALTH t2-stale-credit
MAX_VAS_PER_PASS = 1500  # cap floor (and the cap before any check was timed)
MAX_VAS_CEILING = 10000  # hard ceiling of the derived cap
CHECK_BUDGET_S = 15 * 60  # target check-stage length; the cap is derived from it (rule 45)
MIN_TIMED_VAS = 50  # a check of fewer VAs does not re-measure the per-VA time
CARRY_GROWTH_PASSES = 3  # HEALTH t2-continuous-carry-growing: carry grew this many passes
LAND_ATTEMPTS = 3
SWEEP_WINDOW_S = 24 * 3600  # regression sweep: every credited VA re-checked within this


class PassError(RuntimeError):
    """A pass step failed; `stage` (build, run, promote, land) drives the backoff."""

    def __init__(self, stage: str, message: str) -> None:
        super().__init__(f"{stage}: {message}")
        self.stage = stage


class Busy(RuntimeError):
    """Another instance holds the single-instance lock."""


def state_dir() -> Path:
    return datadir.data_dir(STATE_NAME)


def lock_path(base: Path | None = None) -> Path:
    return (base or state_dir()) / "loop.lock"


def heartbeat_path(base: Path | None = None) -> Path:
    return (base or state_dir()) / "heartbeat.json"


def off_path(base: Path | None = None) -> Path:
    return (base or state_dir()) / "off"


def loop_alive(root: Path) -> bool:
    """T3763 liveness probe for tools/data/loops.json: the loop holds this flock while it runs."""
    from tools.agents import loops

    return loops.lock_held(lock_path())


def kill_switch(root: Path) -> Path:
    """T3830: the kill switch path for tools/data/loops.json `switch` (checked every cycle)."""
    return off_path()


def loop_stopped(root: Path) -> bool:
    """True while the kill switch (data_dir t2continuous/off) is set: no supervisor restart."""
    return kill_switch(root).exists()


@contextlib.contextmanager
def single_instance(path: Path) -> Iterator[None]:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = open(path, "w")  # noqa: SIM115 - held for the block
    try:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise Busy(f"another t2 continuous instance holds {path}") from error
        handle.write(f"{os.getpid()}\n")
        handle.flush()
        yield
    finally:
        handle.close()


# ------------------------------------------------------------------------------- selection


def reverse_map(registry: dict) -> dict[str, set[str]]:
    """port source file -> VAs (0x%08X) from the port registry (`ports` keyed by VA)."""
    out: dict[str, set[str]] = {}
    for va, entries in (registry.get("ports") or {}).items():
        for entry in entries if isinstance(entries, list) else [entries]:
            source = entry.get("source") if isinstance(entry, dict) else None
            if source:
                out.setdefault(source, set()).add(_va(va))
    return out


def _va(value: object) -> str:
    """0x%08X from an int (corpus function_stats keys) or a hex string (registry keys).
    I-T3784-7: ints were parsed as hex text, so corpus VA 73728 (0x12000) became 0x00073728."""
    if isinstance(value, int):
        return f"0x{value:08X}"
    return f"0x{int(str(value), 16):08X}"


@dataclass
class Snapshot:
    """What a pass compares against the previous pass."""

    sha: str
    compare_logic: str
    lift: str
    corpus: dict[str, str]  # VA -> digest_set_hash
    registry: dict
    evidence: dict[str, dict]
    source_commits: dict[str, str | None] = field(default_factory=dict)  # module -> commit
    crate_graph: dict[str, set[str]] | None = None  # crate dir -> path deps; None = unknown
    graph_note: str = ""  # why the graph is unknown or partial (logged as a selection reason)
    weights: dict[str, int] = field(default_factory=dict)  # VA -> executed-call weight
    stale_receipts: set[str] = field(default_factory=set)  # stored receipt predates its port
    # T3835: unverified-after-v9 whole-game VAs (tools.t2wholegame_v9 `priority`), checked first
    priority: list[str] = field(default_factory=list)
    # VAs with at least one corpus row from a live --capture-targets recording.
    live_vas: set[str] = field(default_factory=set)
    source_hashes: dict[str, str] = field(default_factory=dict)  # port source file -> hash
    requested: set[str] = field(default_factory=set)  # T3877 dispatch receipt refresh jobs


def select(
    previous: dict | None, now: Snapshot, changed_files: list[str] | None
) -> tuple[list[str], list[str]]:
    """(VAs to re-check, reasons). `changed_files` None means the diff is unknown (bootstrap)."""
    chosen, reasons, _ = select_detail(previous, now, changed_files)
    return chosen, reasons


def select_detail(
    previous: dict | None, now: Snapshot, changed_files: list[str] | None
) -> tuple[list[str], list[str], set[str]]:
    """`select` plus the VAs chosen ONLY as dependents of a changed crate (T3839: those the
    compiled-code digest may drop, `code_unchanged`)."""
    chosen, reasons, only = _select_all(previous, now, changed_files)
    owned = chosen & harness_owned(now)
    if owned:
        reasons.append(f"{len(owned)} harness-owned (re-checked by its harness lane)")
    return sorted(chosen - owned), reasons, only - owned


def harness_owned(now: Snapshot) -> set[str]:
    """Harness-credited VAs (I-T3784-9) with no recording corpus entries: never re-checked,
    stale-tracked or demoted here. Once the corpus has entries for one, it is checked and a
    recording verdict replaces the harness credit (promote precedence, T3793)."""
    return {va for va, e in now.evidence.items() if is_harness_entry(e) and va not in now.corpus}


def harness_stale_vas(now: Snapshot) -> set[str]:
    """Harness-owned VAs whose evidence is stale: 'harness stale (harness lane rerun pending)'."""
    return _stale_entries(now) & harness_owned(now)


def _select_all(
    previous: dict | None, now: Snapshot, changed_files: list[str] | None
) -> tuple[set[str], list[str], set[str]]:
    """(chosen, reasons, chosen only as a changed crate's dependent)."""
    reasons: list[str] = []
    corpus_vas = set(now.corpus)
    chosen: set[str] = set()
    dependent: set[str] = set()
    if previous is not None:
        if previous.get("compare_logic") != now.compare_logic:
            reasons.append("compare rule changed")
        if previous.get("lift") != now.lift:
            reasons.append("lift changed")
        if reasons:
            return corpus_vas | set(now.evidence), reasons, set()
        old_corpus = previous.get("corpus") or {}
        moved = {va for va, digest in now.corpus.items() if old_corpus.get(va) != digest}
        if moved:
            reasons.append(f"{len(moved)} corpus entry set(s) changed")
            chosen |= moved
    requested = now.requested & corpus_vas
    if requested:
        chosen |= requested
        reasons.append(f"{len(requested)} queue prerequisite refresh job(s)")
    if changed_files:
        sources = reverse_map(now.registry)
        touched = {va for path in changed_files for va in sources.get(path, ())}
        if touched:
            reasons.append(f"{len(touched)} VA(s) with a changed port source")
            chosen |= touched
        if now.graph_note:
            reasons.append(now.graph_note)
        dependent = dependent_vas(now, changed_files, sources) - touched
        if dependent:
            reasons.append(f"{len(dependent)} VA(s) in a changed crate or its dependents")
    receipts_stale = now.stale_receipts - set(now.evidence)
    if receipts_stale:
        reasons.append(f"{len(receipts_stale)} stored receipt(s) older than their port source")
        chosen |= receipts_stale
    stale = stale_vas(now)
    if stale:
        reasons.append(f"{len(stale)} stale evidence entr(ies)")
        chosen |= stale
    return chosen | dependent, reasons, dependent - chosen


def dependent_vas(
    now: Snapshot, changed_files: list[str], sources: dict[str, set[str]]
) -> set[str]:
    """Checkable port VAs compiled into a crate with a changed file or into a crate depending on
    one (claude-1 decision 2026-10-26 15:46: a shared-crate fix re-checks its dependents)."""
    from tools.t2capture import crate_graph

    if now.crate_graph is None:
        return set()
    crates = crate_graph.affected_crates(now.crate_graph, changed_files)
    if not crates:
        return set()
    checkable = set(now.corpus) | set(now.evidence)
    return {
        va
        for source, vas in sources.items()
        if crate_graph.owning_crate(source, now.crate_graph) in crates
        for va in vas
        if va in checkable
    }


def uncredited_vas(now: Snapshot, seen: dict[str, str]) -> set[str]:
    """Port VAs (registry) with corpus entries but no evidence row, not yet checked at their
    current corpus digest (claude-1 2026-10-26: e.g. 0xA1DC0 had format-2 entries, no row)."""
    ports = {_va(va) for va in (now.registry.get("ports") or {})}
    return {
        va
        for va, digest in now.corpus.items()
        if va in ports and va not in now.evidence and seen.get(va) != digest
    }


def receipt_statuses(receipts: Path, vas: set[str]) -> dict[str, int]:
    """Status counts (equal, diverged, no_abi_entry, ...) of the receipts of `vas`."""
    counts: dict[str, int] = {}
    for va in vas:
        try:
            status = str(json.loads((receipts / f"{va}.json").read_text()).get("status"))
        except (OSError, ValueError, AttributeError):
            status = "no_receipt"
        counts[status] = counts.get(status, 0) + 1
    return dict(sorted(counts.items()))


def stale_vas(now: Snapshot) -> set[str]:
    """Credited VAs whose evidence no longer matches the tree (compare rule, lift or port
    source commit): credit kept, re-check pending (claude-1 2026-10-26 18:52). Harness-owned
    VAs are excluded (I-T3784-9, `harness_stale_vas`)."""
    return _stale_entries(now) - harness_owned(now)


def _stale_entries(now: Snapshot) -> set[str]:
    stale = set()
    for va, entry in now.evidence.items():
        commit = now.source_commits.get(entry.get("port_module", ""))
        if (
            entry.get("compare_logic") != now.compare_logic
            or entry.get("lifted_tree_hash") != now.lift
            or (commit is not None and entry.get("port_source_commit") != commit)
        ):
            stale.add(va)
    return stale


def recheck_key(va: str, now: Snapshot, sources: dict[str, set[str]], code: dict[str, str]) -> str:
    """Content address of everything a VA's verdict depends on: compare
    rule, lift, corpus entry set, compiled-code digest and port source file content (owner
    2026-10-27: an unchanged function carries its credit)."""
    files = sorted(path for path, vas in sources.items() if va in vas)
    parts = [now.compare_logic, now.lift, now.corpus.get(va, ""), code.get(va, "")]
    parts += [f"{path}={now.source_hashes.get(path, '')}" for path in files]
    return hashlib.sha256("\n".join(parts).encode()).hexdigest()


def sweep_vas(state: dict, universe: set[str], now: float, window_s: float) -> set[str]:
    """The least recently checked VAs, enough per pass that every one is re-checked within
    `window_s` (regression detection): ceil(len * elapsed / window), elapsed since the last
    pass (none on the first). A VA never checked is the oldest."""
    if not universe or window_s <= 0:  # a window of 0 disables the sweep
        return set()
    last = float(state.get("finished_at") or 0)
    elapsed = min(max(now - last, 0.0), window_s) if last else 0.0
    count = math.ceil(len(universe) * elapsed / window_s)
    checked_at = state.get("checked_at") or {}
    return set(sorted(universe, key=lambda va: (checked_at.get(va, 0.0), va))[:count])


def cap_selection(
    vas: list[str],
    weights: dict[str, int],
    cap: int,
    first: set[str] | frozenset = frozenset(),
    fresh: set[str] | frozenset = frozenset(),
    priority: list[str] | tuple = (),
    live_first: set[str] | frozenset = frozenset(),
) -> tuple[list[str], list[str]]:
    """(this pass, carried): new `live:` corpus VAs first, then `priority` (T3835),
    then `first` (stale credit), `fresh` (new corpus entries since their last check),
    then heaviest executed-call weight, ties by VA.
    """
    rank = {va: index for index, va in enumerate(priority)}
    ordered = sorted(
        set(vas),
        key=lambda va: (
            va not in live_first,
            rank.get(va, len(rank)),
            va not in first,
            va not in fresh,
            -weights.get(va, 0),
            va,
        ),
    )
    effective_cap = max(cap, sum(va in live_first for va in ordered))
    return sorted(ordered[:effective_cap]), sorted(ordered[effective_cap:])


def derive_cap(state: dict) -> int:
    """Per-pass VA cap from the measured per-VA check wall time of the last timed pass:
    CHECK_BUDGET_S / s_per_va, clamped to [MAX_VAS_PER_PASS, MAX_VAS_CEILING] (I-T3784-10: a
    fixed 1500 cap let the carry grow every pass while the check took ~4 of 14 minutes)."""
    per_va = float(state.get("check_s_per_va") or 0)
    if per_va <= 0:
        return MAX_VAS_PER_PASS
    return max(MAX_VAS_PER_PASS, min(MAX_VAS_CEILING, int(CHECK_BUDGET_S / per_va)))


def fresh_vas(state: dict, now: Snapshot) -> set[str]:
    """VAs whose corpus digest differs from the digest at their last check (recaptured).
    Before `checked_corpus` exists the previous pass's corpus is the baseline."""
    checked = state.get("checked_corpus")
    if checked is None:
        checked = (state.get("last") or {}).get("corpus") or {}
    return {va for va, digest in now.corpus.items() if checked.get(va) != digest}


def carry_findings(base: Path, passes: int = CARRY_GROWTH_PASSES) -> list[tuple[str, str]]:
    """HEALTH t2-continuous-carry-growing: the carried backlog grew `passes` passes in a row."""
    history = [int(n) for n in load_state(base).get("carry_history") or []][-(passes + 1) :]
    if len(history) <= passes or any(b <= a for a, b in zip(history, history[1:], strict=False)):
        return []
    return [
        (
            "t2-continuous-carry-growing",
            f"carried backlog grew {passes} passes in a row ({' -> '.join(map(str, history))}); "
            "the derived cap does not keep up, shard the check or raise MAX_VAS_CEILING "
            "(docs/incidents/I-T3784-10.md)",
        )
    ]


def stale_since(previous: dict[str, float], stale: set[str], now: float) -> dict[str, float]:
    """First time each currently stale VA was seen stale (state `stale_since`)."""
    return {va: previous.get(va, now) for va in sorted(stale)}


def stale_findings_24h(
    base: Path, now: float, limit: float = STALE_CREDIT_S
) -> list[tuple[str, str]]:
    """HEALTH t2-stale-credit: credit kept with a stale port/compare/lift for over 24 h."""
    since = load_state(base).get("stale_since") or {}
    old = sorted(va for va, at in since.items() if now - float(at) > limit)
    if not old:
        return []
    return [
        (
            "t2-stale-credit",
            f"{len(old)} credited VA(s) stale (port changed, recheck pending) for over "
            f"{int(limit // 3600)} h, e.g. {', '.join(old[:5])}; t2-continuous re-checks them "
            "first (docs/t3738-capture-design.md)",
        )
    ]


# ------------------------------------------------------------------------------- dependencies


Clock = Callable[[], float]


@dataclass
class Deps:
    """Every side effect of a pass, injectable (tests use fakes)."""

    git: Callable[..., str]  # git(*args, cwd=Path) -> stdout, raises on failure
    snapshot: Callable[[Path, str], Snapshot]  # (checker tree, sha)
    build: Callable[[Path, str, float], Path]  # (checker, sha, timeout) -> host path
    check: Callable[[Path, Path, list[str], Path, float], None]  # checker, host, vas, out, timeout
    promote: Callable[[Path, Path], tuple[int, int]]  # (checker, receipts) -> promoted, demoted
    land: Callable[[Path, str], str]  # (checker, base tip) -> landed commit; PassError("land")
    ledger_total: Callable[[Path], int | None]
    clock: Clock = time.time
    sleep: Callable[[float], None] = time.sleep
    beat: Callable[[str], None] = lambda status: None
    # T3826 harness stage: (checker, sha, corpus, host getter, seconds left, main VAs) -> result
    harness: Callable[..., object] | None = None
    ledger_constructed: Callable[[Path], int | None] = lambda checker: None
    # T3839: (checker, capture host, VAs) -> VA -> compiled-code digest (code_digest.digests)
    code_digests: Callable[[Path, Path, list[str]], dict[str, str]] | None = None


@dataclass
class Config:
    base: Path  # state dir
    store: Path
    poll_s: float = POLL_S
    max_pass_s: float = MAX_PASS_S
    jobs: int = JOBS
    max_vas: int | None = None  # None: derived per pass (derive_cap); an int fixes the cap
    receipt_store: Path | None = None  # shared store refreshed with each pass's receipts
    land_attempts: int = LAND_ATTEMPTS
    sweep_window_s: float = SWEEP_WINDOW_S


def load_state(base: Path) -> dict:
    try:
        return json.loads((base / "state.json").read_text())
    except (OSError, ValueError):
        return {}


def save_state(base: Path, state: dict) -> None:
    base.mkdir(parents=True, exist_ok=True)
    temporary = base / f".state.{os.getpid()}.tmp"
    temporary.write_text(json.dumps(state, indent=1, sort_keys=True) + "\n")
    os.replace(temporary, base / "state.json")


def backoff_s(failures: int) -> float:
    return min(BACKOFF_BASE_S * 2 ** max(0, failures - 1), BACKOFF_MAX_S)


def write_heartbeat(base: Path, status: str, now: float) -> None:
    with contextlib.suppress(OSError):
        base.mkdir(parents=True, exist_ok=True)
        temporary = base / f".heartbeat.{os.getpid()}.tmp"
        temporary.write_text(json.dumps({"pid": os.getpid(), "at": now, "status": status}) + "\n")
        os.replace(temporary, heartbeat_path(base))


def stale_findings(base: Path, now: float, limit: float = STALE_S) -> list[tuple[str, str]]:
    """HEALTH t2-continuous-stale: a heartbeat exists, is not a clean stop and is too old."""
    try:
        beat = json.loads(heartbeat_path(base).read_text())
        at = float(beat["at"])
    except (OSError, ValueError, KeyError, TypeError):
        return []
    if str(beat.get("status", "")).startswith("stopped") or off_path(base).exists():
        return []
    if now - at <= limit:
        return []
    return [
        (
            "t2-continuous-stale",
            f"heartbeat {int(now - at)} s old (pid {beat.get('pid')}, {beat.get('status')!r}, "
            f"limit {int(limit)} s); python3 -m tools.agents.loops status, log "
            f"<data dir>/{STATE_NAME}/loop.out (docs/t3738-capture-design.md, Continuous "
            "capture Tier 2)",
        )
    ]


def build_findings(base: Path) -> list[tuple[str, str]]:
    """HEALTH t2-continuous-no-capture-host: the last pass could not obtain a capture build."""
    state = load_state(base)
    if state.get("failed_stage") != "build" or off_path(base).exists():
        return []
    return [
        (
            "t2-continuous-no-capture-host",
            f"pass {str(state.get('failed_sha', ''))[:10]} has no TSFP_BUILD_CAPTURE=1 host "
            f"({state.get('failures', 1)} failure(s)): {str(state.get('error', ''))[-240:]}; "
            f"log <data dir>/{STATE_NAME}/loop.out (docs/t3738-capture-design.md)",
        )
    ]


# ------------------------------------------------------------------------------- one pass


def code_unchanged(
    deps: Deps,
    state: dict,
    checker: Path,
    host: Callable[[], Path],
    vas: set[str],
    candidates: set[str],
) -> tuple[dict[str, str], set[str], set[str]]:
    """(digests of `vas`, candidates whose digest equals the one stored at their last check,
    VAs of `vas` whose digest differs from it). Degrades to no skip and no addition when the
    digest cannot be computed (a build failure is raised again by the check). Baselines are per
    VA and only move when the VA is checked and landed (`next_code_digests`)."""
    if deps.code_digests is None or not vas:
        return {}, set(), set()
    try:
        digests = deps.code_digests(checker, host(), sorted(vas | candidates))
    except Exception as error:  # noqa: BLE001 - degrade loudly: every dependent is re-checked
        print(f"t2 continuous: code digest unavailable: {error}"[:400], file=sys.stderr)
        return {}, set(), set()
    stored = state.get("code_digests") or {}
    stored = stored if stored.get("version") == code_digest_version() else {}
    known = dict(stored.get("vas") or {})
    same = {va for va in candidates if va in digests and known.get(va) == digests[va]}
    moved = {va for va in vas if va in digests and va in known and known[va] != digests[va]}
    return digests, same, moved


def code_inputs(changed: list[str] | None) -> bool:
    """The diff can move compiled code (anything outside docs/, tests/ and tools/), so every
    port VA is digested; otherwise only the selection is (for its baselines)."""
    if changed is None:
        return True
    return any(not path.startswith(("docs/", "tests/", "tools/")) for path in changed)


def next_code_digests(state: dict, code_now: dict[str, str], checked: list[str]) -> dict:
    """The per-VA code baselines after a landed pass: every checked VA takes its digest now
    (one without a digest loses its baseline), everything else keeps the one of its last check."""
    stored = state.get("code_digests") or {}
    known = dict(stored.get("vas") or {}) if stored.get("version") == code_digest_version() else {}
    for va in checked:
        if va in code_now:
            known[va] = code_now[va]
        else:
            known.pop(va, None)
    return {"version": code_digest_version(), "vas": dict(sorted(known.items()))}


def code_digest_version() -> int:
    from tools.t2capture import code_digest

    return code_digest.DIGEST_VERSION


def run_pass(config: Config, deps: Deps, sha: str, checker: Path) -> dict:
    """One idempotent pass at `sha` (40-char). Returns the summary; raises PassError."""
    base = config.base
    state = load_state(base)
    now = deps.clock()
    if state.get("retry_after", 0) > now and state.get("failed_sha") == sha:
        return {"skipped": f"backoff until {int(state['retry_after'] - now)} s from now"}
    deadline = now + config.max_pass_s

    def left() -> float:
        remaining = deadline - deps.clock()
        if remaining <= 0:
            raise PassError("timeout", f"pass exceeded {int(config.max_pass_s)} s")
        return remaining

    deps.beat(f"pass {sha[:10]} prepare")
    deps.git("checkout", "--quiet", "--force", "--detach", sha, cwd=checker)
    snap = deps.snapshot(checker, sha)
    previous = state.get("last") if state.get("last", {}).get("sha") else None
    old_sha = previous["sha"] if previous else state.get("bootstrap_sha")
    changed: list[str] | None = None
    if old_sha and old_sha != sha:
        changed = deps.git("diff", "--name-only", f"{old_sha}..{sha}", cwd=checker).split()
    selected, reasons, only = select_detail(previous, snap, changed)
    seen_uncredited: dict[str, str] = dict(state.get("uncredited") or {})
    uncredited = uncredited_vas(snap, seen_uncredited)  # reported even when selected already
    if uncredited:
        reasons.append(f"{len(uncredited)} port VA(s) in the corpus without an evidence row")
        selected = sorted(set(selected) | uncredited)
    owned = harness_owned(snap)  # I-T3784-9: never checked, tracked or demoted here
    carried_in = [
        va for va in state.get("carry") or [] if va not in set(selected) and va not in owned
    ]
    if carried_in:
        reasons.append(f"{len(carried_in)} VA(s) carried from the previous pass")
    stale_now = stale_vas(snap) | ((snap.stale_receipts | snap.requested) - owned)
    harness_stale = harness_stale_vas(snap)
    if harness_stale:
        reasons.append(f"{len(harness_stale)} harness stale (harness lane rerun pending)")
    priority = [va for va in snap.priority if va in snap.corpus and va not in owned]
    if priority:
        reasons.append(f"{len(priority)} unverified-after-v9 VA(s) first (T3835)")
    cap = config.max_vas if config.max_vas is not None else derive_cap(state)
    fresh = fresh_vas(state, snap)
    previous_live = set(state.get("checked_live_vas") or [])
    new_live = snap.live_vas - previous_live
    if new_live:
        reasons.append(f"{len(new_live)} VA(s) with new live: captures first")
        selected = sorted(set(selected) | new_live)
    built: list[Path] = []

    def host() -> Path:
        if not built:
            deps.beat(f"pass {sha[:10]} build")
            try:
                built.append(deps.build(checker, sha, left()))
            except PassError:
                raise
            except Exception as error:  # noqa: BLE001 - every build failure backs off
                raise PassError("build", str(error)) from error
        return built[0]

    # T3839 (I-T3839-4): the compiled code is the dependency. A VA chosen only as a changed
    # crate's dependent is dropped when its code equals the code of its last check; a VA whose
    # code moved since its last check is added (a lock, profile or inlining change the crate
    # rule misses). Every other selection reason stands.
    candidates = only - set(state.get("carry") or []) - fresh - set(priority) - uncredited
    candidates -= stale_now
    ports = {_va(va) for va in (snap.registry.get("ports") or {})}
    universe = ((set(snap.corpus) | set(snap.evidence)) & ports) - owned
    code_now, skipped, moved = code_unchanged(
        deps, state, checker, host, universe if code_inputs(changed) else set(selected), candidates
    )
    moved -= set(selected) | set(carried_in)
    if skipped:
        reasons.append(f"{len(skipped)} dependent VA(s) skipped: compiled code unchanged (T3839)")
        selected = sorted(set(selected) - skipped)
    elif candidates and deps.code_digests is not None and not code_now:
        reasons.append("code digest unavailable: dependents re-checked (T3839)")
    if moved:
        reasons.append(f"{len(moved)} VA(s) with changed compiled code (T3839)")
        selected = sorted(set(selected) | moved)
    sweep = sweep_vas(state, universe, now, config.sweep_window_s)
    if sweep:
        reasons.append(f"{len(sweep)} VA(s) in the regression sweep")
    # Content-addressed carry: a VA whose inputs equal those of its last landed check keeps its
    # credit, unless it is forced (priority, uncredited, new live, stale evidence, sweep).
    sources = reverse_map(snap.registry)
    keys = {va: recheck_key(va, snap, sources, code_now) for va in set(selected) | set(carried_in)}
    forced = set(priority) | uncredited | new_live | stale_now | sweep | fresh
    stored_keys = state.get("checked_keys") or {}
    if (state.get("code_digests") or {}).get("version") != code_digest_version():
        stored_keys = {}  # the checker's code-digest logic changed: every baseline is dropped
    unchanged = {va for va, key in keys.items() if va not in forced and stored_keys.get(va) == key}
    if unchanged:
        reasons.append(f"{len(unchanged)} VA(s) skipped: inputs unchanged since last check")
        selected = sorted(set(selected) - unchanged)
        carried_in = [va for va in carried_in if va not in unchanged]
    vas, carry = cap_selection(
        sorted(set(selected) | set(priority) | sweep) + carried_in,
        snap.weights,
        cap,
        stale_now,
        fresh,
        priority,
        new_live,
    )
    if carry:
        reasons.append(f"{len(carry)} VA(s) over the {cap} cap carried to the next pass")
    summary: dict = {"sha": sha, "rechecked": len(vas), "carried": len(carry), "reasons": reasons}
    summary["cap"] = cap
    check_s_per_va = state.get("check_s_per_va")
    started = deps.clock()
    summary["harness_stale"] = sorted(harness_stale)
    summary["new_live_vas"] = sorted(new_live)
    promoted = demoted = 0
    landed = ""
    summary["code_skipped"] = len(skipped)

    try:
        out = base / "passes" / sha[:12]
        receipts = out / "receipts"
        if vas:
            host()
            if out.exists():
                shutil.rmtree(out)
            out.mkdir(parents=True)
            deps.beat(f"pass {sha[:10]} check {len(vas)}")
            checking = deps.clock()
            summary["build_s"] = int(checking - started)
            deps.check(checker, built[0], vas, out, left())
            check_s = deps.clock() - checking
            summary["check_s"] = int(check_s)
            if len(vas) >= MIN_TIMED_VAS:
                check_s_per_va = check_s / len(vas)
            receipts.mkdir(exist_ok=True)
            for receipt in out.rglob("*.json"):
                if receipt.parent != receipts:
                    shutil.copy2(receipt, receipts / receipt.name)
            summary["uncredited_status"] = receipt_statuses(receipts, uncredited & set(vas))
        stage_dirs: list = []
        if deps.harness is not None:  # T3826: after the main check, inside the same pass
            stage = deps.harness(checker, sha, snap.corpus, host, left(), set(vas))
            stage_dirs = list(getattr(stage, "dirs", []))
            summary["harness"] = dict(getattr(stage, "summary", {}))
        if vas or stage_dirs:
            landing = deps.clock()
            landed, promoted, demoted = land_on_fresh_main(
                config, deps, checker, sha, receipts if vas else None, left, stage_dirs
            )
            summary["land_s"] = int(deps.clock() - landing)
            if vas and config.receipt_store is not None:  # newest verdicts: stale ones clear
                config.receipt_store.mkdir(parents=True, exist_ok=True)
                for receipt in receipts.glob("0x*.json"):
                    shutil.copy2(receipt, config.receipt_store / receipt.name)
            shutil.rmtree(out, ignore_errors=True)  # rule 30: receipts are folded into evidence
            if stage_dirs:
                shutil.rmtree(base / "harness" / sha[:12], ignore_errors=True)
    except PassError as error:
        failures = int(state.get("failures", 0)) + 1 if state.get("failed_sha") == sha else 1
        state.update(
            failures=failures,
            failed_sha=sha,
            failed_stage=error.stage,
            retry_after=deps.clock() + backoff_s(failures),
            error=str(error)[-500:],
        )
        save_state(base, state)
        raise
    summary.update(promoted=promoted, demoted=demoted, landed=landed)
    summary["ledger_tier2"] = deps.ledger_total(checker)
    constructed = deps.ledger_constructed(checker)
    before = state.get("ledger_constructed")
    if constructed is not None and before is not None and summary.get("harness"):
        summary["harness"]["constructed_delta"] = constructed - int(before)
    state = {
        "last": {
            "sha": sha,
            "compare_logic": snap.compare_logic,
            "lift": snap.lift,
            "corpus": snap.corpus,
        },
        "carry": carry,
        "carry_history": [*(state.get("carry_history") or []), len(carry)][-10:],
        "check_s_per_va": check_s_per_va,
        "checked_corpus": {
            **(state.get("checked_corpus") or (previous or {}).get("corpus") or {}),
            **{va: snap.corpus[va] for va in vas if va in snap.corpus},
        },
        "checked_live_vas": sorted(snap.live_vas),
        "stale_since": stale_since(load_state(base).get("stale_since") or {}, stale_now, now),
        "uncredited": {
            **seen_uncredited,
            **{va: snap.corpus[va] for va in uncredited & set(vas)},
        },
        "code_digests": next_code_digests(state, code_now, vas),
        "checked_keys": {
            **stored_keys,
            **{va: recheck_key(va, snap, sources, code_now) for va in vas},
        },
        "checked_at": {**(state.get("checked_at") or {}), **{va: now for va in vas}},
        "last_summary": summary,
        "ledger_constructed": constructed if constructed is not None else before,
        "finished_at": deps.clock(),
    }
    save_state(base, state)
    return summary


def land_on_fresh_main(
    config: Config,
    deps: Deps,
    checker: Path,
    sha: str,
    receipts: Path | None,
    left: Callable[[], float],
    harness: list | tuple = (),
) -> tuple[str, int, int]:
    """(landed commit or "", promoted, demoted). Each attempt rebuilds the evidence on the
    current origin/main tip from the pass's receipts, so a main that moved during the pass (or
    during the landing) never makes a JSON conflict; the evidence file is never hand-merged.
    `harness` (T3826): (tracked harness dir, stage dir) pairs; their fresh receipts are folded
    into the tracked dirs of the tip tree and promoted after the main receipts (a recording
    receipt or credit always wins, promote precedence T3793)."""
    from tools.t2capture import harness_stage

    tracked = [EVIDENCE, *sorted({name for name, _ in harness})]
    errors: list[str] = []
    for attempt in range(1, config.land_attempts + 1):
        left()
        deps.beat(f"pass {sha[:10]} promote (land attempt {attempt})")
        deps.git("fetch", "--quiet", "origin", "main", cwd=checker)
        tip = deps.git("rev-parse", "origin/main", cwd=checker).strip()
        deps.git("checkout", "--quiet", "--force", "--detach", tip, cwd=checker)
        promoted = demoted = 0
        if receipts is not None:
            promoted, demoted = deps.promote(checker, receipts)
        for fresh in harness_stage.fold(checker, list(harness)):
            more, fewer = deps.promote(checker, fresh)
            promoted, demoted = promoted + more, demoted + fewer
        if not deps.git("status", "--porcelain", "--", *tracked, cwd=checker).strip():
            return "", promoted, demoted
        deps.beat(f"pass {sha[:10]} land on {tip[:10]} (attempt {attempt})")
        left()
        try:
            return deps.land(checker, tip), promoted, demoted
        except PassError as error:
            if error.stage != "land":
                raise
            errors.append(f"attempt {attempt} on {tip[:10]}: {str(error)[-300:]}")
    raise PassError(
        "land",
        f"{config.land_attempts} landing attempt(s) from fresh main failed: " + " | ".join(errors),
    )


def summary_line(summary: dict) -> str:
    if "skipped" in summary:
        return f"t2 continuous: skipped ({summary['skipped']})"
    return (
        f"t2 continuous {summary['sha'][:10]}: rechecked {summary['rechecked']}, "
        f"promoted {summary.get('promoted', 0)}, demoted {summary.get('demoted', 0)}, "
        f"ledger tier2 {summary.get('ledger_tier2')}"
        + (f", carried {summary['carried']}" if summary.get("carried") else "")
        + (
            f", uncredited {json.dumps(summary['uncredited_status'], sort_keys=True)}"
            if summary.get("uncredited_status")
            else ""
        )
        + (f", landed {summary['landed'][:10]}" if summary.get("landed") else "")
        + (f", cap {summary['cap']}" if summary.get("cap") else "")
        + (
            f", build {summary['build_s']} s, check {summary['check_s']} s, "
            f"land {summary.get('land_s', '-')} s"
            if "check_s" in summary
            else ""
        )
        + (
            f", harness {json.dumps(summary['harness'], sort_keys=True)}"
            if summary.get("harness")
            else ""
        )
        + (f" [{'; '.join(summary['reasons'])}]" if summary.get("reasons") else "")
    )


def loop(
    config: Config,
    deps: Deps,
    checker: Path,
    remote_tip: Callable[[], str],
    *,
    cycles: int = 0,
    log: Callable[[str], None] = print,
    heal: Callable[[str], None] | None = None,
) -> int:
    """Poll origin/main every poll_s and run one pass per move (or after a backoff expires)."""
    done = 0
    while not cycles or done < cycles:
        done += 1
        if off_path(config.base).exists():
            deps.beat("stopped (kill switch)")
            return 0
        deps.beat("polling")
        try:
            tip = remote_tip()
        except Exception as error:  # noqa: BLE001 - a flaky remote must not kill the loop
            log(f"t2 continuous: ls-remote failed: {error}")
            tip = ""
        state = load_state(config.base)
        if tip and tip != state.get("last", {}).get("sha"):
            try:
                if heal is not None:  # every pass: another container may have pruned it
                    heal(tip)
                deps.git("fetch", "--quiet", "origin", "main", cwd=checker)
                log(summary_line(run_pass(config, deps, tip, checker)))
            except PassError as error:
                after = max(0.0, load_state(config.base).get("retry_after", 0) - deps.clock())
                log(f"t2 continuous {tip[:10]}: FAILED {error}; backoff {int(after)} s")
            except Exception as error:  # noqa: BLE001 - log and keep the service up
                log(f"t2 continuous {tip[:10]}: ERROR {type(error).__name__}: {error}")
        if not cycles or done < cycles:
            deps.sleep(config.poll_s)
    return 0


# ------------------------------------------------------------------------------- real deps


def _run(
    cmd: list[str],
    cwd: Path,
    timeout: float,
    env: dict | None = None,
    beat: Callable[[str], None] | None = None,
) -> subprocess.CompletedProcess:
    """Run with a deadline, beating every minute; kills the process group on timeout."""
    process = subprocess.Popen(
        cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        start_new_session=True,
    )  # fmt: skip
    deadline = time.time() + timeout
    while True:
        try:
            out, _ = process.communicate(timeout=min(60, max(1, deadline - time.time())))
            return subprocess.CompletedProcess(cmd, process.returncode, out, "")
        except subprocess.TimeoutExpired as expired:
            if beat is not None:
                beat(f"running {Path(cmd[min(3, len(cmd) - 1)]).name}")
            if time.time() >= deadline:
                with contextlib.suppress(OSError):
                    os.killpg(process.pid, 15)
                with contextlib.suppress(subprocess.TimeoutExpired):
                    process.communicate(timeout=10)
                with contextlib.suppress(OSError):
                    os.killpg(process.pid, 9)
                raise PassError(
                    "timeout", f"{' '.join(cmd[:6])} exceeded {int(timeout)} s"
                ) from expired


def real_git(*args: str, cwd: Path) -> str:
    done = subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True, timeout=300, check=False
    )
    if done.returncode:
        raise PassError("git", f"git {' '.join(args)}: {done.stderr.strip()[-300:]}")
    return done.stdout


def _python(checker: Path, code: str) -> str:
    done = subprocess.run(
        [sys.executable, "-c", code], cwd=checker, capture_output=True, text=True,
        timeout=600, check=False,
    )  # fmt: skip
    if done.returncode:
        raise PassError("snapshot", done.stderr.strip()[-300:])
    return done.stdout.strip()


def source_hashes(checker: Path, sources: dict[str, set[str]]) -> dict[str, str]:
    out = {}
    for path in sources:
        try:
            out[path] = hashlib.sha256((checker / path).read_bytes()).hexdigest()
        except OSError:
            out[path] = "missing"
    return out


def real_snapshot(store: Path) -> Callable[[Path, str], Snapshot]:
    def snapshot(checker: Path, sha: str) -> Snapshot:
        # compare digest, lift and source commits come from the checker tree's own code.
        data = json.loads(
            _python(
                checker,
                "import json,sys\n"
                "from pathlib import Path\n"
                "from tools.t2capture import runner\n"
                "from tools import t2capture_promote as p\n"
                "root=Path('.').resolve()\n"
                f"ev=json.loads(Path({EVIDENCE!r}).read_text()).get('functions',{{}})\n"
                "mods={e.get('port_module','') for e in ev.values()}\n"
                "print(json.dumps({'compare':runner.compare_logic_digest(),"
                "'lift':p.current_lifted_tree_hash(root),"
                "'commits':{m:p.git_last_commit(root,m) for m in mods if m},"
                f"'stale_receipts':p.stale_receipts(root,Path({str(receipt_store())!r}))}}))",
            )
        )
        from tools.t2corpus.store import Corpus

        corpus = Corpus(store)
        try:
            tree = corpus.newest_tree() or ""
            stats = corpus.function_stats(tree)
            live_vas = {
                _va(row["va"])
                for row in corpus.entry_rows(tree)
                if str(row["recording_sha256"]).startswith("live:")
            }
        finally:
            corpus.close()
        lift = data["lift"] or ""
        registry = json.loads((checker / REGISTRY).read_text())
        from tools.t2capture import crate_graph, requests, runner

        fallbacks: list[str] = []
        graph: dict[str, set[str]] | None
        try:
            graph = crate_graph.load(checker, fallbacks=fallbacks)
            note = f"crate graph read {len(fallbacks)} manifest(s) from TOML" if fallbacks else ""
        except crate_graph.GraphError as error:  # degrade loudly: own-file selection only
            graph, note = None, f"crate graph UNKNOWN, dependents not re-selected: {error}"[:300]
        weights_file = checker / runner.WEIGHTS_PATH
        weights = {_va(va): calls for va, calls in runner.exec_weights(weights_file).items()}
        return Snapshot(
            sha=sha,
            compare_logic=data["compare"],
            lift=lift,
            corpus={_va(va): str(stat["digest_set_hash"]) for va, stat in stats.items()},
            registry=registry,
            evidence=json.loads((checker / EVIDENCE).read_text()).get("functions", {}),
            source_commits=data["commits"],
            crate_graph=graph,
            graph_note=note,
            weights=weights,
            stale_receipts=set(data.get("stale_receipts") or []),
            priority=v9_priority(checker),
            live_vas=live_vas,
            source_hashes=source_hashes(checker, reverse_map(registry)),
            requested={_va(va) for va in requests.pending("refresh")},
        )

    return snapshot


def v9_priority(checker: Path) -> list[str]:
    """unverified-after-v9 VAs in queue order (`priority` of the T3835 verdict file)."""
    from tools import verification

    path = checker / verification.DEFAULT_V9_PATH
    if not path.is_file():
        return []
    return [_va(va) for va in json.loads(path.read_text()).get("priority") or []]


def receipt_store() -> Path:
    """The shared t2capture receipt store (tools.t2capture.run --receipt-root default)."""
    return datadir.main_root() / "tmp/wt-cache/receipts/t2capture"


def real_build(cache: Path, beat: Callable[[str], None]) -> Callable[[Path, str, float], Path]:
    def build(checker: Path, sha: str, timeout: float) -> Path:
        if not re.fullmatch(r"[0-9a-f]{40}", sha):
            raise PassError("build", f"the build tool needs a full 40-char SHA, got {sha!r}")
        # The capture flag is part of the prebuilt input hash (prebuilt.capture_identity), so
        # this never reuses or restores a normal host; a host cached before that fix is checked.
        env = {**os.environ, "TSFP_BUILD_CAPTURE": "1"}
        cmd = [sys.executable, "-m", "tools.wt", "prebuilt"]
        common = ["--worktree", str(checker), "--cache", str(cache), "--commit", sha]
        found = _run([*cmd, "path", "--if-present", *common], checker, 120, env)
        if found.returncode == 0 and found.stdout.strip():
            host = Path(found.stdout.strip().splitlines()[-1])
            if capture_problem(host) is None:
                return host
            discard_host(cache, host)
        done = _run(
            ["nice", "-n", str(NICE), *cmd, "build", "--remote", "local", *common],
            checker, timeout, env, beat,
        )  # fmt: skip
        if done.returncode:
            raise PassError("build", f"prebuilt build rc {done.returncode}: {done.stdout[-600:]}")
        found = _run([*cmd, "path", *common], checker, 120, env)
        if found.returncode or not found.stdout.strip():
            raise PassError("build", f"no host after build: {found.stdout[-300:]}")
        host = Path(found.stdout.strip().splitlines()[-1])
        problem = capture_problem(host)
        if problem is not None:
            raise PassError("build", f"not a capture build after building: {problem}")
        return host

    return build


def capture_problem(host: Path) -> str | None:
    """None for a usable capture host (T3767 preflight would accept it), else why not."""
    from tools.t2capture import runtime_env

    return runtime_env.capture_host_problem(host)


def discard_host(cache: Path, host: Path) -> None:
    """Remove a cached non-capture host from OUR cache (prebuilt trees are read-only)."""
    directory = host.parent.resolve()
    if directory.parent != cache.resolve():
        raise PassError("build", f"cached host {host} is not a capture build and not ours")
    for path in [directory, *directory.rglob("*")]:
        with contextlib.suppress(OSError):
            path.chmod(0o755 if path.is_dir() else 0o644)
    shutil.rmtree(directory)
    print(f"t2 continuous: discarded non-capture cached host {directory.name}", flush=True)


def real_check(store: Path, jobs: int, beat: Callable[[str], None]):  # noqa: ANN201
    def check(checker: Path, host: Path, vas: list[str], out: Path, timeout: float) -> None:
        cmd = ["nice", "-n", str(NICE), sys.executable, "-m", "tools.t2capture.run"]
        cmd += ["--receipt-root", str(out), "run", "--host", str(host), "--store", str(store)]
        cmd += ["--jobs", str(jobs), "--timeout", str(VA_TIMEOUT_S)]
        for va in vas:
            cmd += ["--va", va]
        done = _run(cmd, checker, timeout, beat=beat)
        (out / "run.log").write_text(done.stdout[-200_000:])
        if done.returncode:
            raise PassError("run", f"t2capture.run rc {done.returncode}: {done.stdout[-600:]}")

    return check


def real_promote(checker: Path, receipts: Path) -> tuple[int, int]:
    done = _run(
        [sys.executable, "-m", "tools.t2capture_promote", "--receipts", str(receipts),
         "--root", str(checker)],
        checker, 1800,
    )  # fmt: skip
    if done.returncode:
        raise PassError("promote", done.stdout[-600:])
    match = re.search(r"(\d+) function\(s\) promoted, (\d+) demoted", done.stdout)
    if not match:
        raise PassError("promote", f"unparsed output: {done.stdout[-300:]}")
    return int(match.group(1)), int(match.group(2))


def real_land(beat: Callable[[str], None]) -> Callable[[Path, str], str]:
    def land(checker: Path, sha: str) -> str:
        branch = f"task/cx-t2continuous-{sha[:12]}"
        real_git("checkout", "--quiet", "-B", branch, cwd=checker)
        real_git("add", "--", EVIDENCE, *harness_tracked(), cwd=checker)
        real_git(
            "commit", "--quiet", "-m",
            f"t2capture continuous (T3784): evidence re-check at {sha[:10]}", cwd=checker,
        )  # fmt: skip
        head = real_git("log", "-1", "--format=%H", branch, cwd=checker).strip()
        # merge_task lands the LOCAL ref; detach first so its worktree cleanup never sees ours.
        real_git("checkout", "--quiet", "--detach", head, cwd=checker)
        try:
            done = _run(
                [sys.executable, "-m", "tools.agents.merge_task", "--branch", branch,
                 "--profile", "docs"],
                checker, 3600, beat=beat,
            )  # fmt: skip
            real_git("fetch", "--quiet", "origin", "main", cwd=checker)
            on_origin = subprocess.run(
                ["git", "merge-base", "--is-ancestor", head, "origin/main"], cwd=checker,
                timeout=60, check=False,
            ).returncode == 0  # fmt: skip
            if done.returncode or not on_origin:
                raise PassError(
                    "land", f"merge_task rc {done.returncode}, on origin {on_origin}: "
                    f"{done.stdout[-600:]}",
                )  # fmt: skip
        finally:
            subprocess.run(
                ["git", "branch", "-D", branch], cwd=checker, capture_output=True, timeout=60,
                check=False,
            )  # fmt: skip
            subprocess.run(
                ["git", "push", "--quiet", "origin", "--delete", branch], cwd=checker,
                capture_output=True, timeout=120, check=False,
            )  # fmt: skip
        return head

    return land


def real_code_digests(checker: Path, host: Path, vas: list[str]) -> dict[str, str]:
    """`tools.t2capture.code_digest` over the checker tree (project venv: capstone, nice)."""
    import tempfile

    from tools.t2capture.harness_stage import project_python

    with tempfile.TemporaryDirectory(prefix="t3839-digest-") as scratch:
        wanted, out = Path(scratch) / "vas.json", Path(scratch) / "digests.json"
        wanted.write_text(json.dumps(vas))
        argv = ["nice", "-n", str(NICE), project_python(), "-m", "tools.t2capture.code_digest",
                "--host", str(host), "--vas", str(wanted), "--out", str(out)]  # fmt: skip
        timeout = 300 + len(vas) * 0.2  # rule 45: scales with the selection (~25 ms per VA)
        done = subprocess.run(argv, cwd=checker, capture_output=True, text=True,
                              timeout=timeout, check=False)  # fmt: skip
        if done.returncode:
            raise RuntimeError(f"code_digest rc {done.returncode}: {done.stderr.strip()[-300:]}")
        doc = json.loads(out.read_text())
    if doc.get("version") != code_digest_version():
        raise RuntimeError(f"code_digest version {doc.get('version')} != {code_digest_version()}")
    return dict(doc.get("vas") or {})


def real_ledger() -> tuple[Callable[[Path], int | None], Callable[[Path], int | None]]:
    """(Tier 2 total, constructed-call functions) from one `tools.ledger tier2` run per pass:
    the second reads the output the first produced (the T3826 stage's ledger delta)."""
    last: dict[str, str] = {}

    def output(checker: Path, fresh: bool) -> str:
        if fresh or "out" not in last:
            last["out"] = _run([sys.executable, "-m", "tools.ledger", "tier2"], checker, 600).stdout
        return last["out"]

    def total(checker: Path) -> int | None:
        match = re.search(r"Tier 2 port progress: (\d+)/", output(checker, True))
        return int(match.group(1)) if match else None

    def constructed(checker: Path) -> int | None:
        match = re.search(r"constructed-call (\d+) functions", output(checker, False))
        return int(match.group(1)) if match else None

    return total, constructed


def real_ledger_total(checker: Path) -> int | None:
    return real_ledger()[0](checker)


def harness_tracked() -> tuple[str, ...]:
    """The tracked harness receipt dirs (read through their owner, rule 45)."""
    from tools.t2capture import harness_stage

    return harness_stage.harness_dirs()


def ensure_checker(main: Path, checker: Path, sha: str, log: Callable[[str], None] = print) -> None:
    """Validate the checker every pass; rebuild it with relative, locked links if broken (T3784)."""
    if not relative_worktree.is_valid(checker):
        real_git("fetch", "--quiet", "origin", "main", cwd=main)  # the tip may be newer than main
    if relative_worktree.ensure(main, checker, sha, scratch_root=checker.parent):
        log(f"t2 continuous: checker {checker} (re)created at {sha[:10]} with relative links")


def ls_remote_tip(main: Path) -> str:
    out = real_git("ls-remote", "origin", "refs/heads/main", cwd=main)
    return out.split()[0] if out.strip() else ""


def real_deps(config: Config, base: Path) -> Deps:
    def beat(status: str) -> None:
        write_heartbeat(base, status, time.time())
        from tools.agents import loops

        loops.tick(NAME, datadir.main_root(), status)

    ledger_total, ledger_constructed = real_ledger()
    return Deps(
        git=real_git,
        snapshot=real_snapshot(config.store),
        build=real_build(base / "hosts", beat),
        check=real_check(config.store, config.jobs, beat),
        promote=real_promote,
        land=real_land(beat),
        ledger_total=ledger_total,
        beat=beat,
        harness=real_harness(config, base, beat),
        ledger_constructed=ledger_constructed,
        code_digests=real_code_digests,
    )


def real_harness(config: Config, base: Path, beat: Callable[[str], None]) -> Callable[..., object]:
    """The T3826 harness stage with real deps (kill switch `<state dir>/harness-off`)."""
    from tools.t2capture import harness_stage

    deps = harness_stage.real_deps(config.store, config.receipt_store or receipt_store(), beat)

    def stage(checker: Path, sha: str, corpus: dict, host, left: float, exclude: set) -> object:  # noqa: ANN001
        if (base / "harness-off").exists():
            return harness_stage.StageResult(summary={"skipped": "harness-off"})
        return harness_stage.run_stage(base, deps, checker, sha, corpus, host, left, exclude)

    return stage


def _bootstrap(base: Path, main: Path) -> None:
    """First run: diff from the last commit that changed the evidence file (a full pass)."""
    state = load_state(base)
    if state.get("last") or state.get("bootstrap_sha"):
        return
    commit = real_git("log", "-1", "--format=%H", "origin/main", "--", EVIDENCE, cwd=main).strip()
    state["bootstrap_sha"] = commit
    save_state(base, state)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2capture.continuous", description=__doc__.split("\n\n")[0]
    )
    parser.add_argument("mode", choices=("once", "loop"))
    parser.add_argument("--sha", help="once: full 40-char SHA to check (default origin/main tip)")
    parser.add_argument("--store", type=Path, help=f"corpus dir (default data_dir {DEFAULT_STORE})")
    parser.add_argument("--jobs", type=int, default=JOBS)
    parser.add_argument("--poll", type=float, default=POLL_S)
    parser.add_argument("--max-pass", type=float, default=MAX_PASS_S)
    args = parser.parse_args(argv)
    base = state_dir()
    config = Config(
        base=base, store=args.store or datadir.data_dir(DEFAULT_STORE), poll_s=args.poll,
        max_pass_s=args.max_pass, jobs=min(args.jobs, JOBS), receipt_store=receipt_store(),
    )  # fmt: skip
    main_root = datadir.main_root()
    checker = base / "checker"
    deps = real_deps(config, base)
    try:
        with single_instance(lock_path(base)):
            from tools.agents import loops

            if args.mode == "loop":
                from tools.agents import canonical_mount

                canonical_mount.require(NAME, main_root)  # T3784: exits 0 off /workspace
            loops.install_term_handler()
            real_git("fetch", "--quiet", "origin", "main", cwd=main_root)
            _bootstrap(base, main_root)
            try:
                if args.mode == "once":
                    sha = args.sha or ls_remote_tip(main_root)
                    ensure_checker(main_root, checker, sha)
                    real_git("fetch", "--quiet", "origin", "main", cwd=checker)
                    print(summary_line(run_pass(config, deps, sha, checker)), flush=True)
                    return 0
                stamp = lambda line: print(time.strftime("%F %T ") + line, flush=True)  # noqa: E731
                ensure_checker(main_root, checker, ls_remote_tip(main_root), stamp)
                return loop(
                    config, deps, checker, lambda: ls_remote_tip(main_root), log=stamp,
                    heal=lambda tip: ensure_checker(main_root, checker, tip, stamp),
                )  # fmt: skip
            finally:
                write_heartbeat(base, f"stopped ({args.mode})", time.time())
    except Busy as error:
        print(f"t2 continuous: {error}", file=sys.stderr)
        return 3
    except PassError as error:
        print(f"t2 continuous: FAILED {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
