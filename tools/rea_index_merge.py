"""Git merge driver for docs/data/rea-continuous/index.json (T3386).

Usage as a driver (``%O`` base, ``%A`` ours, rewritten in place, ``%B`` theirs):
``python3 -m tools.rea_index_merge %O %A %B``. Register it with ``--register``
(done by ``python3 -m tools.wt bootstrap`` and by ``merge_task``).

Unknowns are unioned by (id, revision). A record present on both sides has its
list fields unioned without duplicates and keeps the further-along status
(resolved > answered-static > investigating > open); other scalar fields come
from the side with the further-along status, theirs on a tie. Top-level
metadata comes from theirs, with counters recomputed. The output is the
normalised compact form of ``tools.rea_index_check``.

T3461: when the ancestor ``%O`` parses, the driver merges three-way
(``merge3_indexes``): a record or field only one side edited takes that edit, a
record neither side removed is never dropped, lists are unioned, status never
downgrades (resolved > answered-static > investigating > open) and
``contradicted`` wins only when the side that carries it set it. A scalar both
sides changed comes from the newer record (``updated_utc``/``last_updated``, then
the further status, theirs on a tie). ``merge_indexes`` stays the two-way form.

T3486: the index is sharded (``tools.rea_index``). The same driver merges every store file
by content kind (``merge_documents``): shards ``unknowns/<bb>.json`` record-wise as above,
``meta.json`` with ``merge3_meta``, the generated summary ``index.json`` row-wise (view
regeneration makes it exact), and a legacy full index meeting the new layout becomes a
``rea-legacy-delta`` document that regeneration absorbs into the shards.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

from tools.rea_index_check import dedupe_lists, dump

STATUS_RANK = {
    "open": 0,
    "investigating": 1,
    "answered-static": 2,
    "resolved": 3,
    "contradicted": 3,
}
DRIVER_NAME = "rea-index"
DRIVER_COMMAND = "python3 -m tools.rea_index_merge %O %A %B"


def _rank(record: dict) -> int:
    return STATUS_RANK.get(record.get("status"), -1)


def _key(record: dict) -> str:
    if "id" in record:
        return f"{record['id']}#{record.get('revision')}"
    return json.dumps(record, sort_keys=True, ensure_ascii=False)


def merge_records(ours: dict, theirs: dict) -> dict:
    winner, loser = (ours, theirs) if _rank(ours) > _rank(theirs) else (theirs, ours)
    out = dict(loser)
    out.update(winner)
    for field in set(ours) | set(theirs):
        left, right = ours.get(field), theirs.get(field)
        lists = [value for value in (left, right) if isinstance(value, list)]
        # Union only when every present value is a list; a type mismatch keeps the winner's value.
        if lists and all(isinstance(v, list) or v is None for v in (left, right)):
            out[field] = [item for value in lists for item in value]
    return dedupe_lists(out)


def merge_unknowns(ours: list, theirs: list) -> list:
    merged: dict[str, dict] = {}
    for record in ours + theirs:
        key = _key(record)
        merged[key] = merge_records(merged[key], record) if key in merged else record
    return list(merged.values())


def merge_indexes(ours: dict, theirs: dict) -> dict:
    out = dict(ours)
    out.update(theirs)
    unknowns = merge_unknowns(ours.get("unknowns", []), theirs.get("unknowns", []))
    out["unknowns"] = [dedupe_lists(u) for u in unknowns]
    out["unknown_revision_count"] = len(unknowns)
    out["unknown_id_count"] = len({u["id"] for u in unknowns if "id" in u})
    return out


# ---- T3461: three-way merge (base-aware); the driver uses it whenever %O parses ----

# Progress order without "contradicted": that status wins only when a side set it (T3461).
PROGRESS = {"open": 0, "investigating": 1, "answered-static": 2, "resolved": 3}
TIME_FIELDS = ("updated_utc", "last_updated")


def merge_status3(base: object, ours: object, theirs: object) -> object:
    """Status never downgrades; contradicted wins only when that side changed it."""
    if ours == theirs:
        return ours
    if "contradicted" in (ours, theirs) and base != "contradicted":
        return "contradicted"
    if base == "contradicted":  # exactly one side may still carry the inherited value
        return theirs if ours == base else ours if theirs == base else _further(ours, theirs)
    return _further(ours, theirs)


def _further(ours: object, theirs: object) -> object:
    return ours if PROGRESS.get(ours, -1) > PROGRESS.get(theirs, -1) else theirs


def _union(ours: list | None, theirs: list | None) -> list:
    return dedupe_lists([*(ours or []), *(theirs or [])])


def merge_value3(base: object, ours: object, theirs: object, prefer_ours: bool) -> object:
    """Generic three-way merge: lists union, dicts per key, scalars by the side that changed.

    A scalar both sides changed differently comes from the newer side (`prefer_ours`).
    """
    if ours == theirs:
        return ours
    if isinstance(ours, list) and isinstance(theirs, list):
        return _union(ours, theirs)
    if isinstance(ours, dict) and isinstance(theirs, dict):
        base_dict = base if isinstance(base, dict) else {}
        out = {}
        for key in [*ours, *(k for k in theirs if k not in ours)]:
            if key not in theirs:
                if key in base_dict and base_dict[key] == ours[key]:
                    continue  # theirs deleted an unchanged key
                out[key] = ours[key]
            elif key not in ours:
                if key in base_dict and base_dict[key] == theirs[key]:
                    continue  # ours deleted an unchanged key
                out[key] = theirs[key]
            else:
                out[key] = merge_value3(base_dict.get(key), ours[key], theirs[key], prefer_ours)
        return out
    if theirs == base:
        return ours
    if ours == base:
        return theirs
    return ours if prefer_ours else theirs


def _newer_is_ours(ours: dict, theirs: dict) -> bool:
    """The newer revision of a record both sides edited: timestamp, then progress, then theirs."""
    for field in TIME_FIELDS:
        left, right = ours.get(field), theirs.get(field)
        if isinstance(left, str) and isinstance(right, str) and left != right:
            return left > right
    return PROGRESS.get(ours.get("status"), -1) > PROGRESS.get(theirs.get("status"), -1)


def merge_record3(base: dict | None, ours: dict, theirs: dict) -> dict:
    base = base or {}
    out = dict(merge_value3(base, ours, theirs, _newer_is_ours(ours, theirs)))
    if "status" in ours or "status" in theirs:
        out["status"] = merge_status3(base.get("status"), ours.get("status"), theirs.get("status"))
    return dedupe_lists(out)


def _by_key(records: list) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for record in records:
        key = _key(record)
        out[key] = merge_records(out[key], record) if key in out else record
    return out


def merge_unknowns3(base: list, ours: list, theirs: list) -> list:
    """Union by (id, revision); a record only one side edited takes that edit; never drop."""
    base_map, ours_map, theirs_map = _by_key(base), _by_key(ours), _by_key(theirs)
    out = []
    for key, mine in ours_map.items():
        other, old = theirs_map.get(key), base_map.get(key)
        if other is None or other == old or other == mine:
            out.append(mine)
        elif mine == old:
            out.append(other)
        else:
            out.append(merge_record3(old, mine, other))
    for key, other in theirs_map.items():
        if key in ours_map:
            continue
        if key in base_map and base_map[key] == other:
            continue  # ours removed a record theirs never touched
        out.append(other)
    return [dedupe_lists(u) for u in out]


def merge3_meta(base: dict, ours: dict, theirs: dict) -> dict:
    """Three-way merge of top-level metadata (everything but ``unknowns``).

    Keys are never dropped: a worker index that lost `schema` or `source_snapshot` must not
    strip them from main (T3461, seen in a remote branch). ``generated_utc`` takes the max."""
    meta = {k: v for k, v in ours.items() if k != "unknowns"}
    other = {k: v for k, v in theirs.items() if k != "unknowns"}
    old = {k: v for k, v in base.items() if k != "unknowns"}
    out = merge_value3(old, meta, other, prefer_ours=False)
    out = {**{k: v for k, v in meta.items() if k not in out}, **out}
    if isinstance(ours.get("generated_utc"), str) and isinstance(theirs.get("generated_utc"), str):
        out["generated_utc"] = max(ours["generated_utc"], theirs["generated_utc"])
    return out


def merge3_indexes(base: dict, ours: dict, theirs: dict) -> dict:
    """Three-way merge of whole legacy index documents; counters recomputed (T3461)."""
    out = merge3_meta(base, ours, theirs)
    unknowns = merge_unknowns3(
        base.get("unknowns", []), ours.get("unknowns", []), theirs.get("unknowns", [])
    )
    out["unknowns"] = unknowns
    out["unknown_revision_count"] = len(unknowns)
    out["unknown_id_count"] = len({u["id"] for u in unknowns if "id" in u})
    return out


def merge_unknowns2(ours: list, theirs: list) -> list:
    """Two-way absorb of a stale full copy (T3486): ours wins unless theirs is strictly further.

    Used when a legacy index has no usable ancestor; lists are unioned, nothing is dropped."""
    merged: dict[str, dict] = {}
    for record in ours:
        key = _key(record)
        merged[key] = merge_records(merged[key], record) if key in merged else record
    for record in theirs:
        key = _key(record)
        mine = merged.get(key)
        if mine is None:
            merged[key] = record
        elif mine != record:
            winner, loser = (record, mine) if _rank(record) > _rank(mine) else (mine, record)
            out = {**loser, **winner}
            for name in set(mine) | set(record):
                left, right = mine.get(name), record.get(name)
                if isinstance(left, list) and isinstance(right, list):
                    out[name] = [*left, *right]
            merged[key] = dedupe_lists(out)
    return list(merged.values())


# ---- T3486: sharded store documents ----


def merge_shard3(base: dict | None, ours: dict, theirs: dict) -> dict:
    """Merge two versions of one ``unknowns/<bb>.json`` shard; seq keeps the earliest."""
    from tools import rea_index

    if base is not None:
        unknowns = merge_unknowns3(base.get("unknowns", []), ours["unknowns"], theirs["unknowns"])
    else:
        unknowns = [dedupe_lists(u) for u in merge_unknowns(ours["unknowns"], theirs["unknowns"])]
    seq = dict(theirs.get("seq") or {})
    for key, value in (ours.get("seq") or {}).items():
        seq[key] = min(value, seq[key]) if isinstance(seq.get(key), int) else value
    out = {**theirs, **ours, "seq": seq, "unknowns": unknowns}
    return rea_index.normalize_shard(out)


def merge_summary3(base: dict | None, ours: dict, theirs: dict) -> list[list]:
    """Row-level merge of the generated summary; regeneration after the merge makes it exact."""

    def keyed(doc: dict | None) -> dict[str, list]:
        rows = (doc or {}).get("rows", [])
        return {(r[0] if isinstance(r[0], str) else json.dumps(r)): r for r in rows}

    old, mine, other = keyed(base), keyed(ours), keyed(theirs)
    out = []
    for key in [*mine, *(k for k in other if k not in mine)]:
        left, right = mine.get(key), other.get(key)
        if left is None or right is None:
            out.append(left or right)
        elif right == old.get(key) or left == right:
            out.append(left)
        elif left == old.get(key):
            out.append(right)
        else:
            out.append(left if PROGRESS.get(left[1], -1) > PROGRESS.get(right[1], -1) else right)
    no_seq = 1 << 62
    return sorted(out, key=lambda r: (r[6] if isinstance(r[6], int) else no_seq, str(r[0])))


def merge_documents(base: object, ours: object, theirs: object) -> str | None:
    """Merged text for any REA store file (legacy index, shard, meta, summary, legacy delta).

    None when the pair cannot be merged automatically (the driver then reports a conflict)."""
    from tools import rea_index

    kinds = (rea_index.kind(ours), rea_index.kind(theirs))
    base_kind = rea_index.kind(base)
    if kinds == ("legacy", "legacy"):
        assert isinstance(ours, dict) and isinstance(theirs, dict)
        if base_kind == "legacy":
            assert isinstance(base, dict)
            return dump(merge3_indexes(base, ours, theirs))
        return dump(merge_indexes(ours, theirs))
    if kinds == ("shard", "shard"):
        assert isinstance(ours, dict) and isinstance(theirs, dict)
        old = base if base_kind == "shard" and isinstance(base, dict) else None
        return rea_index.dump_shard(merge_shard3(old, ours, theirs))
    if kinds == ("meta", "meta"):
        assert isinstance(ours, dict) and isinstance(theirs, dict)
        old = base if base_kind == "meta" and isinstance(base, dict) else {}
        return rea_index.dump_meta(merge3_meta(old, ours, theirs))
    if kinds == ("summary", "summary"):
        assert isinstance(ours, dict) and isinstance(theirs, dict)
        old = base if base_kind == "summary" and isinstance(base, dict) else None
        return rea_index.dump_summary(merge_summary3(old, ours, theirs))
    if set(kinds) <= {"summary", "legacy", "delta"}:
        # A legacy full index meets the sharded layout: keep its edits as a delta that view
        # regeneration (merge_task) or tools.rea_index_check --normalize absorbs (T3486).
        legacy_base = base if base_kind == "legacy" and isinstance(base, dict) else None
        deltas = []
        for side in (ours, theirs):
            if rea_index.kind(side) == "delta":
                assert isinstance(side, dict)
                deltas.append(side)
            elif rea_index.kind(side) == "legacy":
                assert isinstance(side, dict)
                deltas.append(rea_index.legacy_delta(legacy_base, side))
        return rea_index.compact(rea_index.combine_deltas(*deltas)) + "\n"
    return None


def register(cwd: Path | None = None) -> None:
    """Write the driver into the repository config shared by all worktrees."""
    for key, value in (
        (f"merge.{DRIVER_NAME}.name", "REA continuous index union merge"),
        (f"merge.{DRIVER_NAME}.driver", DRIVER_COMMAND),
    ):
        for attempt in range(20):
            try:
                subprocess.run(["git", "config", key, value], cwd=cwd, check=True, timeout=30)
                break
            except subprocess.CalledProcessError as error:
                # Worktree bootstraps can concurrently update the shared common config.
                # Git reports a lock collision as a failed config command; retry briefly.
                if error.returncode != 255 or attempt == 19:
                    raise
                time.sleep(min(0.05 * (attempt + 1), 0.25))


def load_base(path: Path) -> dict | None:
    """The %O ancestor, or None when there is none (add/add) or it does not parse."""
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def main() -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    parser.add_argument("--register", action="store_true", help="write the git config entry")
    parser.add_argument("paths", nargs="*", type=Path, help="base ours theirs (git %%O %%A %%B)")
    args = parser.parse_args()
    if args.register:
        register()
        return 0
    if len(args.paths) != 3:
        parser.error("expected base, ours and theirs paths")
    base_path, ours_path, theirs_path = args.paths
    ours = json.loads(ours_path.read_text(encoding="utf-8"))
    theirs = json.loads(theirs_path.read_text(encoding="utf-8"))
    merged = merge_documents(load_base(base_path), ours, theirs)
    if merged is None:
        print(
            f"rea-index merge driver: cannot merge {ours_path} automatically (T3486)",
            file=sys.stderr,
        )
        return 1
    ours_path.write_text(merged, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
