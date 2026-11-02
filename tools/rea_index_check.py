"""Validate the continuous REA index: JSON syntax, status vocabulary, list duplicates, sizes.

T3486: the index is the sharded store of ``tools.rea_index`` (``unknowns/<bb>.json`` shards,
``meta.json`` and the generated summary ``index.json``). With no path argument the whole store
is checked: canonical shard text, every id in its own bucket, seq present, per-file size caps
and a summary equal to its regeneration. ``--normalize`` rewrites the store canonically,
absorbs a pending legacy ``index.json`` or legacy delta and regenerates the summary. A path
argument that is a single legacy-format file is validated with the pre-T3486 rules.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

MAX_BYTES = 25 * 1024 * 1024  # a legacy single-file index and the generated summary
MAX_SHARD_BYTES = 4 * 1024 * 1024  # per unknowns/<bb>.json shard (below the 5 MB data guard)
MAX_META_BYTES = 4 * 1024 * 1024
SUMMARY_WARN_BYTES = 12 * 1024 * 1024
FIX = "run python3 -m tools.rea_index_check --normalize"


def _key(item: object) -> str:
    return json.dumps(item, sort_keys=True, ensure_ascii=False)


def dedupe_lists(value: object) -> object:
    """Recursively drop duplicate entries from every list, keeping first occurrence."""
    if isinstance(value, dict):
        return {k: dedupe_lists(v) for k, v in value.items()}
    if isinstance(value, list):
        seen: set[str] = set()
        out = []
        for item in value:
            item = dedupe_lists(item)
            key = _key(item)
            if key not in seen:
                seen.add(key)
                out.append(item)
        return out
    return value


def normalize(data: dict) -> dict:
    """Idempotent: unknowns' list fields deduplicated (top-level unknowns list kept as is)."""
    out = dict(data)
    out["unknowns"] = [dedupe_lists(u) for u in data["unknowns"]]
    return out


def dump(data: dict) -> str:
    return json.dumps(data, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n"


def duplicate_errors(path: Path, data: dict) -> list[str]:
    errors = []
    for i, item in enumerate(data["unknowns"]):
        if isinstance(item, dict) and item != dedupe_lists(item):
            errors.append(f"{path}: unknowns[{i}] has duplicate list entries (run --normalize)")
    return errors


def va_evidence_errors(path: str, item: dict) -> list[str]:
    """Keep the T2368 inventory anchor correction from silently regressing."""
    if item.get("id") != "unk_gameplay_inventory_duplicate_item_identity":
        return []
    errors: list[str] = []
    evidence = item.get("evidence_links")
    methods = (
        " ".join(link.get("method", "") for link in evidence if isinstance(link, dict))
        if isinstance(evidence, list)
        else ""
    )
    answer = str(item.get("static_answer", "")).lower()
    if "0x179130" not in answer:
        errors.append(f"{path}: T2368 anchor finding must retain mapped containing entry 0x179130")
    rejects_unmapped_probe = "rizin -n" in methods and not (
        "read file offsets" in methods and "invalid" in methods
    )
    if "io.va=true" not in methods or "objdump" not in methods or rejects_unmapped_probe:
        errors.append(
            f"{path}: T2368 anchor evidence must require VA-mapped Rizin and objdump, not rizin -n"
        )
    if "24 c3" in answer or "and al,0xc3" in answer:
        errors.append(
            f"{path}: T2368 anchor finding contains the superseded file-offset byte claim"
        )
    return errors


STATUSES = {"open", "investigating", "answered-static", "resolved", "contradicted"}


def validate(path: Path) -> list[str]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [f"{path}: invalid JSON or unreadable file: {exc}"]
    if not isinstance(data, dict) or not isinstance(data.get("unknowns"), list):
        return [f"{path}: expected an object with an unknowns array"]
    errors = []
    for i, item in enumerate(data["unknowns"]):
        if not isinstance(item, dict):
            errors.append(f"{path}: unknowns[{i}] must be an object")
        elif item.get("status") not in STATUSES:
            errors.append(f"{path}: unknowns[{i}] has invalid status {item.get('status')!r}")
    errors += duplicate_errors(path, data)
    if path.stat().st_size > MAX_BYTES:
        errors.append(f"{path}: {path.stat().st_size} bytes exceeds {MAX_BYTES} (run --normalize)")
    return errors


def validate_store(root: Path) -> tuple[list[str], list[str]]:
    """(errors, warnings) for the sharded store under ``root`` (T3486)."""
    from tools import rea_index

    store = rea_index.read_store(root)
    errors = list(store.errors)
    warnings: list[str] = []
    for rel, doc in store.shards.items():
        bucket = Path(rel).stem
        size = len(store.texts[rel].encode("utf-8"))
        if size > MAX_SHARD_BYTES:
            errors.append(
                f"{rel}: {size} bytes exceeds {MAX_SHARD_BYTES}; raise tools.rea_index."
                "BUCKET_HEX and run --normalize to re-shard"
            )
        seq = doc.get("seq") if isinstance(doc.get("seq"), dict) else {}
        for i, item in enumerate(doc["unknowns"]):
            if not isinstance(item, dict):
                errors.append(f"{rel}: unknowns[{i}] must be an object")
                continue
            if item.get("status") not in STATUSES:
                errors.append(f"{rel}: unknowns[{i}] has invalid status {item.get('status')!r}")
            errors.extend(va_evidence_errors(rel, item))
            key = rea_index.record_key(item)
            if rea_index.bucket_of(key) != bucket:
                errors.append(f"{rel}: {key} belongs in {rea_index.shard_path(key)} ({FIX})")
            elif key not in seq:
                errors.append(f"{rel}: {key} has no seq entry ({FIX})")
            if item != dedupe_lists(item):
                errors.append(f"{rel}: unknowns[{i}] has duplicate list entries ({FIX})")
        if not errors and store.texts[rel] != rea_index.dump_shard(rea_index.normalize_shard(doc)):
            errors.append(f"{rel}: not in canonical form ({FIX})")
    if store.meta_text is None:
        errors.append(f"{rea_index.META_PATH}: missing")
    elif not store.errors:
        if rea_index.kind(store.meta) != "meta":
            errors.append(f"{rea_index.META_PATH}: expected a metadata object")
        elif store.meta_text != rea_index.dump_meta(store.meta):
            errors.append(f"{rea_index.META_PATH}: not in canonical form ({FIX})")
        if len(store.meta_text.encode("utf-8")) > MAX_META_BYTES:
            errors.append(f"{rea_index.META_PATH}: exceeds {MAX_META_BYTES} bytes")
    summary_kind = rea_index.kind(store.index_doc)
    if store.index_text is None:
        errors.append(f"{rea_index.INDEX_PATH}: missing generated summary ({FIX})")
    elif summary_kind in ("legacy", "delta"):
        errors.append(
            f"{rea_index.INDEX_PATH}: holds a pending {summary_kind} index; absorb it into the "
            f"shards ({FIX}, or python3 -m tools.agents.views --write)"
        )
    elif summary_kind != "summary":
        errors.append(f"{rea_index.INDEX_PATH}: expected the generated summary ({FIX})")
    else:
        size = len(store.index_text.encode("utf-8"))
        if size > MAX_BYTES:
            errors.append(f"{rea_index.INDEX_PATH}: {size} bytes exceeds {MAX_BYTES}")
        elif size > SUMMARY_WARN_BYTES:
            warnings.append(
                f"{rea_index.INDEX_PATH}: summary is {size} bytes (cap {MAX_BYTES}); plan the "
                "summary split described in docs/t3486-rea-index-shards.md"
            )
        if not errors and store.index_text != rea_index.summary_text(store.shards):
            errors.append(f"{rea_index.INDEX_PATH}: summary is stale ({FIX})")
    return errors, warnings


def main() -> int:
    from tools import rea_index

    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="legacy single-file indexes to check (default: the sharded store under --root)",
    )
    parser.add_argument("--root", type=Path, default=Path("."), help="repository root")
    parser.add_argument(
        "--normalize",
        action="store_true",
        help="rewrite canonically (dedupe lists, re-bucket, seq, absorb legacy edits, summary)",
    )
    args = parser.parse_args()
    if not args.paths and rea_index.has_store(args.root):
        if args.normalize:
            for path in rea_index.write_generated(args.root):
                print(f"wrote {path}")
        errors, warnings = validate_store(args.root)
        for warning in warnings:
            print(f"warning: {warning}")
        if errors:
            print("\n".join(errors[:50]))
            if len(errors) > 50:
                print(f"... {len(errors) - 50} more")
            return 1
        print(f"validated the sharded REA index under {args.root / rea_index.REA_DIR}")
        return 0
    paths = args.paths or [args.root / rea_index.INDEX_PATH]
    if args.normalize:
        for path in paths:
            data = json.loads(path.read_text(encoding="utf-8"))
            path.write_text(dump(normalize(data)), encoding="utf-8")
    errors = [error for path in paths for error in validate(path)]
    if errors:
        print("\n".join(errors))
        return 1
    print(f"validated {len(paths)} REA index file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
