"""CLI: python3 -m tools.wt {bootstrap,doctor,env,run,build,display,prebuilt,sweep}."""

from __future__ import annotations

import argparse
import math
import os
import shlex
import sys
from pathlib import Path

from tools.wt import core, display, safety, scratch, sparse


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="tools.wt", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name, text in (
        ("bootstrap", "link gitignored inputs and write the cache env"),
        ("doctor", "check a worktree and list what is missing"),
    ):
        item = sub.add_parser(name, help=text)
        item.add_argument(
            "--worktree", type=Path, default=None, help="worktree path (default: cwd's)"
        )
        item.add_argument(
            "--files-dir",
            type=Path,
            default=os.environ.get("TSFP_FILES_DIR"),
            help="owner bios/mcpx/hdd dir",
        )
        if name == "bootstrap":
            item.add_argument(
                "--family",
                default=os.environ.get("TSFP_WT_FAMILY", "default"),
                help="shared cargo target family (branch family; env TSFP_WT_FAMILY)",
            )
            item.add_argument("--dry-run", action="store_true")
    inventory = sub.add_parser(
        "inventory", help="dry-check expensive artifact recipes; never rerun"
    )
    inventory.add_argument("--root", type=Path)
    sub.add_parser("env", help="print the shell exports").add_argument(
        "--worktree", type=Path, default=None
    )
    run = sub.add_parser("run", help="run CMD with the shared cache environment")
    run.add_argument("--worktree", type=Path, default=None)
    run.add_argument("--family", default=os.environ.get("TSFP_WT_FAMILY", "default"))
    run.add_argument("cmd", nargs=argparse.REMAINDER)
    disp = sub.add_parser(
        "display", help="run CMD under a private Xvfb (DISPLAY set, torn down after)"
    )
    disp.add_argument(
        "--gpu",
        action="store_true",
        help="headless weston + Xwayland on the real GPU (Vulkan/RADV presents; Xvfb has no DRI3)",
    )
    disp.add_argument("--size", default=None, help="screen WxH (default 1280x720)")
    disp.add_argument("cmd", nargs=argparse.REMAINDER)
    prebuilt = sub.add_parser(
        "prebuilt", help="build, select, and collect shared primary-commit hosts"
    )
    prebuilt.add_argument("prebuilt_args", nargs=argparse.REMAINDER)
    build = sub.add_parser(
        "build", help="T3472: the one cargo entry point (input-hash cache, canonical, slots)"
    )
    build.add_argument("build_args", nargs=argparse.REMAINDER)
    cache_keys = sub.add_parser(
        "cache-keys", help="check host-independent cache keys or explain a key mismatch"
    )
    cache_keys.add_argument("--check", action="store_true")
    cache_keys.add_argument("--why-miss", nargs=2, metavar=("RUN_A.json", "RUN_B.json"))
    replay = sub.add_parser("replay", help="serialize GPU replays and reuse cached verdicts")
    replay.add_argument("replay_args", nargs=argparse.REMAINDER)
    control = sub.add_parser("control", help="inject input or capture a live --gpu display")
    control.add_argument("--socket", type=Path, required=True, help="WT_CONTROL_SOCKET path")
    actions = control.add_subparsers(dest="action", required=True)
    focus = actions.add_parser("focus", help="focus the launched game window")
    focus.add_argument("--title", default=None, help="optional xdotool window-title regex")
    focus.add_argument("--timeout", type=float, default=5.0)
    for action in ("key", "type"):
        item = actions.add_parser(action)
        item.add_argument("--title", default=None, help="optional xdotool window-title regex")
        item.add_argument("value", help="keysym/chord" if action == "key" else "text to type")
    for action in ("move", "click"):
        item = actions.add_parser(action)
        item.add_argument("x", type=int)
        item.add_argument("y", type=int)
        item.add_argument("--title", default=None, help="optional xdotool window-title regex")
        if action == "click":
            item.add_argument("--button", choices=("left", "middle", "right"), default="left")
    screenshot = actions.add_parser("screenshot", help="save a PNG screenshot of the game window")
    screenshot.add_argument("output", type=Path)
    screenshot.add_argument("--title", default=None, help="optional xdotool window-title regex")
    screenshot.add_argument("--timeout", type=float, default=5.0)
    remove = sub.add_parser("remove", help="inventory and quarantine a clean merged worktree")
    remove.add_argument("path", type=Path)
    remove.add_argument("--force-i-checked", action="store_true")
    remove.add_argument(
        "--scratch", action="store_true", help="drop a throwaway preflight/merge-task candidate"
    )
    sparse_cmd = sub.add_parser("sparse", help="manage role-based cone sparse checkout")
    sparse_sub = sparse_cmd.add_subparsers(dest="sparse_action", required=True)
    sparse_init = sparse_sub.add_parser("init", help="apply code or full profile")
    sparse_init.add_argument("--profile", choices=("code", "full"), default="code")
    sparse_init.add_argument("--worktree", type=Path, default=None)
    sparse_add = sparse_sub.add_parser("add", help="expand checkout to include a path")
    sparse_add.add_argument("path")
    sparse_add.add_argument("--worktree", type=Path, default=None)
    worktree = sub.add_parser("worktree", help="create a worktree with a role sparse profile")
    worktree_sub = worktree.add_subparsers(dest="worktree_action", required=True)
    worktree_add = worktree_sub.add_parser("add", help="create a code or full worktree")
    worktree_add.add_argument("path", type=Path)
    worktree_add.add_argument("ref", nargs="?", default="HEAD")
    worktree_add.add_argument("--branch", help="create and check out a new branch")
    worktree_add.add_argument("--detach", action="store_true")
    worktree_add.add_argument("--profile", choices=("code", "full"), default="code")
    gc = sub.add_parser("gc", help="list idle trees and expired trash (dry-run by default)")
    gc.add_argument("--idle-hours", type=float, default=24)
    gc.add_argument(
        "--ttl-hours",
        type=float,
        default=None,
        help="trash/empty scratch husk TTL (default 72 h; 6 h with --scratch)",
    )
    gc.add_argument("--trash-cap-gb", type=float, default=100, help="0 disables the trash size cap")
    gc.add_argument("--cache-idle-hours", type=float, default=2)
    gc.add_argument("--prune", action="store_true")
    gc.add_argument(
        "--prune-caches", action="store_true", help="delete eligible target/build caches"
    )
    gc.add_argument(
        "--scratch", action="store_true", help="collect marked merge/preflight scratch only"
    )
    sweep = sub.add_parser(
        "scratch-sweep", help="remove leaked preflight-/merge-task-/wc-cand- scratch dirs"
    )
    from tools.wt import scratch_sweep

    scratch_sweep.add_arguments(sweep)
    arch = sub.add_parser(
        "archive", help="T2438: archive idle trees (ref, patch, ignored data) then delete them"
    )
    arch.add_argument("--apply", action="store_true", help="archive, verify and delete")
    arch.add_argument("--idle-hours", type=float, default=24)
    arch.add_argument("--only", type=Path, action="append", help="restrict to these trees")
    arch.add_argument("--expire-claims", action="store_true", help="also reap claims silent >6 h")
    arch.add_argument(
        "--push-refs", action="store_true", help="also push refs/archive/* (owner-authorized only)"
    )
    rest = sub.add_parser("restore", help="T2438: recreate an archived tree")
    rest.add_argument("name")
    rest.add_argument("--path", type=Path, default=None, help="default: the original path")
    sub.add_parser("gc-install", help="install and enable the hourly user housekeeping timer")
    loop = sub.add_parser("hygiene-loop", help="run the hygiene loop (T2419), detached by ensure")
    loop.add_argument("--interval", type=int, default=600, help="seconds between passes")
    loop.add_argument("--cargo-idle-hours", type=float, default=6)
    loop.add_argument("--cargo-cap-gb", type=float, default=None)
    loop.add_argument("--once", action="store_true")
    cache_hygiene = sub.add_parser(
        "hygiene-cache", help="T3417 dry-run or apply replay and prebuilt cache retention"
    )
    cache_hygiene.add_argument("--replay-cap-gb", type=float, default=150.0)
    cache_hygiene.add_argument("--replay-keep", type=int, default=3)
    cache_hygiene.add_argument("--prebuilt-keep", type=int, default=12)
    cache_hygiene.add_argument("--apply", action="store_true", help="delete eligible cache entries")
    cargo_hygiene = sub.add_parser(
        "hygiene-cargo", help="T3422 dry-run or prune shared Cargo target profiles"
    )
    cargo_hygiene.add_argument("--cargo-idle-hours", type=float, default=6)
    cargo_hygiene.add_argument("--cargo-cap-gb", type=float, default=None)
    cargo_hygiene.add_argument(
        "--apply", action="store_true", help="delete eligible whole profiles"
    )
    gate_data_hygiene = sub.add_parser(
        "gate-data-gc", help="T3467 dry-run or apply gate snapshot and gate trace bulk retention"
    )
    from tools.wt import gate_data_gc

    gate_data_gc.add_arguments(gate_data_hygiene)
    retention = sub.add_parser(
        "replay-retention",
        help="T3467 owner policy in one command: replay runs, gate snapshots and gate trace bulk",
    )
    retention.add_argument("--apply", action="store_true", help="delete (default: dry-run)")
    retention.add_argument("--max-age-h", type=float, default=2.0, help="idle age of a raw run")
    retention.add_argument("--triage-h", type=float, default=24.0)
    retention.add_argument("--idle-h", type=float, default=2.0, help="idle age of a snapshot")
    retention.add_argument("--keep-hosts", type=int, default=1)
    retention.add_argument("--bulk-age-h", type=float, default=2.0)
    prune = sub.add_parser(
        "prune-remote-merged",
        help="T3514: delete merged task/codex/work branches on origin (dry run unless --apply)",
    )
    from tools.wt import prune_remote

    prune_remote.add_arguments(prune)
    sub.add_parser("envhook", help="PreToolUse Bash hook: shared cargo env in any worktree (T2424)")
    sub.add_parser("pyheal", help="reinstall the pinned uv interpreter if .venv dangles (T2424)")
    ensure = sub.add_parser(
        "hygiene-ensure", help="start the detached hygiene loop if it is not running"
    )
    ensure.add_argument("--cargo-idle-hours", type=float, default=6)
    ensure.add_argument("--cargo-cap-gb", type=float, default=None)
    return parser


def strip_dashes(cmd: list[str]) -> list[str]:
    return cmd[1:] if cmd and cmd[0] == "--" else cmd


DISPATCH_CORES = 8.0
"""Cores a heavy `tools.wt run` is priced at when CARGO_BUILD_JOBS is unset (T3885)."""


def dispatcher(main_dir: Path) -> object | None:
    """The work dispatcher (T3885); tests replace it with tools.dispatch.testing's fake.

    None under pytest (a test's subprocess `tools.wt run` never dispatches for real, rule 39)."""
    if os.environ.get("PYTEST_CURRENT_TEST"):
        return None
    from tools import dispatch

    return dispatch.default(main_dir, log=lambda text: print(text, file=sys.stderr, flush=True))


def dispatch_heavy(
    root: Path, main_dir: Path, cmd: list[str], env: dict[str, str]
) -> int | tuple[object, object] | None:
    """T3885: place a heavy command. Its exit code when it ran remotely; (dispatcher, lease)
    for an owner-PC placement (token in `env`, run it with `run_under_lease`); None to run it
    here as it is: already placed (a valid owner-PC token from `tools.wt build` or `tools.dispatch
    submit`), not the owner PC (an exec host), or under pytest."""
    from tools import local_game_slots
    from tools.dispatch import queue as dispatch_queue
    from tools.dispatch import runner
    from tools.dispatch.hosts import policy
    from tools.wt import remote_fallback

    token = env.get(dispatch_queue.TOKEN_ENV, "")
    if token and dispatch_queue.token_valid(token):
        return None
    if not local_game_slots.is_owner_pc(root, env):
        return None
    disp = dispatcher(main_dir)
    if disp is None:
        return None
    from tools import dispatch

    def say(text: str) -> None:
        print(text, file=sys.stderr, flush=True)

    command = shlex.join(cmd)
    portable = remote_fallback.remote_form(command, env, root=root) is not None
    spec = dispatch.job(
        "cargo",
        float(env.get("CARGO_BUILD_JOBS") or DISPATCH_CORES),
        local_only=not portable,
        local_reason="" if portable else "not portable",
        label=f"wt-run {command[:60]}",
    )
    call = runner.remote_commands([command], root=root, main=main_dir, log=say, env=env)

    def remote(lease: dispatch.Lease) -> int:
        outcome = call(lease)
        if outcome.output:
            print(outcome.output[-4000:], file=sys.stderr)
        return 0 if outcome.status == "pass" else int(outcome.returncode or 1)

    try:
        placed = runner.run(
            disp,  # type: ignore[arg-type]
            spec,
            remote=remote,
            local=lambda _lease: 0,
            max_attempts=policy().max_attempts,
            mark_failed=runner.mark_degraded(main_dir),
            hand_over_local=True,
        )
    except (runner.DispatchError, TimeoutError) as error:
        say(f"tools.wt run: {error}")
        return 3
    if isinstance(placed, dispatch.Lease):
        env.update(placed.env())
        return disp, placed
    return int(placed)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "worktree":
        main_dir = core.main_root(core.worktree_root(Path.cwd()))
        destination = args.path if args.path.is_absolute() else Path.cwd() / args.path
        command = ["worktree", "add"]
        if args.branch:
            command.extend(("-b", args.branch))
        if args.detach:
            command.append("--detach")
        command.extend((str(destination), args.ref))
        try:
            safety.git(main_dir, *command, timeout=600)
            sparse.init(destination, args.profile)
            return 0
        except safety.Unsafe as exc:
            print(f"refused: {exc}", file=sys.stderr)
            return 1
    if args.command == "sparse":
        root = core.worktree_root(args.worktree or Path.cwd())
        try:
            if args.sparse_action == "init":
                sparse.init(root, args.profile)
            else:
                sparse.add(root, args.path)
            return 0
        except safety.Unsafe as exc:
            print(f"refused: {exc}", file=sys.stderr)
            return 1
    if args.command == "cache-keys":
        from tools.wt import cache_keys as cache_key_tools

        cache_args = ["--check"] if args.check else []
        if args.why_miss:
            cache_args = ["--why-miss", *args.why_miss]
        return cache_key_tools.main(cache_args)
    if args.command == "prebuilt":
        from tools.wt import prebuilt as prebuilt_tools

        return prebuilt_tools.cli(strip_dashes(args.prebuilt_args))
    if args.command == "build":
        from tools.wt import build as build_tools

        return build_tools.main(args.build_args)
    if args.command == "replay":
        from tools.wt import replay as replay_tools

        return replay_tools.cli(strip_dashes(args.replay_args))
    if args.command == "control":
        from tools.wt.input import send_request

        request = {"op": args.action}
        if args.action == "focus":
            request.update(title=args.title, timeout=args.timeout)
        elif args.action in ("key", "type"):
            request["title"] = args.title
            request["key" if args.action == "key" else "text"] = args.value
        elif args.action == "move":
            request.update(x=args.x, y=args.y, title=args.title)
        elif args.action == "click":
            request.update(x=args.x, y=args.y, title=args.title, button=args.button)
        elif args.action == "screenshot":
            request.update(output=str(args.output), title=args.title, timeout=args.timeout)
        try:
            import json

            print(json.dumps(send_request(args.socket, request)))
            return 0
        except (OSError, RuntimeError, ValueError) as exc:
            print(f"control failed: {exc}", file=sys.stderr)
            return 1
    if args.command == "inventory":
        import json

        from tools.datadir import data_dir, recipe_inventory

        print(json.dumps(recipe_inventory(args.root or data_dir("")), indent=2))
        return 0
    if args.command == "display":
        cmd = strip_dashes(args.cmd)
        if not cmd:
            return 2
        if args.gpu:
            return display.run_with_gpu_display(cmd, args.size or "1280x720")
        return display.run_with_display(cmd, f"{args.size}x24" if args.size else "1280x720x24")
    if args.command == "envhook":
        from tools.wt import envhook

        return envhook.cli()
    if args.command == "pyheal":
        from tools.wt import pyheal

        result = pyheal.ensure(core.main_root(core.worktree_root(Path.cwd())))
        print(f"pyheal {result}")
        return 1 if result.startswith("failed") else 0
    if args.command == "gate-data-gc":
        from tools.wt import gate_data_gc

        return gate_data_gc.run_cli(args, core.main_root(core.worktree_root(Path.cwd())))
    if args.command == "replay-retention":
        from tools.wt import gate_data_gc, replay_gc

        main_dir = core.main_root(core.worktree_root(Path.cwd()))
        try:
            replay_gc.gc(
                main_dir, apply=args.apply, max_age_h=args.max_age_h, triage_h=args.triage_h
            )
            replay_gc.remote_staging_gc(apply=args.apply, triage_h=args.triage_h)
            gate_data_gc.run(
                main_dir,
                apply=args.apply,
                idle_h=args.idle_h,
                keep_hosts=args.keep_hosts,
                bulk_age_h=args.bulk_age_h,
            )
        except (OSError, ValueError, gate_data_gc.safety.Unsafe) as exc:
            print(f"replay-retention failed: {exc}", file=sys.stderr)
            return 1
        return 0
    if args.command == "prune-remote-merged":
        from tools.wt import prune_remote

        return prune_remote.run_cli(args, core.main_root(core.worktree_root(Path.cwd())))
    if args.command in ("hygiene-loop", "hygiene-ensure", "hygiene-cache", "hygiene-cargo"):
        from tools.wt import hygiene

        main_dir = core.main_root(core.worktree_root(Path.cwd()))
        if args.command == "hygiene-cache":
            try:
                hygiene.prune_replay_cache(
                    main_dir,
                    cap_gb=args.replay_cap_gb,
                    replay_keep=args.replay_keep,
                    prebuilt_keep=args.prebuilt_keep,
                    apply=args.apply,
                )
                return 0
            except (OSError, ValueError, hygiene.safety.Unsafe) as exc:
                print(f"hygiene-cache failed: {exc}", file=sys.stderr)
                return 1
        if args.command == "hygiene-cargo":
            try:
                hygiene.prune_cargo_target(
                    main_dir, args.cargo_idle_hours, args.cargo_cap_gb, apply=args.apply
                )
                return 0
            except (OSError, ValueError, hygiene.safety.Unsafe) as exc:
                print(f"hygiene-cargo failed: {exc}", file=sys.stderr)
                return 1
        if args.command == "hygiene-ensure":
            result = hygiene.ensure_loop(
                main_dir,
                cargo_idle_hours=args.cargo_idle_hours,
                cargo_cap_gb=args.cargo_cap_gb,
            )
            print(f"hygiene loop {result}")
            from tools.agents import loops

            try:  # T3845: every session start is a supervisor pass (finishes staged restarts)
                report = loops.ensure(main_dir)
                loops.write_findings(main_dir, report.findings)
                print(f"loops ensure: {len(report.actions)} action(s)")
            except Exception as exc:  # noqa: BLE001 - the session hook must not fail on it
                print(f"loops ensure failed: {exc}", file=sys.stderr)
            return 0
        return hygiene.run_loop(
            main_dir,
            args.interval,
            args.once,
            cargo_idle_hours=args.cargo_idle_hours,
            cargo_cap_gb=args.cargo_cap_gb,
        )
    if args.command in ("archive", "restore"):
        from tools.wt import archive

        main_dir = core.main_root(core.worktree_root(Path.cwd()))
        try:
            if args.command == "restore":
                archive.restore(main_dir, args.name, args.path)
                return 0
            if not math.isfinite(args.idle_hours) or args.idle_hours < 0:
                raise safety.Unsafe("hours must be finite and non-negative")
            if args.expire_claims:
                archive.expire_claims(main_dir, archive.CLAIM_EXPIRE_HOURS, args.apply)
            archive.run(
                main_dir,
                idle_hours=args.idle_hours,
                apply=args.apply,
                only=args.only,
                push_refs=args.push_refs,
            )
            return 0
        except (safety.Unsafe, archive.ArchiveError, OSError) as exc:
            print(f"refused: {exc}", file=sys.stderr)
            return 1
    if args.command == "scratch-sweep":
        from tools.wt import scratch_sweep

        return scratch_sweep.main(
            ["--hours", str(args.hours)] + (["--apply"] if args.apply else [])
        )
    if args.command in ("remove", "gc", "gc-install"):
        try:
            if args.command == "gc-install":
                try:
                    safety.install_gc_timer(Path.cwd())
                except safety.Unsafe as exc:
                    # No systemd (containers): fall back to the detached loop (T2419).
                    from tools.wt import hygiene

                    print(f"systemd timer unavailable ({exc}); using the background loop")
                    main_dir = core.main_root(core.worktree_root(Path.cwd()))
                    print(f"hygiene loop {hygiene.ensure_loop(main_dir)}")
                return 0
            if args.command == "remove":
                safety.remove(args.path, force_i_checked=args.force_i_checked, scratch=args.scratch)
                return 0
            if args.ttl_hours is None:
                args.ttl_hours = scratch.SCRATCH_TTL_HOURS if args.scratch else 72.0
            if any(
                not math.isfinite(h) or h < 0
                for h in (args.idle_hours, args.ttl_hours, args.cache_idle_hours)
            ):
                raise safety.Unsafe("hours must be finite and non-negative")
            if args.scratch:
                return scratch.gc(Path.cwd(), ttl_hours=args.ttl_hours, prune=args.prune)
            return safety.gc(
                Path.cwd(),
                idle_hours=args.idle_hours,
                ttl_hours=args.ttl_hours,
                prune=args.prune,
                cache_idle_hours=args.cache_idle_hours,
                prune_caches=args.prune_caches,
                trash_cap_gb=args.trash_cap_gb,
            )
        except (safety.Unsafe, OSError) as exc:
            print(f"refused: {exc}", file=sys.stderr)
            return 1
    root = core.worktree_root(args.worktree)
    files = Path(args.files_dir) if getattr(args, "files_dir", None) else None
    if args.command == "bootstrap":
        print("\n".join(core.bootstrap(root, files, args.family, args.dry_run)))
        return 0
    if args.command == "doctor":
        lines, bad = core.doctor(root, files)
        print("\n".join(lines))
        return 1 if bad else 0
    main_dir = core.main_root(root)
    if args.command == "env":
        print(core.env_script(root, main_dir, "default"), end="")
        return 0
    cmd = strip_dashes(args.cmd)
    if not cmd:
        return 2
    from tools import datadisk
    from tools.wt import hygiene

    datadisk.no_core_dumps()  # T3745: builds, tests and hosts run under tools.wt never dump cores
    try:
        hygiene.ensure_free(main_dir)
        hygiene.ensure_loop(main_dir)
    except (safety.Unsafe, OSError) as exc:
        print(f"refused: {exc}", file=sys.stderr)
        return 1
    from tools.wt import buildslots

    env = {**os.environ, **core.env_vars(root, main_dir, args.family, cmd)}
    lease: tuple[object, object] | None = None
    if buildslots.is_heavy(cmd):
        # T3885 (replaces the T3878 remote_fallback.admit path and its local confirmation): the
        # dispatcher places the build; a remote verdict returns here, an owner-PC placement
        # runs below as a child under its lease (token in env), the end logged on exit.
        placed = dispatch_heavy(root, main_dir, cmd, env)
        if isinstance(placed, int):
            return placed
        lease = placed
        core.apply_shared_sccache(root, main_dir, env)  # T3472: LAN-shared store when reachable
        from tools.wt import canonical_build

        # T3472: the slot BEFORE the canonical tree (T3469 order). A worker queued for a slot
        # while holding its family's tree lock stalled every other worker of the family.
        buildslots.acquire(main_dir)  # T3411: descriptor survives exec
        key = core.cargo_family_key(root, args.family, cmd)
        canon = canonical_build.prepare(root, main_dir, cmd, env, Path.cwd(), key)
        if canon is not None:  # T3432: fixed source path, own target, lock held through exec
            cmd, cwd, _lock = canon
            os.chdir(cwd)
        profile = core.cargo_profile_dir(Path(env["CARGO_TARGET_DIR"]), cmd)
        buildslots.acquire_profile_lock(main_dir, profile, shared=True)
        from tools.wt import sccache_shared

        sccache_shared.start_server(env)  # T3472 p3: the daemon must not inherit the lock fds
    if lease is not None:
        return run_under_lease(cmd, env, *lease)
    os.execvpe(cmd[0], cmd, env)
    return 0


def run_under_lease(cmd: list[str], env: dict[str, str], disp: object, lease: object) -> int:
    """T3885: run an owner-PC placement as a child (signals forwarded, the child dies with this
    process) and release the lease with its exit code, so the event log has the real end."""
    import signal
    import subprocess

    def die_with_parent() -> None:
        import ctypes

        ctypes.CDLL("libc.so.6", use_errno=True).prctl(1, signal.SIGTERM)  # PR_SET_PDEATHSIG

    child = subprocess.Popen(cmd, env=env, preexec_fn=die_with_parent)  # noqa: PLW1509
    forwarded = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
    previous = {
        sig: signal.signal(sig, lambda sig, _f: child.send_signal(sig)) for sig in forwarded
    }
    try:
        code = child.wait()
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        disp.release(lease, rc=child.returncode)  # type: ignore[attr-defined]
    return code if code >= 0 else 128 - code


if __name__ == "__main__":
    sys.exit(main())
