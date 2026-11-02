# SPDX-License-Identifier: GPL-3.0-or-later
"""Classify never-executed port-class functions by how they can reach Tier 2 (T3793).

Exactly one class per function, decided in this order:

a  unreachable: dead in the static fixpoint (`tools.t2reach.graph`), MEASURED. Only this
   class may be excluded from the Tier 2 denominator.
b  needs a live service or a fault: calls Xbox Live / XNet library code that no executed
   function calls (MEASURED), or its name matches a `service` rule (INFERRED).
c  reachable by owner play, MEASURED variant: a code caller within `ancestor_depth` levels
   executed in a recording, or another function of the same live table (vtable, dispatch
   table) executed. The recording trigger comes from the session rules (INFERRED).
d  reachable by a constructed offline call: a small self-contained forward closure (no XDK
   library code, no kernel import, no dispatch table), MEASURED structure. Evidence from it is
   NOT original-recording Tier 2 and must be labelled so.
c  reachable by owner play, INFERRED variant: a session rule matches the name.
e  unknown.
"""

from __future__ import annotations

import re
from collections import deque
from collections.abc import Collection, Mapping
from dataclasses import dataclass

CLASSES = ("a", "b", "c", "d", "e")
CLASS_TITLES = {
    "a": "unreachable (static fixpoint)",
    "b": "needs a live service or a fault",
    "c": "reachable by owner play",
    "d": "reachable by a constructed offline call (not original-recording Tier 2)",
    "e": "unknown",
}


@dataclass(frozen=True)
class Facts:
    """What the classifier reads; built from the graph by `tools.t2reach.audit`."""

    nodes: Mapping[int, tuple[int, int]]  # start -> (start, end)
    edges: Mapping[int, Collection[int]]
    callers: Mapping[int, Collection[int]]
    live: frozenset[int]
    functions: frozenset[int]
    data_objects: frozenset[int]
    imports: Mapping[int, Collection[str]]
    executed: frozenset[int]
    sections: Mapping[str, tuple[int, int]]  # name -> [lo, hi)


@dataclass(frozen=True)
class Row:
    va: int
    subsystem: str
    name: str
    klass: str
    confidence: str
    evidence: str
    session: str = ""
    trigger: str = ""


def in_sections(va: int, names: Collection[str], sections: Mapping[str, tuple[int, int]]) -> bool:
    return any(sections[n][0] <= va < sections[n][1] for n in names if n in sections)


def offline_unused_service_calls(
    va: int, facts: Facts, targets: Collection[str]
) -> tuple[int, ...]:
    """Xbox Live / XNet library code `va` calls directly that NO executed function calls.

    Offline play also calls into those sections (random numbers, signatures), so a call alone
    proves nothing; a library routine that no recorded run ever reached does.
    """
    found = []
    for callee in sorted(facts.edges.get(va, ())):
        if callee in facts.data_objects or not in_sections(callee, targets, facts.sections):
            continue
        if not any(c in facts.executed for c in facts.callers.get(callee, ())):
            found.append(callee)
    return tuple(found)


def executed_ancestor(va: int, facts: Facts, depth: int) -> tuple[int, int] | None:
    """(executed live code caller, levels up) within `depth` code levels, nearest first."""
    seen = {va}
    frontier = [va]
    for level in range(1, depth + 1):
        following = []
        for node in frontier:
            for caller in sorted(facts.callers.get(node, ())):
                if caller in seen or caller in facts.data_objects or caller not in facts.live:
                    continue
                seen.add(caller)
                if caller in facts.executed:
                    return caller, level
                following.append(caller)
        frontier = following
    return None


def executed_sibling(va: int, facts: Facts) -> tuple[int, int] | None:
    """(live table that names `va`, an executed function the same table names)."""
    for table in sorted(facts.callers.get(va, ())):
        if table not in facts.data_objects or table not in facts.live:
            continue
        for other in sorted(facts.edges.get(table, ())):
            if other != va and other in facts.executed:
                return table, other
    return None


def self_contained(va: int, facts: Facts, rules: Mapping[str, object]) -> int | None:
    """Size of the forward code closure when it is offline-constructible, else None."""
    limit = int(rules["max_closure"])  # type: ignore[call-overload]
    library = rules["library_sections"]  # type: ignore[index]
    seen = {va}
    queue = deque([va])
    while queue:
        node = queue.popleft()
        if facts.imports.get(node):
            return None
        for callee in facts.edges.get(node, ()):
            if callee in seen:
                continue
            if callee in facts.data_objects:
                if any(target in facts.functions for target in facts.edges.get(callee, ())):
                    return None  # a dispatch table or vtable: needs fabricated objects
                continue
            if in_sections(callee, library, facts.sections):  # type: ignore[arg-type]
                return None
            seen.add(callee)
            if len(seen) > limit:
                return None
            queue.append(callee)
    return len(seen)


def match_service(name: str, rules: list[Mapping[str, str]]) -> Mapping[str, str] | None:
    low = name.lower()
    for rule in rules:
        if re.search(rule["pattern"], low):
            return rule
    return None


def pick_session(
    name: str, subsystem: str, sessions: list[Mapping[str, object]], *, allow_subsystem: bool
) -> Mapping[str, object] | None:
    low = name.lower()
    for session in sessions:
        pattern = str(session.get("pattern") or "")
        if pattern and re.search(pattern, low):
            return session
    if not allow_subsystem:
        return None
    best: tuple[int, Mapping[str, object]] | None = None
    for session in sessions:
        for prefix in session.get("subsystems", ()):  # type: ignore[union-attr]
            prefix = str(prefix)
            if subsystem == prefix or subsystem.startswith(prefix + "/"):
                if best is None or len(prefix) > best[0]:
                    best = (len(prefix), session)
    return best[1] if best else None


def classify(
    va: int,
    subsystem: str,
    name: str,
    facts: Facts,
    rules: Mapping[str, object],
    names: Mapping[int, str],
    where: Mapping[int, str] | None = None,
) -> Row:
    """One never-executed port-class function. `where` maps executed VAs to recordings."""
    harness: Mapping[str, object] = rules["harness"]  # type: ignore[assignment]
    sessions: list[Mapping[str, object]] = rules["sessions"]  # type: ignore[assignment]
    where = where or {}

    def label(target: int) -> str:
        found = where.get(target)
        return f"{target:#x}" + (f" (ran in {found})" if found else "")

    if va not in facts.functions:
        return Row(va, subsystem, name, "e", "INFERRED", "not in the function table graph")
    if va not in facts.live:
        callers = sorted(facts.callers.get(va, ()))
        text = ", ".join(f"{c:#x}" for c in callers[:6]) or "none"
        return Row(
            va,
            subsystem,
            name,
            "a",
            "MEASURED",
            f"static fixpoint: not reachable from the XBE entry, TLS or any live table; "
            f"{len(callers)} static referrers, all dead: {text}",
        )
    online = offline_unused_service_calls(
        va,
        facts,
        harness["online_sections"],  # type: ignore[arg-type]
    )
    if online:
        targets = ", ".join(f"{node:#x}" for node in online[:4])
        return Row(
            va,
            subsystem,
            name,
            "b",
            "MEASURED",
            f"online: calls Xbox Live/XNet library code no executed function calls ({targets})",
        )
    service = match_service(name, rules["service"])  # type: ignore[arg-type]
    ancestor = executed_ancestor(va, facts, int(harness["ancestor_depth"]))  # type: ignore[call-overload]
    sibling = executed_sibling(va, facts)
    reached = ""
    if ancestor is not None:
        reached = f"caller {label(ancestor[0])} executed, {ancestor[1]} level(s) up"
    elif sibling is not None:
        reached = f"table {sibling[0]:#x} is live and its entry {label(sibling[1])} executed"
    if service is not None:
        extra = f"; {reached}" if reached else ""
        return Row(
            va,
            subsystem,
            name,
            "b",
            "INFERRED",
            f"{service['kind']}: name rule{extra}",
        )
    if reached:
        session = pick_session(name, subsystem, sessions, allow_subsystem=True)
        sid, title = _session(session)
        return Row(
            va,
            subsystem,
            name,
            "c",
            "MEASURED",
            f"live; {reached}",
            sid,
            title,
        )
    closure = self_contained(va, facts, harness)
    if closure is not None:
        return Row(
            va,
            subsystem,
            name,
            "d",
            "MEASURED",
            f"live; no executed caller within {harness['ancestor_depth']} levels; forward code "
            f"closure {closure} function(s), no XDK library, kernel import or dispatch table",
        )
    session = pick_session(name, subsystem, sessions, allow_subsystem=False)
    if session is not None:
        sid, title = _session(session)
        return Row(
            va,
            subsystem,
            name,
            "c",
            "INFERRED",
            "live; no executed caller nearby; session from the name rule",
            sid,
            title,
        )
    guess = pick_session(name, subsystem, sessions, allow_subsystem=True)
    hint = f"; probable session {guess['id']} by subsystem" if guess else ""
    return Row(
        va,
        subsystem,
        name,
        "e",
        "INFERRED",
        f"live; no executed caller nearby, closure not self-contained, no name rule{hint}",
    )


def _session(session: Mapping[str, object] | None) -> tuple[str, str]:
    if session is None:
        return "", "unmapped"
    return str(session["id"]), str(session["title"])
