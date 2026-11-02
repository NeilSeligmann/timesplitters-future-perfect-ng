"""Retired remote hosts (T3753): entries of tools/data/remote_hosts.json (or hosts of
tools/data/fleet.json) with `"retired": true`.

The owner deletes a machine (the DigitalOcean droplets, 2026-10-26) and every consumer must stop
selecting it without erroring on the missing host. The tracked flag is the one switch: unlike the
untracked `tmp/codexapi/<name>.off` file it holds in every checkout and survives a registry
reload. The entry stays in the JSON as the decommission record.
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

from tools import host_registry

REGISTRY = host_registry.REGISTRY


def is_retired(entry: Mapping[str, Any]) -> bool:
    """True for a registry or fleet host entry marked retired."""
    return entry.get("retired") is True


def active(entries: Iterable[Mapping[str, Any]]) -> list[Mapping[str, Any]]:
    """The entries that are not retired, in order."""
    return [entry for entry in entries if not is_retired(entry)]


def retired_names(path: Path = REGISTRY) -> frozenset[str]:
    """Names (and replay names) of retired hosts in the registry. Empty when unreadable."""
    entries = host_registry.hosts_or_empty(path)  # T3754: the one registry loader
    return frozenset(
        str(entry[key])
        for entry in entries
        if isinstance(entry, dict) and is_retired(entry)
        for key in ("name", "replay_name")
        if entry.get(key)
    )


def retired_identities(path: Path = REGISTRY) -> frozenset[str]:
    """Names, replay names, addresses and replay targets of retired hosts (replay worker match)."""
    entries = host_registry.hosts_or_empty(path)
    return frozenset(
        str(entry[key])
        for entry in entries
        if isinstance(entry, dict) and is_retired(entry)
        for key in ("name", "replay_name", "address", "replay_target")
        if entry.get(key)
    )
