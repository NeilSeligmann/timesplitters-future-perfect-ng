# SPDX-License-Identifier: GPL-3.0-or-later
"""T3501: c-diff lifted bundles, so private-lift c-diff steps can run on remote verify hosts.

`crates/tsfp-c-diff/build.rs` extracts the lifted bodies of its differential roots from the
private lift (`generated/lifted/gen`, main host only) and compiles them with the src/game C
replacements into two static archives (about 13 s, MEASURED). A verify host has no lift, so the
same step there compiled without the lifted reference and ran zero lifted tests
(`zero_tests_remote`) before falling back to the owner's PC.

Mechanism:

* `ensure(root)` runs that same build script standalone on the main host (compiled once with
  rustc, cached by its source hash) and keeps its archives plus the cargo directives it printed
  (`rustc-cfg`, `rustc-check-cfg`, link libs, warnings) as a bundle under
  `tmp/wt-cache/cdiff-lifted/<id>/`. The id is `<inputs key>-<lift hash>`: the inputs key is the
  build script's own hash of every non-lift file it reads (`TSFP_CDIFF_PRINT_KEY=1`), the lift
  hash covers every file of the lift dir. Single-flight under one lock; a present id is reused.
* `remote_command(cmd, bundle, env)` is the portable form: `env TSFP_CDIFF_LIFT=<id> <forwarded
  portable TSFP_* and RUST_TEST_* variables> cargo ...` (TSFP_REF_LIFTED and friends change
  which reference runs, so they must reach the host).
* `ship(...)` (called by `remote_exec.run_verify`) copies the bundle to
  `<host.workdir>/cdiff-lifted/<id>/` once (immutable, atomic `mv`, newest KEEP kept) and the
  verify script exports TSFP_CDIFF_BUNDLE_DIR there. On the host the build script finds no lift,
  sees TSFP_CDIFF_LIFT, checks the id against its own inputs key and links the bundle; a missing
  or foreign bundle fails the build (never a silent run without the lifted reference).

Bundles hold compiled archives and directives only, never lifted C text. They are owner-derived
data: they live in ignored `tmp/` and on the owner's verify hosts, never in tracked paths.
"""

from __future__ import annotations

import fcntl
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

from tools.cache_telemetry import record_lookup
from tools.wt import cache_keys

CRATE = "tsfp-c-diff"
CRATE_REL = Path("crates") / CRATE
STORE_REL = Path("tmp/wt-cache/cdiff-lifted")
REMOTE_DIR = "cdiff-lifted"  # under the verify host's workdir
ENV_ID = "TSFP_CDIFF_LIFT"
ENV_DIR = "TSFP_CDIFF_BUNDLE_DIR"
ARCHIVES = ("libwave3c.a", "libmemcdiff.a")
DIRECTIVES = "directives.txt"
KEEP = 24
ID = re.compile(r"\bTSFP_CDIFF_LIFT=([0-9a-f]{32}-[0-9a-f]{16})\b")
KEPT_DIRECTIVE = re.compile(r"^cargo:(rustc-cfg|rustc-check-cfg|rustc-link-lib|warning)=")
PORTABLE_VALUE = re.compile(r"^[A-Za-z0-9_.,:+=-]*$")
# Volatile bookkeeping (tools.wt.build.VOLATILE_TSFP_ENV) and host-local paths stay home.
NOT_FORWARDED = {
    "TSFP_LIFTED_DIR",
    "TSFP_BUILD_SLOT_HELD",
    "TSFP_BUILD_REMOTE",
    "TSFP_WT_FAMILY",
    "TSFP_CARGO_INCREMENTAL",
    "TSFP_MERGE_URGENT",
    "TSFP_REMOTE_VERIFY_FALLBACK",
}

Log = Callable[[str], object]


class BundleError(RuntimeError):
    """No bundle could be built (no lift, compiler or build-script failure)."""


@dataclass(frozen=True)
class Bundle:
    id: str
    path: Path
    built: bool  # this call built it (False: reused)
    seconds: float


def lift_dir(root: Path, env: Mapping[str, str] | None = None) -> Path | None:
    """The lift the build script would read, or None (same rule as `*_lifted::lifted_dir`)."""
    env = os.environ if env is None else env
    value = env.get("TSFP_LIFTED_DIR")
    path = Path(value) if value else root / "generated/lifted/gen"
    return path if (path / "recomp_types.h").is_file() else None


def is_cdiff(crate: Path) -> bool:
    return crate.name == CRATE


def store(main: Path) -> Path:
    return main / STORE_REL


def lift_hash(directory: Path, main: Path) -> str:
    """sha256 (16 hex) over lift content, using logical names independent of host paths."""
    files = sorted(p for p in directory.iterdir() if p.is_file())
    stamp = hashlib.sha256(
        "\n".join(f"{p.name}:{p.stat().st_size}:{p.stat().st_mtime_ns}" for p in files).encode()
    ).hexdigest()
    cache = store(main) / "lift-hash-v2.json"
    try:
        known = json.loads(cache.read_text())
    except (OSError, ValueError):
        known = {}
    key = f"{directory.resolve()}:{stamp}"
    if key in known:
        return known[key]
    content = cache_keys.named_file_components({path.name: path for path in files})
    value = cache_keys.make_key(
        "cdiff-lift", files=content, tools={}, env={}, extra={"key-version": 2}
    ).key[:16]
    cache.parent.mkdir(parents=True, exist_ok=True)
    known = {k: v for k, v in known.items() if not k.startswith(f"{directory.resolve()}:")}
    known[key] = value
    temp = cache.with_suffix(f".{os.getpid()}.tmp")
    temp.write_text(json.dumps(known, sort_keys=True) + "\n")
    os.replace(temp, cache)
    return value


def _script_hash(root: Path, crate_dir: Path, env: Mapping[str, str] | None = None) -> str:
    """Identify the standalone bundler by source, rustc and its compile options."""
    files = {path.name: path for path in sorted(crate_dir.glob("*.rs"))}
    return cache_keys.make_key(
        "cdiff-bundler",
        files=cache_keys.named_file_components(files),
        tools=cache_keys.tool_versions(root, env=env, names=("rustc",)),
        env={},
        extra={"edition": "2024", "optimization": "-O"},
    ).key[:16]


def bundler(root: Path, main: Path) -> Path:
    """The crate's build script compiled as a standalone binary (std only), cached."""
    crate_dir = root / CRATE_REL
    binary = store(main) / "bin" / f"build-script-{_script_hash(root, crate_dir)}"
    if binary.is_file():
        return binary
    binary.parent.mkdir(parents=True, exist_ok=True)
    temp = binary.with_suffix(f".{os.getpid()}.tmp")
    env = {k: v for k, v in os.environ.items() if k not in ("RUSTC_WRAPPER", "RUSTFLAGS")}
    result = subprocess.run(
        ["rustc", "--edition", "2024", "-O", "--crate-name", "cdiff_build_script"]
        + ["build.rs", "-o", str(temp)],
        cwd=crate_dir,
        env=env,
        capture_output=True,
        text=True,
        timeout=600,
        check=False,
    )
    if result.returncode:
        temp.unlink(missing_ok=True)
        raise BundleError(f"rustc build.rs failed: {result.stderr[-800:]}")
    os.replace(temp, binary)
    return binary


def _script_env(root: Path, out: Path, env: Mapping[str, str]) -> dict[str, str]:
    script = dict(os.environ)
    script.update(
        {
            "CARGO_MANIFEST_DIR": str((root / CRATE_REL).resolve()),
            "OUT_DIR": str(out),
        }
    )
    script.pop("TSFP_C_OPT", None)
    if env.get("TSFP_C_OPT"):
        script["TSFP_C_OPT"] = env["TSFP_C_OPT"]
    return script


def inputs_key(root: Path, main: Path, env: Mapping[str, str] | None = None) -> str:
    env = os.environ if env is None else env
    binary = bundler(root, main)
    with tempfile.TemporaryDirectory(prefix="cdiff-key-") as temp:
        script = _script_env(root, Path(temp), env)
        script["TSFP_CDIFF_PRINT_KEY"] = "1"
        result = subprocess.run(
            [str(binary)], env=script, capture_output=True, text=True, timeout=120, check=False
        )
    key = result.stdout.strip()
    if result.returncode or not re.fullmatch(r"[0-9a-f]{32}", key):
        raise BundleError(f"inputs key failed: {(result.stderr or result.stdout)[-400:]}")
    return key


def _gc(directory: Path, keep: int = KEEP) -> None:
    bundles = sorted(
        (p for p in directory.iterdir() if p.is_dir() and re.fullmatch(r"[0-9a-f-]{49}", p.name)),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    for old in bundles[keep:]:
        shutil.rmtree(old, ignore_errors=True)


def ensure(
    root: Path, main: Path, env: Mapping[str, str] | None = None, log: Log | None = None
) -> Bundle:
    """The bundle for this worktree's c-diff inputs and the current lift (built at most once)."""
    env = os.environ if env is None else env
    say = log or (lambda _text: None)
    started = time.monotonic()
    lift = lift_dir(root, env)
    if lift is None:
        raise BundleError("no private lift here (generated/lifted/gen or TSFP_LIFTED_DIR)")
    bundle_id = f"{inputs_key(root, main, env)}-{lift_hash(lift, main)}"
    base = store(main)
    path = base / bundle_id
    if (path / DIRECTIVES).is_file():
        os.utime(path)
        record_lookup("lifted-cdiff-bundle", True)
        return Bundle(bundle_id, path, False, round(time.monotonic() - started, 2))
    base.mkdir(parents=True, exist_ok=True)
    with open(base / ".build.lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)  # single flight: a peer may have built it meanwhile
        if (path / DIRECTIVES).is_file():
            record_lookup("lifted-cdiff-bundle", True)
            return Bundle(bundle_id, path, False, round(time.monotonic() - started, 2))
        say(f"cdiff bundle: building {bundle_id} (build.rs with the private lift)")
        work = Path(tempfile.mkdtemp(prefix=".build-", dir=base))
        try:
            out = work / "out"
            out.mkdir()
            script = _script_env(root, out, env)
            script["TSFP_LIFTED_DIR"] = str(lift)
            result = subprocess.run(
                [str(bundler(root, main))],
                env=script,
                capture_output=True,
                text=True,
                timeout=900,
                check=False,
            )
            if result.returncode:
                raise BundleError(f"build script failed: {result.stderr[-800:]}")
            kept = [line[len("cargo:") :] for line in result.stdout.splitlines()]
            kept = [line for line in kept if KEPT_DIRECTIVE.match(f"cargo:{line}")]
            if not any(line.startswith("rustc-cfg=") for line in kept):
                raise BundleError("build script used no lifted root (no rustc-cfg emitted)")
            final = work / "bundle"
            final.mkdir()
            for name in ARCHIVES:
                shutil.copy2(out / name, final / name)
            (final / DIRECTIVES).write_text("\n".join(kept) + "\n")
            cc = subprocess.run(
                ["cc", "--version"], capture_output=True, text=True, timeout=30, check=False
            )
            meta = {
                "id": bundle_id,
                "cc": (cc.stdout.splitlines() or ["?"])[0],
                "opt": env.get("TSFP_C_OPT", "2"),
                "built_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "seconds": round(time.monotonic() - started, 1),
                "archives": {
                    name: hashlib.sha256((final / name).read_bytes()).hexdigest()
                    for name in ARCHIVES
                },
            }
            (final / "bundle.json").write_text(json.dumps(meta, indent=1, sort_keys=True) + "\n")
            os.replace(final, path)
        finally:
            shutil.rmtree(work, ignore_errors=True)
        _gc(base)
    seconds = round(time.monotonic() - started, 2)
    say(f"cdiff bundle: built {bundle_id} in {seconds} s")
    record_lookup("lifted-cdiff-bundle", False)
    return Bundle(bundle_id, path, True, seconds)


def forwarded_env(env: Mapping[str, str]) -> list[str]:
    """`NAME=value` pairs the remote command must carry: portable TSFP_* and RUST_TEST_*."""
    pairs = []
    for name, value in sorted(env.items()):
        if not (name.startswith("TSFP_") or name.startswith("RUST_TEST_")):
            continue
        if name in NOT_FORWARDED or name in (ENV_ID, ENV_DIR):
            continue
        if PORTABLE_VALUE.match(value):
            pairs.append(f"{name}={value}")
    return pairs


def remote_command(cmd: Sequence[str], bundle: Bundle, env: Mapping[str, str]) -> str:
    """The cargo command as a verify host runs it with this bundle."""
    return shlex.join(["env", f"{ENV_ID}={bundle.id}", *forwarded_env(env), *cmd])


def ids_in(commands: Sequence[str]) -> list[str]:
    found: list[str] = []
    for command in commands:
        found += [m for m in ID.findall(command) if m not in found]
    return found


def remote_base(workdir: str) -> str:
    return f"{workdir}/{REMOTE_DIR}"


def ship(
    bundle_ids: Sequence[str],
    *,
    main: Path,
    workdir: str,
    run: Callable[[str, int], subprocess.CompletedProcess[str]],
    copy: Callable[[Path, str], None],
    rpath: Callable[[str], str],
    log: Log | None = None,
) -> float:
    """Make every bundle present on the host (`run(cmd, timeout)` is ssh); seconds spent."""
    say = log or (lambda _text: None)
    started = time.monotonic()
    base = remote_base(workdir)
    for bundle_id in bundle_ids:
        local = store(main) / bundle_id
        if not (local / DIRECTIVES).is_file():
            raise BundleError(f"bundle {bundle_id} is not in {STORE_REL}")
        dest = f"{base}/{bundle_id}"
        have = run(f"test -f {rpath(dest)}/{DIRECTIVES} && touch {rpath(dest)}", 30)
        if have.returncode == 0:
            continue
        with tempfile.TemporaryDirectory(prefix="cdiff-ship-") as temp:
            archive = Path(temp) / f"{bundle_id}.tar"
            with tarfile.open(archive, "w") as tar:
                for name in (*ARCHIVES, DIRECTIVES, "bundle.json"):
                    tar.add(local / name, arcname=name)
            remote_tar = f"{workdir}/archives/cdiff-{bundle_id}-{os.getpid()}.tar"
            copy(archive, remote_tar)
        install = (
            f"mkdir -p {rpath(base)} && t=$(mktemp -d {rpath(base)}/.in-XXXXXX) && "
            f'tar -xf {rpath(remote_tar)} -C "$t" && '
            f'{{ mv -T "$t" {rpath(dest)} 2>/dev/null || rm -rf "$t"; }}; '
            f"rm -f {rpath(remote_tar)}; "
            f"ls -1dt {rpath(base)}/*-*/ 2>/dev/null | tail -n +{KEEP + 1} | xargs -r rm -rf; "
            f"test -f {rpath(dest)}/{DIRECTIVES}"
        )
        done = run(install, 120)
        if done.returncode:
            raise BundleError(f"installing bundle {bundle_id} failed: {done.stderr[-300:]}")
        say(f"cdiff bundle: shipped {bundle_id}")
    return round(time.monotonic() - started, 2)
