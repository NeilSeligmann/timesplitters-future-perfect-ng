# SPDX-License-Identifier: GPL-3.0-or-later
# ruff: noqa: E501
"""T1154: one command for a PRIVATE lift, a host built from it, and a play or replay on that host.

The canonical `generated/lifted` is owned by T659/T822 and is never read as a base or written here.
Everything lives under the gitignored `tmp/private-host/`:

    python -m tools.private_host build            # lift (cached), gitignored xdk files, host
    python -m tools.private_host play -- --disc ISO --window --interactive --gpu-live ...
    python -m tools.private_host replay --repeats 3 --input tmp/recorded-input-spam-a

`build` lifts the retail XBE with the default seeds (`tools/config/lift_thread_entries.json`, bundled
for the bound XBE) from the CURRENT source, into `tmp/private-host/lift-<key>/gen`. `<key>` is a hash of the
lifter source, the vendored lifter, the seeds, the flag bridge, the manual list, the XBE and the extra
seeds, so an unchanged input set skips the slow step. The host is built with the container compiler
against the container glibc into `tmp/private-host/build-<key>`. `play` passes every other argument to
`python -m tools.play` with `--host <private host>`; `replay` is the headless xvfb form.

Fresh worktree: the gitignored inputs (XBE, manual list, xdk files, disc, owner files, recorded input) are
found in the main checkout (`git rev-parse --git-common-dir`), or via --main-root / TSFP_MAIN_ROOT,
and are symlinked or copied, never printed or committed. Owner files: --files-dir or TSFP_FILES_DIR.
"""

import argparse
import fcntl
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from tools import hostcaps, owner_files
from tools.datadir import write_recipe
from tools.wt import shared_artifacts, source_identity

ROOT = Path(__file__).resolve().parent.parent
WORK = Path("tmp/private-host")
SURFACE_DIR = Path("src/xbox")
SURFACE_FILES = ("xdk_abi.inc", "xdk_surface.c", "xdk_surface.h")
DEFAULT_SEEDS = Path("tools/config/lift_thread_entries.json")
FLAG_BRIDGE = Path("tools/config/flag_bridge.json")
XBE_NAME = Path("build/default.xbe")
LIFTER_DIRS = (Path("tools/lift"), Path("third_party/xboxrecomp"))
DEFAULT_INPUT = Path("tmp/recorded-input-spam-a")
KEY_LENGTH = 16
DEFAULT_LIFTED_OPT = "O2"  # T1289: was O0
LIFT_TIMEOUT = 3600
BUILD_TIMEOUT = 3600
GEN_TIMEOUT = 600
#: T1461: measured peak RSS of one lifted-chunk compile per optimisation level, MB (docs/private-host.md).
PEAK_MB_PER_JOB = {"O0": 350, "O1": 400, "O2": 450}
HEADROOM_MB = 2048  # left for the OS, the linker and the rest of the session
FAILED_CONTEXT = 60
OOM_MARKERS = ("Killed", "out of memory", "Cannot allocate memory", "std::bad_alloc")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def tree_files(directory: Path) -> list[Path]:
    """Source files of a directory, caches excluded, sorted for a stable key."""
    return sorted(
        path
        for path in directory.rglob("*")
        if path.is_file() and "__pycache__" not in path.parts and path.suffix != ".pyc"
    )


STOP_BOUNDARIES = (
    Path("tools/config/stream_stop_boundaries.json"),
    Path("tools/config/buffer_stop_boundaries.json"),
    Path("tools/config/movie_synch_boundaries.json"),
)
#: Generators whose output ends up in the lift tree, hashed so a change to one re-lifts.
KEY_FILES = (
    DEFAULT_SEEDS,
    FLAG_BRIDGE,
    *STOP_BOUNDARIES,
    Path("tools/data/shader_original_entries.json"),
    Path("tools/gen_xdk_manual_list.py"),
    Path("tools/gen_xdk_original_bodies.py"),
    Path("tools/gen_xmv_original_bodies.py"),
    Path("tools/install_shader_original.py"),
    Path("tools/retained_original.py"),
    Path("tools/xdk_abi.py"),
    Path("tools/gen_d3d8_surface.py"),
    *(SURFACE_DIR / name for name in SURFACE_FILES),
)


def key_from_digests(
    lifter: dict[str, str],
    key_files: dict[str, str | None],
    xbe_digest: str,
    extra_digests: list[str] | tuple[str, ...] = (),
    fpo_leaf_ebp_entry: str = "seh",
) -> tuple[str, list[str]]:
    """The lift key from per-file SHA-256 digests (the ONE place that defines the key format).

    `lifter` maps repo-relative POSIX paths under LIFTER_DIRS to digests, `key_files` maps each
    KEY_FILES path to its digest or None when absent. Used by `cache_key` (files on disk) and by
    `tools.wt.lift_ship` (blobs of a commit, so the owner PC knows the key a remote build needs)."""
    lines: list[str] = []
    for directory in LIFTER_DIRS:
        prefix = directory.as_posix() + "/"
        inside = sorted(Path(name) for name in lifter if name.startswith(prefix))
        lines += [f"{name.as_posix()} {lifter[name.as_posix()]}" for name in inside]
    for name in KEY_FILES:
        digest = key_files.get(name.as_posix())
        lines.append(f"{name.as_posix()} {digest if digest is not None else 'absent'}")
    lines.append(f"xbe {xbe_digest}")
    lines += [f"extra-seed {digest}" for digest in extra_digests]
    if fpo_leaf_ebp_entry != "seh":  # T1860: absent for the default, so existing keys are unchanged
        lines.append(f"fpo-leaf-ebp-entry {fpo_leaf_ebp_entry}")
    digest = hashlib.sha256("\n".join(lines).encode()).hexdigest()
    return digest[:KEY_LENGTH], lines


def cache_key(
    root: Path,
    xbe: Path,
    extra_seeds: list[Path],
    fpo_leaf_ebp_entry: str = "seh",
    *,
    xbe_digest: str | None = None,
) -> tuple[str, list[str]]:
    """Hash of every input of the private lift, with the per-input lines it was built from."""
    lifter = {
        path.relative_to(root).as_posix(): sha256_file(path)
        for directory in LIFTER_DIRS
        for path in tree_files(root / directory)
    }
    key_files = {
        name.as_posix(): sha256_file(root / name) if (root / name).is_file() else None
        for name in KEY_FILES
    }
    return key_from_digests(
        lifter,
        key_files,
        xbe_digest if xbe_digest is not None else sha256_file(xbe),
        [sha256_file(seed) for seed in extra_seeds],
        fpo_leaf_ebp_entry,
    )


def recorded_xbe_digest(lift_dirs: list[Path]) -> str | None:
    """The retail XBE digest a complete lift recorded in its KEY.txt (first usable dir wins)."""
    for candidate in lift_dirs:
        record = candidate / "KEY.txt"
        if not complete_lift(candidate) or not record.is_file():
            continue
        for line in record.read_text().splitlines():
            if line.startswith("xbe "):
                digest = line.removeprefix("xbe ")
                if len(digest) == 64 and all(c in "0123456789abcdef" for c in digest):
                    return digest
    return None


def newest_lifts(root: Path) -> list[Path]:
    """Lift dirs newest first; a dangling bootstrap link (a pruned host lift) is skipped."""
    found: list[tuple[float, Path]] = []
    for path in (root / WORK).glob("lift-*"):
        try:
            found.append((path.stat().st_mtime, path))
        except OSError:
            continue
    return [path for _, path in sorted(found, key=lambda item: item[0], reverse=True)]


def owner_lift_key(lifted: Path | None = None) -> str:
    """Current default lift key; a complete lift can supply the absent retail XBE hash.

    A git-less remote exec tree has no XBE (owner file): there the lift the owner PC shipped
    for this tree (`tools.wt.lift_ship`) supplies it."""
    main = main_checkout(ROOT, None)
    xbe = locate(ROOT, main, XBE_NAME)
    if xbe is not None:
        return cache_key(ROOT, xbe, [])[0]
    candidates = [lifted.parent] if lifted is not None else []
    digest = recorded_xbe_digest(candidates + newest_lifts(ROOT))
    if digest is not None:
        return cache_key(ROOT, ROOT / XBE_NAME, [], xbe_digest=digest)[0]
    return "unavailable"


def main_checkout(root: Path, override: Path | None) -> Path:
    """The main checkout holding the gitignored inputs (this one when not a worktree).

    Never runs git in a git-less exec tree (rule 47, T3760): there TSFP_MAIN_ROOT names the
    host dir standing in for the main checkout, else the exec tree is its own."""
    if override is not None:
        return override.resolve()
    env = os.environ.get("TSFP_MAIN_ROOT")
    if env:
        return Path(env).resolve()
    if source_identity.exec_root(root) is not None:
        return root
    probe = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if probe.returncode == 0 and probe.stdout.strip():
        return Path(probe.stdout.strip()).parent
    return root


def locate(root: Path, main: Path, relative: Path) -> Path | None:
    for base in (root, main):
        candidate = base / relative
        if candidate.exists():
            return candidate
    return None


def link_into_worktree(root: Path, main: Path, relative: Path) -> Path | None:
    """Make a gitignored input of the main checkout visible in this tree by symlink."""
    here = root / relative
    if here.exists():
        return here
    source = main / relative
    if root == main or not source.exists():
        return None
    here.parent.mkdir(parents=True, exist_ok=True)
    here.symlink_to(source)
    return here


def find_python(root: Path, main: Path, override: str | None) -> str:
    """An interpreter with capstone: --python, TSFP_PYTHON, this tree's .venv, the main .venv, else this one."""
    candidates = [override, os.environ.get("TSFP_PYTHON")]
    candidates += [str(base / ".venv/bin/python") for base in (root, main)]
    for candidate in candidates:
        if candidate and Path(candidate).exists():
            return candidate
    return sys.executable


def run_step(
    argv: list[str],
    *,
    label: str,
    timeout: int,
    cwd: Path,
    log: Path | None = None,
    ok_codes: tuple[int, ...] = (0,),
) -> float:
    """One build step under a timeout, output to `log` when given. Returns the seconds taken."""
    print(f"[{label}] {' '.join(argv)}", flush=True)
    started = time.monotonic()
    handle = log.open("w") if log else None
    try:
        done = subprocess.run(
            argv,
            cwd=cwd,
            timeout=timeout,
            check=False,
            stdout=handle or None,
            stderr=subprocess.STDOUT if handle else None,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit(f"{label}: timed out after {timeout} s") from None
    finally:
        if handle:
            handle.close()
    if done.returncode not in ok_codes:
        tail = log.read_text().splitlines()[-15:] if log else []
        raise SystemExit(
            f"{label}: exit {done.returncode}" + ("\n" + "\n".join(tail) if tail else "")
        )
    return time.monotonic() - started


def mem_available_mb(meminfo: Path = Path("/proc/meminfo")) -> int | None:
    """MemAvailable in MB (not MemTotal: a busy desktop has far less free than it has installed)."""
    available = hostcaps.compute_mem_available_bytes(meminfo=meminfo)  # T3844: cgroup cap
    return None if available is None else available // 2**20


def choose_jobs(
    cpus: int, available_mb: int | None, lifted_opt: str, requested: int | None = None
) -> int:
    """Parallel compile jobs: --jobs when given, else min(cpus, (available - headroom) / peak per job), at least 1."""
    if requested is not None:
        return max(1, requested)
    if available_mb is None:
        return max(1, cpus)
    by_memory = (available_mb - HEADROOM_MB) // PEAK_MB_PER_JOB[lifted_opt]
    return max(1, min(cpus, by_memory))


def failure_report(text: str, context: int = FAILED_CONTEXT) -> str:
    """The first `FAILED:` block of ninja output with the compiler lines under it, else the last lines."""
    lines = text.splitlines()
    starts = [i for i, line in enumerate(lines) if line.startswith("FAILED:")]
    if not starts:
        return "\n".join(lines[-context:])
    end = starts[1] if len(starts) > 1 else len(lines)
    block = lines[starts[0] : end][:context]
    errors = [line for line in lines if re.search(r"\berror\b|\bfatal error\b", line)]
    extra = [line for line in errors if line not in block][:20]
    more = f"\n({len(starts) - 1} further FAILED: blocks in the log)" if len(starts) > 1 else ""
    return "\n".join(block + (["--- other error lines ---", *extra] if extra else [])) + more


def looks_oom(text: str, returncode: int) -> bool:
    """A compiler killed by the OOM killer: exit 137 / SIGKILL, or a Killed / out of memory line."""
    if returncode in (137, -9):
        return True
    return any(marker in text for marker in OOM_MARKERS)


OOM_REMEDY = (
    "The compiler looks OOM-killed. Check `free -h` and `dmesg | tail` (or `journalctl -k | grep -i oom`). Retry with "
    "fewer jobs and a cheaper lifted optimisation: `python -m tools.private_host --lifted-opt O1 --jobs 4 build` "
    "(or `--lifted-opt O0`). Ninja resumes, finished objects are kept."
)


def ninja_build(build_dir: Path, args: argparse.Namespace, log: Path) -> float:
    """`ninja tsfp_host` with a RAM-aware -j, the real compiler error printed on failure."""
    jobs = choose_jobs(hostcaps.effective_cpus(), mem_available_mb(), args.lifted_opt, args.jobs)
    argv = ["cmake", "--build", str(build_dir), "--target", "tsfp_host", "--", "-j", str(jobs)]
    if args.keep_going:
        argv += ["-k", "0"]
    print(
        f"[ninja] {' '.join(argv)} (jobs {jobs}, MemAvailable {mem_available_mb()} MB)", flush=True
    )
    started = time.monotonic()
    try:
        with log.open("w") as handle:
            done = subprocess.run(
                argv,
                cwd=ROOT,
                timeout=BUILD_TIMEOUT,
                check=False,
                stdout=handle,
                stderr=subprocess.STDOUT,
            )
    except subprocess.TimeoutExpired:
        raise SystemExit(f"ninja: timed out after {BUILD_TIMEOUT} s, log {log}") from None
    if done.returncode != 0:
        text = log.read_text(errors="replace")
        hint = f"\n{OOM_REMEDY}" if looks_oom(text, done.returncode) else ""
        raise SystemExit(
            f"ninja: exit {done.returncode}, full log {log}\n{failure_report(text)}{hint}\n"
            f"Re-run for the details: `ninja -C {build_dir} -v tsfp_host 2>&1 | grep -B3 -A25 FAILED:`"
        )
    return time.monotonic() - started


def ensure_surface(root: Path, main: Path, python: str, xbe: Path, xtlid: Path | None) -> list[str]:
    """Create the gitignored xdk_abi.inc and xdk_surface.c/.h when missing. Returns what was done."""
    missing = [name for name in SURFACE_FILES if not (root / SURFACE_DIR / name).is_file()]
    if not missing:
        return []
    actions: list[str] = []
    xtlid = xtlid or locate_xtlid(root, main)
    if xtlid is not None:
        stage = root / WORK / "surface-stage"
        shutil.rmtree(stage, ignore_errors=True)
        stage.mkdir(parents=True)
        run_step(
            [
                python,
                "-m",
                "tools.gen_d3d8_surface",
                str(xbe),
                "--xtlid",
                str(xtlid),
                "--out-dir",
                str(stage),
            ],
            label="gen-surface",
            timeout=GEN_TIMEOUT,
            cwd=root,
            log=stage / "gen.log",
        )
        run_step(
            [
                python,
                "-m",
                "tools.xdk_abi",
                "--xbe",
                str(xbe),
                "--surface",
                str(stage / "xdk_surface.c"),
                "--emit-c",
                str(stage / "xdk_abi.inc"),
            ],
            label="xdk-abi",
            timeout=GEN_TIMEOUT,
            cwd=root,
            log=stage / "abi.log",
        )
        pair_missing = any(name in missing for name in SURFACE_FILES[1:])
        for name in SURFACE_FILES if pair_missing else missing:
            shutil.copy2(stage / name, root / SURFACE_DIR / name)
            actions.append(f"generated {name}")
        return actions
    for name in missing:
        source = main / SURFACE_DIR / name
        if root == main or not source.is_file():
            raise SystemExit(
                f"{name} is missing and no xtlid.xml was found. Pass --xtlid <xtlid.xml> or run "
                f"`python -m tools.gen_d3d8_surface {xbe} --xtlid <xtlid.xml>` and "
                f"`python -m tools.xdk_abi --xbe {xbe} --emit-c src/xbox/xdk_abi.inc`"
            )
        shutil.copy2(source, root / SURFACE_DIR / name)
        actions.append(f"copied {name} from the main checkout")
    return actions


def locate_xtlid(root: Path, main: Path) -> Path | None:
    env = os.environ.get("TSFP_XTLID")
    if env and Path(env).is_file():
        return Path(env)
    return locate(root, main, Path("tmp/analysis_scratch/xtlid.xml"))


def host_paths(root: Path, key: str) -> tuple[Path, Path, Path]:
    lift = root / WORK / f"lift-{key}"
    build = root / WORK / f"build-{key}"
    return lift, build, build / "tsfp_host"


def lift_artifacts_complete(lift_dir: Path) -> bool:
    """The summary and key record are written before a directory can be published."""
    return (lift_dir / "summary/summary.json").is_file() and (lift_dir / "KEY.txt").is_file()


def complete_lift(lift_dir: Path, key: str | None = None) -> bool:
    """A published lift is complete only after its summary and expected key exist."""
    if key is not None and lift_dir.name != f"lift-{key}":
        return False
    return (
        lift_dir.name.startswith("lift-")
        and ".tmp-" not in lift_dir.name
        and lift_artifacts_complete(lift_dir)
    )


def valid_lift_stage(stage: Path, key: str, lines: list[str]) -> bool:
    """Validate a fetched lift under its temporary name before atomic publication."""
    return (
        stage.name.startswith(f"lift-{key}.tmp-")
        and lift_artifacts_complete(stage)
        and (stage / "KEY.txt").read_text() == "\n".join(lines) + "\n"
    )


def publish_lift(stage: Path, destination: Path, key: str) -> bool:
    """Atomically publish a completed sibling lift; never remove a complete lift."""
    if not lift_artifacts_complete(stage):
        raise RuntimeError(f"refusing to publish incomplete lift staging directory: {stage}")
    lock_path = destination.with_name(f".{destination.name}.publish.lock")
    with lock_path.open("a") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        if complete_lift(destination, key):
            shutil.rmtree(stage)
            return False
        if destination.is_symlink():
            # T3737: a stale link (a dangling or incomplete lift of another checkout) makes the
            # rename fail with ENOTDIR and rmtree refuse; drop the link, never its target.
            destination.unlink()
        elif destination.exists():
            # Old interrupted lifts were written in place. Only discard an incomplete one.
            shutil.rmtree(destination)
        os.rename(stage, destination)
        return True


def generate_lift(
    args: argparse.Namespace,
    root: Path,
    python: str,
    xbe: Path,
    extra: list[Path],
    key: str,
    lift_dir: Path,
    key_lines: list[str],
    recipe_command: list[str],
    recipe_inputs: list[Path],
) -> dict[str, float]:
    """Build a lift and its postprocessors entirely in a private staging directory."""
    lift_dir.mkdir()
    timings: dict[str, float] = {}
    manual, trampolines = lift_dir / "xdk-manual.json", lift_dir / "recomp_xdk_manual.c"
    gen_manual = [
        python,
        "-m",
        "tools.gen_xdk_manual_list",
        "--surface",
        str(SURFACE_DIR / "xdk_surface.c"),
        "--json",
        str(manual),
        "--trampolines",
        str(trampolines),
    ]
    for boundary in STOP_BOUNDARIES:
        gen_manual += ["--stop-boundaries", str(boundary)]
    timings["manual-list"] = run_step(
        gen_manual,
        label="manual-list",
        timeout=GEN_TIMEOUT,
        cwd=root,
        log=lift_dir / "manual-list.log",
    )
    argv = [
        python,
        "-m",
        "tools.lift",
        "run",
        str(xbe),
        "--out-dir",
        str(lift_dir),
        "--flag-bridge",
        str(FLAG_BRIDGE),
        "--python",
        python,
        "--manual-functions",
        str(manual),
    ]
    for seed in extra:
        argv += ["--seed-functions", str(seed)]
    if args.fpo_leaf_ebp_entry != "seh":
        argv += ["--fpo-leaf-ebp-entry", args.fpo_leaf_ebp_entry]
    timings["lift"] = run_step(
        argv,
        label="lift",
        timeout=LIFT_TIMEOUT,
        cwd=root,
        log=root / WORK / f"lift-{key}.log",
        ok_codes=(0, 1) if args.allow_failed_functions else (0,),
    )
    shutil.copy2(trampolines, lift_dir / "gen/recomp_xdk_manual.c")
    timings["shader-original"] = run_step(
        [
            python,
            "-m",
            "tools.install_shader_original",
            "--xbe",
            str(xbe),
            "--lifted-dir",
            str(lift_dir),
            "--install",
        ],
        label="shader-original",
        timeout=GEN_TIMEOUT,
        cwd=root,
        log=lift_dir / "shader-original.log",
    )
    timings["xmv-original"] = run_step(
        [
            python,
            "-m",
            "tools.gen_xmv_original_bodies",
            "--xbe",
            str(xbe),
            "--lifted-dir",
            str(lift_dir),
            "--install",
        ],
        label="xmv-original",
        timeout=GEN_TIMEOUT,
        cwd=root,
        log=lift_dir / "xmv-original.log",
    )
    (lift_dir / "KEY.txt").write_text("\n".join(key_lines) + "\n")
    write_recipe(
        lift_dir,
        command=[*recipe_command, "--lift-only", "--force-lift", "build"],
        inputs=recipe_inputs,
        wall_seconds=sum(timings.values()),
        cwd=root,
    )
    return timings


def build(args: argparse.Namespace) -> dict[str, str]:
    root = ROOT
    main = main_checkout(root, args.main_root)
    python = find_python(root, main, args.python)
    xbe = link_into_worktree(root, main, XBE_NAME) if args.xbe is None else args.xbe.resolve()
    if xbe is None:
        raise SystemExit(f"no {XBE_NAME} in {root} or {main}: pass --xbe")
    extra = [path.resolve() for path in args.extra_seeds]
    surface = [] if args.dry_run else ensure_surface(root, main, python, xbe, args.xtlid)
    key, lines = cache_key(root, xbe, extra, args.fpo_leaf_ebp_entry)
    lift_dir, build_dir, host = host_paths(root, key)
    (root / WORK).mkdir(parents=True, exist_ok=True)
    recipe_command = [
        python,
        "-m",
        "tools.private_host",
        "--xbe",
        str(xbe),
        "--fpo-leaf-ebp-entry",
        args.fpo_leaf_ebp_entry,
        "--lifted-opt",
        args.lifted_opt,
    ]
    for seed in extra:
        recipe_command += ["--extra-seeds", str(seed)]
    if args.allow_failed_functions:
        recipe_command += ["--allow-failed-functions"]
    recipe_inputs = [
        xbe,
        *extra,
        *[root / name for name in KEY_FILES if (root / name).is_file()],
        *[path for directory in LIFTER_DIRS for path in tree_files(root / directory)],
    ]
    timings: dict[str, float] = {}
    if args.dry_run:
        print(
            f"key {key}\nlift {lift_dir.relative_to(root)}\nhost {host.relative_to(root)}\ninputs {len(lines)}"
        )
        return {"key": key, "host": str(host.relative_to(root))}
    if complete_lift(lift_dir, key):
        print(f"[lift] complete cache hit {key}")
    else:
        stage = lift_dir.with_name(f"{lift_dir.name}.tmp-{os.getpid()}")
        if stage.exists():
            raise SystemExit(f"temporary lift path already exists: {stage}")
        try:
            with tempfile.TemporaryDirectory(prefix=".lift-fetch-", dir=lift_dir.parent) as temp:
                archive = Path(temp) / "lift.tar.gz"
                restored = shared_artifacts.fetch_file("lift", key, archive)
                if restored:
                    restored = shared_artifacts.unpack_tree(archive, stage)
                if restored:
                    if not valid_lift_stage(stage, key, lines):
                        shutil.rmtree(stage, ignore_errors=True)
                        restored = False
                if restored:
                    print(f"[lift] restored {key} from shared artifact store")
                else:
                    shutil.rmtree(stage, ignore_errors=True)
                    timings = generate_lift(
                        args,
                        root,
                        python,
                        xbe,
                        extra,
                        key,
                        stage,
                        lines,
                        recipe_command,
                        recipe_inputs,
                    )
            publish_lift(stage, lift_dir, key)
        except BaseException:
            shutil.rmtree(stage, ignore_errors=True)
            raise
    if not shared_artifacts.already_published("lift", key):
        with tempfile.TemporaryDirectory(prefix=".lift-publish-") as temp:
            archive = Path(temp) / "lift.tar.gz"
            shared_artifacts.pack_tree(lift_dir, archive)
            shared_artifacts.publish_file("lift", key, archive)
    if args.lift_only:
        print(f"private lift: {lift_dir.relative_to(root)}\nkey: {key}")
        return {"key": key, "lifted": str(lift_dir / "gen")}
    configure = [
        "cmake",
        "-S",
        str(root),
        "-B",
        str(build_dir),
        "-G",
        "Ninja",
        "-DCMAKE_C_COMPILER=/usr/bin/clang",
        "-DCMAKE_BUILD_TYPE=Release",
        f"-DTSFP_LIFTED_OPT=-{args.lifted_opt}",
        f"-DTSFP_LIFTED_DIR={lift_dir / 'gen'}",
    ]
    timings["configure"] = run_step(
        configure,
        label="cmake",
        timeout=BUILD_TIMEOUT,
        cwd=root,
        log=root / WORK / f"configure-{key}.log",
    )
    timings["build"] = ninja_build(build_dir, args, root / WORK / f"ninja-{key}.log")
    write_recipe(
        host,
        command=[*recipe_command, "build"],
        inputs=[*recipe_inputs, lift_dir / "KEY.txt", root / "CMakeLists.txt"],
        wall_seconds=sum(timings.values()),
        cwd=root,
    )
    digest = sha256_file(host)
    record = {
        "key": key,
        "host": str(host.relative_to(root)),
        "sha256": digest,
        "xbe_sha256": sha256_file(xbe),
        "surface": surface,
        "seconds": {k: round(v, 1) for k, v in timings.items()},
    }
    (root / WORK / "current.json").write_text(json.dumps(record, indent=2) + "\n")
    print(
        f"private host: {host.relative_to(root)}\nsha256: {digest}\nkey: {key}\nsteps: {record['seconds']}"
    )
    return {"key": key, "host": str(host.relative_to(root)), "sha256": digest}


def current_host(args: argparse.Namespace) -> Path:
    """The built private host, building it first unless --no-build."""
    current = ROOT / WORK / "current.json"
    if not args.no_build:
        info = build(args)
        return ROOT / info["host"]
    if not current.is_file():
        raise SystemExit("no private host yet: run `python -m tools.private_host build`")
    return ROOT / json.loads(current.read_text())["host"]


def private_env(headless: bool) -> dict[str, str]:
    env = dict(os.environ)
    if "XDG_RUNTIME_DIR" not in env:
        runtime = ROOT / WORK / "xdg-runtime"
        runtime.mkdir(parents=True, exist_ok=True)
        runtime.chmod(0o700)
        env["XDG_RUNTIME_DIR"] = str(runtime)
    if headless:
        env["SDL_AUDIODRIVER"] = "dummy"
    return env


def play_argv(python: str, host: Path, passthrough: list[str], headless_display: bool) -> list[str]:
    argv = [python, "-m", "tools.play", *passthrough, "--host", str(host)]
    return ["xvfb-run", "-a", *argv] if headless_display else argv


def play(args: argparse.Namespace, passthrough: list[str]) -> int:
    host = current_host(args)
    main = main_checkout(ROOT, args.main_root)
    python = find_python(ROOT, main, args.python)
    argv = play_argv(python, host, passthrough, args.headless_display)
    if args.dry_run:
        print(" ".join(argv))
        return 0
    return subprocess.run(
        argv, cwd=ROOT, env=private_env(args.headless_display), check=False
    ).returncode


def record_flags(path: Path) -> list[str]:
    """The tools.play flags a record's header demands (it refuses a replay under a different flag set)."""
    wanted = []
    for line in path.read_text(errors="replace").splitlines()[:8]:
        if line.startswith("# flags:"):
            wanted = [
                flag for flag in ("--skip-intro", "--xonline-offline") if flag in line.split()
            ]
    return wanted


def parse_stop(run_dir: Path) -> dict[str, str]:
    """STOP line, detail, census and last guest calls from a run's stop.txt."""
    fields = {"stop": "none", "detail": "", "census": "", "calls": ""}
    path = run_dir / "stop.txt"
    if not path.is_file():
        return fields
    for line in path.read_text().splitlines():
        text = line.strip()
        if line.startswith("STOP"):
            fields["stop"] = re.sub(r"^STOP\s+", "", line)
        elif text.startswith("detail:"):
            fields["detail"] = text[len("detail:") :].strip()
        elif line.startswith("CALL CENSUS"):
            fields["census"] = re.sub(r"^CALL CENSUS\s+", "", line)
        elif line.startswith("LAST GUEST CALLS"):
            fields["calls"] = line.split(":", 1)[1].strip()
    return fields


def replay_argv(
    args: argparse.Namespace, host: Path, disc: Path, record: Path, run_dir: Path, python: str
) -> list[str]:
    tool = [
        "--window",
        "--gpu-live",
        "--gpu-live-inferred",
        "--present",
        "window",
        "--mute",
        "--disc",
        str(disc),
        *record_flags(record),
        "--replay-input",
        str(record),
        "--replay-interactive",
        "--run-dir",
        str(run_dir.relative_to(ROOT)),
        "--timeout",
        str(args.timeout),
        *(["--xbe", str(args.xbe)] if args.xbe is not None else []),
        *args.extra,
    ]
    return play_argv(python, host, tool, True)


def replay(args: argparse.Namespace) -> int:
    main = main_checkout(ROOT, args.main_root)
    python = find_python(ROOT, main, args.python)
    host = current_host(args)
    record = link_into_worktree(ROOT, main, args.input) or (ROOT / args.input)
    if not record.is_file():
        raise SystemExit(f"recorded input {args.input} not found in {ROOT} or {main}")
    disc = args.disc or locate_disc(ROOT, main)
    if disc is None:
        raise SystemExit("no disc image found: pass --disc")
    if args.disc is None:
        link_into_worktree(
            ROOT, main, disc.relative_to(main) if disc.is_relative_to(main) else disc
        )
    print(
        f"host {host.relative_to(ROOT)} sha256 {sha256_file(host)}\nrecord {record} sha256 {sha256_file(record)}"
    )
    results = []
    failed = False
    stamp = time.strftime("%Y%m%d-%H%M%S")
    for index in range(1, args.repeats + 1):
        run_dir = ROOT / WORK / "runs" / f"{stamp}-r{index}"
        argv = replay_argv(args, host, disc, record, run_dir, python)
        if args.dry_run:
            print(" ".join(argv))
            continue
        started = time.monotonic()
        run_dir.mkdir(parents=True, exist_ok=True)
        launch_log = run_dir / "launcher.log"
        launch_code = None
        try:
            with launch_log.open("wb") as output:
                launched = subprocess.run(
                    argv,
                    cwd=ROOT,
                    env=private_env(True),
                    timeout=args.timeout + 120,
                    check=False,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                )
                launch_code = launched.returncode
                failed |= launch_code != 0
        except subprocess.TimeoutExpired:
            failed = True
            print(f"r{index}: harness timeout")
        seconds = time.monotonic() - started
        found = parse_stop(run_dir)
        results.append(found)
        print(
            f"r{index}: STOP {found['stop']}\n    detail: {found['detail']}\n    census: {found['census']}\n"
            f"    last guest calls: {found['calls']}\n    time: {seconds:.0f} s\n    run dir: {run_dir.relative_to(ROOT)}"
        )
        print(f"    launcher exit: {launch_code}; log: {launch_log.relative_to(ROOT)}")
    return int(failed)


def locate_disc(root: Path, main: Path) -> Path | None:
    for base in (root, main):
        for pattern in ("tmp/*(USA) (XBOX).iso", "tmp/*XBOX*.iso"):
            found = sorted(base.glob(pattern))
            if found:
                return found[0]
    return None


def make_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--main-root",
        type=Path,
        help="main checkout with the gitignored inputs (env TSFP_MAIN_ROOT)",
    )
    parser.add_argument(
        "--files-dir",
        # Keep parser construction side-effect free. Resolve worktree/main-checkout
        # fallbacks when the command consumes this value, not while building help or
        # tests are replacing subprocess.run.
        default=os.environ.get("TSFP_FILES_DIR", str(owner_files.FILES_DIR)),
        help="owner files dir, recorded only, never printed or copied (env TSFP_FILES_DIR)",
    )
    parser.add_argument(
        "--python",
        help="interpreter with capstone (default: .venv of this tree or the main checkout)",
    )
    parser.add_argument("--xbe", type=Path, help="retail default.xbe (default build/default.xbe)")
    parser.add_argument(
        "--xtlid", type=Path, help="xtlid.xml for regenerating the xdk surface (env TSFP_XTLID)"
    )
    parser.add_argument(
        "--extra-seeds",
        type=Path,
        action="append",
        default=[],
        help="extra observed entries, vendored seed-functions JSON (repeatable)",
    )
    parser.add_argument(
        "--fpo-leaf-ebp-entry",
        choices=("seh", "g_ebp-if-unread"),
        default="seh",
        help="T1860: lifter policy for the fpo-leaf EBP entry (default seh, unchanged); part of the cache key",
    )
    parser.add_argument(
        "--lift-only",
        action="store_true",
        help="build command: lift only, no C host configure/build",
    )
    parser.add_argument(
        "--force-lift",
        action="store_true",
        help="regenerate a missing or incomplete lift; complete matching lifts are reused",
    )
    parser.add_argument(
        "--allow-failed-functions",
        action="store_true",
        help="accept lift exit 1 (some function failed to lift), for blanket-seed reach measurements only",
    )
    parser.add_argument("--no-build", action="store_true", help="use the last built private host")
    parser.add_argument(
        "--lifted-opt",
        default=DEFAULT_LIFTED_OPT,
        choices=("O0", "O1", "O2"),
        help="T1289: optimisation of the lifted guest code (default O2: the guest thread needs 20 to 25 percent less CPU per frame than at -O0, "
        "frames and guest inputs identical in the equivalence runs of docs/t982-timing-audio.md; O0 is the previous behaviour)",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        help="parallel compile jobs (default: min(cpus, MemAvailable / peak RSS per lifted compile))",
    )
    parser.add_argument(
        "--keep-going",
        action="store_true",
        help="ninja -k 0: compile every file and list ALL failures, not just the first",
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="print the key, paths and commands, run nothing slow"
    )
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("build", help="private lift (cached), xdk files, host")
    play_parser = sub.add_parser(
        "play", help="tools.play on the private host; other arguments pass through"
    )
    play_parser.add_argument(
        "--headless-display",
        action="store_true",
        help="run under xvfb-run (default: the real display)",
    )
    replay_parser = sub.add_parser(
        "replay", help="headless xvfb replay of a recorded input, N repeats"
    )
    replay_parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    replay_parser.add_argument("--disc", type=Path)
    replay_parser.add_argument("--repeats", type=int, default=1)
    replay_parser.add_argument(
        "--timeout", type=int, default=600, help="per run hard kill, seconds"
    )
    replay_parser.add_argument("extra", nargs="*", help="more tools.play arguments")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = make_parser()
    raw = sys.argv[1:] if argv is None else argv
    passthrough: list[str] = []
    if "play" in raw and "--" in raw:
        split = raw.index("--")
        raw, passthrough = raw[:split], raw[split + 1 :]
    args = parser.parse_args(raw)
    if args.command == "build":
        build(args)
        return 0
    if args.command == "play":
        return play(args, passthrough)
    return replay(args)


if __name__ == "__main__":
    sys.exit(main())
