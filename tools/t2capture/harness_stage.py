# SPDX-License-Identifier: GPL-3.0-or-later
"""The t2-continuous harness stage (T3826): constructed-call Tier 2 kept current after each pass.

claude-1 decision 2026-10-27 03:18: after the main re-check of a pass, the stage
1. runs the T3822 mutation harness (`python3 -m tools.t2harness mutate`) on
   (a) functions newly below the 3-call floor or newly single-shape since the last stage run
       (`mutation_run.floor_vas`, `mutation_run.single_shape_vas`),
   (b) ports newly wired: a shared recording receipt that flips off `no_abi_entry`, or a
       registry entry that is new or changed for a VA whose last known status was no_abi_entry,
   (c) class c functions whose static caller gained corpus entries (the caller's
       `digest_set_hash` moved; the caller's read-sets seed and fill them, `--fill caller`);
2. runs `python3 -m tools.t2harness recheck --changed` over every harness receipt dir: harness
   credit whose port source, compare rule or lift changed (t2-continuous excludes that credit
   from its own re-check, I-T3784-9). Each stale key is tried once (`--skip-key`); a rerun with
   no receipt keeps the old credit, and promote demotes only on a diverged or error verdict;
3. stays inside a per-pass budget: `derive_stage_cap` (STAGE_BUDGET_S over the measured seconds
   per function), a stage timeout that leaves LAND_RESERVE_S of the pass for the landing, and
   a carry for the rest; recording-credited VAs are never selected (recording credit wins).
The fresh receipts go to a stage dir and are folded into the tracked harness dirs and promoted
(`tools.t2capture_promote`, per-VA evidence merge driver) by the pass's own landing on fresh main.

Status from artifacts (rule 46): `<state dir>/harness_stage.json` (last summary, failures) and
the pass line's `harness {...}`; HEALTH `t2-continuous-harness-failing` after
FAIL_HEALTH failures in a row. docs/t3738-capture-design.md "Harness stage (T3826)".
"""

from __future__ import annotations

import contextlib
import json
import os
import re
import shutil
import sys
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

STATE_FILE = "harness_stage.json"
STAGE_BUDGET_S = 600  # target stage length per pass; the function cap is derived from it
STAGE_MIN = 4  # cap floor (and enough functions to re-measure the per-function time)
STAGE_MAX = 200
DEFAULT_S_PER_FUNCTION = 45.0  # before any stage was timed (T3822: ~30-60 s per function)
LAND_RESERVE_S = 1800  # pass time the stage never takes: the landing of the main receipts
MIN_STAGE_S = 120  # less time left than this: the stage is skipped for the pass
FAIL_HEALTH = 2
STALE_UNPROCESSED_PASSES = 2  # HEALTH when a runnable stale receipt waits longer (I-T3839-1)
NICE = 19  # owner PC (T3823): every local harness process at nice 19
JOBS = 1  # owner PC (T3823): at most one local game host
TARGET_ORDER = ("wired", "caller", "single-shape", "floor")  # selection priority
V9_REASON = "v9-unverified"  # T3835: ahead of the carry and TARGET_ORDER
MUTATE_TARGET = {
    V9_REASON: None,
    "wired": None,
    "caller": "class-c",
    "single-shape": "single-shape",
    "floor": "floor",
}


@dataclass
class View:
    """What the stage compares against its previous run (all read through their owners)."""

    floor: set[str]
    single_shape: set[str]
    class_c_callers: dict[str, str]  # class c callee -> caller with corpus entries
    store_status: dict[str, str]  # shared recording receipt store: VA -> status
    harness_status: dict[str, str]  # harness summaries: VA -> status
    registry: dict[str, str]  # port VA -> digest of its registry entry
    recording_credit: set[str]  # evidence entries that are not harness labelled
    # T3835: unverified-after-v9 whole-game VAs, queued ahead of everything (verdict file order)
    priority: list[str] = field(default_factory=list)


@dataclass
class Plan:
    work: list[tuple[str, str]]  # (selection reason, VA), in run order
    carry: list[tuple[str, str]]
    counts: dict[str, int]  # newly selected per reason (before the cap)


@dataclass
class Stale:
    """One harness dir's stale receipts: `run` = rerun order (the recheck takes a prefix),
    `refused` = stale keys whose finished rerun gave no current receipt (credit kept)."""

    run: list[str] = field(default_factory=list)
    refused: int = 0


@dataclass
class StageDeps:
    """Every side effect of the stage, injectable (tests use a fake runner and clock)."""

    view: Callable[[Path], View]  # (checker)
    stale: Callable[[Path, str, Path], Stale]  # (checker, harness dir, skip-key file)
    recheck: Callable[[Path, Path, str, int, Path, Path, float], None]
    # (checker, host, harness dir, limit, into dir, skip-key file, timeout)
    mutate: Callable[[Path, Path, str, list[str], Path, float], None]
    # (checker, host, mutate target, VAs, into dir, timeout)
    clock: Callable[[], float]
    beat: Callable[[str], None] = lambda status: None


@dataclass
class StageResult:
    """Receipt dirs for the landing: (tracked harness dir, fresh stage dir)."""

    dirs: list[tuple[str, Path]] = field(default_factory=list)
    summary: dict = field(default_factory=dict)


def harness_dirs() -> tuple[str, ...]:
    """The tracked harness receipt dirs, from their owners (T3822 mutation, T3796 class d)."""
    from tools.t2harness import mutation_run, receipts

    return (str(mutation_run.OUT_DIR), str(receipts.OUT_DIR))


def load(base: Path) -> dict:
    try:
        return json.loads((base / STATE_FILE).read_text())
    except (OSError, ValueError):
        return {}


def save(base: Path, state: dict) -> None:
    base.mkdir(parents=True, exist_ok=True)
    temporary = base / f".{STATE_FILE}.{os.getpid()}.tmp"
    temporary.write_text(json.dumps(state, indent=1, sort_keys=True) + "\n")
    os.replace(temporary, base / STATE_FILE)


def derive_stage_cap(state: dict) -> int:
    """Functions per pass: STAGE_BUDGET_S over the measured seconds per function of the last
    timed stage, clamped to [STAGE_MIN, STAGE_MAX] (the derive_cap rule of the main check)."""
    per = float(state.get("s_per_function") or 0) or DEFAULT_S_PER_FUNCTION
    return max(STAGE_MIN, min(STAGE_MAX, int(STAGE_BUDGET_S / per)))


def baseline(view: View, corpus: dict[str, str]) -> dict:
    return {
        "floor": sorted(view.floor),
        "single_shape": sorted(view.single_shape),
        "store_status": view.store_status,
        "registry": view.registry,
        "corpus": corpus,
    }


def select_stage(
    previous: dict | None,
    view: View,
    corpus: dict[str, str],
    carry: list[tuple[str, str]],
    cap: int,
    exclude: set[str] = frozenset(),  # type: ignore[assignment]
) -> Plan:
    """New work since `previous` (None: first run, baseline only), carry first, then by
    TARGET_ORDER; recording-credited and `exclude`d VAs never run. Over `cap` is carried."""
    found: dict[str, set[str]] = {reason: set() for reason in TARGET_ORDER}
    if previous is not None:
        found["floor"] = view.floor - set(previous.get("floor") or [])
        found["single-shape"] = view.single_shape - set(previous.get("single_shape") or [])
        old_status = previous.get("store_status") or {}
        flipped = {
            va
            for va, status in view.store_status.items()
            if old_status.get(va) == "no_abi_entry" and status != "no_abi_entry"
        }
        old_registry = previous.get("registry") or {}
        known = {**view.harness_status, **old_status}
        moved = {
            va
            for va, digest in view.registry.items()
            if old_registry.get(va) != digest and known.get(va, "no_abi_entry") == "no_abi_entry"
        }
        found["wired"] = flipped | moved
        old_corpus = previous.get("corpus") or {}
        found["caller"] = {
            callee
            for callee, caller in view.class_c_callers.items()
            if caller in corpus and old_corpus.get(caller) != corpus[caller]
        }
    blocked = view.recording_credit | set(exclude)
    ordered: list[tuple[str, str]] = []
    seen: set[str] = set()
    runnable = view.floor | view.single_shape | set(view.class_c_callers) | set(view.registry)
    for va in view.priority:
        if va in runnable and va not in blocked and va not in seen:
            ordered.append((V9_REASON, va))
            seen.add(va)
    counts: dict[str, int] = {V9_REASON: len(ordered)} if ordered else {}
    for reason, va in carry:
        if va not in blocked and va not in seen:
            ordered.append((reason, va))
            seen.add(va)
    for reason in TARGET_ORDER:
        fresh = sorted(found[reason] - blocked - seen)
        counts[reason] = len(fresh)
        ordered += [(reason, va) for va in fresh]
        seen |= set(fresh)
    return Plan(ordered[:cap], ordered[cap:], counts)


def mutate_target(reason: str, va: str, view: View) -> str:
    """The mutation target a selection runs as; a wired VA as class-c when it has a caller in
    the corpus and no seeds of its own would exist, else single-shape or floor."""
    target = MUTATE_TARGET[reason]
    if target is not None:
        return target
    if va in view.class_c_callers:
        return "class-c"
    return "single-shape" if va in view.single_shape else "floor"


def findings(base: Path) -> list[tuple[str, str]]:
    """HEALTH t2-continuous-harness-failing: the stage failed FAIL_HEALTH passes in a row;
    t2-continuous-harness-stale-unprocessed: a runnable stale harness receipt sat through more
    than STALE_UNPROCESSED_PASSES stage runs without a finished rerun (I-T3839-1)."""
    state = load(base)
    if (base / "off").exists():
        return []
    where = (
        f"; state <data dir>/t2continuous/{STATE_FILE}, log <data dir>/t2continuous/loop.out "
        "(docs/t3738-capture-design.md, Harness stage)"
    )
    found = []
    failures = int(state.get("failures") or 0)
    if failures >= FAIL_HEALTH:
        found.append(
            (
                "t2-continuous-harness-failing",
                f"harness stage failed {failures} passes in a row: "
                f"{str(state.get('error', ''))[-240:]}{where}",
            )
        )
    waiting = {k: int(v) for k, v in dict(state.get("unprocessed") or {}).items()}
    old = sorted(k for k, passes in waiting.items() if passes > STALE_UNPROCESSED_PASSES)
    if old:
        found.append(
            (
                "t2-continuous-harness-stale-unprocessed",
                f"{len(old)} stale harness receipt(s) unprocessed for over "
                f"{STALE_UNPROCESSED_PASSES} passes (first {old[0]}, oldest "
                f"{max(waiting.values())} passes){where}",
            )
        )
    return found


def run_stage(
    base: Path,
    deps: StageDeps,
    checker: Path,
    sha: str,
    corpus: dict[str, str],
    host: Callable[[], Path],
    time_left: float,
    exclude: set[str] = frozenset(),  # type: ignore[assignment]
) -> StageResult:
    """One stage run at `sha`. Never raises: a failure is recorded (failures, error) and the
    pass lands its main receipts anyway. The baseline advances only on success; a failed run's
    selection goes back to the carry (behind new work, so one bad VA cannot block the stage)."""
    state = load(base)
    started = deps.clock()
    out = base / "harness" / sha[:12]
    result = StageResult()
    budget = min(2 * STAGE_BUDGET_S, time_left - LAND_RESERVE_S)
    if budget < MIN_STAGE_S:
        result.summary = {"skipped": f"{int(time_left)} s left in the pass"}
        return _finish(base, state, result)
    plan: Plan | None = None
    try:
        deps.beat(f"pass {sha[:10]} harness select")
        view = deps.view(checker)
        previous = state.get("last")
        carry = [tuple(item) for item in state.get("carry") or []]
        cap = derive_stage_cap(state)
        found = {name: deps.stale(checker, name, _skip_key(base, name)) for name in harness_dirs()}
        stale = {name: len(item.run) for name, item in found.items()}
        refused = sum(item.refused for item in found.values())
        # I-T3839-1: every runnable stale key counts a pass unprocessed until a rerun finishes
        waiting = dict(state.get("unprocessed") or {})
        state["unprocessed"] = {
            f"{Path(name).name}/{va}": int(waiting.get(f"{Path(name).name}/{va}", 0)) + 1
            for name, item in found.items()
            for va in item.run
        }
        recheck_total = sum(stale.values())
        plan = select_stage(previous, view, corpus, carry, cap, exclude)  # type: ignore[arg-type]
        recheck_cap = cap if not plan.work else max(1, cap // 2)
        limits = _split(stale, min(recheck_total, recheck_cap))
        plan = select_stage(
            previous, view, corpus, carry, max(0, cap - sum(limits.values())), exclude
        )  # type: ignore[arg-type]
        if out.exists():
            shutil.rmtree(out)
        out.mkdir(parents=True)
        deadline = deps.clock() + budget
        ran = 0
        for name, limit in limits.items():
            if limit:
                deps.beat(f"pass {sha[:10]} harness recheck {limit} ({Path(name).name})")
                into = out / Path(name).name
                deps.recheck(checker, host(), name, limit, into, _skip_key(base, name),
                             _left(deadline, deps))  # fmt: skip
                for va in found[name].run[:limit]:
                    state["unprocessed"].pop(f"{Path(name).name}/{va}", None)
                result.dirs.append((name, into))
                ran += limit
        groups: dict[str, list[str]] = {}
        for reason, va in plan.work:
            groups.setdefault(mutate_target(reason, va, view), []).append(va)
        mutation_dir = harness_dirs()[0]
        into = out / Path(mutation_dir).name
        for target, vas in sorted(groups.items()):
            deps.beat(f"pass {sha[:10]} harness mutate {len(vas)} {target}")
            deps.mutate(checker, host(), target, vas, into, _left(deadline, deps))
            ran += len(vas)
        if groups and not any(name == mutation_dir for name, _ in result.dirs):
            result.dirs.append((mutation_dir, into))
        elapsed = deps.clock() - started
        result.summary = {
            "cap": cap,
            "recheck": {Path(n).name: c for n, c in limits.items() if c},
            # never 0 while stale receipts exist: refused keys stay pending (credit kept)
            "recheck_pending": recheck_total + refused - sum(limits.values()),
            "recheck_refused": refused,
            "unprocessed_passes": max(state["unprocessed"].values(), default=0),
            "new": {k: v for k, v in plan.counts.items() if v},
            "mutate": {t: len(v) for t, v in sorted(groups.items())},
            "receipts": sum(len(list(d.glob("0x*.json"))) for _, d in result.dirs if d.is_dir()),
            "carried": len(plan.carry),
            "s": int(elapsed),
        }
        state.update(
            last=baseline(view, corpus),
            carry=[list(item) for item in plan.carry],
            failures=0,
            error="",
        )
        if ran >= STAGE_MIN:
            state["s_per_function"] = elapsed / ran
    except Exception as error:  # noqa: BLE001 - the stage never fails the main pass
        failures = int(state.get("failures") or 0) + 1
        retry = [list(item) for item in ((plan.work + plan.carry) if plan else [])]
        state.update(failures=failures, error=f"{type(error).__name__}: {error}"[-500:])
        if plan is not None:
            state["carry"] = retry
        result.dirs = []
        result.summary = {"error": state["error"][-200:], "failures": failures}
    return _finish(base, state, result)


def _finish(base: Path, state: dict, result: StageResult) -> StageResult:
    state["last_summary"] = result.summary
    save(base, state)
    return result


def _left(deadline: float, deps: StageDeps) -> float:
    remaining = deadline - deps.clock()
    if remaining <= 0:
        raise TimeoutError("harness stage budget spent")
    return remaining


def _split(stale: dict[str, int], total: int) -> dict[str, int]:
    """`total` rerun slots over the harness dirs in their order (mutation first)."""
    out = {}
    for name, count in stale.items():
        out[name] = min(count, total)
        total -= out[name]
    return out


def _skip_key(base: Path, name: str) -> Path:
    return base / f"harness-tried-{Path(name).name}.json"


# ------------------------------------------------------------------------------- landing


def fold(checker: Path, dirs: list[tuple[str, Path]]) -> list[Path]:
    """Copy the stage's fresh receipts into the tracked harness dirs of the checker tree and
    merge their summary facts; returns the stage dirs to promote (fresh receipts only)."""
    from tools.t2harness import receipts

    promote = []
    for name, fresh in dirs:
        files = sorted(fresh.glob("0x*.json")) if fresh.is_dir() else []
        target = checker / name
        target.mkdir(parents=True, exist_ok=True)
        for path in files:
            shutil.copy2(path, target / path.name)
        summary = receipts.summary_path(fresh)
        if summary.is_file():
            doc = json.loads(summary.read_text())
            tracked = receipts.summary_path(target)
            old = json.loads(tracked.read_text()) if tracked.is_file() else doc
            inputs = dict(old.get("inputs") or {})
            receipts.merge_summary(
                target, dict(doc.get("functions") or {}), task=str(old.get("task", "T3826")),
                generator=str(inputs.get("generator", "")),
                generator_version=int(inputs.get("generator_version", 1) or 1),
            )  # fmt: skip
        if files:
            promote.append(fresh)
    return promote


def summary_text(summary: dict) -> str:
    return json.dumps(summary, sort_keys=True, separators=(",", ":"))


# ------------------------------------------------------------------------------- real deps


VIEW_CODE = """
import hashlib, json, sys
from pathlib import Path
from tools import datadir
from tools.t2capture import continuous as c
from tools.t2capture_promote import is_harness_entry
from tools.t2corpus.store import Corpus
from tools.t2harness import mutation_run as m, receipts as r
store = Path(sys.argv[1]); shared = Path(sys.argv[2])
def va(v): return c._va(v)
corpus = Corpus(store)
try:
    tree = corpus.newest_tree() or ''
    floor = [va(v) for v in m.floor_vas(shared)]
    single = [va(v) for v in m.single_shape_vas(corpus, tree, shared)]
    callers = {va(k): va(v) for k, v in m.class_c_callers(corpus, tree).items()}
finally:
    corpus.close()
status = {}
for p in shared.glob('0x*.json'):
    try: status[va(p.stem)] = str(json.loads(p.read_text()).get('status'))
    except (OSError, ValueError): pass
harness = {}
for d in (m.OUT_DIR, r.OUT_DIR):
    s = r.summary_path(Path(d))
    if s.is_file():
        for k, f in json.loads(s.read_text()).get('functions', {}).items():
            harness[va(k)] = str(f.get('status'))
reg = json.loads(Path(c.REGISTRY).read_text()).get('ports') or {}
registry = {va(k): hashlib.sha256(json.dumps(v, sort_keys=True).encode()).hexdigest()[:16]
            for k, v in reg.items()}
ev = json.loads(Path(c.EVIDENCE).read_text()).get('functions', {})
credit = sorted(k for k, e in ev.items() if not is_harness_entry(e))
print(json.dumps({'floor': floor, 'single': single, 'callers': callers, 'status': status,
                  'harness': harness, 'registry': registry, 'credit': credit}))
"""


def v9_priority(checker: Path) -> list[str]:
    from tools.t2capture.continuous import v9_priority as read

    return read(checker)


def real_view(store: Path, receipt_store: Path) -> Callable[[Path], View]:
    def view(checker: Path) -> View:
        from tools.t2capture.continuous import PassError

        argv = ["nice", "-n", str(NICE), project_python(), "-c", VIEW_CODE]
        done = _subprocess([*argv, str(store), str(receipt_store)], checker, 1800)
        if done[0]:
            raise PassError("harness", f"view rc {done[0]}: {done[1][-400:]}")
        data = json.loads(done[1].strip().splitlines()[-1])
        return View(
            floor=set(data["floor"]), single_shape=set(data["single"]),
            class_c_callers=data["callers"], store_status=data["status"],
            harness_status=data["harness"], registry=data["registry"],
            recording_credit=set(data["credit"]),
            priority=v9_priority(checker),
        )  # fmt: skip

    return view


def project_python() -> str:
    """The main checkout's project venv (the harness needs capstone and the T3793 audit deps the
    service's system python3 lacks); sys.executable when there is none."""
    from tools import datadir

    venv = datadir.main_root() / ".venv" / "bin" / "python"
    return str(venv) if venv.is_file() else sys.executable


def _subprocess(argv: list[str], cwd: Path, timeout: float) -> tuple[int, str]:
    from tools.t2capture.continuous import _run

    done = _run(argv, cwd, timeout)
    return done.returncode, done.stdout


def _unbuffered() -> dict[str, str]:
    """Child env with unbuffered stdout: the error tail is then the real end of the run (a
    traceback came first and the buffered stdout last, hiding the cause, I-T3839-1)."""
    return {**os.environ, "PYTHONUNBUFFERED": "1"}


def _harness(*args: str) -> list[str]:
    return ["nice", "-n", str(NICE), project_python(), "-m", "tools.t2harness", *args]


def real_stale(checker: Path, name: str, skip: Path) -> Stale:
    code, out = _subprocess(
        _harness("recheck", "--changed", "--dry-run", "--out", str(checker / name),
                 "--skip-key", str(skip)),
        checker, 900,
    )  # fmt: skip
    return parse_stale(name, code, out)


def parse_stale(name: str, code: int, out: str) -> Stale:
    """The dry run's rerun list (`stale <va>:` lines, in rerun order) and refused count; a
    missing or inconsistent count line fails loudly (rule 45)."""
    total = re.search(r"(\d+) function\(s\) to rerun", out)
    refused = re.search(r"(\d+) stale function\(s\) refused", out)
    run = re.findall(r"^stale (0x[0-9A-Fa-f]+):", out, flags=re.MULTILINE)
    if code or not total or not refused or int(total.group(1)) != len(run):
        raise RuntimeError(f"recheck --dry-run {name} rc {code}: {out[-300:]}")
    return Stale(run=run, refused=int(refused.group(1)))


def real_recheck(beat: Callable[[str], None]) -> Callable[..., None]:
    def recheck(checker: Path, host: Path, name: str, limit: int, into: Path, skip: Path,
                timeout: float) -> None:  # fmt: skip
        from tools.t2capture.continuous import _run

        argv = _harness(
            "recheck", "--changed", "--out", str(checker / name), "--into", str(into),
            "--limit", str(limit), "--skip-key", str(skip), "--host", str(host),
            "--jobs", str(JOBS), "--work", str(into.parent / f"work-{into.name}"),
        )  # fmt: skip
        done = _run(argv, checker, timeout, _unbuffered(), beat=beat)
        if done.returncode:
            raise RuntimeError(f"recheck {name} rc {done.returncode}: {done.stdout[-400:]}")

    return recheck


def real_mutate(beat: Callable[[str], None]) -> Callable[..., None]:
    def mutate(checker: Path, host: Path, target: str, vas: list[str], into: Path,
               timeout: float) -> None:  # fmt: skip
        from tools.t2capture.continuous import _run

        argv = _harness(
            "mutate", "--host", str(host), "--target", target, "--jobs", str(JOBS),
            "--out", str(into), "--work", str(into.parent / f"work-mutate-{target}"),
        )  # fmt: skip
        if target == "class-c":
            argv += ["--fill", "caller"]
        for va in vas:
            argv += ["--va", va]
        done = _run(argv, checker, timeout, _unbuffered(), beat=beat)
        with contextlib.suppress(OSError):
            (into.parent / f"mutate-{target}.log").write_text(done.stdout[-100_000:])
        if done.returncode:
            raise RuntimeError(f"mutate {target} rc {done.returncode}: {done.stdout[-400:]}")

    return mutate


def real_deps(store: Path, receipt_store: Path, beat: Callable[[str], None]) -> StageDeps:
    import time

    return StageDeps(
        view=real_view(store, receipt_store), stale=real_stale, recheck=real_recheck(beat),
        mutate=real_mutate(beat), clock=time.time, beat=beat,
    )  # fmt: skip
