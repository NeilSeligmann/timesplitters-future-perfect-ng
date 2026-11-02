"""T3472: the one Rust build entry point, keyed by a build-input hash.

    python3 -m tools.wt build <crate|path|alias> [--check|--test|--clippy|--build]
                              [--release] [--cold] [--remote auto|never|only]
                              [--no-cache] [--print-key] [-- extra cargo args]

One command replaces the per-script cargo recipes (worker_check, verify profiles, build.sh):

* resolve: a crate name (`tsfp-ports`), an alias (`ports`, `bevy`, `workspace`) or a crate dir;
* differential: the input key hashes only what the build can read: the toolchain pin, the
  root manifests and lockfiles, and the git tree (worktree content, untracked files included)
  of the crate's path-dependency closure. A crate with a build script, or one whose closure
  leaves `crates/`, keys on the whole worktree tree plus the ignored inputs it may read. A
  `--check`/`--test`/`--clippy` that already passed with the same key returns the recorded
  verdict at once (`tmp/wt-cache/build-results/`), on any worktree;
* incremental: dev-profile runs use CARGO_INCREMENTAL=1 in their own canonical target
  (T3445: re-edit 115 s -> 28 s); `--release`, `--cold` and merge verifies
  (TSFP_MERGE_VERIFY=1) stay non-incremental and fully sccache-cached;
* then `tools.wt run` does the rest: canonical per-family tree (T3432), slot admission
  (T3411/T3452), and sccache (the LAN-shared store when reachable, T3472);
* placement (T3885): every build is a `tools.dispatch` job; the dispatcher places it on a remote
  host (capacity-weighted least load) or, last, on the owner PC within its cap. A busy fleet
  means a queue, never a silent local build; a remote FAIL is the verdict (no local confirmation).
  `--remote never` makes it an owner-PC-only job, `--remote only` never runs it on the owner PC.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import socket
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable
from pathlib import Path

from tools.agents import jobevents
from tools.wt import cache_keys, cdiff_bundle, shared_artifacts

RESULTS_REL = Path("tmp/wt-cache/build-results")
ALIASES = {
    "ports": "tsfp-ports",
    "bevy": "tsfp-bevy",
    "host": "tsfp-rt-host",
    "types": "tsfp-types",
}
MODES = {"check": "check", "test": "test", "clippy": "clippy", "build": "build"}
CACHEABLE = {"check", "test", "clippy"}
# Owner 2026-10-21: the local host is the owner's PC. Heavy crate graphs go to a remote slot
# first (T3459 fallback hosts), small ones stay local. A closure this large counts as heavy.
HEAVY_CRATES = {"tsfp-ports", "tsfp-bevy", "tsfp-rt-host", "tsfp-game", "tsfp-wgpu"}
HEAVY_CLOSURE = 12
REMOTE_ENV = "TSFP_BUILD_REMOTE"


def heavy(crate: Path) -> bool:
    return crate.name in HEAVY_CRATES or len(closure(crate)) >= HEAVY_CLOSURE


ROOT_INPUTS = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rust-toolchain", ".cargo")
PATH_DEP = re.compile(r"""path\s*=\s*["']([^"']+)["']""")
KEY_VERSION = "t3472-v3-cache-key-v1"
# Bookkeeping variables that do not change what is compiled or run.
VOLATILE_TSFP_ENV = {
    *cache_keys.VOLATILE_ENV,
    "TSFP_BUILD_SLOT_HELD",
    "TSFP_BUILD_REMOTE",
    "TSFP_WT_FAMILY",
    "TSFP_CARGO_INCREMENTAL",
    # Decision 2026-10-23 (I-T3505-13): tsfp-bevy and tsfp-game build.rs embed TSFP_GIT_SHA
    # only as a printed build stamp; keying it would miss the cache on every commit.
    "TSFP_GIT_SHA",
    # Paths are host/worktree coordinates. Lift inputs are represented by content below.
    "TSFP_LIFTED_DIR",
    "TSFP_CDIFF_BUNDLE_DIR",
    "TSFP_CANON_ROOT",
    "TSFP_FILES_DIR",
}
# Replayed on a cached pass so callers that count tests (worker_check) see the same verdict.
SUMMARY_LINE = re.compile(r"^(test result: |\s+Running |\s+Doc-tests )")
BUILD_INPUTS = Path(__file__).resolve().parents[1] / "data/build_inputs.json"


def declared_inputs(path: Path | None = None) -> dict[str, list[str]]:
    try:
        data = json.loads((path or BUILD_INPUTS).read_text())
    except (OSError, ValueError):
        return {}
    return {name: list(paths) for name, paths in data.items() if not name.startswith("_")}


def resolve(root: Path, target: str) -> Path:
    """The crate directory (holding Cargo.toml) for a name, alias or path."""
    name = ALIASES.get(target, target)
    if name in ("workspace", "."):
        return root
    for candidate in (root / name, root / "crates" / name):
        if (candidate / "Cargo.toml").is_file():
            return candidate.resolve()
    raise SystemExit(f"tools.wt build: no crate {target!r} (looked in . and crates/)")


def closure(crate: Path) -> list[Path]:
    """The crate and its path dependencies, recursively, from the manifests' `path = ...`."""
    seen: list[Path] = []
    todo = [crate.resolve()]
    while todo:
        current = todo.pop()
        if current in seen or not (current / "Cargo.toml").is_file():
            continue
        seen.append(current)
        text = (current / "Cargo.toml").read_text()
        for match in PATH_DEP.finditer(text):
            todo.append((current / match.group(1)).resolve())
        members = re.search(r"^members\s*=\s*\[(.*?)\]", text, re.M | re.S)
        if members:
            for item in re.findall(r"""["']([^"']+)["']""", members.group(1)):
                todo.append((current / item).resolve())
    return sorted(seen)


def snapshot_tree(root: Path) -> str:
    """git tree id of source content, excluding volatile local tmp data.

    A private copy of the index keeps the stat cache, so it is cheap and does not touch the
    real index. `tmp/` contains build-slot leases and caches that can appear or disappear
    while Git traverses the worktree; they are not build source inputs.
    """
    gitdir = Path(
        subprocess.run(
            ["git", "rev-parse", "--absolute-git-dir"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=60,
            check=True,
        ).stdout.strip()
    )
    index = gitdir / "index"
    descriptor, temp_name = tempfile.mkstemp(prefix="tsfp-build-index-", dir=gitdir)
    os.close(descriptor)
    temp = Path(temp_name)
    try:
        if index.is_file():
            temp.write_bytes(index.read_bytes())
        env = {**os.environ, "GIT_INDEX_FILE": str(temp)}
        ignored_tmp = subprocess.run(
            ["git", "check-ignore", "-q", "--", "tmp"],
            cwd=root,
            env=env,
            timeout=60,
            check=False,
        )
        if ignored_tmp.returncode not in (0, 1):
            raise RuntimeError(f"git check-ignore failed while snapshotting {root}")
        paths = ["."] if ignored_tmp.returncode == 0 else [".", ":(exclude)tmp/**"]
        try:
            subprocess.run(
                ["git", "add", "-A", "--", *paths],
                cwd=root,
                env=env,
                timeout=300,
                check=True,
                capture_output=True,
            )
        except subprocess.CalledProcessError as error:
            detail = (error.stderr or b"").decode(errors="replace").strip()
            raise RuntimeError(f"git add failed while snapshotting {root}: {detail}") from error
        try:
            return subprocess.run(
                ["git", "write-tree"],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                timeout=120,
                check=True,
            ).stdout.strip()
        except subprocess.CalledProcessError as error:
            detail = (error.stderr or "").strip()
            message = f"git write-tree failed while snapshotting {root}: {detail}"
            raise RuntimeError(message) from error
    finally:
        temp.unlink(missing_ok=True)


def subtree(root: Path, tree: str, rel: str) -> str:
    if rel in ("", "."):
        return tree
    out = subprocess.run(
        ["git", "rev-parse", f"{tree}:{rel}"],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    return out.stdout.strip() if out.returncode == 0 else "absent"


def ignored_stamps(root: Path) -> list[str]:
    """size/mtime of the gitignored inputs build scripts may read (core.LINKS)."""
    from tools.wt import core

    rows = []
    for link in core.LINKS:
        if "*" in link.path or link.path.startswith("tmp/"):
            continue
        path = root / link.path
        try:
            st = path.stat()
        except OSError:
            rows.append(f"{link.path}:absent")
            continue
        rows.append(f"{link.path}:{st.st_size}:{st.st_mtime_ns}")
    return rows


def input_key(
    root: Path, crate: Path, cargo: list[str], env: dict[str, str], tree: str | None = None
) -> tuple[str, dict[str, object]]:
    """(key, description) for this build; the description is what went into the hash."""
    tree = tree or snapshot_tree(root)
    crates = closure(crate)
    rels = []
    whole = False
    extras: list[str] = []
    declared = declared_inputs()
    for item in crates:
        try:
            rel = item.relative_to(root.resolve()).as_posix()
        except ValueError:
            whole = True
            continue
        rels.append(rel or ".")
        if rel in ("", ".") or not rel.startswith("crates/"):
            whole = True
        elif (item / "build.rs").is_file():
            if item.name in declared:
                # entries may glob ("crates/tsfp-port-*"): families the build script scans
                for entry in declared[item.name]:
                    extras += (
                        sorted(p.relative_to(root).as_posix() for p in root.glob(entry))
                        if "*" in entry
                        else [entry]
                    )
            else:
                whole = True
    parts: dict[str, object] = {
        "version": KEY_VERSION,
        "cargo": cargo,
        "env": {
            name: env.get(name, "")
            for name in (
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
                "CARGO_BUILD_TARGET",
                "TSFP_CARGO_FEATURES",
                "RUSTUP_TOOLCHAIN",
            )
        },
        # TSFP_LIFTED_DIR, TSFP_REF_LIFTED etc. change which tests compile and run.
        "tsfp_env": {
            name: value
            for name, value in sorted(env.items())
            if name.startswith("TSFP_") and name not in VOLATILE_TSFP_ENV
        },
        "root": {name: subtree(root, tree, name) for name in ROOT_INPUTS},
    }
    if any(cdiff_bundle.is_cdiff(item) for item in crates):
        # T3501: the c-diff build script compiles the private lift; its content decides the run.
        lift = cdiff_bundle.lift_dir(root, env)
        parts["cdiff_lift"] = (
            cdiff_bundle.lift_hash(lift, core_main(root)) if lift is not None else "absent"
        )
    elif env.get("TSFP_LIFTED_DIR"):
        # Lift paths are host-local; when present, represent the code by its content only.
        lift = Path(env["TSFP_LIFTED_DIR"])
        parts["lift_content"] = (
            cdiff_bundle.lift_hash(lift, core_main(root)) if lift.is_dir() else "absent"
        )
    if whole:
        parts["tree"] = tree
        parts["ignored"] = ignored_stamps(root)
    else:
        parts["crates"] = {rel: subtree(root, tree, rel) for rel in sorted(rels)}
        if extras:
            parts["extra"] = {rel: subtree(root, tree, rel) for rel in sorted(set(extras))}
            parts["ignored"] = ignored_stamps(root)
        for rel in rels:
            for lock in ("Cargo.lock", "rust-toolchain.toml"):
                parts["crates"][f"{rel}/{lock}"] = subtree(root, tree, f"{rel}/{lock}")
    shared = cache_keys.make_key(
        "wt-build-result",
        files={},
        tools=cache_keys.tool_versions(root, env=env),
        env=env,
        allow_env=(
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
            "CARGO_PROFILE",
            "TSFP_CARGO_FEATURES",
        ),
        extra={"input-description-version": KEY_VERSION, "inputs": parts},
    )
    # `extra.inputs` in the key components references `parts`; detach the snapshot before
    # adding it back to `parts`, or receipts become a circular JSON structure.
    parts["key_components"] = json.loads(json.dumps(shared.components))
    return shared.key[:32], parts


def uncacheable_env(env: dict[str, str], crate: Path) -> bool:
    """Skip verdict caching when build scripts read mutable host-build directories."""
    names = {item.name for item in closure(crate)}
    if "tsfp-bevy" in names and env.get("TSFP_BEVY_HOST_BUILD_DIR"):
        return True
    # tsfp-game's build script reads CMakeCache.txt, build.ninja and every object/archive
    # selected by Ninja from this external directory. These mutable files cannot be
    # represented by build_inputs.json; avoid serving or recording a cached verdict.
    return "tsfp-game" in names and bool(
        env.get("TSFP_GAME_HOST_BUILD_DIR") or env.get("TSFP_LIFTED_DIR")
    )


def cmd_is_test(cmd: list[str]) -> bool:
    return "test" in cmd[:3]


def result_path(main: Path, key: str) -> Path:
    return main / RESULTS_REL / f"{key}.json"


def cached(main: Path, key: str) -> dict | None:
    try:
        data = json.loads(result_path(main, key).read_text())
    except (OSError, ValueError):
        return None
    return data if data.get("returncode") == 0 else None


def record(main: Path, key: str, data: dict) -> None:
    path = result_path(main, key)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(f".{os.getpid()}.tmp")
    temp.write_text(json.dumps(data, sort_keys=True) + "\n")
    os.replace(temp, path)


def shared_verdict(key: str) -> dict | None:
    """Fetch a successful Bevy check receipt published by another host."""
    with tempfile.TemporaryDirectory(prefix=".shared-build-verdict-") as temp:
        directory = Path(temp)
        archive = directory / "verdict.tar.gz"
        if not shared_artifacts.fetch_file("build-verdict", key, archive):
            return None
        extracted = directory / "receipt"
        if not shared_artifacts.unpack_tree(archive, extracted):
            return None
        try:
            receipt = json.loads((extracted / "verdict.json").read_text())
        except (OSError, ValueError):
            return None
    result = receipt.get("result")
    if (
        receipt.get("key") != key
        or not isinstance(result, dict)
        or result.get("returncode") != 0
        or result.get("mode") != "check"
        or result.get("target") != "tsfp-bevy"
    ):
        return None
    return result


def publish_shared_verdict(key: str, result: dict) -> bool:
    """Publish only a successful Bevy check, addressed by its canonical build input key."""
    if (
        result.get("returncode") != 0
        or result.get("mode") != "check"
        or result.get("target") != "tsfp-bevy"
    ):
        return False
    with tempfile.TemporaryDirectory(prefix=".shared-build-verdict-") as temp:
        directory = Path(temp)
        receipt = directory / "receipt"
        receipt.mkdir()
        (receipt / "verdict.json").write_text(
            json.dumps({"key": key, "result": result}, sort_keys=True) + "\n"
        )
        archive = directory / "verdict.tar.gz"
        shared_artifacts.pack_tree(receipt, archive)
        return shared_artifacts.publish_file("build-verdict", key, archive)


def shareable_verdict(mode: str, crate: Path, cacheable: bool) -> bool:
    """Cross-host receipt reuse is safe for the content-keyed Bevy check only."""
    return cacheable and mode == "check" and crate.name == "tsfp-bevy"


def run_build_miss(
    root: Path,
    main_dir: Path,
    crate: Path,
    args: argparse.Namespace,
    env: dict[str, str],
    cmd: list[str],
    key: str,
    key_components: dict[str, object],
    inc: bool,
    cacheable: bool,
    shareable: bool,
    started: float,
    log: Callable[[str], None],
) -> int:
    log(f"tools.wt build: {shlex.join(cmd)} (key {key}, incremental={int(inc)})")
    summary: list[str] = []
    remote_cmd = remote_command(root, main_dir, crate, cmd, env, args.remote, log)
    if remote_cmd is None and args.remote == "only":
        return 3
    from tools.dispatch import DispatchError

    try:
        returncode, where = run_dispatched(root, main_dir, args, env, cmd, remote_cmd, log, summary)
    except (DispatchError, TimeoutError) as error:
        log(f"tools.wt build: {error}")
        return 3
    seconds = round(time.monotonic() - started, 1)
    zero_tests = cmd_is_test(cmd) and not any(
        re.match(r"^test result: ok\. [1-9]", line) for line in summary
    )
    if cacheable and returncode == 0 and not zero_tests:
        result = {
            "returncode": 0,
            "mode": args.mode,
            "target": crate.name,
            "command": cmd,
            # Persist the canonical inputs beside successful verdicts so cache-keys
            # --why-miss can compare the receipt with a later run descriptor.
            "key_components": key_components,
            "host": f"{socket.gethostname()}:{where}",
            "at": time.strftime("%Y-%m-%dT%H:%M:%S"),
            "seconds": seconds,
            "incremental": inc,
            "summary": summary[-400:],
        }
        record(main_dir, key, result)
        if shareable:
            published = publish_shared_verdict(key, result)
            log(
                f"tools.wt build: shared Bevy check verdict "
                f"{'published' if published else 'not published'} for {key}"
            )
    log(f"tools.wt build: rc={returncode} in {seconds} s ({where})")
    return returncode


def cargo_command(mode: str, manifest: str, release: bool, extra: list[str]) -> list[str]:
    cmd = ["cargo", MODES[mode], "--offline", "--manifest-path", manifest]
    if release:
        cmd.append("--release")
    return cmd + extra


TEST_MARK = re.compile(r"#\[\s*(?:[\w:]+::)?test\b|#\[\s*cfg\(\s*test\s*\)|^\s*\[\[test\]\]", re.M)


def crate_has_tests(crate: Path) -> bool:
    """The package defines a test (`#[test]`, `#[cfg(test)]`, `[[test]]`, a tests/ file)."""
    if any((crate / "tests").glob("**/*.rs")):
        return True
    sources = [crate / "Cargo.toml", *sorted((crate / "src").glob("**/*.rs"))]
    return any(
        TEST_MARK.search(path.read_text(errors="replace")) for path in sources if path.is_file()
    )


def compile_only_without_tests(
    cmd: list[str], crate: Path, log: Callable[[str], None]
) -> list[str]:
    """T3885 (claude-1 19:35): `cargo test` of a package with no tests ran zero tests, which
    the remote runner rightly reads as no verdict (zero_tests_remote) and the landing timed out.
    Such a package is verified by compiling its test targets: `--no-run`, before any `--`."""
    if not cmd_is_test(cmd) or "--no-run" in cmd or crate_has_tests(crate):
        return cmd
    log(f"tools.wt build: {crate.name} defines no tests, compiling its test targets (--no-run)")
    cut = cmd.index("--") if "--" in cmd else len(cmd)
    return [*cmd[:cut], "--no-run", *cmd[cut:]]


def incremental(mode: str, release: bool, cold: bool, env: dict[str, str]) -> bool:
    """Opt in for persistent local edit loops; shared builds stay sccache-cacheable."""
    if release or cold or env.get("TSFP_MERGE_VERIFY") == "1":
        return False
    if env.get("TSFP_CARGO_INCREMENTAL") in ("0", "1"):
        return env["TSFP_CARGO_INCREMENTAL"] == "1"
    return False


def run_local(
    root: Path,
    family: str,
    cmd: list[str],
    env: dict[str, str],
    summary: list[str] | None = None,
    target_activity: list[str] | None = None,
) -> int:
    """Run through `tools.wt run`, streaming output; keep the verdict lines in `summary`."""
    wt = [sys.executable, "-m", "tools.wt", "run", "--family", family, "--", *cmd]
    with subprocess.Popen(
        wt,
        cwd=root,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        bufsize=1,
    ) as proc:
        assert proc.stdout is not None
        for line in proc.stdout:
            sys.stdout.write(line)
            if summary is not None and SUMMARY_LINE.match(line):
                summary.append(line.rstrip("\n"))
            if target_activity is not None and _CARGO_TARGET_ACTIVITY.search(line):
                target_activity.append(line.rstrip("\n"))
        sys.stdout.flush()
        return proc.wait()


_CARGO_TARGET_ACTIVITY = re.compile(r"\b(?:Fresh|Compiling|Checking|Building|Finished)\b")


def cargo_target_cache_hit(output: str, returncode: int) -> bool | None:
    """Classify Cargo's target-cache result when its normal status lines are present."""
    if returncode != 0:
        return None
    statuses = _CARGO_TARGET_ACTIVITY.findall(output)
    if not statuses:
        return None
    if any(status in {"Compiling", "Checking", "Building"} for status in statuses):
        return False
    return True if "Finished" in statuses or "Fresh" in statuses else None


def record_cargo_target_lookup(
    output: str,
    returncode: int,
    *,
    host: str | None = None,
    root: Path | None = None,
    command: list[str] | str | None = None,
    env: dict[str, str] | None = None,
) -> bool | None:
    """Record Cargo freshness separately from the tools.wt build-verdict cache."""
    hit = cargo_target_cache_hit(output, returncode)
    if hit is not None:
        from tools.cache_telemetry import record_lookup

        cache = "cargo-target-canon-slot"
        if command is not None:
            words = shlex.split(command) if isinstance(command, str) else command
            from tools.wt import canonical_build

            if canonical_build.cargo_argv(words) is None or not canonical_build.enabled(env or {}):
                cache = "cargo-target-worktree"
        record_lookup(cache, hit, host=host, root=root)
    return hit


def remote_infra_reason(notes: list[str]) -> str:
    """T3821: the dominant reason of a remote 'infra' outcome from notes like 'laptop:slot_busy'."""
    kinds = []
    for note in notes:
        kind = note.split(":", 1)[-1] if ":" in note else note
        kind = re.split(r"[ (:]", kind.strip(), maxsplit=1)[0]
        if kind and kind != "disabled":
            kinds.append(kind)
    if any("stopped_by_signal" in note for note in notes):
        return "stopped_by_signal"
    return max(sorted(set(kinds)), key=kinds.count) if kinds else "no-host"


def run_remote(
    root: Path,
    main: Path,
    cmd: list[str] | str,
    log: Callable[[str], None],
    summary: list[str] | None = None,
) -> int | None:
    """Exit code from a remote host, or None when no remote verdict was possible (any host)."""
    try:
        from tools.wt import remote_fallback
    except ImportError:
        log("tools.wt build: remote fallback not available in this checkout")
        return None
    # T3582: the same remote queue window as the merge verifies (300 s, 60 s when urgent).
    queue_s = remote_fallback.queue_wait(
        locked=False, urgent=os.environ.get("TSFP_MERGE_URGENT") == "1"
    )
    return remote_verdict(remote_outcome(root, main, cmd, log, queue_s=queue_s), log, summary)


def remote_outcome(
    root: Path,
    main: Path,
    cmd: list[str] | str,
    log: Callable[[str], None],
    *,
    hosts: list[object] | None = None,
    queue_s: float | None = None,
    budget_reservation: object | None = None,
) -> object:
    """One `remote_fallback.run_remote` (on `hosts` when given), logged like every remote build.

    Every attempt is logged with caller wt-build in tmp/verify-admission.jsonl (T3461)."""
    from tools.wt import remote_fallback

    started = time.monotonic()
    command = cmd if isinstance(cmd, str) else shlex.join(cmd)
    outcome = remote_fallback.run_remote(
        [command],
        root=root,
        main=main,
        log=log,
        queue_s=queue_s,
        hosts=hosts,
        **({"budget_reservation": budget_reservation} if budget_reservation is not None else {}),
    )
    if outcome.status in {"pass", "fail"}:
        record_cargo_target_lookup(
            outcome.output,
            0 if outcome.status == "pass" else (outcome.returncode or 1),
            host=outcome.host,
            command=command,
            env=os.environ,
        )
    record_admission(
        main,
        {
            "decision": "remote",
            "remote_status": outcome.status,
            "remote_host": outcome.host,
            "remote_s": round(time.monotonic() - started, 1),
            "notes": remote_fallback.record_notes(outcome.notes),
        },
    )
    jobevents.emit(
        "remote-build",
        start=time.time() - (time.monotonic() - started),
        result=outcome.status,
        cause="remote-unavailable" if outcome.status == "infra" else None,
        remote_host=outcome.host,
        # T3821: infra events had an empty error; the per-host notes name why (busy, signal, ...)
        error=", ".join(outcome.notes)[:300] if outcome.status != "pass" else None,
        reason=remote_infra_reason(outcome.notes) if outcome.status == "infra" else None,
    )
    return outcome


def remote_verdict(
    outcome: object, log: Callable[[str], None], summary: list[str] | None
) -> int | None:
    """The exit code of a remote pass or fail (verdict lines into `summary`), None for infra.

    T3501: the verdict lines of a remote pass go to `summary`, so it is recorded in the result
    cache like a local one (before, every remote pass looked like zero tests)."""
    from tools.wt import remote_fallback

    assert isinstance(outcome, remote_fallback.Outcome)
    if outcome.status == "infra":
        log(f"tools.wt build: no remote verdict ({', '.join(outcome.notes)[:200]})")
        return None
    if outcome.output:
        print(outcome.output[-4000:], file=sys.stderr)
    if summary is not None and outcome.status == "pass":
        summary += [line for line in outcome.output.splitlines() if SUMMARY_LINE.match(line)]
        total = remote_fallback.tests_ran(outcome.output)
        if total and not any(re.match(r"^test result: ok\. [1-9]", line) for line in summary):
            # T3798: the passing line fell outside the log tail; the host counted the full log
            summary.append(f"test result: ok. {total} passed (remote full-log count)")
    return 0 if outcome.status == "pass" else (outcome.returncode or 1)


DISPATCH_CORES = 8.0
"""Cores a cargo job is priced at when CARGO_BUILD_JOBS is unset (the laptop's verify jobs)."""


def dispatcher(main: Path, log: Callable[[str], None]) -> object:
    """The work dispatcher (T3885); tests replace this with a fake-host dispatcher."""
    from tools import dispatch

    return dispatch.default(main, log=log)


def run_dispatched(
    root: Path,
    main_dir: Path,
    args: argparse.Namespace,
    env: dict[str, str],
    cmd: list[str],
    remote_cmd: str | None,
    log: Callable[[str], None],
    summary: list[str],
) -> tuple[int, str]:
    """T3885: place the build with `tools.dispatch` and run it there: (exit code, where).

    A portable build runs on the placed remote host; a host without a verdict requeues it
    (busy) or is benched and excluded (failure). A remote FAIL is the verdict: no local
    confirmation compile. Only a non-portable build (`--remote never`, private lift without
    bundle, absolute paths) or a placement on the owner PC runs locally, with the placement
    token in its environment and within the owner PC's dispatch cap."""
    from tools import dispatch
    from tools.dispatch import hosts as dispatch_hosts
    from tools.dispatch import remote_evidence, runner
    from tools.wt import remote_exec

    reason = (
        ""
        if remote_cmd is not None
        else ("remote never" if args.remote == "never" else "not portable")
    )
    cores = float(env.get("CARGO_BUILD_JOBS") or DISPATCH_CORES)
    spec = dispatch.job(
        "cargo",
        cores,
        local_only=remote_cmd is None,
        local_reason=reason,
        priority=10 if os.environ.get("TSFP_MERGE_URGENT") == "1" else 0,
        label=f"wt-build {args.target} {args.mode}",
        commands=[remote_cmd or shlex.join(cmd)],
        root=root,
    )
    disp = dispatcher(main_dir, log)

    def remote(lease: dispatch.Lease) -> tuple[int, str]:
        assert remote_cmd is not None
        outcome = remote_outcome(
            root,
            main_dir,
            remote_cmd,
            log,
            hosts=[remote_exec.find_host(lease.host)],
            queue_s=0,
            **({"budget_reservation": lease.reservation} if lease.reservation is not None else {}),
        )
        verdict = remote_verdict(outcome, log, summary)
        if verdict is None:
            notes = list(getattr(outcome, "notes", [])) or ["no_verdict"]
            raise runner.classify_infra(notes)("; ".join(notes)[:300])
        return verdict, "remote"

    def local(lease: dispatch.Lease) -> tuple[int, str]:
        env.update(lease.env())
        slot_fd = None
        if os.environ.get("TSFP_MERGE_URGENT") == "1":
            env["TSFP_BUILD_SLOT_HELD"] = "1"  # urgent: build now, no slot wait
        else:
            # The local build slot still caps paths not yet on the dispatcher.
            from tools.wt import buildslots

            slot_fd = buildslots.acquire(main_dir, log=log)
            # T3631: this slot also covers the `tools.wt run` child, avoiding a second slot wait.
            env["TSFP_BUILD_SLOT_HELD"] = "1"
        local_started = time.monotonic()
        target_activity: list[str] = []
        try:
            code = run_local(root, args.family, cmd, env, summary, target_activity)
        finally:
            if slot_fd is not None:
                os.close(slot_fd)
        if code == 0 and not os.environ.get("PYTEST_CURRENT_TEST"):
            remote_evidence.record_duration(spec.validation_key, time.monotonic() - local_started)
        record_cargo_target_lookup("\n".join(target_activity), code, command=cmd, env=env)
        record_admission(
            main_dir,
            {
                "decision": "local",
                "reason": lease.job.local_reason or "dispatch placed on the owner PC",
                "local_s": round(time.monotonic() - local_started, 1),
                "after_remote": False,
            },
        )
        jobevents.emit(
            "local-build",
            start=time.time() - (time.monotonic() - local_started),
            result="ok" if code == 0 else "fail",
            rc=code,
            target=args.target,
        )
        return code, "local"

    rules = dispatch_hosts.policy()
    return runner.run(
        disp,  # type: ignore[arg-type]
        spec,
        remote=remote,
        local=local,
        max_attempts=rules.max_attempts,
        mark_failed=runner.mark_degraded(main_dir),
        exclude=(dispatch.OWNER_PC,) if args.remote == "only" else (),
        timeout_s=rules.remote_only_wait_s if args.remote == "only" else None,
    )


def record_admission(main: Path, entry: dict[str, object]) -> None:
    try:
        from tools.wt import remote_fallback
    except ImportError:
        return
    remote_fallback.record(main, {"caller": "wt-build", **entry})


def primary_copy(root: Path, main: Path) -> bool:
    """T3461: True when this linked worktree runs an older, unmodified copy of the build and
    remote tools than the primary checkout (workers kept reporting dirty_tree after the fix
    landed because their worktrees carried the old remote_fallback)."""
    if root.resolve() == main.resolve() or os.environ.get("TSFP_WT_BUILD_LOCAL") == "1":
        return False
    names = ("tools/wt/build.py", "tools/wt/remote_fallback.py", "tools/wt/remote_exec.py")
    differs = False
    for name in names:
        mine, theirs = root / name, main / name
        if not theirs.is_file():
            return False
        if mine.is_file() and mine.read_bytes() == theirs.read_bytes():
            continue
        edited = subprocess.run(
            ["git", "-C", str(root), "diff", "--quiet", "HEAD", "--", name],
            capture_output=True,
            timeout=60,
        )
        if edited.returncode != 0:
            return False  # the worktree edits the tools itself: run its own copy
        last = subprocess.run(
            ["git", "-C", str(root), "log", "-1", "--format=%H", "HEAD", "--", name],
            capture_output=True,
            text=True,
            timeout=60,
        ).stdout.strip()
        behind = subprocess.run(
            ["git", "-C", str(root), "merge-base", "--is-ancestor", last, "refs/heads/main"],
            capture_output=True,
            timeout=60,
        )
        if not last or behind.returncode != 0:
            return False  # the branch changed the file itself (or main lacks it): keep ours
        differs = True
    return differs


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="tools.wt build",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "target",
        help="crate name, alias (ports, bevy, host, types, workspace), crate dir, or prebuilt "
        "(the game host via tools.wt prebuilt build; extra args after --)",
    )
    group = parser.add_mutually_exclusive_group()
    for mode in MODES:
        group.add_argument(f"--{mode}", dest="mode", action="store_const", const=mode)
    parser.set_defaults(mode="check")
    parser.add_argument("--release", action="store_true")
    parser.add_argument(
        "--cold", action="store_true", help="no CARGO_INCREMENTAL (merge-verify mode)"
    )
    parser.add_argument(
        "--remote",
        choices=("auto", "never", "only"),
        default=os.environ.get(REMOTE_ENV, "auto"),
        help="auto (default, env TSFP_BUILD_REMOTE): placed by tools.dispatch (T3885); never: "
        "an owner-PC-only job within its cap; only: never on the owner PC (exit 3 without a host)",
    )
    parser.add_argument("--no-cache", action="store_true", help="ignore a recorded pass")
    parser.add_argument("--print-key", action="store_true", help="print the input key and exit")
    parser.add_argument("--family", default=os.environ.get("TSFP_WT_FAMILY", "default"))
    parser.add_argument("--worktree", type=Path, default=None)
    return parser


def split_extra(argv: list[str]) -> tuple[list[str], list[str]]:
    """Own options before `--`, extra cargo arguments after it."""
    if "--" in argv:
        index = argv.index("--")
        return argv[:index], argv[index + 1 :]
    return argv, []


def main(argv: list[str] | None = None) -> int:
    from tools.process_priority import lower_priority
    from tools.wt import core

    lower_priority()

    own, extra = split_extra(list(sys.argv[1:] if argv is None else argv))
    args = build_parser().parse_args(own)
    if args.target == "prebuilt":
        # The game host artifact: already keyed by its build-input hash (T3448, remote-first).
        from tools.wt import prebuilt

        return prebuilt.cli(["build", *extra])
    root = core.worktree_root(args.worktree)
    main_dir = core.main_root(root)
    if primary_copy(root, main_dir):
        print(
            "tools.wt build: this worktree carries older build tools; running the primary's",
            file=sys.stderr,
            flush=True,
        )
        code = (
            "import runpy, sys; sys.path.insert(0, sys.argv.pop(1)); "
            "runpy.run_module('tools.wt.build', run_name='__main__', alter_sys=True)"
        )
        argv_rest = list(sys.argv[1:] if argv is None else argv)
        env = {**os.environ, "TSFP_WT_BUILD_LOCAL": "1"}
        os.execve(sys.executable, [sys.executable, "-c", code, str(main_dir), *argv_rest], env)
    crate = resolve(root, args.target)
    manifest = (crate / "Cargo.toml").relative_to(root.resolve()).as_posix()
    cmd = cargo_command(args.mode, manifest, args.release, extra)
    env = dict(os.environ)
    inc = incremental(args.mode, args.release, args.cold, env)
    env["TSFP_CARGO_INCREMENTAL"] = "1" if inc else "0"
    env["CARGO_INCREMENTAL"] = env["TSFP_CARGO_INCREMENTAL"]

    def log(text: str) -> None:
        print(text, file=sys.stderr, flush=True)

    cmd = compile_only_without_tests(cmd, crate, log)

    try:
        _normalise_manifest_paths(root, cmd)
    except ValueError as error:
        log(f"tools.wt build: {error}")
        return 2
    started = time.monotonic()
    key, parts = input_key(root, crate, cmd, env)
    if args.print_key:
        print(json.dumps({"key": key, "inputs": parts}, indent=1, sort_keys=True))
        return 0
    cacheable = args.mode in CACHEABLE and not args.no_cache
    if uncacheable_env(env, crate):
        cacheable = False
    hit = cached(main_dir, key) if cacheable else None
    shareable = shareable_verdict(args.mode, crate, cacheable)
    if hit:
        from tools.cache_telemetry import record_lookup

        record_lookup("build-verdict", True)
        for line in hit.get("summary", []):
            print(line)
        sys.stdout.flush()
        log(
            f"tools.wt build: cached pass {key} ({hit.get('mode')} {hit.get('target')} on "
            f"{hit.get('host')} at {hit.get('at')}, took {hit.get('seconds')} s; "
            "--no-cache reruns)"
        )
        jobevents.emit("cache-hit", start=time.time(), result="hit", target=args.target)
        return 0
    if shareable:
        with shared_artifacts.build_verdict_lock(key) as locked:
            shared_hit = shared_verdict(key)
            if shared_hit is not None:
                from tools.cache_telemetry import record_lookup

                record_lookup("build-verdict", True)
                record(main_dir, key, shared_hit)
                for line in shared_hit.get("summary", []):
                    print(line)
                sys.stdout.flush()
                log(
                    f"tools.wt build: shared cached pass {key} "
                    f"(from {shared_hit.get('host')}, took {shared_hit.get('seconds')} s)"
                )
                return 0
            if not locked:
                log(
                    "tools.wt build: shared verdict lock unavailable; "
                    "proceeding without single-flight"
                )
            from tools.cache_telemetry import record_lookup

            record_lookup("build-verdict", False)
            return run_build_miss(
                root,
                main_dir,
                crate,
                args,
                env,
                cmd,
                key,
                parts["key_components"],
                inc,
                cacheable,
                True,
                started,
                log,
            )
    if cacheable:
        from tools.cache_telemetry import record_lookup

        record_lookup("build-verdict", False)
    return run_build_miss(
        root,
        main_dir,
        crate,
        args,
        env,
        cmd,
        key,
        parts["key_components"],
        inc,
        cacheable,
        False,
        started,
        log,
    )


def core_main(root: Path) -> Path:
    from tools.wt import core

    return core.main_root(root)


def remote_command(
    root: Path,
    main: Path,
    crate: Path,
    cmd: list[str],
    env: dict[str, str],
    policy: str,
    log: Callable[[str], None],
) -> str | None:
    """The command a verify host runs, or None when this build must stay local.

    T3501: a c-diff build with the private lift here ships the lifted bundle instead (built
    once per inputs key and lift hash on this host); without a bundle it stays local, since the
    host would silently build without the lifted reference."""
    if policy == "never":
        return None
    cmd = list(cmd)
    try:
        _normalise_manifest_paths(root, cmd)
    except ValueError as error:
        log(f"tools.wt build: remote manifest is not portable ({error}), building locally")
        return None
    if not cdiff_bundle.is_cdiff(crate) or cdiff_bundle.lift_dir(root, env) is None:
        return shlex.join(cmd)
    try:
        bundle = cdiff_bundle.ensure(root, main, env, log=log)
    except (cdiff_bundle.BundleError, OSError, subprocess.SubprocessError) as error:
        log(f"tools.wt build: no c-diff lifted bundle ({str(error)[:200]}), building locally")
        return None
    return cdiff_bundle.remote_command(cmd, bundle, env)


def _normalise_manifest_paths(root: Path, command: list[str]) -> None:
    """Rewrite absolute in-repository Cargo manifests to portable repo-relative paths.

    Worker commands sometimes pass a manifest resolved from their worktree. Remote hosts check
    out the same repository content at a different path, so preserving that absolute path makes
    an otherwise portable build appear local-only. A manifest outside this repository remains
    non-portable and is rejected.
    """
    repo = root.resolve()
    index = 0
    while index < len(command):
        word = command[index]
        if word == "--manifest-path":
            value_index = index + 1
            if value_index >= len(command):
                raise ValueError("--manifest-path has no value")
            value = command[value_index]
        elif word.startswith("--manifest-path="):
            value_index = index
            value = word.partition("=")[2]
        else:
            index += 1
            continue
        path = Path(value)
        if path.is_absolute():
            try:
                relative = path.resolve().relative_to(repo)
            except ValueError as error:
                raise ValueError(f"{value} is outside the worktree") from error
            portable = relative.as_posix()
            if value_index == index:
                command[index] = f"--manifest-path={portable}"
            else:
                command[value_index] = portable
        index = value_index + 1


def _local_full(main: Path) -> bool:
    try:
        from tools.wt import remote_fallback
    except ImportError:
        return False
    return remote_fallback.local_slots_full(main)


if __name__ == "__main__":
    sys.exit(main())
