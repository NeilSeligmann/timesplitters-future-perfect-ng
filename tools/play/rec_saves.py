"""T2421: bind a T2348 input.rec to the save/HDD state it was recorded from.

A recording replays only from the storage it started on (profile, unlocks, Story progress).
`--record` snapshots the storage at start and stamps the header with `saves=<path>` and
`saves-sha256=<tree hash>`. Replay (`play.sh --replay`, `tools.rust_trace capture`) restores that
snapshot into a scratch copy and points `--hdd` at it.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import sys
from pathlib import Path

from tools import owner_files
from tools.datadir import output_path

ROOT = Path(__file__).resolve().parents[2]
SAVES_KEY = "saves="
HASH_KEY = "saves-sha256="


def tree_hash(directory: Path) -> str:
    """sha256 over sorted (relative path, file sha256) pairs: layout and content."""
    digest = hashlib.sha256()
    for path in sorted(p for p in directory.rglob("*") if p.is_file() and not p.is_symlink()):
        digest.update(path.relative_to(directory).as_posix().encode() + b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


def snapshot(source: Path, dest: Path) -> str:
    """Copy `source` to `dest` (cp -a semantics, dest must not exist) and return the tree hash."""
    if dest.exists():
        raise FileExistsError(dest)
    if source.is_dir():
        shutil.copytree(source, dest, symlinks=True)
    else:
        dest.mkdir(parents=True)  # no storage yet: an empty start state is still a state
    return tree_hash(dest)


def read_header(rec: Path) -> list[str]:
    with rec.open("r", encoding="utf-8") as handle:
        return handle.readline().rstrip("\r\n").split("\t")


def header_saves(rec: Path) -> tuple[str, str] | None:
    """(path, sha256) stamped into the header, or None for an unstamped recording."""
    path = digest = None
    for field in read_header(rec)[3:]:
        if field.startswith(SAVES_KEY):
            path = field[len(SAVES_KEY) :]
        elif field.startswith(HASH_KEY):
            digest = field[len(HASH_KEY) :]
    return (path, digest) if path and digest else None


def header_unlock_all(rec: Path) -> bool:
    return "unlock-all=1" in read_header(rec)[3:]


def header_host_args(rec: Path) -> list[str]:
    """Host flags for the session options the header records (ValueError: not appliable)."""
    from tools.play import rec_format

    return rec_format.session_host_args("\t".join(read_header(rec)))


def with_header_host_args(argv: list[str], rec: Path) -> list[str]:
    """`argv` plus the recorded session flags it lacks."""
    return argv + [flag for flag in header_host_args(rec) if flag not in argv]


def stamp_header(rec: Path, saves: Path, digest: str) -> None:
    """Append the saves fields to the header line (idempotent: older saves fields are replaced)."""
    try:
        shown = saves.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        shown = saves.resolve().as_posix()
    text = rec.read_bytes().decode("utf-8")
    header, sep, rest = text.partition("\n")
    fields = [f for f in header.rstrip("\r").split("\t") if not f.startswith((SAVES_KEY, HASH_KEY))]
    fields += [SAVES_KEY + shown, HASH_KEY + digest]
    tmp = rec.with_name(rec.name + ".tmp")
    tmp.write_bytes(("\t".join(fields) + sep + rest).encode("utf-8"))
    os.replace(tmp, rec)


def locate(rec: Path, stamped: str) -> Path:
    """Resolve stamped paths, including tmp paths shared by linked worktrees."""
    candidate = Path(stamped)
    extra = (
        candidate,
        output_path(candidate),
        ROOT / candidate,
        rec.parent / candidate,
        rec.parent / candidate.name,
        rec.parent / f"{rec.stem}.saves",  # T3740: the adopted snapshot beside the recording
    )
    resolved = owner_files.resolve_path(rec.parent, candidate, extra=extra, kind="dir")
    if resolved.is_dir():
        return resolved
    raise FileNotFoundError(f"{rec}: saves snapshot {stamped} not found at {resolved}")


def restore(rec: Path, scratch: Path) -> Path | None:
    """Copy the verified snapshot into `scratch`; None when unstamped."""
    stamped = header_saves(rec)
    if stamped is None:
        return None
    source = locate(rec, stamped[0])
    found = tree_hash(source)
    if found != stamped[1]:
        raise ValueError(f"{rec}: snapshot {source} hash {found} != header {stamped[1]}")
    if scratch.exists():
        shutil.rmtree(scratch)
    shutil.copytree(source, scratch, symlinks=True)
    return scratch


def set_hdd(command: list[str], hdd: Path) -> list[str]:
    result = list(command)
    result[result.index("--hdd") + 1] = str(hdd)
    return result


def option_value(command: list[str], flag: str) -> str | None:
    return command[command.index(flag) + 1] if flag in command[:-1] else None


def measure_route_marks(rec: Path) -> int:
    """Measure replay polls for owner F12 marks after binding a recording's saves."""
    from tools import gate, gate_marks

    user_marks = [mark for mark in gate.route_marks(rec) if mark[2] != "record-start"]
    if not user_marks:
        print("mark_polls=not-needed (recording has no user F12 marks)")
        return 0

    measured_rec, marks = gate_marks.measure(str(rec.resolve()), "weapons", 600, None)
    measured_users = [mark for mark in marks if mark.get("label") != "record-start"]
    if len(measured_users) < len(user_marks):
        raise RuntimeError(
            f"{rec.name} has {len(user_marks)} user F12 mark(s), but replay reached "
            f"{len(measured_users)}; check that the route reaches its recorded endpoint"
        )
    gate_marks.store(measured_rec, marks)
    print(f"mark_polls=measured ({len(measured_users)} user F12 mark(s))")
    return len(measured_users)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="inspect or adopt the save snapshot of an input.rec"
    )
    sub = parser.add_subparsers(dest="cmd", required=True)
    show = sub.add_parser("show", help="print the stamped snapshot and whether it verifies")
    show.add_argument("rec", type=Path)
    adopt = sub.add_parser(
        "adopt", help="snapshot a save dir beside a recording and stamp its header"
    )
    adopt.add_argument("rec", type=Path)
    adopt.add_argument("saves", type=Path, help="storage dir the recording started from (copied)")
    args = parser.parse_args(argv)
    if args.cmd == "show":
        stamped = header_saves(args.rec)
        if stamped is None:
            print("unstamped")
            return 1
        found = tree_hash(locate(args.rec, stamped[0]))
        print(f"saves={stamped[0]} sha256={stamped[1]} verified={found == stamped[1]}")
        return 0 if found == stamped[1] else 1
    dest = args.rec.with_name(args.rec.stem + ".saves")
    digest = snapshot(args.saves, dest)
    stamp_header(args.rec, dest, digest)
    print(f"{dest} sha256={digest}")
    measure_route_marks(args.rec)
    return 0


if __name__ == "__main__":
    sys.exit(main())
