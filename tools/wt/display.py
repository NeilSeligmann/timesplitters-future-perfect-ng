"""Collision-free Xvfb display allocation for parallel hybrid-host / xemu runs (T1883).

Each display number is guarded by an flock on `<lock dir>/<n>.lock`. The kernel drops the lock when
the owner dies, so a crashed run never leaks a number; the owner PID is written into the lock file
so `sweep` can also report and reap an orphaned Xvfb. Capture helpers now reserve by default
through tools.hostlock.
"""

from __future__ import annotations

import contextlib
import fcntl
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable
from pathlib import Path

FIRST = 130
LAST = 199
_HELD: dict[int, int] = {}
_LEASES: dict[int, object] = {}


def lock_dir() -> Path:
    path = Path(
        os.environ.get("TSFP_XDISPLAY_DIR") or Path(tempfile.gettempdir()) / "tsfp-xdisplay"
    )
    path.mkdir(parents=True, exist_ok=True)
    return path


def x_busy(number: int, x_dir: Path) -> bool:
    return (x_dir / f".X{number}-lock").exists() or (x_dir / ".X11-unix" / f"X{number}").exists()


def reserve_number(
    first: int = FIRST,
    last: int = LAST,
    directory: Path | None = None,
    x_dir: Path | None = None,
    sock_dir: Path | None = None,
) -> int:
    """Reserve a display number for the life of this process (flock held in-process)."""
    directory = directory or lock_dir()
    x_dir = x_dir or Path("/tmp")
    directory.mkdir(parents=True, exist_ok=True)
    sock_dir = sock_dir or x_dir / ".X11-unix"
    for number in range(first, last + 1):
        if number in _HELD:
            continue
        descriptor = os.open(directory / f"{number}.lock", os.O_RDWR | os.O_CREAT, 0o600)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            os.close(descriptor)
            continue
        from tools.wt import lease

        lease_handle = lease.attach(descriptor, directory / f"{number}.lock", "x-display")
        # Sweep only while holding the reservation, and only with a dead X PID.
        from tools.t982_acceptance import display_owner_alive

        lock = x_dir / f".X{number}-lock"
        socket = sock_dir / f"X{number}"
        if lock.exists() and not display_owner_alive(number, x_dir, reservation_held=True):
            lock.unlink(missing_ok=True)
            socket.unlink(missing_ok=True)
        if lock.exists() or os.path.lexists(socket):
            if lease_handle is not None:  # attach returns None with TSFP_LEASE=0 or on error
                lease_handle.release()
            os.close(descriptor)
            continue
        os.ftruncate(descriptor, 0)
        os.write(descriptor, f"{os.getpid()}\n".encode())
        _HELD[number] = descriptor
        _LEASES[number] = lease_handle
        return number
    raise SystemExit("no free X display")


def release_number(number: int) -> None:
    lease_handle = _LEASES.pop(number, None)
    if lease_handle is not None:
        lease_handle.release()
    descriptor = _HELD.pop(number, None)
    if descriptor is not None:
        os.close(descriptor)


class XDisplay:
    """Own one Xvfb on a reserved display; `close()` kills it by PID and frees the number."""

    def __init__(self, screen: str = "1280x720x24", timeout: float = 15.0) -> None:
        self.number = reserve_number()
        self.name = f":{self.number}"
        try:
            self.process = subprocess.Popen(
                ["Xvfb", self.name, "-screen", "0", screen],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except BaseException:
            release_number(self.number)
            raise
        socket = Path("/tmp/.X11-unix") / f"X{self.number}"
        deadline = time.monotonic() + timeout
        while not socket.exists():
            if self.process.poll() is not None or time.monotonic() > deadline:
                self.close()
                raise SystemExit(f"Xvfb {self.name} did not start")
            time.sleep(0.05)

    def close(self) -> None:
        from tools.t982_acceptance import stop_xvfb

        stop_xvfb(self.process, self.number)

    def __enter__(self) -> XDisplay:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


def run_with_display(argv: list[str], screen: str = "1280x720x24") -> int:
    """Run argv with DISPLAY set to a private Xvfb, then tear the Xvfb down."""
    with XDisplay(screen) as display:
        env = dict(os.environ, DISPLAY=display.name)
        child = subprocess.Popen(argv, env=env)
        try:
            return child.wait()
        except KeyboardInterrupt:
            child.send_signal(signal.SIGTERM)
            return child.wait()


# T2168: a real-GPU display. Xvfb has no DRI3, so Vulkan on the container's GPU (RADV)
# cannot present into it ("vulkan: No DRI3 support detected"). A headless weston with the
# GL renderer and Xwayland gives X11 clients (the tsfp-bevy shell is built X11-only) a
# DRI3-capable X server on the real GPU. Private XDG_RUNTIME_DIR and socket, torn down by
# process group (never by name pattern).

WESTON_DISPLAY_LINE = re.compile(r"xserver listening on display (:\d+)")


def weston_argv(socket: str, size: str, log: Path) -> list[str]:
    width, _, height = size.partition("x")
    return [
        "weston",
        "--backend=headless",
        "--renderer=gl",
        f"--socket={socket}",
        f"--width={int(width)}",
        f"--height={int(height.split('x')[0])}",
        "--xwayland",
        f"--log={log}",
    ]


def weston_x_display(log_text: str) -> str | None:
    """The Xwayland DISPLAY weston announced in its log, if any yet."""
    found = WESTON_DISPLAY_LINE.findall(log_text)
    return found[-1] if found else None


class GpuDisplay:
    """Own one headless weston + Xwayland; `close()` kills its group, removes the runtime dir."""

    def __init__(
        self,
        size: str = "1280x720",
        timeout: float = 20.0,
        compositor: Callable[[str, str, Path], list[str]] = weston_argv,
    ) -> None:
        """`compositor(socket, size, log)` is the argv; tests inject a fake one (T3825)."""
        self.runtime = Path(tempfile.mkdtemp(prefix="tsfp-weston-"))
        os.chmod(self.runtime, 0o700)
        self.socket = f"tsfp-wl-{os.getpid()}"
        self.log = self.runtime / "weston.log"
        self.control_path = self.runtime / "control.sock"
        env = dict(os.environ, XDG_RUNTIME_DIR=str(self.runtime))
        env.pop("DISPLAY", None)
        env.pop("WAYLAND_DISPLAY", None)
        self.process = subprocess.Popen(
            compositor(self.socket, size, self.log),
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        deadline = time.monotonic() + timeout
        self.name: str | None = None
        while self.name is None:
            text = self.log.read_text(errors="replace") if self.log.exists() else ""
            self.name = weston_x_display(text)
            if self.name is None and (
                self.process.poll() is not None or time.monotonic() > deadline
            ):
                tail = self.log_tail()
                self.close()
                raise SystemExit(f"weston did not announce an Xwayland display; log tail:\n{tail}")
            time.sleep(0.1)

    def log_tail(self, lines: int = 30) -> str:
        """The compositor log's last lines (the runtime dir holding it is removed on close)."""
        try:
            return "\n".join(self.log.read_text(errors="replace").splitlines()[-lines:])
        except OSError:
            return "(no compositor log)"

    def env(self) -> dict[str, str]:
        return dict(
            os.environ,
            DISPLAY=self.name or "",
            XDG_RUNTIME_DIR=str(self.runtime),
            WAYLAND_DISPLAY=self.socket,
            TSFP_WT_CONTROL_SOCKET=str(self.control_path),
        )

    def close(self) -> None:
        if self.process.poll() is None:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
                self.process.wait(timeout=10)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait()
        shutil.rmtree(self.runtime, ignore_errors=True)

    def __enter__(self) -> GpuDisplay:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


def run_with_gpu_display(
    argv: list[str],
    size: str = "1280x720",
    compositor: Callable[[str, str, Path], list[str]] = weston_argv,
) -> int:
    """Run argv under a private headless weston/Xwayland on the real GPU, then tear it down.

    A failing child gets the compositor log tail on stderr: the log dies with the runtime dir."""
    from tools.wt.input import InputControl, InputController

    with GpuDisplay(size, compositor=compositor) as display:
        width, _, height = size.partition("x")
        controller = InputController(
            display.name or "", int(width), int(height.split("x")[0]), display.env()
        )
        with InputControl(display.control_path, controller):
            child = subprocess.Popen(argv, env=display.env())
            controller.child_pid = child.pid
            print(f"WT_CONTROL_SOCKET={display.control_path}", flush=True)
            try:
                code = child.wait()
            except KeyboardInterrupt:
                child.send_signal(signal.SIGTERM)
                return child.wait()
            if code != 0:
                print(
                    f"gpu display: child exited {code} on {display.name}; compositor log tail:\n"
                    + display.log_tail(),
                    file=sys.stderr,
                    flush=True,
                )
            return code
