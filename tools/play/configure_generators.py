"""Cache and run the Rust-lane configure generators as one parallel phase."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from tools.wt import shared_artifacts

# Owner name for the generated cache marker written below.
CACHE_NAME = ".configure-generators.json"


def is_cache_artifact(root_relative_path: str) -> bool:
    """Identify the generator's own scratch cache without naming build directories."""
    path = Path(root_relative_path)
    return bool(path.parts) and path.parts[0] == "tmp" and path.name == CACHE_NAME


def owns_generated_build_output(root: Path, root_relative_path: str) -> bool:
    """Recognize files inside a tmp build dir carrying our cache and CMake markers."""
    path = Path(root_relative_path)
    if not path.parts or path.parts[0] != "tmp":
        return False
    if is_cache_artifact(root_relative_path):
        return True
    directory = root / path.parent
    tmp = root / "tmp"
    while directory != tmp and tmp in directory.parents:
        cache = directory / CACHE_NAME
        cmake = directory / "CMakeCache.txt"
        if all(marker.is_file() and not marker.is_symlink() for marker in (cache, cmake)):
            return True
        directory = directory.parent
    return False


# Files outside the scanned directories that a generator reads (rust_weapons: LANE and PICKUP).
EXTRA_INPUTS = (
    "crates/tsfp-ports/src/weapons_lane.rs",
    "crates/tsfp-ports/src/pickup_tick.rs",
    "crates/tsfp-ports/lanes.json",
)


def _input_files(root: Path, gen: Path, game: Path) -> list[Path]:
    files = {}
    for label, base, suffixes in (
        ("tools/play", root / "tools/play", {".py"}),
        ("tools/replace", root / "tools/replace", {".py", ".json"}),
        ("tools/data", root / "tools/data", {".json"}),
        ("game", game, {".c", ".h", ".inc"}),
        ("gen", gen, {".c", ".h"}),
    ):
        if base.exists():
            files.update(
                {
                    p: f"{label}/{p.relative_to(base).as_posix()}"
                    for p in base.rglob("*")
                    if p.is_file() and p.suffix in suffixes
                }
            )
    for name in EXTRA_INPUTS:
        path = root / name
        if path.is_file():
            files[path] = name
    return sorted(files.items(), key=lambda item: item[1])


def _input_hash(root: Path, gen: Path, game: Path) -> str:
    digest = hashlib.sha256()
    digest.update(sys.executable.encode())
    digest.update(sys.version.encode())
    for path, name in _input_files(root, gen, game):
        digest.update(name.encode())
        digest.update(b"\0")
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    return digest.hexdigest()


def _run(command: list[str], root: Path, stage: Path) -> tuple[str, str]:
    completed = subprocess.run(
        command + ["--out-dir", str(stage)], cwd=root, capture_output=True, text=True
    )
    if completed.returncode:
        raise RuntimeError(
            f"{' '.join(command[:3])} failed ({completed.returncode}):\n"
            f"{completed.stdout[-2000:]}\n{completed.stderr[-4000:]}"
        )
    return " ".join(command[2:3]), completed.stdout.strip()


def _output_map(outputs: dict[str, Path]) -> dict[str, dict[str, str]]:
    result = {}
    for tag, directory in outputs.items():
        result[tag] = {}
        if directory.is_dir():
            for path in sorted(p for p in directory.rglob("*") if p.is_file()):
                result[tag][path.relative_to(directory).as_posix()] = hashlib.sha256(
                    path.read_bytes()
                ).hexdigest()
    return result


def _cache_valid(cache: Path, fingerprint: str, outputs: dict[str, Path]) -> bool:
    try:
        record = json.loads(cache.read_text())
        return record.get("inputs") == fingerprint and record.get("outputs") == _output_map(outputs)
    except (OSError, ValueError):
        return False


def _publish(stage: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    staged = {p.relative_to(stage).as_posix(): p for p in stage.rglob("*") if p.is_file()}
    old = {p.relative_to(destination).as_posix(): p for p in destination.rglob("*") if p.is_file()}
    for relative, source in staged.items():
        target = destination / relative
        if target.is_file() and target.read_bytes() == source.read_bytes():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        temp = target.with_name(target.name + ".tmp")
        shutil.copyfile(source, temp)
        os.replace(temp, target)
    for relative, path in old.items():
        if relative not in staged:
            path.unlink()


STORE_KEEP = 6


def default_store(root: Path) -> Path:
    """T2454: content-addressed store shared by build dirs and worktrees."""
    override = os.environ.get("TSFP_CONFIGURE_GEN_STORE")
    if override:
        return Path(override)
    try:
        common = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        main = Path(common).parent
    except (OSError, subprocess.CalledProcessError):
        from tools.wt.source_identity import main_override

        main = main_override() or root  # T3760: a git-less remote exec tree
    shared = main / "tmp" / "wt-cache"
    return (shared if shared.is_dir() else main / "tmp") / "configure-generators-store"


def _store_save(store: Path, fingerprint: str, stage_dirs: dict[str, Path], summary: str) -> None:
    final = store / fingerprint
    store.mkdir(parents=True, exist_ok=True)
    if not final.exists():
        temp = Path(tempfile.mkdtemp(prefix=".tmp-", dir=store))
        try:
            for tag, directory in stage_dirs.items():
                shutil.copytree(directory, temp / tag)
            (temp / "summary.txt").write_text(summary)
            os.replace(temp, final)
        except OSError:
            shutil.rmtree(temp, ignore_errors=True)
            return
        entries = sorted(
            (e for e in store.iterdir() if e.is_dir() and not e.name.startswith(".")),
            key=lambda e: e.stat().st_mtime,
        )
        for old in entries[:-STORE_KEEP]:
            shutil.rmtree(old, ignore_errors=True)
    if not shared_artifacts.already_published("configure-generators", fingerprint):
        with tempfile.TemporaryDirectory(prefix=".configure-share-") as temp:
            archive = Path(temp) / "generators.tar.gz"
            shared_artifacts.pack_tree(final, archive)
            shared_artifacts.publish_file("configure-generators", fingerprint, archive)


def _store_restore(store: Path, fingerprint: str, outputs: dict[str, Path]) -> str | None:
    entry = store / fingerprint
    if not (all((entry / tag).is_dir() for tag in outputs) and (entry / "summary.txt").is_file()):
        with tempfile.TemporaryDirectory(prefix=".configure-fetch-") as temp:
            archive = Path(temp) / "generators.tar.gz"
            restored = shared_artifacts.fetch_file("configure-generators", fingerprint, archive)
            extracted = Path(temp) / "entry"
            if restored and shared_artifacts.unpack_tree(archive, extracted):
                if (
                    all((extracted / tag).is_dir() for tag in outputs)
                    and (extracted / "summary.txt").is_file()
                ):
                    store.mkdir(parents=True, exist_ok=True)
                    try:
                        os.replace(extracted, entry)
                    except OSError:
                        pass
    if not all((entry / tag).is_dir() for tag in outputs) or not (entry / "summary.txt").is_file():
        return None
    for tag, destination in outputs.items():
        _publish(entry / tag, destination)
    os.utime(entry)
    return (entry / "summary.txt").read_text()


def generate(
    root: Path,
    gen: Path,
    game: Path,
    outputs: dict[str, Path],
    cache_dir: Path,
    store: Path | None = None,
) -> str:
    cache_dir.mkdir(parents=True, exist_ok=True)
    cache = cache_dir / CACHE_NAME
    fingerprint = _input_hash(root, gen, game)
    if _cache_valid(cache, fingerprint, outputs):
        return f"configure generators: input cache hit {fingerprint[:12]}"
    store = store if store is not None else default_store(root)
    restored = _store_restore(store, fingerprint, outputs)
    if restored is not None:
        record = {"inputs": fingerprint, "outputs": _output_map(outputs)}
        temp = cache.with_suffix(".tmp")
        temp.write_text(json.dumps(record, sort_keys=True) + "\n")
        os.replace(temp, cache)
        return f"configure generators: restored {fingerprint[:12]} from the shared store" + (
            f" ({restored})" if restored else ""
        )

    stage_root = Path(tempfile.mkdtemp(prefix=".configure-generators-", dir=cache_dir))
    commands = [
        [
            sys.executable,
            "-m",
            "tools.play.rust_adapters",
            "--gen-dir",
            str(gen),
            "--game-dir",
            str(game),
        ],
        [
            sys.executable,
            "-m",
            "tools.play.rust_weapons",
            "--gen-dir",
            str(gen),
            "--game-dir",
            str(game),
        ],
        [
            sys.executable,
            "-m",
            "tools.play.ui_draw_hooks",
            "--gen-dir",
            str(gen),
            "--game-dir",
            str(game),
        ],
        [
            sys.executable,
            "-m",
            "tools.play.hud_gate",
            "--gen-dir",
            str(gen),
            "--game-dir",
            str(game),
        ],
    ]
    stage_dirs = {tag: stage_root / tag for tag in outputs}
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            futures = [
                pool.submit(_run, command, root, stage_dirs[tag])
                for (tag, _), command in zip(outputs.items(), commands, strict=True)
            ]
            results = [future.result() for future in futures]
        for tag, destination in outputs.items():
            _publish(stage_dirs[tag], destination)
        record = {"inputs": fingerprint, "outputs": _output_map(outputs)}
        temp = cache.with_suffix(".tmp")
        temp.write_text(json.dumps(record, sort_keys=True) + "\n")
        os.replace(temp, cache)
        summary = "; ".join(text for _, text in results if text)
        _store_save(store, fingerprint, stage_dirs, summary)
        return f"configure generators: refreshed {fingerprint[:12]}" + (
            f" ({summary})" if summary else ""
        )
    finally:
        shutil.rmtree(stage_root, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--gen-dir", type=Path, required=True)
    parser.add_argument("--game-dir", type=Path, required=True)
    parser.add_argument("--cache-dir", type=Path, required=True)
    parser.add_argument("--adapters-dir", type=Path, required=True)
    parser.add_argument("--weapons-dir", type=Path, required=True)
    parser.add_argument("--ui-draw-dir", type=Path, required=True)
    parser.add_argument("--hud-gate-dir", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    gen = args.gen_dir.resolve()
    game = args.game_dir.resolve()
    outputs = {
        "rust-adapters": args.adapters_dir.resolve(),
        "rust-weapons": args.weapons_dir.resolve(),
        "ui-draw-hooks": args.ui_draw_dir.resolve(),
        "hud-gate": args.hud_gate_dir.resolve(),
    }
    try:
        print(generate(root, gen, game, outputs, args.cache_dir.resolve()))
    except (OSError, RuntimeError, ValueError) as error:
        print(f"configure generators failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
