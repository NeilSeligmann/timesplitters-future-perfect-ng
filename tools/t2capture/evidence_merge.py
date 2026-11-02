# SPDX-License-Identifier: GPL-3.0-or-later
"""Git merge driver for docs/data/tier2-capture-evidence.json (T3784).

    python3 -m tools.t2capture.evidence_merge %O %A %B     # driver: base, ours (rewritten), theirs
    python3 -m tools.t2capture.evidence_merge --register   # write the shared git config entry

The evidence file is one JSON object keyed by function VA. Concurrent promoters (the
t2-continuous service, compare-version lanes, manual promotes) each change a few keys, which a
line merge reports as a conflict. This driver merges three-way by key, so two promotes that
touched different VAs never conflict:
- a key only one side changed (added, replaced or removed) takes that change;
- a key both sides changed to different entries takes the newer `created` stamp (theirs on a
  tie); a key one side removed (a demotion) and the other replaced stays removed, since a
  missing entry never overstates a tier (rule 10) and the next re-check restores it;
- a key where one side holds a new recording (`capture`) entry and the merge would pick a
  harness-labelled entry (T3793) takes the recording entry: recording credit always wins;
- other top-level fields: ours when only ours changed them, else theirs.
The output is the promoter's own form (indent 2, sorted keys). An unreadable side exits 1 so
git reports a normal conflict (never a silent pick). merge_task passes the driver with `-c` on
every landing (like rea-index); `--register` covers plain git merges in any worktree.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path

DRIVER_NAME = "t2-evidence"
DRIVER_COMMAND = "python3 -m tools.t2capture.evidence_merge %O %A %B"
PATH = "docs/data/tier2-capture-evidence.json"
_MISSING = object()


def _created(entry: object) -> str:
    return str(entry.get("created", "")) if isinstance(entry, dict) else ""


def merge_key(base: object, ours: object, theirs: object) -> object:
    """One key; `_MISSING` means absent on that side."""
    if ours == theirs:
        return ours
    if ours == base:
        return theirs
    if theirs == base:
        return ours
    if ours is _MISSING or theirs is _MISSING:
        return _MISSING  # removed on one side, replaced on the other: keep the demotion
    return ours if _created(ours) > _created(theirs) else theirs


def _source(entry: object) -> object:
    return entry.get("source") if isinstance(entry, dict) else None


def merge_function(base: object, ours: object, theirs: object) -> object:
    """`merge_key` plus T3793 precedence: a harness entry never replaces a recording one."""
    value = merge_key(base, ours, theirs)
    if value is not _MISSING and _source(value) != "capture":
        for side in (ours, theirs):
            if _source(side) == "capture" and side != base:
                return side
    return value


def merge3(base: dict | None, ours: dict, theirs: dict) -> dict:
    base = base or {}
    base_functions = base.get("functions") or {}
    our_functions = ours.get("functions") or {}
    their_functions = theirs.get("functions") or {}
    functions: dict = {}
    for key in sorted(set(base_functions) | set(our_functions) | set(their_functions)):
        value = merge_function(
            base_functions.get(key, _MISSING),
            our_functions.get(key, _MISSING),
            their_functions.get(key, _MISSING),
        )
        if value is not _MISSING:
            functions[key] = value
    merged = {}
    for key in sorted((set(ours) | set(theirs)) - {"functions"}):
        value = merge_key(
            base.get(key, _MISSING), ours.get(key, _MISSING), theirs.get(key, _MISSING)
        )
        if value is not _MISSING:
            merged[key] = value
    merged["functions"] = functions
    return merged


def dump(document: dict) -> str:
    return json.dumps(document, indent=2, sort_keys=True) + "\n"


def _load(path: Path) -> dict | None:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return None
    if not text.strip():
        return None
    document = json.loads(text)
    return document if isinstance(document, dict) else None


Run = Callable[..., subprocess.CompletedProcess]
CONFIG_ATTEMPTS = 20


def register(
    cwd: Path | None = None,
    run: Run = subprocess.run,
    sleep: Callable[[float], None] = time.sleep,
) -> None:
    """Write the driver into the repository config shared by all worktrees, only when it
    differs (I-T3784-6: an unconditional write on every bootstrap raced concurrent landings
    on .git/config.lock). A lock collision (git exit 255) is retried with backoff."""
    for key, value in (
        (f"merge.{DRIVER_NAME}.name", "Tier 2 capture evidence per-VA merge"),
        (f"merge.{DRIVER_NAME}.driver", DRIVER_COMMAND),
    ):
        current = run(
            ["git", "config", "--get", key], cwd=cwd, capture_output=True, text=True,
            timeout=30, check=False,
        )  # fmt: skip
        if current.returncode == 0 and current.stdout.strip() == value:
            continue
        for attempt in range(CONFIG_ATTEMPTS):
            done = run(
                ["git", "config", key, value], cwd=cwd, capture_output=True, text=True,
                timeout=30, check=False,
            )  # fmt: skip
            if done.returncode == 0:
                break
            if done.returncode != 255 or attempt == CONFIG_ATTEMPTS - 1:
                raise subprocess.CalledProcessError(
                    done.returncode, done.args, done.stdout, done.stderr
                )
            sleep(min(0.05 * (attempt + 1), 0.5))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    parser.add_argument("--register", action="store_true", help="write the git config entry")
    parser.add_argument("paths", nargs="*", type=Path, help="base ours theirs (git %%O %%A %%B)")
    args = parser.parse_args(argv)
    if args.register:
        register()
        return 0
    if len(args.paths) != 3:
        parser.error("expected base, ours and theirs paths")
    base_path, ours_path, theirs_path = args.paths
    try:
        base = _load(base_path)
        ours = _load(ours_path)
        theirs = _load(theirs_path)
    except ValueError as error:
        print(f"t2-evidence merge driver: unreadable JSON ({error}); conflict", file=sys.stderr)
        return 1
    if ours is None or theirs is None:
        print(
            "t2-evidence merge driver: a side is empty or not an object; conflict", file=sys.stderr
        )
        return 1
    ours_path.write_text(dump(merge3(base, ours, theirs)), encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
