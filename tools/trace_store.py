"""Capture one shared-host replay for later, bounded trace queries (T2471)."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

from tools import gate_data, owner_files, prebuilt_select, rea_index
from tools.rust_trace import load_regions
from tools.wt import prebuilt

ROOT = Path(__file__).resolve().parents[1]
REGIONS = ROOT / "tools/data/rust_trace_regions_gameplay.json"
INDEX = ROOT / "docs/data/rea-continuous/index.json"
PROBE_FIELDS = ("runtime_probe", "recommended_probes", "static_evidence_to_collect")
HEX = re.compile(r"(?i)(?<![\w])0x[0-9a-f]{4,8}\b")
POLL_RE = re.compile(r"^(\d+)-(\d+)$")
CALL_LIMIT = 1024  # T3391: matches CALL_ARGS_MAX in src/host/rust_ports.c (was 256)
REGION_LIMIT = 1024
QUERY_LIMIT = 60


def main_root() -> Path:
    return owner_files.main_checkout(ROOT)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def tree_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    for item in sorted(path.rglob("*")):
        if item.is_file():
            digest.update(item.relative_to(path).as_posix().encode())
            digest.update(bytes.fromhex(sha256(item)))
    return digest.hexdigest()


def probe_text(value: object) -> str:
    if isinstance(value, str):
        return value
    if value is None:
        return ""
    return json.dumps(value, sort_keys=True)


def is_call_target(text: str, address_match: re.Match[str]) -> bool:
    keywords = list(
        re.finditer(
            r"\b(call(?:er|ee|s)?|function|root|callee|entry|watch|address|variable|field|state)\b",
            text,
            re.I,
        )
    )
    if not keywords:
        return False
    position = address_match.start()
    nearest = min(keywords, key=lambda match: abs(match.start() - position))
    return (
        nearest.group(0).lower() not in {"watch", "address", "variable", "field", "state"}
        and abs(nearest.start() - position) <= 40
    )


def probe_unknowns(index_path: Path) -> list[dict]:
    """Records of the REA store: a repo root (sharded or legacy tree) or an index.json file.

    A file holding ``unknowns`` is a legacy single-file index (old fixtures); any other file is
    the generated summary of the sharded store (T3486) and the root is found from its path."""
    if index_path.is_dir():
        return rea_index.iter_records(index_path)
    index = json.loads(index_path.read_text())
    if isinstance(index.get("unknowns"), list):
        return index["unknowns"]
    return rea_index.iter_records(index_path.parents[len(Path(rea_index.INDEX_PATH).parts) - 1])


def store_sha256(root: Path = ROOT) -> str:
    """Deterministic digest of the REA store: the summary index.json text followed by every
    shard text in sorted path order (a legacy tree without shards hashes index.json alone,
    which equals the old single-file hash)."""
    digest = hashlib.sha256()
    digest.update((root / rea_index.INDEX_PATH).read_bytes())
    for shard in sorted((root / rea_index.SHARD_DIR).glob("*.json")):
        digest.update(shard.read_bytes())
    return digest.hexdigest()


def probe_targets(index_path: Path = ROOT) -> dict:
    unknowns = probe_unknowns(index_path)
    selected = [
        item for item in unknowns if item.get("status") in {"investigating", "answered-static"}
    ]
    watch_counts: Counter[int] = Counter()
    call_counts: Counter[int] = Counter()
    source_ids: dict[int, set[str]] = {}
    for item in selected:
        uid = str(item.get("id") or item.get("canonical_id") or "unknown")
        combined = "\n".join(probe_text(item.get(field)) for field in PROBE_FIELDS)
        matches = {int(match.group(0), 16): match for match in HEX.finditer(combined)}
        for value, match in matches.items():
            source_ids.setdefault(value, set()).add(uid)
            watch_counts[value] += 1
            # The host captures call args only for function-like probes. Keep the
            # matching rule visible: explicit call/callee/function/root wording.
            if is_call_target(combined, match):
                call_counts[value] += 1
    watches = sorted(watch_counts, key=lambda address: (-watch_counts[address], address))
    calls = sorted(call_counts, key=lambda address: (-call_counts[address], address))
    return {
        "selected_unknowns": len(selected),
        "watch_addresses": watches,
        "call_functions": calls,
        "source_unknowns": {f"0x{address:X}": sorted(source_ids[address]) for address in watches},
    }


def capture_regions(targets: dict, base_config: dict) -> dict:
    config = {
        "_comment": base_config.get("_comment", ""),
        "regions": list(base_config["regions"]),
    }
    known = []
    for item in config["regions"]:
        if "spec" in item:
            bounds = region_base(item["spec"])
            if bounds is not None:
                known.append(bounds)
    dropped = 0
    for address in targets["watch_addresses"]:
        if any(start <= address < start + length for start, length in known):
            continue
        if len(config["regions"]) >= REGION_LIMIT:
            dropped += 1  # T3388: index outgrew the host range limit; manifest lists captured ones
            continue
        config["regions"].append(
            {
                "name": f"probe_watch_{address:08X}",
                "spec": f"=0x{address:08X}:0x4",
            }
        )
    if dropped:
        print(
            f"trace_store: {dropped} probe watch address(es) over the {REGION_LIMIT}-range host "
            "limit are not captured (manifest captured_watch lists the captured ones)",
            file=sys.stderr,
        )
    regions = load_regions(config)
    if len(regions) > REGION_LIMIT:
        raise ValueError(
            f"probe union requires {len(regions)} ranges; host limit is {REGION_LIMIT}"
        )
    return config


def trace_sample_count(path: Path, report: Path) -> int:
    """T3388: samples in a trace.json without loading a multi-GB file (that OOM-killed the host):
    large traces take the count from rust_trace's report line "capture '<spec>': N samples"."""
    if path.stat().st_size < 256 * 1024 * 1024 or not report.is_file():
        return len(json.loads(path.read_text()).get("samples", []))
    match = re.search(r"capture '[^']*': (\d+) samples", report.read_text(errors="replace"))
    if match is None:
        raise ValueError(f"no sample count in {report}")
    return int(match.group(1))


def route_path(route: str, owner_root: Path) -> tuple[str, Path]:
    if route == "r5":
        path = owner_files.resolve_path(
            ROOT, "tmp/routes/r5-story-gameplay-20261016.rec", kind="file"
        )
        return "r5", path
    path = Path(route).expanduser()
    if not path.is_absolute():
        path = owner_files.resolve_path(ROOT, path, kind="file")
    path = path.resolve()
    return path.stem, path


def parse_queue_result(path: Path) -> dict:
    # The replay subprocess streams progress before its final JSON receipt.
    for line in reversed(path.read_text(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and "run_dir" in value and "exit_code" in value:
            return value
    raise RuntimeError("replay queue did not write a result receipt")


def capture(args: argparse.Namespace) -> int:
    owner_root = main_root()
    label, recording = route_path(args.route, owner_root)
    if not recording.is_file():
        raise ValueError(f"recording not found: {recording}")
    index_targets = probe_targets()
    region_config = capture_regions(index_targets, json.loads(REGIONS.read_text()))
    selected_calls = index_targets["call_functions"]
    if len(selected_calls) > CALL_LIMIT:
        # T3388: the unknowns index outgrew the host limit (396 roots on 2026-10-19). Keep the
        # most-mentioned roots (probe_targets orders them) and record the gap in the manifest
        # (call_union_complete false) instead of refusing the capture.
        print(
            f"trace_store: probe union has {len(selected_calls)} call roots; capturing the "
            f"first {CALL_LIMIT} (host limit), the manifest marks the union incomplete",
            file=sys.stderr,
        )
        selected_calls = selected_calls[:CALL_LIMIT]
    commit = args.commit or prebuilt.primary_commit(ROOT)
    host_input_hash = args.host_input_hash or prebuilt_select.build_input_hash(ROOT, commit)
    host = prebuilt_select.find_host(ROOT, commit=commit, host_input_hash=host_input_hash)
    if not prebuilt_select.available(host):
        raise ValueError(f"shared prebuilt host missing: {host}")
    host_hash = sha256(host)
    output_root = gate_data.path(owner_root, gate_data.TRACES) / label / host_hash  # T3467
    output_root.mkdir(parents=True, exist_ok=True)
    if (output_root / "manifest.json").exists() and not args.replace:
        raise ValueError(f"capture already exists: {output_root}; use --replace to refresh")
    temp_regions = output_root / ".regions.json"
    temp_regions.write_text(json.dumps(region_config, indent=2) + "\n")
    save_dir = args.save_dir
    if save_dir is None and recording.name == "story-start-20261017.rec":
        save_dir = owner_files.resolve_path(ROOT, "tmp/owner/saves-20261017", kind="dir")
    if save_dir is not None:
        save_dir = owner_files.resolve_path(ROOT, save_dir, kind="dir")
        if not save_dir.is_dir():
            raise ValueError(f"save directory not found: {save_dir}")
    queue_stdout = output_root / ".queue-output.log"
    command = [
        sys.executable,
        "-m",
        "tools.wt",
        "replay",
        "submit",
        "--route",
        str(recording),
        "--switches",
        args.switches,
        "--commit",
        commit,
        "--host-input-hash",
        host_input_hash,
        "--regions",
        str(temp_regions),
        "--call-args",
        ",".join(f"0x{value:X}" for value in selected_calls),
        "--max-dumps",
        str(args.max_dumps),
    ]
    command += ["--max-bytes", str(args.max_bytes)]  # T3388: full-length call-args traces
    if args.render:
        command += ["--render", args.render]  # T3390
    if args.seconds is not None:
        command += ["--seconds", str(args.seconds)]  # T3388: optional wall cap, default route end
    if save_dir is not None:
        command += ["--save-dir", str(save_dir)]
    with prebuilt.lease(host):
        with queue_stdout.open("w") as stdout:
            result = subprocess.run(command, cwd=ROOT, stdout=stdout, check=False)
    receipt = parse_queue_result(queue_stdout)
    if result.returncode != 0 or receipt["exit_code"] != 0:
        raise RuntimeError(
            f"replay failed: exit={receipt['exit_code']} run_dir={receipt['run_dir']}"
        )
    run_dir = Path(receipt["run_dir"])
    if run_dir.resolve() == output_root.resolve():
        raise ValueError("queue output must be separate from owner trace store")
    for name in ("trace.json", "call-args.trace", "recipe.json", "report.txt"):
        source = run_dir / name
        if source.is_file():
            shutil.copy2(source, output_root / name)
    captured_specs = {}
    for item in region_config["regions"]:
        if "spec" in item:
            captured_specs[item["name"]] = item["spec"]
    captured_watch = []
    for address in index_targets["watch_addresses"]:
        for spec in captured_specs.values():
            match = re.fullmatch(r"=?0x([0-9a-f]+):(?:0x)?([0-9a-f]+)", spec, re.I)
            if match and int(match.group(1), 16) <= address < int(match.group(1), 16) + int(
                match.group(2), 16
            ):
                captured_watch.append(address)
                break
    manifest = {
        "schema": 1,
        "route": label,
        "recording": str(recording),
        "recording_sha256": sha256(recording),
        "save_dir": str(save_dir) if save_dir else None,
        "save_sha256": tree_sha256(save_dir) if save_dir else None,
        "probe_index_sha256": store_sha256(ROOT),
        "region_config_sha256": sha256(REGIONS),
        "build_commit": commit,
        "host_sha256": host_hash,
        "host_input_hash": prebuilt_select.host_input_hash(host, ROOT),
        "replay": receipt,
        "traced": {
            "regions": captured_specs,
            "rea_unknowns_selected": index_targets["selected_unknowns"],
            "watch_addresses_requested": [f"0x{x:X}" for x in index_targets["watch_addresses"]],
            "watch_addresses_captured": [f"0x{x:X}" for x in captured_watch],
            "call_functions_requested": [f"0x{x:X}" for x in index_targets["call_functions"]],
            "call_functions_captured": [f"0x{x:X}" for x in selected_calls],
            "call_function_limit": CALL_LIMIT,
            "region_limit": REGION_LIMIT,
            "watch_union_complete": len(captured_watch) == len(index_targets["watch_addresses"]),
            "call_union_complete": selected_calls == index_targets["call_functions"],
        },
    }
    (output_root / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    )
    temp_regions.unlink(missing_ok=True)
    sample_count = trace_sample_count(output_root / "trace.json", queue_stdout)
    queue_stdout.unlink(missing_ok=True)
    print(
        json.dumps(
            {"store": str(output_root), "samples": sample_count, "host_sha256": host_hash},
            sort_keys=True,
        )
    )
    return 0


def poll_bounds(value: str) -> tuple[int, int]:
    match = POLL_RE.fullmatch(value)
    if not match:
        raise argparse.ArgumentTypeError("poll range must be START-END")
    low, high = map(int, match.groups())
    if low < 0 or high < low:
        raise argparse.ArgumentTypeError("poll range must be nonnegative and ordered")
    return low, high


def region_base(spec: str) -> tuple[int, int] | None:
    match = re.fullmatch(r"=?0x([0-9a-f]+):(?:0x)?([0-9a-f]+)", spec, re.I)
    return (int(match.group(1), 16), int(match.group(2), 16)) if match else None


def query(args: argparse.Namespace) -> int:
    root = main_root()
    store = Path(args.store) if args.store else None
    if store is None:
        route = args.route or "r5"
        host_hash = args.host_hash
        if not host_hash:
            commit = prebuilt.primary_commit(ROOT)
            host = prebuilt.artifact_dir(ROOT, commit) / "tsfp-bevy"
            if prebuilt.available(host):
                host_hash = sha256(host)
            else:
                # Owner traces are keyed by the host binary, which may belong to
                # a task branch while the shared primary host is being rebuilt.
                # Resolve an unambiguous existing route store without requiring
                # the caller to copy its implementation hash from the path.
                stores = sorted(
                    manifest
                    for base in gate_data.roots(root, gate_data.TRACES)
                    for manifest in (base / route).glob("*/manifest.json")
                )
                if len(stores) != 1:
                    raise ValueError(
                        "shared prebuilt host unavailable and route does not identify "
                        f"one trace store ({len(stores)} found); pass --host-hash"
                    )
                store = stores[0].parent
                host_hash = store.name
        store = gate_data.find(root, gate_data.TRACES, route, host_hash, marker="manifest.json")
    manifest = json.loads((store / "manifest.json").read_text())
    trace = json.loads((store / "trace.json").read_text())
    low, high = args.polls
    limit = max(1, min(args.limit, QUERY_LIMIT))
    byte_count = max(1, min(args.bytes, 64))
    emitted = 0
    if getattr(args, "compare_store", None):
        if not args.calls:
            raise ValueError("--compare-store requires --calls to select call-argument comparison")
        from tools.rust_trace_call_args import compare_files

        left_calls = store / trace.get("meta", {}).get("call_args_file", "call-args.trace")
        right_store = Path(args.compare_store)
        right_trace = json.loads((right_store / "trace.json").read_text())
        right_calls = right_store / right_trace.get("meta", {}).get(
            "call_args_file", "call-args.trace"
        )
        if not left_calls.is_file() or not right_calls.is_file():
            raise ValueError("both stores must contain an external call-args file")
        comparison = compare_files(
            left_calls,
            right_calls,
            from_poll=low,
            strict_poll=high,
            roots={int(args.calls, 16)},
        )
        print(
            f"call_args_equal={comparison['equal']} rows={comparison['rows_compared']} "
            f"diverging={comparison['diverging_rows']} missing={comparison['missing_rows']} "
            f"first={comparison['first_divergence']}"
        )
        for root, stats in comparison["per_root"].items():
            print(
                f"root={root} rows={stats['rows']} "
                f"diverging={stats['diverging']} missing={stats['missing']}"
            )
        print(f"left={left_calls} right={right_calls}")
        return 0 if comparison["equal"] else 2
    if args.watch:
        address = int(args.watch, 16)
        for sample in trace.get("samples", []):
            if emitted >= limit:
                break
            poll = int(sample.get("poll", sample.get("host_poll", -1)))
            if not low <= poll <= high:
                continue
            for name, raw in sample.get("regions", {}).items():
                spec = manifest["traced"]["regions"].get(name)
                base = region_base(spec) if spec else None
                if base and base[0] <= address < base[0] + base[1]:
                    data = bytes.fromhex(raw)
                    offset = address - base[0]
                    value = data[offset : offset + byte_count].hex()
                    print(f"poll={poll} address=0x{address:X} bytes={value}")
                    emitted += 1
                    break
    else:
        target = int(args.calls, 16)
        calls_file = store / "call-args.trace"
        if calls_file.is_file():
            with calls_file.open(errors="replace") as handle:
                for line in handle:
                    columns = line.split()
                    if len(columns) < 3:
                        continue
                    poll, address = int(columns[0]), int(columns[1], 16)
                    if low <= poll <= high and address == target:
                        print(" ".join(columns[: min(len(columns), 19)]))
                        emitted += 1
                        if emitted >= limit:
                            break
    if emitted == 0:
        print(f"no captured rows; store={store} poll_range={low}-{high}")
        if args.watch:
            captured = manifest["traced"].get("watch_addresses_captured", [])
            if f"0x{int(args.watch, 16):X}" not in captured:
                print("requested watch address is outside this capture's snapshots")
        elif f"0x{int(args.calls, 16):X}" not in manifest["traced"].get(
            "call_functions_captured", []
        ):
            print("requested function root is outside this capture's call-argument set")
    elif emitted >= limit:
        print(f"output capped at {limit} rows")
    print(f"manifest={store / 'manifest.json'} host_sha256={manifest['host_sha256']}")
    return 0


def cli(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.trace_store")
    sub = parser.add_subparsers(dest="action", required=True)
    capture_parser = sub.add_parser("capture")
    capture_parser.add_argument("--route", required=True, help="r5 or a recording path")
    capture_parser.add_argument("--switches", default="off")
    capture_parser.add_argument("--commit", help="full commit SHA of the shared prebuilt host")
    capture_parser.add_argument("--host-input-hash", help="pin the shared prebuilt host input hash")
    capture_parser.add_argument(
        "--seconds",
        type=int,
        default=None,
        help="optional wall cap; by default the capture runs to the recorded route end (T3388)",
    )
    capture_parser.add_argument("--max-dumps", type=int, default=5000)
    capture_parser.add_argument(
        "--max-bytes",
        type=int,
        default=24_000_000_000,
        help="capture output cap; story-level1 to its route end exceeds the 4 GB default (T3388)",
    )
    capture_parser.add_argument(
        "--render",
        choices=("auto", "gpu", "none"),
        help="T3390: rust_trace --render (default auto: headless none)",
    )
    capture_parser.add_argument("--save-dir", type=Path)
    capture_parser.add_argument("--replace", action="store_true")
    query_parser = sub.add_parser("query")
    query_parser.add_argument("--route", default="r5")
    query_parser.add_argument("--host-hash")
    query_parser.add_argument("--store")
    query_parser.add_argument(
        "--compare-store",
        help="compare this store's external call-args stream against another store directory",
    )
    choice = query_parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--watch", help="guest virtual address, e.g. 0x78AD48")
    choice.add_argument("--calls", help="function root address, e.g. 0x266250")
    query_parser.add_argument("--polls", type=poll_bounds, required=True)
    query_parser.add_argument("--bytes", type=int, default=16)
    query_parser.add_argument("--limit", type=int, default=QUERY_LIMIT)
    args = parser.parse_args(argv)
    try:
        return capture(args) if args.action == "capture" else query(args)
    except (OSError, ValueError, RuntimeError, KeyError) as error:
        print(f"trace_store: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(cli())
