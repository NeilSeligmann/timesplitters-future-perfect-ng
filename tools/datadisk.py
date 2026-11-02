# SPDX-License-Identifier: GPL-3.0-or-later
"""Data-disk resolver and project disk budget (T3745, docs/t3745-disk-safety.md).

One place that answers "may the project write here, and may it write now":

- check_path(): a data, temporary or cache path must not be on the root filesystem when a separate
  data disk exists (st_dev of the path's nearest existing ancestor vs "/" and vs the data disk
  anchor, the main checkout). The C twin is root_fs_refused() in src/host/t2_capture.c.
- data_tmp() / tmp_env(): the project TMPDIR on the data disk (`<main>/tmp/tmpdir`).
- check(): the budget guard every big writer calls before it starts (replay, capture, prebuilt,
  build, rust_trace, verify scratch). Refuses under the root or data free floor, or when the
  project's data areas are over `project_max_gb`. Limits: tools/data/disk_budget.json.

    python3 -m tools.datadisk check --kind build [--path P] [--need-gb N]
    python3 -m tools.datadisk tmpdir            # prints (and creates) the data-disk TMPDIR
    python3 -m tools.datadisk status [--json]
    python3 -m tools.datadisk measure           # refresh the cached project-area usage
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUDGET_PATH = ROOT / "tools" / "data" / "disk_budget.json"
USAGE_NAME = "disk-budget/usage.json"
GB = 1024**3


class DiskRefused(RuntimeError):
    """A write the disk rules forbid: the message says which rule and how to tune it."""


@dataclass(frozen=True)
class Budget:
    project_max_gb: float
    project_areas: tuple[str, ...]
    usage_max_age_hours: float
    root_min_free_gb: float
    data_min_free_gb: float
    capture_run_max_gb: float
    capture_segment_mb: float
    capture_compact_interval_s: float


def remote_disk_floor(host: str, registry: Path | None = None) -> float:
    """`disk_floor_gb` of a replay worker in tools/data/remote_hosts.json (T3740)."""
    from tools import host_registry

    loaded = host_registry.load(registry)  # T3754: the one remote_hosts.json loader
    entry = loaded.host(host)
    if entry is None or "disk_floor_gb" not in entry:
        raise ValueError(f"TSFP_DISK_HOST={host!r}: no disk_floor_gb in {loaded.path}")
    return float(entry["disk_floor_gb"])


def load_budget(path: Path | None = None) -> Budget:
    """The tracked limits; TSFP_DISK_BUDGET names another file (tests)."""
    source = Path(path or os.environ.get("TSFP_DISK_BUDGET") or BUDGET_PATH)
    raw = json.loads(source.read_text())
    profile = os.environ.get("TSFP_DISK_PROFILE", "")
    if profile:  # T3740: e.g. remote_worker (tools.wt.remote_replay sets it on workers)
        overrides = raw.get("profiles", {}).get(profile)
        if not isinstance(overrides, dict):
            raise ValueError(f"TSFP_DISK_PROFILE={profile}: no such profile in {source}")
        raw = {**raw, **overrides}
        host = os.environ.get("TSFP_DISK_HOST", "")
        if profile == "remote_worker":
            raw["data_min_free_gb"] = remote_disk_floor(host)
    return Budget(
        project_max_gb=float(raw["project_max_gb"]),
        project_areas=tuple(str(area) for area in raw["project_areas"]),
        usage_max_age_hours=float(raw["usage_max_age_hours"]),
        root_min_free_gb=float(raw["root_min_free_gb"]),
        data_min_free_gb=float(raw["data_min_free_gb"]),
        capture_run_max_gb=float(raw["capture_run_max_gb"]),
        capture_segment_mb=float(raw["capture_segment_mb"]),
        capture_compact_interval_s=float(raw["capture_compact_interval_s"]),
    )


def main_root() -> Path:
    """The main checkout (the data disk anchor); linked worktrees share it. Read from the `.git`
    file, no subprocess (callers run under patched subprocess in tests)."""
    dot_git = ROOT / ".git"
    try:
        if dot_git.is_file():  # linked worktree: "gitdir: <main>/.git/worktrees/<name>"
            text = dot_git.read_text().strip()
            if text.startswith("gitdir:"):
                gitdir = (ROOT / text.split(":", 1)[1].strip()).resolve()
                commondir = gitdir / "commondir"
                common = (
                    (gitdir / commondir.read_text().strip()).resolve()
                    if commondir.is_file()
                    else gitdir
                )
                return common.parent
    except OSError:
        pass
    from tools.wt.source_identity import main_override

    main = main_override()  # T3760: a git-less remote exec tree
    if main is not None and not dot_git.exists():
        return main
    return ROOT


def anchor() -> Path:
    override = os.environ.get("TSFP_DATADISK_ANCHOR")
    return Path(override) if override else main_root()


def root_ref() -> Path:
    return Path(os.environ.get("TSFP_DATADISK_ROOT_REF", "/"))


def existing_ancestor(path: Path) -> Path:
    path = Path(os.path.abspath(path))
    while not path.exists() and path != path.parent:
        path = path.parent
    return path


def device(path: Path) -> int:
    return os.stat(existing_ancestor(path)).st_dev


def check_path(
    path: str | Path, *, what: str = "data", dev: Callable[[Path], int] = device
) -> Path:
    """Return the absolute path, or raise DiskRefused when it would land on the root disk."""
    target = Path(os.path.abspath(Path(path).expanduser()))
    path_dev, root_dev = dev(target), dev(root_ref())
    if path_dev != root_dev:
        return target
    data = anchor()
    try:
        data_dev = dev(data)
    except OSError as exc:
        raise DiskRefused(
            f"{what} path {target} is on the root filesystem and the data disk anchor {data} is "
            f"unreadable ({exc}); set TSFP_DATADISK_ANCHOR (T3745)"
        ) from exc
    if data_dev == root_dev:
        return target  # one-disk host: no separate data disk, the free floors guard it
    raise DiskRefused(
        f"{what} path {target} is on the ROOT filesystem; project data goes to the data disk "
        f"({data}). Use a path under {main_root() / 'tmp'} (docs/t3745-disk-safety.md, T3745)"
    )


def resolve(path: str | Path, *, what: str = "data") -> Path:
    """tools.datadir routing (relative tmp/... to the shared data dir) plus the root-disk check."""
    from tools.datadir import output_path

    return check_path(output_path(Path(path)), what=what)


def data_tmp() -> Path:
    """The project TMPDIR on the data disk, created."""
    from tools.datadir import data_dir

    path = check_path(data_dir("tmpdir"), what="TMPDIR")
    path.mkdir(parents=True, exist_ok=True)
    return path


def tmp_env(env: dict[str, str] | None = None) -> dict[str, str]:
    """`env` (default os.environ) with TMPDIR/TMP/TEMP on the data disk."""
    out = dict(os.environ if env is None else env)
    current = out.get("TMPDIR")
    try:
        if current and check_path(current, what="TMPDIR"):
            return out
    except DiskRefused:
        pass
    tmp = str(data_tmp())
    # GIT_CEILING_DIRECTORIES: the data-disk tmp lies inside the main checkout; git must not
    # discover that repo from a temp dir
    out.update(TMPDIR=tmp, TMP=tmp, TEMP=tmp, GIT_CEILING_DIRECTORIES=tmp)
    return out


# ---- budget -----------------------------------------------------------------------------------


def free_gb(path: Path) -> float:
    return shutil.disk_usage(existing_ancestor(path)).free / GB


def used_gb(path: Path) -> float:
    return shutil.disk_usage(existing_ancestor(path)).used / GB


def usage_file(main: Path) -> Path:
    return main / "tmp" / USAGE_NAME


def measure_areas(main: Path, areas: Iterable[str], timeout: int = 1800) -> float:
    """Allocated GB of the project areas (`du -sx`, one filesystem, symlinks not followed)."""
    paths = [str(main / area) for area in areas if (main / area).exists()]
    if not paths:
        return 0.0
    done = subprocess.run(
        ["nice", "-n", "19", "du", "-sxc", "--block-size=1", *paths],
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    lines = [line for line in done.stdout.splitlines() if line.endswith("\ttotal")]
    if not lines:
        raise OSError(f"du gave no total: {done.stderr.strip()[-300:]}")
    return int(lines[-1].split("\t")[0]) / GB


def refresh_usage(main: Path, budget: Budget | None = None, now: float | None = None) -> dict:
    budget = budget or load_budget()
    record = {
        "ts": time.time() if now is None else now,
        "areas": list(budget.project_areas),
        "area_gb": round(measure_areas(main, budget.project_areas), 3),
        "data_used_gb": round(used_gb(main), 3),
    }
    path = usage_file(main)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(path.name + ".tmp")
    temp.write_text(json.dumps(record, sort_keys=True) + "\n")
    temp.replace(path)
    return record


def project_gb(
    main: Path, budget: Budget, *, now: float | None = None, data_used: float | None = None
) -> tuple[float, str]:
    """Project-area GB: the last du plus all data-disk growth since it (an upper bound of the
    areas' growth). (-1, "unmeasured") before the first du: hygiene measures every
    usage_max_age_hours and the HEALTH detector flags a missing or stale measurement."""
    now = time.time() if now is None else now
    try:
        cached = json.loads(usage_file(main).read_text())
        if cached.get("areas") != list(budget.project_areas):
            return -1.0, "unmeasured (areas changed)"
        used_now = used_gb(main) if data_used is None else data_used
        age_h = (now - float(cached["ts"])) / 3600
        growth = max(0.0, used_now - float(cached["data_used_gb"]))
        return float(cached["area_gb"]) + growth, f"du {age_h:.1f} h old + {growth:.0f} GB growth"
    except (OSError, ValueError, KeyError, TypeError):
        return -1.0, "unmeasured"


@dataclass(frozen=True)
class Status:
    root_free_gb: float
    data_free_gb: float
    project_gb: float
    project_source: str
    problems: tuple[str, ...]


def status(
    main: Path | None = None,
    budget: Budget | None = None,
    *,
    need_gb: float = 0.0,
    free: Callable[[Path], float] = free_gb,
    project: Callable[[Path, Budget], tuple[float, str]] | None = None,
) -> Status:
    main = main or main_root()
    budget = budget or load_budget()
    root_free, data_free = free(root_ref()), free(main)
    proj, source = (project or project_gb)(main, budget)
    problems = []
    if root_free < budget.root_min_free_gb:
        problems.append(
            f"root filesystem has {root_free:.0f} GB free < root_min_free_gb "
            f"{budget.root_min_free_gb:g}"
        )
    if data_free - need_gb < budget.data_min_free_gb:
        problems.append(
            f"data disk has {data_free:.0f} GB free (need {need_gb:g}) < data_min_free_gb "
            f"{budget.data_min_free_gb:g}"
        )
    if proj + need_gb > budget.project_max_gb:
        problems.append(
            f"project areas {'+'.join(budget.project_areas)} use {proj:.0f} GB ({source}) "
            f"+ {need_gb:g} > project_max_gb {budget.project_max_gb:g}"
        )
    return Status(root_free, data_free, proj, source, tuple(problems))


def check(
    kind: str,
    *,
    paths: Iterable[str | Path] = (),
    need_gb: float = 0.0,
    main: Path | None = None,
    budget: Budget | None = None,
    free: Callable[[Path], float] = free_gb,
    project: Callable[[Path, Budget], tuple[float, str]] | None = None,
    dev: Callable[[Path], int] = device,
) -> Status:
    """The guard a big writer calls before it starts. Raises DiskRefused with the reason."""
    for path in paths:
        check_path(path, what=kind, dev=dev)
    found = status(main, budget, need_gb=need_gb, free=free, project=project)
    if found.problems:
        raise DiskRefused(
            f"{kind} refused by the disk budget (T3745): " + "; ".join(found.problems) + ". "
            "Free space (python3 -m tools.wt gc --prune --prune-caches) or tune "
            "tools/data/disk_budget.json (docs/t3745-disk-safety.md)"
        )
    return found


def no_core_dumps() -> None:
    """Soft RLIMIT_CORE 0 for this process and its children (T3745, host-core-dumps-on-root): the
    kernel core_pattern pipes cores to the host's systemd-coredump on the root disk. The C host
    applies the TSFP_CORE_DUMPS=1 opt-in itself (src/host/host_core_policy.h)."""
    import resource

    _soft, hard = resource.getrlimit(resource.RLIMIT_CORE)
    resource.setrlimit(resource.RLIMIT_CORE, (0, hard))


def capture_env(budget: Budget | None = None) -> dict[str, str]:
    """The capture runtime's own limits (src/host/t2_capture.c reads these)."""
    budget = budget or load_budget()
    return {
        "TSFP_T2_CAPTURE_MAX_BYTES": str(int(budget.capture_run_max_gb * GB)),
        "TSFP_T2_CAPTURE_MIN_FREE_GB": str(int(budget.data_min_free_gb)),
        "TSFP_T2_CAPTURE_SEGMENT_BYTES": str(int(budget.capture_segment_mb * 1024 * 1024)),
        "TSFP_DATADISK_ANCHOR": str(anchor()),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.datadisk", description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("check", help="refuse (exit 3) when a big write may not start")
    p.add_argument("--kind", required=True)
    p.add_argument("--path", action="append", default=[])
    p.add_argument("--need-gb", type=float, default=0.0)
    sub.add_parser("tmpdir", help="print the data-disk TMPDIR")
    p = sub.add_parser("status")
    p.add_argument("--json", action="store_true")
    sub.add_parser("measure", help="refresh the cached project-area du")
    args = parser.parse_args(argv)
    try:
        if args.cmd == "check":
            check(args.kind, paths=args.path, need_gb=args.need_gb)
        elif args.cmd == "tmpdir":
            print(data_tmp())
        elif args.cmd == "status":
            found = status()
            if args.json:
                print(json.dumps(found.__dict__, sort_keys=True))
            else:
                print(
                    f"root free {found.root_free_gb:.0f} GB, "
                    f"data free {found.data_free_gb:.0f} GB, "
                    f"project {found.project_gb:.0f} GB ({found.project_source})"
                )
                for problem in found.problems:
                    print(f"OVER: {problem}")
            return 3 if found.problems else 0
        elif args.cmd == "measure":
            print(json.dumps(refresh_usage(main_root()), sort_keys=True))
    except DiskRefused as exc:
        print(f"datadisk: {exc}", file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
