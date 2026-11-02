#!/usr/bin/env python3
# ruff: noqa: E501
"""T3605: time the host's direct-rustc Rust ports build (cmake/rust-ports.cmake) outside CMake.

The hybrid host does not build `tsfp-ports` with cargo. CMake runs `rustc` directly for
`tsfp-port-runtime` (rlib), the C2R family crates (rlibs, in parallel) and then the `tsfp-ports`
staticlib (the no_std archive that is linked into the host). This script repeats exactly those
invocations (same flags as cmake/rust-ports.cmake) in a scratch directory so the wall and CPU of each
step can be measured on any machine without the private lift, and so a one-line edit can be rebuilt
incrementally or with a cold output dir.

    python3 tools/dev/time_ports_build.py --src . --out ../ports-out --jobs 8
    python3 tools/dev/time_ports_build.py --src . --out ../ports-out --edit crates/tsfp-ports/src/weapons_lane.rs
    python3 tools/dev/time_ports_build.py --src . --out ../ports-out --time-passes   # rustc -Z time-passes (RUSTC_BOOTSTRAP)

Output: one JSON line per step on stdout (`{"step": ..., "wall_s": ..., "cpu_s": ...}`), a summary table
at the end. Nothing is written into the source tree except the `--edit` comment line, which is removed
again (the file is restored byte for byte).
"""

from __future__ import annotations

import argparse
import json
import os
import resource
import shutil
import subprocess
import sys
import time
import tomllib
from concurrent.futures import Future, ThreadPoolExecutor
from pathlib import Path

if not __package__:  # run as `python3 tools/dev/time_ports_build.py`
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools import hostcaps  # noqa: E402

TARGET = "x86_64-unknown-linux-gnu"


def rustc_base(opt_level: str, cgus: int, incremental: Path | None, extra: list[str]) -> list[str]:
    cmd = [
        "rustc",
        "--crate-type",
        "rlib",
        "--edition=2024",
        "--target",
        TARGET,
        "-Cpanic=abort",
        "-Coverflow-checks=off",
        "-Crelocation-model=pic",
        f"-Copt-level={opt_level}",
        f"-Ccodegen-units={cgus}",
    ]
    if incremental is not None:
        cmd.append(f"-Cincremental={incremental}")
    cmd += extra
    return cmd


def run_step(
    name: str, cmd: list[str], cwd: Path, env: dict[str, str], log_dir: Path
) -> dict[str, object]:
    """Run one rustc, return wall and the CPU time of the child (user + sys) from rusage deltas."""
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    log = (log_dir / f"{name}.log").open("w")
    proc = subprocess.run(cmd, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT, check=False)
    wall = time.monotonic() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime)
    return {"step": name, "rc": proc.returncode, "wall_s": round(wall, 2), "cpu_s": round(cpu, 2)}


def locked_spin_source(src: Path, env: dict[str, str]) -> Path:
    """Find the exact registry source used by CMake for tsfp-port-runtime's spin dependency."""
    lock = tomllib.loads((src / "Cargo.lock").read_text())
    version = next(
        str(package["version"]) for package in lock["package"] if package["name"] == "spin"
    )
    homes = [
        env.get("CARGO_HOME", ""),
        env.get("TSFP_BEVY_CARGO_HOME", ""),
        str(src / "tmp/rust/cargo"),
        str(Path.home() / ".cargo"),
    ]
    for home in filter(None, homes):
        matches = sorted(Path(home).glob(f"registry/src/*/spin-{version}/src/lib.rs"))
        if matches:
            return matches[0]
    raise FileNotFoundError(f"spin {version} source from Cargo.lock is absent from cargo homes")


def build_all(args: argparse.Namespace, label: str) -> list[dict[str, object]]:
    src: Path = args.src.resolve()
    out: Path = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    log_dir = out / "logs" / label
    log_dir.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    if args.time_passes:
        env["RUSTC_BOOTSTRAP"] = "1"
    extra_flags = ["-Ztime-passes"] if args.time_passes else []
    inc = (
        (lambda name: out / f"rustc-{name}-incremental")
        if args.incremental
        else (lambda name: None)
    )
    families = (
        json.loads((src / "tools/data/c2r_family_crates.json").read_text())["families"]
        if (src / "tools/data/c2r_family_crates.json").exists()
        else json.loads(args.families.read_text())["families"]
    )
    steps: list[dict[str, object]] = []

    spin = out / "libspin.rlib"
    spin_cmd = [
        "rustc",
        str(locked_spin_source(src, env)),
        "--crate-name",
        "spin",
        "--crate-type",
        "rlib",
        "--edition=2015",
        "--cap-lints",
        "allow",
        '--cfg=feature="mutex"',
        '--cfg=feature="spin_mutex"',
        "--target",
        TARGET,
        "-Cpanic=abort",
        "-Coverflow-checks=off",
        "-Crelocation-model=pic",
        f"-Copt-level={args.opt_level}",
        "-o",
        str(spin),
    ]
    if not args.skip_families:
        steps.append(run_step("spin", spin_cmd, src, env, log_dir))

    runtime = out / "libtsfp_port_runtime.rlib"
    cmd = [
        sys.executable,
        str(src / "tools/dev/build_port_runtime.py"),
        "--root",
        str(src),
        "--output-dir",
        str(out),
        "--target",
        "x86_64-unknown-linux-gnu",
        "--opt-level",
        str(args.opt_level),
        "--codegen-units",
        str(args.cgus),
    ]
    if args.skip_families:
        steps.append({"step": "runtime", "rc": 0, "wall_s": 0.0, "cpu_s": 0.0})
    else:
        steps.append(run_step("runtime", cmd, src, env, log_dir))

    # T3610: tsfp-port-wire sits between the runtime and the families that hold a wiring-layer user (config key "wire");
    # like the ninja graph of cmake/rust-ports.cmake, only those families wait for it.
    wire_dir = src / "crates/tsfp-port-wire"
    wire = out / "libtsfp_port_wire.rlib"
    have_wire = (wire_dir / "src/lib.rs").exists()

    def build_wire() -> dict[str, object]:
        cmd = rustc_base(args.opt_level, args.cgus, inc("port-wire"), extra_flags) + [
            str(wire_dir / "src/lib.rs"),
            "--crate-name",
            "tsfp_port_wire",
            "--extern",
            f"tsfp_port_runtime={runtime}",
            "-L",
            f"dependency={out}",
            "-o",
            str(wire),
        ]
        return run_step("wire", cmd, src, env, log_dir)

    family_packages = {str(fam["package"]) for fam in families}
    family_dependencies: dict[str, list[str]] = {}
    family_by_package = {str(fam["package"]): fam for fam in families}
    for package in family_by_package:
        manifest = tomllib.loads((src / "crates" / package / "Cargo.toml").read_text())
        dependencies = manifest.get("dependencies", {})
        siblings = []
        for _dependency, spec in dependencies.items():
            if isinstance(spec, dict) and "path" in spec:
                sibling = Path(str(spec["path"])).name
                if sibling in family_packages:
                    siblings.append(sibling)
        family_dependencies[package] = sorted(set(siblings))

    family_futures: dict[str, Future] = {}

    def family(fam: dict[str, object], wire_done: Future | None) -> dict[str, object]:
        package = str(fam["package"])
        crate = package.replace("-", "_")
        needs_wire = bool(fam.get("wire")) and have_wire
        if needs_wire and wire_done is not None:
            wire_done.result()
        dependency_args: list[str] = []
        for dependency in family_dependencies[package]:
            dependency_crate = str(dependency).replace("-", "_")
            family_futures[str(dependency)].result()
            dependency_args += [
                "--extern",
                f"{dependency_crate}={out / f'lib{dependency_crate}.rlib'}",
            ]
        cmd = rustc_base(args.opt_level, args.cgus, inc(package), extra_flags) + [
            str(src / "crates" / package / "src/lib.rs"),
            "--crate-name",
            crate,
            "--extern",
            f"tsfp_port_runtime={runtime}",
            *(["--extern", f"tsfp_port_wire={wire}"] if needs_wire else []),
            *dependency_args,
            "-L",
            f"dependency={out}",
            "-o",
            str(out / f"lib{crate}.rlib"),
        ]
        return run_step(package, cmd, src, env, log_dir)

    start = time.monotonic()
    if args.skip_families:
        family_steps = []
    else:
        # Schedule sibling leaves before their dependents and wire users after the wire build.
        order = sorted(family_by_package.values(), key=lambda fam: bool(fam.get("wire")))
        ordered_packages: list[str] = []
        visited: set[str] = set()
        visiting: set[str] = set()

        def visit(package: str) -> None:
            if package in visited:
                return
            if package in visiting:
                raise ValueError(f"family path dependency cycle includes {package}")
            visiting.add(package)
            for dependency in family_dependencies[package]:
                visit(dependency)
            visiting.remove(package)
            visited.add(package)
            ordered_packages.append(package)

        for fam in order:
            visit(str(fam["package"]))
        with ThreadPoolExecutor(max_workers=args.jobs + 1) as pool:
            wire_done = pool.submit(build_wire) if have_wire else None
            for package in ordered_packages:
                family_futures[package] = pool.submit(family, family_by_package[package], wire_done)
            family_steps = [future.result() for future in family_futures.values()]
            if wire_done is not None:
                steps.append(wire_done.result())
    steps += family_steps
    steps.append(
        {
            "step": "families-total",
            "wall_s": round(time.monotonic() - start, 2),
            "cpu_s": round(sum(float(s["cpu_s"]) for s in family_steps), 2),
        }
    )

    # T3398: this dedicated archive dependency is outside c2r_family_crates.json, but the
    # ports staticlib references it directly. Mirror cmake/rust-ports.cmake so the timing
    # build validates and measures the same link graph instead of failing with E0463.
    player_interaction = out / "libtsfp_player_interaction_fast_path.rlib"
    player_interaction_cmd = rustc_base(
        args.opt_level, args.cgus, inc("player-interaction-fast-path"), extra_flags
    ) + [
        str(src / "crates/tsfp-player-interaction-fast-path/src/lib.rs"),
        "--crate-name",
        "tsfp_player_interaction_fast_path",
        "--extern",
        f"tsfp_port_runtime={runtime}",
        "-L",
        f"dependency={out}",
        "-o",
        str(player_interaction),
    ]
    steps.append(
        run_step("player-interaction-fast-path", player_interaction_cmd, src, env, log_dir)
    )

    # These standalone leaves are linked by the CMake archive but intentionally are not
    # C2R families. Keep the timing build's extern closure in sync with rust-ports.cmake.
    standalone: dict[str, Path] = {}
    standalone_specs = [
        ("tsfp_weapon_realization_protocol", "tsfp-weapon-realization-protocol", []),
        (
            "tsfp_weapon_owner",
            "tsfp-weapon-owner",
            ["tsfp_port_wire", "tsfp_weapon_realization_protocol"],
        ),
        (
            "tsfp_weapon_realization",
            "tsfp-weapon-realization",
            ["tsfp_port_wire", "tsfp_weapon_realization_protocol", "tsfp_weapon_owner"],
        ),
        ("tsfp_ai_spawn_binding", "tsfp-ai-spawn-binding", ["tsfp_port_runtime"]),
        ("tsfp_ai_selector_transition", "tsfp-ai-selector-transition", ["tsfp_port_runtime"]),
        ("lighting_t2", "tsfp-port-lighting-t2-div-lighting-1", []),
    ]
    for crate, package, dependencies in standalone_specs:
        artifact = out / f"lib{crate}.rlib"
        source = src / "crates" / package / "src/lib.rs"
        # rustc expects each --extern flag and its value as separate argv entries.
        command = rustc_base(args.opt_level, args.cgus, inc(package), extra_flags) + [
            str(source),
            "--crate-name",
            crate,
        ]
        for dependency in dependencies:
            command += [
                "--extern",
                f"{dependency}={standalone.get(dependency, out / f'lib{dependency}.rlib')}",
            ]
        command += ["-L", f"dependency={out}", "-o", str(artifact)]
        steps.append(run_step(crate, command, src, env, log_dir))
        standalone[crate] = artifact

    externs: list[str] = []
    for fam in families:
        crate = str(fam["package"]).replace("-", "_")
        externs += ["--extern", f"{crate}={out / f'lib{crate}.rlib'}"]
    # T3765: lane roots are built in a separate crate after the family crates and
    # linked by both the archive and the CMake build. Keep this edge in the timer too.
    lane_c2r = out / "libtsfp_lane_c2r.rlib"
    lane_c2r_cmd = rustc_base(args.opt_level, args.cgus, inc("lane-c2r"), extra_flags) + [
        str(src / "crates/tsfp-lane-c2r/src/lib.rs"),
        "--crate-name",
        "tsfp_lane_c2r",
        "--extern",
        f"tsfp_port_runtime={runtime}",
        *externs,
        "-L",
        f"dependency={out}",
        "-o",
        str(lane_c2r),
    ]
    steps.append(run_step("lane-c2r", lane_c2r_cmd, src, env, log_dir))
    cmd = rustc_base(args.opt_level, args.cgus, inc("ports"), extra_flags)
    cmd[cmd.index("rlib")] = "staticlib"
    cmd += args.ports_flag
    cmd += [
        str(src / "crates/tsfp-ports/src/lib.rs"),
        "--crate-name",
        "tsfp_ports",
        "--extern",
        f"tsfp_port_runtime={runtime}",
        *(["--extern", f"tsfp_port_wire={wire}"] if have_wire else []),
        "--extern",
        f"tsfp_player_interaction_fast_path={player_interaction}",
        "--extern",
        f"tsfp_lane_c2r={lane_c2r}",
        *[
            item
            for crate, artifact in standalone.items()
            if crate != "tsfp_weapon_realization_protocol"
            for item in ("--extern", f"{crate}={artifact}")
        ],
        *externs,
        "-L",
        f"dependency={out}",
        "-o",
        str(out / "libtsfp_ports.a"),
    ]
    steps.append(run_step("ports", cmd, src, env, log_dir))
    for step in steps:
        step["label"] = label
        print(json.dumps(step), flush=True)
    return steps


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--src", type=Path, default=Path("."), help="repo root (default: current directory)"
    )
    parser.add_argument(
        "--out", type=Path, required=True, help="scratch output directory (created)"
    )
    parser.add_argument("--families", type=Path, default=Path("tools/data/c2r_family_crates.json"))
    parser.add_argument(
        "--jobs",
        type=int,
        default=hostcaps.effective_cpus(),
        help="parallel family rustc processes",
    )
    parser.add_argument("--opt-level", default="3")
    parser.add_argument("--cgus", type=int, default=256, help="-Ccodegen-units (CMake uses 256)")
    parser.add_argument(
        "--incremental", action="store_true", help="pass -Cincremental like CMake does"
    )
    parser.add_argument(
        "--time-passes", action="store_true", help="rustc -Ztime-passes (RUSTC_BOOTSTRAP=1)"
    )
    parser.add_argument("--clean", action="store_true", help="remove the out dir first")
    parser.add_argument(
        "--edit",
        type=Path,
        action="append",
        default=[],
        help="after the first build append a comment line to this file and rebuild (restored afterwards)",
    )
    parser.add_argument("--only-cold", action="store_true", help="skip the no-op rebuild")
    parser.add_argument(
        "--ports-flag",
        action="append",
        default=[],
        help="extra rustc flag for the tsfp_ports staticlib only (repeatable), for experiments",
    )
    parser.add_argument(
        "--skip-families",
        action="store_true",
        help="reuse the family rlibs already in --out (experiments on the ports step)",
    )
    args = parser.parse_args()

    if args.clean and args.out.exists():
        shutil.rmtree(args.out)
    all_steps = build_all(args, "cold")
    if not args.only_cold:
        all_steps += build_all(args, "noop")
    for edit in args.edit:
        path = (args.src / edit).resolve()
        original = path.read_bytes()
        try:
            path.write_bytes(original + f"\n// time_ports_build edit {time.time_ns()}\n".encode())
            all_steps += build_all(args, f"edit:{edit}")
        finally:
            path.write_bytes(original)
    print()
    print(f"{'label':<44} {'step':<34} {'wall_s':>8} {'cpu_s':>8}")
    for step in all_steps:
        if step["step"] in ("runtime", "wire", "families-total", "lane-c2r", "ports"):
            print(f"{step['label']:<44} {step['step']:<34} {step['wall_s']:>8} {step['cpu_s']:>8}")
    return 0 if all(step.get("rc", 0) == 0 for step in all_steps) else 1


if __name__ == "__main__":
    sys.exit(main())
