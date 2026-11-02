"""T3514: delete merged branches on origin, one-off and automatically ("always delete merged").

Why: origin held 5,827 branches (5,679 merged) and every fetch and push of about 45 workers paid
for that ref count (see docs/t3514-origin-branch-prune.md). One shared policy serves three callers:

* ``python3 -m tools.wt prune-remote-merged [--apply]``: the sweep (dry run by default), also run
  from the hourly hygiene loop.
* ``delete_landed_source``: merge_task and integrate_remote call it once a merge is verified on
  origin, to delete the merged source branch.

A branch is deleted only when ALL of these hold (owner 2026-10-21, claude-sup conditions):

1. the name is ``task/*``, ``codex/*`` or ``work/*`` (``keep/*`` only with ``include_keep`` and only
   when merged; ``main``, ``master``, ``release/*`` and every non-branch ref never);
2. the tip is an ancestor of the freshly fetched ``origin/main`` (``git merge-base --is-ancestor``);
3. the tip commit is older than ``min_age`` (default 2 h; active lanes may still push), except for
   the post-merge delete, where the branch was just merged by this very caller;
4. no git worktree (registered or under ``tmp/wt/*``) has the branch checked out;
5. no live coord claim exists for the ``T<n>`` in the branch name (post-merge: other owners only);
6. the deletion carries a lease on the verified tip (``--force-with-lease=ref:sha``), so a worker
   that pushed more commits in the meantime keeps its branch.

A ref the forge itself refuses to delete (``[remote rejected] (reference already exists)``,
MEASURED 2026-10-26 on two ``task/*-remote`` refs) is recorded in the state file as
server-refused with its tip and skipped by later sweeps while the tip is unchanged (retried after
``REFUSED_RETRY_SECONDS``). Only a transport failure (no porcelain lines, a timeout, a non-ref
error) fails the sweep (T3804).

The sweep only talks to a healthy forge: ``ls-remote origin main`` must answer in time, twice, and
the work is stopped after repeated failures. Nothing here ever runs ``git fetch origin`` in full.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from tools.agents import primary_fetch

PRUNABLE_PREFIXES = ("task/", "codex/", "work/")
KEEP_PREFIX = "keep/"
PROTECTED_NAMES = frozenset({"main", "master", "HEAD"})
PROTECTED_PREFIXES = ("release/",)
MIN_AGE_SECONDS = 2 * 3600
BATCH_SIZE = 25
BATCH_PAUSE_SECONDS = 30
BACKOFF_SECONDS = 300
MAX_FAILURES = 2
HEALTH_LIMIT_SECONDS = 30.0
HEALTH_GAP_SECONDS = 0
PUSH_TIMEOUT_SECONDS = 300
MAX_DELETE_PER_RUN = 500
STATE_NAME = "tsfp-origin-branches.json"
DRAIN_FLAGS = ("prune-remote.off",)
REFUSED_RETRY_SECONDS = 7 * 24 * 3600
#: Skip reasons for branches kept on purpose (archive/*, rescue/*, keep/*, main, release/*): they
#: never count against the origin branch limit (HEALTH origin-branches compares `sweepable`).
PRESERVED_REASONS = ("other-prefix", "keep", "protected")
TASK_IN_NAME = re.compile(r"(?<![0-9A-Za-z])[tT]([0-9]{2,5})(?![0-9])")
ZERO = "0" * 40


def state_path() -> Path:
    """Where the sweep leaves its last result for local detectors (health-check.sh reads it)."""
    return Path(os.environ.get("TSFP_PRUNE_STATE", Path(tempfile.gettempdir()) / STATE_NAME))


@dataclass
class Decision:
    name: str
    sha: str
    verdict: str  # "delete" or "skip:<reason>"

    @property
    def delete(self) -> bool:
        return self.verdict == "delete"


@dataclass
class Report:
    seen: int = 0
    candidates: int = 0
    deleted: int = 0
    deleted_names: set[str] = field(default_factory=set)
    moved: int = 0
    refused: dict[str, str] = field(default_factory=dict)  # server-refused name -> tip
    skipped: dict[str, int] = field(default_factory=dict)
    failures: int = 0
    stopped: str = ""
    ok: bool = True
    remaining: int | None = None


def git(
    root: Path, *args: str, timeout: int = 120, check: bool = False, **kw: Any
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        capture_output=True,
        text=True,
        timeout=timeout,
        check=check,
        **kw,
    )


def task_ids(name: str) -> list[str]:
    return [f"T{number}" for number in TASK_IN_NAME.findall(name)]


def prunable_name(name: str, *, include_keep: bool = False) -> str | None:
    """None when the branch name may be pruned, else the skip reason."""
    if name in PROTECTED_NAMES or name.startswith(PROTECTED_PREFIXES):
        return "protected"
    if name.startswith(KEEP_PREFIX):
        return None if include_keep else "keep"
    if name.startswith(PRUNABLE_PREFIXES):
        return None
    return "other-prefix"


def live_worktree_branches(root: Path) -> set[str]:
    """Branches checked out by any registered worktree or any tree under tmp/wt (Codex lanes)."""
    branches: set[str] = set()
    listed = git(root, "worktree", "list", "--porcelain")
    for line in listed.stdout.splitlines():
        if line.startswith("branch refs/heads/"):
            branches.add(line.removeprefix("branch refs/heads/"))
    lanes = root / "tmp" / "wt"
    if lanes.is_dir():
        for tree in lanes.iterdir():
            if not (tree / ".git").exists():
                continue
            head = git(tree, "symbolic-ref", "-q", "--short", "HEAD", timeout=15)
            if head.returncode == 0 and head.stdout.strip():
                branches.add(head.stdout.strip())
    return branches


def live_claims(root: Path, stale: int | None = None) -> dict[str, str]:
    """Task id -> owner of every live (not stale) coord claim."""
    from tools.agents import coord

    limit = coord.DEFAULT_STALE_SECONDS if stale is None else stale
    claims: dict[str, str] = {}
    old = Path.cwd()
    os.chdir(root)  # coord resolves its claim directory from the cwd
    try:
        for path in (coord.coord_dir() / "claims").glob("*.json"):
            if path.stem.startswith("NEXT-"):
                continue
            try:
                held = coord.read_claim(path)
            except (ValueError, OSError):
                continue
            if not coord.is_stale(held, limit):
                claims[str(held.get("task"))] = str(held.get("owner"))
    finally:
        os.chdir(old)
    return claims


def present_commits(root: Path, shas: list[str]) -> set[str]:
    """The given commits that exist in the local object store (one `cat-file --batch-check`)."""
    if not shas:
        return set()
    checked = subprocess.run(
        ["git", "-C", str(root), "cat-file", "--batch-check=%(objectname) %(objecttype)"],
        input="\n".join(shas) + "\n",
        capture_output=True,
        text=True,
        timeout=600,
    )
    return {line.split()[0] for line in checked.stdout.splitlines() if line.endswith(" commit")}


def ancestry_map(root: Path, shas: list[str], main_sha: str) -> dict[str, str]:
    """ "merged", "unmerged" or "unknown" (commit not in the local object store) per tip.

    One `rev-list <tips> ^main` walk: a tip is unmerged exactly when it is itself listed, i.e.
    it is reachable from the tips but not from main (the same answer as `merge-base
    --is-ancestor tip main`, without one process per branch)."""
    unique = sorted(set(shas))
    have = present_commits(root, unique)
    states = {sha: "unknown" for sha in unique}
    if have and present_commits(root, [main_sha]):
        walked = subprocess.run(
            ["git", "-C", str(root), "rev-list", "--stdin"],
            input="\n".join([*sorted(have), f"^{main_sha}"]) + "\n",
            capture_output=True,
            text=True,
            timeout=900,
        )
        if walked.returncode == 0:
            unmerged = set(walked.stdout.split())
            for sha in have:
                states[sha] = "unmerged" if sha in unmerged else "merged"
    return states


def ancestry(root: Path, sha: str, main_sha: str) -> str:
    return ancestry_map(root, [sha], main_sha)[sha]


def commit_times(root: Path, shas: list[str]) -> dict[str, int]:
    """Committer time per commit; commits missing locally are absent from the result."""
    times: dict[str, int] = {}
    for start in range(0, len(shas), 2000):
        have = sorted(present_commits(root, shas[start : start + 2000]))
        if not have:
            continue
        shown = subprocess.run(
            ["git", "-C", str(root), "log", "--no-walk=unsorted", "--stdin", "--format=%H %ct"],
            input="\n".join(have) + "\n",
            capture_output=True,
            text=True,
            timeout=300,
        )
        for line in shown.stdout.splitlines():
            sha, _, stamp = line.partition(" ")
            if stamp.isdigit():
                times[sha] = int(stamp)
    return times


def plan(
    root: Path,
    branches: dict[str, str],
    main_sha: str,
    *,
    now: float | None = None,
    min_age: float = MIN_AGE_SECONDS,
    include_keep: bool = False,
    worktree_branches: set[str] | None = None,
    claims: dict[str, str] | None = None,
    ignore_claim_owner: str | None = None,
) -> list[Decision]:
    """Decide for each remote branch (name -> tip) whether it may be deleted. Pure policy."""
    now = time.time() if now is None else now
    live_trees = live_worktree_branches(root) if worktree_branches is None else worktree_branches
    held = live_claims(root) if claims is None else claims
    pending = {
        name: sha
        for name, sha in branches.items()
        if prunable_name(name, include_keep=include_keep) is None
    }
    times = commit_times(root, sorted(set(pending.values()))) if min_age > 0 else {}
    states = ancestry_map(root, sorted(set(pending.values())), main_sha)
    decisions: list[Decision] = []
    for name, sha in sorted(branches.items()):
        reason = prunable_name(name, include_keep=include_keep)
        if reason is None:
            state = states[sha]
            if state != "merged":
                reason = state
            elif name in live_trees:
                reason = "live-worktree"
            elif any(held.get(task) not in (None, ignore_claim_owner) for task in task_ids(name)):
                reason = "live-claim"
            elif min_age > 0 and (sha not in times or now - times[sha] < min_age):
                reason = "young" if sha in times else "unknown-age"
        decisions.append(Decision(name, sha, "delete" if reason is None else f"skip:{reason}"))
    return decisions


def refresh_main(root: Path, remote: str = "origin", target: str = "main") -> str:
    """Fetch the single ref (never a full fetch) and return its sha."""
    fetched = git(
        root,
        "fetch",
        "-q",
        remote,
        f"+refs/heads/{target}:refs/remotes/{remote}/{target}",
        timeout=300,
    )
    if fetched.returncode:
        raise RuntimeError(f"fetch {remote} {target} failed: {fetched.stderr.strip()[:200]}")
    return git(root, "rev-parse", f"refs/remotes/{remote}/{target}", check=True).stdout.strip()


def ls_remote_heads(root: Path, remote: str = "origin", timeout: int = 300) -> dict[str, str]:
    listed = git(root, "ls-remote", "--heads", remote, timeout=timeout)
    if listed.returncode:
        raise RuntimeError(f"ls-remote {remote} failed: {listed.stderr.strip()[:200]}")
    heads: dict[str, str] = {}
    for line in listed.stdout.splitlines():
        sha, _, ref = line.partition("\t")
        if ref.startswith("refs/heads/"):
            heads[ref.removeprefix("refs/heads/")] = sha
    return heads


def fetch_missing(root: Path, remote: str, heads: dict[str, str], names: list[str]) -> None:
    """Fetch only branches whose tip commit is not in the local object store (small chunks)."""
    present = present_commits(root, sorted({heads[name] for name in names}))
    absent = [name for name in names if heads[name] not in present]
    for start in range(0, len(absent), 50):
        chunk = absent[start : start + 50]
        specs = [f"+refs/heads/{n}:refs/remotes/{remote}/{n}" for n in chunk]
        result = primary_fetch.fetch("fetch", "-q", remote, *specs, cwd=root, timeout=600)
        if result.returncode:
            raise RuntimeError(f"fetch {remote} failed: {(result.stderr or '').strip()[-300:]}")


def drop_stale_tracking_refs(root: Path, remote: str, heads: dict[str, str]) -> int:
    """Delete refs/remotes/<remote>/* that origin no longer has (local cache only, one batch)."""
    prefix = f"refs/remotes/{remote}/"
    listed = git(root, "for-each-ref", "--format=%(refname)", prefix, timeout=300)
    stale = [
        ref
        for ref in listed.stdout.split()
        if ref.removeprefix(prefix) not in heads and ref != f"{prefix}HEAD"
    ]
    if stale:
        subprocess.run(
            ["git", "-C", str(root), "update-ref", "--stdin"],
            input="".join(f"delete {ref}\n" for ref in stale),
            capture_output=True,
            text=True,
            timeout=600,
        )
    return len(stale)


def healthy(
    root: Path,
    remote: str = "origin",
    *,
    limit: float = HEALTH_LIMIT_SECONDS,
    checks: int = 2,
    gap: float = HEALTH_GAP_SECONDS,
    sleep: Callable[[float], None] = time.sleep,
    clock: Callable[[], float] = time.monotonic,
) -> tuple[bool, str]:
    """`ls-remote origin main` must answer within `limit` seconds, `checks` times, `gap` apart."""
    for number in range(checks):
        if number:
            sleep(gap)
        started = clock()
        try:
            probe = git(root, "ls-remote", remote, "refs/heads/main", timeout=30)
        except subprocess.TimeoutExpired:
            return False, "ls-remote timed out after 30 s"
        spent = clock() - started
        if probe.returncode or not probe.stdout.strip():
            return False, f"ls-remote failed: {probe.stderr.strip()[:160]}"
        if spent > limit:
            return False, f"ls-remote took {spent:.1f} s (limit {limit} s)"
    return True, "ok"


def parse_push(
    pushed: subprocess.CompletedProcess,
) -> tuple[set[str], set[str], set[str], bool]:
    """(deleted, moved, refused, failure) from `push --porcelain`.

    moved = the lease kept a branch that moved; refused = the forge rejected that one ref
    (`[remote rejected] (<reason>)` names the ref, so it is a per-ref refusal even when every ref
    of the batch gets one); failure = a transport or non-ref error only."""
    deleted: set[str] = set()
    moved: set[str] = set()
    rejected: set[str] = set()
    other = False
    seen = False
    for line in pushed.stdout.splitlines():
        flag, _, rest = line.partition("\t")
        if flag not in ("-", "!"):
            continue
        seen = True
        refspec = rest.split("\t", 1)[0]  # "<from>:<to>", from is empty for a delete
        name = refspec.partition(":")[2].removeprefix("refs/heads/")
        if flag == "-":
            deleted.add(name)
        elif "stale info" in rest or "fetch first" in rest:
            moved.add(name)
        elif "[remote rejected]" in rest:
            rejected.add(name)
        else:
            other = True
    if pushed.returncode and not seen:
        other = True
    # MEASURED 2026-10-21 and 2026-10-26: Forgejo refuses single refs ("reference already
    # exists"), also when they are the whole batch. Retrying can never succeed, so a refusal is a
    # kept, recorded branch, never a failed batch (T3804; it failed every sweep before).
    return deleted, moved, rejected, other


def push_delete(
    root: Path, remote: str, items: list[tuple[str, str]], timeout: int = PUSH_TIMEOUT_SECONDS
) -> tuple[set[str], set[str], set[str], bool, str]:
    """One leased delete push. The lease is the verified tip: a moved branch is kept."""
    args = ["push", "--porcelain", "--no-verify"]
    args += [f"--force-with-lease=refs/heads/{name}:{sha}" for name, sha in items]
    args += [remote, *[f":refs/heads/{name}" for name, _ in items]]
    try:
        pushed = git(root, *args, timeout=timeout)
    except subprocess.TimeoutExpired:
        return set(), set(), set(), True, f"push timed out after {timeout} s"
    deleted, moved, refused, other = parse_push(pushed)
    return deleted, moved, refused, other, pushed.stderr.strip()[-300:]


def delete_batches(
    root: Path,
    remote: str,
    items: list[tuple[str, str]],
    *,
    batch: int = BATCH_SIZE,
    pause: float = BATCH_PAUSE_SECONDS,
    backoff: float = BACKOFF_SECONDS,
    max_failures: int = MAX_FAILURES,
    health: Callable[[], tuple[bool, str]] | None = None,
    push: Callable[..., tuple[set[str], set[str], set[str], bool, str]] | None = None,
    sleep: Callable[[float], None] = time.sleep,
    log: Callable[[str], None] = print,
) -> Report:
    push = push or push_delete  # resolved per call, so a patched push_delete is honoured
    report = Report(candidates=len(items))
    consecutive = 0
    position = 0
    while position < len(items):
        if health is not None:
            good, why = health()
            if not good:
                consecutive += 1
                report.failures += 1
                log(f"forge unhealthy before batch at {position}: {why}")
                if consecutive >= max_failures:
                    report.stopped, report.ok = f"unhealthy: {why}", False
                    break
                sleep(backoff)
                continue
        chunk = items[position : position + batch]
        deleted, moved, refused, other, tail = push(root, remote, chunk)
        report.deleted += len(deleted)
        report.deleted_names |= deleted
        report.moved += len(moved)
        report.refused.update((name, sha) for name, sha in chunk if name in refused)
        if other:
            consecutive += 1
            report.failures += 1
            log(f"batch at {position} failed ({consecutive}/{max_failures}): {tail}")
            if consecutive >= max_failures:
                report.stopped, report.ok = f"push failures: {tail}", False
                break
            sleep(backoff)
            continue  # retry the same batch after backing off
        consecutive = 0
        position += len(chunk)
        log(
            f"batch ok: deleted {len(deleted)}, moved {len(moved)}, server-refused "
            f"{len(refused)}, done {position}/{len(items)}"
        )
        if position < len(items):
            sleep(pause)
    return report


def drained(root: Path) -> str:
    """Local pause switch: tmp/prune-remote.off (or under tmp/codexapi) stops the sweep."""
    for name in DRAIN_FLAGS:
        for base in (root / "tmp" / "codexapi", root / "tmp"):
            if (base / name).exists():
                return str(base / name)
    return ""


def read_refused(now: float | None = None) -> dict[str, dict[str, Any]]:
    """Server-refused refs remembered by earlier sweeps (name -> {sha, ts}), minus expired ones."""
    now = time.time() if now is None else now
    try:
        record = json.loads(state_path().read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    refused = record.get("refused") if isinstance(record, dict) else None
    if not isinstance(refused, dict):
        return {}
    return {
        name: entry
        for name, entry in refused.items()
        if isinstance(entry, dict)
        and isinstance(entry.get("sha"), str)
        and now - float(entry.get("ts", 0)) < REFUSED_RETRY_SECONDS
    }


def still_refused(
    known: dict[str, dict[str, Any]], heads: dict[str, str]
) -> dict[str, dict[str, Any]]:
    """Remembered refusals still on origin at the refused tip (a new tip is retried)."""
    return {name: entry for name, entry in known.items() if heads.get(name) == entry["sha"]}


def write_state(
    report: Report,
    count: int | None,
    error: str = "",
    refused: dict[str, dict[str, Any]] | None = None,
) -> None:
    preserved = sum(report.skipped.get(reason, 0) for reason in PRESERVED_REASONS)
    refused = dict(refused or {})
    record = {
        "ts": int(time.time()),
        "ok": report.ok and not error,
        "error": error or report.stopped,
        "branches": count,
        # T3804: what HEALTH origin-branches compares with its limit (archive/*, rescue/*,
        # keep/* and protected names are kept deliberately and reported as `preserved`).
        "sweepable": None if count is None else max(count - preserved, 0),
        "preserved": preserved,
        "deleted": report.deleted,
        "candidates": report.candidates,
        "skipped": report.skipped,
        "remaining_merged": report.remaining,
        "refused_count": len(refused),
        "refused": refused,
    }
    try:
        state_path().write_text(json.dumps(record), encoding="utf-8")
    except OSError:
        pass


def _sweep(
    root: Path,
    *,
    apply: bool = False,
    remote: str = "origin",
    min_age: float = MIN_AGE_SECONDS,
    include_keep: bool = False,
    batch: int = BATCH_SIZE,
    pause: float = BATCH_PAUSE_SECONDS,
    backoff: float = BACKOFF_SECONDS,
    max_failures: int = MAX_FAILURES,
    max_delete: int = MAX_DELETE_PER_RUN,
    backup_file: Path | None = None,
    health_checks: int = 1,
    health_gap: float = HEALTH_GAP_SECONDS,
    sleep: Callable[[float], None] = time.sleep,
    log: Callable[[str], None] = print,
) -> Report:
    report = Report()
    known_refused = read_refused()
    if git(root, "remote", "get-url", remote).returncode:
        # A clone problem, not a forge outcome: say so, and leave the detector state alone.
        report.ok, report.stopped = False, f"remote {remote} is not configured in {root}"
        log(f"prune-remote-merged: {report.stopped}")
        return report
    flag = drained(root)
    if flag:
        report.ok, report.stopped = True, f"drained ({flag})"
        log(f"prune-remote-merged: skipped, {flag} exists")
        return report
    try:
        if apply:
            good, why = healthy(root, remote, checks=health_checks, gap=health_gap, sleep=sleep)
            if not good:
                raise RuntimeError(f"forge unhealthy: {why}")
        main_sha = refresh_main(root, remote)
        heads = ls_remote_heads(root, remote)
        report.seen = len(heads)
        names = [n for n in heads if prunable_name(n, include_keep=include_keep) is None]
        fetch_missing(root, remote, heads, names)
        decisions = plan(
            root,
            heads,
            main_sha,
            min_age=min_age,
            include_keep=include_keep,
        )
    except (RuntimeError, subprocess.SubprocessError, OSError) as error:
        report.ok, report.stopped = False, str(error)
        write_state(report, None, str(error), known_refused)
        log(f"prune-remote-merged FAILED: {error}")
        return report
    refused = still_refused(known_refused, heads)
    for decision in decisions:
        if decision.delete and decision.name in refused:
            decision.verdict = "skip:server-refused"
    items = [(d.name, d.sha) for d in decisions if d.delete]
    for decision in decisions:
        if not decision.delete:
            reason = decision.verdict.removeprefix("skip:")
            report.skipped[reason] = report.skipped.get(reason, 0) + 1
    report.candidates = len(items)
    log(
        f"origin has {len(heads)} branches, {len(items)} deletable now; "
        f"skipped {dict(sorted(report.skipped.items()))}"
    )
    if not apply:
        log("dry run: nothing deleted (use --apply)")
        report.remaining = len(items)
        write_state(report, len(heads), refused=refused)
        return report
    items = items[:max_delete]
    if backup_file is not None and items:
        # Name and tip of everything about to go, written before the first push.
        backup_file.parent.mkdir(parents=True, exist_ok=True)
        with backup_file.open("a", encoding="utf-8") as sink:
            sink.write("".join(f"{name} {sha}\n" for name, sha in items))
    run = delete_batches(
        root,
        remote,
        items,
        batch=batch,
        pause=pause,
        backoff=backoff,
        max_failures=max_failures,
        health=lambda: healthy(root, remote, checks=1, gap=0),
        sleep=sleep,
        log=log,
    )
    run.seen, run.skipped = report.seen, report.skipped
    for name in run.deleted_names:
        heads.pop(name, None)
    gone = drop_stale_tracking_refs(root, remote, heads)
    log(f"dropped {gone} stale local {remote}/* tracking refs")
    run.remaining = len(items) - run.deleted - len(run.refused)
    stamp = int(time.time())
    refused.update({name: {"sha": sha, "ts": stamp} for name, sha in run.refused.items()})
    write_state(run, len(heads), refused=refused)
    log(
        f"deleted {run.deleted}, lease-kept {run.moved}, server-refused {len(run.refused)} "
        f"(remembered {len(refused)}), stopped: {run.stopped or 'no'}"
    )
    return run


def sweep(root: Path, *, apply: bool = False, **options: Any) -> Report:
    """Run one sweep; a deleting sweep is exclusive per clone (flock), a second one backs off."""
    if not apply:
        return _sweep(root, apply=False, **options)
    lock_dir = root / "tmp" / "wt-cache"
    lock_dir.mkdir(parents=True, exist_ok=True)
    with (lock_dir / "prune-remote.lock").open("a+") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            log = options.get("log", print)
            log("prune-remote-merged: another sweep holds prune-remote.lock; nothing to do")
            return Report(stopped="another sweep running")
        return _sweep(root, apply=True, **options)


def delete_landed_source(
    root: Path,
    branch: str,
    landed_sha: str,
    *,
    target: str = "main",
    owner: str | None = None,
    remote: str = "origin",
) -> str:
    """After a merge is verified on origin: delete the merged source branch on origin.

    Never raises and never fails the merge. `landed_sha` must be on the freshly fetched
    origin/<target> (checked here, one single-ref fetch). A branch that kept moving, or that a
    worktree or another owner's live claim still uses, is left for the sweep.
    """
    try:
        reason = prunable_name(branch)
        if reason is not None:
            return f"kept {branch}: {reason}"
        fresh = refresh_main(root, remote, target)
        if ancestry(root, landed_sha, fresh) != "merged":
            return f"kept {branch}: {landed_sha[:12]} is not on {remote}/{target}"
        listed = git(root, "ls-remote", "--heads", remote, f"refs/heads/{branch}", timeout=60)
        if listed.returncode:
            return f"kept {branch}: ls-remote failed"
        if not listed.stdout.strip():
            return f"{branch} is not on {remote}"
        tip = listed.stdout.split()[0]
        fetch_missing(root, remote, {branch: tip}, [branch])
        decision = plan(root, {branch: tip}, fresh, min_age=0, ignore_claim_owner=owner)[0]
        if not decision.delete:
            return f"kept {branch}: {decision.verdict.removeprefix('skip:')}"
        deleted, moved, refused, _other, tail = push_delete(
            root, remote, [(branch, tip)], timeout=120
        )
        if branch in deleted:
            return f"deleted merged {branch} on {remote}"
        if branch in refused:
            return f"kept {branch}: {remote} refused the delete {tail[-120:]}"
        return f"kept {branch}: {'moved on origin' if moved else 'push failed ' + tail[-120:]}"
    except (RuntimeError, OSError, subprocess.SubprocessError, ValueError) as error:
        return f"kept {branch}: {error}"


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--apply", action="store_true", help="delete (default: dry run)")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--min-age-h", type=float, default=MIN_AGE_SECONDS / 3600)
    parser.add_argument(
        "--include-keep", action="store_true", help="also prune merged keep/* refs (claude-sup)"
    )
    parser.add_argument("--batch", type=int, default=BATCH_SIZE)
    parser.add_argument("--pause", type=float, default=BATCH_PAUSE_SECONDS)
    parser.add_argument("--backoff", type=float, default=BACKOFF_SECONDS)
    parser.add_argument("--max-failures", type=int, default=MAX_FAILURES)
    parser.add_argument("--max-delete", type=int, default=MAX_DELETE_PER_RUN)
    parser.add_argument("--health-gap", type=float, default=HEALTH_GAP_SECONDS)
    parser.add_argument(
        "--backup-file",
        type=Path,
        default=None,
        help="append 'name sha' of every deleted branch here (default tmp/wt-cache/...)",
    )


def run_cli(args: argparse.Namespace, root: Path) -> int:
    report = sweep(
        root,
        apply=args.apply,
        remote=args.remote,
        min_age=args.min_age_h * 3600,
        include_keep=args.include_keep,
        batch=args.batch,
        pause=args.pause,
        backoff=args.backoff,
        max_failures=args.max_failures,
        max_delete=args.max_delete,
        backup_file=args.backup_file or root / "tmp" / "wt-cache" / "origin-prune-backup.log",
        health_gap=args.health_gap,
    )
    return 0 if report.ok else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    add_arguments(parser)
    return run_cli(parser.parse_args(argv), Path.cwd())


if __name__ == "__main__":
    sys.exit(main())
