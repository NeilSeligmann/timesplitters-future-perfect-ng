# SPDX-License-Identifier: GPL-3.0-or-later
"""Log analysis for bounded playable-first smoke runs (T4342)."""

import argparse
import json
import re
import subprocess
import sys
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path

from tools import owner_files
from tools.wt import prebuilt

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "tools/data/smoke_routes.json"


@dataclass
class LogEvidence:
    host_fault: bool = False
    physical_range_errors: int = 0
    scene_reached: bool = False
    frame_p99_ms: float | None = None
    underrun_events: int | None = None


def analyze_logs(paths: list[Path], pak: str) -> LogEvidence:
    """Stream logs: replay output can be large and paths may contain private data."""
    result = LogEvidence()
    expected = pak.lower().replace("/", "\\")
    for path in paths:
        if not path.is_file():
            continue
        with path.open(errors="replace") as source:
            for line in source:
                lower = line.lower()
                if (
                    "stopped: host fault" in lower
                    or re.search(r"\bstop\s+thread .*: host fault", lower)
                    or re.search(r"\bexit(?:ed| code)?\s*[:=]?\s*-11\b", lower)
                ):
                    result.host_fault = True
                if "no synthetic physical range" in lower:
                    result.physical_range_errors += 1
                if "ntquery" in lower and expected in lower.replace("/", "\\"):
                    result.scene_reached = True
                match = re.search(r"gap_ms\s+p50=[\d.]+\s+p99=([\d.]+)", lower)
                if match:
                    value = float(match.group(1))
                    result.frame_p99_ms = max(result.frame_p99_ms or 0, value)
                match = re.search(r"underrun\s+\d+\s+\(events\s+(\d+)(?:\)|;)", lower)
                if match:
                    result.underrun_events = max(result.underrun_events or 0, int(match.group(1)))
    return result


def frame_nonblack(path: Path) -> bool:
    """Decode the captured image, ignoring alpha; missing/invalid images are errors."""
    decoded = subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-vf",
            "scale=64:64,format=rgb24",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-",
        ],
        capture_output=True,
        check=True,
    ).stdout
    if len(decoded) != 64 * 64 * 3:
        raise ValueError("missing or invalid screenshot")
    return max(decoded) > 8


def assess(
    evidence: LogEvidence, *, exit_code: int, nonblack: bool | None, limits: dict
) -> list[str]:
    failures = []
    if exit_code != 0 or evidence.host_fault:
        failures.append("host fault or replay failure")
    if evidence.physical_range_errors > limits["physical_range_errors"]:
        failures.append("synthetic physical range flood")
    if not evidence.scene_reached:
        failures.append("scene not observed")
    if evidence.frame_p99_ms is None or evidence.frame_p99_ms > limits["frame_p99_ms"]:
        failures.append("frame p99 missing or over limit")
    if evidence.underrun_events is None or evidence.underrun_events > limits["underrun_events"]:
        failures.append("audio underruns missing or over limit")
    if nonblack is not True:
        failures.append("final screenshot missing or black")
    return failures


def _run(command: list[str]) -> tuple[int, str]:
    # Queue output can be large; only retain its final lines and the JSON receipt.
    with tempfile.TemporaryFile(mode="w+") as out:
        proc = subprocess.run(command, cwd=ROOT, stdout=out, stderr=out, check=False)
        out.seek(0)
        receipt = ""
        for line in out:
            if line.startswith("{"):
                receipt = line
        return proc.returncode, receipt


def run_route(route: dict, commit: str, seconds: int, limits: dict) -> dict:
    candidates = owner_files.candidates(ROOT, route["recording"])
    recording = next((path for path in candidates if path.is_file()), candidates[0])
    command = [
        sys.executable,
        "-m",
        "tools.wt",
        "replay",
        "submit",
        "--route",
        str(recording),
        "--switches",
        "off",
        "--commit",
        commit,
        "--seconds",
        str(seconds),
        "--render",
        "gpu",
        "--shot-at",
        str(max(1, seconds - 1)),
        "--no-stop-at-route-end",
        "--host-arg=--perf-log",
        "--no-cache",
        "--env",
        "TSFP_VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json",
    ]
    if route.get("save_dir"):
        choices = owner_files.candidates(ROOT, route["save_dir"])
        save = next((path for path in choices if path.is_dir()), choices[0])
        command += ["--save-dir", str(save)]
    code, output = _run(command)
    try:
        receipt = json.loads(output)
        run_dir = Path(receipt["run_dir"])
        host_exit = receipt.get("exit_code", code)
        if not isinstance(host_exit, int):
            raise ValueError("receipt missing exit code")
    except (ValueError, KeyError, TypeError):
        return {
            "route": route["id"],
            "status": "FAIL",
            "failures": ["replay receipt missing"],
            "exit_code": code,
        }
    logs = [run_dir / name for name in ("host.out", "host.err", "stop.txt")]
    # Remote queue may retain only the combined stream; do not count both copies.
    if not all(path.is_file() for path in logs) and (run_dir / "run.log").is_file():
        logs = [run_dir / "run.log"]
    evidence = analyze_logs(logs, route["pak"])
    try:
        image = frame_nonblack(run_dir / f"shot-{max(1, seconds - 1)}.png")
    except (OSError, ValueError, subprocess.SubprocessError):
        image = None
    failures = assess(
        evidence, exit_code=host_exit if code == 0 else code, nonblack=image, limits=limits
    )
    # A fault can prevent shutdown metrics and the scheduled screenshot from being written.
    # Do not waive an absent scene, a recorded black frame, or an observed limit violation.
    expected = route.get("expected_fault_task")
    status = "PASS" if not failures else "FAIL"
    if (
        expected
        and evidence.host_fault
        and evidence.scene_reached
        and evidence.physical_range_errors <= limits["physical_range_errors"]
        and (evidence.frame_p99_ms is None or evidence.frame_p99_ms <= limits["frame_p99_ms"])
        and (
            evidence.underrun_events is None
            or evidence.underrun_events <= limits["underrun_events"]
        )
        and image is not False
    ):
        status = "XFAIL"
    return {
        "route": route["id"],
        "status": status,
        "failures": failures,
        "expected_fault_task": expected,
        "run_dir": str(run_dir),
        "exit_code": host_exit,
        "nonblack": image,
        "evidence": asdict(evidence),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run bounded survival and sanity replays")
    parser.add_argument("--commit", default="HEAD")
    parser.add_argument("--seconds", type=int)
    parser.add_argument("--routes", help="comma-separated route IDs")
    parser.add_argument("--quick", action="store_true")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args(argv)
    seconds = args.seconds if args.seconds is not None else (20 if args.quick else 40)
    if seconds < 2:
        parser.error("--seconds must be at least 2")
    manifest = json.loads(MANIFEST.read_text())
    requested = (
        args.routes.split(",") if args.routes else (manifest["quick"] if args.quick else None)
    )
    routes = [r for r in manifest["routes"] if requested is None or r["id"] in requested]
    if requested is not None and (
        set(requested) != {r["id"] for r in routes} or len(requested) != len(set(requested))
    ):
        parser.error("unknown or duplicate route")
    try:
        commit = prebuilt.resolve_commit(ROOT, args.commit)
        if prebuilt.lookup(ROOT, commit) is None:
            code, _ = _run(
                [sys.executable, "-m", "tools.wt", "prebuilt", "build", "--commit", commit]
            )
            if code:
                raise RuntimeError(f"prebuilt build failed ({code})")
        results = [run_route(route, commit, seconds, manifest["limits"]) for route in routes]
    except (OSError, ValueError, RuntimeError, prebuilt.PrebuiltError) as error:
        print(f"smoke: {error}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps({"commit": commit, "seconds": seconds, "results": results}, indent=2))
    else:
        print(f"{'ROUTE':22} {'RESULT':7} DETAILS")
        for result in results:
            print(f"{result['route']:22} {result['status']:7} {', '.join(result['failures'])}")
    return int(any(result["status"] == "FAIL" for result in results))
