#!/usr/bin/env python3
# ruff: noqa: E501
"""T1879 system-level trace oracle for the Rust ports (see docs/t-rust-trace-oracle.md).

Runs the hybrid host headless (private Xvfb, one host at a time, killed by PID) on a recorded route with `--rust-ports=SPEC`,
captures poll keyed guest-memory dumps (`--dump-on-button` edges plus `--dump-button-idle-every` idle controls, regions from
tools/data/rust_trace_regions.json) and the per-root Rust/lifted counters of the stop report, and diffs two traces under a
tolerance policy (ints exact, floats epsilon, nondeterministic words masked from an off/off control pair).

    python -m tools.rust_trace capture --rust-ports off --out-dir tmp/rt/off --host tmp/hybrid-build/tsfp_host
    python -m tools.rust_trace diff --left tmp/rt/off --right tmp/rt/all [--control tmp/rt/off2] --report tmp/rt/report
    python -m tools.rust_trace run --out-dir tmp/rt/story --host tmp/hybrid-build/tsfp_host --bisect

`run` does off, an optional off control, all, the diff and, with --bisect, delta debugging over the roots the `all` run reached.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path

from tools.datadir import hash_inputs, output_path, write_recipe
from tools.hostlock import serialized
from tools.output_caps import OutputCapExceeded, stop_after_cap
from tools.output_caps import check as check_output
from tools.play import rec_format
from tools.rust_trace_core import (
    Policy,
    compare_traces,
    ddmin,
    load_regions,
    noise_masks_multi,
    parse_counters,
    reached_roots,
    read_dump_file,
    render_markdown,
)
from tools.t982_acceptance import ROOT, main_root, pick_display, send_close, stop_xvfb
from tools.t1240_drive import host_argv

DEFAULT_REGIONS = "tools/data/rust_trace_regions.json"
DEFAULT_DISC = "tmp/TimeSplitters - Future Perfect (USA) (XBOX).iso"
DEFAULT_ROUTE = "tmp/recorded-input-story-mode"
DEFAULT_NATIVE_HOST = "tmp/bevy-target/release/tsfp-bevy"
REA_AI_TICK_ROOT = "00266250"


def rea_ai_tick_root_calls(trace: dict) -> int:
    """Return selected scheduler-root calls for the AI state adoption trace."""
    return trace.get("counters", {}).get(REA_AI_TICK_ROOT, {}).get("rust", 0)


def resolve(path: str) -> Path:
    """A repo relative path: the worktree first, then the main checkout (gitignored inputs live there)."""
    candidate = Path(path)
    if candidate.is_absolute():
        return candidate
    for base in (ROOT, main_root()):
        if (base / candidate).exists():
            return base / candidate
    return ROOT / candidate


def resolve_route(path: str) -> Path:
    """Use the main checkout's default recording and reject unusable routes before launch."""
    route = main_root() / DEFAULT_ROUTE if path == DEFAULT_ROUTE else resolve(path)
    try:
        if not route.is_file():
            raise OSError("not a regular file")
        with route.open("rb") as recording:
            if not recording.read(1):
                raise OSError("empty recording")
    except OSError as error:
        raise ValueError(
            f"--route recording unavailable: {route} ({error}). "
            "Provide --route /absolute/path/to/a/recorded-input file; "
            "the default recording must exist in the main checkout."
        ) from error
    return route.resolve()


def native_pacing_argv(argv: list[str], paced: bool) -> list[str]:
    """T2487: native .rec captures fast-forward unless paced: no wall vblank wait in the present sink.

    Input is keyed to guest polls (T2032) and the guest vblank cadence stays intact."""
    if paced or "--present-no-pace" in argv:
        return list(argv)
    return [*argv, "--present-no-pace"]


def native_pacing_env(env: dict, paced: bool) -> None:
    """T2487: TSFP_FAST_FORWARD=1 selects the virtual guest time base (src/xbox/kernel_vtime.h).

    Fast-forward also takes the C live renderer's frame job off the guest path
    (TSFP_LIVE_FRAME_OFFPATH=off, unless the caller chose a mode): on llvmpipe that job capped
    replays near real time, and r5 is EQUAL with it off. "drop" is not used: it diverged
    (match_state, docs/t2032-offoff-determinism.md)."""
    if paced:
        env.pop("TSFP_FAST_FORWARD", None)
        env.pop("TSFP_DETSCHED", None)  # T3442: the opt-in scheduler needs the virtual time base
    else:
        env["TSFP_FAST_FORWARD"] = "1"
        env.setdefault("TSFP_LIVE_FRAME_OFFPATH", "off")


def native_input_route_end_poll(route: Path) -> int | None:
    """Return the final host poll from a T2348 route, or None for a legacy route."""
    with route.open("r", encoding="utf-8") as recording:
        header = recording.readline().rstrip("\r\n")
        if not header.startswith(rec_format.MAGIC):
            return None
        # T2421 saves=/saves-sha256= and T3820 build= stamps; versions from the format owner (rule 45)
        try:
            version, _ = rec_format.parse_header(header)
        except ValueError as error:
            raise ValueError(f"{error} in {route}") from error
        ended = False
        saw_tick = False
        last_tick = -1
        end_poll = 0
        for line_number, line in enumerate(recording, start=2):
            row = line.rstrip("\r\n")
            if row == "E":
                ended = True
                break
            values = row.split("\t")
            kind = values[0] if values else ""
            # T3392 v5 `P` pointer rows, T3820 v6 `D` device and `S` state rows
            expected = rec_format.row_fields(kind, version)
            if expected is None or len(values) != expected:
                raise ValueError(f"malformed native input route row {line_number} in {route}")
            try:
                sim_tick = int(values[1])
                host_poll = int(values[2])
                wall_ns = int(values[3])
                if min(sim_tick, host_poll, wall_ns) < 0:
                    raise ValueError
                if kind == "T":
                    if sim_tick <= last_tick:
                        raise ValueError
                    last_tick = sim_tick
                    saw_tick = True
                if kind not in ("D", "S"):  # T3820: device and state rows do not extend the route
                    end_poll = max(end_poll, host_poll)
            except ValueError as error:
                raise ValueError(
                    f"invalid native input route clock at row {line_number} in {route}"
                ) from error
        if not ended or not saw_tick:
            raise ValueError(f"native input route is truncated or has no gameplay ticks: {route}")
        return end_poll


def native_input_route_end_sim_tick(route: Path) -> int | None:
    """Return the final recorded guest simulation tick for a native input route.

    Trace samples and the host's snapshot/stop-at-poll controls use this clock.
    Route metadata also carries a host-poll counter, which is a different clock.
    """
    with route.open("r", encoding="utf-8") as recording:
        header = recording.readline().rstrip("\r\n")
        if not header.startswith(rec_format.MAGIC):
            return None
        try:
            rec_format.parse_header(header)
        except ValueError as error:
            raise ValueError(f"{error} in {route}") from error
        last_tick = 0
        ended = False
        for line_number, line in enumerate(recording, start=2):
            row = line.rstrip("\r\n")
            if row == "E":
                ended = True
                break
            values = row.split("\t")
            if values[0] == "T":
                if len(values) != 8:
                    raise ValueError(f"malformed native input route row {line_number} in {route}")
                tick = int(values[1])
                if tick <= last_tick:
                    raise ValueError(
                        f"invalid native input route clock at row {line_number} in {route}"
                    )
                last_tick = tick
        if not ended or not last_tick:
            raise ValueError(f"native input route is truncated or has no gameplay ticks: {route}")
        return last_tick


def native_input_route_unlock_all(route: Path) -> bool:
    """Return the unlock policy recorded by native input formats that carry it."""
    return "--unlock-all" in native_input_route_host_args(route)


def native_input_route_host_args(route: Path) -> list[str]:
    """Host flags for every session option the route header records; ValueError if one cannot apply."""
    from tools.play import rec_saves

    return rec_saves.header_host_args(route)


def trace_host_for_route(
    configured: str | None,
    route: Path,
    native_route: bool,
    host_input_hash: str | None = None,
    host_sha256: str | None = None,
) -> Path:
    """Native routes run through the same Bevy shell that owns the T2348 router."""
    from tools import prebuilt_select
    from tools.wt import prebuilt

    if host_input_hash:
        # Replay dispatch usually provides its selected prebuilt path. Prefer it
        # when its published manifest confirms the requested input hash, avoiding
        # a second cache scan that could race publication or cleanup.
        configured_host = resolve(configured) if configured else None
        if host_sha256:
            # Replay selected and leased this exact artifact. Its manifest can
            # disappear during shared-cache maintenance; the binary hash still
            # pins the host used for this capture.
            pinned = None
            if (
                configured_host is not None
                and configured_host.is_file()
                and file_sha256(configured_host) == host_sha256
            ):
                pinned = configured_host
            if pinned is None:
                cached_host = prebuilt_select.find_host(ROOT, host_input_hash=host_input_hash)
                if (
                    cached_host is not None
                    and cached_host.is_file()
                    and file_sha256(cached_host) == host_sha256
                ):
                    pinned = cached_host
            if pinned is None:
                raise ValueError(f"prebuilt host input hash is not available: {host_input_hash}")
        else:
            configured_available = bool(
                configured_host and prebuilt_select.available(configured_host)
            )
            configured_hash = (
                prebuilt_select.host_input_hash(configured_host, ROOT)
                if configured_available
                else None
            )
            # Snapshot-restored workers can have a different checkout from the
            # replay queue. Accept its manifest key only when it matches the
            # explicit input hash passed by that queue.
            if configured_available and configured_hash is None and configured_host is not None:
                manifest = prebuilt._manifest(configured_host.parent)
                if manifest and manifest.get("input_hash") == host_input_hash:
                    configured_hash = host_input_hash
            if configured_available and configured_hash == host_input_hash:
                pinned = configured_host
            else:
                pinned = prebuilt_select.find_host(ROOT, host_input_hash=host_input_hash)
                pinned_available = bool(pinned and prebuilt_select.available(pinned))
                pinned_hash = (
                    prebuilt_select.host_input_hash(pinned, ROOT) if pinned_available else None
                )
                if pinned_available and pinned_hash is None and pinned is not None:
                    manifest = prebuilt._manifest(pinned.parent)
                    if manifest and manifest.get("input_hash") == host_input_hash:
                        pinned_hash = host_input_hash
                if not pinned_available or pinned_hash != host_input_hash:
                    raise ValueError(
                        f"prebuilt host input hash is not available: {host_input_hash}; "
                        f"configured={configured_host}, configured_available={configured_available}, "
                        f"configured_hash={configured_hash}, pinned={pinned}, "
                        f"pinned_available={pinned_available}, pinned_hash={pinned_hash}"
                    )
        configured = str(pinned)

    def primary_prebuilt() -> Path | None:
        try:
            candidate = prebuilt.expected_path(ROOT)
        except (OSError, prebuilt.PrebuiltError):
            return None
        return candidate if prebuilt.available(candidate) else None

    if not configured and native_route:
        base = prebuilt.primary_commit(ROOT)
        changed = prebuilt.changed_build_inputs(ROOT, base)
        if not changed:
            candidate = primary_prebuilt()
            if candidate:
                configured = str(candidate)
            else:
                subprocess.run(
                    [
                        sys.executable,
                        "-m",
                        "tools.wt",
                        "prebuilt",
                        "build",
                        "--worktree",
                        str(ROOT),
                    ],
                    cwd=ROOT,
                    check=True,
                )
                candidate = primary_prebuilt()
                if candidate:
                    configured = str(candidate)
        if not configured:
            target = Path(os.environ.get("CARGO_TARGET_DIR", "tmp/bevy-target"))
            target = target if target.is_absolute() else ROOT / target
            configured = str(target / "release" / "tsfp-bevy")
            if changed:
                env = os.environ.copy()
                env["CARGO_TARGET_DIR"] = str(target)
                subprocess.run(["sh", "tools/dev/build.sh"], cwd=ROOT, env=env, check=True)
    if not configured:
        configured = "tmp/hybrid-build/tsfp_host"
    if not native_route:
        return resolve(configured)

    host = configured
    if configured == "tmp/hybrid-build/tsfp_host":
        candidate = primary_prebuilt()
        host = str(candidate) if candidate else DEFAULT_NATIVE_HOST
    elif Path(configured).name != "tsfp-bevy":
        raise ValueError(
            f"{route} is a native .rec route; use --host {DEFAULT_NATIVE_HOST}, "
            "not the legacy C host"
        )
    resolved = resolve(host)
    if native_route and not resolved.is_file():
        raise ValueError(
            f"native route host is unavailable at the explicit selected path {resolved}; "
            "build with the same CARGO_TARGET_DIR or omit --host to let tools.rust_trace select/build it"
        )
    if not resolved.is_file():
        raise ValueError(
            f"native route host is unavailable: {resolved}; build it with sh tools/dev/build.sh"
        )
    return resolved.resolve()


def file_sha256(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


# T3592: the host options of a live `sh tools/dev/play.sh [variant] --replay <rec>` are read from play.sh itself
# (`--emit-options`), so a capture, a gate and a replay-queue job cannot drift from what the owner ran.
PLAY_OPTIONS_SKIP_ENV = frozenset(
    {
        # renderer selection and stats: a capture owns its render mode (--render gpu|none, --paced)
        "TSFP_WGPU_LIVE",
        "TSFP_WGPU_PIPELINED_READBACK",
        "TSFP_RT_HOST_STATS",
        # the capture sets these from its own arguments
        "TSFP_UI_DISC",
        "TSFP_SIMULATION_PACE",
    }
)
PLAY_OPTIONS_SKIP_ARGS = frozenset(
    {
        "--wgpu-share",  # GPU-texture presenter: a render-mode choice, not a guest-visible option
        "--wgpu-live",
        "--present-no-pace",  # the capture's pacing model (native_pacing_argv) owns pacing
        "--input-router",  # native_route_argv adds the replay router set itself
        "--mouse-keyboard",
        "--raw-look",
    }
)
PLAY_OPTIONS_SKIP_VALUED = frozenset({"--input-replay", "--input-record"})
# Capture defaults a live play.sh run never passes: a recording made without them must replay without them
# (`--skip-intro` ends the startup logos early, so every later menu and load lands at a different guest time).
PLAY_OPTIONS_DROP_ARGS = frozenset({"--skip-intro"})


def play_options(variant: str = "default", *, root: Path | None = None) -> dict:
    """JSON from `play.sh --emit-options --replay`: {env, ports, args, base} of one play.sh variant."""
    base = root or ROOT
    result = subprocess.run(
        ["sh", str(base / "tools/dev/play.sh"), "--emit-options", "--replay", "route.rec", variant],
        cwd=base,
        capture_output=True,
        text=True,
        check=False,
        timeout=120,
        # Inside a DMTCP snapshot run a closed PID-map fd aborts the child with exit 99 (T3580).
        **dmtcp_child_options(os.environ),
    )
    lines = [line for line in result.stdout.splitlines() if line.startswith("{")]
    if result.returncode != 0 or not lines:
        raise RuntimeError(
            f"play.sh --emit-options {variant} failed: rc={result.returncode} "
            f"stderr={result.stderr[-400:]!r} stdout={result.stdout[-200:]!r}"
        )
    return json.loads(lines[-1])


def play_option_args(options: dict) -> list[str]:
    """Host args of the play.sh variant that a native replay capture must also run."""
    args: list[str] = []
    tokens = list(options["args"])
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if token in PLAY_OPTIONS_SKIP_VALUED:
            index += 2
        elif token in PLAY_OPTIONS_SKIP_ARGS:
            index += 1
        else:
            args.append(token)
            index += 1
    return args


def apply_play_env(env: dict[str, str], options: dict) -> None:
    """Add play.sh's default host environment (the caller's explicit choices win)."""
    for key, value in options["env"].items():
        if key not in PLAY_OPTIONS_SKIP_ENV:
            env.setdefault(key, value)


def apply_play_args(argv: list[str], options: dict) -> list[str]:
    """Add play.sh's default host args that the capture does not already carry."""
    result = [
        token
        for token in argv
        if token not in PLAY_OPTIONS_DROP_ARGS or token in options["args"] + options["base"]
    ]
    for token in play_option_args(options):
        if token not in result:
            result.append(token)
    return result


def play_options_drift(host_options: dict, options: dict) -> list[str]:
    """Differences between a capture's host-options.json and the play.sh options it must follow (empty = none).

    Reports a play.sh arg or env value the capture lacks and a capture-only `--skip-intro` (T3592 option drift)."""
    argv = list(host_options.get("argv", []))
    env = host_options.get("env", {})
    drift = [f"missing arg {token}" for token in play_option_args(options) if token not in argv]
    drift += [
        f"arg {token} not in play.sh"
        for token in PLAY_OPTIONS_DROP_ARGS
        if token in argv and token not in options["args"] + options["base"]
    ]
    for key, value in options["env"].items():
        if key not in PLAY_OPTIONS_SKIP_ENV and env.get(key) != value:
            drift.append(f"env {key}={env.get(key)!r}, play.sh {value!r}")
    return drift


def native_route_argv(argv: list[str], route: Path) -> list[str]:
    """Replace legacy route replay with the Bevy shell's typed Rust input replay."""
    switches = {
        "--replay-handover"
    }  # T2032: --interactive is kept, the owner records are interactive sessions
    valued = {"--replay-input", "--pad-source"}
    result = argv[:2]
    index = 2
    while index < len(argv):
        token = argv[index]
        if token in switches:
            index += 1
        elif token in valued:
            index += 2
        else:
            result.append(token)
            index += 1
    # A native recording's ShimEvent::ButtonDown/Axis rows are replayed as
    # gamepad events through the window-input contract.  The keyboard source
    # ignores those PAD_* events, so select the gamepad adapter for recorded
    # input; otherwise the trace logs queued edges that never reach XInput.
    result.extend(
        [
            # Native .rec playback reproduces an interactive route. Preserve
            # its live wait policy; otherwise the default owner-wait budget
            # can stop before the recorded route is consumed.
            "--interactive",
            "--input-router",
            "--input-replay",
            str(route),
            "--mouse-keyboard",
            "--raw-look",
            "--pad-source",
            "gamepad",
        ]
    )
    result.extend(flag for flag in native_input_route_host_args(route) if flag not in result)
    return result


def route_end_poll_from_logs(out: Path) -> int | None:
    """Read the host-poll marker from either legacy route.log or Bevy run.log."""
    route_ends = []
    for route_log in (out / "route.log", out / "run.log"):
        if route_log.exists():
            for line in route_log.read_text(errors="replace").splitlines():
                if (
                    " route end " in line
                    or "native input replay completed at recorded sim tick" in line
                ) and "poll=" in line:
                    match = re.search(r"\bpoll=(\d+)", line)
                    # T3390: poll 0 is an interleaved fragment (host exit counters cut the
                    # Bevy line), never a route end.
                    if match and int(match[1]) > 0:
                        route_ends.append(int(match[1]))
    # Bevy repeats its route-end marker on every poll after playback finishes.
    # Use the first completed-route poll, even if log formatting appends a button
    # label directly after the number.
    return min(route_ends) if route_ends else None


FRAME_DRAW_MAX_ROW_BYTES = 1024 * 1024
FRAME_DRAW_POLL_BYTES = 4 * 1024 * 1024


class FrameDrawWatcher:
    """Read a bounded JSONL draw capture until its first row for one renderer frame."""

    def __init__(self, path: Path, target_frame: int) -> None:
        self.path = path
        self.target_frame = target_frame
        self.offset = 0

    def poll(self) -> int | None:
        try:
            if self.path.stat().st_size < self.offset:
                self.offset = 0
            with self.path.open("rb") as trace:
                trace.seek(self.offset)
                remaining = FRAME_DRAW_POLL_BYTES
                while remaining > 0:
                    row_start = trace.tell()
                    row = trace.readline(min(FRAME_DRAW_MAX_ROW_BYTES + 1, remaining))
                    if not row:
                        self.offset = trace.tell()
                        return None
                    remaining -= len(row)
                    if len(row) > FRAME_DRAW_MAX_ROW_BYTES:
                        raise ValueError("frame draw capture row exceeds 1 MiB")
                    if not row.endswith(b"\n"):
                        self.offset = row_start
                        return None
                    self.offset = trace.tell()
                    try:
                        record = json.loads(row)
                    except json.JSONDecodeError as error:
                        raise ValueError("frame draw capture contains invalid JSON") from error
                    frame = record.get("frame") if isinstance(record, dict) else None
                    if type(frame) is int and frame == self.target_frame:
                        return frame
        except FileNotFoundError:
            return None
        return None


def configure_frame_shot(
    out: Path, env: dict, render: str
) -> tuple[int, FrameDrawWatcher, Path] | None:
    """Arm an exact frame-boundary screenshot using the next frame's first captured draw."""
    raw_frame = env.get("TSFP_REPLAY_SHOT_FRAME")
    if raw_frame is None:
        return None
    if not re.fullmatch(r"(?:0|[1-9][0-9]*)", raw_frame):
        raise ValueError("TSFP_REPLAY_SHOT_FRAME must be an unsigned decimal frame number")
    image_frame = int(raw_frame)
    if image_frame >= (1 << 64) - 1:
        raise ValueError("TSFP_REPLAY_SHOT_FRAME is outside the supported renderer-frame range")
    if render == "none" or env.get("TSFP_FAST_FORWARD") == "1":
        raise ValueError("frame-matched screenshots require a paced renderer window")
    trace_text = env.get("TSFP_LIVE_DRAW_DUMP_FULL")
    if not trace_text or trace_text.startswith("@capture/"):
        raise ValueError(
            "frame-matched screenshots require a resolved TSFP_LIVE_DRAW_DUMP_FULL path"
        )
    selector = env.get("TSFP_LIVE_DRAW_DUMP_FULL_FRAME")
    if selector and not {str(image_frame), str(image_frame + 1)}.issubset(selector.split(",")):
        raise ValueError(
            "TSFP_LIVE_DRAW_DUMP_FULL_FRAME must include the image frame and its successor"
        )
    ready_path = out / f"shot-frame-{image_frame}.ready"
    ready_path.unlink(missing_ok=True)
    env["TSFP_LIVE_DRAW_DUMP_FULL_WAIT_FILE"] = str(ready_path)
    return image_frame, FrameDrawWatcher(Path(trace_text), image_frame + 1), ready_path


def capture_frame_boundary_shot(
    out: Path, env: dict, image_frame: int, draw_frame: int, ready_path: Path
) -> bool:
    """Capture while the renderer is paused before its first draw after image_frame."""
    shot_path = out / f"shot-frame-{image_frame}.png"
    capture_error = None
    try:
        captured = capture_bevy_window_shot(shot_path, env)
    except (OSError, subprocess.SubprocessError) as error:
        captured = False
        capture_error = str(error)
    finally:
        try:
            ready_path.touch()
        except OSError as error:
            capture_error = f"could not release renderer frame barrier: {error}"
            captured = False
    if not captured:
        details = capture_error or "could not capture the visible Bevy window"
        shot_path.with_suffix(".error").write_text(details + "\n")
        return False
    shot_path.with_suffix(".frame.json").write_text(
        json.dumps(
            {
                "image_frame": image_frame,
                "trigger_draw_frame": draw_frame,
                "capture_boundary": "renderer held before first draw of trigger_draw_frame",
            },
            indent=2,
        )
        + "\n"
    )
    return True


def capture_route_end_shot(out: Path, env: dict) -> bool:
    """Save the Bevy window once the host has logged route completion."""
    if route_end_poll_from_logs(out) is None:
        return False
    if capture_bevy_window_shot(out / "shot-route-end.png", env):
        return True
    (out / "shot-route-end.error").write_text("could not capture the visible Bevy window\n")
    return False


def capture_bevy_window_shot(path: Path, env: dict) -> bool:
    """Capture the Bevy window, preferring Wayland screencopy over Xwayland readback."""
    # X_GetImage returns BadMatch for the Bevy surface under Weston/Xwayland,
    # even when xdotool finds its Xwayland window. Capture the active Wayland
    # output first; the replay owns a dedicated Weston output during this call.
    capture_env = dict(env)
    try:
        display = json.loads((path.parent / "gpu-display.json").read_text())
        runtime_dir = display.get("xdg_runtime_dir")
        wayland_socket = display.get("wayland_socket")
        if runtime_dir and wayland_socket:
            capture_env["XDG_RUNTIME_DIR"] = runtime_dir
            capture_env["WAYLAND_DISPLAY"] = wayland_socket
    except (OSError, json.JSONDecodeError):
        pass
    if capture_env.get("WAYLAND_DISPLAY"):
        wayland = subprocess.run(
            ["grim", str(path)],
            env=capture_env,
            capture_output=True,
            timeout=10,
            check=False,
        )
        if wayland.returncode == 0 and path.is_file() and path.stat().st_size > 0:
            return True
        path.with_suffix(".wayland-capture.log").write_text(
            (wayland.stderr or b"").decode(errors="replace")[-2000:]
        )
    windows = subprocess.run(
        # SDL's native Wayland window is not named after the host executable.
        # Search every named Xwayland window and let ffmpeg reject non-capturable
        # ids; `tsfp` only worked for an older title convention.
        ["xdotool", "search", "--name", ".*"],
        env=env,
        capture_output=True,
        text=True,
        timeout=5,
        check=False,
    )
    ids = [line.strip() for line in (windows.stdout or "").splitlines() if line.strip().isdigit()]
    if windows.returncode != 0 or not ids:
        tree = subprocess.run(
            ["xwininfo", "-root", "-tree"],
            env=env,
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        path.with_suffix(".capture.log").write_text(
            f"xdotool exit={windows.returncode}; windows={ids!r}; "
            f"stderr={(windows.stderr or '').strip()}\n"
            f"xwininfo exit={tree.returncode}\n{(tree.stdout or '')[-5000:]}"
        )
        return False
    for window in reversed(ids):
        raw = path.with_suffix(f".{window}.xwd")
        result = subprocess.run(
            ["xwd", "-silent", "-id", window, "-out", str(raw)],
            env=env,
            capture_output=True,
            timeout=10,
            check=False,
        )
        if result.returncode != 0:
            path.with_suffix(".capture.log").write_text(
                (result.stderr or b"").decode(errors="replace")[-2000:]
            )
            continue
        converted = subprocess.run(
            ["magick", str(raw), str(path)],
            env=env,
            capture_output=True,
            timeout=10,
            check=False,
        )
        raw.unlink(missing_ok=True)
        if converted.returncode == 0 and path.is_file() and path.stat().st_size > 0:
            return True
        path.with_suffix(".capture.log").write_text(
            (converted.stderr or b"").decode(errors="replace")[-2000:]
        )
    # Xwayland may reject individual composited windows with BadMatch while
    # still exposing its root image. Keep this as a fallback after named views.
    raw = path.with_suffix(".root.xwd")
    result = subprocess.run(
        ["xwd", "-silent", "-root", "-out", str(raw)],
        env=env,
        capture_output=True,
        timeout=10,
        check=False,
    )
    if result.returncode == 0:
        converted = subprocess.run(
            ["magick", str(raw), str(path)],
            env=env,
            capture_output=True,
            timeout=10,
            check=False,
        )
        raw.unlink(missing_ok=True)
        if converted.returncode == 0 and path.is_file() and path.stat().st_size > 0:
            return True
        path.with_suffix(".capture.log").write_text(
            (converted.stderr or b"").decode(errors="replace")[-2000:]
        )
    else:
        path.with_suffix(".capture.log").write_text(
            (result.stderr or b"").decode(errors="replace")[-2000:]
        )
    return False


def kill_pid(pid: int) -> None:
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


ICD_DIR = Path("/usr/share/vulkan/icd.d")
LVP_ICD = ICD_DIR / "lvp_icd.json"
# T3388: discrete first. NVIDIA's proprietary ICD lives in /etc/vulkan/icd.d (laptop RTX 3070).
# MEASURED there: the C host's default SDL offscreen surface segfaults in libnvidia-glcore
# (gpu_window_create), so host_env sets SDL_VIDEODRIVER=x11 whenever NVIDIA is selected; that runs
# (r5 fast-forward renderer path 96.3 s, vsync-bound; state captures use --render none anyway).
REAL_ICDS = ("nvidia_icd.json", "radeon_icd.json", "intel_icd.json", "nouveau_icd.json")
ICD_DIRS = (Path("/etc/vulkan/icd.d"), ICD_DIR)


def real_icds() -> list[str]:
    """Installed real-GPU ICD files in preference order (first directory hit per name)."""
    found = []
    for name in REAL_ICDS:
        for directory in ICD_DIRS:
            if (directory / name).exists():
                found.append(str(directory / name))
                break
    return found


def vulkan_icd_policy(base: dict, gpu_display: bool = False) -> tuple[str | None, str]:
    """T2146: (VK_ICD_FILENAMES value or None to leave unset, policy label).

    Headless trace runs and tests keep the lavapipe ICD (deterministic). When the wgpu path is chosen
    (TSFP_WGPU_LIVE set) and a real ICD file exists, the real ICDs come first with lavapipe last, so the
    live path gets the real adapter and a machine without a GPU still falls back to llvmpipe/lavapipe.
    An explicit TSFP_VK_ICD_FILENAMES (or, on the wgpu path, an explicit VK_ICD_FILENAMES) wins."""
    explicit = base.get("TSFP_VK_ICD_FILENAMES")
    if explicit:
        return explicit, "explicit TSFP_VK_ICD_FILENAMES"
    # T3388: a --gpu-display capture (the replay queue default) prefers the discrete GPU too; on
    # lavapipe the 1280x960 Bevy window alone used about 6 cores of a fast-forward replay.
    if base.get("TSFP_WGPU_LIVE") or (gpu_display and not base.get("TSFP_LAVAPIPE")):
        if base.get("VK_ICD_FILENAMES"):
            return base["VK_ICD_FILENAMES"], "explicit VK_ICD_FILENAMES"
        real = real_icds()
        if real:
            return ":".join([*real, str(LVP_ICD)]), "real ICDs first, lavapipe last"
    return str(LVP_ICD), "lavapipe (headless trace default)"


def host_env(
    base: dict, display: str, runtime: Path, anim_shadow: bool, gpu_display: bool = False
) -> dict:
    """Environment of one host run."""
    icd, _label = vulkan_icd_policy(base, gpu_display)
    env = dict(
        base,
        DISPLAY=display,
        SDL_AUDIODRIVER="dummy",
        XDG_RUNTIME_DIR=str(runtime),
    )
    if icd is None:
        env.pop("VK_ICD_FILENAMES", None)
    else:
        env["VK_ICD_FILENAMES"] = icd
        if "nvidia_icd.json" in icd.split(":")[0] and "SDL_VIDEODRIVER" not in base:
            env["SDL_VIDEODRIVER"] = "x11"  # T3388: SDL offscreen + NVIDIA EGL segfaults
    env.pop("TSFP_ANIM_SHADOW", None)
    if anim_shadow:
        env["TSFP_ANIM_SHADOW"] = (
            "1"  # T1986: animation shadow mode (Tier 2) is opt-in in the host, rust_trace turns it on
        )
    # T2053: a -DTSFP_RUST_LANE_MUTATION=ON host flips EAX bit 0 of EVERY selected lane root when no
    # target is named, which faked the 0x190EB0 divergence. Default to an inert target (no root has it),
    # a mutant run still names its root.
    env.setdefault("TSFP_LANE_MUTATE_VA", "ffffffff")
    # T2032 (off/off determinism): guest allocations otherwise come from kernel-randomised MAP_32BIT mmaps,
    # so every guest heap pointer (player_prefix words 0x426DECA0 vs 0x421ECCA0) differs between runs.
    env.setdefault("TSFP_GUEST_PLACE", "det")
    return env


def configure_snapshot_host_env(env: dict[str, str]) -> dict[str, str]:
    """Keep recorded-input snapshots independent of live /dev/input handles."""
    if env.get("TSFP_SNAPSHOT_DIR"):
        env["TSFP_DISABLE_LIVE_GAMEPADS"] = "1"
    return env


def capture_x11_display(display_number: int, snapshot: bool) -> str:
    """Local X11 display; snapshot runs restrict Xvfb to the filesystem socket."""
    return f":{display_number}"


def capture_xvfb_transport_args(snapshot: bool) -> list[str]:
    """Snapshot runs use only the filesystem X11 socket (T2479).

    MEASURED: a TCP X11 pair stalls DMTCP's drain (Xvfb's accepted end is seen as
    external), and the default abstract socket fails with EBADF after restart.
    """
    return ["-nolisten", "tcp", "-nolisten", "local"] if snapshot else ["-nolisten", "tcp"]


def gpu_capture_x11_env(env: dict) -> dict:
    """Use Xwayland for Bevy; grim reconstructs Wayland access at capture time."""
    # Bevy/winit on the private long-socket Wayland path fails to create its
    # event loop. Keep the working Xwayland host environment.
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("WAYLAND_SOCKET", None)
    return env


def gpu_display_host_env(inherited: dict, display_env: dict) -> dict:
    """Keep caller-selected host switches while applying the private GPU display."""
    return {**inherited, **display_env}


def close_host(
    host: subprocess.Popen,
    display: str,
    env: dict[str, str],
    *,
    signal_first: bool = False,
    window: bool = True,
) -> tuple[int, str | None]:
    """Finish a capture even when the window disappears during the close request.

    T3390: a --render none host has no window; SIGTERM is its close request."""
    close_error = None
    if host.poll() is None and not window:
        host.send_signal(signal.SIGTERM)
    elif host.poll() is None:
        try:
            note = send_close(display, env=env)
            if not note.startswith("WM_DELETE_WINDOW sent"):
                raise RuntimeError(note)
        except (subprocess.SubprocessError, OSError, RuntimeError) as error:
            close_error = f"{type(error).__name__}: {error}"
            # T2304: SIGTERM reaches the host as an SDL quit event, the same clean shutdown (stop report,
            # per-function counters, exit dumps) as a window close, instead of a SIGKILL 90 s later.
            host.send_signal(signal.SIGTERM)
    else:
        return host.poll(), close_error
    try:
        return host.wait(timeout=60 if signal_first else 90), close_error
    except subprocess.TimeoutExpired:
        kill_pid(host.pid)
        return host.wait(), close_error


def default_xbe() -> str:
    """T2304: the worktree's build/default.xbe, else the main checkout's (worktrees need no manual symlink)."""
    for base in (ROOT, main_root()):
        candidate = base / "build/default.xbe"
        if candidate.exists():
            return str(candidate)
    return "build/default.xbe"


def trace_only_argv(argv: list[str]) -> list[str]:
    """CPU-only guest-state oracle: no Vulkan replay or live/window presenter.

    Replay still drives real guest ticks. It is a separate input identity and
    provides state evidence, not rendered-frame or GPU-query conformance.
    """
    switches = {
        "--gpu-live",
        "--gpu-live-inferred",
        "--gpu-replay-lenient",
        "--gpu-live-translate",
        "--wgpu-live",
        "--interactive",
        "--replay-handover",
    }
    valued = {
        "--gpu-replay",
        "--gpu-replay-undo-viewport",
        "--gpu-replay-window-to-clip",
        "--pad-source",
    }
    replacements = {"--present": "null"}
    result = argv[:2]
    index = 2
    while index < len(argv):
        token = argv[index]
        if token in switches:
            index += 1
        elif token in valued:
            index += 2
        elif token in replacements:
            result.extend([token, replacements[token]])
            index += 2
        else:
            result.append(token)
            index += 1
    return result


# Every --gpu-live* / --gpu-replay* / --live-* host option is a switch except these, which take a value
# (src/host/host_options.c).
RENDER_NONE_VALUED = frozenset(
    {
        "--gpu-replay",
        "--gpu-replay-dump",
        "--gpu-replay-size",
        "--gpu-replay-draw-dump",
        "--gpu-replay-standin-texture",
        "--gpu-replay-standin-texel-program",
        "--gpu-replay-standin-normalised-program",
        "--gpu-replay-dump-every",
        "--live-pipeline-cache",
        "--live-frame-hash",
        "--live-pipeline",
    }
)


def render_none_dropped(token: str) -> bool:
    # --audio-mute is only valid with the sdl sink; the null sink below is silent anyway.
    return token.startswith(("--gpu-live", "--gpu-replay", "--live-")) or token in (
        "--wgpu-live",
        "--audio-mute",
    )


def render_none_argv(argv: list[str]) -> list[str]:
    """T3390 `--render none`: drop the C live renderer and the Vulkan push-buffer replay and send
    audio to the null sink. `--present window` stays: in the Bevy build it is the input pump and
    picture mailbox (no SDL window, no GPU), and the replay's host polls come from it."""
    result = argv[:2]
    index = 2
    while index < len(argv):
        token = argv[index]
        if token in RENDER_NONE_VALUED:
            index += 2
        elif render_none_dropped(token):
            index += 1
        elif token == "--audio-sink":
            result.extend([token, "null"])
            index += 2
        else:
            result.append(token)
            index += 1
    return result


def render_mode(args: argparse.Namespace, native_route: bool) -> str:
    """T3390: `gpu` (renderer and window on a GPU) or `none`. `auto` is none for a fast-forward
    native .rec replay and gpu otherwise (paced, legacy routes, frame or screenshot captures)."""
    mode = getattr(args, "render", "auto") or "auto"
    if mode != "auto":
        return mode
    visual = getattr(args, "shot_at", None) or getattr(args, "shot_at_route_end", False)
    if native_route and not getattr(args, "paced", False) and not visual:
        return "none"
    return "gpu"


def consume_snapshot_resume_marker() -> float | None:
    """Once after rust_trace resume: None without a resume, else the new budget in seconds.

    0.0 means "the capture's own --seconds"; a `seconds=N` line (T2479, `resume --seconds`)
    shortens or lengthens the resumed window without retaking the snapshot.
    """
    # A resumed segment writes this marker into its pathvirt-mapped private
    # run directory. Retain the old snapshot-dir location for existing images.
    marker_root = os.environ.get("TSFP_SNAPSHOT_RUN_DIR") or os.environ.get("TSFP_SNAPSHOT_DIR")
    if not marker_root:
        return None
    marker = Path(marker_root) / "resume-requested"
    try:
        text = marker.read_text()
        output_match = re.search(r"^out_dir=(.+)$", text, re.MULTILINE)
        if output_match:
            # The marker is consumed while run_host is still polling. Preserve the
            # segment destination for collect(), which runs after this resume returns.
            os.environ["TSFP_SNAPSHOT_OUT_DIR"] = output_match.group(1)
        marker.unlink()
    except FileNotFoundError:
        return None
    match = re.search(r"^seconds=([0-9.]+)$", text, re.MULTILINE)
    return float(match.group(1)) if match else 0.0


def snapshot_resume_output_dir() -> str | None:
    """Output path supplied by the mapped resume marker, if this is a segment resume."""
    marker_root = os.environ.get("TSFP_SNAPSHOT_RUN_DIR") or os.environ.get("TSFP_SNAPSHOT_DIR")
    if not marker_root:
        return None
    try:
        text = (Path(marker_root) / "resume-requested").read_text()
    except OSError:
        return None
    match = re.search(r"^out_dir=(.+)$", text, re.MULTILINE)
    return match.group(1) if match else None


def dmtcp_host_argv(argv: list[str], env: dict[str, str]) -> list[str]:
    """Keep the host a direct child so it inherits the active DMTCP interposer."""
    if env.get("DMTCP_VIRTUAL_PID") and not env.get("TSFP_SNAPSHOT_DIR"):
        raise ValueError("DMTCP host launch requires TSFP_SNAPSHOT_DIR")
    # DMTCP's exec wrapper already tracks a direct child. Running dmtcp_launch
    # from inside a checkpointed process is rejected by DMTCP with exit 99.
    return argv


def dmtcp_child_options(env: dict[str, str]) -> dict[str, bool]:
    """Keep DMTCP's PID-map descriptors open across child execs in a snapshot run."""
    if env.get("DMTCP_VIRTUAL_PID"):
        # Python's close_fds=True closes DMTCP's inherited PID-map lock fd before
        # the exec interposer runs; DMTCP then aborts the new worker with EBADF.
        return {"close_fds": False}
    return {}


RECORDING_TIMING_ARGS = ("--vblank-poll-blank", "--vblank-worker-blanks", "100")
"""T3388: the FABRICATED vblank models the 2026-10 owner recordings were played under (default
until T2436, 2026-10-19 10:50). Without them every native route stops at the first loading gate
(T587 second-event refusal, 1 s, 7 samples), paced and fast-forward alike (MEASURED)."""


def recording_timing_args(args: argparse.Namespace, native_route: bool) -> list[str]:
    """Host args that reproduce the recording-time vblank timing for a native .rec replay."""
    if not native_route or getattr(args, "no_recording_timing", False):
        return []
    if any(
        arg.startswith(
            (
                "--vblank-poll-blank",
                "--no-vblank-poll-blank",
                "--vblank-worker-blanks",
                "--no-vblank-worker-blanks",
            )
        )
        for arg in args.host_arg
    ):
        return []
    return list(RECORDING_TIMING_ARGS)


ROUTE_END_WALL_CAP = 3600.0
"""T3388: safety wall cap of a route-end capture when --seconds is not given."""


def capture_wall_cap(args: argparse.Namespace) -> float:
    """Wall seconds before the graceful close: --seconds if given, else 75 s for legacy routes and
    ROUTE_END_WALL_CAP for a capture that stops at the recorded route end (T3388)."""
    if getattr(args, "seconds", None) is not None:
        return float(args.seconds)
    return ROUTE_END_WALL_CAP if getattr(args, "stop_at_route_end", False) else 75.0


def run_host(args: argparse.Namespace, spec: str, out: Path) -> dict:
    """Validate the route, then run the host under admission. A bad route fails before waiting for
    the host lock (I-T3865-15: the check ran only after admission, so with a game host already
    running it hung past every test timeout and sent main_guard bisecting a phantom)."""
    resolve_route(args.route)
    return _run_host_admitted(args, spec, out)


def _run_host_unadmitted(args: argparse.Namespace, spec: str, out: Path) -> dict:
    """`run_host.__wrapped__`: the same checks and run without host admission (fake-host tests)."""
    resolve_route(args.route)
    return _run_host_admitted.__wrapped__(args, spec, out)  # type: ignore[attr-defined]


run_host.__wrapped__ = _run_host_unadmitted  # type: ignore[attr-defined]


@serialized
def _run_host_admitted(args: argparse.Namespace, spec: str, out: Path) -> dict:
    """One headless host run to completion.  Returns {exit, seconds, report_ok}.  Admission and private resources last through cleanup."""
    from tools import datadisk

    out = datadisk.check_path(output_path(out), what="rust_trace out dir")  # T3745: data disk only
    datadisk.check("rust_trace", paths=[out])
    datadisk.no_core_dumps()  # T3745: a host core would stream to the root disk
    route = resolve_route(args.route)
    native_end_poll = native_input_route_end_poll(route)
    if native_end_poll is not None and getattr(args, "trace_only", False):
        raise ValueError(
            "--trace-only is not supported for native .rec routes; run the Bevy host windowed"
        )
    if getattr(args, "stop_at_route_end", False) and native_end_poll is None:
        raise ValueError(
            "--stop-at-route-end requires a native .rec route with a recorded end poll"
        )
    # T3388: a native .rec route runs to its recorded end by default; --seconds is only a wall cap.
    if native_end_poll is not None and not getattr(args, "no_stop_at_route_end", False):
        args.stop_at_route_end = True
    wall_cap = capture_wall_cap(args)
    # T3592: owner .rec routes run without the animation lane shadow (it aborts them, anim_lane.rs shadow mismatch)
    if getattr(args, "anim_shadow", None) is None:
        args.anim_shadow = "off" if native_end_poll is not None else "on"
    play_opts = None
    if native_end_poll is not None and getattr(args, "play_options", "default") != "off":
        play_opts = play_options(getattr(args, "play_options", "default"))
        if spec == "play":
            spec = play_opts["ports"].removeprefix("--rust-ports=")
    host_path = trace_host_for_route(
        args.host,
        route,
        native_end_poll is not None,
        getattr(args, "host_input_hash", None),
        getattr(args, "host_sha256", None),
    )
    if getattr(args, "t2_capture", False):
        from tools.t2capture import runtime_env

        runtime_env.check_capture_host(host_path)  # T3767: refuse at capture start
    regions = load_regions(json.loads(resolve(args.regions).read_text()))
    recipe_inputs = [host_path, resolve(args.disc), route, resolve(args.regions)]
    # T2479: hash before a DMTCP checkpoint can happen; the restored process must not re-read them.
    pinned_input_hashes = (
        hash_inputs(recipe_inputs) if os.environ.get("TSFP_SNAPSHOT_DIR") else None
    )
    out.mkdir(parents=True, exist_ok=True)
    for sub in ("hdd", "overlay", "dump"):
        (out / sub).mkdir(exist_ok=True)
    if getattr(args, "shot_at_route_end", False):
        (out / "shot-route-end.png").unlink(missing_ok=True)
        (out / "shot-route-end.error").unlink(missing_ok=True)
    argv = host_argv(
        str(host_path),
        out,
        resolve(args.disc),
        str(route),
        str(resolve(args.modules) if args.modules else main_root() / "tmp/play/modules"),
        [
            f"--rust-ports={spec}",
            *(["--show-fps"] if getattr(args, "show_fps", False) else []),
            "--dump-on-button",
            "--dump-after-frames",
            "2",
            "--dump-button-idle-every",
            str(args.idle_every),
            "--dump-button-max-idle",
            str(args.max_idle),
            "--dump-button-max-dumps",
            str(args.max_dumps),
            "--dump-overlay-max",
            str(getattr(args, "max_frames", 0)),
            *(
                ["--snapshot-at-poll", str(args.snapshot_at_poll)]
                if getattr(args, "snapshot_at_poll", None) is not None
                else []
            ),
            *(
                ["--snapshot-at-polls", ",".join(map(str, args.snapshot_at_polls))]
                if getattr(args, "snapshot_at_polls", None)
                else []
            ),
            *args.host_arg,
            *recording_timing_args(args, native_end_poll is not None),
        ],
    )
    if native_end_poll is not None:
        argv = native_route_argv(argv, route)
        argv = native_pacing_argv(argv, getattr(args, "paced", False))
        if play_opts is not None:
            argv = apply_play_args(argv, play_opts)
    render = render_mode(args, native_end_poll is not None)
    args.render = render
    if render == "none":
        argv = render_none_argv(argv)
    if getattr(args, "trace_only", False):
        argv = trace_only_argv(argv)
    argv = [default_xbe() if part == "build/default.xbe" else part for part in argv]
    if getattr(args, "save_dir", None):
        argv[argv.index("--hdd") + 1] = str(resolve(args.save_dir))
    elif native_end_poll is not None:
        # T2421: restore the recording's start storage into a scratch copy, never the owner's live saves
        from tools.play import rec_saves

        scratch = rec_saves.restore(route, out / "hdd")
        if scratch is not None:
            argv[argv.index("--hdd") + 1] = str(scratch)
            # T3592: a capture applies the recorded session options like tools.play (else the story cutscenes
            # are unskippable and the level never loads); native_route_argv already added them
    index = argv.index("--dump-guest-range")
    argv[index + 1] = ",".join(region.spec for region in regions)
    gpu_display = None
    snapshot_x11 = bool(os.environ.get("TSFP_SNAPSHOT_DIR"))
    if getattr(args, "gpu_display", False) and render != "none":
        from tools.wt.display import GpuDisplay

        gpu_display = GpuDisplay("1280x720")
        display_number = None
        display = gpu_display.name
        runtime = gpu_display.runtime
        (out / "gpu-display.json").write_text(
            json.dumps(
                {
                    "xdg_runtime_dir": str(runtime),
                    "wayland_socket": gpu_display.socket,
                    "x_display": display,
                },
                sort_keys=True,
            )
            + "\n"
        )
        # `GpuDisplay.env()` supplies the private display and GPU runtime, but
        # capture callers also pass host switches such as TSFP_RUST_FRAME_LOOP.
        base_env = gpu_display_host_env(os.environ, gpu_display.env())
    else:
        display_number = pick_display()
        display = capture_x11_display(display_number, snapshot_x11)
        runtime = output_path("tmp/xdg")
        runtime.mkdir(parents=True, exist_ok=True)
        runtime.chmod(0o700)
        base_env = os.environ
        if snapshot_dir := os.environ.get("TSFP_SNAPSHOT_DIR"):
            (Path(snapshot_dir) / "display-number").write_text(f"{display_number}\n")
    env = configure_snapshot_host_env(
        host_env(
            base_env,
            display,
            runtime,
            getattr(args, "anim_shadow", "on") == "on",
            gpu_display is not None,
        )
    )
    if play_opts is not None:
        apply_play_env(env, play_opts)
    env.pop("TSFP_FAST_FORWARD", None)  # T2487: native routes set it below unless --paced
    from tools.wt import prebuilt

    lease_context = None
    lease_guard = None
    if prebuilt.is_cached_host(host_path):
        host_library_path = str(host_path.parent)
        if env.get("LD_LIBRARY_PATH"):
            host_library_path += os.pathsep + env["LD_LIBRARY_PATH"]
        env["LD_LIBRARY_PATH"] = host_library_path
        lease_context = prebuilt.lease(host_path)
    if native_end_poll is not None:
        env["TSFP_SIMULATION_PACE"] = "1"
        env["TSFP_UI"] = "rust"
        env["TSFP_UI_DISC"] = str(resolve(args.disc))
        native_pacing_env(env, getattr(args, "paced", False))
        # T3393: recorded rows on the simulation tick, not the wall-clock-driven host poll count
        # (paced, fast-forward and none EQUAL on story-level1-20261018); --replay-clock poll opts out.
        env["TSFP_REPLAY_CLOCK"] = getattr(args, "replay_clock", "tick") or "tick"
        from tools import replay_raw_look

        raw_look = replay_raw_look.apply(env, resolve_route(args.route))
    else:
        raw_look = None
    env.pop("TSFP_RENDER", None)
    if render == "none":
        env["TSFP_RENDER"] = "none"  # T3390: headless Bevy, no window, GPU or audio output
    if getattr(args, "hud_state", None):
        env["TSFP_HUD_STATE_TRACE"] = str(out / "hud-state.trace")
        env["TSFP_HUD_ROWS_TRACE"] = str(out / "hud-rows.trace")
        env["TSFP_HUD_PRODUCERS"] = "1" if args.hud_state == "owned" else "0"
    if getattr(args, "rea_ai_state", None):
        if args.rea_ai_state == "owned" and "--rust-rea-ai" not in args.host_arg:
            raise ValueError("--rea-ai-state owned requires --host-arg=--rust-rea-ai")
        if args.rea_ai_state == "reference" and "--rust-rea-ai" in args.host_arg:
            raise ValueError("--rea-ai-state reference cannot be combined with --rust-rea-ai")
        env["TSFP_REA_AI_STATE_TRACE"] = str(out / "rea-ai-state.trace")
        env["TSFP_REA_AI_STATE_SELECTION"] = args.rea_ai_state
    if getattr(args, "t2_capture", False):
        from tools.t2capture import runtime_env

        seed = getattr(args, "t2_budget_seed", None)
        env.update(
            runtime_env.capture_env(
                out,
                route,
                getattr(args, "host_input_hash", None),
                budget_seed=resolve(seed) if seed else None,
            )
        )
        targets = getattr(args, "t2_capture_targets", None)
        if targets:  # T3833: ALLOW list, only these VAs record
            from tools.t2capture.live_targets import TARGETS_ENV

            env[TARGETS_ENV] = str(resolve(targets))
    if getattr(args, "call_args", None):
        env["TSFP_CALL_ARGS_TRACE"] = str(out / "call-args.trace")
        env["TSFP_CALL_ARGS_VAS"] = ",".join(
            va.strip().removeprefix("0x") for va in args.call_args.split(",")
        )
    if getattr(args, "trace_only", False):
        env.pop("TSFP_WGPU_LIVE", None)
    if getattr(args, "pad_only_ui", False):
        env["TSFP_UI_PAD_REPLAY"] = "1"
    frame_shot = configure_frame_shot(out, env, render)
    from tools.hostresources import prepare

    try:
        argv, env = prepare(argv, env, out, display=display_number, cwd=ROOT)
        env["DISPLAY"] = display
        # T3592: the host command a capture ran, so option drift against `play.sh --emit-options` is diffable
        host_options = {
            "argv": argv[1:],
            "env": {k: v for k, v in sorted(env.items()) if k.startswith("TSFP_")},
        }
        (out / "host-options.json").write_text(json.dumps(host_options, indent=1))
        if play_opts is not None:
            for line in play_options_drift(host_options, play_opts):
                print(f"capture: play.sh option drift: {line}", file=sys.stderr)
        argv = dmtcp_host_argv(argv, env)
        if gpu_display:
            gpu_capture_x11_env(env)
        xvfb = (
            None
            if gpu_display or render == "none"
            else subprocess.Popen(
                [
                    "Xvfb",
                    f":{display_number}",
                    "-auth",
                    env["XAUTHORITY"],
                    *capture_xvfb_transport_args(snapshot_x11),
                    "-screen",
                    "0",
                    "1280x720x24",
                ],
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                **dmtcp_child_options(env),
            )
        )
    except BaseException:
        from tools.wt.display import release_number

        if gpu_display:
            gpu_display.close()
        else:
            release_number(display_number)
        raise
    host = None
    log = None
    compactor = None  # T3745 live compaction of a --t2-capture spool
    route_log = None
    status = None
    output_cap_error = None
    route_end_shot = False
    if frame_shot is None:
        frame_shot_frame, frame_shot_watcher, frame_shot_ready = None, None, None
    else:
        frame_shot_frame, frame_shot_watcher, frame_shot_ready = frame_shot
    try:
        if lease_context is not None:
            lease_context.__enter__()
            lease_guard = lease_context
        log = (out / "run.log").open("wb")
        if render != "none":
            time.sleep(2)  # display start-up; T3390: --render none has no display
        host = subprocess.Popen(
            argv,
            cwd=ROOT,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            **dmtcp_child_options(env),
        )
        from tools.t2capture.runtime_env import preserve_all_calls

        if getattr(args, "t2_capture", False) and not preserve_all_calls(env):
            # An all-calls capture needs its ordered spool; the corpus compactor samples it.
            from tools.datadir import data_dir
            from tools.t2corpus.compact import Compactor

            compactor = Compactor(
                Path(env["TSFP_T2_CAPTURE"]),
                data_dir("t2corpus"),
                datadisk.load_budget().capture_compact_interval_s,
            ).start()
        if getattr(args, "stop_at_route_end", False):
            route_log = (out / "run.log").open("rb")
        # Admission and resource preparation do not consume gameplay capture time.
        started = time.monotonic()
        deadline = started + wall_cap
        shots = sorted(getattr(args, "shot_at", []) or [])
        stop_at_route_end = getattr(args, "stop_at_route_end", False)
        route_end_tail = b""
        try:
            while host.poll() is None:
                if route_log is not None:
                    route_bytes = route_end_tail + route_log.read()
                    if b" route end " in route_bytes:
                        print("rust_trace: recorded route end reached; closing host", flush=True)
                        if getattr(args, "shot_at_route_end", False) and not route_end_shot:
                            route_end_shot = capture_route_end_shot(out, env)
                        break
                    route_end_tail = route_bytes[-32:]
                resumed_budget = consume_snapshot_resume_marker()
                if resumed_budget is not None:
                    started = time.monotonic()
                    if route_log is None:
                        deadline = started + (resumed_budget or wall_cap)
                if time.monotonic() >= deadline:
                    break
                time.sleep(0.005 if frame_shot_watcher is not None else 0.5)
                check_output([out], args.max_files, args.max_bytes)
                if frame_shot_watcher is not None:
                    try:
                        trigger_frame = frame_shot_watcher.poll()
                    except (OSError, ValueError) as error:
                        (out / f"shot-frame-{frame_shot_frame}.error").write_text(
                            f"could not read frame draw capture: {error}\n"
                        )
                        frame_shot_ready.touch()
                        frame_shot_watcher = None
                    else:
                        if trigger_frame is not None:
                            capture_frame_boundary_shot(
                                out,
                                env,
                                frame_shot_frame,
                                trigger_frame,
                                frame_shot_ready,
                            )
                            frame_shot_watcher = None
                if stop_at_route_end and route_end_poll_from_logs(out) is not None:
                    # Capture while the Bevy window is still alive. Closing the host first
                    # makes the later route-end screenshot attempt fail even though the
                    # deterministic route-end marker was observed.
                    if getattr(args, "shot_at_route_end", False) and not route_end_shot:
                        route_end_shot = capture_route_end_shot(out, env)
                    status, close_error = close_host(
                        host, display, env, signal_first=bool(gpu_display), window=render != "none"
                    )
                    if close_error:
                        (out / "close-error.txt").write_text(close_error + "\n")
                    break
                while shots and time.monotonic() - started >= shots[0]:
                    moment = shots.pop(0)
                    shot_path = out / f"shot-{moment:g}.png"
                    if not capture_bevy_window_shot(shot_path, env):
                        (out / f"shot-{moment:g}.error").write_text(
                            "could not capture the visible Bevy window\n"
                        )
                if (
                    not output_cap_error
                    and getattr(args, "shot_at_route_end", False)
                    and not route_end_shot
                ):
                    route_end_shot = capture_route_end_shot(out, env)
        except OutputCapExceeded as error:
            output_cap_error = str(error)
            (out / "output-cap.error").write_text(str(error) + "\n")
            print(f"rust_trace: {error}", file=sys.stderr, flush=True)
            status = stop_after_cap(host)
        if frame_shot_watcher is not None:
            (out / f"shot-frame-{frame_shot_frame}.error").write_text(
                f"did not observe the first draw for renderer frame {frame_shot_frame + 1}\n"
            )
            frame_shot_ready.touch()
            frame_shot_watcher = None
        if (
            not output_cap_error
            and getattr(args, "shot_at_route_end", False)
            and not route_end_shot
        ):
            route_end_shot = capture_route_end_shot(out, env)
        if (
            not output_cap_error
            and getattr(args, "shot_at_route_end", False)
            and not route_end_shot
        ):
            (out / "shot-route-end.error").write_text(
                "route end was not observed before host shutdown\n"
            )
        if status is None:
            status, close_error = close_host(
                host, display, env, signal_first=bool(gpu_display), window=render != "none"
            )
            if close_error:
                (out / "close-error.txt").write_text(close_error + "\n")
    finally:
        if frame_shot_watcher is not None:
            (out / f"shot-frame-{frame_shot_frame}.error").write_text(
                f"frame screenshot stopped before renderer frame {frame_shot_frame + 1}\n"
            )
            frame_shot_ready.touch()
        if route_log is not None:
            route_log.close()
        if host is not None and host.poll() is None:
            kill_pid(host.pid)
            host.wait()
        if compactor is not None:
            ledger = compactor.stop()  # final pass: the host has exited
            print(f"t2 capture compaction: {ledger}", file=sys.stderr, flush=True)
        if gpu_display:
            gpu_display.close()
        elif xvfb is None:
            from tools.wt.display import release_number

            release_number(display_number)  # T3390: --render none started no Xvfb
        else:
            stop_xvfb(xvfb, display_number)  # T2141: reap Xvfb and remove its lock and socket
        if log is not None:
            log.close()
        if lease_guard is not None:
            lease_guard.__exit__(None, None, None)
    seconds = time.monotonic() - started
    recipe_command = [
        sys.executable,
        "-m",
        "tools.rust_trace",
        "capture",
        "--host",
        str(host_path),
        "--disc",
        str(resolve(args.disc)),
        "--route",
        str(route),
        "--regions",
        str(resolve(args.regions)),
        "--rust-ports",
        spec,
        "--out-dir",
        str(out),
        "--anim-shadow",
        getattr(args, "anim_shadow", "on"),
    ]
    if getattr(args, "trace_only", False):
        recipe_command.append("--trace-only")
    if getattr(args, "paced", False):
        recipe_command.append("--paced")
    if getattr(args, "no_recording_timing", False):
        recipe_command.append("--no-recording-timing")
    if getattr(args, "stop_at_route_end", False):
        recipe_command.append("--stop-at-route-end")
    for name in ("seconds", "idle_every", "max_idle", "max_dumps"):
        if getattr(args, name) is not None:
            recipe_command += ["--" + name.replace("_", "-"), str(getattr(args, name))]
    for argument in args.host_arg:
        recipe_command += ["--host-arg=" + argument]
    for shot in getattr(args, "shot_at", []):
        recipe_command += ["--shot-at", str(shot)]
    if getattr(args, "show_fps", False):
        recipe_command.append("--show-fps")
    if getattr(args, "shot_at_route_end", False):
        recipe_command.append("--shot-at-route-end")
    if getattr(args, "hud_state", None):
        recipe_command += ["--hud-state", args.hud_state]
    if getattr(args, "rea_ai_state", None):
        recipe_command += ["--rea-ai-state", args.rea_ai_state]
    if getattr(args, "call_args", None):
        recipe_command += ["--call-args", args.call_args]
    if getattr(args, "t2_capture", False):
        recipe_command.append("--t2-capture")
    if getattr(args, "save_dir", None):
        recipe_command += ["--save-dir", args.save_dir]
    if getattr(args, "snapshot_at_poll", None) is not None:
        recipe_command += ["--snapshot-at-poll", str(args.snapshot_at_poll)]
    if getattr(args, "snapshot_at_polls", None):
        recipe_command += ["--snapshot-at-polls", ",".join(map(str, args.snapshot_at_polls))]
    if getattr(args, "snapshot_dir", None):
        recipe_command += ["--snapshot-dir", str(args.snapshot_dir)]
    if getattr(args, "snapshot_gzip", False):
        recipe_command.append("--snapshot-gzip")
    if getattr(args, "gpu_display", False):
        recipe_command += ["--gpu-display"]
    recipe_command += ["--play-options", getattr(args, "play_options", "default") or "default"]
    recipe_command += ["--render", getattr(args, "render", "auto") or "auto"]
    recipe_command += ["--replay-clock", getattr(args, "replay_clock", "tick") or "tick"]
    if getattr(args, "pad_only_ui", False):
        recipe_command += ["--pad-only-ui"]
    write_recipe(
        out,
        command=recipe_command,
        inputs=recipe_inputs,
        wall_seconds=seconds,
        host_lock=True,
        input_hashes=pinned_input_hashes,
    )
    return {
        "exit": status,
        "seconds": round(seconds, 1),
        "trace_only": getattr(args, "trace_only", False),
        "output_cap_error": output_cap_error,
        "host_path": str(host_path),
        "raw_look_hold": raw_look,
    }


RENDER_READS_RE = re.compile(
    r"render reads\s+T3390 visibility queries: begin (\d+), end (\d+), result reads (\d+) "
    r"\(pending (\d+), answered (\d+)\)"
)


def parse_render_reads(report: str) -> dict | None:
    """T3390 run-time render-read detector from the host's exit receipt (src/gpu/d3d8_visibility.c).

    `guest_reads_render` is true when the guest asked for a visibility result: a --render none run
    has no renderer to answer it, so such a route needs --render gpu (or proof it does not matter)."""
    match = RENDER_READS_RE.search(report)
    if match is None:
        return None
    begin, end, results, pending, answered = (int(value) for value in match.groups())
    return {
        "visibility_begin": begin,
        "visibility_end": end,
        "visibility_result_reads": results,
        "visibility_pending": pending,
        "visibility_answered": answered,
        "guest_reads_render": results > 0 or end > 0,
    }


def desync_meta(route: str, out: Path, route_end: int | None, recorded: int | None) -> dict:
    """T3819: `desync` (tools.replay_desync result) and `desync_poll` (None = no divergence seen)."""
    from tools import replay_desync

    reached = isinstance(route_end, int) and isinstance(recorded, int) and route_end >= recorded
    try:
        result = replay_desync.detect(resolve_route(route), out, reached_route_end=reached)
    except Exception as error:  # noqa: BLE001 - degrade loudly, never lose the trace
        result = {"verdict": "unknown", "desync_poll": None, "reason": f"detector error: {error}"}
    return {"desync": result, "desync_poll": result.get("desync_poll")}


def collect(args: argparse.Namespace, spec: str, out: Path, meta: dict) -> dict:
    """Turn the dumps and the stop report of a finished run into out/trace.json (hex regions) and return the trace dict."""
    resumed_out = os.environ.get("TSFP_SNAPSHOT_OUT_DIR")
    if resumed_out:
        out = output_path(resumed_out)
    regions = load_regions(json.loads(resolve(args.regions).read_text()))
    names = [region.name for region in regions]
    manifest = out / "dump" / "buttons.jsonl"
    samples = []
    if manifest.exists():
        for line in manifest.read_text().splitlines():
            event = json.loads(line)
            for dump in event.get("dumps", []) if event.get("type") == "event" else []:
                path = out / "dump" / dump["file"]
                if path.exists():
                    key = f"{dump['poll']}:{dump['kind']}"
                    samples.append(
                        {"key": key, "poll": dump["poll"], "regions": read_dump_file(path, names)}
                    )
    route_end = route_end_poll_from_logs(out)
    recorded_route_end = native_input_route_end_poll(resolve_route(args.route))
    from tools import replay_clock

    report = (out / "run.log").read_text(errors="replace") if (out / "run.log").exists() else ""
    counters = parse_counters(report)
    trace = {
        "meta": {
            **meta,
            "spec": spec,
            "route_end_poll": route_end,
            "recorded_route_end_poll": recorded_route_end,
            "route_clock": (
                {"domain": "guest-pad-exclusive-v1", **vars(clock)}
                if (clock := replay_clock.parse_marker(report, route_end))
                else None
            ),
            "render_reads": parse_render_reads(report),
            **desync_meta(args.route, out, route_end, recorded_route_end),
        },
        "counters": counters,
        "samples": samples,
    }
    if getattr(args, "hud_state", None):
        from tools.hud_state_trace import load, load_rows

        state_path = out / "hud-state.trace"
        trace["hud_state"] = load(state_path) if state_path.exists() else []
        if not state_path.exists():
            trace["meta"]["hud_state_error"] = "no authoritative HUD state file produced"
        row_path = out / "hud-rows.trace"
        trace["hud_rows"] = load_rows(row_path) if row_path.exists() else []
        trace["meta"]["hud_rows_receipt"] = (
            f"HUD row trace samples={len(trace['hud_rows'])} errors=0" in report
        )
        row_owned = re.search(r"HUD row trace samples=\d+ errors=\d+ owned_samples=(\d+)", report)
        trace["meta"]["hud_rows_owned_samples"] = int(row_owned[1]) if row_owned else None
        trace["meta"]["hud_state_selection"] = args.hud_state
        receipt = f"HUD state trace samples={len(trace['hud_state'])} errors=0"
        trace["meta"]["hud_state_receipt"] = receipt in report
        owned = re.search(r"HUD state trace samples=\d+ errors=\d+ owned_samples=(\d+)", report)
        trace["meta"]["hud_state_owned_samples"] = int(owned[1]) if owned else None
    if getattr(args, "rea_ai_state", None):
        from tools.rea_ai_state_trace import load

        state_path = out / "rea-ai-state.trace"
        trace["rea_ai_state"] = load(state_path) if state_path.exists() else []
        trace["meta"]["rea_ai_state_selection"] = args.rea_ai_state
        trace["meta"]["rea_ai_state_receipt"] = state_path.exists()
    if getattr(args, "t2_capture", False):
        from tools.t2capture import runtime_env

        trace["meta"]["t2_capture"] = runtime_env.capture_meta(out)
    if getattr(args, "call_args", None):
        from tools.rust_trace_call_args import EMBED_LIMIT_BYTES, scan
        from tools.rust_trace_call_args import load as load_call_args

        call_path = out / "call-args.trace"
        if call_path.exists() and call_path.stat().st_size > EMBED_LIMIT_BYTES:
            # T3388: too big to embed in memory; trace.json points at the streamed file.
            trace["meta"]["call_args_file"] = call_path.name
            trace["meta"]["call_args_scan"] = scan(call_path)
        else:
            trace["call_args"] = load_call_args(call_path) if call_path.exists() else []
    (out / "trace.json").write_text(
        json.dumps(
            {
                **({"call_args": trace["call_args"]} if "call_args" in trace else {}),
                **(
                    {"hud_state": trace["hud_state"], "hud_rows": trace["hud_rows"]}
                    if "hud_state" in trace
                    else {}
                ),
                **({"rea_ai_state": trace["rea_ai_state"]} if "rea_ai_state" in trace else {}),
                "meta": trace["meta"],
                "counters": counters,
                "samples": [
                    {
                        "key": s["key"],
                        "poll": s["poll"],
                        "regions": {k: v.hex() for k, v in s["regions"].items()},
                    }
                    for s in samples
                ],
            }
        )
    )
    return trace


def load_trace(directory: Path) -> dict:
    raw = json.loads((directory / "trace.json").read_text())
    for sample in raw["samples"]:
        sample["regions"] = {k: bytes.fromhex(v) for k, v in sample["regions"].items()}
    return raw


def capture(args: argparse.Namespace, spec: str, out: Path) -> dict:
    out = output_path(out)
    result = run_host(args, spec, out)
    # Fast-forward can finish the guest between the checkpoint resume and the
    # next host.poll() iteration. In that case run_host never consumes the
    # resume marker, so collect() would write to the checkpoint's shared path.
    consume_snapshot_resume_marker()
    trace = collect(
        args,
        spec,
        out,
        {
            "host_exit": result["exit"],
            "seconds": result["seconds"],
            "route": args.route,
            "trace_only": result.get("trace_only", False),
            "output_cap_error": result.get("output_cap_error"),
            "raw_look_hold": result.get("raw_look_hold"),  # T3819
        },
    )

    host_path = result.get("host_path")
    if host_path:
        from tools import prebuilt_select

        trace["meta"]["host_input_hash"] = prebuilt_select.host_input_hash(Path(host_path), ROOT)
        trace["meta"]["host_path"] = host_path
    print(
        f"capture {spec[:40]!r}: {len(trace['samples'])} samples, exit {result['exit']}, {result['seconds']} s",
        flush=True,
    )
    return trace


def strict_limit(args: argparse.Namespace, baseline: dict) -> int:
    """Last deterministic poll: explicit limit, route end, or last captured sample (+ margin)."""
    # A native route's recorded completion field is a host poll, not a sample poll.
    meta = baseline["meta"]
    if meta.get("recorded_route_end_poll") is not None and meta.get("route_end_poll") is not None:
        from tools import replay_clock

        end = replay_clock.guest_limit([baseline])
        if end is None:
            raise ValueError("native replay guest-pad completion boundary missing")
        return min(args.strict_poll, end) if args.strict_poll else end
    if args.strict_poll:
        return args.strict_poll
    end = meta.get("route_end_poll")
    if end is None:
        end = max((int(sample["poll"]) for sample in baseline.get("samples", [])), default=0)
    return end + args.window_margin


def route_poll_offset(trace: dict) -> int:
    """Return the offset for samples keyed by the guest controller poll.

    Route completion metadata uses a host-poll clock. It cannot establish an
    origin shift for button-dump samples, whose keys already use guest polls.
    Explicit state anchors remain available for traces that need another basis.
    """
    return 0


def comparable_route_offsets(traces: list[dict]) -> list[int]:
    """Route offsets for traces that are compared with each other.

    An offset needs the observed route end. A run cut off by the wall-clock cap has none and
    would default to 0 while a finished peer reports its own end-of-route skew (typically 1),
    which shifts every peer sample off the poll grid (T3397 hud-messages: 0 common samples,
    byte-identical at equal raw polls). When any trace's offset is unknown, all use raw polls.
    """
    known = [
        isinstance(t.get("meta", {}).get("route_end_poll"), int)
        and isinstance(t.get("meta", {}).get("recorded_route_end_poll"), int)
        for t in traces
    ]
    if all(known):
        return [route_poll_offset(t) for t in traces]
    return [0 for _ in traces]


def parse_region_anchor(value: str | None) -> tuple[str, int] | None:
    """Parse NAME=U32 for aligning traces on a natural region value."""
    if value is None:
        return None
    name, separator, raw = value.partition("=")
    if not separator or not name.strip():
        raise ValueError("--align-region must be NAME=U32, for example level_state=0x18")
    try:
        expected = int(raw, 0)
    except ValueError as error:
        raise ValueError(f"invalid --align-region u32 value: {raw!r}") from error
    if not 0 <= expected <= 0xFFFFFFFF:
        raise ValueError("--align-region value must fit in u32")
    return name.strip(), expected


def region_anchor_poll(trace: dict, name: str, expected: int) -> int:
    """Anchor on a stable pre-frame sample of the requested region value."""
    matching = []
    for sample in trace.get("samples", []):
        raw = sample.get("regions", {}).get(name)
        if isinstance(raw, str):
            try:
                raw = bytes.fromhex(raw)
            except ValueError:
                continue
        if isinstance(raw, (bytes, bytearray)) and len(raw) >= 4:
            if int.from_bytes(raw[:4], "little") == expected:
                matching.append(sample)
    if matching:
        first = matching[0]
        phase = first.get("key", "").partition(":")[2]
        if phase == "idle-after":
            # A mode change can first be visible in the after-frame dump on one
            # run and the before-frame dump on another. Anchor both at the next
            # before-frame sample that still has the requested value.
            for sample in matching[1:]:
                if sample.get("key", "").partition(":")[2] == "idle-before":
                    return int(sample["poll"])
            raise ValueError(
                f"trace has no idle-before sample with {name}=0x{expected:08X} "
                "after the first idle-after match"
            )
        return int(first["poll"])
    raise ValueError(f"trace has no sample with {name}=0x{expected:08X}")


def route_relative_samples(
    trace: dict, offset: int | None = None, *, trim_before_anchor: bool = False
) -> list[dict]:
    """Return samples with the chosen host-poll offset removed from poll and key."""
    if offset is None:
        offset = route_poll_offset(trace)
    if not offset and not trim_before_anchor:
        return trace["samples"]
    result = []
    for sample in trace["samples"]:
        poll = sample["poll"] - offset
        if trim_before_anchor and poll < 0:
            continue
        key = sample["key"]
        _, separator, phase = key.partition(":")
        result.append(
            {
                **sample,
                "key": f"{poll}:{phase}" if separator else key,
                "poll": poll,
            }
        )
    return result


def split_window(samples: list[dict], limit: int) -> tuple[list[dict], list[dict]]:
    return [s for s in samples if s["poll"] <= limit], [s for s in samples if s["poll"] > limit]


COUNTER_TAIL_REL_TOLERANCE = 5e-4
COUNTER_TAIL_ABS_TOLERANCE = 4


def counters_match(left: dict | None, right: dict | None) -> bool:
    """Whole-run exit counters equal; they may differ by the wall-clock-driven tail.

    The replay stops on wall time after the deterministic window, so two identical runs differ by
    a few calls (T3879, MEASURED 1..80 on functions of 185 to 17.8 M calls). Allowed: at most
    COUNTER_TAIL_ABS_TOLERANCE calls, or COUNTER_TAIL_REL_TOLERANCE of the larger count. A change of the
    rust/lifted split stays a mismatch.
    """
    if left == right:
        return True
    if not isinstance(left, dict) or not isinstance(right, dict):
        return False
    if (left.get("rust") == 0) != (right.get("rust") == 0):
        return False
    totals = [(c.get("rust") or 0) + (c.get("lifted") or 0) for c in (left, right)]
    allowed = max(COUNTER_TAIL_ABS_TOLERANCE, max(totals) * COUNTER_TAIL_REL_TOLERANCE)
    return abs(totals[0] - totals[1]) <= allowed


def build_report(
    args: argparse.Namespace, left: dict, right: dict, control: dict | None, label: tuple[str, str]
) -> dict:
    region_config = json.loads(resolve(args.regions).read_text())
    regions = load_regions(region_config)
    policy = Policy(abs_eps=args.abs_eps, rel_eps=args.rel_eps)
    notes = []
    controls = control if isinstance(control, list) else ([control] if control else [])
    control = controls[0] if controls else None
    peers = [left, right, *controls]
    native_finished = any(
        isinstance((trace.get("meta") or {}).get("recorded_route_end_poll"), int)
        and isinstance((trace.get("meta") or {}).get("route_end_poll"), int)
        for trace in peers
    )
    if native_finished:
        from tools import replay_clock

        guest_end = replay_clock.guest_limit(peers)
        if guest_end is None:
            raise ValueError("native replay guest-pad completion boundary missing or unequal")
        # Explicit limits are guest-pad polls too; never include samples past the boundary.
        requested = getattr(args, "strict_poll", None)
        limit = min(requested, guest_end) if requested else guest_end
    else:
        limit = strict_limit(args, left)
    anchor = parse_region_anchor(getattr(args, "align_region", None))
    if anchor is None:
        left_offset, right_offset, *control_offsets = comparable_route_offsets(
            [left, right, *controls]
        )
        trim_before_anchor = False
        alignment_source = "guest controller poll keys; completion metadata is not an origin"
    else:
        region_name, expected = anchor
        region_names = {region.name for region in regions}
        if region_name not in region_names:
            raise ValueError(f"--align-region names unknown region: {region_name}")
        left_offset = region_anchor_poll(left, region_name, expected)
        right_offset = region_anchor_poll(right, region_name, expected)
        control_offsets = [region_anchor_poll(c, region_name, expected) for c in controls]
        trim_before_anchor = True
        alignment_source = (
            f"first {region_name}=0x{expected:08X} sample; idle-after normalized to the next "
            "idle-before sample; pre-anchor samples trimmed"
        )
    # A guest boundary is an absolute poll per run: under region alignment each run's relative
    # bound differs, and the strict window ends at the earliest so no run contributes past its own.
    route_limit = (
        min(limit - offset for offset in [left_offset, right_offset, *control_offsets])
        if native_finished
        else limit - left_offset
    )
    left_samples = route_relative_samples(left, left_offset, trim_before_anchor=trim_before_anchor)
    right_samples = route_relative_samples(
        right, right_offset, trim_before_anchor=trim_before_anchor
    )
    if controls:
        policy.noise, policy.noise_samples = noise_masks_multi(
            regions,
            [
                # Post-boundary samples never teach the noise mask of a finished native replay.
                split_window(samples, route_limit)[0] if native_finished else samples
                for samples in [
                    left_samples,
                    *(
                        route_relative_samples(c, offset, trim_before_anchor=trim_before_anchor)
                        for c, offset in zip(controls, control_offsets, strict=True)
                    ),
                ]
            ],
        )
        notes.append(
            f"noise from the off/off control ({len(controls) + 1} runs): {sum(len(v) for v in policy.noise.values())} global dwords (pointers) in "
            f"{len(policy.noise)} region(s), {sum(len(w) for r in policy.noise_samples.values() for w in r.values())} per-sample jitter words"
        )
    strict_l, tail_l = split_window(left_samples, route_limit)
    strict_r, tail_r = split_window(right_samples, route_limit)
    if native_finished and (not strict_l or not strict_r):
        raise ValueError(
            "native replay guest-pad completion boundary has empty comparison coverage"
        )
    comparison = compare_traces(regions, strict_l, strict_r, policy)
    comparison["route_poll_alignment"] = {
        "left_offset": left_offset,
        "right_offset": right_offset,
        "control_offsets": control_offsets,
        "relative_limit": route_limit,
        "source": alignment_source,
    }
    if anchor is not None:
        notes.append(
            f"poll alignment: {alignment_source}; left/right offsets={left_offset}/{right_offset}, "
            f"control offsets={control_offsets}"
        )
    if comparison["samples_compared"] < args.min_samples:
        comparison["equal"] = False
        comparison["reason"] = (
            f"fewer than {args.min_samples} common samples in the deterministic window"
        )
    tail = compare_traces(regions, tail_l, tail_r, policy)
    if control is not None:
        tail["control_diverging_samples"] = compare_traces(
            regions,
            tail_l,
            split_window(
                route_relative_samples(
                    control,
                    control_offsets[0],
                    trim_before_anchor=trim_before_anchor,
                ),
                route_limit,
            )[1],
            Policy(abs_eps=args.abs_eps, rel_eps=args.rel_eps),
        )["diverging_samples"]
    notes.append(
        f"deterministic window: route-relative polls <= {route_limit}; free-running tail: {tail['samples_compared']} samples compared, "
        f"{tail['diverging_samples']} diverging (wall-clock driven after the hand-over, {'FAILS' if args.strict_tail else 'informational'}; "
        f"unmasked off/off control diverging in the tail: {tail.get('control_diverging_samples', 'n/a')})"
    )
    equal = comparison["equal"] and (tail["equal"] or not args.strict_tail)
    if "hud_state" in left or "hud_state" in right:
        from tools.hud_state_trace import compare

        logical = compare(
            left.get("hud_state", []), right.get("hud_state", []), limit, args.min_samples
        )
        receipts = left["meta"].get("hud_state_receipt") and right["meta"].get("hud_state_receipt")
        logical["receipts_ok"] = bool(receipts)
        ownership = all(
            trace["meta"].get("hud_state_owned_samples")
            == (
                len(trace.get("hud_state", []))
                if trace["meta"].get("hud_state_selection") == "owned"
                else 0
            )
            for trace in (left, right)
        )
        logical["ownership_ok"] = ownership
        logical["equal"] = logical["equal"] and bool(receipts) and ownership
        comparison["hud_state"] = logical
        if "hud_rows" in left or "hud_rows" in right:
            from tools.hud_state_trace import compare_rows

            rows = compare_rows(
                left.get("hud_rows", []), right.get("hud_rows", []), limit, args.min_samples
            )
            rows["receipts_ok"] = bool(
                left["meta"].get("hud_rows_receipt") and right["meta"].get("hud_rows_receipt")
            )
            rows["ownership_ok"] = all(
                trace["meta"].get("hud_rows_owned_samples")
                == (
                    len(trace.get("hud_rows", []))
                    if trace["meta"].get("hud_state_selection") == "owned"
                    else 0
                )
                for trace in (left, right)
            )
            rows["equal"] = rows["equal"] and rows["receipts_ok"] and rows["ownership_ok"]
            comparison["hud_rows"] = rows
            equal = equal and rows["equal"]
        equal = equal and logical["equal"]
        notes.append(f"HUD authoritative lifecycle: {logical}")
    if "rea_ai_state" in left or "rea_ai_state" in right:
        from tools.rea_ai_state_trace import compare

        logical = compare(
            left.get("rea_ai_state", []),
            right.get("rea_ai_state", []),
            limit,
            args.min_samples,
            left.get("meta", {}).get("rea_ai_state_selection"),
            right.get("meta", {}).get("rea_ai_state_selection"),
            left.get("meta", {}).get("route_end_poll"),
            right.get("meta", {}).get("route_end_poll"),
            left.get("meta", {}).get("recorded_route_end_poll"),
            # The trace selects the scheduler's AI tick root. 0x17570 is an
            # indirect boundary called by that root and is not itself in the
            # selected-root counter table.
            rea_ai_tick_root_calls(right),
        )
        receipts = left.get("meta", {}).get("rea_ai_state_receipt") and right.get("meta", {}).get(
            "rea_ai_state_receipt"
        )
        logical["receipts_ok"] = bool(receipts)
        logical["equal"] = logical["equal"] and logical["receipts_ok"]
        comparison["rea_ai_state"] = logical
        equal = equal and logical["equal"]
        notes.append(f"REA AI authoritative actor state: {logical}")
    # T3496: two call streams with no row at all (the switch-off sides of roots that run lifted,
    # T2409's frame-loop roots) are identical for an off/off control, though no candidate evidence.
    calls_empty_both = False
    equal_before_calls = False
    if "call_args" in left or "call_args" in right:
        from tools.rust_trace_call_args import compare as compare_call_args

        call_left = left
        call_controls = controls
        call_limit = limit
        call_reference = getattr(args, "call_args_baseline", None)
        call_reference_control = getattr(args, "call_args_control", None)
        if call_reference:
            # Mutation-power runs compare selected Rust against the same selected
            # root without mutation. The ordinary off trace has no Rust exit row.
            call_left = load_trace(output_path(call_reference))
            call_controls = [load_trace(output_path(call_reference_control))]
            call_limit = strict_limit(args, call_left)
            notes.append(
                "call-argument reference: selected, unmutated Rust trace with a selected-run control"
            )
        calls = compare_call_args(
            call_left.get("call_args", []),
            right.get("call_args", []),
            [c.get("call_args", []) for c in call_controls],
            call_limit,
            [int(va.strip(), 16) for va in args.call_args.split(",")]
            if getattr(args, "call_args", None)
            else None,
        )
        calls["empty_both"] = calls_empty_both = not call_left.get("call_args") and not right.get(
            "call_args"
        )
        comparison["call_args"] = calls
        notes.extend(calls.get("stack_residue_health", []))
        equal_before_calls = equal
        equal = equal and calls["equal"]
        notes.append(
            f"call-argument stream (T2346): {calls['rows_compared']} rows, {calls['diverging_rows']} diverging, "
            f"{calls['missing_rows']} missing, {calls['masked_fields']} control-masked fields, first {calls['first_divergence']}"
        )
    missing_streams = [
        name
        for name in region_config.get("_required_streams", [])
        if name not in left or name not in right
    ]
    if missing_streams:
        equal = False
        notes.append(f"required authoritative streams missing: {missing_streams}")
    capped_inputs = [
        name
        for name, item in [("left", left), ("right", right)]
        + [(f"control{index + 1}", item) for index, item in enumerate(controls)]
        if item.get("meta", {}).get("output_cap_error")
    ]
    if capped_inputs:
        equal = False
        notes.append(f"output-capped captures cannot establish equality: {capped_inputs}")
    comparison["equal"] = equal
    # Off/off repeatability (the gate's control): every check equal, or every check but an empty
    # call stream on both sides. Whole-run counters are exit-report totals, so compare them even
    # when sampled guest state and call streams stop at an earlier common poll.
    left_counters = left.get("counters")
    right_counters = right.get("counters")
    left_counts = left_counters if isinstance(left_counters, dict) else {}
    right_counts = right_counters if isinstance(right_counters, dict) else {}
    mismatches = [
        key
        for key in left_counts.keys() | right_counts.keys()
        if not counters_match(left_counts.get(key), right_counts.get(key))
    ]
    counter_repeatable = (
        isinstance(left_counters, dict) and isinstance(right_counters, dict) and not mismatches
    )
    comparison["counter_repeatable"] = counter_repeatable
    if not counter_repeatable:
        comparison["counter_mismatch_count"] = len(mismatches)
        comparison["counter_mismatch_first"] = sorted(mismatches)[:8]
        notes.append(
            f"whole-run function counters differ or are missing ({len(mismatches)} differing roots)"
        )
    comparison["repeatable"] = counter_repeatable and (
        equal
        or (calls_empty_both and equal_before_calls and not missing_streams and not capped_inputs)
    )
    return {
        "title": args.title or "off versus selected roots",
        "left_label": label[0],
        "right_label": label[1],
        "equal": equal,
        "comparison": comparison,
        "tail": tail,
        "strict_poll_limit": limit,
        "counters": right.get("counters"),
        "noise_words": sum(len(v) for v in policy.noise.values())
        + sum(len(w) for r in policy.noise_samples.values() for w in r.values()),
        "policy": {"abs_eps": policy.abs_eps, "rel_eps": policy.rel_eps},
        "notes": notes,
    }


def write_reports(report: dict, base: Path) -> None:
    base.parent.mkdir(parents=True, exist_ok=True)
    base.with_suffix(".json").write_text(json.dumps(report, indent=2, default=str))
    base.with_suffix(".md").write_text(render_markdown(report))


def start_snapshot_capture(args: argparse.Namespace, raw: list[str]) -> int:
    """Take a reusable DMTCP checkpoint of a native route capture at port 0 poll N."""
    if args.snapshot_at_poll < 1:
        raise ValueError("--snapshot-at-poll needs N >= 1")
    if args.gpu_display:
        raise ValueError("DMTCP capture snapshots require lavapipe/Xvfb; omit --gpu-display")
    if not args.host:
        raise ValueError("snapshot capture needs a pinned host path")
    from tools.snapshot import session

    out = output_path(args.out_dir).resolve()
    snapshot_dir = (
        output_path(args.snapshot_dir).resolve()
        if args.snapshot_dir
        else out.with_name(out.name + "-snapshot")
    )
    disc = getattr(args, "disc", None)
    if disc and not Path(disc).is_absolute():
        # T2479: the checkpointed capture cannot run git to find the main checkout, so a
        # relative disc would resolve inside the worktree there. Pin it before DMTCP starts.
        raw = [*raw, "--disc", str(resolve(disc))]
    return session.take_capture(
        raw,
        poll=args.snapshot_at_poll,
        host=Path(args.host).resolve(),
        xbe=resolve(default_xbe()),
        record=Path(args.route).resolve(),
        run_dir=out,
        snapshot_dir=snapshot_dir,
        gzip=args.snapshot_gzip,
        keep_running=getattr(args, "snapshot_continue", False),
    )


def start_snapshot_capture_many(args: argparse.Namespace, raw: list[str]) -> int:
    """Take ordered reusable DMTCP checkpoints during one native route pass."""
    polls = args.snapshot_at_polls
    if (
        not polls
        or any(poll < 1 for poll in polls)
        or any(a >= b for a, b in zip(polls, polls[1:], strict=False))
    ):
        raise ValueError("--snapshot-at-polls needs unique increasing poll indices")
    if args.gpu_display:
        raise ValueError("DMTCP capture snapshots require lavapipe/Xvfb; omit --gpu-display")
    if not args.host:
        raise ValueError("snapshot capture needs a pinned host path")
    from tools.snapshot import session

    out = output_path(args.out_dir).resolve()
    snapshot_dir = (
        output_path(args.snapshot_dir).resolve()
        if args.snapshot_dir
        else out.with_name(out.name + "-snapshots")
    )
    disc = getattr(args, "disc", None)
    if disc and not Path(disc).is_absolute():
        raw = [*raw, "--disc", str(resolve(disc))]
    return session.take_capture_many(
        raw,
        polls=polls,
        host=Path(args.host).resolve(),
        xbe=resolve(default_xbe()),
        record=Path(args.route).resolve(),
        run_dir=out,
        snapshot_dir=snapshot_dir,
        gzip=args.snapshot_gzip,
    )


def cmd_capture(args: argparse.Namespace, raw: list[str] | None = None) -> int:
    # A DMTCP segment resume supplies a private output root.  The restored
    # argparse namespace still contains the original --out-dir, so apply this
    # at dispatch time before collecting the trace into the shared run tree.
    resumed_out = snapshot_resume_output_dir() or os.environ.get("TSFP_SNAPSHOT_OUT_DIR")
    if resumed_out:
        args.out_dir = resumed_out
    if getattr(args, "snapshot_at_polls", None) and not os.environ.get("TSFP_SNAPSHOT_INNER"):
        return start_snapshot_capture_many(args, raw or ["capture"])
    if getattr(args, "snapshot_at_poll", None) is not None and not os.environ.get(
        "TSFP_SNAPSHOT_INNER"
    ):
        return start_snapshot_capture(args, raw or ["capture"])
    trace = capture(args, args.rust_ports, output_path(args.out_dir))
    return 0 if trace["samples"] and not trace["meta"].get("output_cap_error") else 1


def cmd_resume_capture(args: argparse.Namespace) -> int:
    from tools.snapshot import session

    return session.resume_capture(
        Path(args.snapshot_dir).resolve(),
        host_override=Path(args.host).resolve() if args.host else None,
        timeout=args.resume_timeout,
        fast_verify=args.snapshot_fast_verify,
        seconds=getattr(args, "seconds", None),
        isolate_run_dir=getattr(args, "isolate_run_dir", False),
    )


def trim_from_poll(trace: dict, first_poll: int) -> dict:
    """T2479: keep samples from `first_poll` on (a snapshot resume vs its continued run)."""
    trace["samples"] = [s for s in trace["samples"] if s["poll"] >= first_poll]
    return trace


def compare_segment_windows(
    args: argparse.Namespace, left: dict, right: dict, controls: list[dict] | None
) -> dict:
    """Require every stitched poll segment and its resumed boundary sample to compare equal."""
    # Capture samples are periodic rather than emitted on every guest poll. Allow the
    # measured maximum 13-poll spacing, with a small margin, when checking segment ends.
    max_segment_tail_polls = 16
    left_segments = left.get("meta", {}).get("segments")
    right_segments = right.get("meta", {}).get("segments")
    if not isinstance(left_segments, list) or len(left_segments) != args.segments:
        raise ValueError("left trace does not contain the requested stitched segment plan")
    if not isinstance(right_segments, list) or len(right_segments) != args.segments:
        raise ValueError("right trace does not contain the requested stitched segment plan")
    if left_segments != right_segments:
        raise ValueError("left and right traces use different segment boundaries")

    regions = load_regions(json.loads(resolve(args.regions).read_text()))
    left_offset, right_offset = route_poll_offset(left), route_poll_offset(right)
    left_samples = route_relative_samples(left, left_offset)
    right_samples = route_relative_samples(right, right_offset)
    control_samples = [route_relative_samples(item) for item in (controls or [])]
    left_by_poll = {int(sample["poll"]): sample for sample in left_samples}
    right_by_poll = {int(sample["poll"]): sample for sample in right_samples}
    comparisons = []
    boundaries = []
    for index, segment in enumerate(left_segments):
        first = int(segment["from_poll"])
        last = int(segment["through_poll"])
        lo_left, hi_left = first - left_offset, last - left_offset
        lo_right, hi_right = first - right_offset, last - right_offset
        left_window = [sample for sample in left_samples if lo_left <= sample["poll"] <= hi_left]
        right_window = [
            sample for sample in right_samples if lo_right <= sample["poll"] <= hi_right
        ]
        left_tail = hi_left - max((int(sample["poll"]) for sample in left_window), default=0)
        right_tail = hi_right - max((int(sample["poll"]) for sample in right_window), default=0)
        if left_tail > max_segment_tail_polls or right_tail > max_segment_tail_polls:
            comparisons.append(
                {
                    "index": index,
                    "from_poll": first,
                    "through_poll": last,
                    "equal": False,
                    "reason": "segment trace does not reach its requested end",
                    "left_tail_polls": left_tail,
                    "right_tail_polls": right_tail,
                    "allowed_tail_polls": max_segment_tail_polls,
                }
            )
            boundaries.append(
                {
                    "index": index,
                    "poll": first,
                    "equal": False,
                    "reason": "segment trace does not cover its complete window",
                }
            )
            continue
        common = sorted(
            set(left_by_poll)
            .intersection(right_by_poll)
            .intersection(range(max(lo_left, lo_right), min(hi_left, hi_right) + 1))
        )
        if not common:
            comparisons.append(
                {
                    "index": index,
                    "from_poll": first,
                    "through_poll": last,
                    "equal": False,
                    "reason": "no common poll samples",
                }
            )
            boundaries.append(
                {
                    "index": index,
                    "poll": first,
                    "equal": False,
                    "reason": "no common boundary sample",
                }
            )
            continue
        control_windows = [
            [
                sample
                for sample in samples
                if first <= sample["poll"] + route_poll_offset(control) <= last
            ]
            for samples, control in zip(control_samples, controls or [], strict=True)
        ]
        policy = Policy(abs_eps=args.abs_eps, rel_eps=args.rel_eps)
        if control_windows:
            policy.noise, policy.noise_samples = noise_masks_multi(
                regions, [left_window, *control_windows]
            )
        comparison = compare_traces(regions, left_window, right_window, policy)
        comparisons.append(
            {
                "index": index,
                "from_poll": first,
                "through_poll": last,
                "samples_compared": comparison["samples_compared"],
                "diverging_samples": comparison["diverging_samples"],
                "equal": comparison["equal"],
                "first_divergence": comparison.get("first_divergence"),
            }
        )

        # A segmented adoption verdict requires an observation on the checkpoint poll
        # itself; a later sample is not evidence for the saved boundary state.
        left_boundary_poll = first - left_offset
        right_boundary_poll = first - right_offset
        left_boundary = left_by_poll.get(left_boundary_poll)
        right_boundary = right_by_poll.get(right_boundary_poll)
        if left_boundary is None or right_boundary is None:
            boundaries.append(
                {
                    "index": index,
                    "poll": first,
                    "equal": False,
                    "reason": "trace has no sample at the exact DMTCP snapshot poll",
                }
            )
        else:
            boundary_result = compare_traces(
                regions,
                [left_boundary],
                [right_boundary],
                Policy(abs_eps=args.abs_eps, rel_eps=args.rel_eps),
            )
            boundaries.append(
                {
                    "index": index,
                    "snapshot_poll": first,
                    "sample_poll": first,
                    "equal": boundary_result["equal"],
                    "first_divergence": boundary_result.get("first_divergence"),
                }
            )
    return {
        "count": args.segments,
        "coverage_complete": all(
            "reason" not in item
            or item["reason"] != "segment trace does not reach its requested end"
            for item in comparisons
        ),
        "equal": all(item["equal"] for item in comparisons)
        and all(item["equal"] for item in boundaries),
        "segments": comparisons,
        "boundaries": boundaries,
    }


def cmd_diff(args: argparse.Namespace) -> int:
    left_dir, right_dir = output_path(args.left), output_path(args.right)
    left, right = load_trace(left_dir), load_trace(right_dir)
    control = [load_trace(output_path(c)) for c in args.control] if args.control else None
    segmented = (
        compare_segment_windows(args, left, right, control)
        if getattr(args, "segments", None) is not None
        else None
    )
    first_poll = getattr(args, "from_poll", 0) or 0
    if first_poll:
        left, right = trim_from_poll(left, first_poll), trim_from_poll(right, first_poll)
        control = [trim_from_poll(c, first_poll) for c in control] if control else None
    report = build_report(args, left, right, control, (args.left, args.right))
    if segmented is not None:
        report["segmented_comparison"] = segmented
        report["equal"] = report["equal"] and segmented["equal"]
        report["comparison"]["equal"] = report["equal"]
        report["notes"].append(
            f"segmented comparison: {segmented['count']} segments; "
            f"{sum(item['equal'] for item in segmented['segments'])}/{segmented['count']} segment windows and "
            f"{sum(item['equal'] for item in segmented['boundaries'])}/{segmented['count']} observed boundary samples equal"
        )
    left_calls = left_dir / left.get("meta", {}).get("call_args_file", "call-args.trace")
    right_calls = right_dir / right.get("meta", {}).get("call_args_file", "call-args.trace")
    if left_calls.is_file() and right_calls.is_file():
        from tools.rust_trace_call_args import (
            TailTally,
            common_last_poll,
            compare_files,
            count_tail,
        )

        call_limit = args.strict_poll or strict_limit(args, left)
        tail_horizon = common_last_poll(left, right)
        tail = TailTally(tail_horizon)
        if tail_horizon is not None:
            call_limit = min(call_limit, tail_horizon)
            count_tail(left_calls, tail_horizon, "left", tail)
            count_tail(right_calls, tail_horizon, "right", tail)
        streamed = compare_files(
            left_calls,
            right_calls,
            from_poll=getattr(args, "from_poll", 0) or 0,
            strict_poll=call_limit,
        )
        if tail_horizon is not None:
            streamed.update(tail.summary())
            report["notes"].extend(streamed["call_args_tail_health"])
            if streamed["call_args_tail_text"]:
                report["notes"].append(streamed["call_args_tail_text"])
        report["comparison"]["call_args_external"] = streamed
        report["equal"] = report["equal"] and streamed["equal"]
        streamed_empty = not (
            streamed["rows_compared"] or streamed["missing_rows"] or streamed["diverging_rows"]
        )
        report["comparison"]["repeatable"] = report["comparison"].get(
            "repeatable", report["equal"]
        ) and (streamed["equal"] or streamed_empty)
        for line in streamed.get("stack_residue_health", []):
            report["notes"].append(line)
        if streamed.get("stack_residue_rows"):
            report["notes"].append(
                f"EQUAL with {streamed['stack_residue_rows']} stack-residue rows excluded "
                "(dead-stack pointer-argument digests only, T3702)"
                if streamed["equal"]
                else f"{streamed['stack_residue_rows']} stack-residue rows excluded (T3702)"
            )
        report["notes"].append(
            "external call-argument stream: "
            f"{streamed['rows_compared']} rows, {streamed['diverging_rows']} diverging, "
            f"{streamed['missing_rows']} missing, first {streamed['first_divergence']}"
        )
    write_reports(report, output_path(args.report))
    print(render_markdown(report))
    return 0 if report["equal"] else 2


def cmd_run(args: argparse.Namespace) -> int:
    if bool(getattr(args, "call_args_baseline", None)) != bool(
        getattr(args, "call_args_control", None)
    ):
        raise ValueError("--call-args-baseline and --call-args-control must be supplied together")
    base = output_path(args.out_dir)
    # a retained baseline (and control) is reused when the host differs only in Rust code (`off` never enters Rust)
    off = (
        load_trace(output_path(args.baseline))
        if args.baseline
        else capture(args, "off", base / "off")
    )
    if args.baseline_control:
        control = load_trace(output_path(args.baseline_control))
    elif args.control:
        runs = max(1, args.control_runs)
        control = [
            capture(args, "off", base / ("off2" if i == 0 else f"off{i + 2}")) for i in range(runs)
        ]
    else:
        control = None
    right_spec = args.rust_ports
    right = capture(args, right_spec, base / "right")
    report = build_report(args, off, right, control, ("off", right_spec))
    if not report["equal"] and args.bisect:
        # Counters reflect the host's resolved selection (groups, exclusions and VAs).
        # Bisect only roots that actually ran Rust, never the selection tokens.
        candidates = reached_roots(right["counters"])
        probes = {"count": 0}
        regions = load_regions(json.loads(resolve(args.regions).read_text()))
        policy = Policy(abs_eps=args.abs_eps, rel_eps=args.rel_eps)
        if control is not None:
            ctl = control if isinstance(control, list) else [control]
            policy.noise, policy.noise_samples = noise_masks_multi(
                regions, [off["samples"]] + [c["samples"] for c in ctl]
            )
        cache: dict[tuple[str, ...], bool] = {}

        def fails(subset: list[str]) -> bool:
            key = tuple(sorted(subset))
            if key not in cache:
                probes["count"] += 1
                probe_args = argparse.Namespace(**{**vars(args), "seconds": args.probe_seconds})
                trace = capture(
                    probe_args,
                    ",".join("0x" + va for va in key),
                    base / f"probe{probes['count']:02d}",
                )
                limit = strict_limit(args, off)
                cache[key] = not compare_traces(
                    regions,
                    split_window(off["samples"], limit)[0],
                    split_window(trace["samples"], limit)[0],
                    policy,
                )["equal"]
            return cache[key]

        if candidates and fails(candidates):
            roots = ddmin(candidates, fails)
        else:
            roots = []
        report["attribution"] = {
            "roots": roots,
            "probes": probes["count"],
            "candidates": len(candidates),
        }
        if not roots:
            report["notes"].append(
                "bisect: the reached-root set did not reproduce the divergence (nondeterminism or an unreached root)"
            )
    write_reports(report, base / "report")
    print(render_markdown(report))
    return 0 if report["equal"] else 2


def add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--max-frames", type=int, default=2000, help="maximum overlay frames (0 disables)"
    )
    parser.add_argument(
        "--max-files", type=int, default=20000, help="maximum files in capture output (0 disables)"
    )
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=4_000_000_000,
        help="maximum capture output bytes (0 disables)",
    )
    parser.add_argument(
        "--trace-only",
        action="store_true",
        help="CPU-only guest-state capture; requires a recording with the CPU-only flag identity, no frame/GPU conformance claim",
    )
    parser.add_argument(
        "--fast-forward",
        action="store_true",
        help="accepted for compatibility: native .rec captures fast-forward by default (T2487, see --paced)",
    )
    parser.add_argument(
        "--stop-at-route-end",
        action="store_true",
        help="gracefully close after the native input recording emits its route-end marker "
        "(default for native .rec routes since T3388)",
    )
    parser.add_argument(
        "--render",
        choices=("auto", "gpu", "none"),
        default="auto",
        help="T3388: none = no window, renderer, GPU or audio output (state-only replay); gpu = "
        "window and renderer for visual checks; auto = none for a fast-forward native .rec replay "
        "without screenshots, else gpu",
    )
    parser.add_argument(
        "--replay-clock",
        choices=("tick", "poll"),
        default="tick",
        help="T3393: key recorded rows on the simulation tick (default) or on the live host poll "
        "count (the T2032 schedule, wall-clock dependent)",
    )
    parser.add_argument(
        "--no-recording-timing",
        action="store_true",
        help="T3388: do not add the recording-time FABRICATED vblank models "
        "(--vblank-poll-blank --vblank-worker-blanks 100) to a native .rec replay",
    )
    parser.add_argument(
        "--no-stop-at-route-end",
        action="store_true",
        help="T3388: keep a native .rec capture running past its route end until --seconds",
    )
    parser.add_argument(
        "--pad-only-ui",
        action="store_true",
        help="diagnostic: suppress UI pointer input during an authored pad replay (game input unchanged)",
    )
    parser.add_argument(
        "--gpu-display",
        action="store_true",
        help="use private Weston/Xwayland DRI3 display for real-GPU Bevy captures",
    )
    parser.add_argument(
        "--save-dir",
        help="isolated save seed/root for replay (replaces the single --hdd value before resource preparation)",
    )
    parser.add_argument(
        "--hud-state",
        choices=("reference", "owned"),
        help="capture exact HUD weapon state and actual row advances through the native/reference contract",
    )
    parser.add_argument(
        "--rea-ai-state",
        choices=("reference", "owned"),
        help="capture reference guest or Rust-owned REA actor state for T2407 Tier 2 comparison",
    )
    parser.add_argument(
        "--t2-capture",
        action="store_true",
        help="T3740: record the rule-43 capture spool <out-dir>/capture.bin (host must be a "
        "TSFP_BUILD_CAPTURE=1 build, docs/t3738-capture-design.md)",
    )
    parser.add_argument(
        "--t2-budget-seed",
        help="T3803: per-VA capture budget seed (tools.t2capture.budget_seed) for --t2-capture",
    )
    parser.add_argument(
        "--t2-capture-targets",
        help="T3833: ALLOW list (hex VA lines, tools.t2capture.live_targets) for --t2-capture",
    )
    parser.add_argument(
        "--call-args",
        help="T2346: comma-separated root VAs whose arguments, pointer/input-state digests and lane exit EAX/ST0 are traced and compared",
    )
    parser.add_argument(
        "--host",
        default=os.environ.get("TSFP_HOST"),
        help="trace host (default shared prebuilt for this worktree's primary merge-base, then legacy host; TSFP_HOST is equivalent)",
    )
    parser.add_argument(
        "--host-input-hash",
        help="pin the shared prebuilt host by its C/Rust/CMake build-input hash",
    )
    parser.add_argument("--host-sha256", help="verify the exact host binary selected by replay")
    parser.add_argument("--disc", default=DEFAULT_DISC)
    parser.add_argument(
        "--route",
        default=DEFAULT_ROUTE,
        help="recorded route (a T2348 .rec uses the Bevy shell and typed Rust replay; default: main checkout tmp/recorded-input-story-mode)",
    )
    parser.add_argument(
        "--modules", help="gpu replay modules (default: main checkout tmp/play/modules)"
    )
    parser.add_argument("--regions", default=DEFAULT_REGIONS)
    parser.add_argument(
        "--seconds",
        type=float,
        default=None,
        help="wall-seconds cap before the graceful close (default: native .rec routes run to their "
        "recorded route end with a 3600 s safety cap, legacy routes 75 s)",
    )
    parser.add_argument(
        "--shot-at",
        type=float,
        action="append",
        default=[],
        help="T2116: wall seconds after start at which to save out/shot-<s>.png of the X root (repeatable)",
    )
    parser.add_argument(
        "--shot-at-route-end",
        action="store_true",
        help="save shot-route-end.png when the recorded input route emits its deterministic route-end marker",
    )
    parser.add_argument(
        "--idle-every", type=int, default=8, help="polls between idle control dumps"
    )
    parser.add_argument("--max-idle", type=int, default=400)
    parser.add_argument("--max-dumps", type=int, default=1500)
    parser.add_argument(
        "--host-arg", action="append", default=[], help="extra host argument (repeatable)"
    )
    parser.add_argument("--abs-eps", type=float, default=1e-4)
    parser.add_argument("--rel-eps", type=float, default=1e-4)
    parser.add_argument(
        "--min-samples",
        type=int,
        default=30,
        help="fewer common samples than this is a failure, not a pass",
    )
    parser.add_argument("--title", default="")
    parser.add_argument(
        "--strict-poll",
        type=int,
        default=0,
        help="last poll of the deterministic window (default: route end poll + margin)",
    )
    parser.add_argument("--window-margin", type=int, default=0)
    parser.add_argument(
        "--from-poll",
        type=int,
        default=0,
        help="T2479: compare only samples at or after this poll (a snapshot's poll, so a resumed "
        "capture is checked against <snapshot-dir>/continued from the restore point on)",
    )
    parser.add_argument(
        "--strict-tail",
        action="store_true",
        help="also fail on divergences after the hand-over (wall-clock driven)",
    )


def main() -> int:
    from tools.wt import core

    core.apply_rust_env(Path(__file__).resolve().parents[1])
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="mode", required=True)
    cap = sub.add_parser("capture", help="one host run and its trace")
    add_common(cap)
    cap.add_argument("--rust-ports", default="off")
    cap.add_argument(
        "--paced",
        action="store_true",
        help="wall-clock paced capture; native .rec captures fast-forward by default (TSFP_FAST_FORWARD=1, --present-no-pace)",
    )
    cap.add_argument(
        "--anim-shadow",
        choices=("on", "off"),
        default=None,
        help="TSFP_ANIM_SHADOW for the host (T2043: off = plain ports); default off for native .rec routes (T3592), on otherwise",
    )
    cap.add_argument(
        "--play-options",
        default="default",
        help="play.sh variant whose host options the capture also runs (T3592; read from `play.sh --emit-options`), 'off' = the capture's own options. --rust-ports play takes that variant's port selection",
    )
    cap.add_argument("--out-dir", required=True)
    cap.add_argument(
        "--snapshot-at-poll",
        type=int,
        help="checkpoint the complete route capture process at this guest port-0 poll (DMTCP, lavapipe/Xvfb)",
    )
    cap.add_argument(
        "--snapshot-at-polls",
        type=lambda value: [int(item) for item in value.split(",")],
        help="take ordered DMTCP checkpoints at several port-0 polls during one route pass",
    )
    cap.add_argument("--snapshot-dir", help="directory for the reusable process checkpoint")
    cap.add_argument(
        "--snapshot-gzip", action="store_true", help="compress DMTCP checkpoint images"
    )
    cap.add_argument(
        "--snapshot-continue",
        action="store_true",
        help="T2479: keep capturing after the checkpoint and keep that trace in "
        "<snapshot-dir>/continued (same-run oracle for a resumed capture)",
    )
    cap.add_argument(
        "--show-fps",
        action="store_true",
        help="show host game/present FPS in the same captured run",
    )
    resume = sub.add_parser("resume", help="resume a saved DMTCP rust_trace capture")
    resume.add_argument("--snapshot-dir", required=True)
    resume.add_argument(
        "--host", help="optional host override; it must match the pinned snapshot host"
    )
    resume.add_argument("--resume-timeout", type=int, default=600)
    resume.add_argument("--snapshot-fast-verify", action="store_true")
    resume.add_argument(
        "--isolate-run-dir",
        action="store_true",
        help="resume with a private mount namespace and a reflinked run directory (parallel segments)",
    )
    resume.add_argument(
        "--seconds",
        type=float,
        help="T2479: wall budget of the resumed window (default: the capture's own --seconds)",
    )
    diff = sub.add_parser("diff", help="diff two captured traces")
    add_common(diff)
    diff.add_argument("--left", required=True)
    diff.add_argument("--right", required=True)
    diff.add_argument(
        "--align-region",
        metavar="NAME=U32",
        help="align captures at their first sample with this region value and trim earlier startup samples",
    )
    diff.add_argument(
        "--control",
        action="append",
        help="a further capture of the LEFT configuration: words differing between the runs are masked; "
        "repeat for a 3-run control (T2067)",
    )
    diff.add_argument(
        "--report", required=True, help="report base path (.json and .md are written)"
    )
    diff.add_argument(
        "--segments",
        type=int,
        help="require each stitched poll segment and its resumed boundary sample to compare equal",
    )
    run = sub.add_parser(
        "run", help="off, control, selected roots, diff and optional bisect, sequentially"
    )
    add_common(run)
    run.add_argument(
        "--rust-ports",
        default="all",
        help="right side: all, group tokens or a hex list (host selection grammar)",
    )
    run.add_argument(
        "--anim-shadow",
        choices=("on", "off"),
        default="on",
        help="TSFP_ANIM_SHADOW for the host (T2043: off = plain ports, as played)",
    )
    run.add_argument("--out-dir", required=True)
    run.add_argument(
        "--control", action="store_true", help="also run off twice to mask nondeterministic words"
    )
    run.add_argument(
        "--baseline", help="reuse this captured `off` trace dir instead of running off"
    )
    run.add_argument(
        "--control-runs",
        type=int,
        default=1,
        help="with --control: number of extra off captures (2 = 3-run control that learns run-to-run words)",
    )
    run.add_argument(
        "--baseline-control", help="reuse this captured second `off` trace dir as the control"
    )
    run.add_argument(
        "--call-args-baseline",
        help="selected, unmutated trace directory for call-argument mutation comparisons",
    )
    run.add_argument(
        "--call-args-control",
        help="repeat selected, unmutated trace directory for call-argument noise masking",
    )
    run.add_argument(
        "--probe-seconds",
        type=float,
        default=30.0,
        help="wall seconds of each bisect probe run (the route ends by about 12 s)",
    )
    run.add_argument(
        "--bisect",
        action="store_true",
        help="delta debug the reached roots when the traces diverge",
    )
    raw = sys.argv[1:]
    args = parser.parse_args(raw)
    if args.mode == "diff" and args.segments is not None and args.segments < 1:
        parser.error("--segments needs K >= 1")
    if args.mode in {"capture", "run"}:
        try:
            args.route = str(resolve_route(args.route))
            native_end_poll = native_input_route_end_poll(Path(args.route))
            if native_end_poll is not None:
                if args.trace_only:
                    parser.error(
                        "--trace-only is not supported for native .rec routes; run the Bevy host windowed"
                    )
                args.host = str(
                    trace_host_for_route(args.host, Path(args.route), native_route=True)
                )
        except ValueError as error:
            parser.error(str(error))
    if args.mode == "capture":
        return cmd_capture(args, raw)
    return {"resume": cmd_resume_capture, "diff": cmd_diff, "run": cmd_run}[args.mode](args)


if __name__ == "__main__":
    sys.exit(main())
