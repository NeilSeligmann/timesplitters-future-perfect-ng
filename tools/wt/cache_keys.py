# SPDX-License-Identifier: GPL-3.0-or-later
"""Shared, host-independent cache key primitives (T3638).

Cache keys describe content and output-affecting tools/options. Worktree names, absolute paths,
hostnames, slots, Cargo target directories, job counts and incremental bookkeeping are deliberately
excluded. Callers supply the inputs that define their output and a cache-specific environment
allowlist when the common allowlist is not sufficient.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

MODEL_VERSION = "tsfp-cache-key-v1"
ENV_ALLOWLIST = frozenset(
    {
        "CFLAGS",
        "CMAKE_BUILD_TYPE",
        "CMAKE_C_FLAGS",
        "CMAKE_CXX_FLAGS",
        "CXXFLAGS",
        "CPPFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_PROFILE",
        "CARGO_PROFILE_DEV_DEBUG",
        "CARGO_PROFILE_TEST_DEBUG",
        "CARGO_BUILD_TARGET",
        "RUSTFLAGS",
        "RUSTUP_TOOLCHAIN",
        "TSFP_C_OPT",
        "TSFP_CARGO_FEATURES",
    }
)
VOLATILE_ENV = frozenset(
    {
        "CARGO_BUILD_JOBS",
        "CARGO_INCREMENTAL",
        "CARGO_TARGET_DIR",
        "CARGO_HOME",
        "HOSTNAME",
        "PWD",
        "RUSTC_WRAPPER",
        "TSFP_BUILD_SLOT_HELD",
        "TSFP_BUILD_REMOTE",
        "TSFP_GIT_SHA",
        "TSFP_MERGE_URGENT",
        "TSFP_WT_FAMILY",
    }
)
DEFAULT_TOOLS = ("rustc", "cc", "cmake", "python3")
CACHE_NAMES = (
    "sccache-rustc",
    "sccache-c-cmake",
    "cargo-target-worktree",
    "cargo-target-canon-slot",
    "bevy-host-prebuilt",
    "lifted-cdiff-bundle",
    "tsfp-game-build-script",
    "build-verdict",
    "tier2-verify",
    "replay-capture",
    "gate-receipt",
    "pytest-reverify",
)


@dataclass(frozen=True)
class KeyResult:
    key: str
    components: dict[str, object]


def relative_path(root: Path, path: str | Path) -> str:
    """Return a stable repo-relative POSIX path; reject traversal and paths outside the tree."""
    base = root.resolve()
    candidate = Path(path)
    if candidate.is_absolute():
        candidate = candidate.resolve()
    else:
        candidate = (base / candidate).resolve()
    try:
        relative = candidate.relative_to(base)
    except ValueError as error:
        raise ValueError(f"cache input is outside repository: {path}") from error
    value = relative.as_posix()
    if value == "." or value.startswith("../"):
        raise ValueError(f"cache input is not a repository file: {path}")
    return value


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_components(root: Path, paths: Sequence[str | Path]) -> dict[str, str]:
    """Hash files using only normalized repo-relative names and streamed content."""
    components: dict[str, str] = {}
    for path in paths:
        relative = relative_path(root, path)
        candidate = root.resolve() / relative
        if not candidate.is_file():
            raise FileNotFoundError(f"cache input missing: {relative}")
        components[relative] = sha256_file(candidate)
    return dict(sorted(components.items()))


def named_file_components(files: Mapping[str, Path]) -> dict[str, str]:
    """Hash external/generated files under caller-provided logical names only."""
    clean: dict[str, str] = {}
    for name, path in files.items():
        logical = Path(name)
        if logical.is_absolute() or ".." in logical.parts or str(logical) in ("", "."):
            raise ValueError(f"invalid logical cache input name: {name}")
        clean[logical.as_posix()] = sha256_file(path)
    return dict(sorted(clean.items()))


def normalized_env(
    env: Mapping[str, str] | None = None, *, allow: Sequence[str] = ()
) -> dict[str, str]:
    """Keep only explicit output-affecting variables; exclude the documented volatile set."""
    values = os.environ if env is None else env
    selected = set(ENV_ALLOWLIST | frozenset(allow))
    # Cargo exposes profile overrides as CARGO_PROFILE_<NAME>_<SETTING>. Their values
    # affect generated artifacts, while incremental bookkeeping does not.
    selected.update(
        name
        for name in values
        if name.startswith("CARGO_PROFILE_")
        and not name.endswith("_INCREMENTAL")
        and name not in VOLATILE_ENV
    )
    return {name: values[name] for name in sorted(selected - VOLATILE_ENV) if name in values}


def linker_tool(env: Mapping[str, str]) -> str | None:
    """Return a logical linker name, never a host-specific executable path."""
    candidates = [value for name, value in env.items() if name.endswith("_LINKER")]
    candidates.extend((env.get("RUSTC_LINKER", ""), env.get("CC", "")))
    rustflags = " ".join(env.get(name, "") for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"))
    match = re.search(r"(?:^|[=, ])-fuse-ld=([^\s,]+)", rustflags)
    if match:
        candidates.insert(0, match.group(1))
    for value in candidates:
        if not value:
            continue
        try:
            executable = Path(shlex.split(value)[0]).name.lower()
        except (ValueError, IndexError):
            continue
        if "mold" in executable:
            return "mold"
        if "lld" in executable:
            return "ld.lld"
        if "gold" in executable:
            return "ld.gold"
        if executable in {"ld", "ld.bfd"}:
            return "ld"
    return None


UNAVAILABLE = "unavailable"  # a version probe that failed (non-zero exit, timeout, no output)

# Toolchain variables forwarded to a version probe so a rustup proxy resolves the right home.
PROBE_ENV = ("RUSTUP_HOME", "CARGO_HOME", "RUSTUP_TOOLCHAIN", "RUSTUP_AUTO_INSTALL")


TOOL_PROBE_TIMEOUT_S = 30
"""A rustup proxy answers `--version` in 5 s idle but not under owner-PC load 18+ (T3885 dry
run 2026-10-28: `rustc version unavailable`, cached for the process, failed prebuilt tests)."""


def _tool_version(
    executable: str, path_env: str, probe_env: tuple[tuple[str, str], ...] = ()
) -> str:
    """The cached probe, but a failed probe is never kept: one failing probe (a test that broke
    it, a timeout under load) left `rustc version unavailable` cached for every later caller in
    the process, and prebuilt tests failed in landings by test order (2026-10-28, I-T3885-9)."""
    value = _cached_tool_version(executable, path_env, probe_env)
    if value == UNAVAILABLE:
        _cached_tool_version.cache_clear()
    return value


@lru_cache(maxsize=32)
def _cached_tool_version(
    executable: str, path_env: str, probe_env: tuple[tuple[str, str], ...] = ()
) -> str:
    resolved = shutil.which(executable, path=path_env)
    if not resolved:
        return "missing"
    # T3836: run the probe under the caller's toolchain env, not the hook process's.
    run_env = {**os.environ, "PATH": path_env, **dict(probe_env)}
    try:
        result = subprocess.run(
            [resolved, "--version"],
            capture_output=True,
            text=True,
            timeout=TOOL_PROBE_TIMEOUT_S,
            check=False,
            env=run_env,
        )
    except (OSError, subprocess.TimeoutExpired):
        return UNAVAILABLE
    if result.returncode:  # e.g. a rustup proxy with no toolchain prints an install notice
        return UNAVAILABLE
    first = (result.stdout or result.stderr).splitlines()
    return first[0].strip() if first else f"exit-{result.returncode}"


_tool_version.cache_clear = _cached_tool_version.cache_clear  # type: ignore[attr-defined]


def tool_versions(
    root: Path,
    *,
    env: Mapping[str, str] | None = None,
    names: Sequence[str] = DEFAULT_TOOLS,
    include_pin: bool = True,
) -> dict[str, str]:
    """Capture tool versions and the checked-in Rust toolchain pin, without host paths."""
    values = os.environ if env is None else env
    path_env = values.get("PATH", "")
    probe_env = tuple((key, values[key]) for key in PROBE_ENV if key in values)
    versions = {name: _tool_version(name, path_env, probe_env) for name in names}
    selected_linker = linker_tool(values)
    if selected_linker:
        raw = _tool_version(selected_linker, path_env, probe_env)
        match = re.search(r"(?:^|\s)(\d+)(?:\.\d+)*", raw)
        versions["linker"] = (
            f"{selected_linker} major {match.group(1)}" if match else f"{selected_linker} {raw}"
        )
    if include_pin:
        pins = [
            (name, sha256_file(root / name))
            for name in ("rust-toolchain.toml", "rust-toolchain")
            if (root / name).is_file()
        ]
        versions["rust-toolchain-pin"] = hashlib.sha256(
            json.dumps(pins, separators=(",", ":")).encode()
        ).hexdigest()
    return dict(sorted(versions.items()))


def make_key(
    cache: str,
    *,
    files: Mapping[str, str],
    tools: Mapping[str, str],
    env: Mapping[str, str] | None = None,
    allow_env: Sequence[str] = (),
    extra: Mapping[str, object] | None = None,
) -> KeyResult:
    """Hash canonical named content, tools, allowlisted env and semantic cache options."""
    components: dict[str, object] = {
        "model": MODEL_VERSION,
        "cache": cache,
        "files": dict(sorted(files.items())),
        "tools": dict(sorted(tools.items())),
        "env": normalized_env(env, allow=allow_env),
        "extra": dict(sorted((extra or {}).items())),
    }
    payload = json.dumps(components, sort_keys=True, separators=(",", ":")).encode()
    return KeyResult(hashlib.sha256(payload).hexdigest(), components)


def explain_difference(left: Mapping[str, object], right: Mapping[str, object]) -> list[str]:
    """Report differing key components, descending into mappings for useful miss causes."""

    def walk(prefix: str, a: object, b: object) -> list[str]:
        if isinstance(a, Mapping) and isinstance(b, Mapping):
            out: list[str] = []
            for name in sorted(set(a) | set(b), key=str):
                label = f"{prefix}.{name}" if prefix else str(name)
                out.extend(walk(label, a.get(name), b.get(name)))
            return out
        return [] if a == b else [prefix or "<root>"]

    return walk("", left, right)


def check_cross_host(checkout: Path | None = None) -> list[str]:
    """Exercise the shared key contract for every registered cache in two fake layouts."""
    with tempfile.TemporaryDirectory(prefix="tsfp-cache-key-check-") as temp:
        base = Path(temp)
        roots = (base / "worker-one" / "repo", base / "worker-two" / "repo")
        envs = (
            {"TSFP_C_OPT": "2", "HOSTNAME": "host-a", "CARGO_BUILD_JOBS": "2"},
            {"TSFP_C_OPT": "2", "HOSTNAME": "host-b", "CARGO_BUILD_JOBS": "32"},
        )
        for root in roots:
            (root / "inputs").mkdir(parents=True)
            (root / "inputs/source.bin").write_bytes(b"same repository content\0")
        failures = []
        for name in CACHE_NAMES:
            keys = []
            for root, env in zip(roots, envs, strict=True):
                keys.append(
                    make_key(
                        name,
                        files=file_components(root, ["inputs/source.bin"]),
                        tools={tool: f"{tool} test-version" for tool in DEFAULT_TOOLS},
                        env=env,
                    ).key
                )
            if keys[0] != keys[1]:
                failures.append(name)
        failures.extend(check_production_adapters(checkout))
        return failures


def check_production_adapters(checkout: Path | None = None) -> list[str]:
    """Exercise production adapters that expose pure key functions across fake layouts.

    The synthetic family loop above protects the shared encoder. These checks additionally
    exercise representative consumers; cache families without a pure adapter remain covered
    only by the encoder until their key construction is factored into callable functions.
    """
    from types import SimpleNamespace
    from unittest.mock import patch

    from tools import adoption_gate, gate, gate_history, verify_all_tier2
    from tools.agents import verify_cache
    from tools.wt import cdiff_bundle, core, replay

    with tempfile.TemporaryDirectory(prefix="tsfp-cache-adapters-") as temp:
        base = Path(temp)
        roots = (base / "worker-a" / "repo", base / "worker-b" / "repo")
        for root in roots:
            (root / "bin").mkdir(parents=True)
            cdiff = root / "crates/tsfp-c-diff"
            cdiff.mkdir(parents=True)
            (cdiff / "build.rs").write_text("fn main() {}\n")
            lifted = root / "lifted-c"
            lifted.mkdir()
            (lifted / "unit.c").write_bytes(b"same lifted C input\0")
            (root / "recordings").mkdir()
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="stable"\n')
            (root / "bin/game").write_bytes(b"same host build")
            (root / "recordings/route.rec").write_bytes(b"same route recording")

        gate_spec = SimpleNamespace(
            switches=("rust-ports=inventory",),
            shared_env=("TSFP_TEST=1",),
            adopted=(),
            baseline_env=(),
            baseline_host_args=(),
            shared_host_args=(),
            wsola_activity=False,
            start=10,
            roots=(0x1234,),
            seconds=1,
            snapshot=False,
            segments=None,
            render="none",
            paced=False,
        )
        keys: dict[str, list[str]] = {
            "adapter-cargo-target-family": [],
            "adapter-gate-receipt": [],
            "adapter-gate-control-receipt": [],
            "adapter-gameplay-snapshot": [],
            "adapter-gate-history-identity": [],
            "adapter-verify-cache": [],
            "adapter-focused-worker-verify": [],
            "adapter-tier2-verification-result": [],
            "adapter-replay-capture": [],
            "adapter-cdiff-bundler": [],
            "adapter-cdiff-lifted-bundle": [],
        }
        for index, root in enumerate(roots):
            fake_env = {
                "PATH": os.environ.get("PATH", ""),
                "HOSTNAME": f"host-{index}",
                "CARGO_BUILD_JOBS": str(2 if index == 0 else 32),
                "CARGO_TARGET_DIR": str(root / "target"),
            }
            with patch.dict(os.environ, fake_env, clear=True):
                keys["adapter-cargo-target-family"].append(
                    core.cargo_family_key(root, "default", ["cargo", "build"])
                )
                keys["adapter-cdiff-bundler"].append(
                    cdiff_bundle._script_hash(root, root / "crates/tsfp-c-diff", fake_env)
                )
                keys["adapter-cdiff-lifted-bundle"].append(
                    cdiff_bundle.lift_hash(root / "lifted-c", root)
                )
            keys["adapter-gate-receipt"].append(
                gate.receipt_key(gate_spec, "input", "recording", "regions")
            )
            keys["adapter-gate-control-receipt"].append(
                gate.control_receipt_key(gate_spec, "input", "host-class", "recording", "regions")
            )
            keys["adapter-gameplay-snapshot"].append(
                adoption_gate.snapshot_key({"route": "route.rec", "poll": 100, "switches": ["off"]})
            )
            keys["adapter-gate-history-identity"].append(
                str(
                    gate_history.keys_from_fields(
                        gate_history.spec_fields(gate_spec, "recording", "regions")
                    )
                )
            )
            _, verify_key = verify_cache.key(
                "100644 blob same\tcrates/example/src/lib.rs",
                [f"cargo test --manifest-path {root}/Cargo.toml"],
                root,
                None,
            )
            keys["adapter-verify-cache"].append(verify_key)
            keys["adapter-focused-worker-verify"].append(
                verify_cache.focused_key(
                    "100644 blob same\tcrates/example/src/lib.rs",
                    "pytest tests/test_example.py",
                    None,
                    root,
                )[1]
            )
            keys["adapter-tier2-verification-result"].append(
                verify_all_tier2.verification_cache_key(
                    {
                        "route": "story-level1",
                        "recording_sha256": "recording-content",
                        "save_sha256": "save-content",
                        "prebuilt": "prebuilt-content",
                        "switch_set_sha256": "switches-content",
                        "regions_sha256": "regions-content",
                        "seconds": 10,
                    }
                )
            )
            replay_key, _ = replay.replay_key(
                root / "bin/game",
                "route",
                root / "recordings/route.rec",
                "rust-ports=inventory",
                display_backend="none",
                snapshot_attempt="simulated-attempt",
            )
            keys["adapter-replay-capture"].append(replay_key)
        failures = [name for name, values in keys.items() if values[0] != values[1]]

    # These adapters consume the real checkout tree or commit and cannot be safely reduced
    # to the tiny fake repos above. Run them twice with host/slot layout changes; placement
    # variables must not affect their identities.
    root = checkout if checkout is not None else Path(__file__).resolve().parents[2]
    host_envs = (
        {
            **os.environ,
            "HOSTNAME": "cache-key-host-a",
            "CARGO_BUILD_JOBS": "2",
            "CARGO_TARGET_DIR": str(root / "target-host-a"),
        },
        {
            **os.environ,
            "HOSTNAME": "cache-key-host-b",
            "CARGO_BUILD_JOBS": "32",
            "CARGO_TARGET_DIR": str(root / "target-host-b"),
        },
    )
    failures.extend(_check_real_checkout_adapters(root, host_envs))
    return failures


def _check_real_checkout_adapters(root: Path, host_envs: Sequence[Mapping[str, str]]) -> list[str]:
    """Check adapters whose inputs are the real checkout rather than the tiny fixture repos."""
    return _check_build_result_adapters(root, host_envs) + _check_prebuilt_adapter(root, host_envs)


def _check_build_result_adapters(root: Path, host_envs: Sequence[Mapping[str, str]]) -> list[str]:
    from tools.wt import build

    tree = build.snapshot_tree(root)
    failures: list[str] = []
    for label, crate_name in (
        ("adapter-build-result-leaf", "tsfp-types"),
        ("adapter-build-result-bevy", "tsfp-bevy"),
        ("adapter-build-result-game", "tsfp-game"),
    ):
        crate = root / "crates" / crate_name
        values = [
            build.input_key(
                root,
                crate,
                ["cargo", "check", "--manifest-path", str(crate / "Cargo.toml")],
                env,
                tree,
            )[0]
            for env in host_envs
        ]
        if values[0] != values[1]:
            failures.append(label)
    return failures


def _check_prebuilt_adapter(root: Path, host_envs: Sequence[Mapping[str, str]]) -> list[str]:
    from unittest.mock import patch

    from tools.wt import prebuilt

    prebuilt_values = []
    for env in host_envs:
        with patch.dict(os.environ, env, clear=True):
            prebuilt_values.append(prebuilt.input_hash(root, "HEAD"))
    return [] if prebuilt_values[0] == prebuilt_values[1] else ["adapter-prebuilt-input"]


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.wt cache-keys")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--why-miss", nargs=2, metavar=("RUN_A.json", "RUN_B.json"))
    args = parser.parse_args(argv)
    if args.check:
        failures = check_cross_host()
        if failures:
            print("cache-keys: cross-host mismatch: " + ", ".join(failures), file=sys.stderr)
            return 1
        print(f"cache-keys: cross-host equality passed ({len(CACHE_NAMES)} cache families)")
        return 0
    if args.why_miss:
        records = []
        descriptors = []
        for name in args.why_miss:
            path = Path(name)
            if path.stat().st_size > 1_000_000:
                parser.error(f"{name}: key descriptor exceeds 1 MB")
            value = json.loads(path.read_text(encoding="utf-8"))
            descriptors.append(value)
            records.append(
                value.get("components", value.get("key_components", value.get("inputs", value)))
            )
        differences = explain_difference(records[0], records[1])
        # Cache identity is often stored beside (rather than inside) the components.
        # Include it so two otherwise identical descriptors from different caches cannot
        # be misreported as an equal-key pair.
        if descriptors[0].get("cache") != descriptors[1].get("cache"):
            differences.insert(0, "cache")
        if descriptors[0].get("key") != descriptors[1].get("key"):
            differences.append("key")
        message = (
            "equal key components"
            if not differences
            else "differing components: " + ", ".join(differences)
        )
        print(message)
        return 0
    parser.error("choose --check or --why-miss RUN_A.json RUN_B.json")
