"""`python3 -m tools.dispatch submit|status|hosts` (T3885)."""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path

from tools.dispatch import hosts as host_view
from tools.dispatch import placement, queue, runner
from tools.dispatch.model import KIND_CAPABILITY, JobSpec


def _say(text: str) -> None:
    print(text, file=sys.stderr, flush=True)


def _main() -> Path:
    from tools.datadir import main_root

    return main_root(Path.cwd())


def run_command(command: list[str], env: dict[str, str], timeout: int) -> int:
    """Run a job placed on the owner PC (its env carries the placement token)."""
    return subprocess.run(command, env=env, check=False, timeout=timeout).returncode


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python3 -m tools.dispatch", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    submit = sub.add_parser("submit", help="queue a job, run it where it is placed, return its rc")
    submit.add_argument("--kind", required=True, choices=sorted(KIND_CAPABILITY))
    submit.add_argument("--cores", type=float, required=True, help="CPU the job uses")
    submit.add_argument("--mem-mb", type=int, help="peak RSS (default: the kind's registry price)")
    submit.add_argument("--owner-inputs", action="store_true", help="needs disc/recordings/saves")
    submit.add_argument("--display", action="store_true", help="needs the owner's display")
    submit.add_argument("--gpu", action="store_true")
    submit.add_argument("--local-only", metavar="REASON", help="inputs only the owner PC has")
    submit.add_argument(
        "--est", type=float, default=0.0, help="expected seconds (0 = unknown/local)"
    )
    submit.add_argument("--priority", type=int, default=0)
    submit.add_argument("--label", default="")
    submit.add_argument("--timeout", type=int, default=1800, help="command timeout seconds")
    submit.add_argument("--wait-timeout", type=float, help="give up after this long in the queue")
    submit.add_argument("cmd", nargs=argparse.REMAINDER, help="-- command ...")
    tests = sub.add_parser(
        "pytest",
        help="run pytest test paths as dispatched jobs: tests that need the checkout's repository "
        "on the owner PC (git_repo_tests.json), the rest placed like any job",
    )
    tests.add_argument("--workers", type=int, default=8, help="xdist workers of the placed job")
    tests.add_argument("--per-test-timeout", type=int, default=180)
    tests.add_argument("--timeout", type=int, default=1800, help="command timeout seconds")
    tests.add_argument("--label", default="pytest")
    tests.add_argument("tests", nargs="+", help="test files or node ids (repo-relative)")
    status = sub.add_parser("status", help="queue, live placements and the owner-PC share")
    status.add_argument("--hours", type=float, default=1.0)
    hosts = sub.add_parser("hosts", help="the registry view and where a job would go now")
    hosts.add_argument("--kind", choices=sorted(KIND_CAPABILITY), default="cargo")
    hosts.add_argument("--cores", type=float, default=8.0)
    hosts.add_argument("--validation-key", default="", help="validated command-family key")
    hosts.add_argument("--est-seconds", type=float, default=0.0, help="expected job duration")
    return parser


def spec_of(args: argparse.Namespace) -> JobSpec:
    mem = args.mem_mb if args.mem_mb is not None else host_view.default_mem_mb(args.kind)
    return JobSpec(
        kind=args.kind,
        cores=args.cores,
        mem_mb=mem,
        owner_inputs=args.owner_inputs,
        display=args.display,
        gpu=args.gpu,
        local_only=bool(args.local_only),
        local_reason=args.local_only or "",
        est_seconds=args.est,
        priority=args.priority,
        label=args.label or shlex.join(args.cmd)[:80],
    )


def submit(args: argparse.Namespace) -> int:
    from tools import dispatch

    command = args.cmd[1:] if args.cmd[:1] == ["--"] else args.cmd
    if not command:
        _say("dispatch submit: no command (use: submit --kind K --cores N -- cmd ...)")
        return 2
    args.cmd = command
    spec = spec_of(args)
    main = _main()
    root = Path.cwd()
    from dataclasses import replace

    from tools.dispatch import remote_evidence

    key = remote_evidence.command_key([shlex.join(command)], root)
    spec = replace(spec, validation_key=key, est_seconds=args.est or remote_evidence.estimate(key))
    rules = host_view.policy()
    disp = dispatch.default(main, log=_say)

    def local(lease: queue.Lease) -> int:
        env = {**os.environ, **lease.env()}
        import time

        started = time.monotonic()
        code = run_command(command, env, args.timeout)
        if code == 0 and not os.environ.get("PYTEST_CURRENT_TEST"):
            remote_evidence.record_duration(key, time.monotonic() - started)
        return code

    remote_call = runner.remote_commands(
        [shlex.join(command)], root=root, main=main, log=_say, command_timeout_s=args.timeout
    )

    def remote(lease: queue.Lease) -> int:
        outcome = remote_call(lease)
        if outcome.output:
            print(outcome.output)
        return 0 if outcome.status == "pass" else int(outcome.returncode or 1)

    try:
        rc = runner.run(
            disp,
            spec,
            remote=remote,
            local=local,
            max_attempts=rules.max_attempts,
            mark_failed=runner.mark_degraded(main),
            timeout_s=args.wait_timeout,
        )
    except (runner.DispatchError, TimeoutError) as error:
        _say(str(error))
        return 3
    _say(f"dispatch: rc={rc}")
    return rc


def run_pytest(args: argparse.Namespace) -> int:
    """T3885: the landing's mapped tests as dispatched jobs (`tools.dispatch.pytest_jobs`)."""
    from tools.dispatch import pytest_jobs

    return pytest_jobs.run(
        list(args.tests),
        workers=args.workers,
        per_test_timeout=args.per_test_timeout,
        timeout=args.timeout,
        label=args.label,
        root=Path.cwd(),
        main=_main(),
        say=_say,
    )


def status(args: argparse.Namespace) -> int:
    disp = queue.Dispatcher(view=lambda _c, _p: [])
    since = time.time() - args.hours * 3600
    rows = queue.events(disp.base, since)
    leases = disp.leases()
    report = {
        "queue": [
            {
                "id": row["id"],
                "kind": row["job"]["kind"],
                "label": row["job"].get("label", ""),
                "waiting_s": round(time.time() - float(row["since"]), 1),
                "exclude": row.get("exclude", []),
            }
            for row in disp.waiters()
        ],
        "active": [
            {key: row[key] for key in ("id", "host", "kind", "label", "cores", "mem_mb", "pid")}
            for row in leases
        ],
        "window_hours": args.hours,
        **queue.share(rows, time.time(), live={str(row["id"]) for row in leases}),
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


def hosts(args: argparse.Namespace) -> int:
    disp = queue.Dispatcher(view=lambda _c, _p: [])
    committed, pids = disp.commitments()
    view = host_view.build_view(
        _main(), disp.base, committed=committed, foreign=host_view.foreign_budget(pids)
    )
    job = JobSpec(
        args.kind,
        args.cores,
        host_view.default_mem_mb(args.kind),
        validation_key=args.validation_key,
        est_seconds=args.est_seconds,
    )
    rules = host_view.policy()
    decision = placement.place(job, view, remote_down_after_s=rules.remote_down_after_s)
    print(
        json.dumps(
            {
                "hosts": [{**host.to_json(), "score": placement.score(job, host)} for host in view],
                "job": host_view.describe(job),
                "would_place_on": decision.host,
                "refused": list(decision.refused),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    commands = {"submit": submit, "pytest": run_pytest, "status": status, "hosts": hosts}
    return commands[args.command](args)
