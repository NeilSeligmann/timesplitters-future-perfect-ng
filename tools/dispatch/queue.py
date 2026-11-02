"""The shared job queue and placement leases of the dispatcher (T3885).

Layout under the shared data dir (`tools.datadir.data_dir("dispatch")`, override
`TSFP_DISPATCH_DIR` for tests):

    .mutex                       flock taken while a plan is computed and a lease taken
    queue/<ns>-<id>.wait         a waiting job (JSON), flocked by its submitter while it waits
    active/<id>.lease            a placed job (JSON: host, cores, mem, pid), flocked by its holder
    events.jsonl                 every decision: queued, placed, requeued, host_failed, done

A dead holder's flock is released by the kernel, so its entry is dropped at the next scan (no
stale accounting). Every waiter computes the same plan (placement.plan over all live waiters
in priority, then arrival order) and takes its own placement only; a job without a host keeps
waiting. There is no local fallback: the owner PC is a host of the plan like any other.
"""

from __future__ import annotations

import contextlib
import errno
import fcntl
import json
import os
import secrets
import time
from collections.abc import Callable, Collection, Iterator, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import IO, Any

from tools.dispatch import placement
from tools.dispatch.model import OWNER_PC, Decision, HostState, JobSpec

DIR_ENV = "TSFP_DISPATCH_DIR"
TOKEN_ENV = "TSFP_DISPATCH_TOKEN"
"""The placement token a heavy command on the owner PC must carry (enforce.py)."""
WAIT = ".wait"
LEASE = ".lease"


class DispatchError(RuntimeError):
    """No eligible execution route or the bounded host-attempt budget was exhausted."""


ViewFn = Callable[[dict[str, tuple[float, int, int]], set[int]], list[HostState]]
ReserveFn = Callable[[JobSpec, Decision], Any]
"""(dispatcher commitments per host, dispatcher pids) -> the current host view."""


def state_dir() -> Path:
    override = os.environ.get(DIR_ENV)
    if override:
        return Path(override)
    from tools.datadir import data_dir

    return data_dir("dispatch")


def _held(handle: IO[str]) -> bool:
    """True when another open file description holds the flock (its owner is alive)."""
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        if error.errno not in (errno.EAGAIN, errno.EACCES, errno.EWOULDBLOCK):
            raise
        return True
    fcntl.flock(handle, fcntl.LOCK_UN)
    return False


def active_exclusions(row: Mapping[str, Any], now: float) -> tuple[str, ...]:
    """Hosts a waiting job must not use now: `exclude` maps host -> until (0 = for good); a
    list (older waiters) excludes for good. Expired exclusions let the host be probed again."""
    raw = row.get("exclude", ())
    if isinstance(raw, Mapping):
        return tuple(sorted(h for h, until in raw.items() if not until or float(until) > now))
    return tuple(raw)


def _locked_new(path: Path) -> IO[str]:
    """A flocked temp file beside `path` (renamed into place by the caller once written)."""
    handle = path.with_name(f".{path.name}.{os.getpid()}.tmp").open("w+")
    fcntl.flock(handle, fcntl.LOCK_EX)
    return handle


def live_entries(directory: Path, suffix: str) -> list[dict[str, Any]]:
    """Entries whose holder still flocks them; dead ones are removed."""
    found: list[dict[str, Any]] = []
    if not directory.is_dir():
        return found
    for path in sorted(directory.glob(f"*{suffix}")):
        try:
            handle = path.open("r")
        except FileNotFoundError:
            continue
        with handle:
            if not _held(handle):
                path.unlink(missing_ok=True)
                continue
            try:
                data = json.loads(path.read_text(encoding="utf-8") or "{}")
            except (OSError, ValueError):
                continue
            data["_path"] = str(path)
            found.append(data)
    return found


@dataclass
class Lease:
    """A placement: the host a job runs on and the token proving it (`TSFP_DISPATCH_TOKEN`)."""

    id: str
    host: str
    job: JobSpec
    score: float | None
    waited_s: float
    path: Path
    handle: IO[str] | None = None
    cores: float = 0.0
    """Cores granted (an elastic job may get fewer than `job.cores`)."""
    reservation: Any = None

    @property
    def local(self) -> bool:
        return self.host == OWNER_PC

    def env(self) -> dict[str, str]:
        return {TOKEN_ENV: self.id}


@dataclass
class Dispatcher:
    """Queue, plan and lease against one view. Clock and sleep are injectable (rule 39)."""

    view: ViewFn
    base: Path = field(default_factory=state_dir)
    clock: Callable[[], float] = time.time
    sleep: Callable[[float], None] = time.sleep
    poll_s: float = 5.0
    remote_down_after_s: float = 600.0
    remote_busy_after_s: float = 120.0
    reserve: ReserveFn | None = None
    local_first_kinds: frozenset[str] = frozenset()
    log: Callable[[str], object] = lambda _text: None
    pid: int = field(default_factory=os.getpid)

    def __post_init__(self) -> None:
        for sub in ("queue", "active"):
            (self.base / sub).mkdir(parents=True, exist_ok=True)

    # -- ledger -------------------------------------------------------------------------
    @contextlib.contextmanager
    def _mutex(self) -> Iterator[None]:
        with (self.base / ".mutex").open("a") as handle:
            fcntl.flock(handle, fcntl.LOCK_EX)
            try:
                yield
            finally:
                fcntl.flock(handle, fcntl.LOCK_UN)

    def event(self, name: str, **fields: object) -> None:
        row = {"at": round(self.clock(), 3), "event": name, **fields}
        with contextlib.suppress(OSError), (self.base / "events.jsonl").open("a") as handle:
            handle.write(json.dumps(row, sort_keys=True) + "\n")

    def waiters(self) -> list[dict[str, Any]]:
        rows = live_entries(self.base / "queue", WAIT)
        return sorted(rows, key=lambda row: (-int(row.get("priority", 0)), row["_path"]))

    def leases(self) -> list[dict[str, Any]]:
        return live_entries(self.base / "active", LEASE)

    def commitments(self) -> tuple[dict[str, tuple[float, int, int]], set[int]]:
        totals: dict[str, tuple[float, int, int]] = {}
        pids: set[int] = set()
        for row in self.leases():
            cores, mem, jobs = totals.get(row["host"], (0.0, 0, 0))
            totals[row["host"]] = (
                round(cores + float(row["cores"]), 3),
                mem + int(row["mem_mb"]),
                jobs + 1,
            )
            pids.add(int(row.get("pid", 0)))
        return totals, pids

    # -- queue --------------------------------------------------------------------------
    def _enqueue(
        self, job: JobSpec, job_id: str, since: float, exclude: Mapping[str, float]
    ) -> tuple[Path, IO[str]]:
        stamp = f"{time.time_ns():020d}"
        path = self.base / "queue" / f"{stamp}-{job_id}{WAIT}"
        handle = _locked_new(path)
        handle.write(
            json.dumps(
                {
                    "id": job_id,
                    "job": job.to_json(),
                    "since": since,
                    "pid": self.pid,
                    "priority": job.priority,
                    "exclude": dict(sorted(exclude.items())),
                }
            )
        )
        handle.flush()
        os.replace(handle.name, path)  # visible only once flocked and complete
        return path, handle

    def try_place(
        self, job_id: str, exclude: Collection[str] = ()
    ) -> tuple[Decision, list[HostState]]:
        """Plan every live waiter against the current view (call under the mutex)."""
        totals, pids = self.commitments()
        hosts = self.view(totals, pids)
        now = self.clock()
        jobs = [
            (
                str(row["id"]),
                JobSpec.from_json(row["job"]),
                max(0.0, now - float(row.get("since", now))),
                tuple({*active_exclusions(row, now), *(exclude if row["id"] == job_id else ())}),
            )
            for row in self.waiters()
        ]
        decisions = placement.plan(
            jobs,
            hosts,
            remote_down_after_s=self.remote_down_after_s,
            remote_busy_after_s=self.remote_busy_after_s,
            local_first_kinds=self.local_first_kinds,
        )
        return decisions.get(job_id, Decision(None, None, ("not_queued",))), hosts

    def _take(self, job_id: str, job: JobSpec, decision: Decision, waited: float) -> Lease:
        assert decision.host is not None
        path = self.base / "active" / f"{job_id}{LEASE}"
        handle = _locked_new(path)
        handle.write(
            json.dumps(
                {
                    "id": job_id,
                    "host": decision.host,
                    "cores": decision.cores or job.cores,
                    "mem_mb": decision.mem_mb if decision.mem_mb is not None else job.mem_mb,
                    "kind": job.kind,
                    "label": job.label,
                    "pid": self.pid,
                    "since": self.clock(),
                }
            )
        )
        handle.flush()
        os.replace(handle.name, path)
        cores = decision.cores or job.cores
        return Lease(job_id, decision.host, job, decision.score, waited, path, handle, cores)

    def acquire(
        self,
        job: JobSpec,
        *,
        exclude: Collection[str] | Mapping[str, float] = (),
        since: float | None = None,
        timeout_s: float | None = None,
        job_id: str | None = None,
        logical_id: str | None = None,
    ) -> Lease:
        """Queue `job` and block until the plan places it; never a silent local run.

        `since` keeps the original arrival time across requeues (waited time counts toward the
        owner-PC rule). TimeoutError after `timeout_s` without a placement."""
        job_id = job_id or secrets.token_hex(6)
        logical_id = logical_id or job_id
        start = self.clock() if since is None else since
        until = dict(exclude) if isinstance(exclude, Mapping) else dict.fromkeys(exclude, 0.0)
        path, handle = self._enqueue(job, job_id, start, until)
        self.event(
            "queued",
            id=job_id,
            logical_id=logical_id,
            kind=job.kind,
            label=job.label,
            exclude=sorted(until),
        )
        last_reason = ""
        admission_until: dict[str, float] = {}
        try:
            while True:
                with self._mutex():
                    excluded = [
                        host for host, until in admission_until.items() if until > self.clock()
                    ]
                    decision, _hosts = self.try_place(job_id, excluded)
                    if job.owner_pc_only and not any(
                        host.local
                        and placement.can_ever_run(job, host)
                        and (job.min_cores or job.cores) <= host.capacity
                        for host in _hosts
                    ):
                        raise DispatchError(
                            "dispatch: owner-only request exceeds local capabilities/core cap"
                        )
                    if job.remote_only and not any(
                        not host.local
                        and placement.can_ever_run(job, host)
                        and not placement.automatic_refusal(job, host)
                        for host in _hosts
                    ):
                        raise DispatchError(
                            "dispatch: remote-only work has no validated eligible host; "
                            "use remote_exec verify --host HOST for an explicit budgeted smoke "
                            "check, or run locally. " + "; ".join(decision.refused)
                        )
                    if decision.host is not None:
                        reservation = None
                        if self.reserve is not None:
                            from tools.wt.host_budget import BudgetFull

                            try:
                                reservation = self.reserve(job, decision)
                            except BudgetFull as error:
                                admission_until[decision.host] = self.clock() + max(30, self.poll_s)
                                decision = Decision(None, refused=(f"admission:{error}",))
                        if decision.host is None:
                            # Another admission path won the budget race. Stay queued: no
                            # false placement event and no staging of an unreserved job.
                            pass
                        else:
                            try:
                                lease = self._take(
                                    job_id, job, decision, round(self.clock() - start, 1)
                                )
                            except BaseException:
                                if reservation is not None:
                                    reservation.close()
                                raise
                            lease.reservation = reservation
                    if decision.host is not None:
                        waited = round(self.clock() - start, 1)
                        path.unlink(missing_ok=True)
                        self.event(
                            "placed",
                            id=job_id,
                            logical_id=logical_id,
                            host=decision.host,
                            kind=job.kind,
                            label=job.label,
                            cores=decision.cores or job.cores,
                            waited_s=waited,
                            score=decision.score,
                            local_reason=job.local_reason,
                            display=job.display,
                            local_only=job.local_only,
                        )
                        self.log(
                            f"dispatch: {job.label or job.kind} -> {decision.host} "
                            f"(score {decision.score}, waited {waited}s)"
                        )
                        return lease
                reason = "; ".join(decision.refused) or "no host"
                if reason != last_reason:
                    self.log(f"dispatch: {job.label or job.kind} queued ({reason})"[:400])
                    last_reason = reason
                if timeout_s is not None and self.clock() - start >= timeout_s:
                    self.event("timeout", id=job_id, kind=job.kind, refused=list(decision.refused))
                    raise TimeoutError(
                        f"dispatch: no host for {job.kind} in {timeout_s:g}s ({reason})"
                    )
                self.sleep(self.poll_s)
        finally:
            path.unlink(missing_ok=True)
            handle.close()

    def release(self, lease: Lease, *, outcome: str = "done", **fields: object) -> None:
        lease.path.unlink(missing_ok=True)
        if lease.handle is not None:
            lease.handle.close()
            lease.handle = None
        if lease.reservation is not None:
            lease.reservation.close()
            lease.reservation = None
        self.event(outcome, id=lease.id, host=lease.host, kind=lease.job.kind, **fields)

    @contextlib.contextmanager
    def placed(self, job: JobSpec, **kwargs: Any) -> Iterator[Lease]:
        lease = self.acquire(job, **kwargs)
        try:
            yield lease
        finally:
            if lease.handle is not None:
                self.release(lease)


def token_valid(token: str, base: Path | None = None) -> bool:
    """A live owner-PC lease carries `token` (its holder still flocks it)."""
    if not token or not token.replace("-", "").isalnum():
        return False
    path = (base or state_dir()) / "active" / f"{token}{LEASE}"
    try:
        handle = path.open("r")
    except OSError:
        return False
    with handle:
        if not _held(handle):
            return False
        try:
            data = json.loads(path.read_text(encoding="utf-8") or "{}")
        except (OSError, ValueError):
            return False
    return data.get("host") == OWNER_PC


def events(base: Path | None = None, since: float = 0.0) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    try:
        lines = ((base or state_dir()) / "events.jsonl").read_text().splitlines()
    except OSError:
        return rows
    for line in lines:
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if float(row.get("at", 0)) >= since:
            rows.append(row)
    return rows


ENDS = ("done", "requeued", "host_failed", "error")


def share(
    rows: Sequence[dict[str, Any]],
    now: float | None = None,
    live: Collection[str] | None = None,
) -> dict[str, Any]:
    """Work placed per host in the window and the owner-PC share of it (the task's metric).

    These are sums over placements, not host capacities (capacities: `tools.dispatch hosts`).
    Work is core-seconds: a placement's cores times its time until its end event (or `now`
    while its lease is `live`); the share is the owner PC's fraction of all core-seconds of
    non-display work (display jobs, the owner's own screen, are `display_core_seconds`). The
    owner PC's work is broken down by why it ran there (`owner_pc_by_reason`: the job's
    local_reason, or `last_choice` for a placeable job). A placement with no end event whose
    lease is gone has an unknown length: it is counted in `unterminated`, not in the work."""
    placed = [row for row in rows if row.get("event") == "placed"]
    ended = {row["id"]: float(row["at"]) for row in rows if row.get("event") in ENDS}
    current = max([float(row.get("at", 0)) for row in rows], default=0.0) if now is None else now
    cores: dict[str, float] = {}
    work: dict[str, float] = {}
    reasons: dict[str, float] = {}
    display = 0.0
    unterminated = 0
    for row in placed:
        host, start = row["host"], float(row["at"])
        if row["id"] not in ended and live is not None and row["id"] not in live:
            unterminated += 1
            continue
        seconds = max(0.0, ended.get(row["id"], current) - start)
        amount = float(row.get("cores", 1)) * seconds
        if row.get("display"):
            display = round(display + amount, 1)
            continue
        cores[host] = round(cores.get(host, 0.0) + float(row.get("cores", 1)), 3)
        work[host] = round(work.get(host, 0.0) + amount, 1)
        if host == OWNER_PC:
            why = str(row.get("local_reason") or "last_choice")
            reasons[why] = round(reasons.get(why, 0.0) + amount, 1)
    total = sum(work.values())
    return {
        "placements": len(placed),
        "placed_cores_by_host": cores,
        "core_seconds_by_host": work,
        "display_core_seconds": display,
        "owner_pc_by_reason": reasons,
        "owner_pc_share": round(work.get(OWNER_PC, 0.0) / total, 3) if total else None,
        "requeued": sum(1 for row in rows if row.get("event") == "requeued"),
        "host_failed": sum(1 for row in rows if row.get("event") == "host_failed"),
        "unterminated": unterminated,
    }
