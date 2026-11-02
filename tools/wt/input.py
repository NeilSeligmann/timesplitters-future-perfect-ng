"""Private local control channel for GPU display input and screenshots (T2340)."""

from __future__ import annotations

import ctypes
import ctypes.util
import json
import os
import re
import socket
import socketserver
import subprocess
import threading
import time
from pathlib import Path
from typing import Any

MAX_REQUEST_BYTES = 64 * 1024
MAX_TEXT_CHARS = 4096
KEY_NAME = re.compile(r"[A-Za-z0-9_+]+\Z")


class _XMotionEvent(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("serial", ctypes.c_ulong),
        ("send_event", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("window", ctypes.c_ulong),
        ("root", ctypes.c_ulong),
        ("subwindow", ctypes.c_ulong),
        ("time", ctypes.c_ulong),
        ("x", ctypes.c_int),
        ("y", ctypes.c_int),
        ("x_root", ctypes.c_int),
        ("y_root", ctypes.c_int),
        ("state", ctypes.c_uint),
        ("is_hint", ctypes.c_char),
        ("same_screen", ctypes.c_int),
    ]


class _XButtonEvent(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("serial", ctypes.c_ulong),
        ("send_event", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("window", ctypes.c_ulong),
        ("root", ctypes.c_ulong),
        ("subwindow", ctypes.c_ulong),
        ("time", ctypes.c_ulong),
        ("x", ctypes.c_int),
        ("y", ctypes.c_int),
        ("x_root", ctypes.c_int),
        ("y_root", ctypes.c_int),
        ("state", ctypes.c_uint),
        ("button", ctypes.c_uint),
        ("same_screen", ctypes.c_int),
    ]


class _XEvent(ctypes.Union):
    _fields_ = [
        ("type", ctypes.c_int),
        ("motion", _XMotionEvent),
        ("button", _XButtonEvent),
        ("pad", ctypes.c_long * 24),
    ]


def send_pointer_event(display_name: str, window_id: str, x: int, y: int, button: int = 0) -> None:
    """Send X11 MotionNotify and optional button events without requiring a physical seat."""
    library = ctypes.util.find_library("X11")
    if library is None:
        raise RuntimeError("libX11 is required for mouse injection")
    x11 = ctypes.CDLL(library)
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x11.XDefaultRootWindow.restype = ctypes.c_ulong
    x11.XTranslateCoordinates.argtypes = [
        ctypes.c_void_p,
        ctypes.c_ulong,
        ctypes.c_ulong,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.POINTER(ctypes.c_int),
        ctypes.POINTER(ctypes.c_int),
        ctypes.POINTER(ctypes.c_ulong),
    ]
    x11.XTranslateCoordinates.restype = ctypes.c_int
    x11.XSendEvent.argtypes = [
        ctypes.c_void_p,
        ctypes.c_ulong,
        ctypes.c_int,
        ctypes.c_long,
        ctypes.POINTER(_XEvent),
    ]
    x11.XSendEvent.restype = ctypes.c_int
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]

    display = x11.XOpenDisplay(display_name.encode())
    if not display:
        raise RuntimeError(f"cannot open X display {display_name}")
    try:
        window = int(window_id, 0)
        root = x11.XDefaultRootWindow(display)
        root_x, root_y, child = ctypes.c_int(), ctypes.c_int(), ctypes.c_ulong()
        if not x11.XTranslateCoordinates(
            display,
            window,
            root,
            x,
            y,
            ctypes.byref(root_x),
            ctypes.byref(root_y),
            ctypes.byref(child),
        ):
            raise RuntimeError(f"cannot translate coordinates for X window {window_id}")

        motion = _XEvent()
        motion.motion = _XMotionEvent(
            6, 0, 1, display, window, root, 0, 0, x, y, root_x.value, root_y.value, 0, b"\0", 1
        )
        if not x11.XSendEvent(display, window, 0, 1 << 6, ctypes.byref(motion)):
            raise RuntimeError("X server refused pointer motion event")
        if button:
            button_mask = 1 << (7 + button)
            for event_type, state in ((4, 0), (5, button_mask)):
                event = _XEvent()
                event.button = _XButtonEvent(
                    event_type,
                    0,
                    1,
                    display,
                    window,
                    root,
                    0,
                    0,
                    x,
                    y,
                    root_x.value,
                    root_y.value,
                    state,
                    button,
                    1,
                )
                mask = (1 << 2) if event_type == 4 else (1 << 3)
                if not x11.XSendEvent(display, window, 0, mask, ctypes.byref(event)):
                    raise RuntimeError("X server refused pointer button event")
        x11.XSync(display, 0)
    finally:
        x11.XCloseDisplay(display)


class InputController:
    """Inject X11 input into the launched client and capture its Xwayland window."""

    def __init__(
        self, display: str, width: int, height: int, environment: dict[str, str] | None = None
    ) -> None:
        self.display = display
        self.width = width
        self.height = height
        self.environment = dict(environment or {})
        self.environment["DISPLAY"] = display
        self.child_pid: int | None = None
        self._focused_window: str | None = None
        self._lock = threading.RLock()

    def _run(self, argv: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            argv,
            env=self.environment,
            text=True,
            capture_output=True,
            check=False,
            timeout=15,
        )
        if check and result.returncode:
            detail = result.stderr.strip() or result.stdout.strip()
            raise RuntimeError(detail or f"command failed with status {result.returncode}")
        return result

    @staticmethod
    def _descendants(pid: int) -> list[int]:
        pending = [pid]
        found: list[int] = []
        while pending:
            current = pending.pop()
            if current in found:
                continue
            found.append(current)
            children = Path(f"/proc/{current}/task/{current}/children")
            try:
                pending.extend(int(value) for value in children.read_text().split())
            except (FileNotFoundError, PermissionError, ProcessLookupError, ValueError):
                continue
        return found

    def _find_window(self, title: str | None) -> str | None:
        if title is not None:
            result = self._run(["xdotool", "search", "--onlyvisible", "--name", title], check=False)
            windows = result.stdout.splitlines()
        elif self.child_pid is not None:
            windows = []
            for pid in self._descendants(self.child_pid):
                result = self._run(
                    ["xdotool", "search", "--onlyvisible", "--pid", str(pid)], check=False
                )
                windows.extend(result.stdout.splitlines())
        else:
            raise RuntimeError("the display command has not started")
        return windows[-1].strip() if windows else None

    def focus(self, title: str | None = None, timeout: float = 5.0) -> str:
        if title is not None and (not isinstance(title, str) or len(title) > 256):
            raise ValueError("title must be a string of at most 256 characters")
        if not isinstance(timeout, (float, int)) or not 0 <= timeout <= 30:
            raise ValueError("focus timeout must be between 0 and 30 seconds")
        deadline = time.monotonic() + timeout
        window: str | None = None
        while window is None:
            window = self._find_window(title)
            if window is not None:
                break
            if time.monotonic() >= deadline:
                selector = (
                    f" matching title {title!r}"
                    if title is not None
                    else " for the launched process"
                )
                raise RuntimeError(f"no visible game window found{selector}")
            time.sleep(0.05)
        # xdotool --sync waits for a FocusIn event, which headless Xwayland may not emit for a
        # window that is already focused (or when no window manager is present). Input below is
        # sent directly to this window, so request focus asynchronously and avoid that hang.
        if self._focused_window != window:
            self._run(["xdotool", "windowfocus", window])
            self._focused_window = window
        return window

    def _position(self, x: Any, y: Any) -> tuple[int, int]:
        if (
            isinstance(x, bool)
            or not isinstance(x, int)
            or isinstance(y, bool)
            or not isinstance(y, int)
        ):
            raise ValueError("x and y must be integers")
        if not 0 <= x < self.width or not 0 <= y < self.height:
            raise ValueError(f"x and y must be inside the {self.width}x{self.height} display")
        return x, y

    def handle(self, request: dict[str, Any]) -> dict[str, Any]:
        with self._lock:
            return self._handle(request)

    def _handle(self, request: dict[str, Any]) -> dict[str, Any]:
        operation = request.get("op")
        if operation == "focus":
            return {"window": self.focus(request.get("title"), request.get("timeout", 5))}
        if operation == "key":
            key = request.get("key")
            if not isinstance(key, str) or len(key) > 128 or not KEY_NAME.fullmatch(key):
                raise ValueError("key must be a keysym or chord such as Escape or ctrl+a")
            window = self.focus(request.get("title"), 5)
            self._run(
                ["xdotool", "key", "--window", window, "--clearmodifiers", "--delay", "25", key]
            )
            return {"window": window}
        if operation == "type":
            value = request.get("text")
            if not isinstance(value, str) or len(value) > MAX_TEXT_CHARS:
                raise ValueError(f"text must be a string of at most {MAX_TEXT_CHARS} characters")
            window = self.focus(request.get("title"), 5)
            self._run(
                [
                    "xdotool",
                    "type",
                    "--window",
                    window,
                    "--clearmodifiers",
                    "--delay",
                    "1",
                    "--",
                    value,
                ]
            )
            return {"window": window}
        if operation in ("move", "click"):
            x, y = self._position(request.get("x"), request.get("y"))
            button = request.get("button", "left")
            buttons = {"left": "1", "middle": "2", "right": "3"}
            if operation == "click" and (not isinstance(button, str) or button not in buttons):
                raise ValueError("button must be left, middle, or right")
            window = self.focus(request.get("title"), 5)
            send_pointer_event(
                self.display,
                window,
                x,
                y,
                int(buttons[button]) if operation == "click" else 0,
            )
            return {"window": window}
        if operation == "screenshot":
            output = request.get("output")
            if not isinstance(output, str) or not output or len(output) > 4096:
                raise ValueError("output must be a non-empty path of at most 4096 characters")
            window = self.focus(request.get("title"), request.get("timeout", 5))
            path = Path(output).expanduser().absolute()
            self._run(["import", "-display", self.display, "-window", window, "-silent", str(path)])
            if not path.is_file() or path.stat().st_size == 0:
                raise RuntimeError(f"screenshot was not written: {path}")
            return {"path": str(path)}
        raise ValueError("op must be focus, key, type, move, click, or screenshot")


class _ControlHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        line = self.rfile.readline(MAX_REQUEST_BYTES + 1)
        try:
            if len(line) > MAX_REQUEST_BYTES:
                raise ValueError("request exceeds the 64 KiB limit")
            request = json.loads(line)
            if not isinstance(request, dict):
                raise ValueError("request must be a JSON object")
            result = self.server.controller.handle(request)  # type: ignore[attr-defined]
            response = {"ok": True, **result}
        except (
            json.JSONDecodeError,
            UnicodeDecodeError,
            ValueError,
            RuntimeError,
            OSError,
            subprocess.SubprocessError,
        ) as exc:
            response = {"ok": False, "error": str(exc)}
        self.wfile.write(json.dumps(response).encode() + b"\n")


class _ControlServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, path: Path, controller: InputController) -> None:
        self.controller = controller
        super().__init__(str(path), _ControlHandler)


class InputControl:
    """Own the permission-restricted Unix socket used by the input CLI."""

    def __init__(self, path: Path, controller: InputController) -> None:
        self.path = path
        self.server = _ControlServer(path, controller)
        os.chmod(path, 0o600)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def close(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.path.unlink(missing_ok=True)

    def __enter__(self) -> InputControl:
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()


def send_request(path: Path, request: dict[str, Any]) -> dict[str, Any]:
    """Send one bounded JSON request to a live GPU display and return its response."""
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(20)
        connection.connect(str(path))
        connection.sendall(json.dumps(request).encode() + b"\n")
        stream = connection.makefile("rb")
        line = stream.readline(MAX_REQUEST_BYTES + 1)
    if len(line) > MAX_REQUEST_BYTES:
        raise RuntimeError("control response exceeds the 64 KiB limit")
    response = json.loads(line)
    if not isinstance(response, dict):
        raise RuntimeError("invalid response from GPU display control socket")
    if not response.get("ok"):
        raise RuntimeError(str(response.get("error", "GPU display control failed")))
    return response
