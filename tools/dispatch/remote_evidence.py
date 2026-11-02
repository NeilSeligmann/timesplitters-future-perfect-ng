"""Bounded execution evidence for conditional remote offload, not another scheduler.

Receipts identify an execution environment and command family across logical job IDs.
Only a real passing run enables automatic offload; infrastructure failures back off that
family without turning compiler/test failures into infrastructure verdicts.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import time
from pathlib import Path
from typing import Any
from uuid import uuid4

PASS_TTL_S = 86400.0
BACKOFF_S = 900.0
MAX_BACKOFF_S = 21600.0


def directory(base: Path | None = None) -> Path:
    if base is None:
        from tools.dispatch.queue import state_dir

        base = state_dir()
    return base / "remote-evidence"


def digest(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, default=str).encode()).hexdigest()


def command_key(commands: list[str], root: Path) -> str:
    text = "\n".join(commands)
    cargo = bool(re.search(r"\bcargo\s", text))
    manifests = sorted(set(re.findall(r"--manifest-path(?:=|\s+)[\"']?([^\s\"';]+)", text)))
    files = ["rust-toolchain", "rust-toolchain.toml", ".cargo/config", ".cargo/config.toml"]
    if cargo:
        # -p selectors/path dependencies can change native features without changing root Cargo.toml.
        files.extend(
            str(path.relative_to(root)) for path in sorted(root.glob("crates/*/Cargo.toml"))
        )
    for manifest in manifests or (["Cargo.toml"] if cargo else []):
        files.extend([manifest, str(Path(manifest).with_name("Cargo.lock"))])
    inputs = {}
    for name in files:
        path = (root / name).resolve()
        if not path.is_relative_to(root.resolve()):
            return ""  # a non-portable external input cannot be automatically validated
        try:
            inputs[name] = hashlib.sha256(path.read_bytes()).hexdigest()
        except FileNotFoundError:
            inputs[name] = None
    # Exact command shapes deliberately keep features/targets/config/test filters distinct.
    # A formatting pass or default-feature check cannot authorize another build shape.
    environment = {
        key: value
        for key, value in os.environ.items()
        if key
        in {
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTUP_TOOLCHAIN",
            "CARGO_BUILD_TARGET",
            "CC",
            "CXX",
            "PKG_CONFIG_PATH",
            "PKG_CONFIG_LIBDIR",
            "PKG_CONFIG_SYSROOT_DIR",
            "LIBCLANG_PATH",
        }
    }
    return digest(
        {"commands": commands, "manifests": manifests, "inputs": inputs, "env": environment}
    )


def environment_key(host: Any) -> str:
    return digest(
        {
            key: getattr(host, key, None)
            for key in (
                "name",
                "toolchain_dir",
                "canon_root",
                "target_dir",
                "host_venv",
                "env",
                "sccache",
                "max_jobs",
            )
        }
    )


def read(path: Path) -> dict:
    try:
        value = json.loads(path.read_text())
        return value if isinstance(value, dict) else {}
    except (OSError, ValueError):
        return {}


def write(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    temporary = path.with_name(f".{path.name}.{uuid4().hex}.tmp")
    try:
        with os.fdopen(os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as out:
            json.dump(value, out, sort_keys=True)
            out.write("\n")
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)
    owned = [
        p
        for p in path.parent.glob("*.json")
        if re.fullmatch(r"(?:[0-9a-f]{64}\.[0-9a-f]{64}|duration-[0-9a-f]{64})\.json", p.name)
    ]
    if len(owned) > 512:
        for old in sorted(owned, key=lambda item: item.stat().st_mtime, reverse=True)[512:]:
            old.unlink(missing_ok=True)  # bounded derived receipts only, never logs/owner inputs


def estimate(key: str, base: Path | None = None) -> float:
    if not key:
        return 0.0
    value = read(directory(base) / f"duration-{key}.json").get("seconds", 0)
    return max(0.0, float(value)) if isinstance(value, (float, int)) else 0.0


def record_duration(key: str, seconds: float, base: Path | None = None) -> None:
    if key:
        try:
            write(directory(base) / f"duration-{key}.json", {"seconds": round(seconds, 3)})
        except OSError:
            pass  # optional routing telemetry must not change a successful command verdict


def validated_keys(host: Any, base: Path | None = None, now: float | None = None) -> frozenset[str]:
    now = time.time() if now is None else now
    env = environment_key(host)
    found = set()
    for path in directory(base).glob(f"{env}.*.json"):
        row = read(path)
        passed = row.get("last_pass", 0)
        retry = row.get("retry_after", 0)
        if (
            isinstance(passed, (int, float))
            and passed > 0
            and now - passed <= PASS_TTL_S
            and isinstance(retry, (int, float))
            and retry <= now
            and row.get("environment") == env
            and isinstance(row.get("key"), str)
        ):
            found.add(row["key"])
    return frozenset(found)


def diagnostic(log: str) -> str:
    lines = [
        line.strip()
        for line in log.splitlines()
        if line.strip()
        and (
            not line.startswith("@@")
            or line.startswith(("@@FETCH_FAILED", "@@LOCK_", "@@HOST_HEALTH"))
        )
    ]
    actionable = next(
        (
            line[:400]
            for line in lines
            if re.search(
                r"not found|No such file|not installed|pkg-config exited|@@FETCH_FAILED", line, re.I
            )
        ),
        None,
    )
    if actionable:
        return actionable
    return next(
        (
            line[:400]
            for line in lines
            if re.search(
                r"error:|not found|No such file|not installed|failed to|pkg-config", line, re.I
            )
        ),
        " / ".join(lines[-2:])[:400],
    )


def record_result(
    host: Any,
    root: Path,
    commands: list[str],
    result: Any,
    *,
    infra: bool,
    base: Path | None = None,
    now: float | None = None,
) -> str | None:
    # Unit tests must opt into an isolated evidence store, never the live shared ledger.
    if (
        os.environ.get("PYTEST_CURRENT_TEST")
        and base is None
        and not os.environ.get("TSFP_DISPATCH_DIR")
    ):
        return None
    key = command_key(commands, root)
    if not key:
        return None
    now = time.time() if now is None else now
    env = environment_key(host)
    path = directory(base) / f"{env}.{key}.json"
    prior = read(path)
    from tools.wt.remote_fallback import expects_tests

    complete = len(result.commands) == len(commands) and all(
        item.returncode == 0 and (not expects_tests(item.command) or (item.passed_tests or 0) > 0)
        for item in result.commands
    )
    passed = result.returncode == 0 and complete
    previous_failures = prior.get("failures", 0)
    failures = (
        min(6, previous_failures if isinstance(previous_failures, int) else 0) + 1 if infra else 0
    )
    row = {
        "key": key,
        "environment": env,
        "host": host.name,
        "at": now,
        "last_pass": now if passed else prior.get("last_pass", 0),
        "retry_after": now + min(MAX_BACKOFF_S, BACKOFF_S * 2 ** (failures - 1)) if infra else 0,
        "failures": failures,
        "status": "pass" if passed else "infra" if infra else "candidate-fail",
        "commands": [command[:2000] for command in commands],
        "tree": result.tree,
        "returncode": result.returncode,
        "seconds": result.seconds,
        "transfer_seconds": result.transfer_seconds,
        "diagnostic": diagnostic(result.log),
        "log": result.log[:4000]
        + ("\n... bounded tail ...\n" + result.log[-12000:] if len(result.log) > 4000 else ""),
    }
    write(path, row)
    return str(path)
