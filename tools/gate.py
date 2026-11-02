"""T3416 gate service: one entry point for a Rule 24 Tier 2 adoption gate.

    python3 -m tools.gate --profile weapons --route story-level1 --start gameplay

Resolves the exact candidate host (prebuilt by build-input hash, built once under a build
slot when missing), the owner route with its bound saves, the owner disc from the main
checkout and the start poll (number, `mark:N` from the route's F12 M rows, or `gameplay`).
Captures baseline and candidate through the replay queue from a cached DMTCP snapshot at the
start poll (tools.adoption_gate building blocks; headless by default). `--render gpu --paced`
enables an output-paced full replay from boot. The service enforces EQUAL guards and prints one
JSON verdict (EQUAL | DIVERGED | INVALID) with reason codes. Receipts include non-default
render and pacing choices in their identity. See docs/t3416-gate-service.md.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import os
import re
import subprocess
import sys
import threading
import time
from collections.abc import Iterator
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from tools import (
    datadir,
    gate_data,
    gate_history,
    gate_route_roots,
    owner_files,
    verification_store,
)
from tools.rust_trace_call_args import TailTally, common_last_poll
from tools.wt import cache_keys

ROOT = Path(__file__).resolve().parents[1]
PROFILES = ROOT / "tools/data/gate_profiles.json"
DEFAULT_ON_ROOTS = ROOT / "tools/data/default_on_roots.json"
PORT_REGISTRY = ROOT / "docs/data/port-registry.json"
REGISTERED_PORT_GROUPS = frozenset(
    "anim physics char npc inventory cutscene save audio render hud input object pose leaf "
    "modes state weapons player".split()
)
MIN_WINDOW_SAMPLES = 600
DEFAULT_SECONDS = 180
DISC_NAME = owner_files.DISC_NAME
SCHEMA = 3
# The code that decides a verdict. Gate and control receipts are keyed on its digest, so a verdict
# computed by older comparison logic is never served from cache: a stale cached off/off
# `nondeterministic_baseline` kept failing every re-gate after the call-tail guard landed (T3702).
COMPARE_MODULES = (
    "tools/rust_trace_call_args.py",
    "tools/replay_clock.py",
    "tools/gate_host_profile.py",
    "tools/wsola_gate_activity.py",
    "tools/regress.py",
)
COMPARE_FUNCTIONS = (
    ("tools.gate", "window_samples"),
    ("tools.gate", "check_side"),
    ("tools.gate", "_capture_stop_reason"),
    ("tools.gate", "_check_captured_side"),
    ("tools.gate", "root_calls"),
    ("tools.gate", "_normalise_call_args_row"),
    ("tools.gate", "_past_horizon"),
    ("tools.gate", "call_args_determinism"),
    ("tools.gate", "check_roots"),
    ("tools.gate", "control_report_repeatable"),
    ("tools.gate", "distinct_control_captures"),
    ("tools.gate", "require_control_window"),
    ("tools.gate", "require_wsola_activity"),
    ("tools.gate_host_profile", "validate_wsola_run"),
    ("tools.wsola_gate_activity", "check"),
    ("tools.regress", "scan_log"),
    ("tools.gate", "record_call_tail"),
    ("tools.gate", "record_stack_residue"),
    ("tools.adoption_gate", "compare"),
    ("tools.rust_trace", "build_report"),
    ("tools.rust_trace", "cmd_diff"),
    ("tools.rust_trace", "compare_segment_windows"),
    ("tools.rust_trace", "comparable_route_offsets"),
    # T3835 (compare v9): whole-game credit needs a v9 verdict; changing the rule restales receipts
    ("tools.t2wholegame_v9", "judge"),
    ("tools.verification", "apply_whole_game_v9"),
)
_COMPARE_DIGEST: list[str] = []


def route_label(route: Path) -> str:
    """Route identity for gate history and verdicts.

    `rec_saves adopt` keeps an owner recording as `<folder>/input.rec`, so the bare stem `input`
    made every adopted recording look like the same route and a verdict for one (INVALID level 6)
    refused the others before replay. The folder name identifies those."""
    return route.parent.name if route.stem == "input" else route.stem


def compare_logic_digest() -> str:
    """sha256 prefix over COMPARE_MODULES and COMPARE_FUNCTIONS sources (cached per process)."""
    if not _COMPARE_DIGEST:
        import importlib
        import inspect

        digest = hashlib.sha256()
        root = Path(__file__).resolve().parents[1]
        for relative in COMPARE_MODULES:
            digest.update(relative.encode() + b"\0" + (root / relative).read_bytes())
        for module, name in COMPARE_FUNCTIONS:
            source = inspect.getsource(getattr(importlib.import_module(module), name))
            digest.update(f"{module}.{name}".encode() + b"\0" + source.encode())
        _COMPARE_DIGEST.append(digest.hexdigest()[:16])
    return _COMPARE_DIGEST[0]


DESYNC_SLACK_POLLS = 60  # same slack as replay_fidelity_baseline.json poll_slack


class GateInvalid(Exception):
    """A gate that cannot produce Tier 2 evidence; `code` is a stable reason code."""

    def __init__(self, code: str, message: str, *, details: dict | None = None) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.details = details or {}


def require_complete_segment_coverage(report: dict) -> None:
    """Reject partial segmented traces before classifying their state comparison."""
    segmented = report.get("segmented_comparison")
    if segmented is not None and not segmented.get("coverage_complete", True):
        raise GateInvalid(
            "segment_window_incomplete",
            "a segmented trace ended before the requested route window",
        )


# ---------------------------------------------------------------- argument resolution


def main_root() -> Path:
    from tools.trace_store import main_root as resolve

    return resolve()


def resolve_commit(cwd: Path, commit: str | None, branch: str | None) -> str:
    ref = commit or branch or "HEAD"
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"{ref}^{{commit}}"],
        cwd=cwd,
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode:
        raise GateInvalid("bad_commit", f"cannot resolve {ref!r} in {cwd}: {result.stderr.strip()}")
    return result.stdout.strip()


def resolve_route(spec: str, owner_root: Path) -> Path:
    """A path, a recording stem, or a short name (`story-level1` -> newest `story-level1-*.rec`)."""
    direct = Path(spec).expanduser()
    for candidate in (direct, *owner_files.candidates(ROOT, spec)):
        if candidate.suffix == ".rec" and candidate.is_file():
            return candidate.resolve()
    if spec == "r5":
        path = owner_files.resolve_path(
            ROOT, "tmp/routes/r5-story-gameplay-20261016.rec", kind="file"
        )
        if path.is_file():
            return path
    directories = owner_files.recording_dirs(ROOT)
    for directory in directories:
        exact = directory / f"{spec}.rec"
        if exact.is_file():
            return exact.resolve()
    pattern = re.compile(rf"^{re.escape(spec)}-\d{{8}}(?:-\d{{4}})?$")
    matches = sorted(
        (
            path
            for directory in directories
            if directory.is_dir()
            for path in directory.glob("*.rec")
        ),
        key=lambda path: path.stem,
    )
    named = [path for path in matches if pattern.match(path.stem)]
    if not named:
        known = sorted({re.sub(r"-\d{8}(?:-\d{4})?$", "", path.stem) for path in matches})
        raise GateInvalid(
            "route_not_found", f"route {spec!r} not found; known routes: {', '.join(known)}"
        )
    return named[-1].resolve()  # stems end in the recording date: newest wins


def route_family(stem: str) -> str:
    """Return the profile family for a dated recording stem, including legacy aliases."""
    family = re.sub(r"-\d{8}(?:-\d{4})?$", "", stem)
    if family == "r5-story-gameplay":
        return "r5"
    if family in {"story-level1-v6", "story-arcade-transitions-v6"}:
        return family.removesuffix("-v6")
    return family


def _auto_route(
    owner_root: Path,
    *,
    roots: tuple[int, ...],
    allowed: tuple[str, ...],
    start: str | int | None,
    default_start: str | int | None,
    exclude: str | None = None,
) -> Path:
    """Pick the currently available route with strongest root coverage, then shortest window."""
    rows = gate_route_roots.load(owner_root)
    names = sorted({str(row.get("route")) for row in rows if row.get("route")})
    choices: list[tuple[int, int, str, Path]] = []
    backend = Backend(owner_root)

    for name in names:
        if name == exclude:
            continue
        try:
            recording = resolve_route(name, owner_root)
            family = route_family(recording.stem)
            if allowed and family not in allowed:
                continue
            resolved_start = resolve_start(start, recording, default_start)
            recording_sha = backend.sha256(recording)
            matching = [
                row
                for row in rows
                if row.get("route") == recording.stem
                and row.get("recording_sha256") == recording_sha
                and row.get("start") == resolved_start
                and isinstance(row.get("end"), int)
                and int(row.get("samples") or 0) >= MIN_WINDOW_SAMPLES
            ]
            if not matching:
                continue
            covered = set()
            for row in matching:
                covered.update(gate_route_roots._roots(row.get("roots_present", [])))
            coverage = len(covered & set(roots))
            if coverage <= 0:
                continue
            observed_window = min(max(0, int(row["end"]) - resolved_start) for row in matching)
            choices.append((-coverage, observed_window, recording.stem, recording))
        except (GateInvalid, OSError, ValueError):
            continue
    if not choices:
        raise GateInvalid(
            "route_coverage_unknown",
            "--route auto found no available route with measured coverage for the requested roots "
            "and start window; use a known route or run a gate capture that records these roots",
        )
    return min(choices)[3]


def route_marks(recording: Path) -> list[tuple[int, int, str]]:
    """F12 marks of a recording: `M <tick> <poll> <ns> <hex label>` -> (tick, poll, label)."""
    marks = []
    with recording.open("rb") as handle:
        for raw in handle:
            if not raw.startswith(b"M\t") and not raw.startswith(b"M "):
                continue
            parts = raw.decode(errors="replace").split()
            if len(parts) < 3:
                continue
            try:
                label = bytes.fromhex(parts[4]).decode(errors="replace") if len(parts) > 4 else ""
            except ValueError:
                label = parts[4]
            marks.append((int(parts[1]), int(parts[2]), label))
    return marks


def measured_route_end_poll(recording: Path) -> int | None:
    """Guest-poll route end from the SHA-bound mark table, when available."""
    from tools import gate_marks

    entry = gate_marks.load_table().get(recording.stem)
    if not isinstance(entry, dict) or entry.get("recording_sha256") != gate_marks.recording_sha256(
        recording
    ):
        return None
    # Legacy entries contain recorded host polls, not guest-pad polls.
    if entry.get("route_end_clock") != "guest_pad_poll":
        return None
    value = entry.get("route_end_poll")
    return value if isinstance(value, int) and value >= 0 else None


def preflight_window(
    recording: Path, start: int, seconds: int, route_end_poll: int | None = None
) -> None:
    """Refuse requests whose requested replay window extends beyond measured route end, or beyond
    the route's known replay desync (T3819: evidence past it is from a wrong game state)."""
    from tools import replay_desync

    limit = replay_desync.coverage_until_poll(recording.stem)
    if limit is not None and start + seconds > limit:
        front = replay_desync.front_end_only(recording.stem)
        raise GateInvalid(
            "window_past_known_desync",
            f"route {recording.stem} desyncs from the owner's session at replay poll {limit}"
            f"{' (front end only, no gameplay)' if front else ''}; "
            f"--start {start} plus --seconds {seconds} goes past it. Gate on an in-sync route "
            "(tools/data/replay_fidelity_baseline.json, docs/t-replay-desync-l3-l4.md)",
            details={"coverage_until_poll": limit, "start_poll": start, "seconds": seconds},
        )
    route_end = route_end_poll
    if route_end is None:
        route_end = measured_route_end_poll(recording)
    if route_end is None or start + seconds <= route_end:
        return
    fit_seconds = max(1, route_end - start)
    raise GateInvalid(
        "window_exceeds_route_end",
        f"route {recording.stem} ends at replay poll {route_end}; requested --start {start} "
        f"plus --seconds {seconds} exceeds it. Use --seconds {fit_seconds} or an earlier "
        "--start (the route-end limit is SHA-bound in tools/data/route_mark_polls.json)",
        details={"route_end_poll": route_end, "start_poll": start, "seconds": seconds},
    )


def denylist_path() -> Path:
    """Shared gate deny list; tests and isolated runners can point at their own file."""
    configured = os.environ.get("TSFP_GATE_DENY_FILE")
    return Path(configured) if configured else datadir.data_dir("codexapi/gate-deny.txt")


def preflight_profile_route(profile: str | None, route: str, path: Path | None = None) -> None:
    """Refuse a profile/route that the fleet has explicitly marked unsafe to repeat."""
    deny_file = path or denylist_path()
    try:
        lines = deny_file.read_text().splitlines()
    except FileNotFoundError:
        return
    except OSError as error:
        raise GateInvalid(
            "denylist_unreadable", f"cannot read gate deny list {deny_file}: {error}"
        ) from None
    target = f"{profile or 'gate-custom'} {route}"
    for raw in lines:
        entry = raw.split("#", 1)[0].strip()
        if entry == target:
            raise GateInvalid(
                "profile_route_denied",
                f"profile/route {target!r} is on {deny_file}; choose another route or resolve "
                "the recorded failure",
                details={"deny_file": str(deny_file), "profile": profile, "route": route},
            )


def preflight_host_switches(spec: GateSpec, *, explicit_switches: tuple[str, ...] = ()) -> None:
    """Resolve selectors against the host's accepted selector language before admission.

    This mirrors the argument accepted by src/host/rust_ports.c.  The generated registry
    is the checked source for root addresses and profile/group selectors, so a rejected
    selector never reaches host build or replay.
    """
    try:
        validate_registered_inputs(
            (*spec.switches, *spec.root_selectors),
            spec.roots,
            explicit_switches=explicit_switches,
        )
    except GateInvalid as error:
        code = (
            "host_refused_switches"
            if error.code in {"unknown_switch", "registry_only_selector"}
            else error.code
        )
        raise GateInvalid(
            code,
            f"candidate host cannot resolve profile switches: {error.message}",
            details={"switch_error": error.code, **error.details},
        ) from None


def mark_replay_poll(recording: Path, index: int, user_marks: list[tuple[int, int, str]]) -> int:
    """Replay (guest port-0) poll of the `index`-th F12 mark, from the measured mark table.

    The recorded poll of a mark counts live host polls (about 2 per tick) and is not a replay
    poll: using it as a window start put weapon-lifecycle at 11237 of a replay that ends at 8632,
    so the snapshot was never taken (T3607). An unmeasured recording is refused with the command
    that measures it.
    """
    from tools import gate_marks

    measured = gate_marks.measured_user_marks(recording)
    if measured is None or len(measured) < len(user_marks):
        raise GateInvalid(
            "mark_polls_unmeasured",
            f"{recording.name} has {len(user_marks)} F12 mark(s) but no replay-poll measurement: "
            f"their recorded polls ({', '.join(str(mark[1]) for mark in user_marks)}) count live "
            "host polls and are not replay polls. Run "
            f"`python3 -m tools.gate_marks measure --route {recording.stem} --write` "
            "or pass --start <replay poll>",
        )
    return int(measured[index - 1]["guest_pad_poll"])


def resolve_start(spec: str | None, recording: Path, profile_start: str | None = None) -> int:
    """Poll number, `mark:N` (N-th owner F12 mark, 1-based), `gameplay`, or `boot` (0)."""
    spec = (spec or profile_start or "gameplay").strip()
    if spec in ("boot", "0"):
        return 0
    if spec.isdigit():
        return int(spec)
    user_marks = [mark for mark in route_marks(recording) if mark[2] != "record-start"]
    if spec == "gameplay":
        from tools.adoption_gate import gameplay_start_poll

        poll = gameplay_start_poll(recording)
        if poll:
            return poll
        if user_marks:
            return mark_replay_poll(recording, 1, user_marks)
        raise GateInvalid(
            "no_gameplay_mark",
            f"{recording.name} has no gameplay_start_polls.json entry and no F12 marks; "
            "pass --start <poll>",
        )
    match = re.fullmatch(r"mark:(\d+)", spec)
    if match:
        index = int(match.group(1))
        if not 1 <= index <= len(user_marks):
            raise GateInvalid(
                "bad_start",
                f"{recording.name} has {len(user_marks)} F12 marks "
                f"(polls {', '.join(str(mark[1]) for mark in user_marks) or 'none'}); "
                f"mark:{index} does not exist",
            )
        return mark_replay_poll(recording, index, user_marks)
    raise GateInvalid("bad_start", f"--start {spec!r}: use a poll, mark:N, gameplay or boot")


def parse_roots(text: str | None) -> tuple[int, ...]:
    if not text:
        return ()
    try:
        return tuple(sorted({int(item.strip(), 16) for item in text.split(",") if item.strip()}))
    except ValueError as error:
        raise GateInvalid("bad_roots", f"--roots must be hex VAs: {error}") from None


# T3702: how a profile's switches enable the Rust path. "rust-ports": the roots are --rust-ports
# selectors (default, also the safe choice for env-only profiles whose lane root needs selecting,
# T3499); "env": TSFP_* env switches enable it and the roots are coverage roots only.
SWITCH_KINDS = ("rust-ports", "env")


def load_profiles(path: Path = PROFILES) -> dict:
    return json.loads(path.read_text())["profiles"]


def canonical_va(value: str | int) -> str:
    return f"0x{int(value, 16) if isinstance(value, str) else value:x}"


def registry_only_selectors(
    switches: tuple[str, ...],
    roots: tuple[int, ...] = (),
    registry_path: Path = PORT_REGISTRY,
) -> list[str]:
    """Hex selectors registered only as roots (no selectable `fn` body) and not passed as roots."""
    registry = json.loads(registry_path.read_text()).get("ports", {})
    port_vas = {
        canonical_va(va)
        for va, rows in registry.items()
        if any(set(row.get("forms", ())) & {"roots", "fn"} for row in rows)
    }
    selectable_vas = {
        canonical_va(va)
        for va, rows in registry.items()
        if any("fn" in row.get("forms", ()) for row in rows)
    }
    root_vas = {canonical_va(root) for root in roots}
    return [
        switch
        for switch in switches
        if re.fullmatch(r"0[xX][0-9a-fA-F]+", switch)
        and canonical_va(switch) in port_vas
        and canonical_va(switch) not in selectable_vas
        and canonical_va(switch) not in root_vas
    ]


def validate_registered_inputs(
    switches: tuple[str, ...],
    roots: tuple[int, ...],
    *,
    explicit_switches: tuple[str, ...] = (),
    profiles_path: Path = PROFILES,
    registry_path: Path = PORT_REGISTRY,
) -> None:
    """Reject selectors and roots that cannot name a registered Rust implementation."""
    profile_doc = json.loads(profiles_path.read_text())
    known = {"off", "all", "rea-inventory-native", *REGISTERED_PORT_GROUPS}
    known.update(profile_doc.get("profiles", {}).keys())
    known.update(profile_doc.get("adopted", {}).get("switches", []))
    for profile in profile_doc.get("profiles", {}).values():
        known.update(profile.get("switches", []))
        known.update(profile.get("env", []))
        known.update(profile.get("baseline_env", []))
        known.update(profile.get("baseline_host_args", []))
    for dependency in profile_doc.get("dependencies", []):
        known.update(dependency.get("env", []))
    owner_checks = ROOT / "docs/owner-checks.md"
    if owner_checks.is_file():
        source = owner_checks.read_text()
        known.update(
            f"{match.group(1)}={match.group(2)}"
            for match in re.finditer(r"(TSFP_[A-Z0-9_]+)=([^\s`;&,]+)", source)
        )
        known.update(
            match.group(0) for match in re.finditer(r"--rust-[a-z0-9-]+(?:=[^\s`;&,]+)?", source)
        )

    registry = json.loads(registry_path.read_text()).get("ports", {})
    port_vas = {
        canonical_va(va)
        for va, rows in registry.items()
        if any(set(row.get("forms", ())) & {"roots", "fn"} for row in rows)
    }
    registry_only = registry_only_selectors(explicit_switches, roots, registry_path)
    if registry_only:
        raise GateInvalid(
            "registry_only_selector",
            f"selector(s) {', '.join(registry_only)} are registered roots without a selectable "
            "function body; pass them through --roots or use a supported Rust port group",
        )
    unknown = []
    for switch in switches:
        if re.fullmatch(r"0[xX][0-9a-fA-F]+", switch):
            va = canonical_va(switch)
            if va not in port_vas:
                unknown.append(switch)
        elif switch not in known:
            unknown.append(switch)
    if unknown:
        raise GateInvalid(
            "unknown_switch",
            f"unregistered gate switch(es): {', '.join(unknown)}; use a selector from "
            "tools/data/gate_profiles.json or a registered address from "
            "docs/data/port-registry.json",
        )
    unknown_roots = [root for root in roots if canonical_va(root) not in port_vas]
    if unknown_roots:
        rendered = ", ".join(canonical_va(root) for root in unknown_roots)
        raise GateInvalid(
            "roots_not_registered",
            f"root(s) {rendered} are absent from docs/data/port-registry.json; "
            "register the Rust root before using it in a Tier 2 gate",
        )


@dataclass
class GateSpec:
    """Fully resolved gate request; everything a verdict depends on."""

    commit: str
    route: Path
    start: int
    switches: tuple[str, ...]
    shared_env: tuple[str, ...]
    regions: Path
    roots: tuple[int, ...]
    seconds: int
    root_selectors: tuple[str, ...] = ()
    switch_kind: str = "rust-ports"
    coverage_only_roots: tuple[str, ...] = ()
    profile: str | None = None
    host_profile: str = "gate"
    baseline_host_profile: str = "gate"
    same_host: bool = False
    shared_host_args: tuple[str, ...] = ()
    wsola_activity: bool = False
    allowed_routes: tuple[str, ...] = ()
    adopted: tuple[str, ...] = ()
    baseline_env: tuple[str, ...] = ()
    baseline_host_args: tuple[str, ...] = ()
    route_end_poll: int | None = None
    snapshot: bool = False
    segments: int | None = None
    retry_known_invalid: bool = False
    render: str = "none"
    paced: bool = False
    notes: list[str] = field(default_factory=list)


def prefer_remote_full_capture(spec: GateSpec) -> None:
    """Avoid local-only DMTCP resume paths when the caller enabled remote replay."""
    if spec.snapshot:
        spec.snapshot = False
        spec.notes.append(
            "remote replay uses full-route capture; cached DMTCP resumes are local-only"
        )
    if spec.segments is not None:
        spec.segments = None
        spec.notes.append(
            "remote replay uses one full-route capture instead of local snapshot segments"
        )


def configure_remote_capture(spec: GateSpec, remote: dict[str, object]) -> bool:
    """Use full-route capture only when at least one remote replay host is admissible."""
    if not remote.get("hosts"):
        return False
    prefer_remote_full_capture(spec)
    return True


def resolve_spec(args: argparse.Namespace, owner_root: Path) -> GateSpec:
    profile = {}
    notes: list[str] = []
    if args.profile:
        profiles = load_profiles()
        if args.profile not in profiles:
            raise GateInvalid(
                "bad_profile", f"unknown profile {args.profile!r}; known: {', '.join(profiles)}"
            )
        profile = profiles[args.profile]
    switch_text = (
        args.switches if args.switches is not None else ",".join(profile.get("switches", []))
    )
    # --switches (Rust port selectors, host flags, TSFP_*=v) apply to the candidate only;
    # profile `env` and declared dependencies apply to both sides (host modes such as the
    # Rust frame loop that the lifted reference must also run under).
    selectors = tuple(item.strip() for item in switch_text.split(",") if item.strip())
    env = list(profile.get("env", []))
    if getattr(args, "audio_command_log", False):
        if args.profile != "audio":
            raise GateInvalid(
                "audio_capture_profile", "--audio-command-log requires --profile audio"
            )
        env.append("TSFP_AUDIO_CMD_LOG=@capture/audio.trace")
    for rule in load_dependency_rules():
        if any(re.fullmatch(rule["selector"], item) for item in selectors):
            for needed in rule["env"]:
                if needed not in env:
                    env.append(needed)
                    notes.append(f"{needed} added on both sides: {rule['why']}")
    # Adopted systems run on both sides (gate other systems on top of main's defaults); a
    # profile may opt out ("adopted": false) to reproduce an earlier gate's conditions.
    adopted: list[str] = []
    if profile.get("default_ports") is True:
        # T3784: run both sides under play.sh's default port set so the route reaches gameplay;
        # the sides differ only by the system switch (rule 24).
        adopted = load_default_port_selectors()
    elif not getattr(args, "no_adopted", False) and profile.get("adopted", True) is not False:
        lowered = {item.lower() for item in selectors}
        for item in load_adopted():
            if item.lower() not in lowered:
                adopted.append(item)
    baseline_env = list(profile.get("baseline_env", []))
    baseline_host_args_text = getattr(args, "baseline_host_args", None)
    baseline_host_args = tuple(
        item.strip()
        for item in (
            baseline_host_args_text
            if baseline_host_args_text is not None
            else ",".join(profile.get("baseline_host_args", []))
        ).split(",")
        if item.strip()
    )
    if not selectors:
        raise GateInvalid("no_switches", "no Rust port selector or host switch in --switches")
    regions_text = args.regions or profile.get("regions")
    if not regions_text:
        raise GateInvalid("no_regions", "pass --regions FILE or a --profile that declares one")
    regions = Path(regions_text)
    if not regions.is_absolute():
        regions = ROOT / regions
    if not regions.is_file():
        raise GateInvalid("bad_regions", f"regions file not found: {regions}")
    roots = parse_roots(
        args.roots if args.roots is not None else ",".join(profile.get("roots", []))
    )
    allowed_routes = tuple(profile.get("routes", []))
    route_text = args.route or profile.get("route")
    if not route_text:
        raise GateInvalid("no_route", "pass --route or a --profile with a default route")
    if route_text == "auto":
        recording = _auto_route(
            owner_root,
            roots=roots,
            allowed=allowed_routes,
            start=args.start,
            default_start=profile.get("start"),
        )
    else:
        recording = resolve_route(route_text, owner_root)
    resolved_route_family = route_family(recording.stem)
    profile_route_family = resolved_route_family
    if allowed_routes and resolved_route_family not in allowed_routes:
        raise GateInvalid(
            "profile_route_unsupported",
            f"profile {args.profile!r} was measured for routes {', '.join(allowed_routes)}; "
            f"route {profile_route_family!r} is unsupported",
        )
    start = resolve_start(args.start, recording, profile.get("start"))
    route_end_poll = measured_route_end_poll(recording)
    preflight_window(recording, start, args.seconds, route_end_poll)
    segments = getattr(args, "segments", None)
    if segments is not None and (segments < 1 or start < 1):
        raise GateInvalid("bad_segments", "--segments needs K >= 1 and a nonzero --start")
    render = getattr(args, "render", "none") or "none"
    paced = bool(getattr(args, "paced", False))
    if render not in {"none", "gpu"}:
        raise GateInvalid("bad_render", f"unsupported gate render mode {render!r}")
    if paced and render != "gpu":
        raise GateInvalid("paced_requires_gpu", "--paced requires --render gpu")
    if render == "gpu" and (start != 0 or segments is not None):
        raise GateInvalid(
            "render_requires_boot",
            "--render gpu requires a full replay from boot; snapshots and segmented captures are "
            "headless",
        )
    root_selectors = tuple(str(item).strip() for item in profile.get("root_selectors", []))
    switch_kind = profile.get("switch_kind", "rust-ports")
    if switch_kind not in SWITCH_KINDS:
        raise GateInvalid(
            "bad_switch_kind",
            f"profile {args.profile!r} switch_kind {switch_kind!r}; "
            f"known: {', '.join(SWITCH_KINDS)}",
        )
    explicit_switches = tuple(
        item.strip() for item in (args.switches or "").split(",") if item.strip()
    )
    preflight_host_switches(
        GateSpec(
            commit="",
            route=recording,
            start=start,
            switches=selectors,
            shared_env=tuple(sorted(env)),
            regions=regions.resolve(),
            roots=roots,
            root_selectors=root_selectors,
            switch_kind=switch_kind,
            seconds=args.seconds,
        ),
        explicit_switches=explicit_switches,
    )
    host_profile = str(profile.get("candidate_host_profile", "gate"))
    baseline_profile = str(profile.get("baseline_host_profile", "gate"))
    same_host = bool(profile.get("same_host", False))
    from tools import gate_host_profile

    try:
        gate_host_profile.validate_profiles(
            ROOT, host_profile, baseline_profile, same_host=same_host
        )
    except (ValueError, OSError) as error:
        raise GateInvalid("invalid_host_profile", str(error)) from error
    if profile.get("wsola_activity") and (
        args.render != "gpu"
        or not args.paced
        or args.seconds < 180
        or bool(getattr(args, "snapshot", True))
    ):
        raise GateInvalid(
            "wsola_preflight",
            "WSOLA gate requires GPU, paced, full replay and at least 180 seconds",
        )
    return GateSpec(
        commit=resolve_commit(Path.cwd(), args.commit, args.branch),
        route=recording,
        start=start,
        switches=selectors,
        shared_env=tuple(sorted(env)),
        regions=regions.resolve(),
        roots=roots,
        root_selectors=root_selectors,
        switch_kind=switch_kind,
        coverage_only_roots=tuple(str(item) for item in profile.get("coverage_only_roots", [])),
        seconds=args.seconds,
        profile=args.profile,
        host_profile=host_profile,
        baseline_host_profile=baseline_profile,
        same_host=same_host,
        shared_host_args=tuple(profile.get("shared_host_args", [])),
        wsola_activity=bool(profile.get("wsola_activity", False)),
        allowed_routes=allowed_routes,
        adopted=tuple(adopted),
        snapshot=bool(getattr(args, "snapshot", True)),
        baseline_env=tuple(baseline_env),
        segments=getattr(args, "segments", None),
        baseline_host_args=baseline_host_args,
        route_end_poll=route_end_poll,
        retry_known_invalid=gate_history.retry_requested(
            bool(getattr(args, "retry_known_invalid", False))
        ),
        render=render,
        paced=paced,
        notes=notes,
    )


def segment_checkpoint_polls(start: int, route_end: int, segments: int) -> list[int]:
    """Use one eight-boundary cache grid for K=1/4/8, expanding only for larger K."""
    from tools.adoption_gate import poll_segments

    if segments < 1 or segments > 64:
        raise GateInvalid("bad_segments", "segmented gates support K from 1 through 64")
    count = max(8, segments)
    return [first for first, _last in poll_segments(start, route_end, count)]


def load_default_port_selectors(path: Path = DEFAULT_ON_ROOTS) -> list[str]:
    """Selectors of play.sh's default `--rust-ports` spec (single source: default_on_roots.json)."""
    spec = json.loads(path.read_text())["spec"]
    return [item.strip() for item in spec.split(",") if item.strip()]


def load_adopted(path: Path = PROFILES) -> list[str]:
    return list(json.loads(path.read_text()).get("adopted", {}).get("switches", []))


def load_dependency_rules(path: Path = PROFILES) -> list[dict]:
    return json.loads(path.read_text()).get("dependencies", [])


# ---------------------------------------------------------------- guards


def window_samples(trace: dict, start: int) -> list[dict]:
    return [sample for sample in trace.get("samples", []) if sample.get("poll", 0) >= start]


def region_widths(region_doc: dict) -> dict[str, int]:
    widths = {}
    for item in region_doc.get("regions", []):
        width_text = str(item.get("spec", "")).rpartition(":")[2]
        try:
            widths[item["name"]] = int(
                width_text, 16 if width_text.lower().startswith("0x") else 10
            )
        except ValueError:
            widths[item["name"]] = -1
    return widths


def check_side(
    label: str,
    trace: dict,
    start: int,
    widths: dict[str, int],
    route_end_poll: int | None = None,
    capture_stop_reason: str | None = None,
) -> dict:
    """Window guards for one capture; raises GateInvalid with an actionable message."""
    polls = [sample.get("poll", 0) for sample in trace.get("samples", [])]
    meta = trace.get("meta", {})
    observed_route_end = meta.get("route_end_poll")
    recorded_route_end = meta.get("recorded_route_end_poll")
    # The marker and recorded endpoint share the recording clock; the optional
    # gate window endpoint uses guest polls and cannot be compared with them.
    expected_route_end = recorded_route_end
    completed_host_close = (
        capture_stop_reason is not None
        and "host shutdown requested" in capture_stop_reason
        and meta.get("host_exit") == 0
        and isinstance(observed_route_end, int)
        and observed_route_end > 0
        and isinstance(expected_route_end, int)
        and expected_route_end > 0
        and observed_route_end >= expected_route_end
        and (not isinstance(recorded_route_end, int) or observed_route_end >= recorded_route_end)
    )
    if (
        polls
        and capture_stop_reason
        and (
            "STOPPED:" in capture_stop_reason
            or "stopped: Rust port bridge contract broken" in capture_stop_reason
        )
        and not completed_host_close
    ):
        raise GateInvalid(
            "guest_fault",
            f"{label} guest stopped despite the host exit status: {capture_stop_reason}",
            details={"side": label, "stop_reason": capture_stop_reason},
        )
    last = max(polls, default=0)
    desync_poll = meta.get("desync_poll")
    if isinstance(desync_poll, int) and polls and last > desync_poll + DESYNC_SLACK_POLLS:
        # T3914: the capture's own desync detector (tools.replay_desync) says the replay left the
        # owner's session at desync_poll; samples past it come from a wrong game state, so a
        # verdict on them is not evidence. The preflight cannot see this because --seconds is
        # wall time, not polls (paced gate 10e9c26f sampled to poll 16795, desync at 4998).
        raise GateInvalid(
            "window_past_capture_desync",
            f"{label} capture desyncs from the owner's session at replay poll {desync_poll} but "
            f"sampled to poll {last}; shorten --seconds or gate on an in-sync route "
            "(docs/t-audio-paced-gate-20261030.md)",
            details={"side": label, "desync_poll": desync_poll, "last_poll": last},
        )
    if not polls:
        # Only a route-end marker from this capture proves that playback reached
        # the recording end. The recording's known end is useful for preflight,
        # but must not turn an early clean host exit into a route_end diagnosis.
        route_end = trace.get("meta", {}).get("route_end_poll")
        if not widths:
            reason = "regions_empty"
            explanation = "the capture had no selected regions"
        elif isinstance(route_end, int) and route_end > 0:
            reason = "route_end"
            explanation = f"the recorded route ended at poll {route_end} without trace samples"
        elif capture_stop_reason and " was refused. Forms:" in capture_stop_reason:
            # T3607: the host rejected a switch (exit 2) before any guest code ran: a request
            # error (for example root addresses passed as --rust-ports), not an empty route.
            reason = "host_option_refused"
            explanation = (
                f"the {label} host refused its options before the first poll: {capture_stop_reason}"
            )
        elif capture_stop_reason:
            reason = (
                "guest_stop"
                if "XDK function with no implementation --" in capture_stop_reason
                else "startup_failure"
            )
            explanation = f"guest execution stopped before trace samples: {capture_stop_reason}"
        else:
            reason = "writer_error"
            explanation = "the route capture returned no trace samples and no route-end marker"
        raise GateInvalid(
            "CAPTURE_EMPTY",
            f"{label} capture produced 0 samples: {explanation}; "
            "check the route, region selection, "
            "and trace writer",
            details={
                "side": label,
                "reason": reason,
                "last_poll_reached": last,
                "requested_start": start,
                "samples_reached": 0,
            },
        )
    if last < start:
        route_end = trace.get("meta", {}).get("route_end_poll")
        hint = "raise --seconds or pick an earlier mark"
        if isinstance(route_end, int) and route_end > 0:
            hint = (
                f"the replay reached the route end (recorded poll {route_end}), so more seconds "
                "cannot help; replay polls (guest port-0) are fewer than the recorded host polls "
                "(about 1 per tick against 2), take a start from `python3 -m tools.gate_marks "
                "show --route <route>` or pass a smaller --start"
            )
        raise GateInvalid(
            f"{label}_window_not_reached",
            f"{label} capture ended at poll {last} before the window start {start}; {hint}",
            details={"side": label, "last_poll_reached": last, "requested_start": start},
        )
    window = window_samples(trace, start)
    if len(window) < MIN_WINDOW_SAMPLES:
        raise GateInvalid(
            f"{label}_too_few_samples",
            f"{label} has {len(window)} samples in window {start}..{last} "
            f"(need {MIN_WINDOW_SAMPLES}); raise --seconds or pick an earlier mark",
            details={
                "side": label,
                "last_poll_reached": last,
                "requested_start": start,
                "samples_reached": len(window),
                "samples_required": MIN_WINDOW_SAMPLES,
            },
        )
    varying: list[str] = []
    for name, width in widths.items():
        if width <= 0:
            raise GateInvalid("bad_region_width", f"region {name} has no byte width in its spec")
        values = [sample.get("regions", {}).get(name) for sample in window]
        missing = sum(1 for value in values if value is None)
        if missing:
            raise GateInvalid(
                f"{label}_region_missing",
                f"region {name} absent from {missing} {label} window samples",
            )
        # "" is an unreadable sample (a pointer region before its object exists).
        wrong = {len(value) // 2 for value in values if value and len(value) != width * 2}
        if wrong:
            raise GateInvalid(
                "region_width_mismatch",
                f"region {name}: spec width {width} bytes, {label} trace holds "
                f"{sorted(wrong)} bytes; fix the regions file to the struct size",
            )
        if len(set(values)) > 1 and any(value.strip("0") for value in values):
            varying.append(name)
    if not varying:
        raise GateInvalid(
            "regions_inert",
            f"no region is nonzero and varying in the {label} window {start}..{last}; "
            "the window is not exercising the system, pick another route or mark",
            details={"side": label, "last_poll_reached": last, "requested_start": start},
        )
    return {
        "samples": len(window),
        "first_poll": min(s["poll"] for s in window),
        "last_poll": last,
        "varying_regions": sorted(varying),
    }


def root_calls(trace_dir: Path, trace: dict, roots: tuple[int, ...], start: int) -> dict[str, dict]:
    """Calls per root in the window (call-args stream), else whole-run Rust counters."""
    counts = {f"0x{root:X}": {"window": None, "rust_total": 0} for root in roots}
    for key, value in trace.get("counters", {}).items():
        name = f"0x{int(key, 16):X}"
        if name in counts:
            counts[name]["rust_total"] = int(value.get("rust", 0))
    path = trace_dir / trace.get("meta", {}).get("call_args_file", "call-args.trace")
    if path.is_file():
        wanted = set(roots)
        for name in counts:
            counts[name]["window"] = 0
        with path.open() as handle:
            for line in handle:
                parts = line.split(maxsplit=3)
                if len(parts) < 3 or parts[2] != "0":
                    continue
                try:
                    poll, va = int(parts[0]), int(parts[1], 16)
                except ValueError:
                    continue
                if va in wanted and poll >= start:
                    counts[f"0x{va:X}"]["window"] += 1
    else:
        # rust_trace embeds small call-args captures in trace.json. Snapshot gates can
        # produce this form when the capture stays below the embedding limit; count the
        # same [poll, VA, direction, ...] entry rows as the external line format above.
        inline = trace.get("call_args")
        if isinstance(inline, list):
            wanted = set(roots)
            for name in counts:
                counts[name]["window"] = 0
            for row in inline:
                if not isinstance(row, (list, tuple)) or len(row) < 3:
                    continue
                try:
                    poll, va, direction = (int(value) for value in row[:3])
                except (TypeError, ValueError):
                    continue
                if direction == 0 and va in wanted and poll >= start:
                    counts[f"0x{va:X}"]["window"] += 1
    return counts


def _normalise_call_args_row(row: str | list | tuple) -> tuple | str:
    """Apply only documented ABI masks; keep every poll, root and other word exact."""
    if isinstance(row, str):
        fields = row.split()
        if len(fields) < 5:
            return row.rstrip("\n")
        try:
            va, phase = int(fields[1], 16), int(fields[2], 10)
            fields = [int(fields[0], 10), va, phase, *(int(value, 16) for value in fields[3:])]
        except ValueError:
            return row.rstrip("\n")
        if va == 0x306460 and phase == 1:
            fields[3] = "<undefined-void-eax>"
        return tuple(fields)
    fields = list(row)
    if len(fields) >= 5:
        try:
            va, phase = int(fields[1]), int(fields[2])
        except (TypeError, ValueError):
            return tuple(fields)
        if va == 0x306460 and phase == 1:
            fields[3] = "<undefined-void-eax>"
    return tuple(fields)


def _past_horizon(row: object, horizon: int | None, side: str, tail: TailTally) -> bool:
    """True for a row after the common last poll; every row is tallied per root."""
    try:
        poll, root = int(row[0]), int(row[1])
    except (TypeError, ValueError, IndexError):
        return False
    past = horizon is not None and poll > horizon
    tail.note(side, root, past)
    return past


def call_args_determinism(left_dir: Path, right_dir: Path) -> dict:
    """Compare off/off call-args rows with bounded memory; retain no raw arguments."""

    left_trace, right_trace = _load_trace(left_dir), _load_trace(right_dir)

    # An explicitly referenced external stream is authoritative. In particular,
    # two missing files are not two equal empty streams after cache cleanup.
    for side, directory, trace in (
        ("baseline", left_dir, left_trace),
        ("control", right_dir, right_trace),
    ):
        name = trace.get("meta", {}).get("call_args_file")
        if name and not (directory / name).is_file():
            return {
                "comparable": True,
                "equal": False,
                "reason": "missing_call_args_file",
                "side": side,
            }

    poll_horizon = common_last_poll(left_trace, right_trace)
    tail = TailTally(poll_horizon)

    def rows(directory: Path, trace: dict, side: str) -> Iterator[dict[str, object]]:
        name = trace.get("meta", {}).get("call_args_file", "call-args.trace")
        path = directory / name
        if path.is_file():
            with path.open(errors="replace") as stream:
                for line in stream:
                    row = _normalise_call_args_row(line)
                    if _past_horizon(row, poll_horizon, side, tail):
                        continue
                    yield row
            return
        inline = trace.get("call_args")
        if isinstance(inline, list):
            for row in inline:
                normalized = _normalise_call_args_row(row)
                if _past_horizon(normalized, poll_horizon, side, tail):
                    continue
                yield normalized

    left_path = left_dir / left_trace.get("meta", {}).get("call_args_file", "call-args.trace")
    right_path = right_dir / right_trace.get("meta", {}).get("call_args_file", "call-args.trace")
    if not left_path.is_file() and not right_path.is_file():
        left_inline = left_trace.get("call_args")
        right_inline = right_trace.get("call_args")
        if not isinstance(left_inline, list) and not isinstance(right_inline, list):
            return {"comparable": False, "reason": "call_args_not_recorded"}
    left_iter, right_iter = (
        iter(rows(left_dir, left_trace, "left")),
        iter(rows(right_dir, right_trace, "right")),
    )
    index = 0
    while True:
        left, right = next(left_iter, None), next(right_iter, None)
        if left is None and right is None:
            return {
                "comparable": True,
                "equal": True,
                "rows_compared": index,
                "poll_horizon": poll_horizon,
                **tail.summary(),
            }
        if left != right:
            polls = []
            for differing_row in (left, right):
                if differing_row is None:
                    continue
                try:
                    polls.append(int(differing_row[0]))
                except (TypeError, ValueError, IndexError):
                    match = re.match(r"\s*(\d+)", str(differing_row))
                    if match:
                        polls.append(int(match.group(1)))
            poll = min(polls) if polls else None
            return {
                "comparable": True,
                "equal": False,
                "first_row": index + 1,
                "poll": poll,
                "poll_horizon": poll_horizon,
                **tail.summary(),
                "left_sha256": hashlib.sha256(repr(left).encode()).hexdigest(),
                "right_sha256": hashlib.sha256(repr(right).encode()).hexdigest(),
            }
        index += 1


def check_stop_report(trace: dict) -> None:
    """T3783: a Rust-port candidate must have stopped cleanly and printed its stop report.

    `host_rust_ports_report` (src/host/main.c) prints the `rust port 0x... rust=N` lines that
    become `trace.counters`; `meta.host_exit` is None when the host died or hung before exit.
    Without both, zero Rust counts mean "report lost", not "ran in C", so fail closed (INVALID)
    instead of roots_not_rust, and never let coverage-only roots pass on an empty report.
    """
    meta = trace.get("meta") or {}
    missing = []
    if meta.get("host_exit") is None:
        missing.append("host_exit")
    if not trace.get("counters"):
        missing.append("rust_port_lines")
    if missing:
        raise GateInvalid(
            "stop_report_missing",
            f"candidate stop report missing ({', '.join(missing)}): the host was built with Rust "
            "ports but died or hung before host_rust_ports_report, so the Rust counters are lost; "
            "check the candidate run.log tail (T3783)",
            details={"stop_report_missing": missing},
        )


def check_roots(
    calls: dict[str, dict],
    start: int,
    coverage_only_roots: tuple[str, ...] = (),
    trace: dict | None = None,
) -> None:
    unavailable = [name for name, count in calls.items() if count["window"] is None]
    if unavailable:
        raise GateInvalid(
            "root_window_unavailable",
            f"roots {', '.join(unavailable)} have no call-args window for the candidate "
            f"window starting at {start}; pick a window that records those roots "
            "and check the selector "
            "routes them to Rust",
            details={"roots_not_called": unavailable, "requested_start": start},
        )
    not_called = [name for name, count in calls.items() if count["window"] <= 0]
    if not_called:
        raise GateInvalid(
            "roots_not_called",
            f"roots {', '.join(not_called)} have no entry call in the candidate window "
            f"starting at {start}",
            details={"roots_not_called": not_called, "requested_start": start},
        )
    # T3783: first establish that each requested root ran in this window. A Rust-host
    # shutdown report is meaningful only after that; otherwise a genuinely empty window
    # must keep its deterministic roots_not_called verdict. Check before interpreting Rust
    # counts so a missing host stop report still cannot masquerade as roots_not_rust.
    if trace is not None:
        check_stop_report(trace)
    coverage_only = {canonical_va(root) for root in coverage_only_roots}
    not_rust = [
        name
        for name, count in calls.items()
        if canonical_va(name) not in coverage_only and count["rust_total"] <= 0
    ]
    if not_rust:
        raise GateInvalid(
            "roots_not_rust",
            f"roots {', '.join(not_rust)} appeared in call-args but have no recorded Rust "
            "execution; the selector did not prove that the candidate root ran natively",
            details={"roots_not_rust": not_rust, "requested_start": start},
        )


REA_AI_ROOT = "0x266250"
CONTROL_ENV = "TSFP_GATE_CONTROL=1"
CALL_ARGS_MASKS = {"0x306460": {"exit_eax": "void return has no ABI value; preserved as undefined"}}
"""Inert env value: gives the off/off control its own replay-queue key (a second, real run)."""


def candidate_root_selectors(spec: GateSpec) -> tuple[str, ...]:
    """T3499: select the required roots on the candidate side.

    An A/B environment switch (TSFP_STORY_SEQUENCER, TSFP_REA_INVENTORY_NATIVE, ...) only acts
    inside a lane root that `--rust-ports` selects; an unselected root runs the lifted body
    whatever the switch says. Env-only profiles therefore compared lifted with lifted (T3463,
    T3464: required roots with zero Rust calls). The baseline keeps them unselected (lifted).
    """
    if spec.root_selectors:
        return spec.root_selectors
    if spec.switch_kind == "env":
        # T3702: the env switches enable the Rust path (TSFP_HUD_NATIVE=...); the roots are only
        # coverage roots and the host refuses them in `--rust-ports`.
        return ()
    present = {item.lower() for item in (*spec.switches, *spec.adopted)}
    if "all" in present:
        return ()
    names = tuple(name for root in spec.roots if (name := f"0x{root:x}") not in present)
    # The registry's "roots" form does not prove a root unselectable (0x266250 is roots-only
    # and adopted), so only roots a profile names in `coverage_only_roots` are dropped.
    coverage_only = {canonical_va(item) for item in spec.coverage_only_roots}
    unselectable = {name for name in names if canonical_va(name) in coverage_only}
    if unselectable:
        note = (
            f"root(s) {', '.join(sorted(unselectable))} dropped from --rust-ports: registered "
            "declared coverage-only by the profile (T3702)"
        )
        if note not in spec.notes:
            spec.notes.append(note)
    return tuple(name for name in names if name not in unselectable)


def candidate_expects_rust_ports(spec: GateSpec) -> bool:
    """T3783: detect selected ports even when their roots are already in switches."""
    if spec.root_selectors or spec.coverage_only_roots or candidate_root_selectors(spec):
        return True
    selected: set[str] = set()
    for selector in (*spec.switches, *spec.adopted):
        if selector.lower() == "all":
            return True
        try:
            selected.add(canonical_va(selector))
        except ValueError:
            continue
    return any(canonical_va(root) in selected for root in spec.roots)


def capture_group(spec: GateSpec, candidate: bool) -> tuple[str, ...]:
    """Switch set of one side: candidate = switches + adopted + shared env; baseline =
    adopted (or `off`) + shared env + baseline-only env."""
    if candidate:
        return (*spec.switches, *candidate_root_selectors(spec), *spec.adopted, *spec.shared_env)
    return (*(spec.adopted or ("off",)), *spec.shared_env, *spec.baseline_env)


HOST_FLAGS_WITH_VALUE = frozenset({"--vblank-owner-waits", "--vblank-worker-blanks"})


def split_capture_args(group: tuple[str, ...]) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Partition selectors from host options, preserving values for numeric flags."""
    selectors: list[str] = []
    host_args: list[str] = []
    index = 0
    while index < len(group):
        item = group[index]
        if not item.startswith("--"):
            selectors.append(item)
            index += 1
            continue
        host_args.append(item)
        index += 1
        if item in HOST_FLAGS_WITH_VALUE and index < len(group):
            host_args.append(group[index])
            index += 1
    return tuple(selectors), tuple(host_args)


def capture_host_args(group: tuple[str, ...]) -> tuple[str, ...]:
    """Extract host flags and select the side's REA AI host mode.

    The host enables the REA AI tick by default (T2385) and exits 2 after 0.5 s when the
    port set does not select its root, so a side without the root runs with it off.
    """
    lowered = {item.lower() for item in group}
    _, host_flags = split_capture_args(group)
    if "rust-rea-ai" in lowered:
        return host_flags
    if REA_AI_ROOT in lowered:
        # Root selection and the host runtime switch are separate. Forward the host
        # switch to both sides when the root is part of the shared adopted set.
        return (*host_flags, "--rust-rea-ai")
    return (*host_flags, "--no-rust-rea-ai")


def capture_switches(group: tuple[str, ...]) -> tuple[str, ...]:
    """Keep host CLI flags out of the replay selector list."""
    selectors, _ = split_capture_args(group)
    return selectors


# ---------------------------------------------------------------- receipts


def _root_cache_identity(spec: GateSpec) -> dict[str, list[str]]:
    # Some cache-key adapters and tests provide a lightweight spec object that
    # predates coverage-only roots. Treat that omitted optional field as empty.
    coverage_only_roots = getattr(spec, "coverage_only_roots", ())
    return {
        "roots": [f"{root:X}" for root in spec.roots],
        "coverage_only_roots": sorted({canonical_va(root) for root in coverage_only_roots}),
    }


def receipt_key(
    spec: GateSpec,
    input_hash: str,
    recording_sha: str,
    regions_sha: str,
    baseline_input_hash: str | None = None,
) -> str:
    fields = {
        "schema": SCHEMA,
        "compare_logic": compare_logic_digest(),
        "input_hash": input_hash,
        "switches": sorted(spec.switches),
        "env": sorted(spec.shared_env),
        "adopted": sorted(spec.adopted),
        "baseline_env": sorted(spec.baseline_env),
        "baseline_host_args": list(spec.baseline_host_args),
        "route": recording_sha,
        "start": spec.start,
        "regions": regions_sha,
        **_root_cache_identity(spec),
        "seconds": spec.seconds,
        "snapshot": spec.snapshot,
        "segments": spec.segments,
    }
    if spec.shared_host_args:
        fields["shared_host_args"] = list(spec.shared_host_args)
    if spec.wsola_activity:
        fields["wsola_activity"] = True
    if spec.render != "none":
        fields["render"] = spec.render
    if spec.paced:
        fields["paced"] = True
    if baseline_input_hash and baseline_input_hash != input_hash:
        fields["baseline_input_hash"] = baseline_input_hash
        fields["candidate_host_profile"] = spec.host_profile
    return cache_keys.make_key(
        "gate-receipt",
        files={},
        tools={"gate-schema": str(SCHEMA)},
        env={},
        extra=fields,
    ).key[:32]


def control_receipt_key(
    spec: GateSpec,
    input_hash: str,
    host_class: str,
    recording_sha: str,
    regions_sha: str,
) -> str:
    """Candidate-independent key for the off/off pair and its repeatability verdict."""
    baseline = capture_group(spec, False)
    fields = {
        "schema": SCHEMA,
        "compare_logic": compare_logic_digest(),
        "input_hash": input_hash,
        "host_class": host_class,
        "route": recording_sha,
        "start_poll": spec.start,
        "window_seconds": spec.seconds,
        "regions_sha256": regions_sha,
        "baseline_switch_set": sorted(baseline),
        "baseline_host_args": list(
            capture_host_args(baseline) + spec.shared_host_args + spec.baseline_host_args
        ),
        **_root_cache_identity(spec),
        "snapshot": spec.snapshot,
    }
    if spec.render != "none":
        fields["render"] = spec.render
    if spec.paced:
        fields["paced"] = True
    return cache_keys.make_key(
        "gate-receipt",
        files={},
        tools={"gate-schema": str(SCHEMA)},
        env={},
        extra=fields,
    ).key[:32]


def receipt_dir(owner_root: Path) -> Path:
    return gate_data.receipt_store(owner_root, "gate")


def known_last_poll(owner_root: Path, *, input_hash: str, recording_sha: str) -> int | None:
    """Return the furthest poll proven for this exact build input and recording."""
    furthest: int | None = None
    for path in receipt_dir(owner_root).glob("*.json"):
        if path.name.startswith("control-"):
            continue
        try:
            if path.stat().st_size > 1_000_000:
                continue
            receipt = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if (
            receipt.get("build_input_hash") != input_hash
            or receipt.get("recording_sha256") != recording_sha
        ):
            continue
        for side in ("baseline", "candidate"):
            last = receipt.get("samples", {}).get(side, {}).get("last_poll")
            if isinstance(last, int):
                furthest = last if furthest is None else max(furthest, last)
    return furthest


def known_unreachable_roots(
    owner_root: Path,
    *,
    route: Path,
    input_hash: str,
    recording_sha: str,
    save_sha: str | None,
    roots: tuple[int, ...],
    start: int,
) -> tuple[int, ...]:
    """Refuse roots proven absent by a complete, matching trace-store capture.

    This is deliberately proof-only: a missing index, incomplete call union, different
    host/recording, truncated trace, or absent root in the captured-call manifest yields
    no refusal. Trace-store call streams can be large, so scan them line by line.
    """
    if not roots:
        return ()
    labels = {route.stem}
    if route.stem.startswith("r5-story-gameplay-"):
        labels.add("r5")
    root_names = {f"0x{root:X}" for root in roots}
    roots_with_complete_evidence: set[int] = set()
    roots_seen: set[int] = set()
    for base in gate_data.roots(owner_root, gate_data.TRACES):
        for label in labels:
            route_dir = base / label
            if not route_dir.is_dir():
                continue
            try:
                host_dirs = sorted(p for p in route_dir.iterdir() if p.is_dir())
            except OSError:
                continue
            for host_dir in host_dirs[:128]:
                manifest_path = host_dir / "manifest.json"
                trace_path = host_dir / "trace.json"
                calls_path = host_dir / "call-args.trace"
                try:
                    if manifest_path.stat().st_size > 1_000_000 or not trace_path.is_file():
                        continue
                    manifest = json.loads(manifest_path.read_text())
                    traced = manifest.get("traced", {})
                    if (
                        manifest.get("recording_sha256") != recording_sha
                        or manifest.get("host_input_hash") != input_hash
                        or manifest.get("save_sha256") != save_sha
                        or traced.get("call_union_complete") is not True
                        or not root_names.issubset(set(traced.get("call_functions_captured", [])))
                        or not calls_path.is_file()
                    ):
                        continue
                    replay = manifest.get("replay", {})
                    if not isinstance(replay, dict) or replay.get("exit_code") != 0:
                        continue
                    # The final trace poll must demonstrate that the recorded route had
                    # enough room to contain this gate window. Scan in bounded chunks.
                    last_poll = None
                    with trace_path.open("rb") as trace:
                        tail = b""
                        while chunk := trace.read(1024 * 1024):
                            data = tail + chunk
                            matches = list(re.finditer(rb'"poll"\s*:\s*(\d+)', data))
                            if matches:
                                last_poll = int(matches[-1].group(1))
                            tail = data[-64:]
                    if last_poll is None or last_poll < start + MIN_WINDOW_SAMPLES - 1:
                        continue
                    roots_with_complete_evidence.update(roots)
                    with calls_path.open(errors="replace") as calls:
                        for line in calls:
                            fields = line.split(maxsplit=3)
                            if len(fields) < 3:
                                continue
                            try:
                                poll, address, direction = (
                                    int(fields[0]),
                                    int(fields[1], 16),
                                    int(fields[2]),
                                )
                            except ValueError:
                                continue
                            if poll >= start and direction == 0 and address in roots:
                                roots_seen.add(address)
                                if len(roots_seen) == len(roots):
                                    break
                except (OSError, json.JSONDecodeError, TypeError, ValueError):
                    continue
    return tuple(
        root for root in roots if root in roots_with_complete_evidence and root not in roots_seen
    )


def report_dir(owner_root: Path) -> Path:
    return gate_data.receipt_store(owner_root, "gate-reports")


CACHEABLE_INVALID = frozenset(
    (
        "same_capture",
        "nondeterministic_baseline",
        "roots_not_called",
        "root_window_unavailable",
        "roots_not_rust",
        "regions_inert",
        "region_width_mismatch",
        "baseline_too_few_samples",
        "candidate_too_few_samples",
        "baseline_window_not_reached",
        "candidate_window_not_reached",
    )
)


# ---------------------------------------------------------------- backend (real; faked in tests)


class Backend:
    """Host, capture and diff operations; tests replace this with fakes."""

    def __init__(self, owner_root: Path) -> None:
        self.owner_root = owner_root
        self.timings: dict[str, dict] = {}
        self.prebuilt_prepare_seconds = 0.0

    def sha256(self, path: Path) -> str:
        from tools.trace_store import sha256

        return sha256(path)

    def tree_hash(self, path: Path) -> str:
        from tools.play.rec_saves import tree_hash

        return tree_hash(path)

    def input_hash(self, commit: str, profile: str | None = None) -> str:
        from tools import prebuilt_select

        return prebuilt_select.build_input_hash(ROOT, commit, profile)

    def ensure_host(
        self, commit: str, input_hash: str, build: bool, profile: str | None = None
    ) -> Path:
        began = time.monotonic()
        try:
            return self._ensure_host(commit, input_hash, build, profile)
        finally:
            self.prebuilt_prepare_seconds = round(time.monotonic() - began, 3)

    def _ensure_host(
        self, commit: str, input_hash: str, build: bool, profile: str | None = None
    ) -> Path:
        from tools import prebuilt_select
        from tools.wt import prebuilt

        profile = profile or "gate"
        host = prebuilt_select.find_host(
            ROOT, commit=commit, host_input_hash=input_hash, profile=profile
        )
        if prebuilt_select.available(host):
            return host
        if not build:
            raise GateInvalid(
                "host_missing", f"no prebuilt host for {commit[:12]}; drop --no-build"
            )
        checkout = Path.cwd()
        head = resolve_commit(checkout, None, None)
        # Any checkout with the same build-input tree builds the same host (T3411 key).
        if head != commit and self.input_hash(head, profile) != input_hash:
            raise GateInvalid(
                "host_missing",
                f"no prebuilt host for {commit[:12]} and the cwd checkout ({head[:12]}) has "
                "other build inputs; run the gate from a worktree at the candidate commit",
            )
        try:
            print(
                f"gate: building prebuilt host for {commit[:12]} (single-flight)", file=sys.stderr
            )
            # T3448: prebuilt.build is single-flight per input hash, reuses an equal input tree,
            # builds on the laptop first and takes a local T3411 slot only for a local compile
            # (a saturated host sends the build to the remote worker after 60 s).
            with contextlib.redirect_stdout(sys.stderr):  # stdout carries only the verdict
                built = prebuilt.build(
                    checkout,
                    sha=head,
                    profile=profile,
                    remote="local" if profile != "gate" else "auto",
                )
        except prebuilt.PrebuiltError as error:
            raise GateInvalid("host_build_failed", str(error)) from None
        return built

    def lease(self, host: Path) -> contextlib.AbstractContextManager:
        from tools.wt import prebuilt

        return prebuilt.lease(host)

    def save_dir(self, recording: Path) -> Path | None:
        from tools.adoption_gate import route_save_dir

        try:
            return route_save_dir(recording)
        except (ValueError, OSError) as error:
            raise GateInvalid("saves_unbound", f"{recording.name}: {error}") from None

    def disc(self) -> Path:
        disc = owner_files.disc_image(ROOT)
        if not disc.is_file():
            raise GateInvalid("disc_missing", f"owner disc not found at {disc} (set TSFP_DISC)")
        return disc.resolve()

    def capture(self, *args: Any, **kwargs: Any) -> tuple[Path, dict]:
        """Collect host admission and capture timing for each comparison side."""
        began = time.monotonic()
        directory, receipt = self._capture(*args, **kwargs)
        side = (
            "control"
            if kwargs.get("control")
            else ("candidate" if kwargs.get("candidate") else "baseline")
        )
        breakdown = receipt.get("capture_breakdown") or {}
        elapsed = round(time.monotonic() - began, 3)
        self.timings[side] = {
            **breakdown,
            "elapsed_seconds": elapsed,
            "seconds": elapsed,  # legacy timing key retained for existing callers
            "snapshot_take_elapsed_seconds": receipt.get("snapshot_take_elapsed_seconds"),
            "snapshot_resume_elapsed_seconds": receipt.get("snapshot_resume_elapsed_seconds"),
            "queue_wait_seconds": receipt.get(
                "queue_wait_seconds", breakdown.get("queue_wait_seconds")
            ),
            "run_seconds": receipt.get("run_seconds", breakdown.get("run_seconds")),
            "host": receipt.get("host", breakdown.get("host")),
            "host_name": receipt.get("host_name", breakdown.get("host_name")),
            "paced": receipt.get("paced", breakdown.get("paced", False)),
            "snapshot": receipt.get("snapshot", breakdown.get("snapshot", False)),
            "cache_hit": bool(receipt.get("cache_hit")),
        }
        receipt["capture_breakdown"] = dict(self.timings[side])
        return directory, receipt

    def _capture(
        self,
        spec: GateSpec,
        *,
        candidate: bool,
        commit: str,
        input_hash: str,
        save_dir: Path | None,
        disc: Path,
        host: Path,
        control: bool = False,
    ) -> tuple[Path, dict]:
        """Queue one side; from a cached snapshot when the window starts after boot."""
        from tools import adoption_gate as ag

        group = capture_group(spec, candidate)
        if control:
            group = (*group, CONTROL_ENV)
        # Capture the same selected roots on both sides so `rust_trace diff`
        # compares lifted entry/exit records against the Rust candidate instead
        # of treating the entire candidate stream as missing from the baseline.
        call_args = ",".join(f"{root:x}" for root in spec.roots) if spec.roots else None
        host_args = (*capture_host_args(group), *spec.shared_host_args)
        if not candidate:
            host_args = (*host_args, *spec.baseline_host_args)
        switches = capture_switches(group)
        submit = dict(
            host_args=host_args,
            regions=spec.regions,
            seconds=spec.seconds,
            priority="high",
            no_cache=spec.wsola_activity,
            no_stop_at_route_end=spec.wsola_activity,
            allow_capture_failure=True,
            save_dir=save_dir,
            call_args=call_args,
            max_idle=ag.WINDOW_MAX_IDLE if spec.start else None,
            host_input_hash=input_hash,
            disc=disc,
            render=spec.render if spec.render != "none" else None,
        )
        if spec.render != "none":
            submit["paced"] = spec.paced
        if spec.segments is not None:
            return self.capture_segments(
                spec,
                group=group,
                commit=commit,
                input_hash=input_hash,
                save_dir=save_dir,
                host=host,
                submit=submit,
            )
        route = str(spec.route)
        if not (spec.start and spec.snapshot):
            # Full replay from boot, compared from the start poll: deterministic (two runs of
            # one configuration are EQUAL), and the replay queue caches identical captures.
            receipt = ag.queue_submit(
                self.owner_root,
                route,
                switches,
                commit,
                headless=spec.render == "none",
                **submit,
            )
            return Path(receipt["run_dir"]), receipt
        # Same key fields as adoption_gate.gate_snapshot, so baseline snapshots are shared.
        fields = {
            "switches": list(switches),
            "rust_ports": None,
            "poll": spec.start,
            "seconds": spec.seconds,
            "regions": self.sha256(spec.regions),
            "recording": self.sha256(spec.route),
            "save": self.tree_hash(save_dir) if save_dir is not None else None,
            "host_args": list(host_args),
            "candidate_environment": [],
            "stop_at_route_end": False,
            "paced": False,
        }
        if call_args:
            fields["call_args"] = call_args
        snapshot_root = (
            gate_data.path(self.owner_root, gate_data.SNAPSHOTS)  # T3467
            / route_label(spec.route)
            / self.sha256(host)
        )
        cache, snapshot_poll = ag.earlier_snapshot_cache(snapshot_root, fields, spec.start)

        taken: dict = {}

        def take(directory: Path) -> dict:
            taken.update(
                ag.queue_submit(
                    self.owner_root,
                    route,
                    switches,
                    commit,
                    snapshot=(snapshot_poll, directory),
                    **submit,
                )
            )
            return dict(taken)

        capture_seconds = spec.seconds
        if snapshot_poll < spec.start:
            manifest = json.loads((cache / "manifest.json").read_text())
            take_total = manifest.get("seconds", {}).get("take_total")
            if take_total:
                capture_seconds += round(take_total)
        try:
            trace_dir, receipt = ag.snapshot_capture(
                self.owner_root, cache, commit, take, capture_seconds, input_hash
            )
        except RuntimeError:
            # A failed DMTCP resume raises before run_gate can inspect the replay
            # receipt. Preserve the failed capture when it did write a trace so
            # the gate can classify an empty capture instead of obscuring it as
            # a generic snapshot failure.
            failed_dir = failed_snapshot_trace_dir(cache)
            if failed_dir is None:
                raise
            return failed_dir, {
                "exit_code": 1,
                "snapshot": "resume_failed",
                "run_dir": str(failed_dir),
            }
        if receipt.get("exit_code") and not (trace_dir / "trace.json").is_file():
            # T3607: the take replay ended (route end) before the snapshot poll, so no checkpoint
            # and no continued run exist, but the take itself wrote a complete trace. Hand that
            # trace to the window guards: they name the poll the replay reached (UNREACHABLE)
            # instead of the generic "capture exited 1".
            ended = Path(str(taken.get("run_dir") or ""))
            if not taken.get("exit_code") and (ended / "trace.json").is_file():
                return ended, {**taken, "snapshot": "not_reached", "exit_code": 0}
        receipt.update(
            requested_poll=spec.start,
            snapshot_poll=snapshot_poll,
            reused_earlier_by_polls=spec.start - snapshot_poll,
        )
        return trace_dir, receipt

    def capture_segments(
        self,
        spec: GateSpec,
        *,
        group: tuple[str, ...],
        commit: str,
        input_hash: str,
        save_dir: Path | None,
        host: Path,
        submit: dict,
    ) -> tuple[Path, dict]:
        """Capture one route pass of DMTCP boundaries, then resume its windows."""
        from tools import adoption_gate as ag
        from tools.rust_trace import native_input_route_end_sim_tick
        from tools.snapshot import manifest as snapshot_manifest

        if spec.roots:
            raise GateInvalid(
                "segments_roots_unsupported", "segmented gates do not support --roots"
            )
        route_end = native_input_route_end_sim_tick(spec.route)
        if route_end is None:
            raise GateInvalid("segments_route_end_missing", "route has no deterministic tick end")
        windows = ag.poll_segments(spec.start, route_end, spec.segments or 1)
        starts = segment_checkpoint_polls(spec.start, route_end, spec.segments or 1)
        host_sha = self.sha256(host)
        fields = {
            "schema": "T3423-multisnapshot-v7",
            "switches": list(group),
            "polls": starts,
            "checkpoint_count": len(starts),
            "regions": self.sha256(spec.regions),
            "recording": self.sha256(spec.route),
            "save": self.tree_hash(save_dir) if save_dir else None,
            "host_args": list(submit["host_args"]),
            "host_input_hash": input_hash,
        }
        cache = (
            gate_data.path(self.owner_root, gate_data.SNAPSHOTS)
            / route_label(spec.route)
            / host_sha
            / ag.snapshot_key(fields)
        )

        def valid() -> bool:
            for poll in starts:
                directory = cache / f"poll-{poll}"
                try:
                    data = snapshot_manifest.load(directory)
                    if data.get("run_dir_isolation") != "pathvirt-per-poll-v1":
                        return False
                    if snapshot_manifest.verify(
                        data, directory, host_override=host, fast=True, check_display=False
                    ):
                        return False
                except (OSError, SystemExit, ValueError, KeyError, TypeError):
                    return False
            return True

        if not valid():
            if cache.exists():
                ag.retire_snapshot(cache)
            capture = ag.queue_submit(
                self.owner_root,
                str(spec.route),
                group,
                commit,
                submit["regions"],
                spec.seconds,
                False,
                allow_capture_failure=True,
                paced=False,
                save_dir=save_dir,
                host_args=submit["host_args"],
                stop_at_route_end=True,
                snapshot_polls=(starts, cache),
                headless=True,
                max_idle=ag.WINDOW_MAX_IDLE,
                max_dumps=12000,
                host_input_hash=input_hash,
                disc=submit["disc"],
            )
            if capture.get("exit_code") or not valid():
                raise RuntimeError(f"multi-snapshot route capture failed: {capture}")

        slots = threading.BoundedSemaphore(2)

        def resume(index_window: tuple[int, tuple[int, int]]) -> tuple[Path, dict]:
            _index, (first, last) = index_window
            checkpoint_poll = max(poll for poll in starts if poll <= first)
            directory = cache / f"poll-{checkpoint_poll}"
            (directory / "run-dir" / "snapshot-stop-poll").write_text(f"{last}\n")

            def no_retake(_directory: Path) -> dict:
                raise RuntimeError("segmented snapshot became invalid during resume")

            # Include admission and hostlock waits: the segment receipt should explain total
            # gate wall time, not only the guest's capture duration.
            started = time.monotonic()
            with slots:
                trace_dir, receipt = ag.snapshot_capture(
                    self.owner_root,
                    directory,
                    commit,
                    no_retake,
                    host_input_hash=input_hash,
                    isolate_run_dir=True,
                    fail_on_resume_error=True,
                )
            receipt["segment_window"] = [first, last]
            receipt["checkpoint_poll"] = checkpoint_poll
            receipt["resume_elapsed_seconds"] = round(time.monotonic() - started, 3)
            return trace_dir, receipt

        with ThreadPoolExecutor(max_workers=min(2, len(windows))) as pool:
            captured = list(pool.map(resume, enumerate(windows)))
        if any(receipt.get("exit_code") for _path, receipt in captured):
            raise RuntimeError("one or more segmented resumes failed")
        stitched = ag.stitch_segment_traces(
            [path for path, _receipt in captured],
            windows,
            gate_data.path(self.owner_root, gate_data.SEGMENTS)
            / route_label(spec.route)
            / host_sha
            / ag.snapshot_key({"switches": list(group), "segments": windows}),
        )
        return stitched, {
            "exit_code": 0,
            "snapshot": "segmented",
            "segments": len(windows),
            "segment_receipts": [receipt for _path, receipt in captured],
        }

    def diff(
        self, spec: GateSpec, host: Path, baseline: Path, candidate: Path, report_base: Path
    ) -> dict:
        from tools import adoption_gate as ag

        try:
            ag.compare(
                self.owner_root,
                str(spec.route),
                spec.regions,
                host,
                baseline,
                candidate,
                report_base,
                from_poll=spec.start,
                segments=spec.segments,
            )
        except RuntimeError as error:
            raise GateInvalid("comparison_failed", f"trace comparison failed: {error}") from error
        return json.loads(report_base.with_suffix(".json").read_text())


# ---------------------------------------------------------------- the gate


def control_report_repeatable(report: dict) -> bool:
    """T3496: off/off repeatability from a control diff report.

    The report's `equal` also needs a non-empty call-argument stream, because an empty stream is
    no candidate evidence. For the control, two streams with no row at all are identical: the
    switch-off sides of roots that run lifted record nothing (T2409's frame-loop roots).
    `tools.rust_trace` reports that as `comparison.repeatable`; any row, divergence or other
    failed check keeps it false.
    """
    comparison = report.get("comparison") or {}
    # `equal` covers sampled state and captured call streams, which can stop before
    # route end. Require the independent exit counters before treating the control
    # as repeatable over the whole replay.
    if comparison.get("counter_repeatable") is not True:
        return False
    if comparison.get("repeatable") is True:
        return True
    return False


def control_comparison_failure(comparison: dict) -> str:
    first = comparison.get("first_divergence") or {}
    if not first and comparison.get("counter_repeatable") is False:
        return (
            "off/off whole-run exit counters differ or are missing "
            f"({comparison.get('counter_mismatch_count', 'unknown')} differing functions); "
            "candidate evidence is invalid"
        )
    return (
        f"off/off control diverges from the baseline at poll {first.get('poll')} "
        f"region {first.get('region')} ({comparison.get('diverging_samples')} samples); "
        "candidate evidence is invalid"
    )


def distinct_control_captures(baseline: object, control: object) -> bool:
    """A capture cannot establish its own repeatability, even via a symlink."""
    if not isinstance(baseline, (str, Path)) or not isinstance(control, (str, Path)):
        return False
    if not str(baseline) or not str(control):
        return False
    return Path(baseline).resolve() != Path(control).resolve()


def require_control_window(baseline: dict, control: dict, candidate: dict) -> None:
    """Do not judge a candidate beyond the polls checked by its off/off control."""
    control_horizon = common_last_poll(baseline, control)
    candidate_horizon = common_last_poll(baseline, candidate)
    if control_horizon is None or candidate_horizon is None or candidate_horizon > control_horizon:
        raise GateInvalid(
            "control_window_shorter",
            f"off/off controls agree only through poll {control_horizon}; "
            f"the candidate comparison reaches poll {candidate_horizon}",
        )


def record_call_tail(verdict: dict, comparison: dict) -> None:
    """Copy the excluded call-args tail (after the common last poll) into the receipt."""
    streamed = comparison.get("call_args_external") or comparison.get("call_args") or {}
    if streamed.get("call_args_common_last_poll") is None:
        return
    verdict["call_args_common_last_poll"] = streamed["call_args_common_last_poll"]
    verdict["call_args_tail_excluded"] = streamed.get("call_args_tail_excluded", {})
    if streamed.get("call_args_tail_text"):
        verdict["call_args_tail_text"] = streamed["call_args_tail_text"]
    if streamed.get("call_args_tail_health"):
        verdict["health"] = list(verdict.get("health", [])) + streamed["call_args_tail_health"]


def record_stack_residue(verdict: dict, comparison: dict) -> None:
    """T3702: copy dead-stack exclusions and partial-window counts into the receipt."""
    streamed = comparison.get("call_args_external") or comparison.get("call_args") or {}
    rows = int(streamed.get("stack_residue_rows") or 0)
    if not rows:
        return
    verdict["stack_residue_rows"] = rows
    verdict["stack_residue_partial_rows"] = int(streamed.get("stack_residue_partial_rows") or 0)
    verdict["stack_residue_excluded"] = streamed.get("stack_residue_excluded", {})
    if streamed.get("stack_residue_health"):
        verdict["health"] = list(verdict.get("health", [])) + streamed["stack_residue_health"]


def failed_snapshot_trace_dir(cache: Path) -> Path | None:
    """Return a resume's existing trace directory when its snapshot manifest names one."""
    try:
        manifest = json.loads((cache / "manifest.json").read_text())
        failed_dir = Path(manifest["run_dir"])
    except (OSError, KeyError, json.JSONDecodeError):
        return None
    return failed_dir if (failed_dir / "trace.json").is_file() else None


def _failure_tail(trace_dir: Path) -> str:
    """Bounded useful tail of a failed capture log (queue run.log or snapshot take log)."""
    for log in (trace_dir / "run.log", trace_dir.parent / "capture-take.log"):
        try:
            # Capture logs can be large. Read only the last 16 KiB so failure reporting
            # preserves parser/startup context without loading raw trace-adjacent logs.
            with log.open("rb") as stream:
                stream.seek(0, os.SEEK_END)
                size = stream.tell()
                stream.seek(max(0, size - 16 * 1024))
                text = stream.read(16 * 1024).decode("utf-8", errors="replace")
        except OSError:
            continue
        lines = [line.strip() for line in text.splitlines() if line.strip()][-8:]
        if lines:
            excerpt = "\n".join(line[-500:] for line in lines)
            return f"{log}:\n{excerpt}"
    return "no capture log"


def check_control(
    spec: GateSpec,
    backend: Backend,
    host: Path,
    base_dir: Path,
    report_base: Path,
    verdict: dict,
    capture_kwargs: dict,
) -> None:
    """A DIVERGED claim needs a reproducible baseline: replay the switch-off side again
    (its own queue key) and require it EQUAL to the baseline, else the route does not
    replay deterministically on this host and the divergence means nothing."""
    try:
        control_dir, receipt = backend.capture(
            spec, candidate=False, control=True, **capture_kwargs
        )
    except RuntimeError as error:
        raise GateInvalid("control_capture_failed", str(error)[-800:]) from None
    verdict["control_run"] = str(control_dir)
    control_base = report_base.with_name(report_base.name + "-control")
    report = backend.diff(spec, host, base_dir, control_dir, control_base)
    comparison = report.get("comparison", {})
    verdict["control_equal"] = control_report_repeatable(report)
    if not verdict["control_equal"]:
        first = comparison.get("first_divergence") or {}
        raise GateInvalid(
            "nondeterministic_baseline",
            f"off/off control diverges from the baseline at poll {first.get('poll')} region "
            f"{first.get('region')} ({comparison.get('diverging_samples')} samples): this route "
            "does not replay reproducibly on this host, so the candidate divergence is not "
            f"evidence; report {control_base.with_suffix('.json')}",
        )


def _load_trace(trace_dir: Path) -> dict:
    path = trace_dir / "trace.json"
    if not path.is_file():
        raise GateInvalid(
            "CAPTURE_EMPTY",
            f"capture exited without a trace file ({path}); check the trace writer",
            details={"reason": "writer_error", "samples_reached": 0},
        )
    return json.loads(path.read_text())


def _raise_capture_failure(
    label: str,
    trace_dir: Path,
    receipt: dict,
    start: int,
    widths: dict[str, int],
    route_end_poll: int | None,
) -> None:
    """Keep an empty clean host exit classified instead of hiding it as exit 1.

    rust_trace uses a nonzero process status when its host exits cleanly without
    emitting any samples. The queue receipt therefore has exit_code=1 even
    though trace.meta.host_exit is 0. Preserve CAPTURE_EMPTY as the gate verdict.
    """
    raise_if_host_refused(label, trace_dir)
    trace_path = trace_dir / "trace.json"
    if trace_path.is_file():
        try:
            trace = _load_trace(trace_dir)
        except (OSError, json.JSONDecodeError):
            trace = None
        if trace is not None and not trace.get("samples"):
            check_side(
                label,
                trace,
                start,
                widths,
                route_end_poll,
                _capture_stop_reason(trace_dir),
            )
    raise GateInvalid(
        f"{label}_capture_failed",
        f"capture exited {receipt.get('exit_code')}: {_failure_tail(trace_dir)}",
    )


def raise_if_host_refused(label: str, trace_dir: Path) -> None:
    """T3702: a host that refuses its `--rust-ports` or options exits 2 within seconds; say so
    at once instead of reporting an empty capture after the rest of the gate."""
    reason = _capture_stop_reason(trace_dir)
    if reason and " was refused. Forms:" in reason:
        raise GateInvalid(
            "host_refused_switches",
            f"{label} host refused its switches: {reason}",
            details={"side": label, "reason": "host_refused_switches", "refusal": reason},
        )


def _capture_stop_reason(trace_dir: Path) -> str | None:
    """Return an explicit guest stop from the bounded capture log, if present."""
    log_path = trace_dir / "run.log"
    scope = None
    route_end_reached = False
    try:
        with log_path.open("r", errors="replace") as log:
            for line in log:
                if "tsfp-bevy: input replay route end " in line:
                    route_end_reached = True
                # T3607: the host's own words say which word refused (the scope line precedes the
                # generic stop line), and an option the host refuses ends the run with exit 2.
                if "T3409 startup worker scope rejected" in line:
                    scope = line.strip()[-240:]
                marker = "STOPPED: XDK function with no implementation -- "
                if marker in line:
                    return f"{scope} / {line.strip()[-120:]}" if scope else line.strip()[-240:]
                if "STOPPED:" in line or "stopped: Rust port bridge contract broken" in line:
                    if route_end_reached and (
                        "STOPPED: host shutdown requested" in line
                        or "stopped: guest entry point returned" in line
                        or "guest thread exited via PsTerminateSystemThread" in line
                    ):
                        continue
                    return line.strip()[:500]
                if "T3409 startup worker:" in line:
                    scope = line.strip()[-240:]
                if " was refused. Forms:" in line:
                    return line.strip()[:240]
    except OSError:
        return None
    return scope


def _check_captured_side(
    label: str,
    trace_dir: Path,
    trace: dict,
    start: int,
    widths: dict[str, int],
    route_end_poll: int | None,
) -> dict:
    """Classify an empty capture using the host's bounded stop reason, when present."""
    return check_side(
        label,
        trace,
        start,
        widths,
        route_end_poll,
        _capture_stop_reason(trace_dir),
    )


def require_wsola_activity(spec: GateSpec, label: str, directory: Path, trace: dict) -> None:
    if not spec.wsola_activity:
        return
    from tools.gate_host_profile import validate_wsola_run

    try:
        log = (directory / "run.log").read_text(errors="replace")
    except OSError:
        raise GateInvalid("wsola_activity", f"{label}: run log unavailable") from None
    expected = "rust" if label == "candidate" else "c"
    error = validate_wsola_run(trace, log, expected, 180.0)
    if error:
        raise GateInvalid("wsola_activity", f"{label}: {error}")


def run_gate(
    spec: GateSpec,
    backend: Backend,
    *,
    use_cache: bool = True,
    build: bool = True,
    _lock_held: bool = False,
) -> dict:
    started = time.monotonic()
    from tools import gate_host_profile

    input_hash = gate_host_profile.input_hash(backend, spec.commit, spec.host_profile)
    baseline_input_hash = gate_host_profile.input_hash(
        backend, spec.commit, spec.baseline_host_profile
    )
    recording_sha = backend.sha256(spec.route)
    regions_sha = backend.sha256(spec.regions)
    preflight_base = {
        "schema": SCHEMA,
        "commit": spec.commit,
        "build_input_hash": input_hash,
        "profile": spec.profile,
        "route": route_label(spec.route),
        "recording_sha256": recording_sha,
        "start_poll": spec.start,
        "requested_start": spec.start,
        "seconds": spec.seconds,
        "preflight": True,
        "candidate_run": None,
    }
    try:
        preflight_profile_route(spec.profile, route_label(spec.route))
    except GateInvalid as error:
        return {
            **preflight_base,
            "verdict": "INVALID",
            "reasons": [error.code],
            "message": error.message,
            **error.details,
        }
    recent_invalid = gate_history.check_recent_same_input_invalid(
        backend.owner_root,
        profile=spec.profile,
        route=route_label(spec.route),
        build_input_hash=input_hash,
    )
    if recent_invalid and not spec.retry_known_invalid:
        return gate_history.refusal_verdict(recent_invalid, base=preflight_base)
    coverage_rows = gate_route_roots.load(backend.owner_root)
    absent_roots = gate_route_roots.known_absent(
        coverage_rows,
        route=route_label(spec.route),
        recording_sha256=recording_sha,
        start=spec.start,
        seconds=spec.seconds,
        roots=spec.roots,
    )
    if absent_roots and not spec.retry_known_invalid:
        try:
            suggested = _auto_route(
                backend.owner_root,
                roots=spec.roots,
                allowed=spec.allowed_routes,
                start=str(spec.start),
                default_start=None,
                exclude=route_label(spec.route),
            ).stem
        except GateInvalid:
            suggested = None
        rendered = ", ".join(f"0x{root:X}" for root in absent_roots)
        suggestion = f"; best measured route is --route {suggested}" if suggested else ""
        return {
            "schema": SCHEMA,
            "verdict": "UNREACHABLE",
            "reasons": ["known_roots_not_in_window"],
            "message": (
                f"route {route_label(spec.route)} is known not to call {rendered} from poll "
                f"{spec.start} in a capture at least {spec.seconds}s long{suggestion}"
            ),
            "commit": spec.commit,
            "build_input_hash": input_hash,
            "profile": spec.profile,
            "route": route_label(spec.route),
            "recording_sha256": recording_sha,
            "start_poll": spec.start,
            "requested_start": spec.start,
            "seconds": spec.seconds,
            "roots": [f"0x{root:X}" for root in spec.roots],
            "unreachable_roots": [f"0x{root:X}" for root in absent_roots],
            "suggested_route": suggested,
            "preflight": True,
            "candidate_run": None,
        }
    if spec.route_end_poll is not None and spec.start > spec.route_end_poll:
        return {
            "schema": SCHEMA,
            "verdict": "UNREACHABLE",
            "reasons": ["start_after_recorded_route_end"],
            "message": (
                f"start poll {spec.start} is after the recording end at poll "
                f"{spec.route_end_poll}; "
                "choose a start within the recorded route"
            ),
            "commit": spec.commit,
            "build_input_hash": input_hash,
            "route": route_label(spec.route),
            "recording_sha256": recording_sha,
            "start_poll": spec.start,
            "requested_start": spec.start,
            "last_poll_reached": spec.route_end_poll,
            "preflight": True,
            "candidate_run": None,
        }
    if spec.route_end_poll is not None:
        # A trace can contribute at most one sample per recorded controller poll.
        # Reject windows that cannot possibly meet the gate's hard sample floor;
        # this is a necessary lower bound, not a prediction of capture density.
        available_polls = spec.route_end_poll - spec.start + 1
        if available_polls < MIN_WINDOW_SAMPLES:
            return {
                "schema": SCHEMA,
                "verdict": "UNREACHABLE",
                "reasons": ["recording_too_short_for_window"],
                "message": (
                    f"recording has only {available_polls} polls from start {spec.start} "
                    f"through route end {spec.route_end_poll}; the gate requires at least "
                    f"{MIN_WINDOW_SAMPLES} samples, so choose an earlier start or a longer route"
                ),
                "commit": spec.commit,
                "build_input_hash": input_hash,
                "route": route_label(spec.route),
                "recording_sha256": recording_sha,
                "start_poll": spec.start,
                "requested_start": spec.start,
                "last_poll_reached": spec.route_end_poll,
                "polls_available": available_polls,
                "samples_required": MIN_WINDOW_SAMPLES,
                "preflight": True,
                "candidate_run": None,
            }
    last_known_poll = known_last_poll(
        backend.owner_root, input_hash=input_hash, recording_sha=recording_sha
    )
    if last_known_poll is not None and spec.start > last_known_poll:
        return {
            "schema": SCHEMA,
            "verdict": "UNREACHABLE",
            "reasons": ["start_not_reached_from_profile_data"],
            "message": (
                f"start poll {spec.start} exceeds the furthest poll {last_known_poll} "
                f"already reached for {route_label(spec.route)} with this prebuilt input and recording; "
                "check the route/save binding or choose a start supported by the route evidence"
            ),
            "commit": spec.commit,
            "build_input_hash": input_hash,
            "route": route_label(spec.route),
            "recording_sha256": recording_sha,
            "start_poll": spec.start,
            "requested_start": spec.start,
            "last_poll_reached": last_known_poll,
            "preflight": True,
            "candidate_run": None,
        }
    save_dir = backend.save_dir(spec.route)
    save_sha = backend.tree_hash(save_dir) if save_dir is not None else None
    unreachable_roots = known_unreachable_roots(
        backend.owner_root,
        route=spec.route,
        input_hash=input_hash,
        recording_sha=recording_sha,
        save_sha=save_sha,
        roots=spec.roots,
        start=spec.start,
    )
    if unreachable_roots:
        rendered = ", ".join(f"0x{root:X}" for root in unreachable_roots)
        return {
            "schema": SCHEMA,
            "verdict": "UNREACHABLE",
            "reasons": ["roots_not_reached_from_trace_index"],
            "message": (
                f"root(s) {rendered} were not called from poll {spec.start} onward in a "
                "complete trace-index capture for this exact host input and recording; "
                "choose a route/window that reaches them"
            ),
            "commit": spec.commit,
            "build_input_hash": input_hash,
            "route": route_label(spec.route),
            "recording_sha256": recording_sha,
            "start_poll": spec.start,
            "requested_start": spec.start,
            "roots": [f"0x{root:X}" for root in spec.roots],
            "unreachable_roots": [f"0x{root:X}" for root in unreachable_roots],
            "preflight": True,
            "candidate_run": None,
        }
    key = receipt_key(spec, input_hash, recording_sha, regions_sha, baseline_input_hash)
    receipt_path = receipt_dir(backend.owner_root) / f"{key}.json"
    identity_extra = {
        "regions": regions_sha,
        **_root_cache_identity(spec),
        "baseline_host_args": list(spec.baseline_host_args),
        "snapshot": spec.snapshot,
        "segments": spec.segments,
    }
    if spec.render != "none" or spec.paced:
        identity_extra.update(render=spec.render, paced=spec.paced)
    if baseline_input_hash != input_hash:
        identity_extra.update(
            baseline_build_input_hash=baseline_input_hash,
            candidate_host_profile=spec.host_profile,
        )
    store_key, store_request = verification_store.request_key(
        build_input_hash=input_hash,
        profile=spec.profile or "gate-custom",
        route=recording_sha,
        start=spec.start,
        seconds=spec.seconds,
        switches=(*spec.switches, *spec.adopted, *spec.shared_env, *spec.baseline_env),
        extra=identity_extra,
    )
    if use_cache and not _lock_held:
        with verification_store.request_lock(backend.owner_root, store_key):
            return run_gate(spec, backend, use_cache=True, build=build, _lock_held=True)
    if use_cache and receipt_path.is_file():
        cached = json.loads(receipt_path.read_text())
        control_receipt = cached.get("control_receipt")
        if control_receipt:
            control_path = Path(control_receipt)
            try:
                control_data = json.loads(control_path.read_text())
                control_cache_valid = control_data.get(
                    "control_equal"
                ) is True and distinct_control_captures(
                    control_data.get("baseline_run"), control_data.get("control_run")
                )
            except (OSError, json.JSONDecodeError):
                control_cache_valid = False
            if control_cache_valid:
                from tools.cache_telemetry import record_lookup

                record_lookup("gate-receipt", True)
                cached["cache_hit"] = True
                verification_store.put(
                    backend.owner_root,
                    store_key,
                    store_request,
                    {name: value for name, value in cached.items() if name != "cache_hit"},
                )
                gate_history.record(backend.owner_root, cached)
                return cached
    if use_cache:
        cached = verification_store.get(backend.owner_root, store_key, store_request)
        if cached is not None:
            control_receipt = cached.get("control_receipt")
            try:
                control_data = (
                    json.loads(Path(control_receipt).read_text()) if control_receipt else {}
                )
                control_valid = control_data.get(
                    "control_equal"
                ) is True and distinct_control_captures(
                    control_data.get("baseline_run"), control_data.get("control_run")
                )
            except (OSError, json.JSONDecodeError):
                control_valid = False
            if control_valid:
                from tools.cache_telemetry import record_lookup

                record_lookup("gate-receipt", True)
                gate_history.record(backend.owner_root, cached)
                return cached
    if use_cache:
        from tools.cache_telemetry import record_lookup

        record_lookup("gate-receipt", False)
    refusal = gate_history.check(backend.owner_root, spec, recording_sha, regions_sha)
    if refusal and spec.retry_known_invalid:
        spec.notes.append(
            f"{refusal.code} bypassed by {gate_history.RETRY_FLAG}: {refusal.message}"
        )
    elif refusal:
        return gate_history.refusal_verdict(
            refusal,
            base={
                "schema": SCHEMA,
                "commit": spec.commit,
                "build_input_hash": input_hash,
                "profile": spec.profile,
                "route": route_label(spec.route),
                "recording_sha256": recording_sha,
                "start_poll": spec.start,
                "requested_start": spec.start,
                "seconds": spec.seconds,
            },
        )
    verdict: dict = {
        "schema": SCHEMA,
        "verdict": "INVALID",
        "reasons": [],
        "commit": spec.commit,
        "build_input_hash": input_hash,
        "profile": spec.profile,
        "switches": list(spec.switches),
        "env": list(spec.shared_env),
        "adopted": list(spec.adopted),
        "baseline_env": list(spec.baseline_env),
        "baseline_host_args": list(spec.baseline_host_args),
        "route": route_label(spec.route),
        "recording_sha256": recording_sha,
        "start_poll": spec.start,
        "regions": str(spec.regions),
        "regions_sha256": regions_sha,
        "roots": [f"0x{root:X}" for root in spec.roots],
        "seconds": spec.seconds,
        "segments": spec.segments,
        "render": spec.render,
        "paced": spec.paced,
        "notes": list(spec.notes),
        "receipt": str(receipt_path),
        "cache_hit": False,
        "control_cache_hit": False,
        "compare_logic": compare_logic_digest(),
    }
    cacheable = False
    host_leases = contextlib.ExitStack()
    try:
        host = (
            backend.ensure_host(spec.commit, input_hash, build)
            if spec.host_profile == "gate"
            else backend.ensure_host(spec.commit, input_hash, build, spec.host_profile)
        )
        # Keep the candidate alive while building the baseline. The baseline's
        # publication can trigger prebuilt retention, so waiting until capture
        # setup to lease both hosts leaves the first host exposed to GC.
        host_leases.enter_context(backend.lease(host))
        baseline_host = (
            (
                backend.ensure_host(spec.commit, baseline_input_hash, build)
                if spec.baseline_host_profile == "gate"
                else backend.ensure_host(
                    spec.commit, baseline_input_hash, build, spec.baseline_host_profile
                )
            )
            if baseline_input_hash != input_hash
            else host
        )
        if baseline_host != host:
            host_leases.enter_context(backend.lease(baseline_host))
        verdict["host"] = str(host)
        verdict["prebuilt_prepare_seconds"] = getattr(backend, "prebuilt_prepare_seconds", 0.0)
        verdict["host_sha256"] = backend.sha256(host)
        verdict["baseline_host"] = str(baseline_host)
        verdict["baseline_host_sha256"] = backend.sha256(baseline_host)
        if spec.same_host and (
            baseline_input_hash != input_hash
            or verdict["baseline_host_sha256"] != verdict["host_sha256"]
        ):
            raise GateInvalid(
                "host_mismatch", "same-host gate requires identical host hash and build input hash"
            )
        verdict["baseline_build_input_hash"] = baseline_input_hash
        verdict["candidate_host_profile"] = spec.host_profile
        disc = backend.disc()
        widths = region_widths(json.loads(spec.regions.read_text()))
        with host_leases:
            control_key = control_receipt_key(
                spec,
                baseline_input_hash,
                verdict["baseline_host_sha256"],
                recording_sha,
                regions_sha,
            )
            control_path = receipt_dir(backend.owner_root) / f"control-{control_key}.json"
            control_record = None
            if use_cache and control_path.is_file():
                cached_control = json.loads(control_path.read_text())
                if not distinct_control_captures(
                    cached_control.get("baseline_run"), cached_control.get("control_run")
                ):
                    raise GateInvalid(
                        "same_control_capture", "off/off controls are not distinct captures"
                    )
                if cached_control.get("control_equal") is not True:
                    first = cached_control.get("first_divergence") or {}
                    verdict.update(
                        control_equal=False,
                        control_receipt=str(control_path),
                        control_run=cached_control.get("control_run"),
                        baseline_run=cached_control.get("baseline_run"),
                        first_divergence=first,
                    )
                    raise GateInvalid(
                        "nondeterministic_baseline",
                        cached_control.get("message")
                        or "cached off/off control diverged; candidate was not captured",
                        details={"first_divergence": first},
                    )
                if all(
                    Path(str(cached_control.get(key, ""))).joinpath("trace.json").is_file()
                    for key in ("baseline_run", "control_run")
                ):
                    control_record = cached_control
                    verdict["control_cache_hit"] = True

            if control_record is None:
                try:
                    base_dir, base_receipt = backend.capture(
                        spec,
                        candidate=False,
                        commit=spec.commit,
                        input_hash=baseline_input_hash,
                        save_dir=save_dir,
                        disc=disc,
                        host=baseline_host,
                    )
                except RuntimeError as error:
                    raise GateInvalid("baseline_capture_failed", str(error)[-800:]) from None
                verdict["baseline_run"] = str(base_dir)
                verdict["baseline_snapshot"] = base_receipt.get("snapshot")
                raise_if_host_refused("baseline", base_dir)
                if base_receipt.get("exit_code"):
                    _raise_capture_failure(
                        "baseline",
                        base_dir,
                        base_receipt,
                        spec.start,
                        widths,
                        spec.route_end_poll,
                    )
                base_trace = _load_trace(base_dir)
                require_wsola_activity(spec, "baseline", base_dir, base_trace)
                base_samples = _check_captured_side(
                    "baseline", base_dir, base_trace, spec.start, widths, spec.route_end_poll
                )
                try:
                    control_dir, control_capture_receipt = backend.capture(
                        spec,
                        candidate=False,
                        control=True,
                        commit=spec.commit,
                        input_hash=baseline_input_hash,
                        save_dir=save_dir,
                        disc=disc,
                        host=baseline_host,
                    )
                except RuntimeError as error:
                    raise GateInvalid("control_capture_failed", str(error)[-800:]) from None
                verdict["control_run"] = str(control_dir)
                verdict["control_snapshot"] = control_capture_receipt.get("snapshot")
                if not distinct_control_captures(base_dir, control_dir):
                    raise GateInvalid(
                        "same_control_capture", "off/off controls are not distinct captures"
                    )
                if control_capture_receipt.get("exit_code"):
                    raise GateInvalid(
                        "control_capture_failed",
                        f"off/off control exited {control_capture_receipt.get('exit_code')}: "
                        f"{_failure_tail(control_dir)}",
                    )
                control_trace = _load_trace(control_dir)
                require_wsola_activity(spec, "control", control_dir, control_trace)
                control_samples = _check_captured_side(
                    "control", control_dir, control_trace, spec.start, widths, spec.route_end_poll
                )
                control_report_base = report_dir(backend.owner_root) / ("control-" + control_key)
                control_report_base.parent.mkdir(parents=True, exist_ok=True)
                control_report = backend.diff(
                    spec, host, base_dir, control_dir, control_report_base
                )
                comparison = control_report.get("comparison", {})
                call_args_check = (
                    call_args_determinism(base_dir, control_dir)
                    if spec.roots
                    else {"comparable": False, "reason": "no_requested_roots"}
                )
                first = comparison.get("first_divergence") or {}
                if call_args_check.get("comparable") and not call_args_check.get("equal"):
                    first = {
                        "poll": call_args_check.get("poll"),
                        "region": "call_args",
                        "row": call_args_check.get("first_row"),
                        "words": 0,
                    }
                first_words = first.get("words", [])
                control_record = {
                    "control_equal": control_report_repeatable(control_report)
                    and not (
                        call_args_check.get("comparable") and not call_args_check.get("equal")
                    ),
                    "baseline_run": str(base_dir),
                    "baseline_capture_receipt": base_receipt,
                    "baseline_samples": base_samples,
                    "control_run": str(control_dir),
                    "control_capture_receipt": control_capture_receipt,
                    "control_samples": control_samples,
                    "call_args_determinism": call_args_check,
                    "call_args_masks": CALL_ARGS_MASKS,
                    "report": str(control_report_base.with_suffix(".json")),
                    "first_divergence": {
                        "poll": first.get("poll"),
                        "region": first.get("region"),
                        "words": len(first_words)
                        if isinstance(first_words, (list, tuple))
                        else int(first_words or 0),
                        **({"row": first["row"]} if first.get("row") is not None else {}),
                    }
                    if first
                    else None,
                }
                if call_args_check.get("call_args_tail_text"):
                    control_record["call_args_tail_text"] = call_args_check["call_args_tail_text"]
                if call_args_check.get("call_args_tail_health"):
                    control_record["health"] = list(call_args_check["call_args_tail_health"])
                if not control_record["control_equal"]:
                    if call_args_check.get("reason") == "missing_call_args_file":
                        control_record["message"] = (
                            f"off/off {call_args_check['side']} references a missing "
                            "call-argument stream; baseline agreement cannot be established"
                        )
                    elif call_args_check.get("comparable") and not call_args_check.get("equal"):
                        control_record["message"] = (
                            f"off/off call-args differ at row {call_args_check.get('first_row')} "
                            f"poll {call_args_check.get('poll')}; raw arguments are omitted from "
                            "the receipt"
                        )
                    else:
                        control_record["message"] = control_comparison_failure(comparison)
                control_path.parent.mkdir(parents=True, exist_ok=True)
                control_path.write_text(json.dumps(control_record, indent=2, sort_keys=True) + "\n")

            verdict["control_receipt"] = str(control_path)
            verdict["control_equal"] = control_record.get("control_equal") is True
            verdict["control_run"] = control_record.get("control_run")
            control_tail = control_record.get("call_args_determinism") or {}
            if control_tail.get("call_args_common_last_poll") is not None:
                verdict["control_call_args_common_last_poll"] = control_tail[
                    "call_args_common_last_poll"
                ]
                verdict["control_call_args_tail_excluded"] = control_tail.get(
                    "call_args_tail_excluded", {}
                )
                if control_record.get("call_args_tail_text"):
                    verdict["control_call_args_tail_text"] = control_record["call_args_tail_text"]
                verdict["health"] = list(verdict.get("health", [])) + control_record.get(
                    "health", []
                )
            verdict["baseline_run"] = control_record.get("baseline_run")
            verdict["baseline_snapshot"] = control_record.get("baseline_capture_receipt", {}).get(
                "snapshot"
            )
            verdict["control_snapshot"] = control_record.get("control_capture_receipt", {}).get(
                "snapshot"
            )
            if control_record.get("control_equal") is not True:
                first = control_record.get("first_divergence") or {}
                verdict["first_divergence"] = first
                raise GateInvalid(
                    "nondeterministic_baseline",
                    control_record.get("message") or "off/off control diverged",
                    details={"first_divergence": first},
                )

            base_dir = Path(control_record["baseline_run"])
            base_receipt = control_record.get("baseline_capture_receipt") or {}
            base_trace = _load_trace(base_dir)
            require_wsola_activity(spec, "baseline", base_dir, base_trace)
            control_trace = _load_trace(Path(control_record["control_run"]))
            require_wsola_activity(
                spec, "control", Path(control_record["control_run"]), control_trace
            )
            base_samples = _check_captured_side(
                "baseline", base_dir, base_trace, spec.start, widths, spec.route_end_poll
            )
            try:
                cand_dir, cand_receipt = backend.capture(
                    spec,
                    candidate=True,
                    commit=spec.commit,
                    input_hash=input_hash,
                    save_dir=save_dir,
                    disc=disc,
                    host=host,
                )
            except RuntimeError as error:
                raise GateInvalid("candidate_capture_failed", str(error)[-800:]) from None
            verdict["candidate_run"] = str(cand_dir)
            verdict["candidate_snapshot"] = cand_receipt.get("snapshot")
            raise_if_host_refused("candidate", cand_dir)
            if base_dir.resolve() == cand_dir.resolve():
                raise GateInvalid(
                    "same_capture",
                    "baseline and candidate resolved to the same queued capture (the switch set "
                    f"normalises to the baseline's): {base_dir}; the switches change nothing",
                )
            # The candidate capture is the fault-free run: the whole route with the switches on.
            verdict["fault_free"] = False
            cand_trace = _load_trace(cand_dir)
            require_wsola_activity(spec, "candidate", cand_dir, cand_trace)
            cacheable = True
            verdict["samples"] = {
                "baseline": base_samples,
                "candidate": _check_captured_side(
                    "candidate",
                    cand_dir,
                    cand_trace,
                    spec.start,
                    widths,
                    spec.route_end_poll,
                ),
            }
            control_trace = _load_trace(Path(control_record["control_run"]))
            require_control_window(base_trace, control_trace, cand_trace)
            verdict["fault_free"] = not cand_receipt.get("exit_code")
            if spec.roots:
                calls = root_calls(cand_dir, cand_trace, spec.roots, spec.start)
                verdict["root_calls"] = calls
                # T3783: the candidate host runs Rust ports whenever roots are selected or
                # declared coverage-only; its stop report must exist before counts are read.
                expects_ports = candidate_expects_rust_ports(spec)
                check_roots(
                    calls,
                    spec.start,
                    spec.coverage_only_roots,
                    cand_trace if expects_ports else None,
                )
            if not verdict["fault_free"]:
                verdict["verdict"] = "DIVERGED"
                verdict["reasons"].append("candidate_faulted")
                verdict["fault_detail"] = (
                    f"exit {cand_receipt.get('exit_code')}: {_failure_tail(cand_dir)}"
                )
            report_base = report_dir(backend.owner_root) / key
            report_base.parent.mkdir(parents=True, exist_ok=True)
            report = backend.diff(spec, host, base_dir, cand_dir, report_base)
            comparison = report.get("comparison", {})
            verdict["report"] = str(report_base.with_suffix(".json"))
            verdict["samples_compared"] = comparison.get("samples_compared")
            record_stack_residue(verdict, comparison)
            record_call_tail(verdict, comparison)
            require_complete_segment_coverage(report)
            if (comparison.get("samples_compared") or 0) < MIN_WINDOW_SAMPLES:
                raise GateInvalid(
                    "too_few_compared",
                    f"diff compared {comparison.get('samples_compared')} samples "
                    f"(need {MIN_WINDOW_SAMPLES}): {report_base.with_suffix('.json')}",
                )
            if report.get("equal") and verdict["fault_free"]:
                verdict["verdict"] = "EQUAL"
            else:
                verdict["verdict"] = "DIVERGED"
                first = comparison.get("first_divergence") or {}
                verdict["first_divergence"] = {
                    "poll": first.get("poll"),
                    "region": first.get("region"),
                    "words": len(first.get("words", [])),
                }
                verdict["diverging_regions"] = comparison.get("diverging_regions")
                if not report.get("equal"):
                    verdict["reasons"].append(comparison.get("reason") or "state_divergence")
    except GateInvalid as invalid:
        host_leases.close()
        if invalid.code.endswith("_window_not_reached") or invalid.code.endswith(
            "_too_few_samples"
        ):
            verdict["verdict"] = "UNREACHABLE"
        elif invalid.code in (
            "roots_not_called",
            "root_window_unavailable",
            "roots_not_rust",
            "regions_inert",
        ):
            verdict["verdict"] = "VACUOUS"
        else:
            verdict["verdict"] = "INVALID"
        verdict["reasons"].append(invalid.code)
        verdict["message"] = invalid.message
        verdict.update(invalid.details)
        cacheable = cacheable and invalid.code in CACHEABLE_INVALID
    finally:
        host_leases.close()
    verdict["elapsed_seconds"] = round(time.monotonic() - started, 1)
    verdict["capture_breakdown"] = dict(getattr(backend, "timings", {}))
    verdict["capture_timings"] = verdict["capture_breakdown"]  # T3591 compatibility
    if cacheable:
        receipt_path.parent.mkdir(parents=True, exist_ok=True)
        receipt_path.write_text(json.dumps(verdict, indent=2, sort_keys=True) + "\n")
        verification_store.put(
            backend.owner_root, store_key, store_request, verdict, replace=not use_cache
        )
    else:
        verdict["receipt"] = None
    gate_history.record(backend.owner_root, verdict)
    return verdict


def control_cache_stats(owner_root: Path) -> dict[str, object]:
    """Count candidate captures which reused a cached off/off control receipt."""
    receipt_root = receipt_dir(owner_root)
    candidates = hits = misses = unknown = 0
    controls: set[str] = set()
    if receipt_root.is_dir():
        for path in receipt_root.glob("*.json"):
            if path.name.startswith("control-") or path.stat().st_size > 1_000_000:
                continue
            try:
                receipt = json.loads(path.read_text())
            except (OSError, json.JSONDecodeError):
                continue
            if not receipt.get("candidate_run"):
                continue
            if "control_cache_hit" not in receipt:
                unknown += 1
                continue
            candidates += 1
            if receipt["control_cache_hit"]:
                hits += 1
                controls.add(str(receipt.get("control_receipt", "")))
            else:
                misses += 1
    return {
        "candidate_runs": candidates,
        "control_cache_hits": hits,
        "control_cache_misses": misses,
        "unknown_legacy_candidate_runs": unknown,
        "unique_cached_controls_reused": len(controls),
        "hit_rate": round(hits / candidates, 4) if candidates else 0.0,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.gate",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--profile", help="gate profile from tools/data/gate_profiles.json")
    parser.add_argument("--commit", help="candidate commit (default HEAD of the cwd worktree)")
    parser.add_argument("--branch", help="candidate branch (resolved in the cwd repository)")
    parser.add_argument("--switches", help="comma list: Rust port selectors, host flags, TSFP_*=v")
    parser.add_argument(
        "--audio-command-log",
        action="store_true",
        help="capture audio commands separately in each audio gate replay directory",
    )
    parser.add_argument(
        "--baseline-host-args",
        help="comma list of host CLI args applied only to the switch-off capture",
    )
    parser.add_argument(
        "--route", help="route path, stem, short name, or auto (story-level1, arcade-full)"
    )
    parser.add_argument("--start", help="window start: poll, mark:N, gameplay (default) or boot")
    parser.add_argument(
        "--segments", type=int, help="split the --start through route-end window into K resumes"
    )
    parser.add_argument("--regions", help="rust_trace region-set JSON")
    parser.add_argument("--roots", help="comma list of hex VAs that must be called in the window")
    parser.add_argument(
        "--seconds",
        type=int,
        default=DEFAULT_SECONDS,
        help="capture budget; the candidate run doubles as the fault-free run",
    )
    parser.add_argument(
        "--render",
        choices=("none", "gpu"),
        default="none",
        help=(
            "none is the headless state gate (default); gpu requests Bevy output rendering and "
            "requires a full replay from boot"
        ),
    )
    parser.add_argument(
        "--paced",
        action="store_true",
        help="pace a GPU-rendered gate to let the live audio callback consume output in real time",
    )
    parser.add_argument(
        "--snapshot",
        action="store_true",
        default=True,
        help="(default since T3591) capture both sides from cached DMTCP snapshots at the start "
        "poll when --start is after boot; the off/off control still guards it, "
        "see docs/t3591-replay-speed.md",
    )
    parser.add_argument(
        "--full-replay",
        dest="snapshot",
        action="store_false",
        help="boot and replay from poll 0 on every side instead of resuming from a snapshot",
    )
    parser.add_argument(
        "--no-adopted",
        action="store_true",
        help="do not add the adopted systems (gate_profiles.json `adopted`) to both sides",
    )
    parser.add_argument("--no-cache", action="store_true", help="ignore a cached receipt")
    parser.add_argument(
        "--retry-known-invalid",
        action="store_true",
        help="T3600: run a request the verdict history proves ends INVALID again (known candidate "
        "stall, empty capture, non reproducible control, root outside the window); use it once a "
        "change could have fixed the cause. TSFP_GATE_RETRY_KNOWN_INVALID=1 does the same",
    )
    parser.add_argument("--no-build", action="store_true", help="fail instead of building a host")
    parser.add_argument(
        "--stats",
        action="store_true",
        help="summarize persisted candidate verdicts and off/off control-cache reuse",
    )
    remote_group = parser.add_mutually_exclusive_group()
    remote_group.add_argument(
        "--remote",
        dest="remote",
        action="store_true",
        help="run replay captures on replay-capable registry hosts (the default)",
    )
    remote_group.add_argument(
        "--local",
        dest="remote",
        action="store_false",
        help="keep replay captures on this host",
    )
    parser.set_defaults(remote=True)
    parser.add_argument(
        "--local-reason",
        help="why --local must stay on this host; required with --local (the laptop and Deck hold "
        "the owner inputs, and local gates queue on the owner PC's one game slot)",
    )
    parser.add_argument("--list-profiles", action="store_true")
    parser.add_argument(
        "--resolve-only",
        action="store_true",
        help="print the resolved request and exit (no captures)",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run the gate and print exactly one JSON document on stdout.

    T3600 follow-up: building the prebuilt host (build.sh, cargo, the remote builder) and every
    nested play.sh or replay call wrote to this process's stdout ahead of the JSON report, so
    `json.load` of a gate that had to build raised JSONDecodeError (131 KB, JSON on line 2219).
    While the gate works, file descriptor 1 and `sys.stdout` point to stderr; the report is the
    only thing written to the real stdout, after the work.
    """
    from tools.process_priority import lower_priority
    from tools.wt import core

    core.apply_rust_env(Path(__file__).resolve().parents[1])
    lower_priority()
    sys.stdout.flush()
    report_fd = os.dup(1)
    os.dup2(2, 1)
    try:
        with contextlib.redirect_stdout(sys.stderr):
            document, code = _run(argv)
    finally:
        sys.stdout.flush()
        os.dup2(report_fd, 1)
        os.close(report_fd)
    if document is not None:
        if code:
            try:
                report = json.loads(document)
            except json.JSONDecodeError:
                report = {}
            verdict = report.get("verdict")
            if isinstance(verdict, str):
                message = report.get("message") or "; ".join(report.get("reasons", []))
                print(
                    f"gate: VERDICT {verdict}: {message or 'gate did not pass'}",
                    file=sys.stderr,
                )
        print(document)
        sys.stdout.flush()
    return code


def _run(argv: list[str] | None) -> tuple[str | None, int]:
    """The gate itself: (the JSON report text, exit code)."""
    args = build_parser().parse_args(argv)
    owner_root = main_root()
    if args.stats:
        return json.dumps(control_cache_stats(owner_root), sort_keys=True), 0
    if args.list_profiles:
        return json.dumps(load_profiles(), indent=2, sort_keys=True), 0
    try:
        spec = resolve_spec(args, owner_root)
    except GateInvalid as invalid:
        report = {"verdict": "INVALID", "reasons": [invalid.code], "message": invalid.message}
        return json.dumps(report, sort_keys=True), 2
    if args.resolve_only:
        printable = {
            **spec.__dict__,
            "route": str(spec.route),
            "regions": str(spec.regions),
            "roots": [f"0x{root:X}" for root in spec.roots],
        }
        return json.dumps(printable, indent=2, sort_keys=True), 0
    if not args.remote and not (args.local_reason or "").strip():
        # I-T3885: lanes defaulted to --local and queued 50+ min on the owner PC's single game
        # slot while replay-capable remotes with the owner inputs sat idle.
        message = "--local needs --local-reason TEXT; remote replay hosts are the default"
        report = {"verdict": "INVALID", "reasons": ["local-without-reason"], "message": message}
        return json.dumps(report, sort_keys=True), 2
    remote = None
    if args.remote:
        from tools.wt import remote_exec

        os.environ["TSFP_REMOTE_EXEC"] = "1"
        remote = remote_exec.apply_gate_environment()
        if configure_remote_capture(spec, remote):
            print(f"gate: remote replay hosts {json.dumps(remote)}", file=sys.stderr)
        else:
            # With no admissible remote worker, use the local snapshot path unchanged.
            remote = None
    # The replay queue also reads runner-exported workers, independently of --local.
    inherited_workers = (
        os.environ.pop("TSFP_REPLAY_REMOTE_HOSTS", None) if not args.remote else None
    )
    try:
        verdict = run_gate(
            spec, Backend(owner_root), use_cache=not args.no_cache, build=not args.no_build
        )
    finally:
        if inherited_workers is not None:
            os.environ["TSFP_REPLAY_REMOTE_HOSTS"] = inherited_workers
    if remote is not None:
        verdict["remote_exec"] = remote
    from tools import regress_block

    blocked = regress_block.read()
    if blocked is not None:  # T3587: an EQUAL verdict is not adoption evidence while main faults
        verdict["adoption_blocked"] = regress_block.describe(blocked)
    return json.dumps(verdict, sort_keys=True), {"EQUAL": 0, "DIVERGED": 1}.get(
        verdict["verdict"], 2
    )


if __name__ == "__main__":
    raise SystemExit(main())
