"""Build profiles: the optional Rust modules compiled into a host (T3597).

`tools/data/build-profile-<name>.env` is the one list of `TSFP_BUILD_*=1` switches for a profile.
`tools/dev/build.sh` reads it for `TSFP_BUILD_PROFILE=<name>` and the shared prebuilt (gate and
adoption hosts) sets the `gate` profile, so both build the same modules. Every `TSFP_BUILD_*`
switch of build.sh is either in the profile, default-on already, or excluded with a reason in
`CLASSIFICATION`; tests/test_build_profile.py fails on an unclassified new switch.
"""

from __future__ import annotations

import re
from pathlib import Path

PROFILE_DIR = "tools/data"
PROFILE_PREFIX = "build-profile-"
GATE_PROFILE = "gate"

# Switch -> (state, reason). state: "profile" (compiled into the gate profile), "default-on"
# (build.sh already enables it), "excluded" (changes behaviour without a runtime variable or is a
# measurement build). MEASURED by source audit 2026-10-22, docs/t3597-gate-prebuilt-modules.md.
CLASSIFICATION: dict[str, tuple[str, str]] = {
    "TSFP_BUILD_GAME_BUMP": (
        "profile",
        "0x3E970 route; default on (T3879 MEM-3), runtime TSFP_RUST_GAME_BUMP=0 selects C",
    ),
    "TSFP_BUILD_GAME_ARENA": (
        "profile",
        "arena and constructor routes; runtime TSFP_RUST_GAME_ARENA",
    ),
    "TSFP_BUILD_GAME_POOL": ("profile", "pool routes; runtime TSFP_RUST_GAME_POOL"),
    "TSFP_BUILD_GAME_LEDGER": ("profile", "ledger routes; runtime TSFP_RUST_GAME_LEDGER"),
    "TSFP_BUILD_INPUT_HLE": (
        "profile",
        "XInput enumeration adapter; runtime TSFP_RUST_INPUT_HLE selects Rust",
    ),
    "TSFP_BUILD_KCLOCK": ("default-on", "build.sh default 1; runtime TSFP_RUST_KCLOCK=0 selects C"),
    "TSFP_BUILD_KSERVICES": ("excluded", "experimental K-3 selector, Tier 2 pending"),
    "TSFP_BUILD_NATIVE_FILEIO": (
        "default-on",
        "build.sh default 1 (T3879): no kernel_file.c or xdvdfs.c in the host; =0 keeps C",
    ),
    "TSFP_BUILD_DETSCHED_BACKEDGES": (
        "excluded",
        "experimental detsched profile only; runtime scheduler remains opt-in",
    ),
    "TSFP_BUILD_AUDIO_HOST": (
        "default-on",
        "build.sh default 1 (T3927), mandatory since T3959: the C audio host units are retired",
    ),
    "TSFP_BUILD_WSOLA": (
        "excluded",
        "experimental opt-in WSOLA backend; sealed host conformance and adoption gate pending",
    ),
}

_SWITCH = re.compile(r"^TSFP_BUILD_[A-Z0-9_]+$")


class ProfileError(ValueError):
    """An unknown or malformed build profile."""


def profile_path(root: Path, name: str) -> Path:
    if not re.fullmatch(r"[a-z][a-z0-9-]*", name):
        raise ProfileError(f"invalid build profile name {name!r}")
    return root / PROFILE_DIR / f"{PROFILE_PREFIX}{name}.env"


def parse(text: str) -> dict[str, str]:
    """`KEY=VALUE` lines; blank lines and `#` comments are ignored."""
    switches: dict[str, str] = {}
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        key, separator, value = line.partition("=")
        if not separator or not _SWITCH.fullmatch(key) or value not in {"0", "1"}:
            raise ProfileError(f"line {number}: expected TSFP_BUILD_<NAME>=0|1, got {raw!r}")
        if key in switches:
            raise ProfileError(f"line {number}: duplicate switch {key}")
        switches[key] = value
    return switches


def load(root: Path, name: str | None = None) -> dict[str, str]:
    name = name or GATE_PROFILE
    path = profile_path(root, name)
    try:
        switches = parse(path.read_text())
    except OSError as error:
        raise ProfileError(f"build profile {name!r} is not readable: {path}") from error
    if not switches:
        raise ProfileError(f"build profile {name!r} lists no switches: {path}")
    return switches


# Switches whose CMake option does not follow the TSFP_RUST_ convention.
CMAKE_OPTIONS = {
    "TSFP_BUILD_LIVE_CENSUS": "TSFP_LIVE_CENSUS",
    "TSFP_BUILD_NATIVE_FILEIO": "TSFP_NATIVE_FILEIO",
    "TSFP_BUILD_DETSCHED_BACKEDGES": "TSFP_DETSCHED_BACKEDGES",
}  # T3824/T3879


def cmake_option(switch: str) -> str:
    """TSFP_BUILD_GAME_ARENA -> TSFP_RUST_GAME_ARENA (build.sh's naming convention)."""
    return CMAKE_OPTIONS.get(switch) or switch.replace("TSFP_BUILD_", "TSFP_RUST_", 1)


def verify_cmake_cache(cache_text: str, switches: dict[str, str]) -> list[str]:
    """Problems of a CMakeCache.txt that does not match the profile (empty list: it matches)."""
    problems = []
    for switch, value in sorted(switches.items()):
        option = cmake_option(switch)
        want = "ON" if value == "1" else "OFF"
        match = re.search(rf"^{re.escape(option)}:[A-Z]+=(\S*)$", cache_text, re.MULTILINE)
        if match is None:
            problems.append(f"{option} is missing from the CMake cache")
        elif match.group(1) != want:
            problems.append(f"{option} is {match.group(1)}, the profile needs {want}")
    return problems


def environment(root: Path, name: str | None = None) -> dict[str, str]:
    """Environment that makes build.sh (old or new) build the profile's modules."""
    name = name or GATE_PROFILE
    return {**load(root, name), "TSFP_BUILD_PROFILE": name}
