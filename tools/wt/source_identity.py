"""Source identity of a git-less exec tree (T3760 stage 4).

Owner decision 2026-10-26 (docs/decisions/remote-hosts-execution-only.md): remote hosts keep no
git repository. `tools.wt.exec_sync` places a job's exact tree in an exec dir with two data
files: the marker `.tsfp-source.json` (commit, tree) and the listing `.tsfp-source-files`
(`git ls-tree -r -z --full-tree` of the tree). Tools that used to ask git about their checkout
(HEAD, `ls-tree`, `show`, `ls-files -s`, `diff`) ask this module instead when they run in such a
tree, and get byte-identical answers:

* the listing is checked against the tree id (the git tree objects are rebuilt from it), so a
  stale or truncated listing fails loudly instead of yielding a wrong input hash;
* `TSFP_SOURCE_COMMIT` / `TSFP_SOURCE_TREE` (`exec_sync.identity_exports`) must agree with the
  marker when both are present;
* file contents read for a hash are checked against their blob id.

A tree is an exec tree when the first directory upwards holding `.git` or one of the two data
files holds the data file (`exec_root`). Without git and without an identity, callers fail
loudly (`require`, rule 45). `TSFP_MAIN_ROOT` names the host dir that holds the gitignored
inputs and shared caches (the role the main checkout plays for a linked worktree).
"""

from __future__ import annotations

import argparse
import functools
import hashlib
import json
import os
import re
import sys
from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from pathlib import Path

MARKER = ".tsfp-source.json"
"""The exec dir's marker: commit, tree and sync time (written by `tools.wt.exec_sync`)."""
LISTING = ".tsfp-source-files"
"""`git ls-tree -r -z --full-tree <tree>` of the exported tree, shipped inside it: the blob
ids git-less tools need (input hashes, stamps), checked against the tree id. This module is
stdlib-only: `tools.wt.git_compat` ships it to hosts beside the shim."""
ENV_COMMIT = "TSFP_SOURCE_COMMIT"
ENV_TREE = "TSFP_SOURCE_TREE"
ENV_MAIN = "TSFP_MAIN_ROOT"
_OID = re.compile(r"[0-9a-f]{40}")
_ESCAPES = {7: "a", 8: "b", 9: "t", 10: "n", 11: "v", 12: "f", 13: "r", 34: '"', 92: "\\"}


class SourceIdentityError(RuntimeError):
    """An exec tree whose identity is missing, inconsistent or does not match its files."""


@dataclass(frozen=True)
class Entry:
    mode: str
    kind: str
    oid: str
    path: str

    def row(self) -> str:
        """The `git ls-tree` row of this entry (without the NUL terminator)."""
        return f"{self.mode} {self.kind} {self.oid}\t{self.path}"


@dataclass(frozen=True)
class Identity:
    root: Path
    commit: str
    tree: str


def exec_root(start: Path) -> Path | None:
    """The exec tree containing `start`, or None inside a git checkout (or outside both)."""
    start = Path(start).expanduser().resolve()
    for candidate in (start, *start.parents):
        if (candidate / ".git").exists():
            return None
        if (candidate / MARKER).is_file() or (candidate / LISTING).is_file():
            return candidate
    return None


def main_override() -> Path | None:
    """`TSFP_MAIN_ROOT`: the host dir standing in for the main checkout of an exec tree."""
    value = os.environ.get(ENV_MAIN)
    return Path(value).expanduser() if value else None


def load(start: Path) -> Identity | None:
    """The identity of the exec tree at `start`; None in a git checkout."""
    root = exec_root(start)
    if root is None:
        return None
    marked: dict[str, object] = {}
    if (root / MARKER).is_file():
        try:
            data = json.loads((root / MARKER).read_text())
        except (OSError, ValueError) as error:
            raise SourceIdentityError(f"unreadable {root / MARKER}: {error}") from error
        if not isinstance(data, dict):
            raise SourceIdentityError(f"{root / MARKER} is not an object")
        marked = data
    values: dict[str, str] = {}
    for key, env in (("commit", ENV_COMMIT), ("tree", ENV_TREE)):
        from_marker = str(marked.get(key) or "")
        given = os.environ.get(env, "")
        if from_marker and given and from_marker != given:
            raise SourceIdentityError(
                f"{env}={given} disagrees with the {key} {from_marker} in {root / MARKER}"
            )
        values[key] = given or from_marker
    if not _OID.fullmatch(values["tree"]):
        raise SourceIdentityError(f"{root}: no source tree id in {MARKER} or {ENV_TREE}")
    if values["commit"] and not _OID.fullmatch(values["commit"]):
        raise SourceIdentityError(f"{root}: malformed source commit {values['commit']!r}")
    return Identity(root, values["commit"], values["tree"])


def require(start: Path) -> Identity:
    identity = load(start)
    if identity is None:
        raise SourceIdentityError(
            f"{start}: no git repository and no source identity ({MARKER} / {ENV_TREE}, "
            "written by tools.wt.exec_sync); cannot name the source"
        )
    return identity


def head(identity: Identity) -> str:
    """The commit the exec tree was synced from (what `git rev-parse HEAD` answered)."""
    if not identity.commit:
        raise SourceIdentityError(f"{identity.root} holds a bare tree, not a commit")
    return identity.commit


def tree_id(entries: Iterable[Entry]) -> str:
    """The git tree id of a flat `ls-tree -r` listing (tree objects rebuilt and hashed)."""
    top: dict[str, object] = {}
    for entry in entries:
        *dirs, name = entry.path.split("/")
        node = top
        for part in dirs:
            child = node.setdefault(part, {})
            if not isinstance(child, dict):
                raise SourceIdentityError(f"listing names {entry.path} below a file")
            node = child
        node[name] = entry

    def build(node: dict[str, object]) -> bytes:
        items = []
        for name, value in node.items():
            raw = name.encode("utf-8", "surrogateescape")
            if isinstance(value, dict):
                items.append((raw + b"/", b"40000", raw, build(value)))
            else:
                assert isinstance(value, Entry)
                items.append((raw, value.mode.encode(), raw, bytes.fromhex(value.oid)))
        items.sort(key=lambda item: item[0])
        body = b"".join(mode + b" " + raw + b"\0" + oid for _key, mode, raw, oid in items)
        return hashlib.sha1(b"tree %d\0" % len(body) + body).digest()

    return build(top).hex()


@functools.lru_cache(maxsize=4)
def _entries(listing: Path, tree: str, stamp: tuple[int, int]) -> tuple[Entry, ...]:
    del stamp  # cache key only: a rewritten listing is read again
    try:
        raw = listing.read_bytes()
    except OSError as error:
        raise SourceIdentityError(f"no tree listing {listing}: {error}") from error
    entries = []
    for row in filter(None, raw.split(b"\0")):
        meta, _tab, path = row.partition(b"\t")
        fields = meta.decode().split()
        if len(fields) != 3 or not _tab:
            raise SourceIdentityError(f"malformed row in {listing}: {row[:120]!r}")
        mode, kind, oid = fields
        entries.append(Entry(mode, kind, oid, path.decode("utf-8", "surrogateescape")))
    found = tree_id(entries)
    if found != tree:
        raise SourceIdentityError(f"{listing} describes tree {found}, the source is {tree}")
    return tuple(entries)


def entries(identity: Identity) -> tuple[Entry, ...]:
    listing = identity.root / LISTING
    try:
        status = listing.stat()
    except OSError as error:
        raise SourceIdentityError(f"no tree listing {listing}: {error}") from error
    return _entries(listing, identity.tree, (status.st_mtime_ns, status.st_size))


def matches(path: str, specs: Sequence[str]) -> bool:
    """`git ls-tree` / `ls-files` literal pathspec: the path itself or anything below it."""
    for spec in specs:
        base = spec.rstrip("/")
        if path == base or path.startswith(base + "/"):
            return True
    return False


def ls_tree_rows(identity: Identity, specs: Sequence[str]) -> list[str]:
    """Rows of `git ls-tree -r -z <commit> -- <specs>` (order as git lists them)."""
    return [entry.row() for entry in entries(identity) if matches(entry.path, specs)]


def quote_path(path: str) -> str:
    """git's default `core.quotePath` C-style quoting of a path in `ls-files` output."""
    raw = path.encode("utf-8", "surrogateescape")
    if not any(byte < 0x20 or byte in (0x22, 0x5C) or byte >= 0x7F for byte in raw):
        return path
    out = []
    for byte in raw:
        if byte in _ESCAPES:
            out.append("\\" + _ESCAPES[byte])
        elif byte < 0x20 or byte >= 0x7F:
            out.append(f"\\{byte:03o}")
        else:
            out.append(chr(byte))
    return '"' + "".join(out) + '"'


def ls_files_stage(identity: Identity, specs: Sequence[str]) -> str:
    """Output of `git ls-files -s <specs>` in a clean checkout of the tree."""
    chosen = sorted(
        (entry for entry in entries(identity) if matches(entry.path, specs)),
        key=lambda entry: entry.path.encode("utf-8", "surrogateescape"),
    )
    return "".join(f"{entry.mode} {entry.oid} 0\t{quote_path(entry.path)}\n" for entry in chosen)


def blob_id(path: Path) -> str:
    """git's blob id of a file (of the link text for a symlink)."""
    data = os.readlink(path).encode() if path.is_symlink() else path.read_bytes()
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def read_text(identity: Identity, path: str) -> str:
    """`git show <commit>:<path>`, read from the tree and checked against the blob id."""
    entry = next((item for item in entries(identity) if item.path == path), None)
    if entry is None or entry.kind != "blob":
        raise SourceIdentityError(f"{path} is not a file of tree {identity.tree}")
    file_path = identity.root / path
    try:
        if blob_id(file_path) != entry.oid:
            raise SourceIdentityError(f"{file_path} differs from its blob {entry.oid}")
        return file_path.read_text()
    except OSError as error:
        raise SourceIdentityError(f"cannot read {file_path}: {error}") from error


def modified(identity: Identity, specs: Sequence[str]) -> list[str]:
    """Tracked files under `specs` whose content differs from the tree or that are missing
    (`git diff --name-only <commit> -- <specs>`; untracked files are not seen without git, the
    rsync `--delete` of the sync keeps the exec tree free of them)."""
    changed = []
    for entry in entries(identity):
        if entry.kind != "blob" or not matches(entry.path, specs):
            continue
        file_path = identity.root / entry.path
        try:
            same = blob_id(file_path) == entry.oid
        except OSError:
            same = False
        if not same:
            changed.append(entry.path)
    return sorted(changed)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.wt.source_identity", description=__doc__.split("\n")[0]
    )
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="exec tree (default: cwd)")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("head", help="the source commit (git rev-parse HEAD)")
    stage = sub.add_parser("ls-files-s", help="git ls-files -s PATH... of the tree")
    stage.add_argument("paths", nargs="+")
    args = parser.parse_args(argv)
    try:
        identity = require(args.root)
        if args.command == "head":
            print(head(identity))
        else:
            sys.stdout.write(ls_files_stage(identity, args.paths))
    except SourceIdentityError as error:
        print(f"source_identity: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
