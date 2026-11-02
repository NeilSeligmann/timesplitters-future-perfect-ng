"""Per-host status probe (replaces fleet-status.sh) and the legacy status-line format.

The probe prints `key=value` pairs. Remote hosts run `PROBE_SH` over ssh. The local host is read
natively: runner count from the `.runner.lock` flocks of the lane dirs (cross-container safe), plus
the legacy `ps` count so the two can be compared during the side-by-side phase.
"""

from __future__ import annotations

import os
import re
import shutil
import time
from collections.abc import Mapping
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

from tools import hostcaps
from tools.fleet import exec_load, launch_health
from tools.fleet.config import FleetConfig, HostConfig
from tools.fleet.core import Runner, flock_held, ssh_argv
from tools.wt import disk_mounts

DISK_FREE_SH = "disk_free=$(" + disk_mounts.free_gib_command('"$HOME/csup"') + ") "

PROBE_SH = (
    'echo "workers=$(ps -eo args | grep -c "^sh .*run.sh CX-") stopped=$(ps -eo stat | grep -c '
    '"^T") '
    'memguard=$(ps -eo args | grep -c "^python3 .*memguard.py") '
    "memguard_lock=$([ -f ~/csup/memguard.lock ] && ! flock -n ~/csup/memguard.lock true && echo 1 "
    "|| echo 0) " + DISK_FREE_SH + "mem_avail_gb=$(free -g | awk "
    '"NR==2{print \\$7}") '
    "overcap=$([ -f ~/csup/over-cap ] && echo yes || echo no) cores=$(nproc) "
    'load=$(awk "{print \\$1}" /proc/loadavg) runner_stopped=$(ps -eo stat,args | grep -c '
    '"^T.*run.sh CX-") '
    "cg_headroom_gb=$(m=$(cat /sys/fs/cgroup/memory.max 2>/dev/null); c=$(cat "
    "/sys/fs/cgroup/memory.current 2>/dev/null); "
    'case x$m in xmax|x) echo -1;; *) echo $(( (m-c)/1073741824 ));; esac)"; '
    + launch_health.PROBE_SH  # T3763: steps_start/ok/failed of the last 30 min, requeue_ids
    + "; "
    + exec_load.PROBE_SH  # rule 47: busy remote_exec slots (verify, prebuilt)
)


@dataclass
class HostStatus:
    name: str
    reachable: bool = True
    fields: dict[str, str] = field(default_factory=dict)

    def number(self, key: str, default: float = 0.0) -> float:
        try:
            return float(self.fields.get(key, default))
        except ValueError:
            return default

    @property
    def workers(self) -> int:
        return int(self.number("workers"))

    @property
    def paused(self) -> int:
        return int(self.number("runner_stopped"))

    @property
    def overcap(self) -> bool:
        return self.fields.get("overcap") == "yes"

    @property
    def cores(self) -> int:
        return max(1, int(self.number("cores", 1)))

    @property
    def load_per_core(self) -> float:
        return self.number("load") / self.cores

    @property
    def disk_free_gb(self) -> float:
        return gigabytes(self.fields.get("disk_free", "0"))

    @property
    def mounts(self) -> dict[str, int]:
        """Free GB per filesystem builds write to (`disk_mounts` probe field, root `/` included)."""
        return disk_mounts.parse(self.fields.get(disk_mounts.KEY, ""))

    @property
    def lowest_mount(self) -> tuple[str, int] | None:
        return disk_mounts.lowest(self.mounts)

    def mount_violations(
        self, disk_floor_gb: float, root_floor_gb: float
    ) -> list[disk_mounts.Violation]:
        """Mounts under their floor; the root floor is the registry's `root_floor_gb`."""
        return disk_mounts.violations(
            self.mounts, floor_gb=int(disk_floor_gb), root_floor=int(root_floor_gb)
        )

    @property
    def mem_avail_gb(self) -> float:
        """Free RAM, capped by container cgroup headroom when the host has a memory limit."""
        memory = self.number("mem_avail_gb")
        headroom = self.number("cg_headroom_gb", -1)
        return min(memory, headroom) if headroom >= 0 else memory


def gigabytes(value: str) -> float:
    match = re.match(r"([\d.]+)([KMGT]?)", value or "0")
    if not match:
        return 0.0
    number, unit = float(match.group(1)), match.group(2)
    return number * {"K": 1 / 1048576, "M": 1 / 1024, "G": 1, "T": 1024, "": 1}[unit]


def parse_status(text: str) -> dict[str, HostStatus]:
    """Parse the legacy /tmp/fleet-status.txt (`HH:MM:SS name k=v ...` or `... UNREACHABLE`)."""
    hosts: dict[str, HostStatus] = {}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) < 2:
            continue
        hosts[parts[1]] = HostStatus(
            parts[1], "UNREACHABLE" not in line, dict(re.findall(r"(\w+)=(\S+)", line))
        )
    return hosts


def read_status(config: FleetConfig) -> str:
    """Read shared-disk status, falling back to the legacy per-container path for one release."""
    for path in (config.status_file, config.legacy_status_file):
        try:
            return path.read_text()
        except OSError:
            continue
    return ""


def format_status(statuses: Mapping[str, HostStatus], stamp: str | None = None) -> str:
    stamp = stamp or time.strftime("%T")
    lines = []
    for name, status in statuses.items():
        body = (
            " ".join(f"{key}={value}" for key, value in status.fields.items())
            if status.reachable
            else "UNREACHABLE"
        )
        lines.append(f"{stamp} {name} {body}")
    return "\n".join(lines) + "\n"


def _ps_args() -> list[str]:
    try:
        return [line.strip() for line in os.popen("ps -eo args").read().splitlines()]  # noqa: S605
    except OSError:
        return []


def runners_by_lock(codexapi_dir: Path) -> int:
    """Lane dirs whose runner holds `.runner.lock` (run.sh and run-claude.sh take it)."""
    count = 0
    for lane in codexapi_dir.glob("CX-*/.runner.lock"):
        if flock_held(lane):
            count += 1
    return count


def local_status(config: FleetConfig, host: HostConfig, root: Path | None = None) -> HostStatus:
    """Native probe of the machine the controller runs on (the owner PC)."""
    args = _ps_args()
    ps_workers = sum(1 for line in args if re.match(r"sh .*run\.sh CX-", line))
    claude_runners = sum(1 for line in args if re.match(r"sh run-claude\.sh CX-", line))
    lock_workers = runners_by_lock(config.codexapi_dir)
    mem = _meminfo_available_gb()
    usage = shutil.disk_usage(root or config.root)
    fields = {
        "workers": str(max(0, lock_workers - claude_runners)),
        "workers_ps": str(ps_workers),
        "workers_lock": str(lock_workers),
        "memguard": str(sum(1 for line in args if re.match(r"python3 .*memguard\.py", line))),
        "disk_free": f"{usage.free // 2**30}G",
        "mem_avail_gb": str(mem),
        "cores": str(hostcaps.effective_cpus()),  # T3844: the container's quota
        "load": f"{hostcaps.effective_load():.2f}",
        "runner_stopped": "0",
        "cg_headroom_gb": "-1",
    }
    try:  # T3763: the same step outcome counts the remote probe reports
        outcomes = (config.codexapi_dir / launch_health.OUTCOMES).read_text()
    except OSError:
        outcomes = ""
    fields.update(launch_health.fields(launch_health.counts(outcomes, time.time())))
    return HostStatus(host.name, True, fields)


def _meminfo_available_gb() -> int:
    return (hostcaps.effective_mem_available_bytes() or 0) // 2**30  # T3844: cgroup cap


def remote_status(host: HostConfig, runner: Runner, timeout: float = 25) -> HostStatus:
    mounts = disk_mounts.probe_fragment(disk_mounts.paths_for(host.name))
    result = runner(ssh_argv(host.target, host.port, f"{PROBE_SH}; {mounts}"), timeout)
    if not result.ok or "workers=" not in result.out:
        return HostStatus(host.name, False)
    return HostStatus(host.name, True, dict(re.findall(r"(\w+)=(\S+)", result.out)))


def collect(
    config: FleetConfig, runner: Runner, only: set[str] | None = None
) -> dict[str, HostStatus]:
    """Probe every host (remotes in parallel). Hosts with a present off-file are not probed."""
    out: dict[str, HostStatus] = {}
    remotes = []
    for host in config.hosts.values():
        if only and host.name not in only:
            continue
        if host.off_file and (config.codexapi_dir / host.off_file).exists():
            continue
        if host.is_local:
            out[host.name] = local_status(config, host)
        else:
            remotes.append(host)
    with ThreadPoolExecutor(max_workers=max(1, len(remotes))) as pool:
        for host, status in zip(
            remotes, pool.map(lambda h: remote_status(h, runner), remotes), strict=True
        ):
            out[host.name] = status
    try:  # rule 47: replays hold owner-PC slot locks per host; exec_jobs = verify/prebuilt + replay
        replays = exec_load.replay_jobs([h.replay_name or h.name for h in remotes])
    except OSError:
        replays = {}
    for host in remotes:
        status = out[host.name]
        if status.reachable:
            status.fields["replay_jobs"] = str(replays.get(host.replay_name or host.name, 0))
            status.fields["exec_jobs"] = str(exec_load.exec_jobs(status.fields))
    return {name: out[name] for name in config.hosts if name in out}
