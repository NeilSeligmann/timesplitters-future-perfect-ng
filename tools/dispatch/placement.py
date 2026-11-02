"""Pure placement: capacity-weighted least load with capped, bounded owner-PC spillover.

`place(job, hosts, waited_s=..., remote_down_after_s=...)` has no I/O and no clock: the same
input always gives the same host, so every process sharing the queue computes the same plan.
"""

from __future__ import annotations

from collections.abc import Collection, Sequence

from tools.dispatch.model import Decision, HostState, JobSpec


def can_ever_run(job: JobSpec, host: HostState) -> bool:
    """The host has what the job needs, whatever its current load (capabilities and inputs)."""
    if job.owner_pc_only and not host.local:
        return False
    if job.capability not in host.capabilities:
        return False
    if job.owner_inputs and not host.owner_inputs:
        return False
    return not (job.gpu and "gpu" not in host.capabilities)


def automatic_refusal(job: JobSpec, host: HostState) -> str:
    if host.local:
        return ""
    if (
        host.automatic_capabilities is not None
        and job.capability not in host.automatic_capabilities
    ):
        return "automatic_capability_paused"
    if job.kind in {"cargo", "pytest"} and host.validated_commands is not None:
        if not job.validation_key or job.validation_key not in host.validated_commands:
            return "unvalidated_command_environment"
        if not job.remote_only and job.est_seconds < host.remote_min_seconds:
            return "short_or_unmeasured_work"
    return ""


def granted(job: JobSpec, host: HostState) -> float:
    """Cores the job gets on `host`: all it asks for, or for an elastic job (min_cores) what is
    free down to its minimum, so a pytest set runs with fewer workers instead of waiting."""
    minimum = host.verify_cores if job.kind in ("cargo", "pytest") else 0.0
    if not job.min_cores or not host.jobs:
        return max(job.cores, minimum)
    free = round(host.capacity - host.committed_cores, 3)
    return max(minimum, job.min_cores, min(job.cores, free))


def refusal(job: JobSpec, host: HostState, *, owner_only: bool = False) -> str:
    """Why `host` cannot take `job` now ("" = it can). `owner_only`: no other host could run
    the job, so the owner PC's load gate does not apply (only its cap)."""
    if not can_ever_run(job, host):
        return "unsuitable"
    automatic = automatic_refusal(job, host)
    if automatic:
        return automatic
    if not host.up:
        return f"down({host.reason or 'unknown'})"
    if host.max_jobs is not None and host.jobs >= host.max_jobs:
        return f"jobs_cap_{host.max_jobs}"
    # A job larger than the whole host still runs there, alone (else it would wait forever).
    need = granted(job, host)
    if host.local and need > host.capacity + 1e-9:
        return f"cores_{need:g}>{host.capacity:g}"
    if host.jobs and host.committed_cores + need > host.capacity + 1e-9:
        return f"cores_{host.committed_cores:g}+{need:g}>{host.capacity:g}"
    memory = host.memory(job)
    available = host.mem_free_mb
    if host.budget_mem_mb is not None:
        available = min(available, host.budget_mem_mb)
    if memory and available - host.committed_mem_mb < memory:
        return f"mem_{available - host.committed_mem_mb}MB<{memory}MB"
    if (
        host.local
        and not (job.owner_pc_only or owner_only)
        and host.max_load_per_core is not None
        and host.load1 is not None
        and host.cores
        and host.load1 / host.cores > host.max_load_per_core
    ):
        return f"owner_pc_load_{host.load1:g}/{host.cores}"
    return ""


def score(job: JobSpec, host: HostState) -> float:
    """Load after placement per unit of capacity (smaller is better).

    The live load already contains jobs that were running at the last probe and the commitments
    contain jobs placed since; the larger of the two is the host's present load."""
    present = max(host.load1 or 0.0, host.committed_cores)
    return round((present + granted(job, host)) / max(host.capacity, 0.001), 6)


def place(
    job: JobSpec,
    hosts: Sequence[HostState],
    *,
    waited_s: float = 0.0,
    remote_down_after_s: float = 600.0,
    remote_busy_after_s: float = 120.0,
    local_first_kinds: Collection[str] = (),
    exclude: Collection[str] = (),
) -> Decision:
    """The host for `job`, or a Decision without host (the job waits in the queue).

    Remote hosts first, by score, then most free memory, then registry order. The owner PC is
    taken for owner-only work, after a bounded busy wait, or when every capable remote is
    down and the job has waited `remote_down_after_s`. A job no remote host can
    ever run (no host has the kind's capability) is an owner-PC job at once."""
    refused: list[str] = []
    remote: list[tuple[float, int, int, HostState]] = []
    local: HostState | None = None
    suitable_remote_up = False
    remote_capable = any(not host.local and can_ever_run(job, host) for host in hosts)
    if job.kind in local_first_kinds and not job.remote_only:
        owner = next((host for host in hosts if host.local and host.name not in exclude), None)
        if owner is not None and not refusal(job, owner):
            return Decision(
                owner.name, score(job, owner), (), granted(job, owner), owner.memory(job)
            )
    for host in hosts:
        if host.name in exclude:
            refused.append(f"{host.name}:excluded")  # failed for this job, or reserved
            continue
        if host.local:
            if not job.remote_only:
                local = host
            continue
        if can_ever_run(job, host) and host.up:
            suitable_remote_up = True
        why = refusal(job, host)
        if why:
            if why != "unsuitable" or not job.owner_pc_only:
                refused.append(f"{host.name}:{why}")
            continue
        remote.append(
            (score(job, host), -(host.mem_free_mb - host.committed_mem_mb), host.order, host)
        )
    if remote:
        best = min(remote, key=lambda item: item[:3])
        return Decision(
            best[3].name, best[0], tuple(refused), granted(job, best[3]), best[3].memory(job)
        )
    if local is not None:
        why = refusal(job, local, owner_only=not remote_capable)
        if not why and not job.owner_pc_only and remote_capable:
            if suitable_remote_up:
                if waited_s < remote_busy_after_s:
                    why = (
                        f"last_choice(remote_busy,waited_{waited_s:.0f}s"
                        f"<{remote_busy_after_s:.0f}s)"
                    )
            elif waited_s < remote_down_after_s:
                why = f"last_choice(remote_down,waited_{waited_s:.0f}s<{remote_down_after_s:.0f}s)"
        if not why:
            return Decision(
                local.name,
                score(job, local),
                tuple(refused),
                granted(job, local),
                local.memory(job),
            )
        refused.append(f"{local.name}:{why}")
    return Decision(None, None, tuple(refused))


RESERVE_AFTER_S = 120.0
"""Waited seconds after which an owner-PC-only job reserves the owner PC against backfill."""


def plan(
    jobs: Sequence[tuple[str, JobSpec, float, Collection[str]]],
    hosts: Sequence[HostState],
    *,
    remote_down_after_s: float = 600.0,
    remote_busy_after_s: float = 120.0,
    local_first_kinds: Collection[str] = (),
    reserve_after_s: float = RESERVE_AFTER_S,
) -> dict[str, Decision]:
    """Place `jobs` (id, spec, waited seconds, excluded hosts) in the given order against one
    view, each placement committed before the next job is placed. `hosts` is changed in place.

    No starvation: an owner-PC-only job that has waited `reserve_after_s` and is refused only
    by the owner PC's cap reserves the owner PC; later jobs are not placed there in this plan
    (else small jobs backfill forever ahead of a job that needs the PC to itself)."""
    by_name = {host.name: host for host in hosts}
    decisions: dict[str, Decision] = {}
    reserved: set[str] = set()
    for job_id, spec, waited, exclude in jobs:
        decision = place(
            spec,
            hosts,
            waited_s=waited,
            remote_down_after_s=remote_down_after_s,
            remote_busy_after_s=remote_busy_after_s,
            local_first_kinds=local_first_kinds,
            exclude={*exclude, *reserved},
        )
        decisions[job_id] = decision
        if decision.host is not None:
            by_name[decision.host].commit(spec, decision.cores)
            continue
        owner = next((host for host in hosts if host.local), None)
        if (
            owner is not None
            and owner.name not in exclude
            and waited >= reserve_after_s
            and not owner_capable_remote(spec, hosts)
            and refusal(spec, owner, owner_only=True).startswith(("cores_", "jobs_cap_"))
        ):
            reserved.add(owner.name)
    return decisions


def owner_capable_remote(job: JobSpec, hosts: Sequence[HostState]) -> bool:
    """A remote host could run the job (then it is not an owner-PC-only job)."""
    return not job.owner_pc_only and any(
        not host.local and can_ever_run(job, host) for host in hosts
    )
