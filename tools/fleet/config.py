"""Fleet configuration: tools/data/remote_hosts.json (addresses) + tools/data/fleet.json (policy).

`ConfigWatcher.current()` re-reads both files whenever an mtime changes, so the controller picks up
an edit on its next tick without a restart. A file that does not parse (or fails validation) keeps
the previous configuration and is reported through `ConfigWatcher.error`.
"""

from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from tools import host_registry
from tools.wt import retired_hosts

REPO_ROOT = Path(__file__).resolve().parents[2]
HOSTS_FILE = host_registry.REGISTRY_REL.as_posix()  # for messages; read via tools.host_registry
FLEET_FILE = "tools/data/fleet.json"


def main_checkout(start: Path | None = None) -> Path:
    """The primary checkout (holds tmp/codexapi, tmp/owner...), also when run from a worktree.

    `TSFP_FLEET_ROOT` overrides it (tests).
    """
    override = os.environ.get("TSFP_FLEET_ROOT")
    if override:
        return Path(override)
    here = start or REPO_ROOT
    try:
        common = subprocess.run(
            ["git", "-C", str(here), "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True,
            text=True,
            timeout=20,
            check=True,
        ).stdout.strip()
        return Path(common).parent
    except (OSError, subprocess.SubprocessError):
        return here


@dataclass(frozen=True)
class LaunchPolicy:
    min_mem_gb: float = 4
    max_per_tick: int | None = None
    max_load: float | None = None
    needs_local_disk_gb: float | None = None
    foreground: bool = False


@dataclass(frozen=True)
class CapguardPolicy:
    cap_gb: int = 100
    min_free_gb: int = 20
    every_min: int = 15


@dataclass(frozen=True)
class HostConfig:
    name: str
    tag: str
    var: str
    low: int
    high: int
    start: int
    mem_floor_gb: float
    per_worker_gb: float
    disk_floor_gb: float
    replay_max: int = 0
    replay_name: str = ""
    load_down: float = 1.5
    is_local: bool = False
    nice_all: bool = False
    toolchain_parity: bool = False
    build_container: bool = False
    address: str = ""
    port: int = 22
    user: str = ""
    launch: LaunchPolicy = field(default_factory=LaunchPolicy)
    off_file: str | None = None
    capguard: CapguardPolicy | None = None
    min_exec_jobs: int = 0
    """Rule 47 (T3769): remote hosts are execution-only; their floor is concurrent exec jobs
    (remote verify, prebuilt, replay), not Codex light workers (min/max/start stay 0)."""
    memguard: str = "--pause-below-mb 2500 --resume-above-mb 4000"
    provision_every_min: int | None = None
    provision_only_if_repo_missing: bool = False
    payload_provision_every_min: int | None = None
    lclean: bool = False
    replay_target: str | None = None
    tunnel: str | None = None
    registry: dict[str, Any] = field(default_factory=dict, compare=False)

    @property
    def target(self) -> str:
        """ssh destination: user@address (the remote_hosts.json identity)."""
        return f"{self.user}@{self.address}" if self.user else self.address


@dataclass(frozen=True)
class JobSpec:
    name: str
    every_min: float
    builtin: str | None = None
    command: tuple[str, ...] = ()
    legacy: str | None = None
    cwd: str = "codexapi"
    timeout: float = 120
    background: bool = False
    when_file: str | None = None
    when_repo_file: str | None = None
    needs_fresh_status_min: float | None = None


@dataclass(frozen=True)
class FleetConfig:
    root: Path
    hosts: dict[str, HostConfig]
    jobs: tuple[JobSpec, ...]
    tick_seconds: float
    codexapi_dir: Path
    state_dir: Path
    status_file: Path
    legacy_status_file: Path
    queue: dict[str, Any]
    provision: dict[str, Any]
    retired: tuple[str, ...] = ()
    """T3753: hosts marked `retired` in either file; absent from `hosts`, so no job touches them."""

    def host(self, name_or_target: str) -> HostConfig:
        """Find a host by name, tag, address or user@address."""
        for host in self.hosts.values():
            if name_or_target in (host.name, host.tag, host.address, host.target):
                return host
        if name_or_target in self.retired:
            raise KeyError(f"fleet host {name_or_target!r} is retired (T3753)")
        raise KeyError(f"unknown fleet host {name_or_target!r} (known: {', '.join(self.hosts)})")

    def remote_hosts(self) -> list[HostConfig]:
        return [host for host in self.hosts.values() if not host.is_local]

    def resolve(self, relative: str) -> Path:
        """Resolve a configured dir: `codexapi`, `repo`, or a path under the main checkout."""
        if relative == "codexapi":
            return self.codexapi_dir
        if relative == "repo":
            return self.root
        return self.root / relative


class ConfigError(ValueError):
    pass


def _launch(raw: dict[str, Any]) -> LaunchPolicy:
    return LaunchPolicy(
        min_mem_gb=float(raw.get("min_mem_gb", 4)),
        max_per_tick=raw.get("max_per_tick"),
        max_load=raw.get("max_load"),
        needs_local_disk_gb=raw.get("needs_local_disk_gb"),
        foreground=bool(raw.get("foreground", False)),
    )


REGISTRY_OWNED_CAPGUARD = {"cap_gb": "data_cap_gb", "min_free_gb": "disk_floor_gb"}
"""capguard keys whose value remote_hosts.json owns (rule 45, T3752): the disk cap and floor were
stored in both files and drifted (laptop 110 vs 130 GB, deck floor 30 vs 20 GB). T3754: fleet.json
no longer restates them for a registered host; doing so is a ConfigError."""
REGISTRY_OWNED_HOST = {"disk_floor_gb": "disk_floor_gb"}
REGISTRY_OWNED_EXEC = {"min_exec_jobs": "min_exec_jobs"}
"""Host keys remote_hosts.json owns (T3754 decision, claude-1 2026-10-26): the fleet launch floor
is the registry's `disk_floor_gb` (deck 30, droplet2 20 replaced the restated 10 and 30). Only a
host the registry does not list (the local PC) keeps its own value in fleet.json."""


def _restated(
    name: str,
    raw: dict[str, Any],
    entry: dict[str, Any],
    owned: dict[str, str],
    where: str,
    *,
    equal_ok: bool = False,
) -> dict[str, Any]:
    """Values `raw` takes from `entry`; a key restated in fleet.json is a ConfigError
    (`equal_ok`: an equal restatement is tolerated while old long-running readers need it)."""
    derived = {key: entry[source] for key, source in owned.items() if source in entry}
    restated = sorted(
        key for key in derived if key in raw and not (equal_ok and raw[key] == derived[key])
    )
    if restated:
        detail = ", ".join(f"{key} {raw[key]} (registry {derived[key]})" for key in restated)
        raise ConfigError(
            f"fleet host {name!r}: {where} restates {detail}; {HOSTS_FILE} owns it, drop the key"
        )
    return derived


def _capguard(
    name: str, raw: dict[str, Any] | None, entry: dict[str, Any]
) -> CapguardPolicy | None:
    """capguard policy; a registered host takes cap and floor from remote_hosts.json."""
    if not raw:
        return None
    derived = _restated(name, raw, entry, REGISTRY_OWNED_CAPGUARD, "capguard")
    if entry and len(derived) < len(REGISTRY_OWNED_CAPGUARD):
        missing = sorted(set(REGISTRY_OWNED_CAPGUARD.values()) - set(entry))
        raise ConfigError(f"fleet host {name!r}: capguard needs {missing} in {HOSTS_FILE}")
    return CapguardPolicy(**{**raw, **{key: int(value) for key, value in derived.items()}})


def _host(name: str, raw: dict[str, Any], registry: dict[str, dict[str, Any]]) -> HostConfig:
    is_local = bool(raw.get("local", False))
    entry = registry.get(name, {})
    if not is_local and not entry:
        raise ConfigError(f"fleet host {name!r} is not in {HOSTS_FILE}")
    # equal_ok until the balancer and controller run this code (the T3752 loader requires the
    # key); then drop disk_floor_gb from fleet.json for registered hosts (T3754 follow-up)
    raw = {
        **raw,
        **_restated(name, raw, entry, REGISTRY_OWNED_HOST, "fleet.json", equal_ok=True),
    }
    raw = {**raw, **_restated(name, raw, entry, REGISTRY_OWNED_EXEC, "fleet.json")}
    required = (
        "tag",
        "var",
        "min",
        "max",
        "start",
        "mem_floor_gb",
        "per_worker_gb",
        "disk_floor_gb",
    )
    missing = [key for key in required if key not in raw]
    if missing:
        raise ConfigError(f"fleet host {name!r} lacks {missing}")
    if not raw["min"] <= raw["start"] <= raw["max"]:
        raise ConfigError(f"fleet host {name!r}: need min <= start <= max")
    capguard = _capguard(name, raw.get("capguard"), entry)
    return HostConfig(
        name=name,
        tag=raw["tag"],
        var=raw["var"],
        low=int(raw["min"]),
        high=int(raw["max"]),
        start=int(raw["start"]),
        mem_floor_gb=float(raw["mem_floor_gb"]),
        per_worker_gb=float(raw["per_worker_gb"]),
        disk_floor_gb=float(raw["disk_floor_gb"]),
        replay_max=int(raw.get("replay_max", 0)),
        replay_name=raw.get("replay_name", name),
        load_down=float(raw.get("load_down", 1.5)),
        is_local=is_local,
        nice_all=bool(raw.get("nice_all", False)),
        toolchain_parity=bool(raw.get("toolchain_parity", False)),
        build_container=bool(raw.get("build_container", False)),
        address=entry.get("address", ""),
        port=int(entry.get("port", 22)),
        user=entry.get("user", ""),
        launch=_launch(raw.get("launch", {})),
        off_file=raw.get("off_file"),
        capguard=capguard,
        memguard=raw.get("memguard", "--pause-below-mb 2500 --resume-above-mb 4000"),
        provision_every_min=raw.get("provision_every_min"),
        provision_only_if_repo_missing=bool(raw.get("provision_only_if_repo_missing", False)),
        payload_provision_every_min=raw.get("payload_provision_every_min"),
        lclean=bool(raw.get("lclean", False)),
        replay_target=raw.get("replay_target"),
        tunnel=raw.get("tunnel"),
        registry=entry,
        min_exec_jobs=int(raw.get("min_exec_jobs", 0)),
    )


def _job(raw: dict[str, Any]) -> JobSpec:
    if not any(key in raw for key in ("builtin", "command", "legacy")):
        raise ConfigError(f"job {raw.get('name')!r} needs builtin, command or legacy")
    return JobSpec(
        name=raw["name"],
        every_min=float(raw["every_min"]),
        builtin=raw.get("builtin"),
        command=tuple(raw.get("command", ())),
        legacy=raw.get("legacy"),
        cwd=raw.get("cwd", "codexapi"),
        timeout=float(raw.get("timeout", 120)),
        background=bool(raw.get("background", False)),
        when_file=raw.get("when_file"),
        when_repo_file=raw.get("when_repo_file"),
        needs_fresh_status_min=raw.get("needs_fresh_status_min"),
    )


def load_config(
    root: Path | None = None, fleet_file: Path | None = None, hosts_file: Path | None = None
) -> FleetConfig:
    """Parse and validate the two files. Raises ConfigError (or OSError/JSONDecodeError wrapped)."""
    root = root or main_checkout()
    fleet_path = fleet_file or REPO_ROOT / FLEET_FILE
    hosts_path = hosts_file or host_registry.REGISTRY
    try:
        fleet = json.loads(fleet_path.read_text())
        # T3754: remote_hosts.json is read only through its loader
        registry = {entry["name"]: entry for entry in host_registry.load(hosts_path).hosts()}
    except (OSError, ValueError, KeyError) as error:
        raise ConfigError(f"cannot read fleet configuration: {error}") from error
    retired = tuple(
        name
        for name, raw in fleet["hosts"].items()
        if retired_hosts.is_retired(raw) or retired_hosts.is_retired(registry.get(name, {}))
    )
    hosts = {
        name: _host(name, raw, registry)
        for name, raw in fleet["hosts"].items()
        if name not in retired
    }
    jobs = tuple(_job(raw) for raw in fleet.get("jobs", []))
    names = [job.name for job in jobs]
    if len(set(names)) != len(names):
        raise ConfigError("duplicate job names")
    paths = fleet.get("paths", {})
    return FleetConfig(
        root=root,
        hosts=hosts,
        jobs=jobs,
        tick_seconds=float(fleet.get("tick_seconds", 60)),
        codexapi_dir=root / paths.get("codexapi_dir", "tmp/codexapi"),
        state_dir=root / paths.get("state_dir", "tmp/fleet"),
        status_file=(root / paths.get("status_file", "tmp/fleet/status.txt")).resolve(),
        legacy_status_file=Path("/tmp/fleet-status.txt"),
        queue=fleet.get("queue", {}),
        provision=fleet.get("provision", {}),
        retired=retired,
    )


class ConfigWatcher:
    """Hands out the current config; reloads when either source file changed on disk."""

    def __init__(
        self,
        root: Path | None = None,
        fleet_file: Path | None = None,
        hosts_file: Path | None = None,
    ) -> None:
        self.root = root
        self.fleet_file = fleet_file or REPO_ROOT / FLEET_FILE
        self.hosts_file = hosts_file or host_registry.REGISTRY
        self._stamp: tuple[float, float] | None = None
        self._config: FleetConfig | None = None
        self.error: str | None = None
        self.reloads = 0

    def _signature(self) -> tuple[float, float]:
        return (self.fleet_file.stat().st_mtime, self.hosts_file.stat().st_mtime)

    def current(self, force: bool = False) -> FleetConfig:
        try:
            signature = self._signature()
        except OSError as error:
            if self._config is None:
                raise ConfigError(str(error)) from error
            self.error = str(error)
            return self._config
        if force or self._config is None or signature != self._stamp:
            try:
                self._config = load_config(self.root, self.fleet_file, self.hosts_file)
                self._stamp = signature
                self.error = None
                self.reloads += 1
            except ConfigError as error:
                if self._config is None:
                    raise
                self.error = str(error)
                self._stamp = signature  # do not retry the same broken bytes every tick
        return self._config
