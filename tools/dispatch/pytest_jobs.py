"""Pytest sets as dispatched jobs (`python3 -m tools.dispatch pytest`, T3885).

Routing: a test file listed in `tools/data/git_repo_tests.json` (it needs the owner PC's
checkout, inputs or toolchain; measured per file) or learned below runs as an owner-PC job with
at most LOCAL_WORKERS workers; the rest is one `pytest` job placed like any other (hosts with
the `pytest` capability). Both groups run at once.

A remote run that fails only in test nodes which then pass on the owner PC failed for the exec
host's environment, not the code: those nodes (at most CONFIRM_MAX) are re-run once on the
owner PC under their own placement, the files are remembered in
`<dispatch state>/remote-incompatible.json` so later sets route them to the owner PC at once,
and a `remote_env_failure` event names them. A node that fails on the owner PC too is the
verdict. There is no other local re-run.
"""

from __future__ import annotations

import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time
from collections.abc import Callable
from pathlib import Path

from tools.dispatch import owner_skips, queue, runner

TAIL_LINES = 4000
"""Remote pytest log lines brought back: the whole -rfE summary (main_guard parses it)."""
LOCAL_WORKERS = 4
"""xdist workers of a pytest job on the owner PC (the enforcement limit)."""
CONFIRM_MAX = 60
"""Remote-only failures confirmed on the owner PC at most; more is a verdict as it stands."""
LEARNED = "remote-incompatible.json"
NODE = re.compile(r"^(?:FAILED|ERROR) (tests/[^\s:]+\.py(?:::[^\s\[@]+)?)", re.M)
NODE_MARK = re.compile(r"^(FAILED|ERROR) ", re.M)

Say = Callable[[str], object]


def run_command(command: list[str], env: dict[str, str], timeout: int) -> int:
    """Run a pytest job placed on the owner PC (its env carries the placement token)."""
    return subprocess.run(command, env=env, check=False, timeout=timeout).returncode


def exec_host_env(environ: dict[str, str] | None = None) -> dict[str, str]:
    """The project toolchain of the exec host a pytest job runs on (tests/conftest.py applies it).

    MEASURED 2026-10-28: on the laptop the suite found no `rustc` (the bare rustup proxy without
    RUSTUP_HOME), so every input-hash test failed there. The host's `toolchain_dir` (registry,
    the parent of its rustup and cargo homes) is exported when the job has none."""
    env = dict(os.environ) if environ is None else environ
    host = env.get("TSFP_REMOTE_EXEC_HOST", "")
    if not host or env.get("RUSTUP_HOME"):
        return {}
    from tools import host_registry

    try:
        base = Path(host_registry.host_key(host, "toolchain_dir")).expanduser()
    except host_registry.RegistryError:
        return {}
    rustup, cargo = base / "rustup", base / "cargo"
    if not rustup.is_dir():
        return {}
    found = {"RUSTUP_HOME": str(rustup), "RUSTUP_AUTO_INSTALL": "0"}
    if cargo.is_dir():
        found["CARGO_HOME"] = str(cargo)
        found["PATH"] = f"{cargo / 'bin'}{os.pathsep}{env.get('PATH', '')}"
    return found


def learned(base: Path) -> dict[str, str]:
    try:
        data = json.loads((base / LEARNED).read_text())
    except (OSError, ValueError):
        return {}
    return {str(k): str(v) for k, v in data.items()} if isinstance(data, dict) else {}


def remember(base: Path, files: dict[str, str]) -> None:
    merged = {**learned(base), **files}
    tmp = base / f".{LEARNED}.{os.getpid()}.tmp"
    tmp.write_text(json.dumps(merged, indent=1, sort_keys=True) + "\n")
    tmp.replace(base / LEARNED)


def split(tests: list[str], root: Path, base: Path) -> tuple[list[str], list[str]]:
    """(placeable tests, owner-PC tests): git_repo_tests.json (T3782) plus learned files."""
    from tools.wt import remote_fallback

    known = learned(base)
    placeable: list[str] = []
    local: list[str] = []
    for test in tests:
        owner_only = test.split("::")[0] in known or remote_fallback.names_git_repo_test(
            [test], root
        )
        (local if owner_only else placeable).append(test)
    return placeable, local


def remote_hosts() -> list[str]:
    """Registry hosts with the `pytest` capability."""
    from tools.wt import remote_exec

    return [spec.name for spec in remote_exec.load_registry() if "pytest" in spec.capabilities]


def command(workers: int, per_test_timeout: int, tests: list[str]) -> list[str]:
    return [
        sys.executable,
        "-m",
        "pytest",
        "-q",
        "-p",
        "no:cacheprovider",
        "-rfEs",
        "-n",
        str(workers),
        "--timeout",
        str(per_test_timeout),
        *tests,
    ]


MIN_WORKERS = 2.0
"""A pytest job runs with fewer workers, down to this, rather than wait for a full host."""


def lease_workers(lease: queue.Lease) -> int:
    """xdist workers for the cores the placement granted (at least 1)."""
    return max(1, int(lease.cores or lease.job.cores))


def failed_nodes(output: str) -> list[str]:
    return sorted(set(NODE.findall(output)))


def run(
    tests: list[str],
    *,
    workers: int,
    per_test_timeout: int,
    timeout: int,
    label: str,
    root: Path,
    main: Path,
    say: Say,
) -> int:
    """Run `tests` as dispatched jobs; the worst exit code of the groups."""
    from tools import dispatch
    from tools.dispatch import hosts as host_view

    rules = host_view.policy()
    base = queue.state_dir()
    placeable, local_tests = split(list(tests), root, base)
    reason = "needs the owner PC (tools/data/git_repo_tests.json or learned)"
    if placeable and not remote_hosts():
        # No remote host has the pytest capability: one owner-PC job instead of two in a row.
        placeable, local_tests = [], placeable + local_tests
        reason = "no host has the pytest capability"
    local_workers = min(workers, LOCAL_WORKERS)
    # Two sessions of one set on the owner PC at once collided on shared test state (two
    # load-dependent failures in the 2026-10-28 dry run, both pass alone): take turns.
    owner_pc_turn = threading.Lock()

    def owner_pc_job(nodes: list[str], why: str, tag: str) -> int:
        spec = dispatch.job(
            "pytest",
            float(local_workers),
            local_only=True,
            local_reason=why,
            label=f"{label} {tag} ({len(nodes)})",
            min_cores=MIN_WORKERS,
        )

        def local(lease: queue.Lease) -> int:
            env = {**os.environ, **lease.env()}
            with owner_pc_turn:  # one owner-PC session of this set at a time (shared state)
                return run_command(
                    command(lease_workers(lease), per_test_timeout, nodes), env, timeout
                )

        return runner.run(
            dispatch.default(main, log=say),
            spec,
            remote=local,
            local=local,
            max_attempts=rules.max_attempts,
        )

    def placed_job(nodes: list[str]) -> int:
        spec = dispatch.job(
            "pytest",
            float(workers),
            label=f"{label} ({len(nodes)})",
            min_cores=MIN_WORKERS,
            commands=[shlex.join(command(workers, per_test_timeout, nodes))],
            root=root,
        )

        def remote(lease: queue.Lease) -> int:
            call = runner.remote_commands(  # sized to the cores the placement granted
                [shlex.join(command(lease_workers(lease), per_test_timeout, nodes))],
                root=root,
                main=main,
                log=say,
                command_timeout_s=timeout,
                tail_lines=TAIL_LINES,
            )
            outcome = call(lease)
            skipped = owner_skips.skipped_files(outcome.output)
            if outcome.status == "pass" and skipped:
                # Rule 48: an owner-input skip is not a pass; the owner PC has them.
                say(
                    f"dispatch pytest: {len(skipped)} file(s) skipped on {lease.host} (owner input)"
                )
                dispatch.default(main, log=say).event(
                    "owner_input_skip", host=lease.host, files=skipped
                )
                return owner_pc_job(skipped, f"owner-input skips on {lease.host}", "owner-skip")
            failed = failed_nodes(outcome.output) if outcome.status != "pass" else []
            if outcome.status == "pass" or not failed or len(failed) > CONFIRM_MAX:
                if outcome.output:
                    print(outcome.output, flush=True)
                return 0 if outcome.status == "pass" else int(outcome.returncode or 1)
            # The remote verdict lines are not printed as FAILED/ERROR: only the owner-PC
            # confirmation below decides (its own output carries the real failure lines).
            print(NODE_MARK.sub(r"REMOTE-\1 ", outcome.output), flush=True)
            say(f"dispatch pytest: {len(failed)} failure(s) on {lease.host}, confirming here")
            confirmed = owner_pc_job(
                failed, f"confirm remote failures from {lease.host}", "confirm"
            )
            if confirmed != 0:
                return confirmed
            files = {node.split("::")[0]: f"failed on {lease.host}, passed on the owner PC"
                     for node in failed}  # fmt: skip
            remember(base, {k: f"{v} ({time.strftime('%Y-%m-%d')})" for k, v in files.items()})
            dispatch.default(main, log=say).event(
                "remote_env_failure", host=lease.host, files=sorted(files)
            )
            return 0

        def local(lease: queue.Lease) -> int:
            env = {**os.environ, **lease.env()}
            with owner_pc_turn:
                workers_here = min(lease_workers(lease), local_workers)
                started = time.monotonic()
                code = run_command(command(workers_here, per_test_timeout, nodes), env, timeout)
                if code == 0 and not os.environ.get("PYTEST_CURRENT_TEST"):
                    from tools.dispatch import remote_evidence

                    remote_evidence.record_duration(spec.validation_key, time.monotonic() - started)
                return code

        return runner.run(
            dispatch.default(main, log=say),
            spec,
            remote=remote,
            local=local,
            max_attempts=rules.max_attempts,
            mark_failed=runner.mark_degraded(main),
        )

    jobs: list[Callable[[], int]] = []
    if placeable:
        jobs.append(lambda: placed_job(placeable))
    if local_tests:
        jobs.append(lambda: owner_pc_job(local_tests, reason, "local"))
    codes = [0] * len(jobs)

    def one(index: int) -> None:
        try:
            codes[index] = jobs[index]()
        except (runner.DispatchError, TimeoutError) as error:
            say(str(error))
            codes[index] = 3

    threads = [
        threading.Thread(target=one, args=(index,), daemon=True) for index in range(len(jobs))
    ]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    say(f"dispatch pytest: rc={codes} ({len(placeable)} placeable, {len(local_tests)} owner-PC)")
    return max(codes, default=0)
