# SPDX-License-Identifier: GPL-3.0-or-later
# ruff: noqa: E501
"""Take and resume a whole-process snapshot of a `tools.play` run (T1153, docs/state-snapshot.md).

Take: one DMTCP computation holds Xvfb (via xvfb-run) and `python -m tools.play ...`, which runs the host.
The host asks for the checkpoint itself at the requested port 0 poll (src/host/host_snapshot.c), so the
point is a function of the guest, never of wall time. The plugin copies the run directory (the HDD files the
title created) while every process is quiesced. The launcher then writes the manifest that pins the exact
host binary and every mapped library, and ends the run.

Resume: verify the manifest (loud refusal on any mismatch), restore the run directory copy, start a fresh
coordinator and `dmtcp_restart` the images. The restored tools.play finishes the run and writes its stop
report into the run directory, which this module prints.
"""

import os
import shutil
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

from tools.private_host import parse_stop
from tools.snapshot import dmtcp, manifest, options

ROOT = Path(__file__).resolve().parents[2]
SNAPSHOTS = Path("tmp/snapshots")
INNER_ENV = options.INNER_ENV
ISOLATED_RUN_DIR_SCHEMA = "pathvirt-per-poll-v1"
#: DMTCP's own checkpoint signal collides with the host's use of SIGUSR2, so a real-time signal is used.
CKPT_SIGNAL = "40"
INNER_LOG = "snapshot-inner.log"
TAKE_TIMEOUT = 900
SCREEN = "1280x1024x24"


def free_port() -> int:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def free_display() -> int:
    """An X display number nobody holds, so the snapshot's Xvfb can be restored on the same number."""
    for number in range(200, 400):
        if not Path(f"/tmp/.X11-unix/X{number}").exists() and not manifest.x_display_in_use(number):
            return number
    raise SystemExit("snapshot: no free X display number in 200..399")


def coordinator_env(port: int) -> dict[str, str]:
    return {**os.environ, "DMTCP_COORD_PORT": str(port), "DMTCP_SIGCKPT": CKPT_SIGNAL}


def pathvirt_original_prefixes(run_dir: Path) -> list[str]:
    """Paths owned by a snapshot process tree that parallel resumes must privatize."""
    return [str(run_dir), "/tmp/.X11-unix", "/tmp/.X", str(ROOT / "tmp/xdg-snapshot")]


def pathvirt_new_prefixes(isolated_run_dir: Path, isolated_runtime: Path) -> list[str]:
    """Replacement prefixes corresponding to ``pathvirt_original_prefixes``."""
    return [
        str(isolated_run_dir),
        str(isolated_run_dir / "parallel-x11"),
        str(isolated_run_dir),
        str(isolated_runtime),
    ]


def pathvirt_resume_environment(
    run_dir: Path, isolated_run_dir: Path, isolated_runtime: Path
) -> dict[str, str]:
    """Provide both sides of the path mapping to DMTCP at restart."""
    return {
        "DMTCP_ORIGINAL_PATH_PREFIX": ":".join(pathvirt_original_prefixes(run_dir)),
        "DMTCP_NEW_PATH_PREFIX": ":".join(
            pathvirt_new_prefixes(isolated_run_dir, isolated_runtime)
        ),
    }


def write_resume_marker(
    run_dir: Path, output_dir: Path, *, isolate_run_dir: bool, seconds: float | None
) -> list[Path]:
    """Publish the resume request where both mapped and captured paths can read it.

    DMTCP path virtualization restores filesystem paths, but the captured
    ``TSFP_SNAPSHOT_RUN_DIR`` environment value can still name the original run
    directory in the resumed Python process. Keep a matching marker at that
    path and at its isolated replacement; remove both after the resume exits.
    """
    marker_dirs = [run_dir]
    if isolate_run_dir and output_dir != run_dir:
        marker_dirs.append(output_dir)
    contents = "resume\n" + (f"seconds={seconds:g}\n" if seconds else "")
    contents += f"out_dir={output_dir}\n"
    paths = []
    for directory in marker_dirs:
        directory.mkdir(parents=True, exist_ok=True)
        marker = directory / "resume-requested"
        marker.write_text(contents)
        paths.append(marker)
    return paths


def start_coordinator(tools: dict[str, Path], port: int, ckpt_dir: Path, log: Path) -> None:
    ckpt_dir.mkdir(parents=True, exist_ok=True)
    done = subprocess.run(
        [
            str(tools["dmtcp_coordinator"]),
            "--daemon",
            "--port",
            str(port),
            "--ckptdir",
            str(ckpt_dir),
            "--coord-logfile",
            str(log),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=60,
        check=False,
    )
    if done.returncode != 0:
        raise SystemExit(f"snapshot: dmtcp_coordinator failed to start on port {port}")


def coordinator_alive(tools: dict[str, Path], port: int) -> bool:
    done = subprocess.run(
        [str(tools["dmtcp_command"]), "--port", str(port), "--status"],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    return "NUM_PEERS" in done.stdout


def quit_computation(tools: dict[str, Path], port: int) -> None:
    """Ask the coordinator to kill every process of the computation and exit, until it is gone."""
    for _ in range(6):
        subprocess.run(
            [str(tools["dmtcp_command"]), "--port", str(port), "--quit"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=60,
            check=False,
        )
        time.sleep(0.5)
        if not coordinator_alive(tools, port):
            return
    print(f"snapshot: WARNING coordinator on port {port} did not exit", file=sys.stderr)


def private_runtime() -> Path:
    runtime = ROOT / "tmp/xdg-snapshot"
    runtime.mkdir(parents=True, exist_ok=True)
    runtime.chmod(0o700)
    return runtime


def repository_head() -> str:
    probe = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    return probe.stdout.strip()


def inner_command(
    tools: dict[str, Path],
    port: int,
    ckpt_dir: Path,
    display: int,
    play_argv: list[str],
    gzip: bool,
) -> list[str]:
    return [
        str(tools["dmtcp_launch"]),
        "--pathvirt",
        "--join-coordinator",
        "--coord-port",
        str(port),
        "--ckptdir",
        str(ckpt_dir),
        "--gzip" if gzip else "--no-gzip",
        "--with-plugin",
        str(tools["plugin"]),
        "xvfb-run",
        "-n",
        str(display),
        "-s",
        f"-screen 0 {SCREEN} -nolisten tcp",
        sys.executable,
        "-m",
        "tools.play",
        *play_argv,
    ]


def capture_command(
    tools: dict[str, Path],
    port: int,
    ckpt_dir: Path,
    capture_argv: list[str],
    gzip: bool,
) -> list[str]:
    """Launch rust_trace under the same DMTCP process tree as its Bevy host and Xvfb."""
    return [
        str(tools["dmtcp_launch"]),
        "--pathvirt",
        "--join-coordinator",
        "--coord-port",
        str(port),
        "--ckptdir",
        str(ckpt_dir),
        "--gzip" if gzip else "--no-gzip",
        "--with-plugin",
        str(tools["plugin"]),
        sys.executable,
        "-m",
        "tools.rust_trace",
        *capture_argv,
    ]


def wait_for_checkpoint(process: subprocess.Popen[bytes], ckpt_dir: Path, limit: float) -> bool:
    """True once the coordinator wrote the restart script (all images complete), False if the run ended."""
    script = ckpt_dir / "dmtcp_restart_script.sh"
    deadline = time.monotonic() + limit
    while time.monotonic() < deadline:
        if script.exists() and list(ckpt_dir.glob("ckpt_*.dmtcp")):
            time.sleep(0.5)
            return True
        if process.poll() is not None:
            return script.exists() and bool(list(ckpt_dir.glob("ckpt_*.dmtcp")))
        time.sleep(0.2)
    return False


def stop_process_group(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    for sig, wait in ((signal.SIGTERM, 10), (signal.SIGKILL, 10)):
        try:
            os.killpg(process.pid, sig)
        except ProcessLookupError:
            return
        try:
            process.wait(timeout=wait)
            return
        except subprocess.TimeoutExpired:
            continue


def take(
    play_argv: list[str],
    *,
    poll: int,
    host: Path,
    xbe: Path,
    record: Path | None,
    run_dir: Path,
    snapshot_dir: Path,
    keep_running: bool,
    gzip: bool,
) -> int:
    """Run `tools.play play_argv` under DMTCP and snapshot it at port 0 poll `poll`."""
    tools = dmtcp.locate()
    started = time.monotonic()
    snapshot_dir = snapshot_dir.resolve()
    run_dir = (ROOT / run_dir).resolve() if not run_dir.is_absolute() else run_dir
    shutil.rmtree(snapshot_dir, ignore_errors=True)
    snapshot_dir.mkdir(parents=True)
    ckpt_dir = snapshot_dir / "ckpt"
    run_dir.mkdir(parents=True, exist_ok=True)
    port, display = free_port(), free_display()
    start_coordinator(tools, port, ckpt_dir, snapshot_dir / "coordinator.log")
    env = {
        **coordinator_env(port),
        INNER_ENV: "1",
        "TSFP_SNAPSHOT_DIR": str(snapshot_dir),
        "TSFP_SNAPSHOT_RUN_DIR": str(run_dir),
        "DMTCP_ORIGINAL_PATH_PREFIX": ":".join(pathvirt_original_prefixes(run_dir)),
        "XDG_RUNTIME_DIR": str(private_runtime()),
        "SDL_AUDIODRIVER": os.environ.get("SDL_AUDIODRIVER", "dummy"),
    }
    argv = inner_command(tools, port, ckpt_dir, display, play_argv, gzip)
    print(
        f"snapshot: taking at poll {poll} of port 0, host {host}, X display :{display}", flush=True
    )
    log = (run_dir / INNER_LOG).open("wb")
    process = subprocess.Popen(  # noqa: S603
        argv,
        cwd=ROOT,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        taken = wait_for_checkpoint(process, ckpt_dir, TAKE_TIMEOUT)
        run_seconds = time.monotonic() - started
        if taken and not keep_running:
            quit_computation(tools, port)
        if keep_running:
            process.wait(timeout=TAKE_TIMEOUT)
    except subprocess.TimeoutExpired:
        taken = False
    finally:
        quit_computation(tools, port)  # also ends the coordinator when the run ended by itself
        stop_process_group(process)
        log.close()
    if not taken:
        found = parse_stop(run_dir)
        print(
            f"snapshot: NOT taken: the run ended before port 0 reached poll {poll} "
            f"(stop: {found['stop']}). See {run_dir / INNER_LOG}",
            file=sys.stderr,
        )
        return 1
    seconds = {"take_total": run_seconds}
    record_manifest(
        play_argv=play_argv,
        poll=poll,
        host=host,
        xbe=xbe,
        record=record,
        run_dir=run_dir,
        display=display,
        snapshot_dir=snapshot_dir,
        gzip=gzip,
        seconds=seconds,
    )
    return 0


def capture_environment(port: int, snapshot_dir: Path, run_dir: Path) -> dict[str, str]:
    """Build an inner environment whose tmp paths do not need Git under DMTCP."""
    env = {
        **coordinator_env(port),
        INNER_ENV: "1",
        "TSFP_SNAPSHOT_DIR": str(snapshot_dir),
        "TSFP_SNAPSHOT_RUN_DIR": str(run_dir),
        # rust_trace's args are restored from the checkpoint. Keep its output
        # under the pathvirt-managed run directory so each segment can collect
        # its trace from its private resumed directory.
        "TSFP_SNAPSHOT_OUT_DIR": str(run_dir),
        "DMTCP_ORIGINAL_PATH_PREFIX": ":".join(pathvirt_original_prefixes(run_dir)),
        "XDG_RUNTIME_DIR": str(private_runtime()),
        "SDL_AUDIODRIVER": "dummy",
    }
    # tools.datadir otherwise shells out to `git rev-parse` from the checkpointed
    # process. Pin the shared tmp root before DMTCP starts that process tree.
    env["TSFP_DATA_DIR"] = os.environ.get("TSFP_DATA_DIR") or str(dmtcp.main_root() / "tmp")
    # run_host writes a recipe after the capture. Resolve its source revision
    # before checkpointing so the restored child does not need a new Git process.
    env["TSFP_RECIPE_GIT_HEAD"] = repository_head()
    return env


def keep_continued_capture(run_dir: Path, destination: Path) -> None:
    """Copy the continued capture's trace and log before a resume restores the run directory."""
    destination.mkdir(parents=True, exist_ok=True)
    for name in ("trace.json", "run.log"):
        if (run_dir / name).is_file():
            shutil.copy2(run_dir / name, destination / name)


def take_capture(
    capture_argv: list[str],
    *,
    poll: int,
    host: Path,
    xbe: Path,
    record: Path,
    run_dir: Path,
    snapshot_dir: Path,
    gzip: bool,
    keep_running: bool = False,
) -> int:
    """Checkpoint a `tools.rust_trace capture` process and its Bevy host at port 0 poll `poll`.

    With `keep_running` the capture runs on to its normal end after the checkpoint and its
    trace and log are kept in `<snapshot_dir>/continued`: the exact execution the snapshot was
    cut from, so a resumed capture can be compared with it sample for sample (T2479).
    """
    tools = dmtcp.locate()
    started = time.monotonic()
    snapshot_dir = snapshot_dir.resolve()
    run_dir = (ROOT / run_dir).resolve() if not run_dir.is_absolute() else run_dir
    shutil.rmtree(snapshot_dir, ignore_errors=True)
    snapshot_dir.mkdir(parents=True)
    ckpt_dir = snapshot_dir / "ckpt"
    run_dir.mkdir(parents=True, exist_ok=True)
    port = free_port()
    start_coordinator(tools, port, ckpt_dir, snapshot_dir / "coordinator.log")
    env = capture_environment(port, snapshot_dir, run_dir)
    argv = capture_command(tools, port, ckpt_dir, capture_argv, gzip)
    print(f"snapshot: taking rust_trace capture at poll {poll}, host {host}", flush=True)
    log = (snapshot_dir / "capture-take.log").open("wb")
    process = subprocess.Popen(  # noqa: S603
        argv,
        cwd=ROOT,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        taken = wait_for_checkpoint(process, ckpt_dir, TAKE_TIMEOUT)
        run_seconds = time.monotonic() - started
        if taken and not keep_running:
            quit_computation(tools, port)
        if taken and keep_running:
            process.wait(timeout=TAKE_TIMEOUT)
            keep_continued_capture(run_dir, snapshot_dir / "continued")
    except subprocess.TimeoutExpired:
        taken = False
    finally:
        quit_computation(tools, port)
        stop_process_group(process)
        log.close()
    display_file = snapshot_dir / "display-number"
    if not taken or not display_file.is_file():
        print(
            f"snapshot: NOT taken: rust_trace capture ended or failed before port 0 poll {poll}; "
            f"see {snapshot_dir / 'capture-take.log'}",
            file=sys.stderr,
        )
        return 1
    record_manifest(
        play_argv=capture_argv,
        poll=poll,
        host=host,
        xbe=xbe,
        record=record,
        run_dir=run_dir,
        display=int(display_file.read_text().strip()),
        snapshot_dir=snapshot_dir,
        gzip=gzip,
        seconds={"take_total": run_seconds},
    )
    return 0


def take_capture_many(
    capture_argv: list[str],
    *,
    polls: list[int],
    host: Path,
    xbe: Path,
    record: Path,
    run_dir: Path,
    snapshot_dir: Path,
    gzip: bool,
) -> int:
    """Take several ordered checkpoints during one capture pass, copying each before continuing."""
    import json

    if not polls or len(polls) > 64 or any(poll < 1 for poll in polls):
        raise ValueError("multi-snapshot capture needs 1..64 positive polls")
    if any(first >= second for first, second in zip(polls, polls[1:], strict=False)):
        raise ValueError("multi-snapshot polls must be unique and increasing")
    tools = dmtcp.locate()
    started = time.monotonic()
    snapshot_dir = snapshot_dir.resolve()
    run_dir = (ROOT / run_dir).resolve() if not run_dir.is_absolute() else run_dir
    shutil.rmtree(snapshot_dir, ignore_errors=True)
    snapshot_dir.mkdir(parents=True)
    ckpt_dir = snapshot_dir / "ckpt"
    run_dir.mkdir(parents=True, exist_ok=True)
    port, display = free_port(), free_display()
    start_coordinator(tools, port, ckpt_dir, snapshot_dir / "coordinator.log")
    env = capture_environment(port, snapshot_dir, run_dir)
    env["TSFP_SNAPSHOT_ISOLATE_RUN_DIR"] = "1"
    argv = capture_command(tools, port, ckpt_dir, capture_argv, gzip)
    print(f"snapshot: taking {len(polls)} checkpoints in one route pass at {polls}", flush=True)
    log = (run_dir / INNER_LOG).open("wb")
    process = subprocess.Popen(  # noqa: S603
        argv,
        cwd=ROOT,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    completed = 0
    try:
        for poll in polls:
            marker = snapshot_dir / f"poll-{poll}.checkpointed"
            deadline = time.monotonic() + TAKE_TIMEOUT
            while time.monotonic() < deadline and not marker.is_file():
                if process.poll() is not None:
                    break
                time.sleep(0.1)
            if not marker.is_file():
                raise RuntimeError(f"host ended before checkpoint poll {poll}")
            destination = snapshot_dir / f"poll-{poll}"
            destination.mkdir(parents=True, exist_ok=True)
            destination_ckpt = destination / "ckpt"
            destination_ckpt.mkdir()
            images = list(ckpt_dir.glob("ckpt_*.dmtcp"))
            if not images:
                raise RuntimeError(f"DMTCP produced no images at poll {poll}")
            for image in images:
                shutil.copy2(image, destination_ckpt / image.name)
            restart_script = ckpt_dir / "dmtcp_restart_script.sh"
            if restart_script.is_file():
                shutil.copy2(restart_script, destination_ckpt / restart_script.name)
            for name in ("mapped-files.txt", "display-number"):
                source = snapshot_dir / name
                if source.is_file():
                    shutil.copy2(source, destination / name)
            shutil.copytree(snapshot_dir / "run-dir", destination / "run-dir", symlinks=True)
            record_manifest(
                play_argv=capture_argv,
                poll=poll,
                host=host,
                xbe=xbe,
                record=record,
                run_dir=run_dir,
                display=display,
                snapshot_dir=destination,
                gzip=gzip,
                seconds={"take_total": time.monotonic() - started},
            )
            manifest_path = destination / manifest.MANIFEST_NAME
            manifest_data = manifest.load(destination)
            manifest_data["run_dir_isolation"] = ISOLATED_RUN_DIR_SCHEMA
            manifest_path.write_text(json.dumps(manifest_data, indent=2) + "\n")
            (snapshot_dir / f"poll-{poll}.ack").touch()
            completed += 1
        process.wait(timeout=TAKE_TIMEOUT)
    except (RuntimeError, subprocess.TimeoutExpired) as error:
        print(
            f"snapshot: multi-checkpoint capture failed after {completed}: {error}",
            file=sys.stderr,
        )
        completed = 0
    finally:
        quit_computation(tools, port)
        stop_process_group(process)
        log.close()
    return 0 if completed == len(polls) else 1


def record_manifest(
    *,
    play_argv: list[str],
    poll: int,
    host: Path,
    xbe: Path,
    record: Path | None,
    run_dir: Path,
    display: int,
    snapshot_dir: Path,
    gzip: bool,
    seconds: dict[str, float],
) -> None:
    import json

    data = manifest.build_manifest(
        poll=poll,
        play_argv=play_argv,
        host=host,
        xbe=xbe,
        record=record,
        run_dir=run_dir,
        display=display,
        snapshot_dir=snapshot_dir,
        gzip=gzip,
        seconds=seconds,
        repository_head=repository_head(),
    )
    (snapshot_dir / manifest.MANIFEST_NAME).write_text(json.dumps(data, indent=2) + "\n")
    images = data["images"]
    assert isinstance(images, dict)
    total = sum(int(entry["bytes"]) for entry in images.values())
    host_info = data["host"]
    assert isinstance(host_info, dict)
    print(
        f"snapshot: taken at poll {poll}: {len(images)} images, {total / 1e6:.0f} MB, "
        f"host sha256 {host_info['sha256']}, lift key {host_info['lift_key']}\n"
        f"snapshot: {snapshot_dir / manifest.MANIFEST_NAME}\n"
        f"snapshot: resume with: python -m tools.play --resume-snapshot {snapshot_dir}"
    )


def restore_run_dir(snapshot_dir: Path, run_dir: Path) -> None:
    """Put the HDD files of the snapshot instant back, replacing whatever the run directory holds."""
    shutil.rmtree(run_dir, ignore_errors=True)
    shutil.copytree(snapshot_dir / "run-dir", run_dir, symlinks=True)


def resume(
    snapshot_dir: Path,
    *,
    host_override: Path | None,
    timeout: int,
    fast_verify: bool,
) -> int:
    """Verify, restore the run directory and run the snapshot to its stop. 2 when refused."""
    snapshot_dir = snapshot_dir.resolve()
    data = manifest.load(snapshot_dir)
    problems = manifest.verify(data, snapshot_dir, host_override=host_override, fast=fast_verify)
    if problems:
        for problem in problems:
            print(f"snapshot: REFUSED: {problem}", file=sys.stderr)
        print(
            "snapshot: nothing was resumed. A snapshot is valid only for the exact host binary and "
            "libraries it was taken on, take a new one with --snapshot-at-poll.",
            file=sys.stderr,
        )
        return 2
    host = data["host"]
    assert isinstance(host, dict)
    print(
        f"snapshot: resuming poll {data['poll']}, host {host['path']}\n"
        f"snapshot: pinned host sha256 {host['sha256']}, lift key {host['lift_key']}",
        flush=True,
    )
    tools = dmtcp.locate()
    run_dir = Path(str(data["run_dir"]))
    restore_run_dir(snapshot_dir, run_dir)
    port = free_port()
    ckpt_dir = snapshot_dir / "ckpt"
    started = time.monotonic()
    start_coordinator(tools, port, ckpt_dir, snapshot_dir / f"coordinator-resume-{port}.log")
    images = [str(ckpt_dir / name) for name in sorted(data["images"])]  # type: ignore[arg-type]
    log = (snapshot_dir / f"resume-{port}.log").open("wb")
    process = subprocess.Popen(  # noqa: S603
        [
            str(tools["dmtcp_restart"]),
            "--join-coordinator",
            "--coord-port",
            str(port),
            "--ckptdir",
            str(ckpt_dir),
            *images,
        ],
        cwd=ROOT,
        env=coordinator_env(port),
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        print(f"snapshot: resumed run exceeded {timeout} s, killed", file=sys.stderr)
    finally:
        quit_computation(tools, port)
        stop_process_group(process)
        log.close()
    seconds = time.monotonic() - started
    found = parse_stop(run_dir)
    print(
        f"snapshot: resumed run STOP {found['stop']}\n    detail: {found['detail']}\n"
        f"    census: {found['census']}\n    last guest calls: {found['calls']}\n"
        f"    time: {seconds:.1f} s\n    run dir: {run_dir}"
    )
    return 0 if found["stop"] != "none" else 1


def resume_capture(
    snapshot_dir: Path,
    *,
    host_override: Path | None,
    timeout: int,
    fast_verify: bool,
    seconds: float | None = None,
    isolate_run_dir: bool = False,
) -> int:
    """Resume a DMTCP image whose process is a rust_trace capture rather than tools.play."""
    snapshot_dir = snapshot_dir.resolve()
    data = manifest.load(snapshot_dir)
    isolate_run_dir = isolate_run_dir or data.get("run_dir_isolation") == ISOLATED_RUN_DIR_SCHEMA
    capture_argv = data.get("play_argv")
    if not isinstance(capture_argv, list) or not capture_argv or capture_argv[0] != "capture":
        print("snapshot: REFUSED: this is not a rust_trace capture snapshot", file=sys.stderr)
        return 2
    problems = manifest.verify(
        data,
        snapshot_dir,
        host_override=host_override,
        fast=fast_verify,
        check_display=not isolate_run_dir,
    )
    if problems:
        for problem in problems:
            print(f"snapshot: REFUSED: {problem}", file=sys.stderr)
        print(
            "snapshot: nothing was resumed; take a new snapshot for these inputs.", file=sys.stderr
        )
        return 2
    host = data["host"]
    assert isinstance(host, dict)
    run_dir = Path(str(data["run_dir"]))
    isolated_run_dir = snapshot_dir / "isolated-run-dir"
    if isolate_run_dir:
        shutil.rmtree(isolated_run_dir, ignore_errors=True)
        copied = subprocess.run(
            ["cp", "-a", "--reflink=auto", str(snapshot_dir / "run-dir"), str(isolated_run_dir)],
            check=False,
            timeout=180,
        )
        if copied.returncode != 0:
            raise RuntimeError(
                f"could not make isolated snapshot run directory: {copied.returncode}"
            )
        x11_dir = isolated_run_dir / "parallel-x11"
        x11_dir.mkdir()
        runtime_dir = ROOT / "tmp/xdg-snapshot"
        runtime_dir.mkdir(parents=True, exist_ok=True)
        isolated_runtime = isolated_run_dir / "parallel-xdg-runtime"
        if runtime_dir.is_dir():
            copied_runtime = subprocess.run(
                ["cp", "-a", "--reflink=auto", str(runtime_dir), str(isolated_runtime)],
                check=False,
                timeout=60,
            )
            if copied_runtime.returncode != 0:
                raise RuntimeError("could not isolate snapshot XDG runtime directory")
        else:
            isolated_runtime.mkdir()
    else:
        restore_run_dir(snapshot_dir, run_dir)
    output_dir = isolated_run_dir if isolate_run_dir else run_dir
    marker_paths = write_resume_marker(
        run_dir, output_dir, isolate_run_dir=isolate_run_dir, seconds=seconds
    )
    tools = dmtcp.locate()
    port = free_port()
    ckpt_dir = snapshot_dir / "ckpt"
    started = time.monotonic()
    start_coordinator(tools, port, ckpt_dir, snapshot_dir / f"coordinator-resume-{port}.log")
    images = [str(ckpt_dir / name) for name in sorted(data["images"])]  # type: ignore[arg-type]
    log = (snapshot_dir / f"resume-{port}.log").open("wb")
    restart_argv = [
        str(tools["dmtcp_restart"]),
        "--join-coordinator",
        "--coord-port",
        str(port),
        "--ckptdir",
        str(ckpt_dir),
        *images,
    ]
    argv = restart_argv
    resume_env = coordinator_env(port)
    if isolate_run_dir:
        run_dir.mkdir(parents=True, exist_ok=True)
        resume_env.update(pathvirt_resume_environment(run_dir, isolated_run_dir, isolated_runtime))
    process = subprocess.Popen(  # noqa: S603
        argv,
        cwd=ROOT,
        env=resume_env,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    timed_out = False
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        print(f"snapshot: resumed capture exceeded {timeout} s, killed", file=sys.stderr)
    finally:
        quit_computation(tools, port)
        stop_process_group(process)
        log.close()
        for marker in marker_paths:
            marker.unlink(missing_ok=True)
    if isolate_run_dir:
        keep_continued_capture(isolated_run_dir, snapshot_dir / "continued")
    seconds = time.monotonic() - started
    code = process.returncode
    print(
        f"snapshot: resumed rust_trace capture poll {data['poll']} in {seconds:.1f} s; "
        f"exit={code}; output={run_dir}"
    )
    return 124 if timed_out else (code if code is not None else 1)
