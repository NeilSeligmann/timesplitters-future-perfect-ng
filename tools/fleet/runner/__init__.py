"""Tracked Codex lane runner (T3708): template, per-host renderer and drift check.

`run.sh.in` is the single source of the capped Codex runner. `render_runner` fills the per-host
variables from tools/data/fleet.json (`provision.runner`) and remote_hosts.json, so every host runs
the same guarded script and a hand-patched copy shows up as a sha256 mismatch.

Template directives (own line each, not nested): `#@IF local` / `#@IF remote` ... `#@END` keep the
block only for the local host or only for remote hosts. `@NAME@` tokens are substituted.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

from tools import host_registry
from tools.fleet import replay_registry
from tools.fleet.config import FleetConfig

RUNNER_DIR = Path(__file__).resolve().parent
TEMPLATE = RUNNER_DIR / "run.sh.in"
RGRC = RUNNER_DIR / "rgrc"
LANE_GATE = RUNNER_DIR / "lane_gate.sh"
DEFAULT_ROTATE = 250000
REPLAY_HOST = "deck"
"""Registry host that serves the local runner's remote headless replays (T3407)."""
REMOTE_CODEXAPI = "$HOME/csup/codexapi"
_TOKEN = re.compile(r"@[A-Z_]+@")


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def _data_root(host_name: str) -> str:
    """The host repo dir as a double-quote-safe shell word; the local host has none (unused)."""
    try:
        return host_registry.home_relative(host_registry.host_key(host_name, "data_root"))
    except host_registry.RegistryError:
        return ""


def runner_variables(config: FleetConfig, host_name: str) -> dict[str, str]:
    """Per-host template values: provision.runner defaults, then its per-host overrides."""
    host = config.host(host_name)
    policy = config.provision.get("runner", {})
    override = policy.get("hosts", {}).get(host.name, {})

    def pick(key: str, default: object) -> str:
        return str(override.get(key, policy.get(key, default)))

    user = host.user or "claude-user"
    if host.is_local:
        codexapi = str(config.codexapi_dir)
        workers_map = str(config.root / "tmp" / "sup" / "workers.map")
    else:
        codexapi = REMOTE_CODEXAPI
        workers_map = "/dev/null"  # claude-sup workers.map is local only: those guards stay inert
    return {
        "DATA_ROOT": _data_root(host.name),
        "REPLAY_REPO": host_registry.host_key(REPLAY_HOST, "data_root"),
        "CODEXAPI": codexapi,
        "WORKERS_MAP": workers_map,
        "CODEX_BIN": pick("codex_bin", f"/home/{user}/.codex-cli/bin/codex"),
        "ROTATE": pick("rotate", DEFAULT_ROTATE),
        "REPLAY_SLOTS": pick("replay_slots", max(host.replay_max, 1)),
        "REPLAY_HOSTS_JSON": json.dumps(
            replay_registry.replay_hosts(),
            separators=(",", ":"),
        ),
    }


def _select_blocks(template: str, is_local: bool) -> str:
    keep = True
    out: list[str] = []
    for line in template.split("\n"):
        if line.startswith("#@IF "):
            kind = line[5:].strip()
            if kind not in ("local", "remote"):
                raise ValueError(f"unknown template condition: {line}")
            keep = (kind == "local") == is_local
        elif line.strip() == "#@END":
            keep = True
        elif keep:
            out.append(line)
    return "\n".join(out)


def render_runner(config: FleetConfig, host_name: str, template: str | None = None) -> str:
    host = config.host(host_name)
    text = _select_blocks(template if template is not None else TEMPLATE.read_text(), host.is_local)
    for key, value in runner_variables(config, host_name).items():
        text = text.replace(f"@{key}@", value)
    left = _TOKEN.findall(text)
    if left:
        raise ValueError(f"unfilled template tokens: {sorted(set(left))}")
    return text


def rgrc_text() -> str:
    return RGRC.read_text()


def lane_gate_text() -> str:
    return LANE_GATE.read_text()
