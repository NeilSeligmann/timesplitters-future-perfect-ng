"""Resolve REA light-queue IDs against the canonical shard store and planner refs.

The old refill hook treated ``bad-ids.txt`` as a permanent tombstone list. This
module validates the current definition and status instead, and owns the shared
queue rewrite used by ``sup-refill.sh``.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from tools import datadir
from tools.fleet.queue import index_rows, unknown_name
from tools.fleet.tier2_prerequisites import Prerequisites
from tools.fleet.tier2_queue import ID_PATTERN as TIER2_ID
from tools.fleet.tier2_queue import load_spec as load_tier2_spec

INDEX = "docs/data/rea-continuous/index.json"
SHARDS = "docs/data/rea-continuous/unknowns"
DEFAULT_REPO = datadir.main_root()  # T3755: no host literal
DEFAULT_QUEUE = DEFAULT_REPO / "tmp" / "codexapi" / "p3queue.txt"
PLACEHOLDER_ANCHOR = re.compile(r"(?i)(?:near\s+)?0x(?:[0-9a-f]*x{2,}|[0-9a-f.]*\.{2,}[0-9a-fx]*)")
MAX_UNANSWERED_LAUNCHES = 2


def shard_path(unknown_id: str) -> str:
    bucket = hashlib.sha256(unknown_id.encode()).hexdigest()[:2]
    return f"{SHARDS}/{bucket}.json"


def _row_map(rows: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for row in rows:
        unknown_id = row.get("id")
        if not isinstance(unknown_id, str) or not unknown_id:
            continue
        old = result.get(unknown_id)
        if old is None or int(row.get("revision", 0) or 0) >= int(old.get("revision", 0) or 0):
            result[unknown_id] = row
    return result


@dataclass
class QueueSource:
    """One index/shard source. ``shards`` is injectable for fast, isolated tests."""

    name: str
    rows: dict[str, dict[str, Any]] = field(default_factory=dict)
    shards: dict[str, set[str]] = field(default_factory=dict)
    root: Path | None = None
    ref: str | None = None
    _shard_cache: dict[str, dict[str, str]] = field(default_factory=dict)
    placeholder_ids: set[str] = field(default_factory=set)

    @classmethod
    def from_rows(
        cls,
        name: str,
        rows: list[dict[str, Any]],
        shards: dict[str, set[str]] | None = None,
    ) -> QueueSource:
        mapped = _row_map(rows)
        return cls(
            name=name,
            rows=mapped,
            shards=shards or {},
            placeholder_ids={key for key, row in mapped.items() if _has_placeholder_anchor(row)},
        )

    def _read_shard(self, path: str) -> dict[str, str]:
        if path in self._shard_cache:
            return self._shard_cache[path]
        statuses = {item: "open" for item in self.shards.get(path, set())}
        if self.root is not None and self.ref is not None:
            git_path = path
            if path.startswith("unknowns/"):
                git_path = f"docs/data/rea-continuous/{path}"
            try:
                raw = subprocess.run(
                    ["git", "show", f"{self.ref}:{git_path}"],
                    cwd=self.root,
                    capture_output=True,
                    text=True,
                    timeout=30,
                    check=True,
                ).stdout
                parsed = json.loads(raw)
                statuses = {
                    row["id"]: str(row.get("status", "open"))
                    for row in parsed.get("unknowns", [])
                    if isinstance(row, dict) and isinstance(row.get("id"), str)
                }
                self.placeholder_ids.update(
                    row["id"]
                    for row in parsed.get("unknowns", [])
                    if isinstance(row, dict)
                    and isinstance(row.get("id"), str)
                    and _has_placeholder_anchor(row)
                )
            except (OSError, subprocess.SubprocessError, json.JSONDecodeError):
                statuses = {}
        self._shard_cache[path] = statuses
        return statuses

    def status(self, unknown_id: str) -> str:
        """Return open, a concrete non-open status, missing-definition, or missing."""
        row = self.rows.get(unknown_id)
        path = str(row.get("path") or shard_path(unknown_id)) if row else shard_path(unknown_id)
        status = self._read_shard(path).get(unknown_id)
        if status is not None:
            return str(row.get("status", status)) if row else status
        # Planner refs before the shard migration can carry full definition rows.
        if row and row.get("question"):
            return str(row.get("status", "open"))
        if row:
            return "missing-definition"
        return "missing"

    def candidate_ids(self) -> list[str]:
        return [unknown_id for unknown_id, row in self.rows.items() if row.get("status") == "open"]


@dataclass
class Tier2Source(QueueSource):
    """Tier 2 items (``t2_<subsystem>_<n>``, tools.fleet.tier2_queue): the spec is the definition.

    Never offers candidates itself: tier2_queue generate --append queues them, so the existing
    unknown top-up is unchanged.
    """

    specs: Path = field(default_factory=lambda: DEFAULT_QUEUE.parent / "t2items")
    prerequisites: Prerequisites | None = None
    route_prerequisites: bool = False

    def status(self, unknown_id: str) -> str:
        if not TIER2_ID.match(unknown_id):
            return "missing"
        spec = load_tier2_spec(self.specs, unknown_id)
        if spec is None or not spec.get("functions"):
            return "missing"
        state = str(spec.get("state", "open"))
        if state != "open":
            return state
        if self.prerequisites is None:
            self.prerequisites = Prerequisites(self.root or DEFAULT_REPO)
        failures = self.prerequisites.check(spec, route=self.route_prerequisites)
        return "prerequisite:" + ";".join(failures) if failures else "open"

    def candidate_ids(self) -> list[str]:
        return []


def _show(root: Path, ref: str, path: str) -> str | None:
    try:
        return subprocess.run(
            ["git", "show", f"{ref}:{path}"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=45,
            check=True,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return None


def source_from_ref(root: Path, ref: str) -> QueueSource:
    raw = _show(root, ref, INDEX)
    rows: list[dict[str, Any]] = []
    if raw:
        try:
            rows = index_rows(json.loads(raw))
        except (json.JSONDecodeError, KeyError, TypeError):
            rows = []
    return QueueSource(name=ref, rows=_row_map(rows), root=root, ref=ref)


def planner_refs(root: Path) -> list[str]:
    try:
        out = subprocess.run(
            [
                "git",
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname)",
                "refs/remotes/origin/task/cx-planner*-remote",
            ],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=20,
            check=True,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    return sorted(line for line in out.splitlines() if line)


def resolve_id(unknown_id: str, sources: list[QueueSource]) -> tuple[bool, str, str | None]:
    """Main status wins; a missing main definition may resolve on a planner ref."""
    if not unknown_id or any(ch.isspace() for ch in unknown_id):
        return False, "malformed-id", None
    seen: list[tuple[str, str]] = []
    for index, source in enumerate(sources):
        status = source.status(unknown_id)
        if status.startswith("prerequisite:"):
            return False, status, source.name
        if unknown_id in source.placeholder_ids or _has_placeholder_anchor(
            source.rows.get(unknown_id, {})
        ):
            return False, "placeholder-anchor", source.name
        if status == "missing":
            continue
        seen.append((source.name, status))
        if index == 0 and status not in {"open", "missing-definition"}:
            return False, f"main-status-{status}", source.name
        if status == "open":
            return True, "open", source.name
    if any(status.startswith("main-status-") for _, status in seen):
        return False, seen[0][1], seen[0][0]
    if any(status == "missing-definition" for _, status in seen):
        return False, "missing-definition", seen[0][0]
    if seen:
        return False, f"not-open-{seen[-1][1]}", seen[-1][0]
    return False, "definition-not-found", None


def _has_placeholder_anchor(value: Any) -> bool:
    if isinstance(value, str):
        return PLACEHOLDER_ANCHOR.search(value) is not None
    if isinstance(value, dict):
        return any(_has_placeholder_anchor(item) for item in value.values())
    if isinstance(value, (list, tuple)):
        return any(_has_placeholder_anchor(item) for item in value)
    return False


def validate_ids(ids: list[str], sources: list[QueueSource]) -> tuple[list[str], dict[str, str]]:
    """Return unique resolvable IDs and one rejection reason per distinct bad ID."""
    accepted: list[str] = []
    rejected: dict[str, str] = {}
    seen: set[str] = set()
    for unknown_id in ids:
        if unknown_id in seen:
            rejected.setdefault(unknown_id, "duplicate-queue-row")
            continue
        seen.add(unknown_id)
        ok, reason, _source = resolve_id(unknown_id, sources)
        if ok:
            accepted.append(unknown_id)
        else:
            rejected[unknown_id] = reason
    return accepted, rejected


def resolvable_work_rows(
    repo: Path, rows: list[dict[str, Any]], ref: str = "origin/main"
) -> tuple[list[dict[str, Any]], dict[str, str]]:
    """Keep work-eligible rows whose record exists in the referenced shards.

    Open and investigating rows remain available to the feed and its timed
    investigation retry path. Planner shards are consulted only for unresolved
    main definitions.
    """
    eligible = [
        row for row in rows if row.get("status") in {"open", "investigating"} and row.get("id")
    ]
    main = QueueSource(name=ref, rows=_row_map(rows), root=repo, ref=ref)
    valid: list[str] = []
    rejected: dict[str, str] = {}
    unresolved: list[str] = []
    for row in eligible:
        unknown_id = str(row["id"])
        status = main.status(unknown_id)
        if status in {"open", "investigating"}:
            valid.append(unknown_id)
        elif status in {"missing", "missing-definition"}:
            unresolved.append(unknown_id)
        else:
            rejected[unknown_id] = f"not-work-eligible-{status}"
    if unresolved:
        fallback = [
            QueueSource(name=branch, root=repo, ref=branch) for branch in planner_refs(repo)
        ]
        for unknown_id in unresolved:
            result = "missing"
            for source in fallback:
                result = source.status(unknown_id)
                if result in {"open", "investigating"}:
                    valid.append(unknown_id)
                    break
                if result not in {"missing", "missing-definition"}:
                    break
            else:
                result = "missing"
            if unknown_id not in valid:
                rejected[unknown_id] = f"planner-{result}"
    valid_set = set(valid)
    return [row for row in eligible if row.get("id") in valid_set], rejected


def resolvable_ids(
    repo: Path,
    ids: list[str],
    rows: list[dict[str, Any]],
    ref: str = "origin/main",
    tier2_specs: Path | None = None,
) -> tuple[list[str], dict[str, str]]:
    """Resolve selected queue IDs against main, Tier 2 specs and, when needed, planner shards."""
    main = QueueSource(name=ref, rows=_row_map(rows), root=repo, ref=ref)
    specs = tier2_specs or repo / "tmp" / "codexapi" / "t2items"
    valid, rejected = validate_ids(ids, [main, Tier2Source(name="tier2", specs=specs, root=repo)])
    unresolved = [
        item
        for item, reason in rejected.items()
        if reason in {"missing-definition", "definition-not-found"}
    ]
    if unresolved:
        fallback = [
            QueueSource(name=branch, root=repo, ref=branch) for branch in planner_refs(repo)
        ]
        rescued, still_rejected = validate_ids(unresolved, fallback)
        valid.extend(rescued)
        for item in rescued:
            rejected.pop(item, None)
        rejected.update(still_rejected)
    return valid, rejected


def write_validation_report(
    directory: Path,
    valid_count: int,
    rejected: dict[str, str],
    *,
    before_count: int = 0,
    added_count: int = 0,
    launch_count: int = 0,
) -> dict[str, Any]:
    """Persist the last feed/refill verdict for the HEALTH detector."""
    report = {
        "validated_at": time.time(),
        "before_count": before_count,
        "valid_count": valid_count,
        "added_count": added_count,
        "launch_count": launch_count,
        "rejected_count": len(rejected),
        "rejected": rejected,
        "queue_empty": valid_count == 0,
    }
    _atomic_text(directory / "queue-validation.json", json.dumps(report, sort_keys=True) + "\n")
    _atomic_text(
        directory / "queue-rejections.jsonl",
        "".join(
            json.dumps({"id": item, "reason": reason}, sort_keys=True) + "\n"
            for item, reason in sorted(rejected.items())
        ),
    )
    return report


def _atomic_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as out:
            out.write(text)
            out.flush()
            os.fsync(out.fileno())
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def _branch_names(root: Path) -> set[str]:
    try:
        out = subprocess.run(
            ["git", "for-each-ref", "--format=%(refname)", "refs/remotes/origin/task/cx-*"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=20,
            check=True,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return set()
    names = set()
    for ref in out.splitlines():
        name = ref.removeprefix("refs/remotes/origin/task/").removesuffix("-remote")
        if name:
            names.add(name)
    return names


def plan_refill(
    raw_queue: list[str],
    old_bad: list[str],
    sources: list[QueueSource],
    occupied: set[str],
    target: int = 60,
    launch_limit: int = 8,
    launch_counts: dict[str, int] | None = None,
) -> tuple[list[str], list[str], list[str], dict[str, str], list[str]]:
    """Pure queue transition: launch, remaining, permanent bad IDs, rejects, added IDs."""
    valid, rejected = validate_ids(raw_queue, sources)
    _, old_bad_rejected = validate_ids(old_bad, sources)
    launch_counts = launch_counts or {}
    permanent = {"definition-not-found", "missing-definition", "malformed-id", "placeholder-anchor"}
    bad = [
        item
        for item in dict.fromkeys(old_bad)
        if item in old_bad_rejected and old_bad_rejected[item] in permanent
    ]
    bad.extend(item for item, reason in rejected.items() if reason in permanent and item not in bad)
    limit = max(0, launch_limit)
    rejects = dict(rejected)
    launch: list[str] = []
    remaining: list[str] = []
    for item in valid:
        if launch_counts.get(item, 0) >= MAX_UNANSWERED_LAUNCHES:
            rejects[item] = "launch-limit-2"
        elif len(launch) < limit:
            launch.append(item)
        else:
            remaining.append(item)
    added: list[str] = []
    for source in sources:
        if len(remaining) >= max(0, target) and len(launch) >= limit:
            break
        for candidate in source.candidate_ids():
            if len(remaining) >= max(0, target) and len(launch) >= limit:
                break
            if candidate in remaining or candidate in launch:
                continue
            if unknown_name(candidate) in occupied:
                continue
            ok, reason, _ = resolve_id(candidate, sources)
            if ok:
                if launch_counts.get(candidate, 0) >= MAX_UNANSWERED_LAUNCHES:
                    rejects[candidate] = "launch-limit-2"
                    continue
                if len(launch) < limit:
                    launch.append(candidate)
                else:
                    remaining.append(candidate)
                added.append(candidate)
            else:
                rejects.setdefault(candidate, reason)
    return launch, remaining, list(dict.fromkeys(bad)), rejects, added


def queue_lock(queue_path: Path) -> Path:
    """The flock that serialises every read-modify-write of the shared queue."""
    if queue_path == DEFAULT_QUEUE:
        return Path("/tmp/p3queue.lock")
    return queue_path.with_suffix(".lock")


def requeue(queue_path: Path, ids: list[str], launched_path: Path | None = None) -> list[str]:
    """T3763: put ids whose lanes failed every step back at the head of the queue and forget one
    launch of each (a launch that never ran a step must not count toward launch-limit-2)."""
    if not ids:
        return []
    lock_path = queue_lock(queue_path)
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    launch_history = launched_path or queue_path.parent / "launched.txt"
    with lock_path.open("a") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        queued = queue_path.read_text().split() if queue_path.exists() else []
        added = [item for item in dict.fromkeys(ids) if item not in queued]
        _atomic_text(queue_path, "".join(f"{item}\n" for item in [*added, *queued]))
        if launch_history.exists():
            launched = launch_history.read_text().split()
            for item in added:
                if item in launched:
                    launched.remove(item)
            _atomic_text(launch_history, "".join(f"{item}\n" for item in launched))
    return added


def refill(
    repo: Path,
    queue_path: Path,
    bad_path: Path,
    target: int = 60,
    launch_limit: int = 8,
    launched_path: Path | None = None,
) -> tuple[list[str], dict[str, Any]]:
    """Validate, prune, top up the shared queue, and return IDs for this refill."""
    lock_path = queue_lock(queue_path)
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        raw_queue = queue_path.read_text().split() if queue_path.exists() else []
        deferred_path = queue_path.parent / "tier2-deferred.txt"
        deferred = deferred_path.read_text().split() if deferred_path.exists() else []
        raw_queue = list(dict.fromkeys([*raw_queue, *deferred]))
        old_bad = bad_path.read_text().split() if bad_path.exists() else []
        refs = ["origin/main", *planner_refs(repo)]
        sources = [source_from_ref(repo, ref) for ref in refs]
        sources.append(
            Tier2Source(
                name="tier2",
                specs=queue_path.parent / "t2items",
                root=repo,
                route_prerequisites=True,
            )
        )
        launch_history = launched_path or queue_path.parent / "launched.txt"
        launch_counts: dict[str, int] = {}
        if launch_history.exists():
            for item in launch_history.read_text().split():
                launch_counts[item] = launch_counts.get(item, 0) + 1
        launch, remaining, bad, rejects, added = plan_refill(
            raw_queue, old_bad, sources, _branch_names(repo), target, launch_limit, launch_counts
        )
        _atomic_text(
            deferred_path,
            "".join(
                f"{item}\n"
                for item, reason in rejects.items()
                if reason.startswith("prerequisite:")
            ),
        )
        _atomic_text(queue_path, "".join(f"{item}\n" for item in remaining))
        _atomic_text(bad_path, "".join(f"{item}\n" for item in dict.fromkeys(bad)))
        planner_fix = sorted(item for item, reason in rejects.items() if reason == "launch-limit-2")
        if planner_fix:
            fix_path = queue_path.parent / "planner-fix.txt"
            existing = fix_path.read_text().split() if fix_path.exists() else []
            _atomic_text(
                fix_path, "".join(f"{item}\n" for item in dict.fromkeys([*existing, *planner_fix]))
            )
        report = {
            "validated_at": time.time(),
            "before_count": len(raw_queue),
            "before_unique": len(set(raw_queue)),
            "valid_before_launch": len(remaining) + len(launch) - len(added),
            "launch_count": len(launch),
            "added_count": len(added),
            "valid_count": len(remaining),
            "rejected_count": len(rejects),
            "rejected": rejects,
            "queue_empty": not remaining,
            "sources": refs,
        }
        write_validation_report(
            queue_path.parent,
            len(remaining),
            rejects,
            before_count=len(raw_queue),
            added_count=len(added),
            launch_count=len(launch),
        )
        return launch, report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=DEFAULT_REPO)
    parser.add_argument("--queue", type=Path, default=DEFAULT_QUEUE)
    parser.add_argument("--bad-ids", type=Path, default=DEFAULT_QUEUE.with_name("bad-ids.txt"))
    parser.add_argument("--target", type=int, default=60)
    parser.add_argument("--limit", type=int, default=8, help="IDs to hand to this refill call")
    args = parser.parse_args()
    launch, report = refill(args.repo, args.queue, args.bad_ids, args.target, args.limit)
    print("\n".join(launch))
    print(
        f"queue-valid={report['valid_count']} launch={report['launch_count']} "
        f"added={report['added_count']} rejected={report['rejected_count']}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
