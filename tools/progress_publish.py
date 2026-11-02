# SPDX-License-Identifier: GPL-3.0-or-later
"""Deterministic progress publisher (T3736): heatmap and badges onto the `progress` branch.

    python3 -m tools.progress_publish request [--sha SHA]   # cheap, never blocks; spawns `run`
    python3 -m tools.progress_publish run [--once]          # single flight, latest sha wins
    python3 -m tools.progress_publish status                # print the last-result.json status file

No AI is involved: one publish renders tools.port_heatmap and tools.ci.update_badges from a scratch
worktree at the requested origin/main sha and appends ONE commit to the orphan branch `progress`
(never main). Design and rationale: docs/t3736-progress-publisher.md.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Iterator
from pathlib import Path

from tools import progress_history
from tools.port_heatmap import IMAGE_NAMES, ROWS_NAME

BRANCH = "progress"
STATE_REL = "tmp/progress-publish"
IMAGES = tuple(f"docs/img/{name}" for name in IMAGE_NAMES)
BADGE_DIR = "docs/badges"
ROWS_FILE = "progress-rows.json"
BADGE_INTERVAL_MIN = 0.0  # re-measure badges on every publish (about 20 s); raise to throttle
RENDER_TIMEOUT_S = 900
GIT_TIMEOUT_S = 120
HEATMAP_EXTRAS = ("--with", "matplotlib", "--with", "squarify")

Files = dict[str, bytes]
Renderer = Callable[[Path, str, Files, bool, str], Files]


class PublishError(Exception):
    """One publish failed; the message lands in last-result.json."""


def state_dir(root: Path) -> Path:
    path = root / STATE_REL
    path.mkdir(parents=True, exist_ok=True)
    return path


def read_text(path: Path) -> str:
    try:
        return path.read_text().strip()
    except OSError:
        return ""


def write_atomic(path: Path, text: str) -> None:
    pending = path.with_name(path.name + ".new")
    pending.write_text(text)
    pending.replace(path)


def read_result(root: Path) -> dict:
    try:
        return json.loads(read_text(root / STATE_REL / "last-result.json"))
    except ValueError:
        return {}


def write_result(
    root: Path, clock: Callable[[], float], exit_code: int, sha: str, detail: str, **extra: object
) -> None:
    """Status file read by the port-heatmap-stale HEALTH detector."""
    previous = read_result(root)
    data = {**previous, "exit": exit_code, "sha": sha, "at": int(clock()), "detail": detail[:300]}
    if exit_code == 0:
        data["ok_sha"], data["ok_at"] = sha, int(clock())
    data.update(extra)
    write_atomic(state_dir(root) / "last-result.json", json.dumps(data, indent=2) + "\n")


def git(root: Path, *args: str, stdin: bytes | None = None, timeout: int = GIT_TIMEOUT_S) -> bytes:
    done = subprocess.run(  # noqa: S603 -- fixed argv, no shell
        ["git", "-C", str(root), *args],
        input=stdin,
        capture_output=True,
        timeout=timeout,
        check=False,
    )
    if done.returncode:
        raise PublishError(f"git {args[0]} failed: {done.stderr.decode(errors='replace')[-300:]}")
    return done.stdout


def killed(root: Path) -> bool:
    return (root / STATE_REL / "off").exists()


@contextlib.contextmanager
def flock(path: Path, blocking: bool = False) -> Iterator[bool]:
    """Yield True when the lock is held by this process, False when another process holds it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | (0 if blocking else fcntl.LOCK_NB))
        except BlockingIOError:
            yield False
            return
        try:
            yield True
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)


def run_python(root: Path) -> str:
    candidate = root / ".venv/bin/python"
    return str(candidate) if candidate.exists() else sys.executable


def spawn_run(root: Path) -> None:
    """Detached `run`; its output goes to tmp/progress-publish/run.log."""
    log = (state_dir(root) / "run.log").open("ab")
    subprocess.Popen(  # noqa: S603 -- fixed argv, no shell
        [run_python(root), "-m", "tools.progress_publish", "--root", str(root), "run"],
        cwd=root,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=log,
        start_new_session=True,
    )


def origin_main_sha(root: Path) -> str:
    return git(root, "rev-parse", "--verify", "refs/remotes/origin/main").decode().strip()


def request(
    root: Path,
    sha: str | None = None,
    spawn: Callable[[Path], None] = spawn_run,
) -> str:
    """Record the newest wanted sha and start `run` unless one is already in flight."""
    if killed(root):
        return "off: kill switch tmp/progress-publish/off is present"
    wanted = sha or origin_main_sha(root)
    write_atomic(state_dir(root) / "requested", wanted + "\n")
    with flock(root / STATE_REL / "run.lock") as free:
        if not free:
            return f"requested {wanted[:12]}; a run is in flight and will pick it up"
    spawn(root)
    return f"requested {wanted[:12]}; started run"


# ---------------------------------------------------------------------------------------
# git plumbing: one commit on top of the previous progress tip, no checkout


def fetch_tip(root: Path, remote: str = "origin") -> str:
    """The remote `progress` tip ('' when the branch does not exist yet)."""
    listed = git(root, "ls-remote", remote, f"refs/heads/{BRANCH}", timeout=60).decode().split()
    if not listed:
        return ""
    git(root, "fetch", "-q", remote, f"+refs/heads/{BRANCH}:refs/remotes/{remote}/{BRANCH}")
    return listed[0]


def build_tree(root: Path, files: Files) -> str:
    """Nested trees from {path: bytes} via hash-object and mktree."""
    nested: dict = {}
    for path, data in files.items():
        node = nested
        *dirs, name = path.split("/")
        for part in dirs:
            node = node.setdefault(part, {})
        node[name] = data

    def write(node: dict) -> str:
        lines = []
        for name, value in sorted(node.items()):
            if isinstance(value, dict):
                lines.append(f"040000 tree {write(value)}\t{name}")
            else:
                blob = git(root, "hash-object", "-w", "--stdin", stdin=value).decode().strip()
                lines.append(f"100644 blob {blob}\t{name}")
        return git(root, "mktree", stdin=("\n".join(lines) + "\n").encode()).decode().strip()

    return write(nested)


def commit_files(
    root: Path, files: Files, source_sha: str, tip: str, remote: str = "origin"
) -> str:
    """Commit `files` on top of `tip` and push it as a fast-forward. Returns 'skip' or the sha."""
    tree = build_tree(root, files)
    if tip and git(root, "rev-parse", f"{tip}^{{tree}}").decode().strip() == tree:
        return "skip"
    message = (
        f"progress: heatmap and badges for main {source_sha[:12]}\n\n"
        f"Source: origin/main {source_sha}\n"
        "Generated by python3 -m tools.progress_publish (T3736), deterministic, no AI.\n"
    )
    parents = ["-p", tip] if tip else []
    identity = {
        "GIT_AUTHOR_NAME": "progress-publisher",
        "GIT_AUTHOR_EMAIL": "progress-publisher@users.noreply.local",
        "GIT_COMMITTER_NAME": "progress-publisher",
        "GIT_COMMITTER_EMAIL": "progress-publisher@users.noreply.local",
    }
    done = subprocess.run(  # noqa: S603 -- fixed argv, no shell
        ["git", "-C", str(root), "commit-tree", tree, *parents, "-m", message],
        capture_output=True,
        text=True,
        timeout=GIT_TIMEOUT_S,
        env={**os.environ, **identity},
        check=False,
    )
    if done.returncode:
        raise PublishError(f"commit-tree failed: {done.stderr[-300:]}")
    commit = done.stdout.strip()
    # Never force: a rejected push means the tip moved, the next run rebuilds on top of it.
    git(root, "push", "-q", remote, f"{commit}:refs/heads/{BRANCH}", timeout=300)
    return commit


# ---------------------------------------------------------------------------------------
# rendering from a scratch worktree


def tip_files(root: Path, tip: str) -> Files:
    """Every file of the previous progress tip (empty before the first publish)."""
    if not tip:
        return {}
    names = git(root, "ls-tree", "-r", "-z", "--name-only", tip).decode().split("\0")
    return {n: git(root, "show", f"{tip}:{n}") for n in names if n}


def heatmap_command(root: Path, out_dir: Path, extras: bool) -> list[str]:
    """uv run with the heatmap extras (cached, offline capable); else render_python."""
    module = ["python", "-m", "tools.port_heatmap", "--write", "--out-dir", str(out_dir)]
    if extras:
        return ["uv", "run", "--offline", "--no-project", *HEATMAP_EXTRAS, *module]
    from tools import port_heatmap

    return [port_heatmap.render_python(root), *module[1:]]


def run_checked(command: list[str], cwd: Path, what: str, timeout: int = RENDER_TIMEOUT_S) -> str:
    env = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}
    try:
        done = subprocess.run(  # noqa: S603 -- fixed argv, no shell
            command, cwd=cwd, capture_output=True, text=True, timeout=timeout, env=env, check=False
        )
    except (OSError, subprocess.SubprocessError) as exc:
        raise PublishError(f"{what}: {exc}") from exc
    if done.returncode:
        tail = (done.stderr or done.stdout).strip().splitlines()[-3:]
        raise PublishError(f"{what} exit {done.returncode}: {' | '.join(tail)}")
    return done.stdout


def render_heatmap(candidate: Path, work: Path) -> Files:
    out_dir = Path(tempfile.mkdtemp(prefix="heatmap-", dir=work))
    last = ""
    for extras in (True, False):
        try:
            run_checked(heatmap_command(candidate, out_dir, extras), candidate, "port_heatmap")
            break
        except PublishError as exc:
            last = str(exc)
    else:
        raise PublishError(last)
    from tools.port_heatmap_platform import DOC

    return {
        **{rel: (out_dir / Path(rel).name).read_bytes() for rel in IMAGES},
        DOC: (candidate / DOC).read_bytes(),
        ROWS_FILE: (out_dir / ROWS_NAME).read_bytes(),
    }


def add_history(files: Files, previous: Files, sha: str, clock: Callable[[], float]) -> None:
    """Replace the rows snapshot with the progress changelog files (state + history)."""
    rows = json.loads(files.pop(ROWS_FILE))
    files[progress_history.STATE_PATH], files[progress_history.HISTORY_PATH] = (
        progress_history.append(
            previous.get(progress_history.STATE_PATH),
            previous.get(progress_history.HISTORY_PATH),
            rows,
            sha,
            int(clock()),
        )
    )


def render_badges(candidate: Path, previous: Files) -> Files:
    """tools.ci.update_badges over the previous progress badges, guarded by the T1462 check."""
    badges = candidate / BADGE_DIR
    badges.mkdir(parents=True, exist_ok=True)
    for name, data in previous.items():
        if name.startswith(BADGE_DIR + "/"):
            (candidate / name).write_bytes(data)
    cmake = candidate / "build/cmake"
    python = run_python(candidate)
    if (candidate / "CMakeLists.txt").exists():
        # Configure only, like .forgejo/workflows/badges.yml: enough for `ctest -N` (test badge).
        with contextlib.suppress(PublishError):
            run_checked(
                ["cmake", "-S", ".", "-B", str(cmake), "-DCMAKE_C_COMPILER=clang"],
                candidate,
                "cmake configure",
                300,
            )
    run_checked(
        [python, "-m", "tools.ci.update_badges", "--cmake-build", str(cmake)],
        candidate,
        "update_badges",
    )
    return {
        str(path.relative_to(candidate)): path.read_bytes()
        for path in sorted(badges.iterdir())
        if path.is_file()
    }


def link_inputs(candidate: Path, main: Path) -> None:
    """Symlink the gitignored generated inputs from the main checkout (no copies)."""
    from tools.wt import core

    for spec, target, source in core.plan(candidate, main, None):
        if source is not None and spec.kind != core.GLOB_DIRS:
            if spec.kind == core.ENTRIES:
                core.entries_link(source, target, False)
            else:
                core.link_one(source, target, False)


def render_files(root: Path, sha: str, previous: Files, with_badges: bool, tip: str = "") -> Files:
    """Everything one publish contains, rendered in an owned scratch tree at `sha`."""
    from tools.wt import scratch

    work = Path(tempfile.mkdtemp(prefix="work-", dir=state_dir(root)))
    try:
        with scratch.owned(root, "preflight", sha) as candidate:
            link_inputs(candidate, root)
            scratch.seal(candidate)
            try:
                files = dict(render_heatmap(candidate, work))
                add_history(files, previous, sha, time.time)
                if with_badges:
                    files.update(render_badges(candidate, previous))
                    guard = [run_python(candidate), "-m", "tools.ci.check_badges_not_downgraded"]
                    guard += ["--new", "worktree", "--base", tip or sha]
                    run_checked(guard, candidate, "T1462 badge downgrade guard", 120)
                else:
                    files.update({n: d for n, d in previous.items() if n.startswith(BADGE_DIR)})
            finally:  # leave the scratch tree clean so it can be removed
                for cmd in (("checkout", "-q", "--", "."), ("clean", "-fdq", "--", "docs")):
                    subprocess.run(  # noqa: S603 -- fixed argv, no shell
                        ["git", "-C", str(candidate), *cmd], check=False, timeout=60
                    )
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return files


# ---------------------------------------------------------------------------------------
# the run loop


def publish_one(
    root: Path,
    sha: str,
    clock: Callable[[], float],
    render: Renderer = render_files,
    badge_interval_min: float = BADGE_INTERVAL_MIN,
    remote: str = "origin",
) -> str:
    """One publish for `sha`. Returns the commit sha or 'skip'. Raises PublishError."""
    if not git(root, "cat-file", "-t", sha, timeout=30).strip():
        raise PublishError(f"unknown sha {sha}")
    tip = fetch_tip(root, remote)
    previous = tip_files(root, tip)
    last = read_result(root).get("badges_at", 0)
    with_badges = not previous or clock() - last >= badge_interval_min * 60
    files = render(root, sha, previous, with_badges, tip)
    if not files:
        raise PublishError("renderer produced no files")
    result = commit_files(root, files, sha, tip, remote)
    if with_badges:
        data = read_result(root)
        data["badges_at"] = int(clock())
        write_atomic(state_dir(root) / "last-result.json", json.dumps(data, indent=2) + "\n")
    return result


def run(
    root: Path,
    clock: Callable[[], float] = time.time,
    publish: Callable[[Path, str], str] | None = None,
    once: bool = False,
) -> int:
    """Hold the single-flight lock and publish the newest requested sha until nothing is newer."""

    def default_publish(path: Path, sha: str) -> str:
        return publish_one(path, sha, clock)

    publish = publish or default_publish
    attempted = ""
    while True:
        with flock(root / STATE_REL / "run.lock") as held:
            if not held:
                print("progress_publish: another run holds the lock")
                return 0
            while not killed(root):
                wanted = read_text(root / STATE_REL / "requested")
                last = read_result(root)
                already = last.get("sha") == wanted and last.get("exit") == 0
                if not wanted or wanted == attempted or (not attempted and already):
                    attempted = wanted or attempted
                    break
                attempted = wanted
                try:
                    detail = publish(root, wanted)
                    write_result(root, clock, 0, wanted, f"ok: {detail}")
                    print(f"progress_publish: {wanted[:12]} -> {detail}")
                except Exception as exc:  # noqa: BLE001 - the loop must report, not die
                    write_result(root, clock, 1, wanted, f"failed: {exc}")
                    print(f"progress_publish: {wanted[:12]} failed: {exc}", file=sys.stderr)
        # Lock released: a request that raced the end of the loop saw it held and did not spawn.
        wanted = read_text(root / STATE_REL / "requested")
        if once or killed(root) or not wanted or wanted == attempted:
            return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--root", default=".", help="main checkout (relative ok)")
    sub = parser.add_subparsers(dest="command", required=True)
    req = sub.add_parser(
        "request", help="record the wanted sha and start a run if none is in flight"
    )
    req.add_argument("--sha", help="default: refs/remotes/origin/main")
    req.add_argument("--no-spawn", action="store_true", help="only record the request")
    runner = sub.add_parser("run", help="single-flight publisher; exits 0 when nothing is new")
    runner.add_argument("--once", action="store_true", help="one pass, no re-check after the lock")
    sub.add_parser("status", help="print the last result")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()
    if args.command == "request":
        try:
            print(request(root, args.sha, (lambda _root: None) if args.no_spawn else spawn_run))
        except (PublishError, OSError, subprocess.SubprocessError) as exc:
            print(f"progress_publish: request failed: {exc}", file=sys.stderr)
            return 1
        return 0
    if args.command == "status":
        print(json.dumps(read_result(root), indent=2))
        return 0
    return run(root, once=args.once)


if __name__ == "__main__":
    sys.exit(main())
