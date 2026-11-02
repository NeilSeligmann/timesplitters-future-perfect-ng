"""The dispatcher's one registry view (T3885), built from the existing owners of each fact.

* static facts: `tools.wt.remote_exec.load_registry` (tools/data/remote_hosts.json, retired hosts
  dropped) and its `dispatch` section (`tools.host_registry`) for the owner PC's cap;
* live state: the balancer's headroom cache (`tools.fleet.headroom.read_cache`), else a direct
  probe (`remote_exec.probe`) kept for `probe_ttl_s`;
* switches: `remote_exec.host_disabled` (owner `.off`, retired) and `remote_exec.degraded_reason`
  (benched after a failure);
* commitments: the dispatcher's own leases plus `tools.wt.host_budget` leases of processes that
  hold no dispatcher lease (paths not migrated yet).
"""

from __future__ import annotations

import json
import time
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from tools import host_registry
from tools.dispatch.model import OWNER_PC, HostState, JobSpec

SECTION = "dispatch"


@dataclass(frozen=True)
class Policy:
    """The `dispatch` section of the host registry (loud error when it is missing)."""

    owner_max_cores: float
    owner_max_jobs: int
    owner_max_load_per_core: float
    remote_down_after_s: float
    owner_capabilities: frozenset[str]
    mem_mb_by_kind: Mapping[str, int]
    poll_s: float
    probe_ttl_s: float
    max_attempts: int
    remote_only_wait_s: float
    remote_busy_after_s: float = 120.0
    short_pytest_max_files: int = 2
    local_first_kinds: frozenset[str] = frozenset({"cargo", "pytest"})
    remote_min_seconds: float = 120.0


def policy(path: Path | None = None) -> Policy:
    section = host_registry.load(path).section(SECTION)
    owner = section.get("owner_pc")
    if not isinstance(owner, dict):
        raise host_registry.RegistryError(f"{SECTION}.owner_pc is missing from the host registry")
    return Policy(
        owner_max_cores=float(owner["max_cores"]),
        owner_max_jobs=int(owner["max_jobs"]),
        owner_max_load_per_core=float(owner["max_load_per_core"]),
        remote_down_after_s=float(owner["remote_down_after_s"]),
        owner_capabilities=frozenset(str(item) for item in owner["capabilities"]),
        mem_mb_by_kind={str(k): int(v) for k, v in section.get("mem_mb_by_kind", {}).items()},
        poll_s=float(section.get("poll_s", 5)),
        probe_ttl_s=float(section.get("probe_ttl_s", 60)),
        max_attempts=int(section.get("max_attempts", 3)),
        remote_only_wait_s=float(section.get("remote_only_wait_s", 900)),
        remote_busy_after_s=float(owner.get("remote_busy_after_s", 120)),
        short_pytest_max_files=int(section.get("short_pytest_max_files", 2)),
        local_first_kinds=frozenset(section.get("local_first_kinds", ["cargo", "pytest"])),
        remote_min_seconds=float(section.get("remote_min_seconds", 120)),
    )


def default_mem_mb(kind: str, path: Path | None = None) -> int:
    """A kind's memory price: cargo from `job_costs.verify`, capture from `job_costs.capture`,
    the rest from `dispatch.mem_mb_by_kind` (0 when unpriced)."""
    costs = host_registry.load(path).section("job_costs")
    alias = {"cargo": "verify", "capture": "capture"}.get(kind)
    if alias and isinstance(costs.get(alias), dict):
        return int(costs[alias].get("mem_mb", 0))
    return int(policy(path).mem_mb_by_kind.get(kind, 0))


@dataclass(frozen=True)
class Live:
    """Live state of one host: reachable/eligible, load, free memory, cores."""

    up: bool
    reason: str
    load1: float | None
    mem_free_mb: int
    cores: int


def live_from_headroom(cache: Mapping[str, Any] | None) -> dict[str, Live]:
    """Per-host live state from the balancer cache (`headroom.read_cache`), {} without it."""
    if not cache:
        return {}
    found: dict[str, Live] = {}
    for name, item in (cache.get("hosts") or {}).items():
        if not isinstance(item, dict):
            continue
        reachable = bool(item.get("reachable", False))
        reason = str(item.get("reason") or "")
        # The balancer marks the owner PC ineligible for codex workers; for dispatch only its
        # reachability matters (its cap and load gate are the dispatcher's own).
        up = reachable if item.get("is_local") else reachable and bool(item.get("eligible"))
        found[str(name)] = Live(
            up=up,
            reason="" if up else (reason or "unreachable"),
            load1=float(item["load1"]) if item.get("load1") is not None else None,
            mem_free_mb=int(float(item.get("mem_avail_gb") or 0) * 1024),
            cores=int(item.get("cores") or 0),
        )
    return found


def _probe_live(spec: Any, root: Path) -> Live:
    from tools.wt import remote_exec

    probe = remote_exec.probe(spec, root=root)
    return Live(
        up=probe.ok,
        reason=",".join(probe.reasons)[:120],
        load1=probe.load1,
        mem_free_mb=probe.mem_available_mb,
        cores=probe.cores or spec.cores,
    )


def probed(
    specs: list[Any], root: Path, cache_file: Path, ttl_s: float, now: float
) -> dict[str, Live]:
    """Direct probes when the balancer cache is stale, reused for `ttl_s` by every caller."""
    try:
        saved = json.loads(cache_file.read_text())
        if now - float(saved.get("at", 0)) <= ttl_s:
            return {name: Live(**value) for name, value in saved["hosts"].items()}
    except (OSError, ValueError, KeyError, TypeError):
        pass
    found = {spec.name: _probe_live(spec, root) for spec in specs}
    try:
        cache_file.parent.mkdir(parents=True, exist_ok=True)
        tmp = cache_file.with_suffix(".tmp")
        tmp.write_text(json.dumps({"at": now, "hosts": {k: v.__dict__ for k, v in found.items()}}))
        tmp.replace(cache_file)
    except OSError:
        pass
    return found


def build_view(
    main: Path,
    state_dir: Path,
    *,
    committed: Mapping[str, tuple[float, int, int]] | None = None,
    foreign: Callable[[str], tuple[float, int]] | None = None,
    now: float | None = None,
) -> list[HostState]:
    """Every host the dispatcher may place on, the owner PC last.

    `committed`: host -> (cores, mem_mb, jobs) of live dispatcher leases; `foreign`: host ->
    (cores, mem_mb) committed by processes outside the dispatcher (host_budget leases)."""
    from tools.dispatch import remote_evidence
    from tools.fleet import headroom
    from tools.wt import remote_exec

    current = time.time() if now is None else now
    rules = policy()
    specs = remote_exec.load_registry()
    cache = headroom.read_cache(main, now=current)
    live = live_from_headroom(cache)
    aliases = (cache or {}).get("aliases", {}) if cache else {}
    missing = [spec for spec in specs if aliases.get(spec.name, spec.name) not in live]
    if missing:
        live.update(probed(missing, main, state_dir / "probes.json", rules.probe_ttl_s, current))
    commitments = dict(committed or {})
    hosts: list[HostState] = []
    for order, spec in enumerate(specs):
        state = live.get(aliases.get(spec.name, spec.name)) or live.get(spec.name)
        up, reason = (state.up, state.reason) if state else (False, "no_live_state")
        if remote_exec.host_disabled(main, spec.name):
            up, reason = False, "disabled"
        else:
            benched = remote_exec.degraded_reason(main, spec.name)
            if benched:
                up, reason = False, f"benched:{benched}"[:120]
        capacity = spec.budget.cores if spec.budget else float(spec.cores)
        verify_cost = remote_exec.verify_cost(spec)
        host = HostState(
            name=spec.name,
            cores=spec.cores,
            capacity=capacity,
            mem_free_mb=state.mem_free_mb if state else 0,
            load1=state.load1 if state else None,
            up=up,
            reason=reason,
            capabilities=frozenset(spec.capabilities),
            owner_inputs=spec.has_owner_inputs(),
            order=order,
            max_jobs=spec.slots,
            budget_mem_mb=spec.budget.mem_mb if spec.budget else None,
            verify_cores=verify_cost.cores,
            verify_mem_mb=verify_cost.mem_mb,
            automatic_capabilities=(
                frozenset(spec.automatic_capabilities)
                if spec.automatic_capabilities is not None
                else None
            ),
            validated_commands=remote_evidence.validated_keys(spec, state_dir, current),
            remote_min_seconds=rules.remote_min_seconds,
        )
        cores, mem, jobs = commitments.get(spec.name, (0.0, 0, 0))
        extra_cores, extra_mem = foreign(spec.name) if foreign else (0.0, 0)
        host.committed_cores = round(cores + extra_cores, 3)
        host.committed_mem_mb = mem + extra_mem
        host.jobs = jobs + (1 if extra_cores else 0)
        hosts.append(host)
    owner = live.get(OWNER_PC)
    from tools import hostcaps

    owner_cores = owner.cores if owner and owner.cores else hostcaps.effective_cpus()
    local = HostState(
        name=OWNER_PC,
        cores=owner_cores,
        capacity=rules.owner_max_cores,
        mem_free_mb=owner.mem_free_mb if owner else 0,
        load1=owner.load1 if owner else None,
        up=True,
        capabilities=rules.owner_capabilities,
        owner_inputs=True,
        local=True,
        order=len(hosts),
        max_jobs=rules.owner_max_jobs,
        max_load_per_core=rules.owner_max_load_per_core,
    )
    if not owner or not owner.mem_free_mb:  # no memory reading: never refuse on memory alone
        local.mem_free_mb = 1 << 30
    cores, mem, jobs = commitments.get(OWNER_PC, (0.0, 0, 0))
    local.committed_cores, local.committed_mem_mb, local.jobs = cores, mem, jobs
    hosts.append(local)
    return hosts


def foreign_budget(dispatch_pids: set[int]) -> Callable[[str], tuple[float, int]]:
    """host -> committed (cores, mem) of host_budget leases held by non-dispatcher processes."""
    from tools.wt import host_budget

    def read(name: str) -> tuple[float, int]:
        try:
            _cost, leases = host_budget.usage(name)
        except OSError:
            return 0.0, 0
        mine = [lease for lease in leases if lease.pid not in dispatch_pids]
        return (
            round(sum(lease.cost.cores for lease in mine), 3),
            sum(lease.cost.mem_mb for lease in mine),
        )

    return read


def describe(job: JobSpec) -> str:
    needs = [f"{job.cores:g} cores", f"{job.mem_mb} MB"]
    needs += [
        name for name in ("owner_inputs", "display", "gpu", "local_only") if getattr(job, name)
    ]
    return f"{job.kind}({', '.join(needs)})"
