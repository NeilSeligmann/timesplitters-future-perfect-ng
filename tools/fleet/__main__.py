"""`python3 -m tools.fleet`: controller daemon, status, detectors, comparison and provisioning."""

from __future__ import annotations

import argparse
import os
import sys
from collections.abc import Sequence

from tools.fleet import compare, controller, detectors, provision, replay_registry, scale, status
from tools.fleet.config import ConfigError, ConfigWatcher, main_checkout
from tools.fleet.core import local_runner


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python3 -m tools.fleet", description=__doc__)
    parser.add_argument(
        "--root", help="main checkout (default: detected; TSFP_FLEET_ROOT overrides)"
    )
    sub = parser.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="controller daemon (dry-run unless --live)")
    run.add_argument(
        "--live", action="store_true", help="execute actions (default: record decisions only)"
    )
    run.add_argument("--once", action="store_true", help="one tick, then exit")
    run.add_argument(
        "--interval", type=float, help="seconds between ticks (default: fleet.json tick_seconds)"
    )

    sub.add_parser("check-config", help="validate tools/data/fleet.json and remote_hosts.json")
    sub.add_parser("status", help="probe all hosts and print the status lines")
    sub.add_parser("detect", help="run every health detector once and print the findings")
    sub.add_parser(
        "autoscale", help="print the scaling decision per host from the current status file"
    )

    cmp = sub.add_parser("compare", help="compare the dry-run controller with the live sup-loop")
    cmp.add_argument("--hours", type=float, default=1.0)

    prov = sub.add_parser("provision", help="make a host identical (checks only unless --apply)")
    provision.add_arguments(prov)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    from tools.process_priority import lower_priority

    lower_priority()
    args = build_parser().parse_args(argv)
    root = main_checkout() if not args.root else __import__("pathlib").Path(args.root)
    watcher = ConfigWatcher(root)
    try:
        config = watcher.current()
    except ConfigError as error:
        print(f"fleet: {error}", file=sys.stderr)
        return 2
    if args.command == "check-config":
        if not replay_registry.check_fleet():
            print(
                "fleet: replay fields in fleet.json drift from remote_hosts.json; "
                "run python3 -m tools.fleet.replay_registry --write",
                file=sys.stderr,
            )
            return 2
        print(
            f"ok: {len(config.hosts)} hosts, {len(config.jobs)} jobs, codexapi "
            f"{config.codexapi_dir}"
        )
        return 0
    if args.command == "provision":
        return provision.run_cli(args, config)
    if args.command == "status":
        sys.stdout.write(status.format_status(status.collect(config, local_runner(config.root))))
        return 0
    if args.command == "detect":
        runner = local_runner(config.root)
        statuses = status.collect(config, runner)
        import time

        ctx = detectors.default_context(
            config, runner, statuses, time.time(), config.state_dir / "health-state"
        )
        for finding in detectors.run_all(ctx):
            print(finding.line())
        return 0
    if args.command == "autoscale":
        statuses = status.parse_status(status.read_status(config))
        state = scale.AutoscaleState.load(config.codexapi_dir / "autoscale-state.json", config)
        for item in scale.autoscale(config, statuses, state, 100, 1e12):
            print(f"{item.host:9} {item.old}->{item.new} {item.reason}")
        return 0
    if args.command == "compare":
        verdicts = compare.run_compare(config, args.hours)
        for verdict in verdicts:
            print(verdict.line())
        return 0 if all(v.agree for v in verdicts) else 1
    ctl = controller.Controller(watcher, dry_run=not args.live)
    instance = controller.single_instance(ctl)
    inherited_fd = os.environ.pop("TSFP_FLEET_LOCK_FD", None)
    inherited = False
    if inherited_fd is not None:
        try:
            inherited = instance.adopt(int(inherited_fd))
        except ValueError:
            inherited = False
    if not inherited and not instance.acquire():
        print("fleet: another controller of this mode already holds the lock", file=sys.stderr)
        return 3
    try:
        if args.once:
            for decision in ctl.tick():
                print(decision.line())
            return 0

        def restart_with_current_code() -> None:
            fd = instance.prepare_exec()
            env = os.environ.copy()
            env["TSFP_FLEET_LOCK_FD"] = str(fd)
            os.execvpe(
                sys.executable,
                [sys.executable, "-m", "tools.fleet", *sys.argv[1:]],
                env,
            )

        return controller.run_forever(
            ctl,
            config.state_dir / controller.STOP_NAME,
            args.interval,
            on_code_update=restart_with_current_code,
        )
    finally:
        instance.release()


if __name__ == "__main__":
    raise SystemExit(main())
