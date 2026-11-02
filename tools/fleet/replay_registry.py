"""Derive replay worker views from the single remote host registry."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = ROOT / "tools/data/remote_hosts.json"
FLEET = ROOT / "tools/data/fleet.json"


def _hosts(registry: Path | None = None) -> list[dict[str, Any]]:
    registry = registry or REGISTRY
    document = json.loads(registry.read_text())
    entries = document["hosts"]
    return list(entries.values()) if isinstance(entries, dict) else list(entries)


def replay_hosts(registry: Path | None = None) -> list[dict[str, Any]]:
    """Return configured replay-capable hosts, including their address and capacity."""
    result = []
    for host in _hosts(registry):
        replay = host.get("replay")
        if host.get("retired") or "replay" not in host.get("capabilities", []) or not replay:
            continue
        slots = int(replay["max_slots"])
        if slots <= 0:
            continue
        result.append(
            {
                "name": replay.get("worker_name", host["name"]),
                "target": replay.get("target") or f"{host['user']}@{host['address']}",
                "port": int(host.get("port", 22)),
                "max_slots": slots,
            }
        )
    return result


def worker_memory_mb(job_rss_mb: int, registry: Path | None = None) -> dict[str, tuple[int, int]]:
    """Per-worker (reserve, job RSS) tuples, keyed by the actual replay worker name."""
    result = {}
    for host in _hosts(registry):
        replay = host.get("replay")
        if "replay" in host.get("capabilities", []) and replay:
            result[replay.get("worker_name", host["name"])] = (
                int(replay.get("memory_reserve_mb", 0)),
                int(replay.get("job_rss_mb", job_rss_mb)),
            )
    return result


def remote_host_caps(registry: Path | None = None) -> dict[str, int]:
    return {
        host["address"]: int(host["replay"]["remote_host_cap"])
        for host in _hosts(registry)
        if host.get("replay", {}).get("remote_host_cap") is not None
    }


def fleet_data(fleet_file: Path | None = None) -> dict[str, Any]:
    """Render registry-owned replay policy into fleet.json's compatibility view."""
    fleet_file = fleet_file or FLEET
    fleet = json.loads(fleet_file.read_text())
    registered = {host["name"]: host for host in _hosts()}
    for name, entry in fleet["hosts"].items():
        host = registered.get(name, {})
        replay = host.get("replay") if "replay" in host.get("capabilities", []) else None
        for key in ("replay_max", "replay_name", "replay_target"):
            entry.pop(key, None)
        if replay:
            entry["replay_max"] = int(replay["max_slots"])
            entry["replay_name"] = replay.get("worker_name", name)
            if replay.get("target"):
                entry["replay_target"] = replay["target"]
    return fleet


def check_fleet(fleet_file: Path | None = None) -> bool:
    fleet_file = fleet_file or FLEET
    return json.loads(fleet_file.read_text()) == fleet_data(fleet_file)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--runner-json", action="store_true")
    args = parser.parse_args()
    if args.runner_json:
        print(json.dumps(replay_hosts(), separators=(",", ":")))
    elif args.check:
        if not check_fleet():
            print(
                "fleet.json replay entries drift from remote_hosts.json; "
                "run python3 -m tools.fleet.replay_registry --write"
            )
            return 1
    else:
        FLEET.write_text(json.dumps(fleet_data(), indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
