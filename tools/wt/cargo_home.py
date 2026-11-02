"""T3472: one CARGO_HOME for every crate, including the Bevy and wgpu graphs.

Before T3472 the Bevy/wgpu crates resolved offline from `tmp/spikes-cargo-home` and
everything else from `tmp/rust/cargo`, so agents had to set CARGO_HOME (or
TSFP_BEVY_CARGO_HOME) by hand per crate. `merge` folds a legacy home into the unified one
(union of downloaded `.crate` files, unpacked sources and sparse-index cache entries; never
overwrites a crate, keeps the larger index entry, which is the newer superset because the
crates.io index is append-only). `bevy_home` is the single resolver every script uses.

    python3 -m tools.wt.cargo_home merge [--apply]   # dry-run by default
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

UNIFIED_REL = Path("tmp/rust/cargo")
LEGACY_REL = Path("tmp/spikes-cargo-home")
MARKER = "registry/.tsfp-t3472-merged.json"
SKIP_INDEX = {".package-cache", ".package-cache-mutate", ".last-updated"}


def unified(main: Path) -> Path:
    return main / UNIFIED_REL


def merged(home: Path) -> bool:
    return (home / MARKER).is_file()


def bevy_home(main: Path, env: dict[str, str] | None = None) -> Path:
    """The cargo home for the Bevy/wgpu crates: explicit env, else the unified home once the
    legacy home was merged into it (or no legacy home exists), else the legacy home."""
    env = os.environ if env is None else env
    if env.get("TSFP_BEVY_CARGO_HOME"):
        return Path(env["TSFP_BEVY_CARGO_HOME"])
    home = unified(main)
    legacy = main / LEGACY_REL
    if merged(home) or not (legacy / "registry").is_dir():
        return home
    return legacy


def _copy(source: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    temp = dest.with_name(f".{dest.name}.t3472tmp")
    if source.is_dir():
        if temp.exists():
            shutil.rmtree(temp)
        shutil.copytree(source, temp, symlinks=True)
    else:
        shutil.copy2(source, temp)
    os.replace(temp, dest)


def merge(source: Path, dest: Path, apply: bool) -> dict[str, int]:
    """Union `source/registry` into `dest/registry`; return counters."""
    stats = {"crates": 0, "sources": 0, "index_new": 0, "index_grown": 0, "kept": 0}
    reg_src, reg_dst = source / "registry", dest / "registry"
    if not reg_src.is_dir():
        return stats
    for kind in ("cache", "src"):
        for registry in sorted((reg_src / kind).glob("*")):
            for entry in sorted(registry.iterdir()):
                target = reg_dst / kind / registry.name / entry.name
                if target.exists():
                    stats["kept"] += 1
                    continue
                if kind == "src" and not (entry / ".cargo-ok").is_file():
                    continue  # a half-unpacked source; cargo would redo it anyway
                stats["crates" if kind == "cache" else "sources"] += 1
                if apply:
                    _copy(entry, target)
    for registry in sorted((reg_src / "index").glob("*")):
        for path in sorted(registry.rglob("*")):
            if not path.is_file() or path.name in SKIP_INDEX:
                continue
            rel = path.relative_to(reg_src)
            target = reg_dst / rel
            if not target.exists():
                stats["index_new"] += 1
            elif path.stat().st_size > target.stat().st_size:
                stats["index_grown"] += 1
            else:
                continue
            if apply:
                _copy(path, target)
    if apply:
        marker = dest / MARKER
        marker.parent.mkdir(parents=True, exist_ok=True)
        marker.write_text(json.dumps({"source": str(source), **stats}, sort_keys=True) + "\n")
    return stats


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="tools.wt.cargo_home", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    item = sub.add_parser("merge", help="fold the legacy Bevy cargo home into the unified one")
    item.add_argument("--main", type=Path, default=None, help="main checkout (default: cwd's)")
    item.add_argument("--apply", action="store_true")
    sub.add_parser("which", help="print the unified and the Bevy cargo home")
    args = parser.parse_args(argv)
    from tools.wt import core

    main_dir = getattr(args, "main", None) or core.main_root(core.worktree_root(Path.cwd()))
    if args.command == "which":
        print(f"unified {unified(main_dir)}\nbevy {bevy_home(main_dir)}")
        return 0
    stats = merge(main_dir / LEGACY_REL, unified(main_dir), args.apply)
    print(("merged " if args.apply else "would merge ") + json.dumps(stats, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
