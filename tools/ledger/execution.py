# SPDX-License-Identifier: GPL-3.0-or-later
# ruff: noqa: E501
"""Live-execution weights (T1884): which functions the game actually runs, and how often.

Source: host indirect-call census files (`census.txt.<phase>`, lines
`target 0xVA calls N presents P ...`, written by the host on SIGUSR1 marks in owner sessions,
Story and Map Maker routes). Only addresses, counts and a neutral subsystem label are stored.
Pure functions: no clock, no network, deterministic output.
"""

from __future__ import annotations

import re
from collections.abc import Collection, Iterable, Mapping
from dataclasses import dataclass, field
from pathlib import Path

from tools import verification
from tools.ledger.dispositions import CLASSES, disposition_of
from tools.tracked_files import read_optional_path_bytes

SCHEMA = 1
SKIP_SUFFIXES = (".ms", ".pid", ".ack", ".phase")
LINE = re.compile(r"^target 0x([0-9A-Fa-f]+) calls (\d+) presents (\d+)")
DEFAULT_PATH = Path("docs/data/t1884-exec-weights.csv")


@dataclass
class Weights:
    """Aggregated census: per VA total calls and census files it appeared in."""

    frames: int = 0  # sum of presented frames over all census files
    files: int = 0
    sessions: int = 0
    calls: dict[int, int] = field(default_factory=dict)
    seen_in: dict[int, int] = field(default_factory=dict)
    subsystem: dict[int, str] = field(default_factory=dict)
    # T1888: `calls` and `frames` are the primary volume. With entry-counter sessions (census.txt.<phase>.calls
    # next to the census file) they are ENTRY counts (direct and indirect) over those sessions' frames, and the
    # indirect-call census sums are kept apart. Without them `calls` is the indirect census as before.
    entry_data: bool = False
    indirect: dict[int, int] = field(default_factory=dict)
    indirect_frames: int = 0
    samples: dict[int, int] = field(default_factory=dict)  # profile.txt self-time samples per VA
    # T2417: part of `calls` that came from the r5 Story-gameplay replay run (exec-merge), labelled per row.
    run_calls: dict[int, int] = field(default_factory=dict)

    def rate(self, va: int) -> float:
        """Calls per presented frame over every census file (0.0 when unknown)."""
        return self.calls.get(va, 0) / self.frames if self.frames else 0.0

    def total_calls(self) -> int:
        return sum(self.calls.values())


def parse_census(text: str) -> tuple[dict[int, int], int]:
    """One census file -> ({va: calls}, presents). Presents is the largest value on any line."""
    calls: dict[int, int] = {}
    presents = 0
    for line in text.splitlines():
        match = LINE.match(line)
        if match:
            va = int(match.group(1), 16)
            calls[va] = calls.get(va, 0) + int(match.group(2))
            presents = max(presents, int(match.group(3)))
    return calls, presents


CALLS_SUFFIX = ".calls"
CALL_LINE = re.compile(r"^function 0x([0-9A-Fa-f]+) calls (\d+)")


def parse_calls(text: str) -> dict[int, int]:
    """One entry-counter file (host built with TSFP_CALL_COUNT) -> {va: entries}. Lines `function 0xVA calls N`."""
    calls: dict[int, int] = {}
    for line in text.splitlines():
        match = CALL_LINE.match(line)
        if match:
            va = int(match.group(1), 16)
            calls[va] = calls.get(va, 0) + int(match.group(2))
    return calls


def parse_profile(text: str) -> tuple[int, list[int]]:
    """profile.txt (cpu sampler) -> (exe load base, instruction pointers of the first frame of each sample)."""
    base = 0
    rips: list[int] = []
    for line in text.splitlines():
        if line.startswith("# base "):
            base = int(line.split()[2], 16)
        elif line and not line.startswith("#"):
            words = line.split()
            if len(words) >= 2:
                try:
                    rips.append(int(words[1], 16))
                except ValueError:
                    continue
    return base, rips


def samples_by_va(base: int, rips: Iterable[int], resolve: Mapping[int, str]) -> dict[int, int]:
    """Count samples per lifted function. `resolve` maps an exe offset (rip - base) to the addr2line function
    name; names `sub_XXXXXXXX` give the VA, everything else (host code, libraries) is dropped."""
    out: dict[int, int] = {}
    for rip in rips:
        name = resolve.get(rip - base, "")
        if name.startswith("sub_"):
            try:
                va = int(name[4:], 16)
            except ValueError:
                continue
            out[va] = out.get(va, 0) + 1
    return out


def census_files(roots: Iterable[Path]) -> list[Path]:
    found: set[Path] = set()
    for root in roots:
        if root.is_file():
            found.add(root)
        else:
            found |= {p for p in root.rglob("census.txt.*") if p.is_file()}
    return sorted(p for p in found if not p.name.endswith(SKIP_SUFFIXES + (CALLS_SUFFIX,)))


def aggregate(
    files: Iterable[tuple[str, str] | tuple[str, str, str | None]],
    subsystems: Mapping[int, str] | None = None,
) -> Weights:
    """files: (session id, census text[, entry-counter text]). Files without any target line count nothing.

    A census file with an entry-counter partner contributes its ENTRY counts to `calls` and its presents to
    `frames`; the indirect census sums of every file are kept in `indirect`. With no partner anywhere the
    result is the indirect census, exactly as before T1888."""
    indirect: dict[int, int] = {}
    entries: dict[int, int] = {}
    indirect_seen: dict[int, int] = {}
    entry_seen: dict[int, int] = {}
    out = Weights()
    sessions: set[str] = set()
    entry_files = 0
    for item in files:
        session, text = item[0], item[1]
        partner = item[2] if len(item) > 2 else None  # type: ignore[misc]
        calls, presents = parse_census(text)
        if not calls or presents <= 0:
            continue
        out.files += 1
        out.indirect_frames += presents
        sessions.add(session)
        for va, count in calls.items():
            indirect[va] = indirect.get(va, 0) + count
            indirect_seen[va] = indirect_seen.get(va, 0) + 1
        if partner is not None:
            counted = parse_calls(partner)
            if counted:
                entry_files += 1
                out.frames += presents
                for va, count in counted.items():
                    entries[va] = entries.get(va, 0) + count
                    entry_seen[va] = entry_seen.get(va, 0) + 1
    out.sessions = len(sessions)
    out.indirect = indirect
    if entry_files:
        out.entry_data = True
        out.calls, out.seen_in = entries, entry_seen
    else:
        out.frames = out.indirect_frames
        out.calls, out.seen_in = dict(indirect), indirect_seen
    for va in set(out.calls) | set(indirect):
        out.subsystem[va] = (subsystems or {}).get(va, "unknown")
    return out


def collect(roots: Iterable[Path], subsystems: Mapping[int, str] | None = None) -> Weights:
    pairs: list[tuple[str, str, str | None]] = []
    for path in census_files(roots):
        partner = path.with_name(path.name + CALLS_SUFFIX)
        pairs.append(
            (
                str(path.parent.name),
                path.read_text(encoding="utf-8", errors="ignore"),
                partner.read_text(encoding="utf-8", errors="ignore") if partner.is_file() else None,
            )
        )
    return aggregate(pairs, subsystems)


def dumps(weights: Weights) -> str:
    if not weights.entry_data and not weights.samples:
        lines = [
            f"# T1884 execution weights schema={SCHEMA} frames={weights.frames} "
            f"files={weights.files} sessions={weights.sessions}",
            "# source: host indirect-call target census (a lower bound: direct calls are not counted)",
            "va,calls,files,subsystem",
        ]
        for va in sorted(weights.calls):
            lines.append(
                f"0x{va:08x},{weights.calls[va]},{weights.seen_in[va]},{weights.subsystem[va]}"
            )
        return "\n".join(lines) + "\n"
    lines = [
        f"# T1884 execution weights schema=2 frames={weights.frames} files={weights.files} "
        f"sessions={weights.sessions} entry_data={int(weights.entry_data)} "
        f"indirect_frames={weights.indirect_frames}",
        "# source: calls = function ENTRY counts (direct and indirect) from TSFP_CALL_COUNT host runs over frames "
        "(T1888); indirect = T1884 indirect-call census sums; samples = profile.txt self-time samples; "
        "t2417_calls = the part of calls (entry counts, not samples) from the T2417 r5 Story replay run",
        "va,calls,files,subsystem,indirect,samples,t2417_calls",
    ]
    for va in sorted(set(weights.calls) | set(weights.indirect) | set(weights.samples)):
        lines.append(
            f"0x{va:08x},{weights.calls.get(va, 0)},{weights.seen_in.get(va, 0)},"
            f"{weights.subsystem.get(va, 'unknown')},{weights.indirect.get(va, 0)},"
            f"{weights.samples.get(va, 0)},{weights.run_calls.get(va, 0)}"
        )
    return "\n".join(lines) + "\n"


def merge_run(
    weights: Weights,
    calls: Mapping[int, int],
    frames: int,
    subsystems: Mapping[int, str] | None = None,
) -> Weights:
    """T2417: add one more entry-counted session (call counts over `frames` presents) to `weights`.

    Counts and frames are summed, so `rate` stays total calls per total frame. The added part is also kept per
    row in `run_calls` so the source stays visible. Existing rows are never dropped."""
    weights.entry_data = True
    weights.frames += frames
    weights.files += 1
    weights.sessions += 1
    for va, count in calls.items():
        weights.calls[va] = weights.calls.get(va, 0) + count
        weights.seen_in[va] = weights.seen_in.get(va, 0) + 1
        weights.run_calls[va] = weights.run_calls.get(va, 0) + count
        weights.subsystem.setdefault(va, (subsystems or {}).get(va, "unknown"))
    for va, name in weights.subsystem.items():
        if name == "unknown" and subsystems and va in subsystems:
            weights.subsystem[va] = subsystems[
                va
            ]  # rows written before the placement table knew them
    return weights


def loads(text: str) -> Weights:
    out = Weights()
    for line in text.splitlines():
        if line.startswith("# T1884"):
            meta = dict(part.split("=") for part in line.split() if "=" in part)
            out.frames, out.files, out.sessions = (
                int(meta["frames"]),
                int(meta["files"]),
                int(meta["sessions"]),
            )
            out.entry_data = meta.get("entry_data") == "1"
            out.indirect_frames = int(meta.get("indirect_frames", out.frames))
        elif line and not line.startswith(("#", "va,")):
            fields = line.split(",")
            va = int(fields[0], 16)
            calls, seen = int(fields[1]), int(fields[2])
            out.subsystem[va] = fields[3]
            if len(fields) >= 6:
                if int(fields[4]):
                    out.indirect[va] = int(fields[4])
                if int(fields[5]):
                    out.samples[va] = int(fields[5])
            if len(fields) >= 7 and int(fields[6]):
                out.run_calls[va] = int(fields[6])
            if calls:
                out.calls[va], out.seen_in[va] = calls, seen
    return out


def load(path: Path) -> Weights | None:
    content = read_optional_path_bytes(path)
    return loads(content.decode("utf-8")) if content is not None else None


def tree_of(subsystem: str, depth: int = 2) -> str:
    return "/".join(subsystem.split("/")[:depth]) if subsystem else "unknown"


def is_library(subsystem: str) -> bool:
    return subsystem == "library" or subsystem.startswith("library/")


def readiness(counts: Mapping[str, int]) -> float:
    """Done fraction of the applicable obligations (1.0 means nothing left but the port)."""
    applicable = counts["done"] + counts["open"] + counts["blocked"]
    return counts["done"] / applicable if applicable else 0.0


def score(mode: str, rate: float, ready: float) -> float:
    if mode == "rate":
        return rate
    if mode == "exec":
        return rate * ready
    return 0.0


def coverage(
    weights: Weights, ported: Mapping[int, set[str]], proven: set[int]
) -> dict[str, object]:
    """Share of executed call volume owned by ported/proven roots, overall, per tree, per crate.

    ported: va -> crates holding a Rust port of it. Volume is the sum of call counts."""
    total = weights.total_calls()
    ported_vas = {va for va in weights.calls if va in ported}
    proven_vas = {va for va in ported_vas if va in proven}

    def volume(vas: Iterable[int]) -> int:
        return sum(weights.calls[va] for va in vas)

    trees: dict[str, dict[str, int]] = {}
    for va, count in weights.calls.items():
        bucket = trees.setdefault(
            tree_of(weights.subsystem[va]),
            {"functions": 0, "calls": 0, "ported_calls": 0, "proven_calls": 0},
        )
        bucket["functions"] += 1
        bucket["calls"] += count
        bucket["ported_calls"] += count if va in ported_vas else 0
        bucket["proven_calls"] += count if va in proven_vas else 0
    crates: dict[str, dict[str, int]] = {}
    for va in sorted(ported_vas):
        for crate in sorted(ported[va]):
            bucket = crates.setdefault(crate, {"executed_roots": 0, "calls": 0, "proven_calls": 0})
            bucket["executed_roots"] += 1
            bucket["calls"] += weights.calls[va]
            bucket["proven_calls"] += weights.calls[va] if va in proven_vas else 0
    game = [va for va in weights.calls if not is_library(weights.subsystem[va])]
    return {
        "game_calls": volume(game),
        "game_ported_calls": volume(v for v in game if v in ported_vas),
        "game_proven_calls": volume(v for v in game if v in proven_vas),
        "total_calls": total,
        "executed_functions": len(weights.calls),
        "ported_calls": volume(ported_vas),
        "proven_calls": volume(proven_vas),
        "trees": dict(sorted(trees.items())),
        "crates": dict(sorted(crates.items())),
        "executed_ported_vas": len(ported_vas),
    }


BREADTH_CALL_SHARE = 0.80  # owner decision 2026-10-10: Rust owns 80% of executed call volume
BREADTH_TIER1_SHARE = 0.60  # and 60% of ledger functions are at Tier 1 or higher


def breadth_bar(
    rows: Iterable[Mapping[str, object]],
    weights: Weights | None,
    modules: Mapping[int, str] | None = None,
    scope: str = "all",
    table: Mapping[str, str] | None = None,
) -> dict[str, object]:
    """Progress against the breadth-phase done bar (docs/decisions/breadth-bar-and-tier2.md).

    Call share: executed game call volume (library excluded) owned by functions whose
    rust_port obligation is done. Tier share: ledger functions with verification_tier >= 1."""
    rows = list(rows)
    all_rows = rows
    table = table or {}
    disp = {
        int(str(r["va"]), 16): disposition_of(str(r.get("subsystem", "unknown")), table)
        for r in rows
    }
    by_disp = disposition_table(rows, weights, disp)
    if scope == "port":
        rows = [r for r in rows if disp[int(str(r["va"]), 16)] == "port"]
    ported: set[int] = set()
    tiers: dict[int, int | None] = {}
    weak: set[int] = set()
    tier1 = 0
    for row in rows:
        obligations = row.get("obligations") or {}
        va = int(str(row["va"]), 16)
        port = obligations.get("rust_port") or {}
        if port.get("status") == "done":  # type: ignore[union-attr]
            ported.add(va)
        elif str(port.get("note", "")).startswith("port-registry doc-leader"):  # type: ignore[union-attr]
            weak.add(va)
        tier = row.get("verification_tier")
        tiers[va] = tier if isinstance(tier, int) else None
        tier1 += int(isinstance(tier, int) and tier >= 1)
    game_calls = ported_calls = 0
    if weights is not None:
        in_scope = {int(str(row["va"]), 16) for row in rows}
        weights = scoped_weights(weights, in_scope) if scope == "port" else weights
        for va, count in weights.calls.items():
            if not is_library(weights.subsystem.get(va, "unknown")):
                game_calls += count
                ported_calls += count if va in ported else 0
    call_share = ported_calls / game_calls if game_calls else 0.0
    tier_share = tier1 / len(rows) if rows else 0.0
    side = side_by_side(ported, weak, tiers, weights, modules or {})
    excluded = sorted(
        {
            str(row.get("subsystem", "unknown"))
            for row in all_rows
            if disp[int(str(row["va"]), 16)] == "undecided"
        }
    )
    return {
        **side,
        "scope": scope,
        "by_disposition": by_disp,
        "undecided_subsystems": excluded,
        "game_calls": game_calls,
        "ported_calls": ported_calls,
        "call_share": round(call_share, 4),
        "call_target": BREADTH_CALL_SHARE,
        "functions": len(rows),
        "tier1_functions": tier1,
        "tier1_share": round(tier_share, 4),
        "tier1_target": BREADTH_TIER1_SHARE,
        "weights_present": weights is not None,
        "met": weights is not None
        and call_share >= BREADTH_CALL_SHARE
        and tier_share >= BREADTH_TIER1_SHARE,
    }


def tier2_metrics(
    rows: Iterable[Mapping[str, object]],
    weights: Weights | None,
    table: Mapping[str, str],
    excluded: Collection[int] = (),
) -> dict[str, object]:
    """Tier 2 progress over port-class functions only, weighted by recorded calls.

    `excluded` (T3793) are validated MEASURED-unreachable functions: they leave the
    denominator and are counted in `excluded_unreachable`, never silently dropped.
    """
    overall = _tier2_bucket()
    systems: dict[str, dict[str, int]] = {}
    skip = frozenset(excluded)
    for row in rows:
        system = str(row.get("subsystem", "unknown"))
        if disposition_of(system, table) != "port":
            continue
        va = int(str(row["va"]), 16)
        bucket = systems.setdefault(system, _tier2_bucket())
        if va in skip:
            for target in (overall, bucket):
                target["excluded_unreachable"] += 1
            continue
        calls = weights.calls.get(va, 0) if weights is not None else 0
        tier = row.get("verification_tier")
        tier2 = verification.is_tier2(tier)
        source = str(row.get("verification_tier2_source") or verification.WHOLE_GAME)
        for target in (overall, bucket):
            target["functions"] += 1
            target["tier2_functions"] += int(tier2)
            target["executed_functions"] += int(calls > 0)
            target["tier2_executed_functions"] += int(tier2 and calls > 0)
            # Promoted but absent from the weights corpus: counted, never given invented calls.
            target["tier2_unweighted_functions"] += int(tier2 and calls == 0)
            target["executed_calls"] += calls
            target["tier2_calls"] += calls if tier2 else 0
            if tier2:
                for prefix in SOURCE_PREFIXES:
                    if source == prefix.replace("_", "-"):
                        target[f"tier2_{prefix}_functions"] += 1
                        target[f"tier2_{prefix}_calls"] += calls
                # Every credit source label, read generically (new labels need no code change).
                by_source = target.setdefault("tier2_by_source", {})  # type: ignore[arg-type]
                item = by_source.setdefault(source, {"functions": 0, "calls": 0})
                item["functions"] += 1
                item["calls"] += calls
    return {
        "scope": "port",
        "weights_present": weights is not None,
        "overall": _finish_tier2_bucket(overall),
        "by_system": {
            system: _finish_tier2_bucket(bucket) for system, bucket in sorted(systems.items())
        },
    }


SOURCE_PREFIXES = ("whole_game", "capture", "constructed_call", "harness_service", "symbolic")


def _tier2_bucket() -> dict[str, int]:
    return {
        "functions": 0,
        "tier2_functions": 0,
        "executed_functions": 0,
        "tier2_executed_functions": 0,
        "tier2_unweighted_functions": 0,
        "executed_calls": 0,
        "tier2_calls": 0,
        # Split of the Tier 2 totals by credit source (T3744, rule 43).
        "tier2_whole_game_functions": 0,
        "tier2_whole_game_calls": 0,
        "tier2_capture_functions": 0,
        "tier2_capture_calls": 0,
        # T3793: fabricated-input harness credit, never folded into the capture count.
        "tier2_constructed_call_functions": 0,
        "tier2_constructed_call_calls": 0,
        "tier2_harness_service_functions": 0,
        "tier2_harness_service_calls": 0,
        # T3875: symbolic equivalence proofs (tools.t2symbolic).
        "tier2_symbolic_functions": 0,
        "tier2_symbolic_calls": 0,
        # T3793: MEASURED-unreachable functions left out of `functions` (shown, not hidden).
        "excluded_unreachable": 0,
    }


def _finish_tier2_bucket(bucket: Mapping[str, int]) -> dict[str, int | float]:
    return {
        **bucket,
        "tier2_function_share": bucket["tier2_functions"] / bucket["functions"]
        if bucket["functions"]
        else 0.0,
        "tier2_executed_function_share": bucket["tier2_executed_functions"]
        / bucket["executed_functions"]
        if bucket["executed_functions"]
        else 0.0,
        "tier2_call_share": bucket["tier2_calls"] / bucket["executed_calls"]
        if bucket["executed_calls"]
        else 0.0,
    }


def scoped_weights(weights: Weights, keep: set[int]) -> Weights:
    """Weights restricted to the given VAs (the same object shape, so the bar code is shared)."""
    out = Weights()
    out.frames, out.files, out.sessions = weights.frames, weights.files, weights.sessions
    for va in weights.calls:
        if va in keep:
            out.calls[va] = weights.calls[va]
            out.seen_in[va] = weights.seen_in.get(va, 0)
            out.subsystem[va] = weights.subsystem.get(va, "unknown")
    return out


def disposition_table(
    rows: list[Mapping[str, object]], weights: Weights | None, disp: Mapping[int, str]
) -> dict[str, dict[str, object]]:
    """Per disposition: functions, ported (rust_port done), Tier>=1, executed call volume and its
    share of all executed calls (library included in the denominator, shown as its own class)."""
    out: dict[str, dict[str, object]] = {
        c: {"functions": 0, "ported": 0, "tier1": 0, "calls": 0} for c in CLASSES
    }
    for row in rows:
        va = int(str(row["va"]), 16)
        bucket = out[disp[va]]
        bucket["functions"] += 1  # type: ignore[operator]
        port = (row.get("obligations") or {}).get("rust_port") or {}  # type: ignore[union-attr]
        bucket["ported"] += 1 if port.get("status") == "done" else 0  # type: ignore[operator,union-attr]
        tier = row.get("verification_tier")
        bucket["tier1"] += 1 if isinstance(tier, int) and tier >= 1 else 0  # type: ignore[operator]
        if weights is not None:
            bucket["calls"] += weights.calls.get(va, 0)  # type: ignore[operator]
    total = sum(int(b["calls"]) for b in out.values())  # type: ignore[call-overload]
    for bucket in out.values():
        bucket["call_share"] = round(int(bucket["calls"]) / total, 4) if total else 0.0  # type: ignore[call-overload]
    return out


def side_by_side(
    ported: set[int],
    weak: set[int],
    tiers: Mapping[int, int | None],
    weights: Weights | None,
    modules: Mapping[int, str],
) -> dict[str, object]:
    """T1997: ported (any tier) vs Tier>=1 vs Tier>=2, as share of executed game volume and of
    executed game functions, plus the modules whose executed ports carry no recorded tier.
    A port without a recorded tier is Tier 0 (rule 10): never higher than its evidence."""
    if weights is None:
        return {
            "side_by_side": None,
            "verification_debt_modules": [],
            "verification_debt_functions": [],
        }
    executed = {
        va: n
        for va, n in weights.calls.items()
        if not is_library(weights.subsystem.get(va, "unknown"))
    }

    def level(va: int) -> int:
        tier = tiers.get(va)
        return -1 if va not in ported else (tier if tier is not None else 0)

    def share(selected: list[int]) -> dict[str, object]:
        calls = sum(executed[va] for va in selected)
        total = sum(executed.values())
        return {
            "calls": calls,
            "call_share": round(calls / total, 4) if total else 0.0,
            "functions": len(selected),
            "function_share": round(len(selected) / len(executed), 4) if executed else 0.0,
        }

    rows = {
        "ported_any_tier": share([va for va in executed if level(va) >= 0]),
        "tier_ge_1": share([va for va in executed if level(va) >= 1]),
        "tier_ge_2": share([va for va in executed if level(va) >= 2]),
        "doc_leader_only_not_counted": share([va for va in executed if va in weak]),
    }
    debt: dict[str, list[int]] = {}
    for va in executed:
        if va in ported and tiers.get(va) is None:
            debt.setdefault(modules.get(va, "unknown"), []).append(va)
    ranked = sorted(debt.items(), key=lambda kv: (-sum(executed[v] for v in kv[1]), kv[0]))[:5]
    return {
        "executed_game_calls": sum(executed.values()),
        "executed_game_functions": len(executed),
        "side_by_side": rows,
        "verification_debt_modules": [
            {"module": m, "executed_functions": len(v), "calls": sum(executed[x] for x in v)}
            for m, v in ranked
        ],
        "verification_debt_functions": [
            {"va": f"0x{va:08X}", "module": m, "calls": executed[va]}
            for m, v in sorted(debt.items())
            for va in sorted(v, key=lambda x: -executed[x])
        ],
    }


def top_unported(
    weights: Weights, ported: Iterable[int], limit: int = 100
) -> list[tuple[int, int, int]]:
    """(va, calls, files) of executed game functions (library excluded) with no Rust port."""
    skip = set(ported)
    rows = [
        (va, weights.calls[va], weights.seen_in[va])
        for va in weights.calls
        if va not in skip and not is_library(weights.subsystem[va])
    ]
    rows.sort(key=lambda r: (-r[1], r[0]))
    return rows[:limit]
