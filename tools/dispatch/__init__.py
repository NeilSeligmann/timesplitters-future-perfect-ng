"""T3885: the one deterministic work dispatcher (docs/t3885-dispatcher.md).

Every heavy job is submitted with its needs (`JobSpec`), placed by capacity-weighted least load
over one registry view (`hosts.build_view`), queued while no host fits (never a silent local
run), and executed through the existing remote machinery (`remote_fallback.run_remote`,
`remote_exec`, `exec_sync`) or, on the owner PC, with a placement token in its environment.

    python3 -m tools.dispatch submit --kind cargo --cores 8 -- cargo test -p tsfp-types
    python3 -m tools.dispatch status
    python3 -m tools.dispatch hosts
"""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from tools.dispatch.model import OWNER_PC, Decision, HostState, JobSpec
from tools.dispatch.queue import TOKEN_ENV, Dispatcher, Lease, state_dir, token_valid
from tools.dispatch.runner import DispatchError, HostBusy, HostFailed, NotPortable, run

__all__ = [
    "OWNER_PC",
    "TOKEN_ENV",
    "Decision",
    "DispatchError",
    "Dispatcher",
    "HostBusy",
    "HostFailed",
    "HostState",
    "JobSpec",
    "Lease",
    "NotPortable",
    "default",
    "job",
    "run",
    "state_dir",
    "token_valid",
]


def default(main: Path, log: Callable[[str], object] = lambda _text: None) -> Dispatcher:
    """The dispatcher over the live registry view of `main` (the main checkout).

    Under pytest it refuses (rule 39): a test injects `tools.dispatch.testing.fake_dispatcher`
    instead of probing and dispatching to the real hosts."""
    import os

    if os.environ.get("PYTEST_CURRENT_TEST") and not os.environ.get("TSFP_DISPATCH_LIVE_IN_TEST"):
        raise RuntimeError(
            "tools.dispatch.default under pytest: inject a fake dispatcher "
            "(tools.dispatch.testing.fake_dispatcher)"
        )
    from tools.dispatch import hosts
    from tools.wt import host_budget, remote_exec

    rules = hosts.policy()
    base = state_dir()

    def view(committed: dict[str, tuple[float, int, int]], pids: set[int]) -> list[HostState]:
        return hosts.build_view(main, base, committed=committed, foreign=hosts.foreign_budget(pids))

    def reserve(spec: JobSpec, decision: Decision) -> host_budget.Reservation | None:
        if decision.host == OWNER_PC or spec.kind not in ("cargo", "pytest"):
            return None
        host = remote_exec.find_host(decision.host)
        if host.budget is None:
            return None
        cost = host_budget.Cost(
            decision.cores or spec.cores,
            decision.mem_mb if decision.mem_mb is not None else spec.mem_mb,
        )
        handle, used = host_budget.try_acquire(
            host.name,
            "verify",
            cost,
            host.budget,
            register=False,
        )
        if handle is None:
            raise host_budget.BudgetFull(f"{host.name}: {cost} beside {used}")
        return host_budget.Reservation(host.name, "verify", cost, handle)

    return Dispatcher(
        view=view,
        base=base,
        poll_s=rules.poll_s,
        remote_down_after_s=rules.remote_down_after_s,
        remote_busy_after_s=rules.remote_busy_after_s,
        local_first_kinds=rules.local_first_kinds,
        reserve=reserve,
        log=log,
    )


def job(kind: str, cores: float, **needs: object) -> JobSpec:
    """A JobSpec with the kind's registry memory price unless `mem_mb` is given."""
    from tools.dispatch import hosts, remote_evidence

    commands, root = needs.pop("commands", None), needs.pop("root", None)
    if commands is not None and root is not None:
        key = remote_evidence.command_key(list(commands), root)
        needs["validation_key"] = key
        needs.setdefault("est_seconds", remote_evidence.estimate(key))

    if "mem_mb" not in needs:
        needs["mem_mb"] = hosts.default_mem_mb(kind)
    return JobSpec(kind=kind, cores=cores, **needs)  # type: ignore[arg-type]
