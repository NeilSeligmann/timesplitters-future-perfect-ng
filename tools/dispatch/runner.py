"""Run a placed job and handle what the host did with it (T3885).

`run(dispatcher, job, remote=..., local=...)` loops: acquire a placement, run the job there,
release the lease. The callables raise to say the host gave no verdict:

* `HostBusy`: slot taken while staging, budget full, run-time host-health refusal. The job goes
  back to the queue with its arrival time; the host is not penalised.
* `HostFailed`: transport, staging or infrastructure failure. The host is benched through the
  shared bench (`remote_exec.mark_degraded`), excluded for this job, and the job is requeued, up
  to `max_attempts` failed hosts; then `DispatchError`. Never a local re-run.
* `NotPortable`: the job cannot run on a remote host (command shape, zero tests remotely). It is
  requeued as `local_only` with the reason, so it is placed on the owner PC within its cap and
  counted in the owner-PC share.

`remote_commands` adapts the existing remote runner (`tools.wt.remote_fallback.run_remote`, which
syncs the exact tree with `exec_sync` and runs it in a slot of the placed host, rule 47).
"""

from __future__ import annotations

import dataclasses
import secrets
from collections.abc import Callable, Collection, Sequence
from pathlib import Path
from typing import Any, TypeVar

from tools.dispatch.model import OWNER_PC, JobSpec
from tools.dispatch.queue import Dispatcher, DispatchError, Lease

R = TypeVar("R")


class HostBusy(RuntimeError):  # noqa: N818 - a requeue signal, not an error
    pass


class HostFailed(RuntimeError):  # noqa: N818
    pass


class NotPortable(RuntimeError):  # noqa: N818
    pass


class HostRefused(RuntimeError):  # noqa: N818
    """The host's own admission refused the job (data cap, disk floor, memory): skip it for
    REFUSED_EXCLUDE_S, then probe it again; no bench, not a failed attempt."""


FAILED_EXCLUDE_S = 300.0
"""A host that failed a job is tried again by that job after this long (and its bench ends)."""
REFUSED_EXCLUDE_S = 120.0
BUSY_EXCLUDE_S = 30.0
BENCH_MIN = 5.0
"""Minutes a failed host stays benched for every caller (remote_exec.mark_degraded)."""


def run(
    dispatcher: Dispatcher,
    job: JobSpec,
    *,
    remote: Callable[[Lease], R],
    local: Callable[[Lease], R],
    max_attempts: int = 3,
    mark_failed: Callable[[str, str], None] = lambda _host, _reason: None,
    timeout_s: float | None = None,
    exclude: Collection[str] = (),
    hand_over_local: bool = False,
) -> R:
    """Place and run `job` until a host gives a verdict (the callables' return value).

    `exclude`: hosts this job must never use (`OWNER_PC` for a remote-only job). A failed or
    refusing host is excluded only for FAILED_EXCLUDE_S / REFUSED_EXCLUDE_S, so a job never
    ends up with no host for good (claude-1 21:55: a landing sat 21 min with the laptop
    excluded and the rest full).
    `hand_over_local`: an owner-PC placement returns its live `Lease` instead of calling
    `local` (a caller that execs the job keeps the lease's flock across the exec)."""
    never = set(exclude)
    if OWNER_PC in never:
        job = dataclasses.replace(job, remote_only=True)
    until: dict[str, float] = dict.fromkeys(never, 0.0)
    failures: list[str] = []
    since = dispatcher.clock()
    logical_id = secrets.token_hex(6)
    while True:
        now = dispatcher.clock()
        until = {h: t for h, t in until.items() if not t or t > now}
        lease = dispatcher.acquire(
            job, exclude=until, since=since, timeout_s=timeout_s, logical_id=logical_id
        )
        if hand_over_local and lease.local:
            return lease  # type: ignore[return-value]
        try:
            value = local(lease) if lease.local else remote(lease)
        except HostBusy as busy:
            dispatcher.release(lease, outcome="requeued", reason=f"busy:{busy}"[:200])
            until[lease.host] = dispatcher.clock() + BUSY_EXCLUDE_S
            dispatcher.sleep(dispatcher.poll_s)
            continue
        except HostFailed as failed:
            reason = str(failed)[:200]
            dispatcher.release(lease, outcome="host_failed", reason=reason)
            mark_failed(lease.host, reason)
            until[lease.host] = dispatcher.clock() + FAILED_EXCLUDE_S
            failures.append(f"{lease.host}:{reason}")
            if len(failures) >= max_attempts:
                raise DispatchError(
                    f"dispatch: {job.label or job.kind} failed on {len(failures)} hosts: "
                    + "; ".join(failures)
                ) from failed
            continue
        except HostRefused as refused:
            dispatcher.release(lease, outcome="requeued", reason=f"refused:{refused}"[:200])
            until[lease.host] = dispatcher.clock() + REFUSED_EXCLUDE_S
            continue
        except NotPortable as reason:
            dispatcher.release(lease, outcome="requeued", reason=f"not_portable:{reason}"[:200])
            if OWNER_PC in never:  # remote-only: no host can give a verdict, say so now
                raise DispatchError(
                    f"dispatch: {job.label or job.kind} has no remote verdict ({reason}) "
                    "and must not run on the owner PC"
                ) from reason
            job = dataclasses.replace(job, local_only=True, local_reason=str(reason)[:120])
            continue
        except BaseException:
            dispatcher.release(lease, outcome="error")
            raise
        dispatcher.release(lease, outcome="done")
        return value


BUSY_MARKS = ("slot_busy", "budget_full", "slot_taken", "host_health")
REFUSED_MARKS = ("tsfp_data_", "disk_free_", "disk_mount_", "mem_", "missing_tool_", "environment:")
"""remote_exec.admission refusals: the host is fine but full or short of something now."""
NOT_PORTABLE_MARKS = ("not_portable", "zero_tests_remote", "dirty_tree", "cdiff_snapshot")


def classify_infra(notes: Sequence[str]) -> type[RuntimeError]:
    """The requeue signal of a remote outcome without a verdict."""
    text = " ".join(notes)
    if any(mark in text for mark in NOT_PORTABLE_MARKS):
        return NotPortable
    if any(mark in text for mark in REFUSED_MARKS):
        return HostRefused
    if any(mark in text for mark in BUSY_MARKS):
        return HostBusy
    return HostFailed


def remote_commands(
    commands: Sequence[str],
    *,
    root: Path,
    main: Path,
    log: Callable[[str], object],
    command_timeout_s: int = 1800,
    env: dict[str, str] | None = None,
    run_remote: Callable[..., Any] | None = None,
    tail_lines: int = 60,
) -> Callable[[Lease], Any]:
    """A `remote` callable running portable `commands` on the placed host; returns the
    `remote_fallback.Outcome` (status pass or fail) or raises a requeue signal. `tail_lines`:
    log lines per command brought back (a pytest run needs its whole failure summary)."""

    def call(lease: Lease) -> Any:
        from tools.wt import remote_exec, remote_fallback

        runner = run_remote or remote_fallback.run_remote
        spec = remote_exec.find_host(lease.host)
        reserved = (
            {"budget_reservation": lease.reservation} if lease.reservation is not None else {}
        )
        outcome = runner(
            list(commands),
            root=root,
            main=main,
            hosts=[spec],
            log=log,
            queue_s=0.0,
            command_timeout_s=command_timeout_s,
            env=env,
            tail_lines=tail_lines,
            **reserved,
        )
        if outcome.status in ("pass", "fail"):
            return outcome
        notes = list(outcome.notes) or ["no_verdict"]
        raise classify_infra(notes)("; ".join(notes)[:300])

    return call


def mark_degraded(main: Path) -> Callable[[str, str], None]:
    """Bench a failed host on the shared bench every path and the balancer read."""

    def mark(host: str, reason: str) -> None:
        from tools.wt import remote_exec

        remote_exec.mark_degraded(main, host, f"dispatch:{reason}", minutes=BENCH_MIN)

    return mark
