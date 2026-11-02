"""Persistent derived data shared by linked worktrees (T2251).

TSFP_DATA_DIR replaces the main checkout's tmp directory. Resolution is pure:
callers create their own directories. Explicit non-tmp destinations stay unchanged.
"""

import os
import subprocess
from pathlib import Path


def data_dir(name: str | Path) -> Path:
    """Return a derived-data path under the main checkout tmp or the override."""
    name = Path(name)
    if name.is_absolute() or ".." in name.parts:
        raise ValueError("data name must be relative and cannot contain ..")
    override = os.environ.get("TSFP_DATA_DIR")
    if override:
        base = Path(override).expanduser().resolve()
    else:
        from tools.wt.source_identity import main_override

        # T3760: a remote exec tree names its data root; never walk up into a host repo.
        main = main_override() if os.environ.get("TSFP_SOURCE_TREE") else None
        if main is not None:
            return main.resolve() / "tmp" / name
        common = _common_dir_from_files(Path.cwd())
        if common is None:
            done = subprocess.run(
                ["git", "rev-parse", "--git-common-dir"],
                check=False,
                capture_output=True,
                text=True,
                timeout=30,
            )
            main = main_override()  # T3760: a git-less remote exec tree
            if done.returncode and main is not None:
                return main.resolve() / "tmp" / name
            done.check_returncode()
            common = Path(done.stdout.strip())
        base = common.resolve().parent / "tmp"
    return base / name


def _common_dir_from_files(start: Path) -> Path | None:
    """`git rev-parse --git-common-dir` from the .git files, no subprocess (T3752: tools that
    resolve paths must not depend on a real `git` run, which tests faking subprocess break).
    None when GIT_DIR is set or the layout is unusual; the caller then asks git."""
    if os.environ.get("GIT_DIR") or os.environ.get("GIT_COMMON_DIR"):
        return None
    for directory in (start, *start.parents):
        dot_git = directory / ".git"
        if dot_git.is_dir():
            return dot_git
        if dot_git.is_file():
            text = dot_git.read_text().strip()
            if not text.startswith("gitdir:"):
                return None
            gitdir = (directory / text.removeprefix("gitdir:").strip()).resolve()
            commondir = gitdir / "commondir"
            if commondir.is_file():
                return (gitdir / commondir.read_text().strip()).resolve()
            return gitdir
    return None


def main_root(start: Path | None = None) -> Path:
    """The main checkout's top directory (T3755: instead of a literal `/workspace`).

    Resolved from the `.git` files of `start` (default: the checkout holding this module), so a
    linked worktree maps to its main checkout and no subprocess runs; git is the fallback."""
    start = (start or Path(__file__).resolve().parents[1]).resolve()
    common = None if os.environ.get("GIT_DIR") else _common_dir_from_files(start)
    if common is None:
        common = Path(
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(start),
                    "rev-parse",
                    "--path-format=absolute",
                    "--git-common-dir",
                ],
                check=True,
                capture_output=True,
                text=True,
                timeout=30,
            ).stdout.strip()
        )
    return common.resolve().parent


def output_path(path: str | Path) -> Path:
    """Route relative tmp/... destinations through data_dir; preserve custom paths."""
    path = Path(path)
    if not path.is_absolute() and path.parts and path.parts[0] == "tmp":
        return data_dir(Path(*path.parts[1:]))
    return path


def input_hash(path: Path) -> str:
    """Stream file hashes; directory manifests include names, never follow symlinks."""
    import hashlib

    digest = hashlib.sha256()
    if path.is_symlink():
        raise ValueError(f"symlink input is not reproducible: {path}")
    if path.is_dir():
        for child in sorted(path.rglob("*")):
            if child.is_symlink():
                raise ValueError(f"symlink input is not reproducible: {child}")
            if child.is_file():
                digest.update(str(child.relative_to(path)).encode())
                digest.update(b"\0")
                digest.update(input_hash(child).encode())
    else:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def hash_inputs(inputs: list[Path], *, cwd: Path | None = None) -> dict[str, str]:
    """Content hashes of recipe inputs, keyed by resolved path."""
    cwd = (cwd or Path.cwd()).resolve()
    hashes = {}
    for item in inputs:
        path = (cwd / item).resolve()
        hashes[str(path)] = input_hash(path)
    return hashes


def write_recipe(
    artifact: str | Path,
    *,
    command: list[str],
    inputs: list[Path],
    wall_seconds: float,
    host_lock: bool = False,
    cwd: Path | None = None,
    input_hashes: dict[str, str] | None = None,
) -> Path:
    """Atomically record regeneration metadata after successful production.

    `input_hashes` (from `hash_inputs` before the run) skips re-hashing: a DMTCP-restored
    capture must not read multi-GB inputs after restart (T2479, MEASURED segfault).

    Commands are argv, never shell-evaluated. Callers must exclude credentials.
    Explicit outputs also receive recipes; no artifact content is copied.
    """
    import json
    import tempfile

    cwd = (cwd or Path.cwd()).resolve()
    artifact = (cwd / artifact).absolute()
    if not artifact.exists():
        raise FileNotFoundError(artifact)
    if wall_seconds < 0 or not command:
        raise ValueError("recipe needs a command and nonnegative wall seconds")
    # A checkpointed capture cannot reliably start a fresh Git subprocess after
    # DMTCP has restored the process tree. Snapshot launchers pin this metadata
    # before checkpointing; ordinary callers still resolve the live checkout.
    sha = os.environ.get("TSFP_RECIPE_GIT_HEAD")
    if not sha:
        from tools.wt import source_identity

        # T3782: an exec tree on a remote host has no .git; its commit identity is data.
        identity = source_identity.load(cwd)
        sha = identity.commit if identity is not None else ""
    if not sha:
        sha = subprocess.run(
            ["git", "-C", str(cwd), "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout.strip()
    hashes = input_hashes if input_hashes is not None else hash_inputs(inputs, cwd=cwd)
    record = {
        "version": 1,
        "artifact": str(artifact),
        "command": command,
        "cwd": str(cwd),
        "git_sha": sha,
        "input_hashes": hashes,
        "wall_seconds": wall_seconds,
        "host_lock": host_lock,
    }
    sidecar = Path(str(artifact) + ".recipe.json")
    with tempfile.NamedTemporaryFile(mode="w", dir=sidecar.parent, delete=False) as stream:
        temporary = Path(stream.name)
        try:
            stream.write(json.dumps(record, indent=2) + "\n")
            stream.flush()
            os.fsync(stream.fileno())
        except BaseException:
            temporary.unlink()
            raise
    temporary.replace(sidecar)
    return sidecar


def recipe_inventory(root: Path) -> list[dict]:
    """Dry check sidecars without executing recipes or following directory links.

    Matching inputs are necessary, not proof of deterministic reproduction.
    """
    import json
    import time

    rows = []
    for directory, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = sorted(d for d in dirs if not (Path(directory) / d).is_symlink())
        for name in sorted(files):
            if not name.endswith(".recipe.json"):
                continue
            sidecar = Path(directory) / name
            artifact = Path(str(sidecar)[: -len(".recipe.json")])
            row = {"artifact": str(artifact), "recipe": str(sidecar), "status": "invalid-recipe"}
            try:
                if sidecar.is_symlink():
                    raise ValueError("symlink recipe")
                record = json.loads(sidecar.read_text())
                if (
                    record["version"] != 1
                    or not record["command"]
                    or not isinstance(record["input_hashes"], dict)
                ):
                    raise ValueError("unsupported recipe")
                row.update(
                    wall_seconds=record["wall_seconds"],
                    host_lock=record["host_lock"],
                    git_sha=record["git_sha"],
                )
                row["status"] = "inputs-match"
                for path, expected in record["input_hashes"].items():
                    try:
                        if input_hash(Path(path)) != expected:
                            row["status"] = "inputs-changed"
                    except (OSError, ValueError):
                        row["status"] = "inputs-missing"
                        break
                if not artifact.exists() or artifact.is_symlink():
                    row["status"] = "artifact-missing"
                else:
                    row["age_seconds"] = max(0, time.time() - artifact.stat().st_mtime)
                    row["size_bytes"] = (
                        artifact.stat().st_size
                        if artifact.is_file()
                        else sum(
                            (Path(base) / file).lstat().st_size
                            for base, children, names in os.walk(artifact, followlinks=False)
                            for file in names
                        )
                    )
            except (OSError, ValueError, KeyError, TypeError):
                row["status"] = "invalid-recipe"
            rows.append(row)
    return rows
