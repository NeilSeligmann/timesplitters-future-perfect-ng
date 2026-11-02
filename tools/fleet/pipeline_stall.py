"""Artifact-based pipeline stall detectors (incident 2026-10-25, T3750).

A pipeline heartbeat (`tools.agents.pipeline_status`) declares progress artifacts. Two findings:

- `pipeline-heartbeat-missed-<name>`: an active pipeline has not beaten for 20 minutes.
- `pipeline-stalled-<name>`: an active pipeline's declared artifacts show no new or updated file
  for 20 minutes (measured from the state change when none exists yet). `capturing` and `checking`
  with no declared artifact at all are flagged as well: "started" must be provable on disk.

The `done`/`total` counters in the heartbeat are messages and never count as progress.
"""

from __future__ import annotations

import glob
import re
from pathlib import Path
from typing import TYPE_CHECKING

from tools.agents.pipeline_status import ACTIVE_STATES, pipeline_dir, read_record
from tools.fleet.core import Finding

if TYPE_CHECKING:
    from tools.fleet.detectors import HealthContext

STALL_SECONDS = 1200
ARTIFACT_REQUIRED_STATES = ("capturing", "checking")


def artifact_progress(root: Path, patterns: list[str]) -> tuple[int, float | None]:
    """(file count, newest mtime) over the declared artifact globs."""
    count, newest = 0, None
    for pattern in patterns:
        target = pattern if pattern.startswith("/") else str(root / pattern)
        for match in glob.glob(target, recursive=True):
            try:
                mtime = Path(match).stat().st_mtime
            except OSError:
                continue
            count += 1
            newest = mtime if newest is None else max(newest, mtime)
    return count, newest


def _ident(name: str) -> str:
    return re.sub(r"[^a-z0-9-]", "-", name.lower())[:40]


def pipeline_findings(root: Path, directory: Path, now: float) -> list[Finding]:
    findings: list[Finding] = []
    for path in sorted(directory.glob("*.json")):
        record = read_record(path)
        if not record or record.get("state") not in ACTIVE_STATES:
            continue
        name = str(record.get("name") or path.stem)
        ident = _ident(name)
        state = record["state"]
        beat_age = now - float(record.get("updated", 0))
        if beat_age > STALL_SECONDS:
            findings.append(
                Finding(
                    f"pipeline-heartbeat-missed-{ident}",
                    f"{name} ({state}) last heartbeat {int(beat_age) // 60} min ago; "
                    f"check the artifacts, not the messages",
                )
            )
        patterns = [p for p in record.get("artifacts", []) if isinstance(p, str)]
        if not patterns:
            if state in ARTIFACT_REQUIRED_STATES:
                findings.append(
                    Finding(
                        f"pipeline-stalled-{ident}",
                        f"{name} says {state} but declares no progress artifacts "
                        f"(pipeline_status set --artifact GLOB)",
                    )
                )
            continue
        count, newest = artifact_progress(root, patterns)
        last_progress = max(newest or 0.0, float(record.get("state_since", 0)))
        idle = now - last_progress
        if idle > STALL_SECONDS:
            findings.append(
                Finding(
                    f"pipeline-stalled-{ident}",
                    f"{name} is {state} ({record.get('done')}/{record.get('total')} reported) "
                    f"but its {count} artifact(s) have not grown for {int(idle) // 60} min",
                )
            )
    return findings


def detect_pipeline_stalled(ctx: HealthContext) -> list[Finding]:
    return pipeline_findings(ctx.repo_root, pipeline_dir(ctx.repo_root), ctx.now)
