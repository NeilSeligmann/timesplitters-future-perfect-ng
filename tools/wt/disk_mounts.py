"""Free space on every filesystem a remote build writes to (class remote-disk-full, 2026-10-28).

The laptop's container root `/` was 100% full (1.6 GB free) while the probe reported 85 GB free,
because it measured only the `/workspace` data volume; remote prebuilds failed HOST-BUSY. The probe
now reports the free space of each mount under `/`, `/tmp`, the cargo/sccache dirs, the exec dirs
and the data paths. The floors come from `tools/data/remote_hosts.json` (rule 45): the root
filesystem uses `root_floor_gb` (host entry, else the top-level `disk_mounts` block), every other
mount the host's `disk_floor_gb`. A mount under its floor is a refusal reason and a HEALTH alert
that names the mount.
"""

from __future__ import annotations

import re
import shlex
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from tools import host_registry

KEY = "disk_mounts"
"""Registry block and probe/status key: `<mount>:<floored_free_gib>` pairs, comma separated."""
ROOT = "/"
PATH_KEYS = ("workdir", "canon_root", "toolchain_dir", "target_dir", "replay_cache_dir")
GIB = 1 << 30


def free_gib_awk(*, include_mount: bool = False, suffix: bool = True) -> str:
    """Awk expression for exact-byte input rounded down to whole GiB."""
    value = f"int($1/{GIB})"
    output = f'$2 ":" {value}' if include_mount else f'{value} "G"' if suffix else value
    return f"awk '$1 ~ /^[0-9]+$/ {{print {output}}}'"


def free_gib_command(
    path_expression: str, *, include_mount: bool = False, suffix: bool = True
) -> str:
    """`df` command producing integer GiB, rounded down before any admission comparison."""
    columns = "avail,target" if include_mount else "avail"
    return (
        f"df -B1 --output={columns} {path_expression} 2>/dev/null | "
        f"tail -n 1 | {free_gib_awk(include_mount=include_mount, suffix=suffix)}"
    )


@dataclass(frozen=True)
class Violation:
    mount: str
    free_gb: int
    floor_gb: int

    def __str__(self) -> str:
        return f"mount {self.mount} free {self.free_gb}GB below floor {self.floor_gb}GB"


def _rpath(path: str) -> str:
    """Shell-quote a remote path, keeping a leading ~/ expandable."""
    if path.startswith("~/"):
        return '"$HOME"/' + shlex.quote(path[2:])
    return shlex.quote(path)


def watched_paths(entry: Mapping[str, Any], block: Mapping[str, Any]) -> list[str]:
    """Paths whose filesystems builds write to: the always-list, exec/cargo dirs, data paths."""
    paths = [str(path) for path in block.get("always", [ROOT])]
    for key in PATH_KEYS:
        if entry.get(key):
            paths.append(str(entry[key]))
    if entry.get("canon_root"):  # cargo home and the shared sccache live under the canon root
        paths.append(f"{entry['canon_root']}/cargo")
    paths.extend(str(path) for path in entry.get("data_paths", []))
    return list(dict.fromkeys(paths))


def root_floor_gb(entry: Mapping[str, Any], block: Mapping[str, Any]) -> int:
    return int(entry.get("root_floor_gb", block.get("root_floor_gb", 0)))


def probe_fragment(paths: Iterable[str]) -> str:
    """Shell printing `disk_mounts=<mount>:<gb>,...` (one pair per distinct mount)."""
    loop = " ".join(_rpath(path) for path in paths)
    free = free_gib_command('"$q"', include_mount=True)
    return (
        f'dm=; for q in {loop}; do while [ ! -e "$q" ] && [ "$q" != / ]; do '
        'q=$(dirname "$q"); done; '
        f"r=$({free}); "
        'case ",$dm," in *",${r%%:*}:"*) ;; *) [ -n "$r" ] && dm="$dm,$r";; esac; done; '
        f'echo "{KEY}=${{dm#,}}"; '
    )


def parse(value: str) -> dict[str, int]:
    """`/:1,/workspace:85` -> {"/": 1, "/workspace": 85}; malformed pairs are dropped."""
    mounts: dict[str, int] = {}
    for pair in (value or "").split(","):
        match = re.fullmatch(r"(/\S*):(\d+)", pair.strip())
        if match:
            mount, free = match.group(1), int(match.group(2))
            mounts[mount] = min(free, mounts.get(mount, free))
    return mounts


def lowest(mounts: Mapping[str, int]) -> tuple[str, int] | None:
    """The mount with the least free space, or None when none was measured."""
    return min(mounts.items(), key=lambda item: (item[1], item[0])) if mounts else None


def violations(mounts: Mapping[str, int], *, floor_gb: int, root_floor: int) -> list[Violation]:
    """Mounts under their floor (root uses `root_floor`, every other mount `floor_gb`)."""
    out = []
    for mount, free in sorted(mounts.items()):
        floor = root_floor if mount == ROOT else floor_gb
        if floor and free < floor:
            out.append(Violation(mount, free, floor))
    return out


def _entry(name: str, path: Path | None) -> tuple[dict[str, Any], dict[str, Any]]:
    registry = host_registry.load(path)
    return registry.host(name) or {}, registry.section(KEY)


def floors_for(name: str, path: Path | None = None) -> tuple[int, int]:
    """(disk_floor_gb, root_floor_gb) of a registry host; (0, root default) when unregistered."""
    entry, block = _entry(name, path)
    return int(entry.get("disk_floor_gb", 0)), root_floor_gb(entry, block)


def paths_for(name: str, path: Path | None = None) -> list[str]:
    entry, block = _entry(name, path)
    return watched_paths(entry, block)
