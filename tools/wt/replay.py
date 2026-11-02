"""Serialized, content-addressed GPU replay runs (T2443)."""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import re
import secrets
import shutil
import socket
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path

from tools import datadir, hostcaps, owner_files, prebuilt_select
from tools.agents import jobevents
from tools.cache_telemetry import record_lookup
from tools.wt import cache_keys, prebuilt


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def replay_key(
    host: Path,
    route_name: str,
    recording: Path,
    switches: str,
    capture_options: tuple[str, ...] = (),
    save_hash: str | None = None,
    disc_hash: str | None = None,
    display_backend: str = "gpu",
    host_input_hash: str | None = None,
    snapshot_attempt: str | None = None,
) -> tuple[str, dict]:
    fields = {
        "host_sha256": digest(host),
        "route": route_name,
        "recording_sha256": digest(recording),
        "switches": sorted({item.strip() for item in switches.split(",") if item.strip()}),
        "capture_options": list(capture_options),
        "save_sha256": save_hash,
        "disc_sha256": disc_hash,
        "display_backend": display_backend,
        "host_input_hash": host_input_hash,
    }
    if snapshot_attempt is not None:
        fields["snapshot_attempt"] = snapshot_attempt
    if "--paced" not in capture_options:
        # T2487: fast-forward is the default capture mode; paced keys are unchanged.
        fields["pacing"] = "fast-forward"
    if "--render" not in capture_options:
        # T3390: rust_trace's --render auto is headless (none) for fast-forward state captures.
        fields["render"] = "auto"
    key = cache_keys.make_key(
        "replay-capture",
        files={"recording": fields["recording_sha256"]},
        tools={"game-host": fields["host_sha256"]},
        env={},
        extra=fields,
    ).key
    return key, fields


def split_switches(switches: str) -> tuple[str, tuple[str, ...], bool]:
    """Separate host-only runtime switches from the Rust-port set for capture."""
    selected = {item.strip() for item in switches.split(",") if item.strip()}
    if "play" in selected:
        # T3592: `play` = what `play.sh default` runs (its --rust-ports, read from play.sh) plus the
        # host-only adopted defaults (REA AI, native inventory) the queue otherwise forces off.
        from tools.rust_trace import play_options

        selected.discard("play")
        selected.update(play_options()["ports"].removeprefix("--rust-ports=").split(","))
        selected.update(("rust-rea-ai", "rea-inventory-native"))
    use_rea_ai = "rust-rea-ai" in selected
    use_weapon_hands = "rust-weapon-hands" in selected
    use_inventory_native = "rea-inventory-native" in selected
    selected.difference_update(("rust-rea-ai", "rust-weapon-hands", "rea-inventory-native"))
    if not selected:
        raise ValueError("--switches must include a Rust port set")
    # The C host defaults REA AI on (and defaults its Rust root to 0x266250).
    # A replay's switch set must be authoritative: without this explicit off
    # flag, even `--switches off` fails after rust-ports overrides the default
    # root to off. The selected host-only switch opts it back in.
    host_args = (
        "--rust-rea-ai" if use_rea_ai else "--no-rust-rea-ai",
        *(("--rust-weapon-hands",) if use_weapon_hands else ()),
    )
    return ",".join(sorted(selected)), host_args, use_inventory_native


def cache_dir() -> Path:
    """The replay cache root; its `hostlock-*` dirs hold the replay queue (T2486)."""
    return Path(os.environ.get("TSFP_REPLAY_CACHE") or datadir.data_dir("wt-cache/replays"))


def replay_slots() -> int:
    """Replay queue capacity (T2486), independent from the hostlock global default."""
    try:
        slots = int(os.environ.get("TSFP_REPLAY_SLOTS", "3"))
    except ValueError as error:
        raise ValueError("TSFP_REPLAY_SLOTS must be a positive integer") from error
    if slots < 1:
        raise ValueError("TSFP_REPLAY_SLOTS must be a positive integer")
    return slots


def replay_priority(requested: str | None = None) -> str:
    """T3388: `high` (replay-system work) or `normal`; --priority, then TSFP_REPLAY_PRIORITY."""
    value = requested or os.environ.get("TSFP_REPLAY_PRIORITY") or "normal"
    if value not in ("normal", "high"):
        raise ValueError("replay priority must be normal or high")
    return value


def priority_markers(cache: Path) -> list[Path]:
    """Live priority markers; local PID identity or a foreign holder heartbeat decides."""
    directory = cache / "priority-active"
    live = []
    for marker in directory.glob("*.pid") if directory.is_dir() else ():
        try:
            record = json.loads(marker.read_text())
        except (OSError, ValueError):
            record = {}
        from tools.wt import lease

        if not lease.holder_alive(record)[0]:
            marker.unlink(missing_ok=True)
            continue
        live.append(marker)
    return live


def yield_to_priority(
    cache: Path, poll: float = 2.0, sleep: Callable[[float], None] = time.sleep
) -> float:
    """T3388: a normal replay waits while a high-priority replay runs; returns seconds waited."""
    waited = 0.0
    reported = False
    while priority_markers(cache):
        if not reported:
            print("replay submit: yielding to a high-priority replay", file=sys.stderr, flush=True)
            reported = True
        sleep(poll)
        waited += poll
    return waited


def host_lease(host: Path, capture_host: bool = False) -> contextlib.AbstractContextManager[Path]:
    """The shared-prebuilt lease; a T3740 capture host lives outside the cache and takes none."""
    return contextlib.nullcontext(host) if capture_host else prebuilt.lease(host)


def capture_renders_none(capture_options: tuple[str, ...] | list[str]) -> bool:
    """T3390: does rust_trace resolve this queue capture to --render none (headless, no GPU)?

    Mirrors rust_trace.render_mode for queue captures (native .rec routes, no screenshots)."""
    options = list(capture_options)
    render = options[options.index("--render") + 1] if "--render" in options else "auto"
    return render == "none" or (render == "auto" and "--paced" not in options)


def remote_visual(capture_options: tuple[str, ...] | list[str], require_remote: bool) -> bool:
    """Use a worker's offscreen Xvfb for screenshot captures before falling back locally.

    Visual captures used to run only on this machine; several paced ones overloaded the owner PC
    (I-T3823-1). A no-verdict falls back locally unless --remote requires a remote pass.
    Remote workers use Xvfb and never --gpu-display."""
    options = list(capture_options)
    visual = "--shot-at" in options or "--shot-at-route-end" in options
    return visual


def headless_replay_slots() -> int:
    """T3390: concurrent --render none replays per host (TSFP_REPLAY_HEADLESS_SLOTS, default
    CPU count / 4, at least 1). MEASURED: 16 at once on 32 cores gave 8.7x the throughput of one."""
    value = os.environ.get("TSFP_REPLAY_HEADLESS_SLOTS")
    if value is None:
        return max(1, hostcaps.effective_cpus() // 4)
    try:
        slots = int(value)
    except ValueError as error:
        raise ValueError("TSFP_REPLAY_HEADLESS_SLOTS must be a positive integer") from error
    if slots < 1:
        raise ValueError("TSFP_REPLAY_HEADLESS_SLOTS must be a positive integer")
    return slots


def replay_host_label(name: str) -> str:
    """Normalize configured replay host names to the fleet labels used by timing reports."""
    value = name.lower()
    for label in ("laptop", "server", "droplet", "deck"):
        if label in value:
            return label
    return "local"


SLIM_MARKER = ".slim.json"


def _raw_bulk(run_dir: Path) -> list[Path]:
    """Raw capture output already folded into trace.json: per-button guest dumps and frames."""
    found = []
    buttons = run_dir / "dump" / "buttons"
    if buttons.is_dir() and not buttons.is_symlink():
        found.extend(path for path in buttons.iterdir() if path.name.startswith("guestdump."))
    overlay = run_dir / "overlay"
    if overlay.is_dir() and not overlay.is_symlink():
        found.extend(
            path
            for path in overlay.iterdir()
            if path.name.startswith("overlay_") and path.suffix in (".png", ".yuv422p")
        )
    return [path for path in found if path.is_file() and not path.is_symlink()]


def slim_run(run_dir: Path) -> dict | None:
    """T3467: drop the raw dumps and frames of a finished capture once trace.json holds them.

    Keeps trace.json, call-args.trace, logs, manifests (dump/buttons.jsonl, overlay index), exit
    dumps, screenshots and seed saves, and writes `.slim.json` with what went. Returns None and
    changes nothing when the run has no trace.json."""
    if not (run_dir / "trace.json").is_file():
        return None
    removed = {"files": 0, "bytes": 0}
    for path in _raw_bulk(run_dir):
        try:
            removed["bytes"] += path.stat().st_size
            path.unlink()
            removed["files"] += 1
        except FileNotFoundError:
            continue
    marker = {"version": 1, "slimmed_at": int(time.time()), "removed": removed}
    (run_dir / SLIM_MARKER).write_text(json.dumps(marker, sort_keys=True) + "\n")
    return marker


def keep_raw_requested(keep_raw: bool = False) -> bool:
    return keep_raw or os.environ.get("TSFP_REPLAY_KEEP_RAW", "") not in ("", "0")


def trace_has_samples(trace_path: Path) -> bool:
    """Check the top-level samples array without loading a potentially huge trace."""
    if not trace_path.is_file():
        return False
    marker = b'"samples"'
    overlap = b""
    try:
        with trace_path.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                data = overlap + chunk
                position = data.find(marker)
                if position >= 0:
                    tail = data[position + len(marker) :]
                    for expected in (b":", b"["):
                        while not tail.strip():
                            tail = stream.read(1024 * 1024)
                            if not tail:
                                return False
                        tail = tail.lstrip(b" \t\r\n")
                        if tail[:1] != expected:
                            return False
                        tail = tail[1:]
                    while not tail.strip():
                        tail = stream.read(1024 * 1024)
                        if not tail:
                            return False
                    return tail.lstrip(b" \t\r\n")[:1] != b"]"
                overlap = data[-(len(marker) - 1) :]
    except OSError:
        return False
    return False


def cached_run_problem(saved: dict, *, keep_raw: bool = False) -> str | None:
    """Why a cached replay receipt cannot be served (its run must be re-captured), else None."""
    run_dir = saved.get("run_dir")
    if not isinstance(run_dir, str) or not Path(run_dir).is_dir():
        return "its run directory was removed by retention"
    trace_path = Path(run_dir) / "trace.json"
    if not trace_path.is_file():
        return "its trace.json is missing"
    if not trace_has_samples(trace_path):
        return "its trace has no samples"
    if keep_raw and (Path(run_dir) / SLIM_MARKER).is_file():
        return "its raw dumps were slimmed and --keep-raw needs them"
    return None


def archive_poisoned_receipt(record: Path) -> None:
    """Preserve an invalid receipt for diagnosis, then remove it from the cache namespace."""
    archive = record.parent.parent / "receipts" / "replay-poisoned"
    archive.mkdir(parents=True, exist_ok=True)
    target = archive / record.name
    if target.exists() and target.read_bytes() != record.read_bytes():
        target = archive / f"{record.stem}-{time.time_ns()}{record.suffix}"
    if not target.exists():
        shutil.copy2(record, target)
    record.unlink(missing_ok=True)


def enable_remote_workers(environ: dict[str, str] | None = None) -> str:
    """T3740 `--remote`: the replay workers from the host registry unless preset; error if none."""
    env = os.environ if environ is None else environ
    if not env.get("TSFP_REPLAY_REMOTE_HOSTS", "").strip():
        from tools.wt import remote_exec

        value, skipped = remote_exec.gate_replay_hosts()
        if not value:
            raise ValueError(f"--remote: no replay worker available ({', '.join(skipped)})")
        env["TSFP_REPLAY_REMOTE_HOSTS"] = value
    return env["TSFP_REPLAY_REMOTE_HOSTS"]


def desync_receipt(route_path: Path, run_dir: Path) -> dict:
    """T3819: run the desync detector here (the owner reference sidecar lives on this machine,
    remote hosts only see the route) and stamp trace.json meta `desync`/`desync_poll`."""
    from tools import replay_desync

    trace_path = run_dir / "trace.json"
    try:
        trace = json.loads(trace_path.read_text())
    except (OSError, ValueError):
        return {"desync_poll": None, "desync_verdict": "unknown"}
    meta = trace.setdefault("meta", {})
    end, recorded = meta.get("route_end_poll"), meta.get("recorded_route_end_poll")
    reached = isinstance(end, int) and isinstance(recorded, int) and end >= recorded
    result = replay_desync.detect(route_path, run_dir, reached_route_end=reached)
    meta["desync"], meta["desync_poll"] = result, result.get("desync_poll")
    trace_path.write_text(json.dumps(trace))
    return {"desync_poll": result.get("desync_poll"), "desync_verdict": result.get("verdict")}


CAPTURE_EVIDENCE_FILES = ("stop.txt", "close-error.txt", "output-cap.error", "run.log")


def capture_failure(
    run_dir: Path,
    remote_attempts: list[dict[str, object]],
    returncode: int | None,
    remote_logs: object,
    no_samples: bool = False,
) -> tuple[str, str]:
    """T3821: classified (cause, error) of a failed capture from its stop report, logs and the
    remote attempts, so replay:capture events never carry cause null with an empty error."""
    if no_samples:
        return "short-capture", "capture exited 0 with no trace samples (not cached)"
    run_parts: list[str] = []  # the run that produced returncode (local, or fetched remote logs)
    if isinstance(remote_logs, dict):
        run_parts.extend(str(value)[-20000:] for value in remote_logs.values())
    for name in CAPTURE_EVIDENCE_FILES:
        path = run_dir / name
        try:
            with path.open("rb") as handle:
                handle.seek(max(0, path.stat().st_size - 40000))
                run_parts.append(handle.read().decode(errors="replace"))
        except OSError:
            continue
    run_text = "\n".join(part for part in run_parts if part)
    if run_text:
        found = jobevents.capture_cause(returncode, run_text)
        if found[0] != "other":
            return found
    attempt_parts: list[str] = []  # earlier remote attempts that fell back
    for attempt in remote_attempts:
        attempt_parts.append(str(attempt.get("error") or ""))
        tail = attempt.get("diagnostic_tail") or []
        attempt_parts.extend(str(line) for line in tail if isinstance(tail, list))
    return jobevents.capture_cause(
        returncode, "\n".join(part for part in [run_text, *attempt_parts] if part)
    )


def submit(
    root: Path,
    route: str,
    switches: str,
    commit: str | None,
    capture_options: tuple[str, ...] = (),
    save_dir: Path | None = None,
    host_args: tuple[str, ...] = (),
    headless: bool = False,
    no_cache: bool = False,
    env_values: tuple[str, ...] = (),
    priority: str | None = None,
    host_input_hash: str | None = None,
    keep_raw: bool = False,
    t2_capture_host: Path | None = None,
    require_remote: bool = False,
) -> int:
    submit_started = time.monotonic()
    priority = replay_priority(priority)
    keep_raw = keep_raw_requested(keep_raw)
    if "--disc" not in capture_options:
        # T3592: match play.sh's owner-input resolution from linked worktrees.
        disc = owner_files.disc_image(root)
        capture_options = (
            *capture_options,
            "--disc",
            str(disc.resolve()),
        )
    route_path = Path(route).expanduser()
    if route == "r5":
        route_path = Path("tmp/routes/r5-story-gameplay-20261016.rec")
    if not route_path.is_absolute():
        route_path = root / route_path
    if not route_path.is_file() and route == "r5":
        from tools.wt.remote_replay import owner_files_dir

        route_path = owner_files_dir(root) / "routes/r5-story-gameplay-20261016.rec"
    if not route_path.is_file():
        raise ValueError(f"route recording not found: {route_path}")
    actual_commit = prebuilt.primary_commit(root)
    selected_commit = commit or actual_commit
    if not re.fullmatch(r"[0-9a-f]{40}", selected_commit):
        raise ValueError(f"--commit must be a full lowercase Git SHA: {selected_commit}")
    selected_input_hash = host_input_hash or prebuilt_select.build_input_hash(root, selected_commit)
    if t2_capture_host is not None:
        # T3740: a rule-43 capture replay runs a TSFP_BUILD_CAPTURE=1 host (never a shared
        # prebuilt), headless, and records <run dir>/capture.bin; the host is part of the key.
        host = t2_capture_host.expanduser().resolve()
        if not host.is_file():
            raise ValueError(f"capture host not found: {host}")
        from tools.t2capture import runtime_env

        runtime_env.check_capture_host(host)  # T3767: refuse before staging, not after a capture
        capture_options = (*capture_options, "--t2-capture")
        if "--rust-rea-ai" not in host_args and "--no-rust-rea-ai" not in host_args:
            # T3740: the host's default-on REA AI refuses to start with the ports off.
            host_args = (*host_args, "--no-rust-rea-ai")
        if "--render" not in capture_options:
            capture_options = (*capture_options, "--render", "none")
    else:
        host = prebuilt_select.find_host(
            root, commit=selected_commit, host_input_hash=selected_input_hash
        )
        if not prebuilt_select.available(host):
            raise ValueError(f"shared prebuilt host missing: {host}; run tools.wt prebuilt build")
    save_hash = None
    if save_dir is not None:
        save_dir = save_dir.expanduser().resolve()
        if not save_dir.is_dir():
            raise ValueError(f"save directory not found: {save_dir}")
        digest_state = hashlib.sha256()
        for item in sorted(save_dir.rglob("*")):
            if item.is_file():
                digest_state.update(item.relative_to(save_dir).as_posix().encode())
                digest_state.update(bytes.fromhex(digest(item)))
        save_hash = digest_state.hexdigest()
    snapshot_capture = (
        "--snapshot-at-poll" in capture_options or "--snapshot-at-polls" in capture_options
    )
    if snapshot_capture != ("--snapshot-dir" in capture_options):
        raise ValueError("snapshot poll(s) and --snapshot-dir must be supplied together")
    snapshot_dir = None
    if snapshot_capture:
        snapshot_dir = Path(capture_options[capture_options.index("--snapshot-dir") + 1])
        if snapshot_dir.exists():
            raise ValueError(
                f"snapshot destination already exists; choose a fresh path: {snapshot_dir}"
            )
    disc_hash = None
    if "--disc" in capture_options:
        disc_path = Path(capture_options[capture_options.index("--disc") + 1])
        if not disc_path.is_file():
            raise ValueError(f"disc image not found: {disc_path}")
        disc_hash = digest(disc_path)
    port_switches, switch_host_args, use_inventory_native = split_switches(switches)
    host_args = (*switch_host_args, *host_args)
    key_options = (
        *capture_options,
        *(f"--host-arg={item}" for item in host_args),
        *(item for value in env_values for item in ("--env", value)),
    )
    key, key_fields = replay_key(
        host,
        route,
        route_path,
        switches,
        key_options,
        save_hash,
        disc_hash,
        "headless" if snapshot_capture or headless else "gpu",
        selected_input_hash,
        secrets.token_hex(8) if snapshot_capture else None,
    )
    key_fields["host_input_hash"] = selected_input_hash
    key_fields["build_commit"] = selected_commit
    cache = cache_dir()
    cache.mkdir(parents=True, exist_ok=True)
    gpu_identity = os.environ.get("TSFP_GPU_QUEUE_ID") or (
        f"{socket.gethostname()}-{os.environ.get('CUDA_VISIBLE_DEVICES', 'default')}"
    )
    gpu_lock = hashlib.sha256(gpu_identity.encode()).hexdigest()
    # Serialize duplicate cache keys only. Distinct replay keys share the GPU through
    # hostlock's bounded slots; a GPU-wide flock here would defeat that admission.
    lock_path = cache / f"replay-{key}.lock"
    with lock_path.open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        record = cache / f"{key}.json"
        if record.is_file() and not no_cache:
            saved = json.loads(record.read_text())
            problem = cached_run_problem(saved, keep_raw=keep_raw)
            if problem is None:
                record_lookup("replay-capture", True)
                receipt = {**saved, "cache_hit": True}
                receipt.setdefault("queue_wait_seconds", 0.0)
                receipt.setdefault("run_seconds", saved.get("run_seconds"))
                receipt.setdefault("host_name", saved.get("remote_host") or socket.gethostname())
                receipt.setdefault(
                    "host", replay_host_label(saved.get("remote_host") or socket.gethostname())
                )
                receipt.setdefault("paced", "--paced" in capture_options)
                receipt.setdefault("snapshot", snapshot_capture)
                receipt["capture_breakdown"] = {
                    field: receipt.get(field)
                    for field in (
                        "queue_wait_seconds",
                        "run_seconds",
                        "host",
                        "host_name",
                        "paced",
                        "snapshot",
                        "cache_hit",
                        "timing_source",
                        "remote_staging_breakdown",
                        "remote_attempts",
                    )
                }
                print(json.dumps(receipt, sort_keys=True), flush=True)
                return int(saved["exit_code"])
            if problem == "its trace has no samples":
                archive_poisoned_receipt(record)
            # T3467: raw runs are a reproducible cache; regenerate instead of failing.
            print(f"replay submit: cached receipt {key}: {problem}; re-running", file=sys.stderr)
        if not no_cache:
            record_lookup("replay-capture", False)
        run_dir = cache / "runs" / key
        from tools import datadisk

        datadisk.check("replay", paths=[run_dir])  # T3745: data disk only, budget and floors
        datadisk.no_core_dumps()
        run_dir.mkdir(parents=True, exist_ok=True)
        replay_saves = None
        if save_dir is not None:
            replay_saves = run_dir / "seed-saves"
            if replay_saves.exists():
                shutil.rmtree(replay_saves)
            shutil.copytree(save_dir, replay_saves)
        command = [
            sys.executable,
            "-m",
            "tools.rust_trace",
            "capture",
            "--route",
            str(route_path),
            f"--rust-ports={port_switches}",
            "--host",
            str(host),
            "--host-input-hash",
            selected_input_hash,
            "--host-sha256",
            digest(host),
            "--out-dir",
            str(run_dir),
            *capture_options,
        ]
        if not snapshot_capture and not headless:
            command.insert(command.index("--out-dir"), "--gpu-display")
        if replay_saves is not None:
            command += ["--save-dir", str(replay_saves)]
        command.extend(f"--host-arg={argument}" for argument in host_args)
        capture_env = os.environ.copy()
        lane = "-priority" if priority == "high" else ""
        if capture_renders_none(capture_options):
            # T3390: a --render none capture uses no GPU; admit it by CPU slots.
            capture_env["TSFP_HOST_SLOTS"] = str(headless_replay_slots())
            capture_env["TSFP_HOSTLOCK_DIR"] = str(
                cache / f"hostlock-headless-{socket.gethostname()}{lane}"
            )
        else:
            capture_env["TSFP_HOST_SLOTS"] = str(replay_slots())
            capture_env["TSFP_HOSTLOCK_DIR"] = str(cache / f"hostlock-{gpu_lock}{lane}")
        if priority == "high":
            # T3388: the reserved lane has its own hostlock slots, never behind normal jobs.
            capture_env["TSFP_HOST_SLOTS"] = os.environ.get(
                "TSFP_REPLAY_PRIORITY_SLOTS", str(max(2, hostcaps.effective_cpus() // 2))
            )
        capture_env.pop("TSFP_HOST_SERIAL", None)
        capture_env.pop("TSFP_HOSTLOCK", None)
        # T3441: an unselected switch must say "0" (lane default-on T3418..T2411 31a2da9a72);
        # merely unsetting it left the "off" side of every A/B on the native lane.
        capture_env["TSFP_REA_INVENTORY_NATIVE"] = "1" if use_inventory_native else "0"
        timing_path = run_dir / "capture-timing.json"
        timing_path.unlink(missing_ok=True)
        capture_env["TSFP_CAPTURE_TIMING_FILE"] = str(timing_path)
        from tools.wt.capture_artifacts import resolve_capture_env

        for value in env_values:
            name, content = resolve_capture_env(value, run_dir)
            capture_env[name] = content
        marker = cache / "priority-active" / f"{os.getpid()}.pid"
        marker_stop = None
        marker_thread = None
        if priority == "high":
            import threading

            marker.parent.mkdir(parents=True, exist_ok=True)
            from tools.wt import lease

            marker_stop = threading.Event()
            marker_record = {**lease.holder(), "key": key, "heartbeat": time.time()}

            def beat_priority_marker() -> None:
                while not marker_stop.wait(10):
                    marker_record["heartbeat"] = time.time()
                    temporary = marker.with_suffix(".tmp")
                    temporary.write_text(json.dumps(marker_record, sort_keys=True) + "\n")
                    temporary.replace(marker)

            marker.write_text(json.dumps(marker_record, sort_keys=True) + "\n")
            marker_thread = threading.Thread(target=beat_priority_marker, daemon=True)
            marker_thread.start()
        else:
            yield_to_priority(cache)
        remote_name = None
        remote_result = None
        remote_attempts: list[dict[str, object]] = []
        remote_worker_health_seconds = 0.0
        prebuilt_lease_wait_seconds = 0.0
        try:
            if (
                capture_renders_none(capture_options)
                or remote_visual(capture_options, require_remote)
            ) and not snapshot_capture:
                from tools.wt import remote_replay

                workers = remote_replay.configured_workers()
                remote_env_values = (
                    *env_values,
                    f"TSFP_REA_INVENTORY_NATIVE={1 if use_inventory_native else 0}",
                )
                for worker in workers:
                    health_started = time.monotonic()
                    worker_health = remote_replay.health(worker, expected_commit=selected_commit)
                    health_seconds = time.monotonic() - health_started
                    remote_worker_health_seconds += health_seconds
                    if worker_health is None:
                        remote_attempts.append(
                            {
                                "host": worker.name,
                                "outcome": "unavailable",
                                "health_seconds": round(health_seconds, 3),
                            }
                        )
                        print(
                            f"replay submit: remote worker {worker.name} unavailable or below "
                            "resource reserve; trying next worker",
                            file=sys.stderr,
                            flush=True,
                        )
                        continue
                    try:
                        lease_started = time.monotonic()
                        with host_lease(host, t2_capture_host is not None):
                            prebuilt_lease_wait_seconds += time.monotonic() - lease_started
                            candidate = remote_replay.run_capture(
                                worker,
                                key=key,
                                host=host,
                                route=route_path,
                                output=run_dir,
                                switches=port_switches,
                                capture_options=capture_options,
                                host_args=host_args,
                                save_dir=save_dir,
                                env_values=remote_env_values,
                                priority=priority,
                                slots=worker_health.slots,
                                home=worker_health.home,
                                repo_path=worker_health.repo_path,
                                private_root=remote_replay.owner_files_dir(root),
                                source_commit=selected_commit,
                            )
                    except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
                        remote_attempts.append(
                            {
                                "host": worker.name,
                                "outcome": "error",
                                "elapsed_seconds": round(time.monotonic() - lease_started, 3),
                                "error": f"{type(error).__name__}: {error}"[-500:],
                            }
                        )
                        print(
                            f"replay submit: remote worker {worker.name} failed ({error}); "
                            "falling back",
                            file=sys.stderr,
                            flush=True,
                        )
                        candidate = None
                    if candidate is not None and candidate.returncode == 0:
                        remote_result = candidate
                        remote_name = worker.name
                        remote_attempts.append(
                            {
                                "host": worker.name,
                                "outcome": "captured",
                                "elapsed_seconds": round(time.monotonic() - lease_started, 3),
                            }
                        )
                        print(
                            f"replay submit: captured remotely on {worker.name} "
                            f"({worker_health.cores} cores, {worker_health.available_mb} MB "
                            f"available, {worker_health.slots} slots)",
                            file=sys.stderr,
                            flush=True,
                        )
                        break
                    if candidate is not None:
                        print(
                            f"replay submit: remote worker {worker.name} capture exited "
                            f"{candidate.returncode}; falling back",
                            file=sys.stderr,
                            flush=True,
                        )
                        stderr_lines = (candidate.stderr or "").splitlines()
                        stdout_lines = (candidate.stdout or "").splitlines()
                        diagnostic = stderr_lines[-12:] + stdout_lines[-12:]
                        remote_attempts.append(
                            {
                                "host": worker.name,
                                "outcome": "failed",
                                "elapsed_seconds": round(time.monotonic() - lease_started, 3),
                                "returncode": candidate.returncode,
                                "diagnostic_tail": [line[-500:] for line in diagnostic[-6:]],
                            }
                        )
                        if diagnostic:
                            print(
                                "replay submit: remote worker diagnostics "
                                "(last 12 stderr and stdout lines):\n" + "\n".join(diagnostic),
                                file=sys.stderr,
                                flush=True,
                            )
                if remote_result is None and workers:
                    for child in run_dir.iterdir():
                        if child.name == "seed-saves":
                            continue
                        shutil.rmtree(child) if child.is_dir() else child.unlink()
            if remote_result is not None:
                result = remote_result
                # Remote worker child receipts carry actual remote slot wait and run time.
                # Preserve them instead of reading this submitter's (empty) timing file.
                remote_timing = getattr(remote_result, "capture_timing", None)
                if remote_timing is not None:
                    remote_timing = dict(remote_timing)
                    remote_timing["remote_staging_breakdown"] = getattr(
                        remote_result, "remote_staging_breakdown", {}
                    )
            else:
                if require_remote:  # T3740 --remote: never fall back to this machine
                    cause, error = capture_failure(run_dir, remote_attempts, None, None)
                    if not remote_attempts:
                        cause, error = "remote-unavailable", "no remote worker"
                    jobevents.emit(
                        "replay:capture",
                        start=time.time() - (time.monotonic() - submit_started),
                        result="fail",
                        cause=cause,
                        error=error,
                        rc=None,
                        remote_attempts=len(remote_attempts),
                    )
                    raise ValueError("--remote: no remote worker completed the capture")
                remote_timing = None
                with host_lease(host, t2_capture_host is not None):
                    result = subprocess.run(command, cwd=root, env=capture_env, check=False)
        finally:
            if priority == "high":
                if marker_stop is not None:
                    marker_stop.set()
                if marker_thread is not None:
                    marker_thread.join(timeout=1)
                marker.unlink(missing_ok=True)
        saved = {
            **key_fields,
            "key": key,
            "run_dir": str(run_dir),
            "exit_code": result.returncode,
            "verdict": "PASS" if result.returncode == 0 else "FAIL",
            "cache_hit": False,
            "host_input_hash": selected_input_hash,
        }
        if remote_name is not None:
            saved["remote_host"] = remote_name
        if remote_result is not None:
            # T3760: run.log, host.out, host.err and stop.txt came back (capped to their tail);
            # their full remote sha256 and size go into the receipt.
            saved["remote_logs"] = getattr(remote_result, "remote_logs", None) or {}
        if remote_attempts:
            saved["remote_attempts"] = remote_attempts
        saved.update(desync_receipt(route_path, run_dir))
        try:
            timing = remote_timing or json.loads(timing_path.read_text())
        except (OSError, ValueError):
            timing = {}
        if remote_worker_health_seconds or prebuilt_lease_wait_seconds:
            timing = dict(timing)
            staging = dict(timing.get("remote_staging_breakdown") or {})
            staging.update(
                remote_worker_health_seconds=round(remote_worker_health_seconds, 3),
                prebuilt_lease_wait_seconds=round(prebuilt_lease_wait_seconds, 3),
            )
            timing["remote_staging_breakdown"] = staging
        if remote_attempts:
            timing["remote_attempts"] = remote_attempts
        host_name = remote_name or timing.get("host_name") or socket.gethostname()
        saved.update(
            queue_wait_seconds=timing.get("queue_wait_seconds", 0.0),
            run_seconds=timing.get("run_seconds", round(time.monotonic() - submit_started, 3)),
            host=replay_host_label(host_name),
            host_name=host_name,
            paced="--paced" in capture_options,
            snapshot=snapshot_capture,
            timing_source=timing.get("timing_source", "replay submit elapsed fallback"),
            remote_staging_breakdown=timing.get("remote_staging_breakdown", {}),
        )
        saved["capture_breakdown"] = {
            field: saved.get(field)
            for field in (
                "queue_wait_seconds",
                "run_seconds",
                "host",
                "host_name",
                "paced",
                "snapshot",
                "cache_hit",
                "timing_source",
                "remote_staging_breakdown",
                "remote_attempts",
            )
        }
        saved["capture_breakdown"].update(
            elapsed_seconds=round(time.monotonic() - submit_started, 3),
            staging_seconds=round(
                max(
                    0.0, time.monotonic() - submit_started - float(saved.get("run_seconds") or 0.0)
                ),
                3,
            ),
        )
        trace_path = run_dir / "trace.json"
        cacheable = trace_has_samples(trace_path)
        if result.returncode == 0 and not keep_raw and not snapshot_capture:
            slim_run(run_dir)
        if cacheable:
            temporary = record.with_suffix(".tmp")
            temporary.write_text(json.dumps(saved, sort_keys=True) + "\n")
            temporary.replace(record)
        else:
            print(
                "replay submit: refusing to cache successful capture with no samples",
                file=sys.stderr,
            )
        failed = result.returncode != 0 or not cacheable
        cause, error = (
            capture_failure(
                run_dir,
                remote_attempts,
                result.returncode,
                saved.get("remote_logs"),
                no_samples=result.returncode == 0,
            )
            if failed
            else (None, None)
        )
        jobevents.emit(
            "replay:capture",
            start=time.time() - (time.monotonic() - submit_started),
            result="fail" if failed else "ok",
            cause=cause,
            error=error,
            rc=result.returncode,
            queue_wait=saved["queue_wait_seconds"],
            run_seconds=saved["run_seconds"],
            replay_host=host_name,
        )
        print(json.dumps(saved, sort_keys=True), flush=True)
        return result.returncode


def resume(
    root: Path,
    snapshot_dir: Path,
    commit: str,
    seconds: float | None = None,
    host_input_hash: str | None = None,
    isolate_run_dir: bool = False,
) -> int:
    submit_started = time.monotonic()
    """Resume a DMTCP route capture inside a shared replay slot."""
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError(f"--commit must be a full lowercase Git SHA: {commit}")
    selected_input_hash = host_input_hash or prebuilt_select.build_input_hash(root, commit)
    host = prebuilt_select.find_host(root, commit=commit, host_input_hash=selected_input_hash)
    if not prebuilt_select.available(host):
        raise ValueError(f"shared prebuilt host missing: {host}; run tools.wt prebuilt build")
    snapshot_dir = snapshot_dir.expanduser().resolve()
    if not (snapshot_dir / "manifest.json").is_file():
        raise ValueError(f"snapshot manifest not found: {snapshot_dir / 'manifest.json'}")
    cache = cache_dir()
    cache.mkdir(parents=True, exist_ok=True)
    gpu_identity = os.environ.get("TSFP_GPU_QUEUE_ID") or (
        f"{socket.gethostname()}-{os.environ.get('CUDA_VISIBLE_DEVICES', 'default')}"
    )
    gpu_lock = hashlib.sha256(gpu_identity.encode()).hexdigest()
    command = [
        sys.executable,
        "-m",
        "tools.rust_trace",
        "resume",
        "--snapshot-dir",
        str(snapshot_dir),
        *(("--seconds", f"{seconds:g}") if seconds else ()),
    ]
    if isolate_run_dir:
        command.append("--isolate-run-dir")
    # T2479: a resumed capture is one more headless host run: admit it through the same
    # replay slots as submissions (T2486) instead of an exclusive queue lock, so a gate's off
    # baseline and candidates resume in parallel. The restored processes are descendants of
    # this admitted process, so other admissions count them as managed.
    from tools.hostlock import HostLock

    # One resume per snapshot at a time: a resume restores the snapshot's run directory.
    with (snapshot_dir / "resume.lock").open("a") as snapshot_lock:
        fcntl.flock(snapshot_lock, fcntl.LOCK_EX)
        with HostLock(directory=cache / f"hostlock-{gpu_lock}", slots=replay_slots()):
            admitted = time.monotonic()
            with prebuilt.lease(host):
                result = subprocess.run(command, cwd=root, check=False)
    finished = time.monotonic()
    host_name = socket.gethostname()
    timing = {
        "queue_wait_seconds": round(max(0.0, admitted - submit_started), 3),
        "run_seconds": round(max(0.0, finished - admitted), 3),
        "host": replay_host_label(host_name),
        "host_name": host_name,
        "paced": False,
        "snapshot": True,
        "cache_hit": False,
        "timing_source": "snapshot resume hostlock admission",
    }
    print(
        json.dumps(
            {
                "snapshot_dir": str(snapshot_dir),
                "host_sha256": digest(host),
                "host_input_hash": selected_input_hash,
                "exit_code": result.returncode,
                "verdict": "PASS" if result.returncode == 0 else "FAIL",
                **timing,
                "capture_breakdown": timing,
            },
            sort_keys=True,
        ),
        flush=True,
    )
    return result.returncode


def cli(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.wt replay")
    sub = parser.add_subparsers(dest="action", required=True)
    submit_parser = sub.add_parser("submit", help="block until this serialized replay finishes")
    submit_parser.add_argument("--route", required=True)
    submit_parser.add_argument("--switches", required=True)
    submit_parser.add_argument("--commit", required=True)
    submit_parser.add_argument("--host-input-hash", help="pin this shared host input hash")
    submit_parser.add_argument(
        "--seconds", type=int, help="optional wall cap (default: recorded route end, T3388)"
    )
    submit_parser.add_argument("--max-idle", type=int)
    submit_parser.add_argument("--max-dumps", type=int)
    submit_parser.add_argument(
        "--max-bytes", type=int, help="capture output byte cap (rust_trace default 4 GB, T3388)"
    )
    submit_parser.add_argument("--anim-shadow", choices=("on", "off"))
    submit_parser.add_argument(
        "--play-options", help="T3592: play.sh variant whose host options the capture runs, or off"
    )
    submit_parser.add_argument(
        "--paced",
        action="store_true",
        help="wall-clock paced capture (fast-forward is the default)",
    )
    submit_parser.add_argument(
        "--render",
        choices=("auto", "gpu", "none"),
        help="T3390: rust_trace --render (default auto: none for a fast-forward state capture, "
        "gpu for paced or screenshot runs)",
    )
    route_end = submit_parser.add_mutually_exclusive_group()
    route_end.add_argument(
        "--stop-at-route-end",
        action="store_true",
        help="stop capture when the recording reaches its final guest poll",
    )
    route_end.add_argument(
        "--no-stop-at-route-end",
        action="store_true",
        help="keep the host running until --seconds after the recording reaches its final poll",
    )
    submit_parser.add_argument(
        "--shot-at-route-end",
        action="store_true",
        help="save the recorded route-end frame (forces the visual GPU capture path)",
    )
    submit_parser.add_argument(
        "--shot-at",
        type=float,
        action="append",
        default=[],
        help="T3818: wall seconds after start for a shot-<s>.png (repeatable, visual GPU path); "
        "with --remote the worker renders offscreen (Xvfb) and the PNGs come back",
    )
    submit_parser.add_argument("--regions", help="rust_trace region-set JSON")
    submit_parser.add_argument(
        "--rea-ai-state",
        choices=("reference", "owned"),
        help="capture T2407's authoritative REA actor-state trace",
    )
    submit_parser.add_argument("--call-args", help="comma-separated root VAs to trace")
    submit_parser.add_argument(
        "--save-dir", type=Path, help="seed saves copied to queue scratch before replay"
    )
    submit_parser.add_argument(
        "--host-arg", action="append", default=[], help="extra argument forwarded to the Bevy host"
    )
    submit_parser.add_argument(
        "--no-cache", action="store_true", help="force a fresh replay despite a cached verdict"
    )
    submit_parser.add_argument(
        "--env",
        action="append",
        default=[],
        metavar="NAME=VALUE",
        help="set a TSFP_* host environment value (repeatable; included in the cache key)",
    )
    submit_parser.add_argument(
        "--fast-forward",
        action="store_true",
        help="accepted for compatibility: fast-forward is the default since T2487 (see --paced)",
    )
    submit_parser.add_argument(
        "--priority",
        choices=("normal", "high"),
        help="T3388: high = reserved lane for replay-system work (own slots, normal jobs yield); "
        "TSFP_REPLAY_PRIORITY is equivalent",
    )
    submit_parser.add_argument("--snapshot-at-poll", type=int)
    submit_parser.add_argument(
        "--snapshot-at-polls", help="ordered comma-separated DMTCP poll list"
    )
    submit_parser.add_argument("--snapshot-dir", type=Path)
    submit_parser.add_argument(
        "--snapshot-continue",
        action="store_true",
        help="T2479: let the snapshot capture run on after the checkpoint; its continued trace is "
        "kept in <snapshot-dir>/continued as the same-run oracle for a resumed capture",
    )
    submit_parser.add_argument(
        "--headless",
        action="store_true",
        help="alias of --render none (T3390): no window, display, GPU or audio output; "
        "snapshot captures imply it",
    )
    submit_parser.add_argument("--disc", type=Path, help="explicit XBE source disc")
    submit_parser.add_argument(
        "--t2-capture-host",
        type=Path,
        help="T3740: rule-43 capture replay with this TSFP_BUILD_CAPTURE=1 host (headless, writes "
        "<run dir>/capture.bin; docs/t3738-capture-design.md)",
    )
    submit_parser.add_argument(
        "--t2-budget-seed",
        type=Path,
        help="T3803: per-VA capture budget seed for a --t2-capture-host run "
        "(python3 -m tools.t2capture.budget_seed; staged to remote workers)",
    )
    submit_parser.add_argument(
        "--t2-capture-targets",
        type=Path,
        help="T3833: capture ALLOW list for a --t2-capture-host run "
        "(python3 -m tools.t2capture.live_targets targets; staged to remote workers)",
    )
    submit_parser.add_argument(
        "--remote",
        action="store_true",
        help="T3740: run on a remote replay worker only (laptop, droplets; registry probe when "
        "TSFP_REPLAY_REMOTE_HOSTS is unset), no local fallback; a --t2-capture-host run is "
        "ingested into the shared corpus",
    )
    submit_parser.add_argument(
        "--keep-raw",
        action="store_true",
        help="T3467: keep the raw per-button dumps and overlay frames (default: removed once "
        "trace.json holds them; TSFP_REPLAY_KEEP_RAW=1 is equivalent)",
    )
    resume_parser = sub.add_parser("resume", help="resume a snapshot under the replay queue lock")
    resume_parser.add_argument("--snapshot-dir", type=Path, required=True)
    resume_parser.add_argument("--commit", required=True)
    resume_parser.add_argument("--host-input-hash", help="pin the snapshot host input hash")
    resume_parser.add_argument("--isolate-run-dir", action="store_true")
    resume_parser.add_argument(
        "--seconds", type=float, help="wall budget of the resumed window (T2479)"
    )
    from tools.wt import replay_gc

    replay_gc.add_parser(sub)
    args = parser.parse_args(argv)
    if args.action == "gc":
        return replay_gc.run_cli(args)
    try:
        if args.action == "resume":
            return resume(
                Path.cwd().resolve(),
                args.snapshot_dir,
                args.commit,
                args.seconds,
                args.host_input_hash,
                args.isolate_run_dir,
            )
        if args.snapshot_at_poll is not None and args.snapshot_at_polls is not None:
            raise ValueError("choose --snapshot-at-poll or --snapshot-at-polls")
        snapshot_requested = args.snapshot_at_poll is not None or args.snapshot_at_polls is not None
        if snapshot_requested != (args.snapshot_dir is not None):
            raise ValueError("snapshot poll(s) and --snapshot-dir must be supplied together")
        if args.snapshot_continue and not snapshot_requested:
            raise ValueError("--snapshot-continue needs --snapshot-at-poll")
        options = (
            tuple(
                item
                for flag, value in (
                    ("--seconds", args.seconds),
                    ("--max-idle", args.max_idle),
                    ("--max-dumps", args.max_dumps),
                    ("--max-bytes", args.max_bytes),
                    ("--anim-shadow", args.anim_shadow),
                    ("--play-options", args.play_options),
                    ("--regions", args.regions),
                    (
                        "--t2-budget-seed",
                        args.t2_budget_seed.resolve() if args.t2_budget_seed else None,
                    ),
                    (
                        "--t2-capture-targets",
                        args.t2_capture_targets.resolve() if args.t2_capture_targets else None,
                    ),
                    ("--rea-ai-state", args.rea_ai_state),
                    ("--call-args", args.call_args),
                    ("--snapshot-at-poll", args.snapshot_at_poll),
                    ("--snapshot-at-polls", args.snapshot_at_polls),
                    ("--snapshot-dir", args.snapshot_dir.resolve() if args.snapshot_dir else None),
                    # Pin the default disc too: remote workers receive every input
                    # explicitly and never rely on a checkout-local tmp/ symlink.
                    (
                        "--disc",
                        (args.disc or owner_files.disc_image(Path.cwd())).resolve(),
                    ),
                    ("--render", args.render),
                )
                if value is not None
                for item in (flag, str(value))
            )
            + (("--paced",) if args.paced else ())
            # T3390: --headless is an alias of --render none (one headless path)
            + (("--render", "none") if args.headless and args.render is None else ())
            + (("--stop-at-route-end",) if args.stop_at_route_end else ())
            + (("--no-stop-at-route-end",) if args.no_stop_at_route_end else ())
            + (("--shot-at-route-end",) if args.shot_at_route_end else ())
            + tuple(item for moment in args.shot_at for item in ("--shot-at", f"{moment:g}"))
            + (("--snapshot-continue",) if args.snapshot_continue else ())
        )
        if args.remote:
            enable_remote_workers()
        return submit(
            Path.cwd().resolve(),
            args.route,
            args.switches,
            args.commit,
            options,
            args.save_dir,
            tuple(args.host_arg),
            args.headless,
            args.no_cache,
            tuple(args.env),
            args.priority,
            args.host_input_hash,
            args.keep_raw,
            args.t2_capture_host,
            **({"require_remote": True} if args.remote else {}),
        )
    except (OSError, ValueError, prebuilt.PrebuiltError) as error:
        print(f"replay submit: {error}", file=sys.stderr)
        return 2
