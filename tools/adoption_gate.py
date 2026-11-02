"""Run cached-baseline Tier 2 adoption gates through the serialized replay queue."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import threading
import time
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from tools import datadir, gate, gate_data, owner_files, prebuilt_select
from tools.play.rec_saves import header_saves, tree_hash
from tools.play.rec_saves import locate as locate_saves
from tools.trace_store import main_root, route_path, sha256
from tools.wt import cache_keys, prebuilt, replay

ROOT = Path(__file__).resolve().parents[1]
REPORT_NAME = Path("adoption-gate") / "report.md"  # under the derived-data dir (tools.datadir)
DEFAULT_REPORT = Path("tmp") / REPORT_NAME
DEFAULT_REGIONS = ROOT / "tools/data/rust_trace_regions_gameplay.json"
OWNER_CHECKS = ROOT / "docs/owner-checks.md"
HOST_ONLY_SWITCHES = frozenset(("rust-rea-ai", "rust-weapon-hands", "rea-inventory-native"))
MIN_COMPARISON_SAMPLES = 600
WINDOW_MAX_IDLE = 30000


def _host_or_wait(commit: str, host_input_hash: str) -> Path | None:
    """T3953: a host whose build is in flight (prewarm or a lane) is waited for."""
    from tools.wt import prewarm

    cache = prebuilt.cache_root(ROOT)

    def find() -> Path | None:
        host = prebuilt_select.find_host(ROOT, commit=commit, host_input_hash=host_input_hash)
        return host if prebuilt_select.available(host) else None

    return prewarm.wait_for_host(
        find, lambda: prebuilt.read_inflight(cache, host_input_hash) is not None
    )


def missing_host_message(commit: str, host_input_hash: str) -> str:
    """I-T3505-8: no published prebuilt matches the commit's build inputs (find_host is None)."""
    return (
        f"no shared prebuilt host for {commit[:12]} (build inputs {host_input_hash[:12]}); "
        f"build it with `python3 -m tools.wt prebuilt build --commit {commit}` (remote-first) "
        "and rerun"
    )


def _capture_max_idle(from_poll: int, rea_ai_gate: bool) -> int | None:
    """Let AI sidecar gates reach the route end instead of the 800-dump idle default."""
    return WINDOW_MAX_IDLE if from_poll or rea_ai_gate else None


def _switches_with_port_set(switches: tuple[str, ...]) -> tuple[str, ...]:
    """Keep host-only toggles and environment checks attached to a Rust port set."""
    selected = set(switches)
    if "rust-rea-ai" in selected and "all" not in selected and "0x266250" not in selected:
        # The REA AI host adapter validates that its tick root is selected. Keep this
        # dependency attached to the switch during combined captures and bisection.
        return (*tuple(item for item in switches if item != "off"), "0x266250")
    if all(item in HOST_ONLY_SWITCHES or item.startswith("TSFP_") for item in switches):
        return ("off", *switches)
    return switches


def _baseline_switches(switches: tuple[str, ...]) -> tuple[str, ...]:
    """Apply shared replay environment to the control capture as well as candidates."""
    return ("off", *(item for item in switches if item.startswith("TSFP_")))


def _candidate_host_args(
    shared: tuple[str, ...], candidate_only: tuple[str, ...]
) -> tuple[str, ...]:
    """Keep adapter A/B selectors off the lifted reference capture."""
    return (*shared, *candidate_only)


def _queue_failure_detail(result: subprocess.CompletedProcess[str], receipt: dict | None) -> str:
    """Keep a failed queued capture actionable, including its guest stop when available."""
    details = [f"returncode={result.returncode}"]
    if receipt is not None:
        for key in ("exit_code", "verdict", "run_dir"):
            if receipt.get(key) is not None:
                details.append(f"{key}={receipt[key]}")
        run_dir = receipt.get("run_dir")
        if isinstance(run_dir, str):
            try:
                run_log = (Path(run_dir) / "run.log").read_text(errors="replace")
            except OSError:
                run_log = ""
            stops = [
                line.strip()
                for line in run_log.splitlines()
                if "STOPPED:" in line or "faulted:" in line.lower()
            ]
            if stops:
                details.append(f"guest_stop={stops[-1][-500:]}")
    for label, output in (("stderr", result.stderr), ("stdout", result.stdout)):
        if output:
            details.append(f"{label}={output[-500:].strip()}")
    return "; ".join(details)


def _split_shared_environment(switches: tuple[str, ...]) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Keep common replay environment out of per-candidate A/B bisection."""
    shared = tuple(item for item in switches if item.startswith("TSFP_"))
    candidates = tuple(item for item in switches if not item.startswith("TSFP_"))
    return candidates, shared


# T3449: the named Rust port groups the host accepts in `--rust-ports` (crates/tsfp-ports
# runtime.rs `group_lists`, plus the `weapons` and `player` tokens of src/host/rust_ports.c).
# The owner-checks doc names only 0x658d0, so before this the "all-ports-on" candidate of the
# Tier 2 runner exercised two registered functions (0x658d0 and the 0x266250 AI root).
RUST_PORT_GROUPS = (
    "anim",
    "physics",
    "char",
    "npc",
    "inventory",
    "cutscene",
    "save",
    "audio",
    "render",
    "hud",
    "input",
    "object",
    "pose",
    "leaf",
    "modes",
    "state",
    "weapons",
    "player",
)


def switch_set_from_owner_checks(path: Path = OWNER_CHECKS) -> tuple[str, ...]:
    """Build a reproducible combined candidate set from documented owner switches."""
    source = path.read_text()
    switches: set[str] = {"rust-rea-ai", "rust-weapon-hands", *RUST_PORT_GROUPS}
    env: set[str] = {
        "TSFP_STORY_SEQUENCER=rust",
        "TSFP_REA_OBJECT_NATIVE=1",
        "TSFP_REA_INVENTORY_NATIVE=1",
        "TSFP_MAPMAKER_HUD_PANELS=1",
        "TSFP_MAPMAKER_OVERLAY=1",
        "TSFP_MAPMAKER_UI=1",
        "TSFP_UI_MAPMAKER_CONTROLS=1",
        "TSFP_UI_MAPMAKER_HANDLERS=1",
        "TSFP_UI_MAPMAKER_HISTORY=1",
    }
    for match in re.finditer(r"--rust-ports=([^\s`;&]+)", source):
        switches.update(value for value in match.group(1).split(",") if value)
    # `--rust-rea-ai-enable=VA` and similar take a value: they are not standalone selectors.
    for match in re.finditer(r"--(rust-[a-z0-9-]+)(?![a-z0-9=-])", source):
        if match.group(1) != "rust-ports":
            switches.add(match.group(1))
    for match in re.finditer(r"(TSFP_[A-Z0-9_]+)=([^\s`;&]+)", source):
        env.add(f"{match.group(1)}={match.group(2)}")
    # T3702: the gate refuses registry-only roots (e.g. 0x658d0, a `roots` form with no selectable
    # `fn` body) as selectors, so the default set derives from the gate's own registry check.
    switches -= set(gate.registry_only_selectors(tuple(switches)))
    return tuple(sorted(switches | env))


def worktree_disc(checkout: Path) -> Path | None:
    """T3449: the owner ISO for a worktree that lacks the `tmp/` symlink.

    `checkout` is where the queue command runs (this worktree, `ROOT`): `replay submit` resolves
    its default disc against that directory, whatever primary checkout the gate selects hosts and
    routes from. Returns None when the checkout has its own copy, so main-checkout replays keep
    their cache keys (the disc hash enters the key only when `--disc` is passed). Otherwise
    `TSFP_DISC`, then the main checkout's `tmp/` copy."""
    if any((checkout / relative).is_file() for relative in owner_files.DISC_LOCATIONS):
        return None
    disc = owner_files.disc_image(checkout)
    return disc if disc.is_file() else None


def queue_submit(
    root: Path,
    route: str,
    switches: tuple[str, ...],
    commit: str,
    regions: Path,
    seconds: int,
    no_cache: bool,
    allow_capture_failure: bool = False,
    paced: bool = False,
    save_dir: Path | None = None,
    host_args: tuple[str, ...] = (),
    stop_at_route_end: bool = False,
    no_stop_at_route_end: bool = False,
    rust_ports: str | None = None,
    rea_ai_state: str | None = None,
    call_args: str | None = None,
    render: str | None = None,
    snapshot: tuple[int, Path] | None = None,
    snapshot_polls: tuple[list[int], Path] | None = None,
    headless: bool = False,
    max_idle: int | None = None,
    max_dumps: int = 12000,
    candidate_environment: tuple[str, ...] = (),
    priority: str | None = "normal",
    max_bytes: int | None = None,
    host_input_hash: str | None = None,
    disc: Path | None = None,
    anim_shadow: str | None = None,
) -> dict:
    candidate_switches = tuple(
        item for item in switches if not (candidate_environment and item == "off")
    )
    switches_for_replay = _switches_with_port_set(candidate_switches) if candidate_switches else ()
    selected_switches = [item for item in switches_for_replay if not item.startswith("TSFP_")]
    if rust_ports:
        # Replace the helper's `off` control selector only when a concrete port
        # set is supplied. Environment A/B switches still need `off` attached
        # when the bisection group contains host-only selectors.
        selected_switches = [item for item in selected_switches if item != "off"]
    if rust_ports and rust_ports not in selected_switches:
        selected_switches.append(rust_ports)
    args = [
        sys.executable,
        "-m",
        "tools.wt",
        "replay",
        "submit",
        "--route",
        route,
        "--switches",
        ",".join(selected_switches) or "off",
        "--commit",
        commit,
        "--regions",
        str(regions),
        "--seconds",
        str(seconds),
        "--max-dumps",
        str(max_dumps),
    ]
    flat = list(args)
    pinned_hash = host_input_hash or prebuilt_select.build_input_hash(root, commit)
    flat.extend(("--host-input-hash", pinned_hash))
    if save_dir is not None:
        flat.extend(("--save-dir", str(save_dir)))
    if disc is None:
        disc = worktree_disc(ROOT)
    if disc is not None:
        flat.extend(("--disc", str(disc)))  # T3416: worktrees lack the ISO symlink
    for item in switches:
        if item.startswith("TSFP_"):
            flat.extend(("--env", item))
    for item in candidate_environment:
        flat.extend(("--env", item))
    flat.extend(f"--host-arg={item}" for item in host_args)
    if anim_shadow:  # T3587: "off" = the plain ports, as play runs them (capture default is on)
        flat.extend(("--anim-shadow", anim_shadow))
    if stop_at_route_end and no_stop_at_route_end:
        raise ValueError("conflicting route-end options")
    if no_stop_at_route_end:
        flat.append("--no-stop-at-route-end")
    if stop_at_route_end:
        flat.append("--stop-at-route-end")
    if rea_ai_state:
        flat.extend(("--rea-ai-state", rea_ai_state))
    if call_args:
        flat.extend(("--call-args", call_args))
    if no_cache:
        flat.append("--no-cache")
    if priority:
        flat.extend(("--priority", priority))
    if paced:
        flat.append("--paced")
    if render:
        flat.extend(("--render", render))  # T3390: passed through to rust_trace
    if max_idle is not None:
        # Idle dumps are the trace samples; the default cap (800) ends a trace near poll 3200.
        flat.extend(("--max-idle", str(max_idle)))
    if max_bytes is not None:
        flat.extend(("--max-bytes", str(max_bytes)))
    if headless and snapshot is None and snapshot_polls is None:
        flat.append("--headless")
    if snapshot is not None:
        poll, snapshot_dir = snapshot
        flat.extend(
            (
                "--headless",
                "--snapshot-at-poll",
                str(poll),
                "--snapshot-dir",
                str(snapshot_dir),
                "--snapshot-continue",
            )
        )
    if snapshot_polls is not None:
        polls, snapshot_dir = snapshot_polls
        flat.extend(
            (
                "--snapshot-at-polls",
                ",".join(map(str, polls)),
                "--snapshot-dir",
                str(snapshot_dir),
            )
        )
    # Run the queue CLI from this worktree so an unmerged --no-cache/--env
    # implementation is exercised by the gate itself. Routes and host paths
    # are absolute or independently resolved against the primary checkout.
    result = subprocess.run(flat, cwd=ROOT, check=False, text=True, capture_output=True)
    receipt = next(
        (
            json.loads(line)
            for line in reversed(result.stdout.splitlines())
            if line.startswith("{") and '"run_dir"' in line
        ),
        None,
    )
    if result.returncode and not receipt:
        raise RuntimeError(f"replay queue command failed: {_queue_failure_detail(result, receipt)}")
    if receipt is not None and receipt.get("exit_code"):
        # Preserve a bounded explanation when callers explicitly allow failed
        # captures (segmented gates need to collect every receipt before
        # reporting). Without this, queue_submit discarded the host's stderr
        # and the only evidence was an opaque exit_code=1.
        output = [*result.stdout.splitlines(), *result.stderr.splitlines()]
        receipt["capture_output_tail"] = [line[-1000:] for line in output[-12:]]
    if receipt is None or (receipt.get("exit_code") and not allow_capture_failure):
        raise RuntimeError(f"replay capture failed: {_queue_failure_detail(result, receipt)}")
    return receipt


def _trace_has_regions(trace_path: Path, region_names: set[str], min_samples: int = 1) -> bool:
    if not trace_path.is_file():
        return False
    trace = json.loads(trace_path.read_text())
    present = {name for sample in trace.get("samples", []) for name in sample.get("regions", {})}
    return len(trace.get("samples", [])) >= min_samples and region_names <= present


def _capture_empty_reason(trace_path: Path, region_items: list[dict]) -> str | None:
    """Classify a successful capture with no trace samples without opening bulk traces."""
    if not region_items:
        return "regions_empty"
    if not trace_path.is_file():
        return "writer_error"
    try:
        if trace_path.stat().st_size > 256 * 1024 * 1024:
            return None
        trace = json.loads(trace_path.read_text())
    except (OSError, json.JSONDecodeError):
        return "writer_error"
    if trace.get("samples"):
        return None
    if trace.get("meta", {}).get("route_end_poll"):
        return "route_end"
    return "writer_error"


def _trace_supports_comparison(trace_path: Path, region_items: list[dict]) -> bool:
    """Reject short, incorrectly sized, or inert traces before comparing them."""
    if not region_items:
        return False
    names = {item["name"] for item in region_items}
    if not _trace_has_regions(trace_path, names, MIN_COMPARISON_SAMPLES):
        return False
    samples = json.loads(trace_path.read_text())["samples"]
    changing_nonzero_region = False
    for item in region_items:
        width_text = item.get("spec", "").rpartition(":")[2]
        try:
            radix = 16 if width_text.lower().startswith("0x") else 10
            expected_width = int(width_text, radix)
        except ValueError:
            return False
        if expected_width <= 0:
            return False
        values = [sample.get("regions", {}).get(item["name"]) for sample in samples]
        if any(
            not isinstance(value, str)
            or len(value) != expected_width * 2
            or re.fullmatch(r"[0-9a-fA-F]+", value) is None
            for value in values
        ):
            return False
        if len(set(values)) > 1 and any(any(char != "0" for char in value) for value in values):
            changing_nonzero_region = True
    return changing_nonzero_region


def _trace_regions_inert(trace_path: Path, region_items: list[dict]) -> bool:
    """Distinguish a valid but non-changing window from a short or malformed capture."""
    if not region_items or not trace_path.is_file():
        return False
    try:
        trace = json.loads(trace_path.read_text())
        samples = trace.get("samples", [])
        if len(samples) < MIN_COMPARISON_SAMPLES:
            return False
        for item in region_items:
            width_text = item.get("spec", "").rpartition(":")[2]
            expected_width = int(width_text, 16 if width_text.lower().startswith("0x") else 10)
            values = [sample.get("regions", {}).get(item["name"]) for sample in samples]
            if any(
                not isinstance(value, str)
                or len(value) != expected_width * 2
                or re.fullmatch(r"[0-9a-fA-F]+", value) is None
                for value in values
            ):
                return False
        return not _trace_supports_comparison(trace_path, region_items)
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError):
        return False


def _validated_region_items(region_doc: dict) -> list[dict]:
    """Accept subsystem-specific trace sets while rejecting ambiguous regions."""
    items = region_doc.get("regions")
    if not isinstance(items, list) or not items:
        raise ValueError("adoption gate requires at least one trace region")
    names = [item.get("name") for item in items if isinstance(item, dict)]
    if len(names) != len(items) or any(not isinstance(name, str) or not name for name in names):
        raise ValueError("adoption gate regions must each have a non-empty name")
    if len(set(names)) != len(names):
        raise ValueError("adoption gate region names must be unique")
    return items


def route_save_dir(recording: Path) -> Path | None:
    """Resolve and verify an owner recording's stamped save snapshot, if present."""
    stamped = header_saves(recording)
    if stamped is None:
        return None
    save_dir = locate_saves(recording, stamped[0])
    found = tree_hash(save_dir)
    if found != stamped[1]:
        raise ValueError(
            f"recording save snapshot hash mismatch: expected {stamped[1]}, found {found}"
        )
    return save_dir


def stored_baseline(
    root: Path,
    route: str,
    recording: Path,
    host: Path,
    region_names: set[str],
    region_items: list[dict] | None = None,
) -> Path | None:
    label, _ = route_path(route, root)
    host_hash = sha256(host)
    store = gate_data.find(root, gate_data.TRACES, label, host_hash, marker="manifest.json")
    manifest_path = store / "manifest.json"
    trace_path = store / "trace.json"
    if not manifest_path.is_file() or not trace_path.is_file():
        return None
    manifest = json.loads(manifest_path.read_text())
    if manifest.get("host_sha256") != host_hash or manifest.get("recording_sha256") != sha256(
        recording
    ):
        return None
    if not _trace_has_regions(trace_path, region_names, MIN_COMPARISON_SAMPLES):
        return None
    if region_items is not None and not _trace_supports_comparison(trace_path, region_items):
        return None
    return store


def cached_queue_baseline(
    recording: Path,
    host: Path,
    region_names: set[str],
    region_items: list[dict] | None = None,
) -> Path | None:
    cache = Path(os.environ.get("TSFP_REPLAY_CACHE") or datadir.data_dir("wt-cache/replays"))
    host_hash, recording_hash = sha256(host), sha256(recording)
    for record_path in sorted(cache.glob("*.json")):
        try:
            saved = json.loads(record_path.read_text())
            if (
                saved.get("host_sha256") != host_hash
                or saved.get("recording_sha256") != recording_hash
                or saved.get("switches") != ["off"]
                or saved.get("exit_code") != 0
            ):
                continue
            run_dir = Path(saved["run_dir"])
            trace_path = run_dir / "trace.json"
            if not _trace_has_regions(trace_path, region_names, MIN_COMPARISON_SAMPLES):
                continue
            if region_items is not None and not _trace_supports_comparison(
                trace_path, region_items
            ):
                continue
            if _trace_has_regions(trace_path, region_names, MIN_COMPARISON_SAMPLES):
                return run_dir
        except (KeyError, OSError, json.JSONDecodeError, TypeError):
            continue
    return None


GAMEPLAY_START_POLLS = ROOT / "tools/data/gameplay_start_polls.json"


def gameplay_start_poll(recording: Path) -> int:
    """T3390: the stored gameplay-start snapshot poll of a recording, 0 when it has none."""
    table = json.loads(GAMEPLAY_START_POLLS.read_text())
    value = table.get(Path(recording).stem, 0)
    return int(value) if isinstance(value, int) else 0


def snapshot_key(fields: dict) -> str:
    """Everything a DMTCP image pins for one gate configuration (T2479)."""
    return cache_keys.make_key("replay-capture", files={}, tools={}, env={}, extra=fields).key[:24]


def snapshot_has_checkpoint(cache: Path) -> bool:
    """Whether a cached snapshot still has at least one non-empty DMTCP image."""
    for image in (cache / "ckpt").glob("ckpt_*.dmtcp"):
        try:
            if image.is_file() and image.stat().st_size > 0:
                return True
        except OSError:
            continue
    return False


def earlier_snapshot_cache(
    snapshot_root: Path, fields: dict, target_poll: int, max_behind: int = 300
) -> tuple[Path, int]:
    """Choose the nearest valid-key snapshot up to five seconds before the requested poll.

    The key fields other than `poll` must match exactly. This keeps route, host, switches,
    save, trace regions and call-args configuration pinned while allowing a nearby earlier
    checkpoint to cover a new boundary without another DMTCP take.
    """
    exact = snapshot_root / snapshot_key({**fields, "poll": target_poll})
    exact_manifest = exact / "manifest.json"
    if exact_manifest.is_file():
        try:
            if json.loads(exact_manifest.read_text()).get(
                "poll"
            ) == target_poll and snapshot_has_checkpoint(exact):
                return exact, target_poll
        except (OSError, json.JSONDecodeError):
            pass
    for poll in range(target_poll - 1, max(0, target_poll - max_behind - 1), -1):
        candidate_fields = {**fields, "poll": poll}
        candidate = snapshot_root / snapshot_key(candidate_fields)
        manifest_path = candidate / "manifest.json"
        if not manifest_path.is_file():
            continue
        try:
            manifest = json.loads(manifest_path.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if manifest.get("poll") == poll and snapshot_has_checkpoint(candidate):
            return candidate, poll
    return snapshot_root / snapshot_key({**fields, "poll": target_poll}), target_poll


def retire_snapshot(cache: Path) -> Path | None:
    """Move an unusable (stale, partial or interrupted) cache entry out of the way.

    The rename goes to a fresh unique sibling, so an earlier `.stale-*` directory (an
    interrupted take, PID reuse) can never block it; a concurrent retirement that already
    moved the entry is a no-op. Only the directory this call moved is deleted.
    """
    import shutil
    import uuid

    if not cache.exists():
        return None
    retired = cache.with_name(f"{cache.name}.stale-{os.getpid()}-{uuid.uuid4().hex[:8]}")
    try:
        cache.rename(retired)
    except FileNotFoundError:
        return None
    shutil.rmtree(retired, ignore_errors=True)
    return retired


def snapshot_capture(
    root: Path,
    cache: Path,
    commit: str,
    take: Callable[[Path], dict],
    capture_seconds: int | None = None,
    host_input_hash: str | None = None,
    isolate_run_dir: bool = False,
    fail_on_resume_error: bool = False,
    _lock_held: bool = False,
) -> tuple[Path, dict]:
    """Resume a cached gate snapshot, or take it once (T2479).

    The first run takes the snapshot with --snapshot-continue, so the continued run itself is
    this configuration's full trace (no extra capture). Later runs resume the image and skip
    boot and menus. A snapshot refused by its manifest (a rebuilt host, another DMTCP build) is
    taken again. Returns (trace directory, receipt).
    """
    capture_started = time.monotonic()
    if not _lock_held:
        import fcntl

        lock_dir = cache.parent / ".snapshot-locks"
        lock_dir.mkdir(parents=True, exist_ok=True)
        lock_path = lock_dir / f"{cache.name}.lock"
        with lock_path.open("a+b") as lock_stream:
            fcntl.flock(lock_stream.fileno(), fcntl.LOCK_EX)
            try:
                return snapshot_capture(
                    root,
                    cache,
                    commit,
                    take,
                    capture_seconds,
                    host_input_hash,
                    isolate_run_dir,
                    fail_on_resume_error,
                    _lock_held=True,
                )
            finally:
                fcntl.flock(lock_stream.fileno(), fcntl.LOCK_UN)

    manifest = cache / "manifest.json"
    if manifest.is_file() and snapshot_has_checkpoint(cache):
        # A full replay of `capture_seconds` spends `take_total` reaching the snapshot poll; the
        # resumed window gets the remainder, so both cover the same gameplay span.
        take_total = json.loads(manifest.read_text()).get("seconds", {}).get("take_total")
        window = (
            round(capture_seconds - take_total)
            if capture_seconds and take_total and capture_seconds - take_total >= 60
            else None
        )
        resume_command = [
            sys.executable,
            "-m",
            "tools.wt",
            "replay",
            "resume",
            "--snapshot-dir",
            str(cache),
            "--commit",
            commit,
            *(("--seconds", str(window)) if window else ()),
        ]
        if host_input_hash:
            resume_command.extend(("--host-input-hash", host_input_hash))
        if isolate_run_dir:
            resume_command.append("--isolate-run-dir")

        resume_started = time.monotonic()
        result = subprocess.run(
            resume_command,
            cwd=ROOT,
            check=False,
            text=True,
            capture_output=True,
        )
        resume_elapsed = round(time.monotonic() - resume_started, 3)
        run_dir = Path(json.loads(manifest.read_text())["run_dir"])
        # DMTCP's isolated resume writes to the sibling created by
        # snapshot.session.resume_capture(), not the normal first-run
        # `continued` directory.
        continued = cache / "continued"
        trace_dir = cache / "isolated-run-dir" if isolate_run_dir else run_dir
        if result.returncode == 0 and not (trace_dir / "trace.json").is_file():
            # Some resumed hosts retain the captured output root despite the
            # per-resume marker. The replay receipt's run directory is unique
            # to this checkpoint resume, so it remains a safe trace source;
            # segment stitching validates its poll bounds before comparison.
            if isolate_run_dir and (run_dir / "trace.json").is_file():
                trace_dir = run_dir
        if result.returncode == 0 and replay.trace_has_samples(trace_dir / "trace.json"):
            resumed = next(
                (
                    json.loads(line)
                    for line in reversed(result.stdout.splitlines())
                    if line.startswith("{") and '"snapshot_dir"' in line
                ),
                {},
            )
            breakdown = resumed.get("capture_breakdown") or {
                key: resumed.get(key)
                for key in (
                    "queue_wait_seconds",
                    "run_seconds",
                    "host",
                    "host_name",
                    "paced",
                    "snapshot",
                    "cache_hit",
                    "timing_source",
                )
            }
            return trace_dir, {
                "exit_code": 0,
                "snapshot": "resumed",
                "run_dir": str(run_dir),
                "window_seconds": window,
                **resumed,
                "snapshot_capture_elapsed_seconds": round(time.monotonic() - capture_started, 3),
                "snapshot_resume_elapsed_seconds": resume_elapsed,
                "capture_breakdown": breakdown,
            }
        if fail_on_resume_error:
            output = [*result.stdout.splitlines(), *result.stderr.splitlines()]
            resume_logs = sorted(cache.glob("resume-*.log"), key=lambda path: path.stat().st_mtime)
            if resume_logs:
                try:
                    output.extend(resume_logs[-1].read_text(errors="replace").splitlines())
                except OSError:
                    pass
            detail = "\n".join(line[-1000:] for line in output[-16:])
            raise RuntimeError(
                f"batched snapshot resume failed (exit {result.returncode}); "
                f"checkpoint preserved at {cache}\n{detail}"
            )
        print(
            f"adoption gate: snapshot {cache} not resumable (exit {result.returncode}); "
            "taking it again",
            file=sys.stderr,
        )
    retire_snapshot(cache)
    cache.parent.mkdir(parents=True, exist_ok=True)
    take_started = time.monotonic()
    receipt = take(cache)
    take_elapsed = round(time.monotonic() - take_started, 3)
    continued = cache / "continued"
    if (
        receipt.get("exit_code")
        or not snapshot_has_checkpoint(cache)
        or not (continued / "trace.json").is_file()
        or not replay.trace_has_samples(continued / "trace.json")
    ):
        if not replay.trace_has_samples(continued / "trace.json"):
            receipt = {**receipt, "snapshot": "capture_empty"}
            retire_snapshot(cache)
        return continued, {
            **receipt,
            "exit_code": receipt.get("exit_code") or 1,
            **({"snapshot": "checkpoint_missing"} if not snapshot_has_checkpoint(cache) else {}),
            "snapshot_capture_elapsed_seconds": round(time.monotonic() - capture_started, 3),
            "snapshot_take_elapsed_seconds": take_elapsed,
        }
    return continued, {
        **receipt,
        "snapshot": "taken",
        "snapshot_capture_elapsed_seconds": round(time.monotonic() - capture_started, 3),
        "snapshot_take_elapsed_seconds": take_elapsed,
        "capture_breakdown": receipt.get("capture_breakdown"),
    }


def _windowed_trace(trace_dir: Path, from_poll: int, scratch: Path) -> Path:
    """Trace directory holding only samples at or after `from_poll` (for the quality guard)."""
    if not from_poll:
        return trace_dir
    trace = json.loads((trace_dir / "trace.json").read_text())
    trace["samples"] = [s for s in trace.get("samples", []) if s.get("poll", 0) >= from_poll]
    scratch.mkdir(parents=True, exist_ok=True)
    (scratch / "trace.json").write_text(json.dumps(trace))
    return scratch


def poll_segments(first_poll: int, last_poll: int, count: int) -> list[tuple[int, int]]:
    """Partition an inclusive deterministic poll window into nonempty segments."""
    if count < 1 or first_poll < 1 or last_poll < first_poll:
        raise ValueError("segments need a nonempty poll window and K >= 1")
    polls = last_poll - first_poll + 1
    if count > polls:
        raise ValueError(f"cannot split {polls} polls into {count} nonempty segments")
    return [
        (
            first_poll + polls * index // count,
            first_poll + polls * (index + 1) // count - 1,
        )
        for index in range(count)
    ]


def stitch_segment_traces(
    trace_dirs: list[Path], segments: list[tuple[int, int]], output: Path
) -> Path:
    """Stitch disjoint poll windows, retaining each capture's sample order."""
    if len(trace_dirs) != len(segments) or not trace_dirs:
        raise ValueError("segment trace count does not match the segment plan")
    if any(first < 1 or last < first for first, last in segments):
        raise ValueError("segment plan contains an empty or invalid poll window")
    if any(left[1] + 1 != right[0] for left, right in zip(segments, segments[1:], strict=False)):
        raise ValueError("segment plan must be contiguous and ordered without overlap")
    merged: dict | None = None
    samples: list[dict] = []
    for trace_dir, (first, last) in zip(trace_dirs, segments, strict=True):
        trace_path = trace_dir / "trace.json"
        if not trace_path.is_file():
            raise RuntimeError(f"segment trace missing: {trace_path}")
        trace = json.loads(trace_path.read_text())
        if merged is None:
            merged = {key: value for key, value in trace.items() if key != "samples"}
        window = [
            sample
            for sample in trace.get("samples", [])
            if first <= int(sample.get("poll", -1)) <= last
        ]
        if not window:
            raise RuntimeError(f"segment {first}-{last} contains no trace samples")
        samples.extend(window)
    assert merged is not None
    merged["samples"] = samples
    merged["meta"] = {
        **merged.get("meta", {}),
        "route_end_poll": segments[-1][1],
        "segments": [{"from_poll": first, "through_poll": last} for first, last in segments],
    }
    output.mkdir(parents=True, exist_ok=True)
    (output / "trace.json").write_text(json.dumps(merged))
    return output


def classify_candidates(
    candidates: tuple[str, ...],
    check: Callable[[tuple[str, ...]], bool],
    results: dict[str, dict],
    max_parallel: int,
) -> None:
    """Run combined and parallel bisection checks, annotating each switch."""
    pending: list[tuple[tuple[str, ...], tuple[str, ...] | None]] = [(candidates, None)]
    parent_equal: dict[tuple[str, ...], bool] = {}
    with ThreadPoolExecutor(max_workers=max_parallel) as executor:
        while pending:
            outcomes = list(executor.map(lambda item: (item[0], item[1], check(item[0])), pending))
            children: list[tuple[tuple[str, ...], tuple[str, ...]]] = []
            children_by_parent: dict[tuple[str, ...], list[bool]] = {}
            for group, parent, equal in outcomes:
                if parent is not None:
                    children_by_parent.setdefault(parent, []).append(equal)
                if equal:
                    parent_equal[group] = True
                    for switch in group:
                        results.setdefault(switch, {})["tier2"] = "EQUAL"
                elif len(group) == 1:
                    parent_equal[group] = False
                    result = results.setdefault(group[0], {})
                    if result.get("tier2") != "VACUOUS":
                        result["tier2"] = "DIVERGED"
                else:
                    parent_equal[group] = False
                    middle = len(group) // 2
                    children.extend(((group[:middle], group), (group[middle:], group)))
            for parent, child_results in children_by_parent.items():
                if (
                    not parent_equal.get(parent, True)
                    and len(child_results) == 2
                    and all(child_results)
                ):
                    key = ", ".join(parent)
                    results[f"interaction: {key}"] = {
                        "tier2": "DIVERGED (interaction; halves equal)"
                    }
            pending = children


def mark_invalid_ai_comparisons_inconclusive(results: dict[str, dict]) -> None:
    """Do not label an AI state gate divergent when either side has no valid rows."""
    for result in results.values():
        quality = result.get("ai_state_quality")
        if not quality:
            continue
        if not quality["reference"]["valid"] or not quality["owned"]["valid"]:
            result["trace_tier2"] = result.get("tier2", "DIVERGED")
            result["tier2"] = "INCONCLUSIVE (invalid AI state rows)"


def compare(
    root: Path,
    route: str,
    regions: Path,
    host: Path,
    baseline: Path,
    candidate: Path,
    report_base: Path,
    min_samples: int | None = None,
    from_poll: int = 0,
    segments: int | None = None,
) -> bool:
    command = [
        sys.executable,
        "-m",
        "tools.rust_trace",
        "diff",
        "--route",
        route,
        "--host",
        str(host),
        "--regions",
        str(regions),
        "--left",
        str(baseline),
        "--right",
        str(candidate),
        "--report",
        str(report_base),
    ]
    command.extend(("--min-samples", str(max(min_samples or 0, MIN_COMPARISON_SAMPLES))))
    if from_poll:
        command.extend(("--from-poll", str(from_poll)))
    if segments is not None:
        command.extend(("--segments", str(segments)))
    # Run the diff module from this checkout so its parser and comparison logic
    # match the segmented capture implementation. `root` is the shared primary
    # checkout used for owner data and replay caches, which may lag this task
    # worktree while a change is being developed.
    result = subprocess.run(
        command,
        cwd=Path(__file__).resolve().parents[1],
        check=False,
        text=True,
        capture_output=True,
    )
    report_json = report_base.with_suffix(".json")
    if not report_json.is_file():
        raise RuntimeError(f"rust_trace diff did not write {report_json}: {result.stderr[-1000:]}")
    return bool(json.loads(report_json.read_text()).get("equal"))


def ai_state_quality(trace_path: Path) -> dict:
    """Require meaningful, correctly-sized typed AI rows before an EQUAL claim."""
    from tools.rea_ai_state_trace import STATE_WORDS

    trace = json.loads(trace_path.read_text()) if trace_path.is_file() else {}
    rows = trace.get("rea_ai_state", [])
    width_ok = bool(rows) and all(len(row.get("words", [])) == STATE_WORDS for row in rows)
    nonzero = any(any(word != 0 for word in row.get("words", [])) for row in rows)
    by_actor: dict[tuple[int, int], list[list[int]]] = {}
    for row in rows:
        by_actor.setdefault((row.get("slot", -1), row.get("generation", -1)), []).append(
            row.get("words", [])
        )
    varying = any(
        any(
            len({words[index] for words in actor_rows if len(words) == STATE_WORDS}) > 1
            for index in range(STATE_WORDS)
        )
        for actor_rows in by_actor.values()
    )
    receipt = bool(trace.get("meta", {}).get("rea_ai_state_receipt"))
    return {
        "rows": len(rows),
        "word_width": STATE_WORDS,
        "width_ok": width_ok,
        "receipt": receipt,
        "nonzero": nonzero,
        "varying": varying,
        "valid": receipt and len(rows) >= 600 and width_ok and nonzero and varying,
    }


def resolve_report_path(report: str | None, root: Path | None = None) -> Path:
    """Report path; the default is untracked, a tracked file is refused in the primary checkout."""
    from tools import gate_marks

    root = root or ROOT
    if report is None:
        if root == ROOT:
            from tools import datadir

            return datadir.data_dir(REPORT_NAME)
        return root / "tmp" / REPORT_NAME
    path = Path(report)
    target = path if path.is_absolute() else Path.cwd() / path
    if not gate_marks.is_primary_checkout(root):
        return path
    tracked = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", str(target.resolve())],
        capture_output=True,
        timeout=30,
    )
    if tracked.returncode == 0:
        raise ValueError(
            f"{path} is a tracked file and {root} is the primary checkout (rule 32 keeps it "
            "clean); run from a linked worktree to regenerate it, or omit --report"
        )
    return path


def adoption_gate(args: argparse.Namespace) -> dict:
    from tools import gate

    root = main_root()
    route_name, recording = route_path(args.route, root)
    if not recording.is_file():
        raise ValueError(f"recording not found: {recording}")
    recorded_save_dir = route_save_dir(recording)
    commit = (
        gate.resolve_commit(ROOT, args.commit, None)
        if args.commit
        else prebuilt.primary_commit(ROOT)
    )
    host_input_hash = getattr(args, "host_input_hash", None) or prebuilt_select.build_input_hash(
        ROOT, commit
    )
    host = _host_or_wait(commit, host_input_hash)
    if not prebuilt_select.available(host):
        raise prebuilt.PrebuiltError(missing_host_message(commit, host_input_hash))
    regions = Path(args.regions).resolve()
    region_doc = json.loads(regions.read_text())
    region_items = _validated_region_items(region_doc)
    candidates = (
        tuple(item.strip() for item in args.switches.split(",") if item.strip())
        if args.switches
        else switch_set_from_owner_checks()
    )
    classify_environment = bool(getattr(args, "classify_environment", False))
    if classify_environment:
        environment_candidates = tuple(item for item in candidates if item.startswith("TSFP_"))
        candidates, _ = _split_shared_environment(candidates)
        shared_environment = ()
        candidates = (*candidates, *environment_candidates)
    else:
        candidates, shared_environment = _split_shared_environment(candidates)
    roots = gate.parse_roots(getattr(args, "call_args", None))
    selected_for_registry = list(candidates)
    for value in (getattr(args, "rust_ports", None), getattr(args, "baseline_rust_ports", None)):
        if value:
            selected_for_registry.extend(item for item in value.split(",") if item)
    try:
        gate.validate_registered_inputs(tuple(selected_for_registry), roots)
    except gate.GateInvalid as error:
        raise ValueError(
            f"{error.message}; fix the switch/root registry entry before capturing"
        ) from None
    save_dir_override = Path(args.save_dir).resolve() if getattr(args, "save_dir", None) else None
    save_dir = save_dir_override or recorded_save_dir
    host_args = tuple(getattr(args, "host_arg", ()) or ())
    candidate_host_args = tuple(getattr(args, "candidate_host_arg", ()) or ())
    stop_at_route_end = bool(getattr(args, "stop_at_route_end", False))
    baseline_rust_ports = getattr(args, "baseline_rust_ports", None)
    custom_capture = (
        save_dir_override is not None
        or bool(host_args)
        or bool(candidate_host_args)
        or stop_at_route_end
        or bool(baseline_rust_ports)
    )
    rust_ports = getattr(args, "rust_ports", None)
    candidate_environment = tuple(getattr(args, "candidate_env", ()) or ())
    rea_ai_gate = bool(getattr(args, "rea_ai_state", False))
    call_args = getattr(args, "call_args", None)
    if rea_ai_gate and "rust-rea-ai" not in candidates:
        raise ValueError("--rea-ai-state requires rust-rea-ai in --switches")
    if rea_ai_gate and not rust_ports:
        raise ValueError("--rea-ai-state requires --rust-ports to select the AI tick root")
    if rea_ai_gate and not call_args:
        raise ValueError("--rea-ai-state requires --call-args to record the selected AI tick root")
    region_names = {item["name"] for item in region_items}
    snapshot_poll = int(getattr(args, "snapshot_poll", 0) or 0)
    if not snapshot_poll and not getattr(args, "full_replay", False) and not rea_ai_gate:
        # T3390: gameplay gates resume a cached gameplay-start snapshot by default.
        snapshot_poll = gameplay_start_poll(recording)
        if snapshot_poll:
            print(
                f"adoption gate: comparing from gameplay start poll {snapshot_poll} "
                "(tools/data/gameplay_start_polls.json, cached snapshot); --full-replay opts out",
                flush=True,
            )
    # The compared window starts at the snapshot poll; --from-poll gives full replays the same
    # window (r10's NPC regions are unreadable before gameplay, so whole-route guards fail).
    from_poll = snapshot_poll or int(getattr(args, "from_poll", 0) or 0)
    if from_poll:
        last_known_poll = gate.known_last_poll(
            root, input_hash=host_input_hash, recording_sha=sha256(recording)
        )
        if last_known_poll is not None and from_poll > last_known_poll:
            raise ValueError(
                f"start poll {from_poll} exceeds the furthest poll {last_known_poll} already "
                f"reached for {recording.stem} with this prebuilt input and recording; "
                "check the route/save binding or choose a start supported by the route evidence"
            )
    headless = bool(getattr(args, "headless", False))
    if snapshot_poll and rea_ai_gate:
        raise ValueError("--snapshot-poll does not support --rea-ai-state sidecars yet")
    requested_segments = getattr(args, "segments", None)
    segment_count = int(requested_segments) if requested_segments is not None else 1
    if segment_count < 1:
        raise ValueError("--segments needs K >= 1")
    segmented = requested_segments is not None
    segment_windows: list[tuple[int, int]] = []
    if segmented:
        if rea_ai_gate or call_args:
            raise ValueError(
                "--segments currently supports region traces, not AI/call-argument sidecars"
            )
        from tools.rust_trace import native_input_route_end_sim_tick

        route_end_tick = native_input_route_end_sim_tick(recording)
        if route_end_tick is None:
            raise ValueError(
                "--segments requires a native .rec route with a deterministic simulation-tick end"
            )
        segment_windows = poll_segments(
            max(1, snapshot_poll or from_poll or 1), route_end_tick, segment_count
        )
    # T3467: bulk gate data lives in tmp/wt-cache/gate-data under retention, not tmp/owner.
    snapshot_root = gate_data.path(root, gate_data.SNAPSHOTS) / route_name / sha256(host)
    window_root = (
        gate_data.path(root, gate_data.ADOPTION_GATE) / route_name / sha256(host) / "windows"
    )
    snapshot_runs: dict[str, str] = {}
    segment_capture_slots = threading.BoundedSemaphore(2)

    def gate_snapshot(
        group: tuple[str, ...],
        ports: str | None,
        *,
        candidate: bool = False,
        candidate_env: tuple[str, ...] | None = None,
    ) -> tuple[Path, dict]:
        """Off baseline or candidate capture from its cached gameplay snapshot."""
        capture_host_args = (
            _candidate_host_args(host_args, candidate_host_args) if candidate else host_args
        )
        selected_environment = (
            (tuple(candidate_env) if candidate_env is not None else candidate_environment)
            if candidate
            else ()
        )
        fields = {
            "switches": list(group),
            "rust_ports": ports,
            "poll": snapshot_poll,
            "seconds": args.seconds,
            "max_bytes": getattr(args, "max_bytes", 4_000_000_000),
            "regions": sha256(regions),
            "recording": sha256(recording),
            "save": tree_hash(save_dir) if save_dir is not None else None,
            "host_args": list(capture_host_args),
            "candidate_environment": list(selected_environment),
            # T2376: a call-args capture is a distinct replay artifact. Include its roots in the
            # snapshot cache key so a prior state-only capture cannot satisfy the request.
            "call_args": call_args,
            "stop_at_route_end": stop_at_route_end,
            "paced": bool(args.paced),
        }
        cache, snapshot_used_poll = earlier_snapshot_cache(snapshot_root, fields, snapshot_poll)

        def take(directory: Path) -> dict:
            return queue_submit(
                root,
                args.route,
                group,
                commit,
                regions,
                args.seconds,
                bool(getattr(args, "no_cache", False)),
                allow_capture_failure=True,
                paced=args.paced,
                save_dir=save_dir,
                host_args=capture_host_args,
                stop_at_route_end=stop_at_route_end,
                rust_ports=ports,
                snapshot=(snapshot_used_poll, directory),
                call_args=call_args,
                max_idle=WINDOW_MAX_IDLE,
                candidate_environment=selected_environment,
                priority=getattr(args, "priority", None),
                max_bytes=getattr(args, "max_bytes", 4_000_000_000),
                host_input_hash=getattr(args, "host_input_hash", None),
            )

        capture_seconds = args.seconds
        if snapshot_used_poll < snapshot_poll:
            manifest = json.loads((cache / "manifest.json").read_text())
            take_total = manifest.get("seconds", {}).get("take_total")
            if take_total:
                capture_seconds += round(take_total)
        trace_dir, receipt = snapshot_capture(
            root,
            cache,
            commit,
            take,
            capture_seconds,
            getattr(args, "host_input_hash", None),
        )
        receipt.update(
            requested_poll=snapshot_poll,
            snapshot_poll=snapshot_used_poll,
            reused_earlier_by_polls=snapshot_poll - snapshot_used_poll,
        )
        snapshot_runs[",".join(group)] = receipt.get("snapshot", "failed")
        return trace_dir, receipt

    def gate_segments(
        group: tuple[str, ...],
        ports: str | None,
        segments: list[tuple[int, int]],
        *,
        candidate: bool = False,
        candidate_env: tuple[str, ...] | None = None,
    ) -> tuple[Path, list[dict]]:
        """Take all boundary snapshots in one route pass, then resume their segments in parallel."""
        capture_host_args = (
            _candidate_host_args(host_args, candidate_host_args) if candidate else host_args
        )
        selected_environment = candidate_env if candidate_env is not None else candidate_environment
        if any(
            item == "--stop-at-poll" or item.startswith("--stop-at-poll=")
            for item in capture_host_args
        ):
            raise ValueError("--segments owns --stop-at-poll; remove a conflicting --host-arg")

        starts = [first for first, _last in segments]
        batch_fields = {
            # Bump when the checkpointed Python capture/resume protocol changes.
            # A DMTCP image retains the old in-memory code even when the worktree
            # has been updated, so the batch cache must not reuse it.
            "schema": "T3423-multisnapshot-v2",
            "switches": list(group),
            "rust_ports": ports,
            "polls": starts,
            "segments": [list(window) for window in segments],
            "regions": sha256(regions),
            "recording": sha256(recording),
            "save": tree_hash(save_dir) if save_dir is not None else None,
            "host_args": list(capture_host_args),
            "candidate_environment": list(selected_environment if candidate else ()),
            "paced": bool(args.paced),
            "host_input_hash": getattr(args, "host_input_hash", None),
        }
        batch_cache = snapshot_root / snapshot_key(batch_fields)
        from tools.snapshot import manifest as snapshot_manifest

        def valid_batch() -> bool:
            for poll in starts:
                directory = batch_cache / f"poll-{poll}"
                try:
                    data = snapshot_manifest.load(directory)
                    if snapshot_manifest.verify(
                        data, directory, host_override=host, fast=True, check_display=False
                    ):
                        return False
                except (OSError, SystemExit, ValueError, KeyError, TypeError):
                    return False
            return True

        if bool(getattr(args, "no_cache", False)) or not valid_batch():
            if batch_cache.exists():
                retire_snapshot(batch_cache)
            capture_receipt = queue_submit(
                root,
                args.route,
                group,
                commit,
                regions,
                args.seconds,
                bool(getattr(args, "no_cache", False)),
                allow_capture_failure=True,
                paced=args.paced,
                save_dir=save_dir,
                host_args=capture_host_args,
                stop_at_route_end=True,
                rust_ports=ports,
                snapshot_polls=(starts, batch_cache),
                max_idle=WINDOW_MAX_IDLE,
                max_dumps=12000,
                candidate_environment=selected_environment if candidate else (),
                priority=getattr(args, "priority", None),
                max_bytes=getattr(args, "max_bytes", 4_000_000_000),
                host_input_hash=getattr(args, "host_input_hash", None),
            )
            if capture_receipt.get("exit_code"):
                raise RuntimeError(f"multi-snapshot route capture failed: {capture_receipt}")
            if not valid_batch():
                raise RuntimeError(
                    "multi-snapshot route capture did not produce every valid checkpoint"
                )

        def capture_segment(index: int, window: tuple[int, int]) -> tuple[Path, dict]:
            first, last = window
            cache = batch_cache / f"poll-{first}"
            (cache / "run-dir" / "snapshot-stop-poll").write_text(f"{last}\n")

            def unexpected_retake(_directory: Path) -> dict:
                raise RuntimeError("a batched segment snapshot became invalid during resume")

            with segment_capture_slots:
                trace_dir, receipt = snapshot_capture(
                    root,
                    cache,
                    commit,
                    unexpected_retake,
                    host_input_hash=getattr(args, "host_input_hash", None),
                    isolate_run_dir=True,
                    fail_on_resume_error=True,
                )
            snapshot_runs[f"{','.join(group)}@{first}-{last}"] = receipt.get("snapshot", "failed")
            return trace_dir, receipt

        workers = max(1, min(8, len(segments)))
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futures = [
                pool.submit(capture_segment, index, window) for index, window in enumerate(segments)
            ]
            captured = [future.result() for future in futures]
        failures = [receipt for _trace_dir, receipt in captured if receipt.get("exit_code")]
        if failures:
            raise RuntimeError(f"segmented capture failed before stitching: {failures}")
        trace_dirs = [trace_dir for trace_dir, _receipt in captured]
        config = snapshot_key({"switches": list(group), "ports": ports, "candidate": candidate})
        stitched = stitch_segment_traces(
            trace_dirs,
            segments,
            gate_data.path(root, gate_data.SEGMENTS) / route_name / sha256(host) / config,
        )
        return stitched, [receipt for _trace_dir, receipt in captured]

    baseline = None
    baseline_receipt = None
    baseline_pool = ThreadPoolExecutor(max_workers=1)
    # Snapshot gates resume (or take) the off baseline alongside the first candidates; it is
    # only needed at comparison time.
    baseline_future = None
    if segmented:
        baseline_future = baseline_pool.submit(
            gate_segments,
            _baseline_switches(shared_environment),
            baseline_rust_ports,
            segment_windows,
        )
    elif snapshot_poll:
        baseline_future = baseline_pool.submit(
            gate_snapshot, _baseline_switches(shared_environment), baseline_rust_ports
        )
    if not snapshot_poll and not custom_capture and not rea_ai_gate and not headless:
        baseline = stored_baseline(root, args.route, recording, host, region_names, region_items)
        if baseline is None:
            baseline = cached_queue_baseline(recording, host, region_names, region_items)
    if baseline is None and not snapshot_poll:
        baseline_receipt = queue_submit(
            root,
            args.route,
            _baseline_switches(shared_environment),
            commit,
            regions,
            args.seconds,
            bool(getattr(args, "no_cache", False)),
            paced=args.paced,
            render=args.render,
            save_dir=save_dir,
            host_args=host_args,
            stop_at_route_end=stop_at_route_end,
            headless=headless,
            max_idle=_capture_max_idle(from_poll, rea_ai_gate),
            rea_ai_state="reference" if rea_ai_gate else None,
            call_args=call_args,
            rust_ports=baseline_rust_ports,
            priority=getattr(args, "priority", None),
            max_bytes=getattr(args, "max_bytes", 4_000_000_000),
            host_input_hash=getattr(args, "host_input_hash", None),
        )
        baseline = Path(baseline_receipt["run_dir"])

    def ready_baseline() -> Path:
        """The validated off baseline (waits for a background snapshot baseline once)."""
        nonlocal baseline, baseline_receipt, baseline_future
        if baseline_future is not None:
            future, baseline_future = baseline_future, None
            baseline, baseline_receipt = future.result()
            if segmented:
                failures = [item for item in baseline_receipt if item.get("exit_code")]
                if failures:
                    raise RuntimeError(f"off baseline segment capture failed: {failures}")
            elif baseline_receipt.get("exit_code"):
                raise RuntimeError(f"off baseline snapshot capture failed: {baseline_receipt}")
            validate_baseline(baseline)
        assert baseline is not None
        return baseline

    def validate_baseline(trace_dir: Path) -> None:
        trace_path = _windowed_trace(trace_dir, from_poll, window_root / "baseline") / "trace.json"
        empty_reason = _capture_empty_reason(trace_path, region_items)
        if empty_reason:
            raise RuntimeError(
                f"CAPTURE_EMPTY: off baseline has 0 samples; reason={empty_reason}; "
                "check route reachability, region selection, and trace writer"
            )
        if not _trace_supports_comparison(trace_path, region_items):
            raise RuntimeError(
                f"baseline trace has fewer than {MIN_COMPARISON_SAMPLES} useful samples "
                "or invalid regions"
            )

    if baseline is not None:
        validate_baseline(baseline)
    baseline_lock = threading.Lock()
    results: dict[str, dict] = {}
    captures: dict[str, dict] = {}
    sequence = 0
    faulted_runs = 0
    max_parallel = 1 if segmented else max(1, min(8, int(os.environ.get("TSFP_REPLAY_SLOTS", "3"))))
    report_dir = gate_data.path(root, gate_data.ADOPTION_GATE) / route_name / sha256(host)

    def locked_baseline() -> Path:
        with baseline_lock:
            return ready_baseline()

    def check(group: tuple[str, ...]) -> bool:
        nonlocal sequence, faulted_runs
        sequence += 1
        tag = f"{sequence:03d}-" + hashlib.sha1(",".join(group).encode()).hexdigest()[:8]
        group_environment = tuple(item for item in group if item.startswith("TSFP_"))
        group_switches = tuple(item for item in group if not item.startswith("TSFP_"))
        candidate_env = (*candidate_environment, *group_environment)
        if segmented:
            candidate_dir, segment_receipts = gate_segments(
                (*group_switches, *shared_environment),
                rust_ports,
                segment_windows,
                candidate=True,
                candidate_env=candidate_env,
            )
            if any(receipt.get("exit_code") for receipt in segment_receipts):
                faulted_runs += 1
                for switch in group:
                    results[switch] = {"tier2": "DIVERGED", "fault_status": "faulted"}
                return False
        elif snapshot_poll:
            candidate_dir, receipt = gate_snapshot(
                (*group_switches, *shared_environment),
                rust_ports,
                candidate=True,
                candidate_env=candidate_env,
            )
        else:
            receipt = queue_submit(
                root,
                args.route,
                (*group_switches, *shared_environment),
                commit,
                regions,
                args.seconds,
                bool(getattr(args, "no_cache", False)),
                allow_capture_failure=True,
                paced=args.paced,
                render=args.render,
                save_dir=save_dir,
                host_args=_candidate_host_args(host_args, candidate_host_args),
                stop_at_route_end=stop_at_route_end,
                headless=headless,
                max_idle=_capture_max_idle(from_poll, rea_ai_gate),
                rust_ports=rust_ports,
                rea_ai_state="owned" if rea_ai_gate else None,
                call_args=call_args,
                candidate_environment=candidate_env,
                priority=getattr(args, "priority", None),
                max_bytes=getattr(args, "max_bytes", 4_000_000_000),
                host_input_hash=getattr(args, "host_input_hash", None),
            )
        if not segmented and receipt.get("exit_code"):
            faulted_runs += 1
            for switch in group:
                results[switch] = {"tier2": "DIVERGED", "fault_status": "faulted"}
            return False
        if not snapshot_poll:
            candidate_dir = Path(receipt["run_dir"])
        captures[",".join(group)] = {
            "trace_dir": str(candidate_dir),
            "receipt": segment_receipts if segmented else receipt,
        }
        if call_args:
            from tools.gate import GateInvalid, _load_trace, check_roots, parse_roots, root_calls

            roots = parse_roots(call_args)
            candidate_trace = _load_trace(candidate_dir)
            counts = root_calls(candidate_dir, candidate_trace, roots, from_poll)
            captures[",".join(group)]["root_calls"] = counts
            try:
                check_roots(counts, from_poll)
            except GateInvalid as invalid:
                # A requested root that never executes cannot be adoption evidence. Keep
                # multi-switch groups eligible for bisection; preserve VACUOUS on leaf rows.
                if len(group) == 1:
                    for switch in group:
                        results.setdefault(switch, {}).update(
                            tier2="VACUOUS", vacuous_reason=invalid.code, root_calls=counts
                        )
                return False
        candidate_window = (
            _windowed_trace(candidate_dir, from_poll, window_root / tag) / "trace.json"
        )
        empty_reason = _capture_empty_reason(candidate_window, region_items)
        if empty_reason:
            for switch in group:
                results[switch] = {
                    "tier2": "CAPTURE_EMPTY",
                    "capture_empty_reason": empty_reason,
                }
            return False
        if not _trace_supports_comparison(candidate_window, region_items):
            vacuous = _trace_regions_inert(candidate_window, region_items)
            # T3449: a candidate whose trace is too short or has invalid regions did not
            # reproduce the baseline. Aborting the whole route lost the bisection (arcade:
            # every isolated selector EQUAL, the combined set ended short), so treat it as a
            # failed group that bisection splits, labelled in the report.
            faulted_runs += 1
            for switch in group:
                results[switch] = {
                    "tier2": "VACUOUS" if vacuous else "DIVERGED",
                    "fault_status": "regions_inert" if vacuous else "short-trace",
                    **({"vacuous_reason": "regions_inert"} if vacuous else {}),
                }
            return False
        equal = compare(
            root,
            args.route,
            regions,
            host,
            locked_baseline(),
            candidate_dir,
            report_dir / tag,
            min_samples=600 if rea_ai_gate else None,
            from_poll=from_poll,
            segments=segment_count if segmented else None,
        )
        quality = None
        if rea_ai_gate:
            quality = {
                "reference": ai_state_quality(locked_baseline() / "trace.json"),
                "owned": ai_state_quality(candidate_dir / "trace.json"),
            }
            equal = equal and quality["reference"]["valid"] and quality["owned"]["valid"]
        captures[",".join(group)]["equal"] = bool(equal)
        for switch in group:
            result = results.setdefault(switch, {})
            result["fault_status"] = "fault-free"
            if quality is not None:
                result["ai_state_quality"] = quality
        return equal

    # Keep replay slots busy; the queue remains responsible for GPU admission
    # and cache controls. Each generation contains disjoint switch groups.
    classify_candidates(candidates, check, results, max_parallel)
    if rea_ai_gate:
        mark_invalid_ai_comparisons_inconclusive(results)
    baseline = locked_baseline()
    baseline_pool.shutdown()
    baseline_capture = {
        "trace_dir": str(baseline),
        "receipt": baseline_receipt,
    }
    report = {
        "route": route_name,
        "recording": str(recording),
        "recording_sha256": sha256(recording),
        "save_sha256": tree_hash(save_dir) if save_dir is not None else None,
        "host_sha256": sha256(host),
        "host_input_hash": prebuilt_select.host_input_hash(host, ROOT),
        "regions": len(region_doc["regions"]),
        "rust_ports": rust_ports,
        "baseline_rust_ports": baseline_rust_ports,
        "rea_ai_state": rea_ai_gate,
        "call_args": call_args,
        "baseline": str(baseline),
        "baseline_capture": baseline_capture,
        "captures": captures,
        "baseline_cache_hit": (
            all(item.get("snapshot") == "resumed" for item in baseline_receipt)
            if segmented and isinstance(baseline_receipt, list)
            else bool(baseline_receipt is None or baseline_receipt.get("cache_hit"))
        ),
        "seconds": args.seconds,
        "save_dir": str(save_dir) if save_dir is not None else None,
        "host_args": list(host_args),
        "candidate_host_args": list(candidate_host_args),
        "candidate_environment": list(candidate_environment),
        "classify_environment": classify_environment,
        "stop_at_route_end": stop_at_route_end,
        "snapshot_poll": snapshot_poll or None,
        "segments": segment_count if segmented else None,
        "from_poll": from_poll or None,
        "snapshot_runs": snapshot_runs,
        "fault_status": "faulted" if faulted_runs else "fault-free",
        "switches": results,
    }
    output = Path(args.report)
    output.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Adoption gate report",
        "",
        f"- Route: `{route_name}`",
        f"- Recording SHA-256: `{report['recording_sha256']}`",
        f"- Save snapshot SHA-256: `{report['save_sha256'] or 'none'}`",
        f"- Host SHA-256: `{report['host_sha256']}`",
        f"- Regions: {report['regions']}",
        f"- Rust port selector: `{rust_ports or 'derived from candidates'}`",
        f"- Baseline Rust port selector: `{baseline_rust_ports or 'off'}`",
        f"- Selected call roots: `{call_args or 'none'}`",
        f"- Typed REA AI state: {'required' if rea_ai_gate else 'not requested'}",
        f"- Baseline: `{baseline}` (cache reuse: {report['baseline_cache_hit']})",
        f"- Capture pacing: {'paced' if args.paced else 'fast-forward (default)'}",
        f"- Capture render: {args.render or 'auto (none for fast-forward state captures, T3390)'}",
        (
            f"- Snapshot window: from poll {snapshot_poll} (T2479 cached DMTCP snapshots; "
            f"{', '.join(f'{k}: {v}' for k, v in sorted(snapshot_runs.items()))})"
            if snapshot_poll
            else "- Snapshot window: none (full replays)"
        ),
        f"- Segments: {report['segments'] if report['segments'] is not None else 'one capture'}",
        f"- Combined run fault status: {report['fault_status']} after {args.seconds} s requested",
        "",
        "| Switch / adapter | Tier 2 | 180 s fault status |",
        "|---|---|---|",
    ]
    for switch, result in sorted(results.items()):
        lines.append(
            f"| `{switch}` | {result['tier2']} | {result.get('fault_status', 'not run')} |"
        )
        quality = result.get("ai_state_quality")
        if quality:
            lines.append(
                f"Generic region comparison for `{switch}`: "
                f"{result.get('trace_tier2', result['tier2'])} (not AI-state evidence)."
            )
            reference = quality["reference"]
            owned = quality["owned"]
            lines.append(
                "\n"
                f"AI state quality for `{switch}`: reference rows={reference['rows']}, "
                f"owned rows={owned['rows']}, width={reference['word_width']} words, "
                f"reference valid={reference['valid']}, owned valid={owned['valid']}."
            )
    output.write_text("\n".join(lines) + "\n")
    report["report"] = str(output)
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--route", required=True)
    parser.add_argument(
        "--switches", help="comma-separated switch set; default derives owner-check candidates"
    )
    parser.add_argument("--commit")
    parser.add_argument(
        "--host-input-hash", help="pin all gate captures to this prebuilt input hash"
    )
    parser.add_argument("--regions", default=str(DEFAULT_REGIONS))
    parser.add_argument(
        "--rust-ports",
        help=(
            "Rust port selector appended to each candidate replay "
            "for host adapters requiring a selected root"
        ),
    )
    parser.add_argument(
        "--baseline-rust-ports",
        help=(
            "Rust port selector for the reference capture; useful when a lifted callee is "
            "observed through a selected translated caller"
        ),
    )
    parser.add_argument(
        "--rea-ai-state",
        action="store_true",
        help="capture and compare reference versus Rust-owned AI actor state sidecars",
    )
    parser.add_argument("--call-args", help="call roots to record alongside an AI state gate")
    parser.add_argument("--seconds", type=int, default=180)
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=4_000_000_000,
        help="cap trace output bytes per capture, including filtered --call-args output",
    )
    parser.add_argument(
        "--priority",
        choices=("normal", "high"),
        default="normal",
        help="replay queue lane; normal is the worker-safe default",
    )
    parser.add_argument(
        "--segments",
        type=int,
        help="split a native .rec deterministic poll window into K cached DMTCP segments",
    )
    parser.add_argument("--paced", action="store_true", help="use wall-clock-paced captures")
    parser.add_argument(
        "--render",
        choices=("auto", "gpu", "none"),
        help="T3390: rust_trace --render for the captures (default auto: headless none for "
        "fast-forward state captures)",
    )
    parser.add_argument("--save-dir", type=Path, help="seed save directory copied for each run")
    parser.add_argument("--host-arg", action="append", default=[], help="extra host argument")
    parser.add_argument(
        "--candidate-host-arg",
        action="append",
        default=[],
        help="host argument applied only to switch-on candidate captures",
    )
    parser.add_argument(
        "--candidate-env",
        action="append",
        default=[],
        help="TSFP_* environment value applied only to switch-on candidate captures",
    )
    parser.add_argument("--stop-at-route-end", action="store_true")
    parser.add_argument(
        "--from-poll",
        type=int,
        default=0,
        help="compare only samples from this poll on (implied by --snapshot-poll)",
    )
    parser.add_argument(
        "--headless",
        action="store_true",
        help="run full-replay captures on Xvfb/lavapipe (snapshot captures always are)",
    )
    parser.add_argument(
        "--no-cache", action="store_true", help="bypass replay-result cache entries"
    )
    parser.add_argument(
        "--full-replay",
        action="store_true",
        help="T3390: replay from boot even when the route has a gameplay-start snapshot poll",
    )
    parser.add_argument(
        "--snapshot-poll",
        type=int,
        default=0,
        help=(
            "T2479: compare from this port-0 poll (e.g. r10 gameplay start 8816) using cached "
            "DMTCP snapshots, one per (route, host, switch set, capture options); the first run "
            "takes them, reruns resume them and skip boot and menus"
        ),
    )
    parser.add_argument(
        "--report",
        help=f"report path (default {DEFAULT_REPORT}); tracked files are refused in primary",
    )
    args = parser.parse_args(argv)
    try:
        args.report = str(resolve_report_path(args.report))
    except ValueError as error:
        parser.error(str(error))
    try:
        from tools import gate

        commit = (
            gate.resolve_commit(ROOT, args.commit, None)
            if args.commit
            else prebuilt.primary_commit(ROOT)
        )
        host_input_hash = args.host_input_hash or prebuilt_select.build_input_hash(ROOT, commit)
        host = _host_or_wait(commit, host_input_hash)
        if not prebuilt_select.available(host):
            raise prebuilt.PrebuiltError(missing_host_message(commit, host_input_hash))
        # Pin the host across baseline discovery and all replay submissions so
        # concurrent prebuilt-cache GC cannot remove it between queue leases.
        with prebuilt.lease(host):
            args.host_input_hash = host_input_hash
            result = adoption_gate(args)
    except (OSError, ValueError, RuntimeError, prebuilt.PrebuiltError) as error:
        print(f"adoption gate: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0 if all(item["tier2"] == "EQUAL" for item in result["switches"].values()) else 2


if __name__ == "__main__":
    raise SystemExit(main())
