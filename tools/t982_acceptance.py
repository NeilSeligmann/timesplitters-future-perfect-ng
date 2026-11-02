#!/usr/bin/env python3
# ruff: noqa: E501
"""T982/T1208 acceptance run: one controllable Story run on the private host with a NORMAL window close.

Starts Xvfb, the private host (`tmp/private-host/current.json`) with the scripted pad, SDL audio on the
`disk` driver (a real unmuted device path that records the PCM the device would play), screenshots every N
seconds, and after --seconds sends the window manager close request (WM_DELETE_WINDOW, the same event the
window's close button produces) instead of killing the host.  It then records the exit status, the stop
line, the audio statistics (RMS and peak per window, a silence control) and the frame counters.

    python -m tools.t982_acceptance --seconds 120 --out-dir tmp/t982/run1

Everything is relative to the repository root (a worktree finds the owner files and disc in the main checkout).
Evidence level: MEASURED for the host run, the pad script is FABRICATED input.
"""

from __future__ import annotations

import argparse
import array
import fcntl
import json
import math
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path

from tools.datadir import output_path
from tools.guest_mem_probe import read_snapshot, save_snapshot  # read-only /proc/PID/mem
from tools.local_game_slots import holding

ROOT = Path(__file__).resolve().parent.parent
SAMPLE_RATE = 48000
CHANNELS = 2


def main_root() -> Path:
    if os.environ.get("TSFP_SOURCE_TREE"):
        return ROOT  # T3760: a remote exec dir (identity as data, no git repo on the host)
    common = subprocess.run(
        ["git", "rev-parse", "--git-common-dir"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        # T3416: under DMTCP close_fds makes every child exit 99 (worktree XBE lookup failed).
        close_fds=not os.environ.get("DMTCP_VIRTUAL_PID"),
    )
    path = (ROOT / common.stdout.strip()).resolve()
    return path.parent if path.name == ".git" else ROOT


DISPLAY_RANGE = range(130, 200)


def pid_alive(pid: int) -> bool:
    """True when `pid` exists and is not a zombie (a dead-but-unreaped Xvfb still holds no display)."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    except (OSError, IndexError):
        return True
    return fields[0] not in ("Z", "X")


def display_owner_alive(number: int, lock_dir: Path, *, reservation_held: bool = False) -> bool:
    """Namespace-safe X owner check; an active shared display reservation is authoritative."""
    if not reservation_held:
        from tools.wt import display, lease

        reservation = display.lock_dir() / f"{number}.lock"
        try:
            fd = os.open(reservation, os.O_RDWR | os.O_CREAT, 0o600)
        except OSError:
            return True
        try:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return True
            record = lease.read_record(lease.record_path(reservation))
            if record and lease.holder_alive(record)[0]:
                return True
        finally:
            os.close(fd)
    try:
        text = (lock_dir / f".X{number}-lock").read_text().strip()
        return pid_alive(int(text))
    except FileNotFoundError:
        return False
    except (OSError, ValueError):
        return True


def sweep_stale_displays(lock_dir: Path = Path("/tmp"), sock_dir: Path | None = None) -> list[int]:
    """T2141: remove the lock and socket of every display in DISPLAY_RANGE whose Xvfb died (SIGKILL leaks them).
    Returns the swept numbers. A lock whose PID is alive is never touched."""
    sock_dir = sock_dir if sock_dir is not None else lock_dir / ".X11-unix"
    swept = []
    for number in DISPLAY_RANGE:
        lock, sock = lock_dir / f".X{number}-lock", sock_dir / f"X{number}"
        if lock.exists():
            if display_owner_alive(number, lock_dir):
                continue
        elif os.path.lexists(sock):
            continue  # a socket without a lock belongs to some other server: leave it
        for path in (lock, sock):
            try:
                path.unlink()
            except FileNotFoundError:
                pass
        swept.append(number)
    return swept


def pick_display(lock_dir: Path = Path("/tmp"), sock_dir: Path | None = None) -> int:
    # T2223: reserve by flock before Xvfb starts, across all clones.
    from tools.hostlock import reserve_display

    return reserve_display(lock_dir, sock_dir)


def stop_xvfb(
    server: subprocess.Popen,
    number: int,
    lock_dir: Path = Path("/tmp"),
    sock_dir: Path | None = None,
) -> None:
    """T2141: terminate the Xvfb this process started (by PID, SIGTERM then SIGKILL), reap it, and remove its lock and socket."""
    sock_dir = sock_dir if sock_dir is not None else lock_dir / ".X11-unix"
    if server.poll() is None:
        for sig, wait in ((signal.SIGTERM, 5), (signal.SIGKILL, 10)):
            try:
                os.kill(server.pid, sig)
            except ProcessLookupError:
                break
            try:
                server.wait(timeout=wait)
                break
            except subprocess.TimeoutExpired:
                continue
    server.wait()
    from tools.wt.display import release_number

    for path in (lock_dir / f".X{number}-lock", sock_dir / f"X{number}"):
        try:
            path.unlink()
        except FileNotFoundError:
            pass

    release_number(number)


def analyse_pcm(raw: bytes, window_seconds: float = 1.0) -> dict:
    """RMS and peak per window of interleaved S16LE stereo, plus overall silence statistics."""
    samples = array.array("h")
    usable = len(raw) - (len(raw) % 2)
    samples.frombytes(raw[:usable])
    if sys.byteorder == "big":
        samples.byteswap()
    per_window = SAMPLE_RATE * CHANNELS * window_seconds
    step = int(per_window)
    windows = []
    for start in range(0, len(samples), step):
        chunk = samples[start : start + step]
        if not chunk:
            break
        square = sum(value * value for value in chunk)
        windows.append(
            {
                "t": round(start / (SAMPLE_RATE * CHANNELS), 3),
                "rms": round(math.sqrt(square / len(chunk)), 2),
                "peak": max(max(chunk), -min(chunk)),
            }
        )
    audible = [entry for entry in windows if entry["rms"] >= 8.0]
    return {
        "samples": len(samples),
        "seconds": round(len(samples) / (SAMPLE_RATE * CHANNELS), 2),
        "windows": windows,
        "audible_windows": len(audible),
        "silent_windows": len(windows) - len(audible),
        "max_peak": max((entry["peak"] for entry in windows), default=0),
        "max_rms": max((entry["rms"] for entry in windows), default=0.0),
    }


def silence_control() -> dict:
    """The analyser must report digital silence as silent (control for analyse_pcm)."""
    return analyse_pcm(bytes(SAMPLE_RATE * CHANNELS * 2 * 3))


def parse_log(text: str) -> dict:
    found: dict = {}
    patterns = {
        "audio_sink": r"^audio sink\s+sdl: (.*)$",
        "stop": r"^(STOP .*)$",
        "present_frames": r"^present sink\s+(.*frames.*)$",
        "gpu_live": r"^gpu live\s+(.*)$",
    }
    for key, pattern in patterns.items():
        hits = re.findall(pattern, text, re.M)
        if hits:
            found[key] = hits[-1][:400]
    match = re.search(r"underrun (\d+) \(events (\d+)\)", text)
    if match:
        found["underrun_frames"] = int(match.group(1))
        found["underrun_events"] = int(match.group(2))
    found["close_logged"] = "interactive window closed by user" in text
    return found


def xlib_python() -> str:
    """T2304: an interpreter that can import python-xlib.  `python3 -m tools.rust_trace` runs under the
    system interpreter, which has no Xlib, so the close child died with ModuleNotFoundError and the host
    was SIGKILLed with no stop report.  Prefer this interpreter, else the worktree's or main checkout's venv."""
    candidates = [
        sys.executable,
        str(ROOT / ".venv/bin/python"),
        str(main_root() / ".venv/bin/python"),
    ]
    for candidate in candidates:
        if not Path(candidate).exists():
            continue
        probe = subprocess.run(
            [candidate, "-c", "import Xlib"], capture_output=True, timeout=30, check=False
        )
        if probe.returncode == 0:
            return candidate
    raise RuntimeError("no interpreter with python-xlib (uv pip install python-xlib into .venv)")


def send_close(display: str, env: dict[str, str] | None = None) -> str:
    """Send WM_DELETE_WINDOW to the host's top level window (what a window manager close button does)."""
    if env is not None:
        # python-xlib reads XAUTHORITY from the environment; isolate it in a child
        # rather than racing another launch by changing the parent environment.
        result = subprocess.run(
            [
                xlib_python(),
                "-c",
                "from tools.t982_acceptance import send_close; import sys; print(send_close(sys.argv[1]))",
                display,
            ],
            env=env,
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=10,
            check=True,
        )
        return result.stdout.strip()
    from Xlib import X, protocol  # python-xlib, project venv
    from Xlib import display as xdisplay

    connection = xdisplay.Display(display)
    root = connection.screen().root
    wm_protocols = connection.intern_atom("WM_PROTOCOLS")
    wm_delete = connection.intern_atom("WM_DELETE_WINDOW")
    for window in root.query_tree().children:
        try:
            protocols = window.get_wm_protocols()
        except Exception:  # noqa: BLE001 - a window may vanish while scanning
            continue
        if wm_delete in protocols:
            event = protocol.event.ClientMessage(
                window=window,
                client_type=wm_protocols,
                data=(32, [wm_delete, X.CurrentTime, 0, 0, 0]),
            )
            window.send_event(event, event_mask=X.NoEventMask)
            connection.flush()
            return f"WM_DELETE_WINDOW sent to window 0x{window.id:x}"
    return "no window with WM_DELETE_WINDOW found"


def host_command(
    host: Path, xbe: Path, out: Path, disc: Path, modules: Path, pad_args: list[str]
) -> list[str]:
    """The T1209 controllable argument set with the given pad arguments (--pad-source/--pad-script or --pad-script-live)."""
    return [
        str(host),
        str(xbe),
        "--hdd",
        str(out / "hdd"),
        "--xonline-offline",
        "--skip-intro",
        "--ac97-ready",
        "--headless-streams",
        "--headless-buffers",
        "--headless-listener",
        "--headless-second-vblank",
        "--native-shader-assembler",
        "--native-xmv",
        "--headless-movie-audio",
        "--couple-vblank-effects",
        "--check-vblank-quiescence",
        "--overlay-consume",
        "--gpu-live",
        "--gpu-live-inferred",
        "--dump-overlay",
        str(out / "overlay"),
        "--interactive",
        "--thread-timeout",
        "2147483647",
        "--vblank-owner-waits",
        "1000",
        "--vblank-worker-blanks",
        "100",
        "--present",
        "window",
        "--present-hold-ms",
        "0",
        "--present-capture",
        str(out / "final.bmp"),
        "--audio-sink",
        "sdl",
        "--synthetic-pad",
        *pad_args,
        "--gpu-replay",
        str(modules),
        "--gpu-replay-lenient",
        "--gpu-live-translate",
        "--disc",
        str(disc),
    ]


@holding("t982_acceptance")  # T3823: direct game-host spawner
def run(args: argparse.Namespace) -> dict:
    main = main_root()
    out = output_path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    for sub in ("hdd", "overlay"):
        (out / sub).mkdir(exist_ok=True)
    current = json.loads((ROOT / "tmp/private-host/current.json").read_text())
    host = ROOT / current["host"]
    disc = (
        Path(args.disc)
        if args.disc
        else main / "tmp/TimeSplitters - Future Perfect (USA) (XBOX).iso"
    )
    pad = (
        Path(args.pad_script)
        if args.pad_script
        else main / "tmp/t1204-evidence/story-controllable.pad"
    )
    modules = main / "tmp/play/modules"
    xbe = ROOT / "build/default.xbe"
    display_number = pick_display()
    display = f":{display_number}"
    xvfb = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1280x720x24"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2)
    raw_path = out / "audio.raw"
    env = dict(
        os.environ,
        DISPLAY=display,
        SDL_AUDIODRIVER=args.audio_driver,
        SDL_AUDIO_DISK_OUTPUT_FILE=str(raw_path),
        SDL_AUDIO_DISK_TIMESCALE="1.0",
    )
    command = host_command(
        host, xbe, out, disc, modules, ["--pad-source", "script", "--pad-script", str(pad)]
    )
    if args.mute:
        command.insert(command.index("--audio-sink") + 2, "--audio-mute")
    log = (out / "run.log").open("wb")
    started = time.monotonic()
    proc = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    shots = 0
    next_shot = args.shot_every
    next_mem = args.mem_start if args.mem_every else None
    mem_count = 0
    close_note = "host exited before the close request"
    close_sent_at = None
    try:
        while proc.poll() is None:
            elapsed = time.monotonic() - started
            if elapsed >= next_shot:
                shots += 1
                subprocess.run(
                    ["import", "-window", "root", str(out / f"shot-{int(next_shot):04d}.png")],
                    env=env,
                    timeout=30,
                    stderr=subprocess.DEVNULL,
                    check=False,
                )
                next_shot += args.shot_every
            if next_mem is not None and elapsed >= next_mem and close_sent_at is None:
                save_snapshot(
                    read_snapshot(proc.pid),
                    Path(args.mem_dir or out / "mem") / f"m{int(next_mem):04d}.bin",
                )
                mem_count += 1
                next_mem += args.mem_every
            if elapsed >= args.seconds and close_sent_at is None:
                close_note = send_close(display)
                close_sent_at = time.monotonic()
            if close_sent_at is not None and time.monotonic() - close_sent_at > args.close_grace:
                close_note += f", NO exit within {args.close_grace} s, killed"
                proc.kill()
                break
            time.sleep(0.5)
        status = proc.wait(timeout=60)
    finally:
        if proc.poll() is None:
            proc.kill()
        xvfb.send_signal(signal.SIGTERM)
        log.close()
    wall = time.monotonic() - started
    text = (out / "run.log").read_text(errors="replace")
    result = {
        "host_sha256": current["sha256"],
        "wall_seconds": round(wall, 1),
        "exit_status": status,
        "close_request": close_note,
        "close_to_exit_seconds": None
        if close_sent_at is None
        else round(time.monotonic() - close_sent_at, 1),
        "screenshots": shots,
        "memory_snapshots": mem_count,
        "log": parse_log(text),
        "muted": args.mute,
    }
    if raw_path.exists():
        analysis = analyse_pcm(raw_path.read_bytes())
        result["audio"] = {key: value for key, value in analysis.items() if key != "windows"}
        result["audio_windows"] = analysis["windows"]
    result["silence_control"] = {
        key: value for key, value in silence_control().items() if key != "windows"
    }
    (out / "result.json").write_text(json.dumps(result, indent=1))
    return result


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--seconds", type=float, default=120.0, help="seconds before the window close request"
    )
    parser.add_argument("--out-dir", default="tmp/t982/run", help="output directory (gitignored)")
    parser.add_argument(
        "--shot-every", type=float, default=20.0, help="screenshot period in seconds"
    )
    parser.add_argument(
        "--close-grace",
        type=float,
        default=60.0,
        help="seconds to wait for the host to exit after the close",
    )
    parser.add_argument(
        "--audio-driver", default="disk", help="SDL_AUDIODRIVER (disk records PCM, dummy discards)"
    )
    parser.add_argument(
        "--mute", action="store_true", help="pass --audio-mute (control for the audio measurement)"
    )
    parser.add_argument("--disc", help="XBOX ISO (default: main checkout tmp)")
    parser.add_argument(
        "--pad-script", help="scripted pad file (default: main checkout tmp/t1204-evidence)"
    )
    parser.add_argument(
        "--mem-every",
        type=float,
        default=0.0,
        help="read-only guest memory snapshot period in seconds (0 = off)",
    )
    parser.add_argument(
        "--mem-start", type=float, default=40.0, help="first snapshot time in seconds"
    )
    parser.add_argument("--mem-dir", help="snapshot directory (default OUT_DIR/mem)")
    args = parser.parse_args()
    result = run(args)
    summary = {key: value for key, value in result.items() if key != "audio_windows"}
    print(json.dumps(summary, indent=1))
    return 0 if result["exit_status"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
