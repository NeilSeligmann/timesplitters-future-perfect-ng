"""Data model of the work dispatcher (T3885): what a job needs and what a host offers."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from typing import Any

OWNER_PC = "local"
"""The owner PC's name in the dispatcher view (the same key `tools.fleet.headroom` uses)."""

KIND_CAPABILITY = {
    "cargo": "verify",
    "pytest": "pytest",
    "replay": "replay",
    "capture": "capture",
    "gate": "replay",
    "game": "game",
}
"""The registry capability a host needs for each job kind (`remote_hosts.json` capabilities)."""


@dataclass(frozen=True)
class JobSpec:
    """One unit of heavy work, described by its needs (never by where it should run)."""

    kind: str
    cores: float
    mem_mb: int = 0
    owner_inputs: bool = False
    display: bool = False
    gpu: bool = False
    local_only: bool = False
    local_reason: str = ""
    est_seconds: float = 0.0
    priority: int = 0
    label: str = ""
    min_cores: float = 0.0
    """Elastic jobs (pytest workers): placed with fewer cores, down to this, when a host has only
    part of `cores` free (0 = rigid). The granted cores are on the lease."""
    validation_key: str = ""
    remote_only: bool = False

    def __post_init__(self) -> None:
        if self.kind not in KIND_CAPABILITY:
            raise ValueError(
                f"unknown job kind {self.kind!r} (known: {', '.join(KIND_CAPABILITY)})"
            )
        if self.cores <= 0:
            raise ValueError("a job needs a positive core count")
        if self.mem_mb < 0:
            raise ValueError("mem_mb must not be negative")

    @property
    def capability(self) -> str:
        return KIND_CAPABILITY[self.kind]

    @property
    def owner_pc_only(self) -> bool:
        """Only the owner PC can run it: a visible display, or inputs only it holds."""
        return self.display or self.local_only

    def to_json(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> JobSpec:
        known = {name for name in cls.__dataclass_fields__}
        return cls(**{key: value for key, value in data.items() if key in known})


@dataclass
class HostState:
    """One host as the dispatcher sees it now (static facts plus live state plus commitments)."""

    name: str
    cores: int
    capacity: float
    """Core units the dispatcher may commit (remote: cores x load_per_core; owner PC: its cap)."""
    mem_free_mb: int
    load1: float | None
    up: bool
    reason: str = ""
    """Why the host is not up (disabled, benched, unreachable), "" when up."""
    capabilities: frozenset[str] = frozenset()
    owner_inputs: bool = False
    local: bool = False
    order: int = 0
    max_jobs: int | None = None
    max_load_per_core: float | None = None
    """Owner PC only: above this load it takes only jobs no other host can run."""
    committed_cores: float = 0.0
    committed_mem_mb: int = 0
    jobs: int = 0
    notes: list[str] = field(default_factory=list)
    budget_mem_mb: int | None = None
    verify_cores: float = 0.0
    verify_mem_mb: int = 0
    automatic_capabilities: frozenset[str] | None = None
    validated_commands: frozenset[str] | None = None
    remote_min_seconds: float = 0.0

    def memory(self, job: JobSpec) -> int:
        """Reserve the executor's price as well as the submitter's requested memory."""
        return max(job.mem_mb, self.verify_mem_mb if job.kind in ("cargo", "pytest") else 0)

    def commit(self, job: JobSpec, cores: float | None = None) -> None:
        self.committed_cores = round(
            self.committed_cores + (job.cores if cores is None else cores), 3
        )
        self.committed_mem_mb += self.memory(job)
        self.jobs += 1

    def to_json(self) -> dict[str, Any]:
        data = asdict(self)
        data["capabilities"] = sorted(self.capabilities)
        for key in ("automatic_capabilities", "validated_commands"):
            if data[key] is not None:
                data[key] = sorted(data[key])
        return data


@dataclass(frozen=True)
class Decision:
    """The placement of one job: a host, or None with every refusal reason (it waits)."""

    host: str | None
    score: float | None = None
    refused: tuple[str, ...] = ()
    cores: float | None = None
    """Cores granted (an elastic job may get fewer than it asked for), None = as asked."""
    mem_mb: int | None = None

    @property
    def placed(self) -> bool:
        return self.host is not None
