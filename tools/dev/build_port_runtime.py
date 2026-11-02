"""Build the standalone port runtime with Cargo and export its rustc dependencies."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


def build(
    root: Path,
    output_dir: Path,
    target: str,
    cargo: str,
    cfgs: list[str],
    opt_level: int,
    codegen_units: int,
) -> None:
    crate_manifest = root / "crates/tsfp-port-runtime/Cargo.toml"
    cargo_target = output_dir / ".port-runtime-cargo"
    cargo_target.mkdir(parents=True, exist_ok=True)
    command = [
        cargo,
        "rustc",
        "--manifest-path",
        str(crate_manifest),
        "--package",
        "tsfp-port-runtime",
        "--lib",
        "--release",
        "--target",
        target,
        "--message-format=json",
        "--",
        "-Cpanic=abort",
        "-Coverflow-checks=off",
        "-Crelocation-model=pic",
        f"-Copt-level={opt_level}",
        f"-Ccodegen-units={codegen_units}",
    ]
    if cfgs:
        command.extend(f"--cfg={cfg}" for cfg in cfgs)
    environment = {**os.environ, "CARGO_TARGET_DIR": str(cargo_target)}
    result = subprocess.run(command, cwd=root, env=environment, text=True, capture_output=True)
    artifact: Path | None = None
    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        if message.get("target", {}).get("name") != "tsfp_port_runtime":
            continue
        artifact = next(
            (Path(path) for path in message.get("filenames", []) if path.endswith(".rlib")),
            None,
        )
    if result.returncode or artifact is None:
        tail = "\n".join((result.stdout + result.stderr).splitlines()[-80:])
        raise RuntimeError(f"port runtime Cargo build failed (exit {result.returncode}):\n{tail}")

    output_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(artifact, output_dir / "libtsfp_port_runtime.rlib")
    # Cargo places the top-level crate artifact beside `deps/`, while its
    # registry dependencies (such as `spin`) live inside `deps/`. The CMake
    # bare-rustc consumers resolve both from output_dir with `-L dependency`.
    copied = 0
    for dependency_dir in (artifact.parent, artifact.parent / "deps"):
        if not dependency_dir.is_dir():
            continue
        for dependency in dependency_dir.glob("*.rlib"):
            shutil.copy2(dependency, output_dir / dependency.name)
            copied += "tsfp_port_runtime" not in dependency.name
    if not copied:  # T3765 (I-T3765-1): a runtime without spin breaks every rustc consumer
        raise RuntimeError(f"port runtime dependencies (spin) not found beside {artifact}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--cfg", action="append", default=[])
    parser.add_argument("--opt-level", type=int, default=3)
    parser.add_argument("--codegen-units", type=int, default=256)
    args = parser.parse_args()
    try:
        build(
            args.root.resolve(),
            args.output_dir.resolve(),
            args.target,
            args.cargo,
            args.cfg,
            args.opt_level,
            args.codegen_units,
        )
    except (OSError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
