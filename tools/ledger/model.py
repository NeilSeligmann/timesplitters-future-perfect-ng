# SPDX-License-Identifier: GPL-3.0-or-later
"""Obligation definitions and the pure per-function ledger builder (no file or network access)."""

from __future__ import annotations

import re
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass, field

from tools import verification

DONE, OPEN, BLOCKED, NA = "done", "open", "blocked", "na"
STATUSES = (DONE, OPEN, BLOCKED, NA)

# (id, one line meaning). Order is the report column order.
OBLIGATIONS: tuple[tuple[str, str], ...] = (
    ("name", "name with evidence line"),
    ("subsystem", "placed in a subsystem leaf"),
    ("signature", "signature and calling convention known"),
    ("layouts", "struct layouts of arguments and globals known (T1807 types)"),
    ("callers", "callers contract known"),
    ("behaviour", "behaviour documented in docs/"),
    ("reference", "C replacement exists and passes a fixture proof or byte-exact match"),
    ("rust_port", "Rust port exists and passes the harness lane"),
    ("hybrid_parity", "A/B hybrid switch parity verified by golden replay"),
    ("formats", "data formats it reads have Rust parsers (T1813)"),
)
OBLIGATION_IDS = tuple(name for name, _ in OBLIGATIONS)
# obligation -> obligations that must be done before it can be worked (else status blocked)
PREREQUISITES: dict[str, tuple[str, ...]] = {
    "rust_port": ("signature",),
    "hybrid_parity": ("rust_port",),
}
PLACEHOLDER = re.compile(r"^(sub|fun|unk|loc|dat)_[0-9a-f]+$", re.IGNORECASE)
FORMAT_SUBSYSTEMS = ("data/",)


@dataclass(frozen=True)
class Facts:
    """Everything the builder reads, already parsed. Keys of every mapping are integer VAs."""

    verification_records: dict[int, dict] = field(default_factory=dict)
    names: Mapping[int, tuple[str, str, str]] = field(
        default_factory=dict
    )  # va -> name, conf, evidence
    subsystems: Mapping[int, str] = field(default_factory=dict)
    proofs: Mapping[int, Mapping[str, object]] = field(
        default_factory=dict
    )  # snapshot function rows
    doc_mentions: Mapping[int, str] = field(default_factory=dict)  # va -> first doc path
    overlay: Mapping[str, object] = field(default_factory=dict)  # tools/data/ledger_facts.json
    edges: Mapping[int, tuple[int, ...]] = field(default_factory=dict)  # caller -> callees
    rust_ports: Mapping[int, str] = field(default_factory=dict)  # va -> evidence
    weak_ports: Mapping[int, str] = field(
        default_factory=dict
    )  # va -> note, doc-leader only (T1997)


def fmt_va(va: int) -> str:
    return f"0x{va:08x}"


def parse_va(text: str) -> int:
    return int(text, 16)


def _overlay_map(overlay: Mapping[str, object], key: str) -> dict[int, str]:
    raw = overlay.get(key, {})
    assert isinstance(raw, dict)
    return {parse_va(str(va)): str(evidence) for va, evidence in raw.items()}


def entry(status: str, evidence: str = "", note: str = "") -> dict[str, str]:
    row = {"status": status}
    if evidence:
        row["evidence"] = evidence
    if note:
        row["note"] = note
    return row


def judge_reference(proof: Mapping[str, object] | None) -> dict[str, str]:
    if proof is None:
        return entry(OPEN, note="no C replacement registered in the proof snapshot")
    ref = f"docs/data/replace-proof-snapshot.json#{proof.get('va')} fp={str(proof.get('fingerprint', ''))[:12]}"  # noqa: E501
    gate = proof.get("gate")
    if gate == "not-a-game-function":
        return entry(NA, ref, "not a game function")
    agree, disagree = int(proof.get("agree", 0) or 0), int(proof.get("disagree", 0) or 0)
    if gate is None and disagree == 0 and agree > 0 and proof.get("replaced_confirmed"):
        return entry(DONE, ref, f"agree={agree}")
    return entry(OPEN, ref, f"gate={gate} agree={agree} disagree={disagree}")


def build_function(
    va: int,
    facts: Facts,
    slices: Mapping[str, tuple[frozenset[int], tuple[str, ...], tuple[str, ...]]],
) -> dict[str, object]:
    overlay = facts.overlay
    named = facts.names.get(va)
    name = named[0] if named else ""
    subsystem = facts.subsystems.get(va, "unknown")
    proof = facts.proofs.get(va)
    obligations: dict[str, dict[str, str]] = {}

    if named and not PLACEHOLDER.match(name) and named[2].strip():
        obligations["name"] = entry(
            DONE, f"tools/data/function_names.csv {named[1]}", named[2][:80]
        )
    else:
        obligations["name"] = entry(OPEN, note="no evidence-backed name")
    obligations["subsystem"] = (
        entry(DONE, f"tools/data/function_subsystems.csv {subsystem}")
        if subsystem not in ("", "unknown")
        else entry(OPEN, note="subsystem unknown")
    )
    if proof and proof.get("convention"):
        obligations["signature"] = entry(
            DONE,
            f"proof snapshot convention={proof['convention']} stack_args={proof.get('stack_args')}",
        )
    else:
        obligations["signature"] = entry(OPEN, note="no judged convention")
    layouts = _overlay_map(overlay, "layouts")
    obligations["layouts"] = (
        entry(DONE, layouts[va])
        if va in layouts
        else entry(OPEN, note="no layout evidence (T1807)")
    )
    callers = _overlay_map(overlay, "callers")
    obligations["callers"] = entry(DONE, callers[va]) if va in callers else entry(OPEN)
    doc = facts.doc_mentions.get(va)
    obligations["behaviour"] = (
        entry(DONE, doc) if doc else entry(OPEN, note="no docs/ mention of the address")
    )
    obligations["reference"] = judge_reference(proof)
    port = facts.rust_ports.get(va)
    if port:
        obligations["rust_port"] = entry(DONE, port)
    elif va in facts.weak_ports:
        obligations["rust_port"] = entry(OPEN, note=facts.weak_ports[va])
    else:
        obligations["rust_port"] = entry(OPEN)
    parity = _overlay_map(overlay, "hybrid_parity")
    obligations["hybrid_parity"] = (
        entry(DONE, parity[va]) if va in parity else entry(OPEN, note="no golden replay (T1818)")
    )
    parsers = _overlay_map(overlay, "formats")
    if va in parsers:
        obligations["formats"] = entry(DONE, parsers[va])
    elif subsystem.startswith(FORMAT_SUBSYSTEMS):
        obligations["formats"] = entry(OPEN, note="reads data formats, no Rust parser linked")
    else:
        obligations["formats"] = entry(NA, note="not a data-format function")

    for target, needs in PREREQUISITES.items():
        row = obligations[target]
        missing = [need for need in needs if obligations[need]["status"] != DONE]
        if row["status"] == OPEN and missing:
            row["status"] = BLOCKED
            row["note"] = "needs " + ",".join(missing)

    memberships = sorted(
        slice_name
        for slice_name, (vas, prefixes, subsystems) in slices.items()
        if va in vas
        or (name and any(name.startswith(prefix) for prefix in prefixes))
        or any(subsystem == s or subsystem.startswith(s + "/") for s in subsystems)
    )
    deps = sorted(facts.edges.get(va, ()))
    counts = {
        status: sum(1 for o in obligations.values() if o["status"] == status) for status in STATUSES
    }
    return {
        **verification.record(va, facts.verification_records),
        "va": fmt_va(va),
        "name": name,
        "subsystem": subsystem,
        "slices": memberships,
        "deps": [fmt_va(dep) for dep in deps],
        "obligations": obligations,
        "counts": counts,
    }


def parse_slices(
    overlay: Mapping[str, object],
) -> dict[str, tuple[frozenset[int], tuple[str, ...], tuple[str, ...]]]:
    raw = overlay.get("slices", {})
    assert isinstance(raw, dict)
    out = {}
    for slice_name, spec in sorted(raw.items()):
        out[slice_name] = (
            frozenset(parse_va(str(v)) for v in spec.get("vas", [])),
            tuple(spec.get("name_prefixes", [])),
            tuple(spec.get("subsystems", [])),
        )
    return out


def build_ledger(facts: Facts, *, include_library: bool = False) -> list[dict[str, object]]:
    vas = set(facts.subsystems) | set(facts.names)
    slices = parse_slices(facts.overlay)
    rows = []
    for va in sorted(vas):
        subsystem = facts.subsystems.get(va, "unknown")
        if not include_library and (subsystem == "library" or subsystem.startswith("library/")):
            continue
        rows.append(build_function(va, facts, slices))
    return rows


def mentions_from_docs(docs: Iterable[tuple[str, str]], wanted: Iterable[int]) -> dict[int, str]:
    """va -> first doc path (sorted by path) mentioning the address as 0x... in any case or width."""  # noqa: E501
    wanted_set = set(wanted)
    found: dict[int, str] = {}
    pattern = re.compile(r"0x([0-9a-fA-F]{4,8})\b")
    for path, text in sorted(docs):
        for match in pattern.finditer(text):
            va = int(match.group(1), 16)
            if va in wanted_set and va not in found:
                found[va] = path
    return found


def open_count(row: Mapping[str, object]) -> int:
    counts = row["counts"]
    assert isinstance(counts, dict)
    return int(counts[OPEN]) + int(counts[BLOCKED])


def pages(rows: Sequence[dict[str, object]], size: int) -> list[list[dict[str, object]]]:
    return [list(rows[i : i + size]) for i in range(0, len(rows), size)]
