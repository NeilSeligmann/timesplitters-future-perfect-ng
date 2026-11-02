"""Tier 2 work items for the light Codex worker queue (rule 42/43, T3702).

    python3 -m tools.fleet.tier2_queue generate [--append] [--max-items N]
    python3 -m tools.fleet.tier2_queue show t2_<subsystem>_<n>

Ranks port-class functions by executed-call weight (docs/data/t1884-exec-weights.csv) and
emits small independent items (about 5 functions, one subsystem, one kind):

* ``port``     executes in recorded sessions, no Rust port in docs/data/port-registry.json:
               port it, then reach Tier 1.
* ``capture``  ported, no Tier 2 credit in the ledger (docs/data/tier2-evidence.json) nor in
               the capture ledger (docs/data/tier2-capture-evidence.json, T3744): capture-based
               Tier 2 from REA's bulk capture results.
* ``diagnose`` DIVERGED in the ledger switches (or a diverged t2capture receipt): diagnose, fix.

With ``--breakdown <t2round-breakdown.md>`` (claude-1's priorities, 2026-10-26) the ranking is
no_abi, error, diverged_gated first, then the kinds above as fallback:

* ``no_abi``    port-class, receipt status no_abi_entry, not registered by REA (lanes.json, the
               port registry groups A/B, ``--exclude-vas``): add the ABI shim, re-check.
* ``error``     receipt status error without a capture-fidelity exclusion, grouped by cause.
* ``diverged_gated`` DIVERGED receipts, gated on REA's exit_regs liveness check (specs are
               written but only queued with ``--include-gated``); seeds `GATED_SEEDS` rank first.

Item specs are JSON files in ``--specs`` (default tmp/codexapi/t2items); the queue holds only
the ids, which ``tools.fleet.queue_validation`` and ``sup-refill.sh`` accept like ``unk_`` ids.
Standard library only.
"""

from __future__ import annotations

import argparse
import csv
import fnmatch
import json
import re
import subprocess
import sys
import time
from collections.abc import Iterable
from dataclasses import dataclass, field
from pathlib import Path

from tools import datadir
from tools.agents import coord
from tools.fleet.tier2_prerequisites import Prerequisites, brief_evidence, save_evidence

WEIGHTS = "docs/data/t1884-exec-weights.csv"
REGISTRY = "docs/data/port-registry.json"
LEDGER = "docs/data/tier2-evidence.json"
CAPTURE_LEDGER = "docs/data/tier2-capture-evidence.json"
RECEIPTS = "receipts/t2capture"
DISPOSITIONS = "tools/data/system_dispositions.json"
DEFAULT_QUEUE = datadir.main_root() / "tmp/codexapi/p3queue.txt"
DEFAULT_SPECS = DEFAULT_QUEUE.parent / "t2items"
DEFAULT_DENY = DEFAULT_QUEUE.parent / "gate-deny.txt"
DEFAULT_LAUNCHED = DEFAULT_QUEUE.parent / "launched.txt"
DEFAULT_RECEIPTS = datadir.main_root() / "tmp/wt-cache/receipts/t2capture"
ID_PATTERN = re.compile(r"^t2_[a-z0-9-]+_\d+$")
KINDS = ("diagnose", "capture", "port")  # queue priority when weights tie
FAMILY_SIZE = 5
DEFAULT_QUOTA = {"diagnose": 3, "port": 7, "capture": 10}
DIVERGED_STATUSES = {"diverged"}
NEW_KINDS = ("no_abi", "error", "diverged_gated")
PRIORITY_KINDS = (*NEW_KINDS, *KINDS)
GATE_NAME = "exit_regs liveness check (REA)"
GATED_SEEDS = (0x5ADC0, 0x59F90, 0xC0C50, 0x10E410)  # anim_lane2, anim_lane, team_pickup, particle
CAPTURE_RERUN = (
    "Finally, re-run t2capture on the touched functions against tmp/t2corpus-f2 so they reach "
    "Tier 2 in this task (rule 43)."
)
PREREQUISITE_GUIDANCE = (
    "Before acting, verify every required capture, receipt, carrier, and source identity. If an "
    "input is missing or stale, do not retry the same capture or attempt a speculative fix. "
    "Record the exact missing input and refresh action in the task evidence note, then stop with "
    "a blocked result until that prerequisite changes."
)
ERROR_CAUSES = (  # (needle in the receipt error text, cause)
    ("read that never armed", "xdk-unarmed-read"),
    ("stop on unarmed thread", "unarmed-indirect-call"),
    ("timed out", "host-timeout"),
    ("lane root original left", "lane-root-esp"),
    ("unimplemented", "xdk-unimplemented"),
)
PATH_TOKEN = re.compile(r"[\w./*-]*(?:/|\.json|\.py)[\w./*-]*")


def norm_va(value: str) -> int | None:
    try:
        return int(str(value).strip(), 16)
    except ValueError:
        return None


def fmt_va(value: int) -> str:
    return f"0x{value:08X}"


def slug(subsystem: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", subsystem.lower()).strip("-") or "misc"


def load_weights(text: str) -> list[dict[str, object]]:
    """Rows (va, calls, subsystem) of the exec-weight csv; comment lines skipped."""
    lines = [line for line in text.splitlines() if line and not line.startswith("#")]
    rows: list[dict[str, object]] = []
    for row in csv.DictReader(lines):
        va = norm_va(row.get("va", ""))
        if va is None:
            continue
        rows.append(
            {"va": va, "calls": int(row.get("calls") or 0), "subsystem": row.get("subsystem", "")}
        )
    return rows


def registry_ports(data: dict) -> dict[int, dict[str, object]]:
    """va -> {tier (best), source (first)} for every ported function."""
    out: dict[int, dict[str, object]] = {}
    for key, forms in data.get("ports", {}).items():
        va = norm_va(key)
        if va is None or not forms:
            continue
        out[va] = {
            "tier": max(int(form.get("tier") or 0) for form in forms),
            "source": str(forms[0].get("source", "")),
        }
    return out


def ledger_credit(ledger: dict, capture_ledger: dict) -> set[int]:
    """VAs with any Tier 2 credit (whole-system EQUAL run or capture credit)."""
    credit: set[int] = set()
    for source in (ledger.get("functions", {}), capture_ledger.get("functions", {})):
        for key, entry in source.items():
            va = norm_va(key)
            if va is None:
                continue
            runs = entry.get("runs") if isinstance(entry, dict) else None
            if runs is None or any("EQUAL" in str(run.get("evidence_class", "")) for run in runs):
                credit.add(va)
    return credit


def diverged_vas(ledger: dict, receipts: Iterable[dict]) -> set[int]:
    """Functions whose newest ledger switch run, or whose t2capture receipt, DIVERGED."""
    found: set[int] = set()
    for key, entry in ledger.get("switches", {}).items():
        runs = [run for run in entry.get("runs", []) if run.get("receipt_sha256")]
        va = norm_va(key)
        if va is not None and runs and str(runs[-1].get("result", "")).startswith("DIVERGED"):
            found.add(va)
    for receipt in receipts:
        va = norm_va(str(receipt.get("va", "")))
        if va is not None and str(receipt.get("status")) in DIVERGED_STATUSES:
            found.add(va)
    return found


def load_receipts(directory: Path) -> list[dict]:
    out: list[dict] = []
    for path in sorted(directory.glob("*.json")) if directory.is_dir() else []:
        try:
            data = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(data, dict):
            data.setdefault("va", path.stem)
            out.append(data)
    return out


def port_subsystem(subsystem: str, dispositions: dict) -> bool:
    """Port-class unless the longest matching disposition entry says otherwise."""
    best = ""
    for name in dispositions:
        if (subsystem == name or subsystem.startswith(name + "/")) and len(name) > len(best):
            best = name
    return not best or str(dispositions[best].get("d", "port")) == "port"


FREEZE_LINE = re.compile(
    r"(?:@(?P<epoch>\d+(?:\.\d+)?)\s+)?(?P<hms>\d\d:\d\d:\d\d)\s+\[(?P<kind>FREEZE-START|FREEZE-END)\]"
    r"\s+(?P<owner>\S+)\s+->\s+\S+:\s*(?P<text>.*)"
)


def _line_epochs(matches: list[re.Match[str]], now: float) -> list[float]:
    """Exact ``@epoch`` when present, else reconstruct the day from HH:MM:SS (UTC).

    Assumption for bare HH:MM:SS: lines are chronological, the newest one is the most recent past
    occurrence of its clock time, and going backwards a clock time later than its successor means
    one midnight was crossed. Gaps of 24 h or more between two lines cannot be seen.
    """
    out = [0.0] * len(matches)
    day = now - (now % 86400)
    later: float | None = None
    for idx in range(len(matches) - 1, -1, -1):
        match = matches[idx]
        if match["epoch"]:
            out[idx] = later = float(match["epoch"])
            day = later - (later % 86400)
            continue
        hours, minutes, seconds = (int(part) for part in match["hms"].split(":"))
        stamp = day + hours * 3600 + minutes * 60 + seconds
        while stamp > (now if later is None else later):
            stamp -= 86400
        day = stamp - (stamp % 86400)
        out[idx] = later = stamp
    return out


def active_freezes(
    lines: Iterable[str], now: float | None = None, ttl_s: float | None = None
) -> dict[str, list[str]]:
    """owner -> frozen paths, from coord lines ``[@epoch ]HH:MM:SS [TYPE] owner -> all: text``.

    A START is cleared by the same owner's END (case-insensitive) or expires after ``ttl_s``
    (default ``coord.freeze_ttl_s()``); expired freezes are reported on stderr and ignored.
    """
    now = time.time() if now is None else now
    ttl_s = coord.freeze_ttl_s() if ttl_s is None else ttl_s
    matches = [m for m in (FREEZE_LINE.match(line) for line in lines) if m]
    state: dict[str, tuple[float, list[str]]] = {}
    for match, epoch in zip(matches, _line_epochs(matches, now), strict=True):
        owner = match["owner"].lower()
        if match["kind"] == "FREEZE-END":
            state.pop(owner, None)
        else:
            state[owner] = (epoch, [tok.rstrip(".,;") for tok in PATH_TOKEN.findall(match["text"])])
    active: dict[str, list[str]] = {}
    for owner, (epoch, paths) in state.items():
        age = now - epoch
        if age > ttl_s:
            print(
                f"tier2_queue: ignoring expired FREEZE-START from {owner} "
                f"(age {age / 3600:.1f} h > TTL {ttl_s / 3600:.1f} h, no FREEZE-END)",
                file=sys.stderr,
            )
            continue
        active[owner] = paths
    return active


def coord_freeze_lines(since: int = 259200) -> list[str]:
    try:
        out = subprocess.run(
            [sys.executable, "-m", "tools.agents.coord", "read", "--owner", "claude-sup",
             "--since", str(since), "--epoch"],
            capture_output=True, text=True, timeout=30, check=False,
        ).stdout  # fmt: skip
    except (OSError, subprocess.SubprocessError):
        print("tier2_queue: coord unreadable, assuming no freeze", file=sys.stderr)
        return []
    return [line for line in out.splitlines() if "[FREEZE-" in line]


def is_frozen(source: str, patterns: Iterable[str]) -> bool:
    return bool(source) and any(fnmatch.fnmatch(source, pat + "*") for pat in patterns if pat)


def read_deny(path: Path) -> set[str]:
    """Subsystems named in gate-deny.txt (first token per line)."""
    if not path.exists():
        return set()
    return {line.split()[0] for line in path.read_text().splitlines() if line.split()}


@dataclass
class Inputs:
    weights: list[dict[str, object]]
    ports: dict[int, dict[str, object]]
    credit: set[int]
    diverged: set[int]
    dispositions: dict
    deny: set[str] = field(default_factory=set)
    frozen: list[str] = field(default_factory=list)


def classify(inputs: Inputs) -> dict[str, dict[str, list[dict[str, object]]]]:
    """kind -> subsystem -> functions (weight descending)."""
    result: dict[str, dict[str, list[dict[str, object]]]] = {k: {} for k in KINDS}
    for row in sorted(inputs.weights, key=lambda r: (-int(r["calls"]), int(r["va"]))):  # type: ignore[call-overload]
        va, calls, subsystem = int(row["va"]), int(row["calls"]), str(row["subsystem"])  # type: ignore[call-overload]
        if calls <= 0 or subsystem in inputs.deny or slug(subsystem) in inputs.deny:
            continue
        if not port_subsystem(subsystem, inputs.dispositions):
            continue
        port = inputs.ports.get(va)
        source = str(port["source"]) if port else ""
        if port and is_frozen(source, inputs.frozen):
            continue
        if va in inputs.diverged and port:
            kind = "diagnose"
        elif port is None:
            kind = "port"
        elif va not in inputs.credit:
            kind = "capture"
        else:
            continue
        result[kind].setdefault(subsystem, []).append(
            {
                "va": fmt_va(va),
                "calls": calls,
                "source": source,
                "tier": port["tier"] if port else 0,
            }
        )
    return result


def next_numbers(existing: Iterable[str]) -> dict[str, int]:
    """subsystem slug -> highest used n, so regenerated items never reuse an id."""
    used: dict[str, int] = {}
    for item in existing:
        match = re.match(r"^t2_([a-z0-9-]+)_(\d+)$", item)
        if match:
            used[match.group(1)] = max(used.get(match.group(1), 0), int(match.group(2)))
    return used


def error_cause(text: str) -> str:
    low = text.lower()
    return next((cause for needle, cause in ERROR_CAUSES if needle in low), "other")


def lane_vas(data: dict) -> set[int]:
    """VAs with a row in crates/tsfp-ports/lanes.json."""
    return {
        va
        for lane in data.get("lanes", [])
        for root in lane.get("roots", [])
        if (va := norm_va(root.get("va", ""))) is not None
    }


def breakdown_source(text: str) -> Path | None:
    """The receipts directory named by the breakdown's ``Source: ... `path/` `` line."""
    match = re.search(r"^Source:.*?`([^`]*partial/?)`", text, re.M)
    return Path(match.group(1)) if match else None


def classify_breakdown(
    inputs: Inputs, receipts: Iterable[dict], rea_vas: set[int]
) -> dict[str, dict[str, list[dict[str, object]]]]:
    """Priority kinds from the round receipts. Subsystem keys carry the error cause for ``error``.

    ``rea_vas`` (REA registers these in lanes.json) never appear as ``no_abi``.
    Capture-fidelity errors (any counts.excluded entry) are REA's T3740 and are skipped.
    """
    weights = {int(r["va"]): r for r in inputs.weights}  # type: ignore[call-overload]
    result: dict[str, dict[str, list[dict[str, object]]]] = {k: {} for k in NEW_KINDS}
    for receipt in receipts:
        va = norm_va(str(receipt.get("va", "")))
        status = str(receipt.get("status"))
        if va is None or (status == "no_abi_entry" and va in rea_vas):
            continue
        row = weights.get(va)
        subsystem = str(row["subsystem"]) if row else ""
        if (row and int(row["calls"]) <= 0) or subsystem in inputs.deny:  # type: ignore[call-overload]
            continue
        if not port_subsystem(subsystem, inputs.dispositions):
            continue
        port = inputs.ports.get(va)
        entry: dict[str, object] = {
            "va": fmt_va(va),
            "calls": int(row["calls"]) if row else 0,  # type: ignore[call-overload]
            "source": str(port["source"]) if port else str(receipt.get("port_module", "")),
            "tier": port["tier"] if port else 0,
        }
        if status == "no_abi_entry":
            if port is None and not receipt.get("port_module"):
                continue  # no Rust port at all: that is a `port` item, not an ABI shim
            kind, key = "no_abi", subsystem
        elif status == "error":
            counts = receipt.get("counts")
            if isinstance(counts, dict) and counts.get("excluded"):
                continue
            kind, key = "error", f"{subsystem} {error_cause(str(receipt.get('error', '')))}"
        elif status in DIVERGED_STATUSES:
            ret_live = receipt.get("ret_live")
            if not isinstance(ret_live, dict) or any(
                not isinstance(ret_live.get(register), bool) for register in ("eax", "edx")
            ):
                continue  # do not queue the REA-gated item without a complete liveness result
            kind, key = "diverged_gated", subsystem
            entry["gate"] = GATE_NAME
        else:
            continue
        result[kind].setdefault(key, []).append(entry)
    for kind in NEW_KINDS:
        for funcs in result[kind].values():
            funcs.sort(
                key=lambda f: (
                    kind != "diverged_gated" or norm_va(str(f["va"])) not in GATED_SEEDS,
                    -int(f["calls"]),  # type: ignore[call-overload]
                    str(f["va"]),
                )
            )
    return result


def build_items(
    classified: dict[str, dict[str, list[dict[str, object]]]],
    used: dict[str, int],
    skip_vas: set[str] | None = None,
    family: int = FAMILY_SIZE,
    kinds: tuple[str, ...] = KINDS,
) -> list[dict[str, object]]:
    """Chunk each (kind, subsystem) list into items of up to ``family`` functions."""
    skip_vas = skip_vas or set()
    counters = dict(used)
    items: list[dict[str, object]] = []
    for kind in kinds:
        for subsystem, funcs in classified.get(kind, {}).items():
            funcs = [f for f in funcs if f["va"] not in skip_vas]
            for start in range(0, len(funcs), family):
                chunk = funcs[start : start + family]
                key = slug(subsystem)
                counters[key] = counters.get(key, 0) + 1
                items.append(
                    {
                        "id": f"t2_{key}_{counters[key]}",
                        "kind": kind,
                        "subsystem": subsystem,
                        "weight": sum(int(f["calls"]) for f in chunk),  # type: ignore[call-overload]
                        "functions": chunk,
                        "state": "open",
                        **({"gated_on": GATE_NAME} if kind == "diverged_gated" else {}),
                    }
                )
    if kinds == KINDS:
        items.sort(key=lambda it: (-int(it["weight"]), KINDS.index(str(it["kind"]))))  # type: ignore[call-overload]
    else:  # priority order: kind first (list order), then seeds, then weight
        items.sort(
            key=lambda it: (
                kinds.index(str(it["kind"])),
                not any(norm_va(str(f["va"])) in GATED_SEEDS for f in it["functions"]),  # type: ignore[attr-defined]
                -int(it["weight"]),  # type: ignore[call-overload]
            )
        )
    return items


def filter_prerequisite_items(
    items: list[dict[str, object]], prereqs: Prerequisites
) -> tuple[list[dict[str, object]], int]:
    """Keep individually eligible functions when a generated family is partly blocked."""
    kept_items = []
    rejected_functions = 0
    for item in items:
        kept_functions = []
        for function in item["functions"]:  # type: ignore[union-attr]
            probe = {**item, "functions": [dict(function)]}
            if prereqs.check(probe, route=False):
                rejected_functions += 1
            else:
                kept_functions.append(probe["functions"][0])  # type: ignore[index]
        if kept_functions:
            item["functions"] = kept_functions
            item["weight"] = sum(int(function["calls"]) for function in kept_functions)
            kept_items.append(item)
    return kept_items, rejected_functions


def select_items(
    items: list[dict[str, object]], limit: int, quota: dict[str, int]
) -> list[dict[str, object]]:
    """Top ``limit`` items: each kind gets its quota (weight order), the rest fills by weight."""
    chosen: list[dict[str, object]] = []
    for kind, count in quota.items():
        chosen += [it for it in items if it["kind"] == kind][:count]
    chosen = chosen[:limit]
    ids = {it["id"] for it in chosen}
    chosen += [it for it in items if it["id"] not in ids][: max(0, limit - len(chosen))]
    return sorted(chosen, key=lambda it: items.index(it))


def render_item(spec: dict) -> str:
    """Text appended to the brief: the item, per kind instructions and function table."""
    steps = {
        "port": "Port each function to readable Rust in a NEW leaf crate or behind a trait "
        "(rule 37), register it, differential test to Tier 1 against the lifted C.",
        "capture": "These are ported but have no Tier 2 credit. Use REA's bulk t2capture results "
        "(receipts/t2capture/<va>.json, docs/t3738-capture-design.md), then "
        "`python3 -m tools.t2capture_promote`; fix any port bug a capture exposes.",
        "diagnose": "These DIVERGED. Read the receipts, find the first diverging call, fix the "
        "port (or record the original's behaviour), re-check with the capture comparison.",
        "no_abi": "These are ported but the host has no ABI entry (receipt no_abi_entry). Add the "
        "ABI shim (src/host/rust_ports.c tables, via crates/tsfp-ports/lanes.json and "
        "`tools/lanes.py`) for ONLY the listed VAs, never rows REA registers, then re-check.",
        "error": "The capture host errored (receipt status error, cause in the subsystem). Find "
        "why (stub surface gap, unarmed read, timeout), fix the port or record the gap.",
        "diverged_gated": "These DIVERGED and are GATED on REA's exit_regs liveness check: do not "
        "start until it has landed. Then compare only the caller-live registers, fix real "
        "port bugs, record scratch-register cases.",
    }[spec["kind"]]
    lines = [
        f"YOUR TIER 2 ITEM {spec['id']} (kind {spec['kind']}, subsystem {spec['subsystem']}, "
        f"executed-call weight {spec['weight']}):",
        steps,
        PREREQUISITE_GUIDANCE,
        *([str(spec["note"])] if spec.get("note") else []),  # hand-written item instructions
        "va | executed calls | port tier | port source",
    ]
    lines += [
        f"{f['va']} | {f['calls']} | {f['tier']} | {f['source'] or '-'}" for f in spec["functions"]
    ]
    if spec["kind"] in NEW_KINDS:
        lines.append(CAPTURE_RERUN)
    for function in spec["functions"]:
        lines.extend(brief_evidence(function))
    return "\n".join(lines)


def spec_path(specs: Path, item_id: str) -> Path:
    return specs / f"{item_id}.json"


def load_spec(specs: Path, item_id: str) -> dict | None:
    if not ID_PATTERN.match(item_id):
        return None
    try:
        data = json.loads(spec_path(specs, item_id).read_text())
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def live_tier2_item_ids(root: Path, launched: Iterable[str]) -> set[str]:
    """Keep previously launched items active only while their worker branch or directory exists."""
    candidates = {item for item in launched if ID_PATTERN.fullmatch(item)}
    if not candidates:
        return set()
    try:
        refs = subprocess.run(
            [
                "git",
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads/task/cx-t2-*",
                "refs/remotes/origin/task/cx-t2-*",
            ],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=20,
            check=True,
        ).stdout.splitlines()
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            f"tier2_queue: cannot inspect active item branches ({type(exc).__name__}); "
            "retaining launched items",
            file=sys.stderr,
        )
        return candidates
    branch_names = {ref.removeprefix("origin/").removeprefix("task/").lower() for ref in refs}
    worker_root = root / "tmp/codexapi"
    try:
        worker_names = {path.name.lower() for path in worker_root.glob("CX-t2-*") if path.is_dir()}
    except OSError as exc:
        print(
            f"tier2_queue: cannot inspect active item workers ({type(exc).__name__}); "
            "retaining launched items",
            file=sys.stderr,
        )
        return candidates
    active = set()
    for item in candidates:
        stem = f"cx-{item.replace('_', '-')}"
        if any(name == stem or name.startswith(stem + "-") for name in branch_names | worker_names):
            active.add(item)
    return active


def load_inputs(root: Path, deny: set[str], frozen: list[str]) -> Inputs:
    def js(rel: str) -> dict:
        path = root / rel
        return json.loads(path.read_text()) if path.exists() else {}

    return Inputs(
        weights=load_weights((root / WEIGHTS).read_text()),
        ports=registry_ports(js(REGISTRY)),
        credit=ledger_credit(js(LEDGER), js(CAPTURE_LEDGER)),
        diverged=diverged_vas(js(LEDGER), load_receipts(root / RECEIPTS)),
        dispositions=js(DISPOSITIONS).get("systems", {}),
        deny=deny,
        frozen=frozen,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)
    gen = sub.add_parser("generate", help="emit items, write specs, optionally queue them")
    gen.add_argument("--root", type=Path, default=Path("."), help="repo checkout to read")
    gen.add_argument("--specs", type=Path, default=DEFAULT_SPECS)
    gen.add_argument("--queue", type=Path, default=DEFAULT_QUEUE)
    gen.add_argument("--deny", type=Path, default=DEFAULT_DENY)
    gen.add_argument("--launched", type=Path, default=DEFAULT_LAUNCHED)
    gen.add_argument("--max-items", type=int, default=40)
    gen.add_argument("--family", type=int, default=FAMILY_SIZE)
    gen.add_argument("--breakdown", type=Path, help="t2round-breakdown.md: priority kinds")
    gen.add_argument("--partial", type=Path, help="receipts dir (default: breakdown Source line)")
    gen.add_argument("--exclude-vas", type=Path, help="file of VAs REA registers (one per line)")
    gen.add_argument("--include-gated", action="store_true", help="queue diverged_gated items too")
    gen.add_argument("--append", action="store_true", help="write specs and append ids to queue")
    gen.add_argument("--no-coord", action="store_true", help="skip the freeze lookup")
    show = sub.add_parser("show", help="print the brief text of one item")
    show.add_argument("item_id")
    show.add_argument("--specs", type=Path, default=DEFAULT_SPECS)
    show.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    if args.cmd == "show":
        spec = load_spec(args.specs, args.item_id)
        if spec is None:
            print(f"no spec for {args.item_id}", file=sys.stderr)
            return 1
        failures = Prerequisites(args.root).check(spec, route=True)
        if failures:
            print("Tier 2 prerequisites: " + "; ".join(failures), file=sys.stderr)
            return 2
        save_evidence(spec, args.specs)
        print(render_item(spec))
        return 0
    frozen: list[str] = []
    if not args.no_coord:
        for owner, paths in active_freezes(coord_freeze_lines()).items():
            if "split4" in owner:
                frozen += paths
    inputs = load_inputs(args.root, read_deny(args.deny), frozen)
    queued = args.queue.read_text().split() if args.queue.exists() else []
    launched = args.launched.read_text().split() if args.launched.exists() else []
    spec_ids = [p.stem for p in args.specs.glob("t2_*.json")]
    active_ids = [*queued, *live_tier2_item_ids(args.root, launched)]
    existing = [*queued, *launched, *spec_ids]
    skip = {
        str(f["va"])
        for item_id in set(active_ids)
        if (spec := load_spec(args.specs, item_id)) and spec.get("state", "open") == "open"
        for f in spec["functions"]
    }
    kinds = KINDS
    classified = classify(inputs)
    append_from_current_receipts = args.append and not args.breakdown
    if args.breakdown or append_from_current_receipts:
        partial = args.partial or (
            breakdown_source(args.breakdown.read_text()) if args.breakdown else DEFAULT_RECEIPTS
        )
        if partial and not partial.is_absolute() and not partial.exists():
            partial = args.breakdown.resolve().parent.parent / partial  # workspace-relative
        receipts = load_receipts(partial) if partial else []
        if not receipts:
            print("tier2_queue: breakdown receipts missing, using fallback kinds", file=sys.stderr)
        else:
            lanes = args.root / "crates/tsfp-ports/lanes.json"
            rea = lane_vas(json.loads(lanes.read_text()) if lanes.exists() else {})
            rea |= set(inputs.ports)  # groups A and B: REA registers every already-ported VA
            if args.exclude_vas and args.exclude_vas.exists():
                rea |= {v for t in args.exclude_vas.read_text().split() if (v := norm_va(t))}
            fresh = classify_breakdown(inputs, receipts, rea)
            claimed = {norm_va(str(f["va"])) for k in fresh.values() for v in k.values() for f in v}
            claimed |= {va for va in inputs.diverged}
            for kind in KINDS:  # fallback kinds keep what the priority kinds do not claim
                classified[kind] = {
                    s: kept
                    for s, fs in classified[kind].items()
                    if (kept := [f for f in fs if norm_va(str(f["va"])) not in claimed])
                }
            classified.update(fresh)
            kinds = PRIORITY_KINDS
    items = build_items(classified, next_numbers(existing), skip, args.family, kinds)
    if args.append:
        items, rejected = filter_prerequisite_items(items, Prerequisites(args.root))
        if rejected:
            print(
                f"tier2_queue: skipped {rejected} function(s) with missing prerequisites",
                file=sys.stderr,
            )
    counts = {kind: sum(1 for it in items if it["kind"] == kind) for kind in kinds}
    funcs = {
        kind: sum(len(it["functions"]) for it in items if it["kind"] == kind) for kind in kinds
    }  # type: ignore[arg-type]
    print(f"items per kind {counts}; functions per kind {funcs}", file=sys.stderr)
    if kinds == KINDS:
        items = select_items(items, args.max_items, DEFAULT_QUOTA)
    else:
        # The automatic append path classifies the current receipt set and only creates
        # diverged_gated groups when both REA exit-register liveness results are explicit booleans.
        if not (args.include_gated or append_from_current_receipts):
            items = [it for it in items if it["kind"] != "diverged_gated"]
        items = items[: args.max_items]
    for item in items:
        print(f"{item['id']}\t{item['kind']}\t{item['subsystem']}\t{item['weight']}")
    if args.append:
        args.specs.mkdir(parents=True, exist_ok=True)
        for item in items:
            spec_path(args.specs, str(item["id"])).write_text(json.dumps(item, indent=1) + "\n")
        with args.queue.open("a") as out:
            out.writelines(f"{item['id']}\n" for item in items)
        print(f"queued {len(items)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
