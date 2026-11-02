"""T3405/T3449: compare all runnable Rust ports through the gate service for Tier 2."""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import io
import json
import os
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

from tools import adoption_gate, gate_data, owner_files
from tools.ledger import dispositions, execution
from tools.ledger.cli import main as ledger_main
from tools.ledger.store import read_snapshot
from tools.trace_store import main_root, route_path, tree_sha256
from tools.wt import cache_keys, prebuilt

ROOT = Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "docs/data/port-registry.json"
EVIDENCE = ROOT / "docs/data/tier2-evidence.json"
REGION_SETS = {
    "story-start": ROOT / "tools/data/rust_trace_regions_gameplay_core.json",
    "story-level1": ROOT / "tools/data/rust_trace_regions_gameplay.json",
    "arcade": ROOT / "tools/data/rust_trace_regions_gameplay_core.json",
    "mapmaker-editor": ROOT / "tools/data/rust_trace_regions_mapmaker_ui.json",
    # r5's NPC actor slots are intermittently absent in its whole-route trace;
    # compare the stable gameplay core while retaining NPC count/table coverage.
    "r5": ROOT / "tools/data/rust_trace_regions_gameplay_core.json",
}
ROUTES = ("story-start", "story-level1", "arcade", "mapmaker-editor", "r5")
ROUTE_FILES = {
    "story-start": "story-start-20261017.rec",
    "story-level1": "story-level1-20261018.rec",
    "arcade": "arcade-20261018.rec",
    "mapmaker-editor": "mapmaker-editor-20261018.rec",
    "r5": "r5-story-gameplay-20261016.rec",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def stable_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def verification_cache_key(cache_fields: dict[str, object]) -> str:
    """Build a Tier 2 result identity with the shared, versioned key model."""
    return cache_keys.make_key(
        "tier2-verify",
        files={},
        tools={},
        env={},
        extra=cache_fields,
    ).key


def resolve_route(name: str, owner_root: Path) -> tuple[str, Path, Path] | None:
    filename = ROUTE_FILES.get(name)
    if filename is None:
        label, recording = route_path(name, owner_root)
    else:
        bases = owner_files.recording_dirs(owner_root)
        recording = next((base / filename for base in bases if (base / filename).is_file()), None)
        if recording is None:
            label, recording = route_path(name, owner_root)
        else:
            label = recording.stem
    if not recording.is_file():
        return None
    saves = adoption_gate.route_save_dir(recording)
    if saves is None or not saves.is_dir():
        return None
    return label, recording.resolve(), saves.resolve()


def function_addresses() -> list[str]:
    data = json.loads(REGISTRY.read_text(encoding="utf-8"))
    return sorted(
        (
            f"{int(key.removeprefix('0x'), 16):08X}"
            for key, modules in data["ports"].items()
            if any(set(module.get("forms", [])) - {"doc"} for module in modules)
        ),
        key=lambda value: int(value, 16),
    )


def load_evidence() -> dict:
    if not EVIDENCE.is_file():
        return {"schema": 1, "functions": {}, "switches": {}, "interactions": []}
    data = json.loads(EVIDENCE.read_text(encoding="utf-8"))
    if data.get("schema") != 1 or not isinstance(data.get("functions"), dict):
        raise ValueError(f"unsupported Tier 2 evidence document: {EVIDENCE}")
    data.setdefault("switches", {})
    data.setdefault("interactions", [])
    return data


def append_once(rows: list[dict], item: dict) -> None:
    identity = (item.get("route"), item.get("prebuilt"), item.get("switch_set_sha256"))
    rows[:] = [
        row
        for row in rows
        if (row.get("route"), row.get("prebuilt"), row.get("switch_set_sha256")) != identity
    ]
    rows.append(item)


def save_evidence(data: dict) -> None:
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    temporary = EVIDENCE.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(EVIDENCE)


def gate_args(
    *,
    route: Path,
    regions: Path,
    switches: tuple[str, ...],
    commit: str,
    save_dir: Path,
    call_args: str,
    report: Path,
    seconds: int,
    max_bytes: int,
    no_cache: bool = False,
) -> SimpleNamespace:
    return SimpleNamespace(
        route=str(route),
        switches=",".join(switches),
        commit=commit,
        regions=str(regions),
        rust_ports=None,
        rea_ai_state=False,
        call_args=call_args,
        seconds=seconds,
        paced=False,
        render="none",
        save_dir=str(save_dir),
        host_arg=[],
        candidate_host_arg=[],
        candidate_env=[],
        stop_at_route_end=True,
        from_poll=0,
        headless=True,
        max_bytes=max_bytes,
        full_replay=False,
        snapshot_poll=0,
        report=str(report),
        classify_environment=True,
        priority="high",
        no_cache=no_cache,
    )


def configure_replay_slots(slots: int = 1) -> None:
    """Honor the active runner policy: at most two replays at once (owner 2026-10-20).

    The default stays one, the memory-safe policy T3405 chose; `--slots 2` is the ceiling."""
    count = max(1, min(2, slots))
    os.environ["TSFP_REPLAY_SLOTS"] = str(count)
    os.environ["TSFP_REPLAY_PRIORITY_SLOTS"] = str(count)


def failed_switches(result: dict, active: tuple[str, ...]) -> dict[str, str]:
    """Return only individually demonstrated failures, never interaction members.

    The combined adoption gate can report that two equal halves interact. That is
    evidence against the combination, not evidence that every member switch fails.
    """
    failures: dict[str, str] = {}
    for name, row in result.get("switches", {}).items():
        status = str(row.get("tier2", ""))
        if not name.startswith("interaction: ") and name in active and status != "EQUAL":
            failures[name] = status or "DIVERGED (no equal segment)"
    return failures


def equal_route_segments(result: dict) -> list[tuple[tuple[str, ...], dict]]:
    """Return every EQUAL candidate segment emitted by the combined gate."""
    segments = []
    for key, capture in result.get("captures", {}).items():
        switches = tuple(item for item in key.split(",") if item)
        if switches and capture.get("equal"):
            segments.append((switches, capture))
    return segments


def record_interactions(
    data: dict,
    *,
    result: dict,
    route: str,
    prebuilt_sha: str,
    switch_set_sha: str,
    report_path: Path,
) -> list[dict]:
    """Persist diverged combinations as interactions without failing their members."""
    recorded = []
    rows = data.setdefault("interactions", [])
    for name, row in result.get("switches", {}).items():
        if not name.startswith("interaction: "):
            continue
        switches = tuple(item for item in name.removeprefix("interaction: ").split(", ") if item)
        status = str(row.get("tier2", ""))
        if not switches or not status.startswith("DIVERGED"):
            continue
        capture = result.get("captures", {}).get(",".join(switches), {})
        item = {
            "status": "DIVERGED",
            "evidence_class": (
                "Tier 2 whole-system trace diverged (interaction; members not attributed)"
            ),
            "route": route,
            "prebuilt": prebuilt_sha,
            "switches": list(switches),
            "switch_set_sha256": switch_set_sha,
            "segment_switch_set_sha256": stable_hash(switches),
            "receipt_sha256": capture_receipt_hash(capture) if capture else None,
            "report": str(report_path),
            "result": status,
        }
        identity = (route, prebuilt_sha, switch_set_sha, switches)
        rows[:] = [
            old
            for old in rows
            if (
                old.get("route"),
                old.get("prebuilt"),
                old.get("switch_set_sha256"),
                tuple(old.get("switches", [])),
            )
            != identity
        ]
        rows.append(item)
        recorded.append(item)
    return recorded


def clear_superseded_switch_failures(
    data: dict, *, route: str, prebuilt_sha: str, switch_set_sha: str, equal_switches: set[str]
) -> None:
    """Remove earlier false member failures when this route proves them EQUAL alone."""
    switch_rows = data.setdefault("switches", {})
    for switch in equal_switches:
        row = switch_rows.get(switch)
        if not row:
            continue
        row["runs"] = [
            old
            for old in row.get("runs", [])
            if not (
                old.get("status") == "FAILED"
                and old.get("route") == route
                and old.get("prebuilt") == prebuilt_sha
                and old.get("switch_set_sha256") == switch_set_sha
            )
        ]
        if not row["runs"]:
            switch_rows.pop(switch, None)


def capture_receipt_hash(capture: dict) -> str:
    receipt = capture.get("receipt") or {}
    key, run_dir = receipt.get("key"), receipt.get("run_dir")
    if key and run_dir:
        record = Path(run_dir).parent.parent / f"{key}.json"
        if record.is_file():
            return sha256(record)
    return stable_hash(receipt or {"trace_dir": capture.get("trace_dir")})


def capture_receipt_path(capture: dict) -> str | None:
    receipt = capture.get("receipt") or {}
    key, run_dir = receipt.get("key"), receipt.get("run_dir")
    if key and run_dir:
        path = Path(run_dir).parent.parent / f"{key}.json"
        if path.is_file():
            return str(path)
    path = capture.get("receipt_path")
    return str(path) if isinstance(path, str) and path else None


def gate_equal_is_active(verdict: dict) -> tuple[bool, str | None]:
    """Reject EQUAL receipts without changing regions and at least one Rust root call."""
    samples = verdict.get("samples", {}).get("candidate", {})
    if not samples.get("varying_regions"):
        return False, "regions_inert"
    root_calls = verdict.get("root_calls") or {}
    if root_calls:
        selected = [count for count in root_calls.values() if isinstance(count, dict)]
        if selected and all(
            int(count.get("window") or 0) > 0 and int(count.get("rust_total") or 0) > 0
            for count in selected
        ):
            return True, None
        if any(int(count.get("window") or 0) <= 0 for count in selected):
            return False, "roots_not_called"
        return False, "roots_not_rust"
    trace_dir = verdict.get("candidate_run")
    if not trace_dir:
        return False, "root_calls_missing"
    trace_path = Path(trace_dir) / "trace.json"
    try:
        trace = json.loads(trace_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False, "root_calls_missing"
    rust_calls = sum(
        int(count.get("rust", 0))
        for count in trace.get("counters", {}).values()
        if isinstance(count, dict)
    )
    return (True, None) if rust_calls > 0 else (False, "roots_not_called")


def failed_receipt_hash(result: dict, switch: str) -> str | None:
    """Hash the selector's actual candidate receipt, never its status row."""
    capture = result.get("captures", {}).get(switch)
    return capture_receipt_hash(capture) if capture else None


def trace_polls(trace_dir: Path, from_poll: int) -> tuple[int, int] | None:
    path = trace_dir / "trace.json"
    if not path.is_file():
        raise ValueError(f"capture trace is missing: {path}")
    trace = json.loads(path.read_text(encoding="utf-8"))
    polls = [
        int(item["poll"])
        for item in trace.get("samples", [])
        if int(item.get("poll", 0)) >= from_poll
    ]
    return (min(polls), max(polls)) if polls else None


def executed_rust_functions(trace_dir: Path, registry: set[str]) -> dict[str, int]:
    trace = json.loads((trace_dir / "trace.json").read_text(encoding="utf-8"))
    executed: dict[str, int] = {}
    for raw, counts in trace.get("counters", {}).items():
        va = f"{int(raw.removeprefix('0x'), 16):08X}"
        if va not in registry:
            continue
        rust_calls = int(counts.get("rust", 0)) if isinstance(counts, dict) else 0
        if rust_calls > 0:
            executed["0x" + va] = rust_calls
    return executed


def record_failure(data: dict, switch: str, item: dict) -> None:
    row = data["switches"].setdefault(switch, {"runs": []})
    append_once(row["runs"], item)


def record_equal_functions(
    data: dict,
    *,
    trace_dir: Path,
    registry: set[str],
    base_capture: dict,
    candidate_capture: dict,
    route: str,
    prebuilt_sha: str,
    poll_range: list[int],
    switch_set_sha: str,
    regions: Path,
) -> set[str]:
    if candidate_capture.get("equal", True) is not True:
        return set()
    baseline_dir = Path(base_capture["trace_dir"])
    candidate_dir = Path(candidate_capture["trace_dir"])
    trace = json.loads((trace_dir / "trace.json").read_text(encoding="utf-8"))
    samples = trace.get("samples", [])
    if not samples:
        return set()
    if candidate_capture.get("control_status", "EQUAL") != "EQUAL":
        return set()
    if candidate_capture.get("fault_status", "fault-free") != "fault-free":
        return set()
    receipt_hash = candidate_capture.get("receipt_sha256") or capture_receipt_hash(
        candidate_capture
    )
    receipt_path = capture_receipt_path(candidate_capture)
    baseline_hash = base_capture.get("receipt_sha256") or capture_receipt_hash(base_capture)
    base_range = trace_polls(baseline_dir, poll_range[0])
    right_range = trace_polls(candidate_dir, poll_range[0])
    if base_range is None or right_range is None:
        return set()
    covered = [max(base_range[0], right_range[0]), min(base_range[1], right_range[1])]
    if covered[0] > covered[1]:
        return set()
    executed = executed_rust_functions(trace_dir, registry)
    residue = candidate_capture.get("stack_residue_excluded") or {}
    for va, calls in executed.items():
        functions = data["functions"].setdefault(va, {"runs": []})
        excluded = residue.get(va) or residue.get(f"0x{int(va, 16):X}") or {}
        append_once(
            functions["runs"],
            {
                "status": "EQUAL",
                "evidence_class": "Tier 2 whole-system trace EQUAL",
                "route": route,
                "prebuilt": prebuilt_sha,
                "poll_range": covered,
                "regions": [item["name"] for item in json.loads(regions.read_text())["regions"]],
                "regions_sha256": sha256(regions),
                "receipt_sha256": receipt_hash,
                **({"receipt_path": receipt_path} if receipt_path else {}),
                "baseline_receipt_sha256": baseline_hash,
                "switch_set_sha256": switch_set_sha,
                "rust_calls": calls,
                "samples": len(samples),
                "control_status": "EQUAL",
                "control_equal": True,
                "fault_status": "fault-free",
                "fault_count": 0,
                "nondeterministic_control": False,
                "cache_poisoned": False,
                # T3702: label dead-stack bytes and partial-window rows for this function
                **(
                    {
                        "stack_residue_rows": int(excluded["rows"]),
                        "stack_residue_partial_rows": int(excluded.get("partial_rows", 0)),
                        "stack_residue_addresses": list(excluded.get("addresses", [])),
                    }
                    if excluded.get("rows")
                    else {}
                ),
            },
        )
    return set(executed)


def run_route(
    *,
    route_name: str,
    recording: Path,
    saves: Path,
    commit: str,
    switches: tuple[str, ...],
    calls: tuple[str, ...],
    regions: Path,
    seconds: int,
    max_bytes: int,
    evidence: dict,
    owner_root: Path,
    no_cache: bool = False,
) -> dict:
    return run_route_service(
        route_name=route_name,
        recording=recording,
        saves=saves,
        commit=commit,
        switches=switches,
        regions=regions,
        seconds=seconds,
        evidence=evidence,
        owner_root=owner_root,
    )
    host = prebuilt.artifact_dir(ROOT, commit) / "tsfp-bevy"
    prebuilt_sha = sha256(host)
    recording_sha = sha256(recording)
    save_sha = tree_sha256(saves)
    switch_set_sha = stable_hash(switches)
    cache_fields = {
        "route": route_name,
        "recording_sha256": recording_sha,
        "save_sha256": save_sha,
        "prebuilt": prebuilt_sha,
        "switch_set_sha256": switch_set_sha,
        "regions_sha256": sha256(regions),
        "call_args_sha256": stable_hash(calls),
        "max_bytes": max_bytes,
        "seconds": seconds,
        "credit_policy": "equal-segments-v3-receipt-hash",
    }
    cache_key = verification_cache_key(cache_fields)
    cache_dir = gate_data.receipt_store(owner_root, "verify-all") / route_name
    cache_path = cache_dir / f"{cache_key}.json"
    if cache_path.is_file():
        cached = json.loads(cache_path.read_text(encoding="utf-8"))
        if cached.get("cache_fields") == cache_fields and cached.get("status") in (
            "EQUAL",
            "DIVERGED",
        ):
            from tools.cache_telemetry import record_lookup

            record_lookup("tier2-verify", True)
            return {**cached, "cache_hit": True}
    from tools.cache_telemetry import record_lookup

    record_lookup("tier2-verify", False)

    active = switches
    failed: dict[str, str] = {}
    gate_result = None
    round_no = 0
    while active:
        round_no += 1
        report_path = cache_dir / f"{cache_key}-round-{round_no}.md"
        args = gate_args(
            route=recording,
            regions=regions,
            switches=active,
            commit=commit,
            save_dir=saves,
            call_args=",".join(calls),
            report=report_path,
            seconds=seconds,
            max_bytes=max_bytes,
            no_cache=no_cache,
        )
        gate_result = adoption_gate.adoption_gate(args)
        full_key = ",".join(active)
        full_capture = gate_result.get("captures", {}).get(full_key)
        if full_capture and full_capture.get("equal"):
            break
        newly_failed = failed_switches(gate_result, active)
        for switch, status in newly_failed.items():
            failed[switch] = status
            record_failure(
                evidence,
                switch,
                {
                    "status": "FAILED",
                    "evidence_class": "Tier 2 whole-system trace diverged",
                    "route": route_name,
                    "prebuilt": prebuilt_sha,
                    "switch_set_sha256": switch_set_sha,
                    "switch": switch,
                    "result": status,
                    "report": str(report_path),
                    "receipt_sha256": failed_receipt_hash(gate_result, switch),
                },
            )
        remaining = tuple(item for item in active if item not in newly_failed)
        if remaining == active:
            break
        active = remaining

    equal_segments = equal_route_segments(gate_result or {})
    equal_switch_set = {
        name
        for name, row in (gate_result or {}).get("switches", {}).items()
        if name in switches and row.get("tier2") == "EQUAL"
    }
    clear_superseded_switch_failures(
        evidence,
        route=route_name,
        prebuilt_sha=prebuilt_sha,
        switch_set_sha=switch_set_sha,
        equal_switches=equal_switch_set,
    )
    interactions = (
        record_interactions(
            evidence,
            result=gate_result or {},
            route=route_name,
            prebuilt_sha=prebuilt_sha,
            switch_set_sha=switch_set_sha,
            report_path=report_path,
        )
        if gate_result
        else []
    )

    equal = bool(
        gate_result
        and active
        and gate_result.get("captures", {}).get(",".join(active), {}).get("equal")
    )
    credited_functions: set[str] = set()
    credited_segments = []
    poll_from = int((gate_result or {}).get("from_poll") or 0)
    candidate = (gate_result or {}).get("captures", {}).get(",".join(active), {})
    baseline = (gate_result or {}).get("baseline_capture", {})
    if baseline:
        registry = set(function_addresses())
        for segment_switches, segment in equal_segments:
            candidate_dir = Path(segment["trace_dir"])
            candidate_trace = json.loads((candidate_dir / "trace.json").read_text(encoding="utf-8"))
            polls = [
                int(sample["poll"])
                for sample in candidate_trace.get("samples", [])
                if int(sample.get("poll", 0)) >= poll_from
            ]
            if not polls:
                continue
            segment_switch_sha = stable_hash(segment_switches)
            credited_functions.update(
                record_equal_functions(
                    evidence,
                    trace_dir=candidate_dir,
                    registry=registry,
                    base_capture=baseline,
                    candidate_capture=segment,
                    route=route_name,
                    prebuilt_sha=prebuilt_sha,
                    poll_range=[min(polls), max(polls)],
                    switch_set_sha=segment_switch_sha,
                    regions=regions,
                )
            )
            credited_segments.append(
                {
                    "switches": list(segment_switches),
                    "receipt_sha256": capture_receipt_hash(segment),
                }
            )
    result = {
        "schema": 1,
        "status": "EQUAL" if equal else "DIVERGED",
        "route": route_name,
        "recording_sha256": recording_sha,
        "save_sha256": save_sha,
        "prebuilt": prebuilt_sha,
        "switches": list(switches),
        "switch_set_sha256": switch_set_sha,
        "active_after_bisection": list(active),
        "failed_switches": failed,
        "interaction_failures": [item["switches"] for item in interactions],
        "from_poll": poll_from,
        "candidate": candidate,
        "baseline": baseline,
        "credited_functions": len(credited_functions),
        "credited_segments": credited_segments,
        "cache_fields": cache_fields,
        "cache_hit": False,
    }
    cache_dir.mkdir(parents=True, exist_ok=True)
    temporary = cache_path.with_suffix(".tmp")
    temporary.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(cache_path)
    return result


def gate_service(
    *, route: Path, regions: Path, switches: tuple[str, ...], seconds: int, commit: str
) -> dict:
    """Run a route segment through the supported Tier 2 gate entry point."""
    snapshot_stems = json.loads(
        (ROOT / "tools/data/gameplay_start_polls.json").read_text(encoding="utf-8")
    )
    use_snapshot = route.stem in snapshot_stems
    start = "gameplay" if use_snapshot else "boot"
    command = [
        sys.executable,
        "-m",
        "tools.gate",
        "--profile",
        "verify-all",
        "--commit",
        commit,
        "--route",
        str(route),
        "--start",
        start,
        "--regions",
        str(regions),
        "--switches",
        ",".join(switches),
        "--seconds",
        str(seconds),
        "--no-adopted",
    ]
    if use_snapshot:
        command.append("--snapshot")
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"tools.gate returned no JSON verdict (exit {completed.returncode}): "
            f"{completed.stderr[-1000:]}"
        ) from error
    result["gate_exit_code"] = completed.returncode
    return result


def run_route_service(
    *,
    route_name: str,
    recording: Path,
    saves: Path,
    commit: str,
    switches: tuple[str, ...],
    regions: Path,
    seconds: int,
    evidence: dict,
    owner_root: Path,
) -> dict:
    """Gate all ports, then bisect divergence with cached gate-service segments."""
    host = prebuilt.artifact_dir(ROOT, commit) / "tsfp-bevy"
    prebuilt_sha = sha256(host)
    recording_sha = sha256(recording)
    save_sha = tree_sha256(saves)
    switch_set_sha = stable_hash(switches)
    cache_fields = {
        "route": route_name,
        "recording_sha256": recording_sha,
        "save_sha256": save_sha,
        "prebuilt": prebuilt_sha,
        "switch_set_sha256": switch_set_sha,
        "regions_sha256": sha256(regions),
        "seconds": seconds,
        "gate_profile": "verify-all",
        "credit_policy": "gate-service-equal-segments-v2-non-vacuous",
    }
    cache_key = verification_cache_key(cache_fields)
    cache_dir = gate_data.receipt_store(owner_root, "verify-all") / route_name
    cache_path = cache_dir / f"{cache_key}.json"
    if cache_path.is_file():
        cached = json.loads(cache_path.read_text(encoding="utf-8"))
        if cached.get("cache_fields") == cache_fields and cached.get("status") in (
            "EQUAL",
            "DIVERGED",
            "INVALID",
        ):
            from tools.cache_telemetry import record_lookup

            record_lookup("tier2-verify", True)
            return {**cached, "cache_hit": True}
    from tools.cache_telemetry import record_lookup

    record_lookup("tier2-verify", False)

    queue = [switches]
    equal_segments: list[tuple[tuple[str, ...], dict]] = []
    failures: dict[str, dict] = {}
    interactions: list[dict] = []
    gate_runs: list[dict] = []
    first_verdict: dict | None = None
    while queue:
        active = queue.pop(0)
        verdict = gate_service(
            route=recording,
            regions=regions,
            switches=active,
            seconds=seconds,
            commit=commit,
        )
        if verdict.get("verdict") == "EQUAL":
            candidate_active, reason = gate_equal_is_active(verdict)
            if not candidate_active:
                verdict["verdict"] = "VACUOUS"
                verdict.setdefault("reasons", []).append(reason or "inactive_candidate")
                verdict["message"] = (
                    f"EQUAL trace did not prove candidate execution and active regions: {reason}"
                )
        if first_verdict is None:
            first_verdict = verdict
        receipt_path = verdict.get("receipt")
        receipt_sha = (
            sha256(Path(receipt_path)) if receipt_path and Path(receipt_path).is_file() else None
        )
        gate_runs.append(
            {
                "switches": list(active),
                "verdict": verdict.get("verdict", "INVALID"),
                "receipt": receipt_path,
                "receipt_sha256": receipt_sha,
                "first_divergence": verdict.get("first_divergence"),
                "reasons": verdict.get("reasons", []),
            }
        )
        if verdict.get("verdict") == "EQUAL":
            equal_segments.append((active, {**verdict, "receipt_sha256": receipt_sha}))
        elif verdict.get("verdict") == "DIVERGED" and len(active) > 1:
            midpoint = len(active) // 2
            queue.extend((active[:midpoint], active[midpoint:]))
            interactions.append(
                {
                    "status": "DIVERGED",
                    "evidence_class": "Tier 2 whole-system trace diverged (switch set)",
                    "route": route_name,
                    "prebuilt": prebuilt_sha,
                    "switches": list(active),
                    "switch_set_sha256": stable_hash(active),
                    "receipt_sha256": receipt_sha,
                    "first_divergence": verdict.get("first_divergence"),
                    "reasons": verdict.get("reasons", []),
                }
            )
        elif verdict.get("verdict") == "DIVERGED":
            switch = active[0]
            failure = {
                "status": "FAILED",
                "evidence_class": "Tier 2 whole-system trace diverged",
                "route": route_name,
                "prebuilt": prebuilt_sha,
                "switch": switch,
                "switch_set_sha256": switch_set_sha,
                "segment_switch_set_sha256": stable_hash(active),
                "receipt_sha256": receipt_sha,
                "first_divergence": verdict.get("first_divergence"),
                "reasons": verdict.get("reasons", []),
            }
            failures[switch] = failure
            rows = evidence.setdefault("switches", {}).setdefault(switch, {"runs": []})["runs"]
            append_once(rows, failure)

    registry = set(function_addresses())
    credited_functions: set[str] = set()
    credited_segments = []
    for segment_switches, verdict in equal_segments:
        candidate_dir = Path(verdict["candidate_run"])
        baseline_dir = Path(verdict["baseline_run"])
        receipt_sha = verdict["receipt_sha256"]
        candidate_trace = json.loads((candidate_dir / "trace.json").read_text(encoding="utf-8"))
        polls = [
            int(sample["poll"])
            for sample in candidate_trace.get("samples", [])
            if int(sample.get("poll", 0)) >= int(verdict["start_poll"])
        ]
        if not polls or not receipt_sha:
            continue
        segment_sha = stable_hash(segment_switches)
        credited_functions.update(
            record_equal_functions(
                evidence,
                trace_dir=candidate_dir,
                registry=registry,
                base_capture={"trace_dir": str(baseline_dir), "receipt_sha256": receipt_sha},
                candidate_capture={"trace_dir": str(candidate_dir), "receipt_sha256": receipt_sha},
                route=route_name,
                prebuilt_sha=prebuilt_sha,
                poll_range=[min(polls), max(polls)],
                switch_set_sha=segment_sha,
                regions=regions,
            )
        )
        credited_segments.append(
            {
                "switches": list(segment_switches),
                "receipt_sha256": receipt_sha,
                "poll_range": [min(polls), max(polls)],
            }
        )

    full_equal = bool(first_verdict and first_verdict.get("verdict") == "EQUAL")
    status = (
        "EQUAL"
        if full_equal
        else (
            "DIVERGED"
            if failures or interactions
            else "VACUOUS"
            if first_verdict and first_verdict.get("verdict") == "VACUOUS"
            else "INVALID"
        )
    )
    for interaction in interactions:
        rows = evidence.setdefault("interactions", [])
        identity = (interaction["route"], interaction["prebuilt"], interaction["switches"])
        rows[:] = [
            row
            for row in rows
            if (row.get("route"), row.get("prebuilt"), row.get("switches")) != identity
        ]
        rows.append(interaction)
    result = {
        "schema": 1,
        "status": status,
        "reason": (first_verdict or {}).get("message")
        or ", ".join((first_verdict or {}).get("reasons", [])),
        "first_divergence": (first_verdict or {}).get("first_divergence"),
        "route": route_name,
        "recording_sha256": recording_sha,
        "save_sha256": save_sha,
        "prebuilt": prebuilt_sha,
        "switches": list(switches),
        "switch_set_sha256": switch_set_sha,
        "failed_switches": failures,
        "interaction_failures": [item["switches"] for item in interactions],
        "gate_runs": gate_runs,
        "credited_functions": len(credited_functions),
        "credited_segments": credited_segments,
        "cache_fields": cache_fields,
        "cache_hit": False,
    }
    # Persist evidence before the route cache. If the process exits between these writes,
    # append_once makes a retry safe and the gate service reuses its immutable receipts.
    save_evidence(evidence)
    cache_dir.mkdir(parents=True, exist_ok=True)
    temporary = cache_path.with_suffix(".tmp")
    temporary.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(cache_path)
    return result


def run(args: argparse.Namespace) -> int:
    # The runner's current memory policy admits one capture at a time. Keep the
    # verifier within that ceiling even when the parent shell exports more slots.
    configure_replay_slots(args.slots)
    owner_root = main_root()
    if args.commit:
        commit = args.commit
        host = prebuilt.artifact_dir(ROOT, commit) / "tsfp-bevy"
    else:
        from tools import prebuilt_select

        primary = prebuilt.primary_commit(ROOT)
        host = prebuilt_select.find_host(ROOT, commit=primary)
        commit = host.parent.name if host else primary
    if host is None or not prebuilt.available(host):
        raise ValueError(
            f"no registered prebuilt matches the primary build inputs: {host}; "
            "run tools.wt prebuilt build only if this task changes host build inputs"
        )
    raw_switches = (
        tuple(args.switches.split(","))
        if args.switches
        else adoption_gate.switch_set_from_owner_checks()
    )
    unique_switches = set(item.strip() for item in raw_switches if item.strip())
    switches = tuple(
        sorted(item for item in unique_switches if not item.startswith("TSFP_"))
    ) + tuple(sorted(item for item in unique_switches if item.startswith("TSFP_")))
    # The trace regions are the Tier 2 comparison surface. Call-args output is
    # intentionally disabled for bulk runs because it can be many gigabytes;
    # inspect a single target separately when a bisection needs it.
    calls: tuple[str, ...] = ()
    selected = tuple(item.strip() for item in args.routes.split(",") if item.strip())
    evidence = load_evidence()
    before = ledger_summary(owner_root)
    route_rows = []
    # One lease for the whole matrix: the shared cache GC may otherwise drop the host between
    # routes (T3449 saw a registered prebuilt vanish before the first route started). Process
    # exit releases the flock, so no try/finally is needed on the error paths.
    held = contextlib.ExitStack()
    held.enter_context(prebuilt.lease(host))
    for name in selected:
        resolved = resolve_route(name, owner_root)
        if resolved is None:
            route_rows.append(
                {
                    "route": name,
                    "status": "SKIPPED",
                    "reason": "recording or bound saves unavailable",
                }
            )
            continue
        label, recording, saves = resolved
        if args.runner == "gate":
            result = run_route_service(
                route_name=label,
                recording=recording,
                saves=saves,
                commit=commit,
                switches=switches,
                regions=REGION_SETS.get(name, ROOT / "tools/data/rust_trace_regions_gameplay.json"),
                seconds=args.seconds,
                evidence=evidence,
                owner_root=owner_root,
            )
        else:
            with prebuilt.lease(host):
                result = run_route(
                    route_name=label,
                    recording=recording,
                    saves=saves,
                    commit=commit,
                    switches=switches,
                    calls=calls,
                    regions=REGION_SETS.get(
                        name, ROOT / "tools/data/rust_trace_regions_gameplay.json"
                    ),
                    seconds=args.seconds,
                    max_bytes=args.max_bytes,
                    evidence=evidence,
                    owner_root=owner_root,
                    no_cache=args.no_cache,
                )
        route_rows.append(result)
        save_evidence(evidence)
        # T3449: credit reaches the ledger as soon as a route is credited, not only at the
        # end of the matrix (a timed-out matrix used to leave the snapshot behind the evidence).
        route_rows[-1]["port_tier2_after_route"] = ledger_summary(owner_root)
    held.close()
    out = (
        Path(args.out).expanduser().resolve()
        if args.out
        else gate_data.receipt_store(owner_root, "verify-all") / "latest.json"
    )
    out.parent.mkdir(parents=True, exist_ok=True)
    after = ledger_summary(owner_root)
    report = {
        "schema": 1,
        "evidence_class": "Tier 2 whole-system trace EQUAL over covered regions and polls",
        "prebuilt_commit": commit,
        "prebuilt_sha256": sha256(host),
        "switches": list(switches),
        "switch_set_sha256": stable_hash(switches),
        "call_args_functions": len(calls),
        "routes": route_rows,
        "port_tier2_before": before,
        "port_tier2_after": after,
    }
    out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "report": str(out),
                "routes": [
                    {
                        "route": row["route"],
                        "status": row["status"],
                        "cache_hit": row.get("cache_hit", False),
                        "credited_functions": row.get("credited_functions", 0),
                        "reason": row.get("reason"),
                    }
                    for row in route_rows
                ],
            },
            sort_keys=True,
        )
    )
    return 0 if all(row.get("status") in ("EQUAL", "SKIPPED") for row in route_rows) else 2


def ledger_refresh_args(owner_root: Path) -> list[str]:
    """Use the shared main checkout's gitignored function-to-subsystem table."""
    return ["refresh", "--subsystems", str(owner_root / "tools/data/function_subsystems.csv")]


def ledger_summary(owner_root: Path) -> dict:
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        # The placement table is gitignored and belongs to the shared main
        # checkout. A worktree-local relative default is absent in clean trees.
        if ledger_main(ledger_refresh_args(owner_root)) != 0:
            raise RuntimeError("tools.ledger refresh failed")
        if (
            ledger_main(
                ["breadth", "--scope", "port", "--snapshot", str(owner_root / "generated/ledger")]
            )
            != 0
        ):
            raise RuntimeError("tools.ledger breadth --scope port failed")
    rows = read_snapshot(owner_root / "generated/ledger")
    weights = execution.load(ROOT / "docs/data/t1884-exec-weights.csv") or execution.Weights()
    table = dispositions.load(ROOT / "tools/data/system_dispositions.json")
    executed = [
        (va, row)
        for va, row in rows.items()
        if weights.calls.get(int(va.removeprefix("0x"), 16), 0) > 0
        and dispositions.disposition_of(str(row.get("subsystem", "unknown")), table) == "port"
    ]
    tier2 = sum(int(row.get("verification_tier") or 0) >= 2 for _, row in executed)
    executed_calls = sum(weights.calls[int(va.removeprefix("0x"), 16)] for va, _ in executed)
    tier2_calls = sum(
        weights.calls[int(va.removeprefix("0x"), 16)]
        for va, row in executed
        if int(row.get("verification_tier") or 0) >= 2
    )
    return {
        "tier2_functions": tier2,
        "executed_port_functions": len(executed),
        "tier2_share": tier2 / len(executed) if executed else 0.0,
        "tier2_calls": tier2_calls,
        "executed_port_calls": executed_calls,
        "tier2_call_share": tier2_calls / executed_calls if executed_calls else 0.0,
        "breadth_output": output.getvalue().splitlines()[-8:],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--routes", default=",".join(ROUTES))
    parser.add_argument("--switches", help="override combined owner-check switch set")
    parser.add_argument("--commit", help="full SHA of a registered prebuilt")
    parser.add_argument("--seconds", type=int, default=900)
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=0,
        help="maximum bytes per capture; 0 disables the rust_trace cap for large all-port receipts",
    )
    parser.add_argument(
        "--runner",
        choices=("gate", "adoption"),
        default="gate",
        help="gate: cached tools.gate service segments (policy default); adoption: the "
        "adoption_gate runner that produced the T3449 evidence (same comparator)",
    )
    parser.add_argument(
        "--no-cache",
        action="store_true",
        help="force fresh replays: the queue otherwise serves a cached FAIL verdict again",
    )
    parser.add_argument(
        "--slots", type=int, default=1, help="concurrent replays, 1 (default) or 2 (the ceiling)"
    )
    parser.add_argument(
        "--out",
        help="JSON report path; default tmp/wt-cache/receipts/verify-all/latest.json",
    )
    args = parser.parse_args(argv)
    try:
        return run(args)
    except (OSError, ValueError, RuntimeError, prebuilt.PrebuiltError) as error:
        print(f"verify-all: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
