# SPDX-License-Identifier: GPL-3.0-or-later
"""Credit Tier 2 (source `capture`, or a T3793 harness label) from t2capture receipts.

    python3 -m tools.t2capture_promote --receipts <dir> [--dry-run]

Reads `receipts/t2capture/<va>.json` (schema: docs/t3738-capture-design.md) and records
credit in docs/data/tier2-capture-evidence.json. Fail closed: every refusal prints its
reason, an unknown "current" digest refuses, whole-game credit is never touched.
Imports only the standard library (merge_task runs under the system python).
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

EVIDENCE_PATH = Path("docs/data/tier2-capture-evidence.json")
REGISTRY_PATH = Path("docs/data/port-registry.json")
# Smallest number of compared corpus entries that earns capture credit. The corpus coverage
# query asks for 3 shapes per function (tools/t2corpus --min-shapes); a receipt's own
# `min_samples_policy` may raise, never lower, this floor.
CAPTURE_MIN_SAMPLES = 3
# Exclusion rule keys the comparator is tested for. Any other key refuses (untested rule).
TESTED_EXCLUSIONS = ("capture_unfaithful",)
TESTED_EXCLUSION_PREFIXES = ("capture_incomplete:",)
# Statuses that can never promote, listed so the refusal names them.
BLOCKING_STATUSES = (
    "diverged",
    "no_abi_entry",
    "capture_incomplete",
    "incomplete_size",  # v9 (T3832): size-flagged calls unjudged, a capped port could hide there
    "no_samples",
    "error",
)
# Credit labels (T3793, rule 9(b)): recording-based capture, and fabricated-input harness runs.
RECORDING_SOURCE = "capture"
HARNESS_SOURCES = ("constructed-call", "harness-service")
# T3875: symbolic equivalence proof (tools.t2symbolic), ranked below recording capture.
SYMBOLIC_SOURCE = "symbolic"
PROOF_VERDICT = "proved_equal"


def receipt_source(receipt: dict) -> object:
    """The receipt's credit label; no `tier2_source` means a recording capture (pre-T3793)."""
    return receipt.get("tier2_source", RECORDING_SOURCE)


def source_refusals(receipt: dict) -> list[str]:
    """Label checks: a known label, and harness labels carry inputs kind, generator and seed."""
    source = receipt_source(receipt)
    if source in (RECORDING_SOURCE, SYMBOLIC_SOURCE):
        return []
    if source not in HARNESS_SOURCES:
        return [f"unknown tier2_source label {source!r}"]
    inputs = receipt.get("inputs")
    inputs = inputs if isinstance(inputs, dict) else {}
    out = [
        f"harness receipt ({source}) without inputs.{field}"
        for field in ("kind", "generator")
        if not (isinstance(inputs.get(field), str) and inputs[field])
    ]
    if type(inputs.get("seed")) is not int:
        out.append(f"harness receipt ({source}) without an integer inputs.seed")
    return out


def _unavailable() -> None:
    return None


@dataclass
class Currents:
    """What 'current' means. Each callable returns the value, or None = unknown = refuse."""

    compare_logic: Callable[[], str | None] = _unavailable
    lifted_tree_hash: Callable[[], str | None] = _unavailable
    # (port_module) -> last commit touching its source file, None when unknown.
    port_source_commit: Callable[[str], str | None] = lambda module: None
    # Version of the symbolic checker (Kani); symbolic proofs only.
    symbolic_tool: Callable[[], str | None] = _unavailable


def current_compare_logic() -> str | None:
    """tools.t2capture.runner.compare_logic_digest(): the digest the checker writes into every
    receipt (I-T3744-2: the whole-game gate's digest is a different code set and never matched)."""
    try:
        from tools.t2capture import runner

        return runner.compare_logic_digest()
    except Exception:  # noqa: BLE001 - unreadable digest must refuse, not crash
        return None


LIFT_ENV = "TSFP_T2_LIFTED_TREE"


def _complete_lifts(work: Path) -> list[Path]:
    """Published lifts (as tools.private_host.complete_lift: summary and KEY.txt, no stage)."""
    if not work.is_dir():
        return []
    return [
        lift
        for lift in work.glob("lift-*")
        if ".tmp-" not in lift.name
        and (lift / "summary/summary.json").is_file()
        and (lift / "KEY.txt").is_file()
    ]


def current_lifted_tree_hash(root: Path = Path(".")) -> str | None:
    """The lifted tree in force: `lift-<key>`, the same content key the capture build writes into
    the spool header (CMakeLists TSFP_CAPTURE_TREE_HASH). $TSFP_T2_LIFTED_TREE overrides. The
    newest complete lift wins, as tools.private_host.owner_lift_key orders them; none = unknown."""
    override = os.environ.get(LIFT_ENV, "").strip()
    if override:
        return override
    try:
        from tools.private_host import WORK
    except Exception:  # noqa: BLE001 - no lift layout known: unknown, refuse
        return None
    for work in (root / WORK, _main_checkout(root) / WORK):
        lifts = _complete_lifts(work)
        if lifts:
            return max(lifts, key=lambda lift: lift.stat().st_mtime).name
    return None


def _main_checkout(root: Path) -> Path:
    done = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=root, capture_output=True, text=True, timeout=30, check=False,
    )  # fmt: skip
    return (
        Path(done.stdout.strip()).parent if done.returncode == 0 and done.stdout.strip() else root
    )


def port_source_file(root: Path, module: str) -> str | None:
    registry = root / REGISTRY_PATH
    if not registry.is_file():
        return None
    for entries in json.loads(registry.read_text(encoding="utf-8")).get("ports", {}).values():
        for entry in entries:
            if entry.get("module") == module and entry.get("source"):
                return str(entry["source"])
    return None


def git_last_commit(root: Path, module: str) -> str | None:
    """Last commit touching the module's source file (port registry `source`)."""
    source = port_source_file(root, module)
    if source is None:
        return None
    done = subprocess.run(
        ["git", "log", "-1", "--format=%H", "--", source],
        cwd=root, capture_output=True, text=True, timeout=60, check=False,
    )  # fmt: skip
    return done.stdout.strip() or None if done.returncode == 0 else None


def port_changed(root: Path, entries: dict[str, dict]) -> list[str]:
    """VAs of `entries` (evidence entries or receipts) whose port source file has a newer
    last commit than the entry's `port_source_commit`. One registry read, one git log per
    source file."""
    registry = root / REGISTRY_PATH
    sources: dict[str, str] = {}
    if registry.is_file():
        for ports in json.loads(registry.read_text(encoding="utf-8")).get("ports", {}).values():
            for port in ports:
                if port.get("module") and port.get("source"):
                    sources.setdefault(str(port["module"]), str(port["source"]))
    commits: dict[str, str | None] = {}
    stale = []
    for va, entry in sorted(entries.items()):
        source = sources.get(str(entry.get("port_module", "")))
        if source is None:
            continue
        if source not in commits:
            done = subprocess.run(
                ["git", "log", "-1", "--format=%H", "--", source],
                cwd=root, capture_output=True, text=True, timeout=60, check=False,
            )  # fmt: skip
            commits[source] = done.stdout.strip() or None if done.returncode == 0 else None
        if commits[source] is not None and entry.get("port_source_commit") != commits[source]:
            stale.append(va)
    return stale


def is_harness_entry(entry: object) -> bool:
    """Evidence credited by a harness lane (T3796, I-T3784-9): t2-continuous never re-checks
    it against the recording corpus, its re-check is the harness lane's job."""
    return isinstance(entry, dict) and entry.get("source") in (*HARNESS_SOURCES, SYMBOLIC_SOURCE)


def stale_port_entries(root: Path, *, harness: bool = False) -> list[str]:
    """Credited VAs whose port source changed after the receipt, credit kept until a re-check.
    `harness=False` is recording credit only (the ledger column "stale (port changed, recheck
    pending)"), `harness=True` is harness credit ("harness stale (harness lane rerun
    pending)", I-T3784-9)."""
    path = root / EVIDENCE_PATH
    if not path.is_file():
        return []
    functions = json.loads(path.read_text(encoding="utf-8")).get("functions", {})
    stale = port_changed(
        root, {va: e for va, e in functions.items() if is_harness_entry(e) == harness}
    )
    if harness:  # T3875: a symbolic proof is also stale once the lift moved
        lift = current_lifted_tree_hash(root)
        moved = [
            va
            for va, e in functions.items()
            if e.get("source") == SYMBOLIC_SOURCE
            and lift is not None
            and e.get("lifted_tree_hash") != lift
        ]
        stale = sorted(set(stale) | set(moved))
    return stale


def stale_receipts(root: Path, receipts_dir: Path) -> list[str]:
    """VAs whose newest stored receipt predates a change of their port source."""
    entries: dict[str, dict] = {}
    for file in receipts_dir.glob("0x*.json"):
        try:
            receipt = json.loads(file.read_text(encoding="utf-8"))
            entries[f"0x{int(str(receipt['va']), 16):08X}"] = receipt
        except (OSError, ValueError, KeyError, TypeError):
            continue
    return port_changed(root, entries)


def current_symbolic_tool() -> str | None:
    """The `cargo kani --version` line, or None when Kani is not installed (fail closed)."""
    try:
        from tools.t2symbolic import runner

        return runner.kani_version()
    except Exception:  # noqa: BLE001 - unknown tool version must refuse, not crash
        return None


def real_currents(root: Path) -> Currents:
    return Currents(
        current_compare_logic,
        lambda: current_lifted_tree_hash(root),
        lambda module: git_last_commit(root, module),
        current_symbolic_tool,
    )


def _count(value: object) -> int | None:
    return value if type(value) is int and value >= 0 else None


def refusals(receipt: object, currents: Currents) -> list[str]:
    """Every reason this receipt cannot earn capture credit (empty = promotable)."""
    if not isinstance(receipt, dict):
        return ["receipt is not a JSON object"]
    if receipt.get("schema") != 1 or receipt.get("kind") != "t2capture":
        return ["not a schema 1 t2capture receipt"]
    if receipt_source(receipt) == SYMBOLIC_SOURCE:
        return symbolic_refusals(receipt, currents)
    out: list[str] = source_refusals(receipt)
    status = receipt.get("status")
    if status != "equal":
        out.append(f"status is {status!r}, not 'equal'")
    counts = receipt.get("counts")
    counts = counts if isinstance(counts, dict) else {}
    compared, equal = _count(counts.get("compared")), _count(counts.get("equal"))
    diverged, fp_ulp = _count(counts.get("diverged")), _count(counts.get("fp_ulp"))
    excluded = counts.get("excluded")
    if None in (compared, equal, diverged, fp_ulp) or not isinstance(excluded, dict):
        return [*out, "counts are missing or malformed"]
    skipped = 0
    for key, number in excluded.items():
        known = key in TESTED_EXCLUSIONS or any(
            key.startswith(p) and len(key) > len(p) for p in TESTED_EXCLUSION_PREFIXES
        )
        if not known:
            out.append(f"excluded by untested rule key {key!r}")
        if _count(number) is None:
            out.append(f"excluded count for {key!r} is malformed")
        else:
            skipped += number
    if equal != compared - skipped:
        out.append(f"equal {equal} != compared {compared} - excluded {skipped}")
    if diverged != 0:
        out.append(f"diverged is {diverged}")
    if fp_ulp != 0:
        out.append(f"fp_ulp is {fp_ulp} (not credited)")
    policy = _count(receipt.get("min_samples_policy")) or 0
    floor = max(CAPTURE_MIN_SAMPLES, policy)
    if compared < floor:
        out.append(f"compared {compared} < minimum sample count {floor}")
    for field, getter in (
        ("compare_logic", currents.compare_logic),
        ("lifted_tree_hash", currents.lifted_tree_hash),
    ):
        now = getter()
        if now is None:
            out.append(f"current {field} unknown (fail closed)")
        elif receipt.get(field) != now:
            out.append(f"stale {field}: receipt {receipt.get(field)!r} != current {now!r}")
    module = receipt.get("port_module")
    commit = currents.port_source_commit(module) if isinstance(module, str) and module else None
    if commit is None:
        out.append(f"last commit of port module {module!r} unknown (fail closed)")
    elif receipt.get("port_source_commit") != commit:
        out.append(f"port source changed since {receipt.get('port_source_commit')!r} ({commit})")
    return out


def symbolic_refusals(receipt: dict, currents: Currents) -> list[str]:
    """T3875: a proof credits only when `proved_equal` and nothing it was proved against moved:
    the port source commit, the lift (`lift-<key>`) and the Kani version. Unknown = refuse."""
    proof = receipt.get("proof")
    proof = proof if isinstance(proof, dict) else {}
    out = []
    if receipt.get("status") != "equal" or proof.get("verdict") != PROOF_VERDICT:
        out.append(f"proof verdict is {proof.get('verdict')!r}, not {PROOF_VERDICT!r}")
    if not (isinstance(proof.get("unwind"), int) and proof["unwind"] > 0):
        out.append("proof without a positive unwind bound")
    for label, field, getter in (
        ("lifted_tree_hash", receipt.get("lifted_tree_hash"), currents.lifted_tree_hash),
        ("tool version", proof.get("tool_version"), currents.symbolic_tool),
    ):
        now = getter()
        if now is None:
            out.append(f"current {label} unknown (fail closed)")
        elif field != now:
            out.append(f"stale {label}: receipt {field!r} != current {now!r}")
    module = receipt.get("port_module")
    commit = currents.port_source_commit(module) if isinstance(module, str) and module else None
    if commit is None:
        out.append(f"last commit of port module {module!r} unknown (fail closed)")
    elif receipt.get("port_source_commit") != commit:
        out.append(f"port source changed since {receipt.get('port_source_commit')!r} ({commit})")
    return out


def load_evidence(path: Path) -> dict:
    if path.is_file():
        return json.loads(path.read_text(encoding="utf-8"))
    return {"schema": 1, "functions": {}}


def whole_game_vas(root: Path) -> set[int]:
    """Whole-game Tier 2 credits as the ledger counts them (port-class functions only, I-T3744-6).

    A non-port VA's whole-game evidence is never counted by `tools.ledger tier2`, so it must not
    hide a capture receipt either. Without a placement table every whole-game VA is skipped
    (fail closed: no double credit), with a warning.
    """
    import sys

    from tools import verification
    from tools.ledger import dispositions

    port = dispositions.tier2_port_vas(root)
    if port is None:
        print(
            f"t2capture_promote: {dispositions.SUBSYSTEMS_PATH} missing, every whole-game VA "
            "is skipped (ledger may count fewer)",
            file=sys.stderr,
        )
    records = verification.load_records(
        root / verification.DEFAULT_PATH,
        c2rust_status=root / "docs/data/c2rust-status.csv",
        tier2_path=root / verification.DEFAULT_TIER2_PATH,
        tier2_functions=port,
        tier2_capture_path=None,
    )
    # verification-tiers.json Tier 2 rows bypass `tier2_functions`: filter them the same way.
    return {
        va
        for va, item in records.items()
        if verification.is_tier2(item["tier"]) and (port is None or va in port)
    }


# Refusal prefixes that are a real negative verdict on an existing credit (T3744 demotion).
# claude-1 2026-10-26 18:52 (I-T3784-8): staleness alone (compare rule, lift or port source
# changed since the receipt) never demotes; it only refuses, the credit stays and is reported
# as "stale (port changed, recheck pending)" until a fresh re-check. Only an actual DIVERGED
# or ERROR verdict (or diverged/fp_ulp counts) demotes.
DEMOTING_REASON_PREFIXES = ("diverged is ", "fp_ulp is ")
# v9 (T3832) made `incomplete_size` withdraw credit; T3867 reverts that for EXISTING credit: the
# status says the capture was too small to judge (size-flagged calls unjudged), not that the port
# is wrong. The credit stays, marked pending recheck; it is never granted on that status (refusals).
DEMOTING_STATUSES = ("diverged", "error")
PENDING_RECHECK = "pending_recheck"


def demotion_reasons(receipt: object, reasons: list[str]) -> list[str]:
    """The subset of `reasons` that withdraws existing capture credit (empty = keep it)."""
    if not isinstance(receipt, dict):
        return []
    if any(r.startswith(("stale ", "port source changed")) for r in reasons):
        return []  # a verdict from an older port or rule says nothing about the current one
    out = [r for r in reasons if r.startswith(DEMOTING_REASON_PREFIXES)]
    if receipt.get("status") in DEMOTING_STATUSES:
        out.insert(0, f"status is {receipt.get('status')!r}")
    return out


def _recording_kept(evidence: dict, key: str | None, receipt: object) -> bool:
    """Precedence (T3793): recording credit wins, a harness receipt never touches it."""
    existing = evidence["functions"].get(key) if key else None
    return (
        isinstance(existing, dict)
        and existing.get("source") == RECORDING_SOURCE
        and isinstance(receipt, dict)
        and receipt_source(receipt) != RECORDING_SOURCE
    )


SINGLE_PATH = "single-path"
SHAPE_CAP = 3  # tools.t2capture.path_count.CAP: the flat shape minimum of the coverage query


def shape_basis(va: int, paths: dict[int, int]) -> dict:
    """T3795 audit field (not an eligibility rule): a function whose lifted CFG has fewer than
    SHAPE_CAP entry-to-exit paths is asked for fewer shapes by the coverage query, and its
    evidence says so. An unknown count is the flat minimum ("shapes")."""
    count = paths.get(va)
    reduced = count is not None and count < SHAPE_CAP
    return {"cfg_paths": count, "shape_basis": SINGLE_PATH if reduced else "shapes"}


def load_paths(tree: str | None) -> dict[int, int]:
    try:
        from tools.t2capture import path_count

        return path_count.load(tree or "")
    except Exception:  # noqa: BLE001 - no table: every entry is labelled "shapes" (fail closed)
        return {}


def promote(
    receipts_dir: Path,
    root: Path,
    currents: Currents,
    *,
    dry_run: bool = False,
    paths: dict[int, int] | None = None,
) -> tuple[list[str], list[str]]:
    """(promoted VAs, messages). Writes the evidence file unless dry_run.

    Demotion: a receipt for a VA already in the evidence file that is refused for a real
    negative verdict (status in BLOCKING_STATUSES, diverged > 0, fp_ulp > 0) removes the
    entry and emits "DEMOTE <va> (<reason>)" ("WOULD DEMOTE" in dry run). Stale currents
    (compare_logic, lifted_tree_hash, port_source_commit mismatch) also demote, because
    credit requires matching currents. Unreadable JSON, a bad va, malformed counts or
    unknown currents only refuse: they say nothing about the function, credit stays.
    The file is written when anything was promoted or demoted.
    Labels (T3793): one entry per VA, `source` is `capture` (recording) or a harness label.
    A harness receipt never replaces nor demotes a `capture` entry; a recording receipt
    replaces a harness entry.
    """
    path = root / EVIDENCE_PATH
    evidence = load_evidence(path)
    whole_game = whole_game_vas(root)
    promoted: list[str] = []
    demoted: list[str] = []
    messages: list[str] = []
    changed = False
    for file in sorted(receipts_dir.glob("*.json")):
        try:
            receipt = json.loads(file.read_text(encoding="utf-8"))
        except ValueError as error:
            messages.append(f"REFUSE {file.name}: unreadable JSON ({error})")
            continue
        reasons = refusals(receipt, currents)
        va = receipt.get("va") if isinstance(receipt, dict) else None
        key = None
        try:
            key = f"0x{int(str(va), 16):08X}"
        except ValueError:
            reasons.append(f"va {va!r} is not a hex address")
        if reasons:
            messages.extend(f"REFUSE {file.name}: {reason}" for reason in reasons)
            demote = demotion_reasons(receipt, reasons) if key else []
            held = evidence["functions"].get(key) if key else None
            if (
                isinstance(receipt, dict)
                and receipt.get("status") == "incomplete_size"
                and isinstance(held, dict)
            ):
                held[PENDING_RECHECK] = "incomplete_size"
                messages.append(f"KEEP {key}: incomplete_size, credit kept pending recheck")
                changed = True
            if demote and _recording_kept(evidence, key, receipt):
                messages.append(f"KEEP {key}: a harness receipt never demotes recording credit")
                demote = []
            if demote and key in evidence["functions"]:
                del evidence["functions"][key]
                if key in promoted:
                    promoted.remove(key)
                demoted.append(key)
                verb = "WOULD DEMOTE" if dry_run else "DEMOTE"
                messages.append(f"{verb} {key} ({'; '.join(demote)})")
            continue
        held = evidence["functions"].get(key)
        if (
            receipt_source(receipt) == SYMBOLIC_SOURCE
            and isinstance(held, dict)
            and held.get("source") not in (None, SYMBOLIC_SOURCE)
        ):
            messages.append(f"SKIP {file.name}: {key} keeps its {held.get('source')} credit")
            continue
        if int(key, 16) in whole_game:
            messages.append(f"SKIP {file.name}: {key} already has whole-game Tier 2 credit")
            continue
        source = receipt_source(receipt)
        if _recording_kept(evidence, key, receipt):
            messages.append(f"SKIP {file.name}: {key} keeps its recording capture credit")
            continue
        entry = {
            k: receipt.get(k)
            for k in (
                "status", "counts", "compare_logic", "corpus_digest", "lifted_tree_hash",
                "port_module", "port_source_commit", "build_input_hash", "created",
            )
        }  # fmt: skip
        entry["source"] = source
        if source in HARNESS_SOURCES:
            entry["inputs"] = receipt["inputs"]
        if source == SYMBOLIC_SOURCE:
            entry["proof"] = receipt["proof"]
            entry.pop("counts")
            entry.pop("compare_logic")
            entry.pop("corpus_digest")
        entry.update(shape_basis(int(key, 16), paths or {}))
        evidence["functions"][key] = entry
        promoted.append(key)
        messages.append(f"{'WOULD PROMOTE' if dry_run else 'PROMOTE'} {key} ({source})")
    if (promoted or demoted or changed) and not dry_run:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
        )
    return promoted, messages


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--receipts", required=True, type=Path, help="dir of <va>.json receipts")
    parser.add_argument("--root", type=Path, default=Path("."), help="repo root")
    parser.add_argument("--dry-run", action="store_true", help="print, change nothing")
    args = parser.parse_args(argv)
    promoted, messages = promote(
        args.receipts,
        args.root,
        real_currents(args.root),
        dry_run=args.dry_run,
        paths=load_paths(current_lifted_tree_hash(args.root)),
    )
    print("\n".join(messages))
    demoted = sum(1 for m in messages if m.startswith(("DEMOTE ", "WOULD DEMOTE ")))
    would = "would be " if args.dry_run else ""
    print(f"{len(promoted)} function(s) {would}promoted, {demoted} {would}demoted")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
