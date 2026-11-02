# SPDX-License-Identifier: GPL-3.0-or-later
# ruff: noqa: E501  (Rust and TOML templates keep their generated line layout)
"""T3433: mechanical, deterministic split of self-contained C2R modules out of crates/tsfp-ports.

A module of crates/tsfp-ports/src/c2r moves to a per-subsystem family crate (`tsfp-port-<subsystem>`)
when it is self-contained: it only names `crate::abi`, `crate::memory`, `crate::state_wire` and `super::rt`
(T3605: `state_wire` and the `fs_base` global are hosted once in tsfp-port-runtime, so a family crate no longer
duplicates their process statics), and no code path-includes its file. Its subsystem is the majority subsystem of its ROOTS
in tools/data/function_subsystems.csv. The move follows the T2399 family-crate pattern:

* the source moves byte-identically to crates/<package>/src/c2r/<module>.rs,
* tools/data/c2r_family_crates.json records it (tools.c2rust, coverage, no_std archive, ledger
  and the Tier 1 harness all read that file),
* `c2r/mod.rs` re-exports the module, so every `crate::c2r::<module>` path, C ABI symbol and
  feature of the facade is unchanged; no source stub is left in the frozen facade crate,
* new family crates are created from a template, listed in the root `exclude` and as a
  dependency of tsfp-ports.

Re-running on any main commit gives the same result for the same inputs. Design and measurements:
docs/t3433-ports-crate-split.md.

    python3 -m tools.dev.split_ports --plan          # print the grouping, change nothing
    python3 -m tools.dev.split_ports --apply         # move the modules
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

from tools import crate_size

# T3605/T3606: a family crate takes modules only while it stays under the soft crate limit of tools.crate_size.
DEFAULT_CRATE_LINES = crate_size.DEFAULT_LIMITS["crate_soft"]
PORTS = Path("crates/tsfp-ports")
C2R = PORTS / "src/c2r"
FAMILIES = Path("tools/data/c2r_family_crates.json")
SUBSYSTEMS = Path("tools/data/function_subsystems.csv")
# T3605: `crate::state_wire` (hosted once in tsfp-port-runtime since T3484) and `fs_base` (moved there by T3605) are
# shared, so they no longer keep a module in the facade; the family lib re-exports state_wire.
# T3610: the wiring layer (`leaf_wire`, `player_wire`, ...) is hosted once in tsfp-port-wire below the family crates, so its users
# are movable too; a family that receives one depends on tsfp-port-wire and re-exports these modules at the old `crate::` paths.
WIRE_MODULES = (
    "contract_views",
    "hud_metrics",
    "leaf_wire",
    "menu_state",
    "missions_system",
    "missions_wire",
    "player_wire",
    "views",
)
WIRE_PACKAGE = "tsfp-port-wire"
WIRE_REFS = {f"crate::{module}" for module in WIRE_MODULES}
ALLOWED_REFS = {"crate::abi", "crate::memory", "crate::state_wire", "super::rt"} | WIRE_REFS
REF = re.compile(r"\b(?:crate|super)::[A-Za-z_0-9]+")
ROOTS = re.compile(r"pub const ROOTS: \[u32; \d+\] = \[([^\]]*)\]")
CODE_REF = re.compile(r"#\[path|Path\(|root /|read_text|write_text|include")


def git_grep(root: Path, pattern: str) -> list[str]:
    result = subprocess.run(
        ["git", "grep", "-nE", pattern, "--", "crates", "tools", "tests"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout.splitlines()


def load_subsystems(path: Path) -> dict[int, str]:
    with path.open() as handle:
        return {int(row["address"], 16): row["subsystem"] for row in csv.DictReader(handle)}


def package_for(subsystem: str) -> str:
    # T3605: roots without a row in function_subsystems.csv ("unknown") go to a neutral crate name.
    if subsystem == "unknown":
        subsystem = "misc"
    return "tsfp-port-" + re.sub(r"[^a-z0-9]+", "-", subsystem.lower()).strip("-")


def candidates(root: Path, split: set[str]) -> tuple[list[str], dict[str, str]]:
    """Self-contained modules and the reason each other module stays in the facade."""
    movable, kept = [], {}
    pathrefs = {}
    for line in git_grep(root, r"c2r/[a-z0-9_]+\.rs"):
        if CODE_REF.search(line):
            for name in re.findall(r"c2r/([a-z0-9_]+)\.rs", line):
                pathrefs.setdefault(name, line.split(":", 1)[0])
    for source in sorted((root / C2R).glob("*.rs")):
        module = source.stem
        if module in ("mod", "rt") or module in split:
            continue
        text = source.read_text()
        refs = set(REF.findall(text)) - ALLOWED_REFS
        if refs:
            kept[module] = "uses " + ", ".join(sorted(refs))
        elif module in pathrefs:
            kept[module] = f"path-included by {pathrefs[module]}"
        elif not ROOTS.search(text):
            kept[module] = "no ROOTS table"
        else:
            movable.append(module)
    return movable, kept


def uses_wire(text: str) -> bool:
    """Whether a batch names one of the tsfp-port-wire modules (its family then depends on tsfp-port-wire)."""
    return bool(WIRE_REFS & set(REF.findall(text)))


def module_lines(root: Path, module: str) -> int:
    """Non-test line count of a facade batch, the unit of tools.crate_size (AGENTS.md rule 31)."""
    return crate_size.count_lines((root / C2R / f"{module}.rs").read_bytes())


def pick_package(
    base: str, lines: int, sizes: dict[str, int], planned: dict[str, int], limit: int
) -> str:
    """First-fit: `base`, `base-2`, `base-3`, ... Existing crates keep within `limit` lines (T3606: baselined
    crates must never grow, new ones should stay under the soft limit); a module larger than the limit gets
    a crate of its own."""
    index = 1
    while True:
        package = base if index == 1 else f"{base}-{index}"
        current = sizes.get(package, 0) + planned.get(package, 0)
        if current == 0 or (current < limit and current + lines <= limit):
            return package
        index += 1


def plan(
    root: Path, subsystems: dict[int, str], crate_lines: int = DEFAULT_CRATE_LINES
) -> tuple[dict[str, tuple[str, list[str]]], dict[str, str]]:
    """Packages to receive modules ({package: (subsystem, [module])}) and why the rest stays."""
    config = json.loads((root / FAMILIES).read_text())
    split = {module for family in config["families"] for module in family["modules"]}
    movable, kept = candidates(root, split)
    sizes = crate_size.scan(root)[0]
    planned: dict[str, int] = {}
    groups: dict[str, tuple[str, list[str]]] = {}
    for module in movable:
        text = (root / C2R / f"{module}.rs").read_text()
        roots = [int(v, 16) for v in re.findall(r"0x[0-9a-fA-F]+", ROOTS.search(text)[1])]
        votes = Counter(subsystems.get(va, "unknown") for va in roots)
        # Deterministic: most roots, then name.
        subsystem = sorted(votes.items(), key=lambda item: (-item[1], item[0]))[0][0]
        lines = module_lines(root, module)
        package = pick_package(package_for(subsystem), lines, sizes, planned, crate_lines)
        planned[package] = planned.get(package, 0) + lines
        groups.setdefault(package, (subsystem, []))[1].append(module)
    return groups, kept


CARGO_TEMPLATE = """# SPDX-License-Identifier: GPL-3.0-or-later

[package]
name = "{package}"
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-or-later"

[dependencies]
tsfp-port-runtime = {{ path = "../tsfp-port-runtime" }}

[workspace]

[profile.dev]
debug = "line-tables-only"
debug-assertions = true
overflow-checks = true

[profile.dev.package."*"]
debug = false

# T2454: cfgs set only by tools (coverage and MMX harnesses, lane mutation); declared so rustc does not warn at every use.
[lints.rust]
unexpected_cfgs = {{ level = "warn", check-cfg = ["cfg(tsfp_cover)", "cfg(rust_lane_mutation)", "cfg(tsfp_mmx_harness)", "cfg(s3_missions_owned_differential)"] }}
"""

LIB_TEMPLATE = """// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: {subsystem} C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
//! tsfp-ports re-exports every module at its old `crate::c2r::<module>` path.
#![no_std]
#![allow(unexpected_cfgs)]

#[cfg(test)]
extern crate std;

pub mod abi {{
    pub use tsfp_port_runtime::abi::*;
}}

pub mod memory {{
    pub use tsfp_port_runtime::memory::*;
}}

pub use tsfp_port_runtime::state_wire;

pub mod c2r;
"""

C2R_MOD_TEMPLATE = """// SPDX-License-Identifier: GPL-3.0-or-later
// T2454: machine-translated code keeps the original shape (dead stores, unreachable arms); these lint classes are expected.
#![allow(
    clippy::all,
    dead_code,
    unused_assignments,
    unused_mut,
    unused_parens,
    unused_variables,
    unused_labels,
    unused_comparisons,
    unreachable_code,
    unreachable_patterns
)]
#[path = "../../../tsfp-ports/src/c2r/rt.rs"]
pub mod rt;
"""

TEST_TEMPLATE = """
#[cfg(test)]
mod tests {{
    #[test]
    fn split_modules_keep_their_roots() {{
{checks}
    }}
}}
"""


def render_config(config: dict) -> str:
    """tools/data/c2r_family_crates.json in its hand-written layout (one module list per line)."""
    out = ["{", f'  "schema": {config["schema"]},', '  "families": [']
    for index, family in enumerate(config["families"]):
        out += [
            "    {",
            f'      "subsystem": {json.dumps(family["subsystem"])},',
            f'      "package": {json.dumps(family["package"])},',
            f'      "modules": {json.dumps(family["modules"])}'
            + ("," if family.get("wire") else ""),
            *(['      "wire": true'] if family.get("wire") else []),
            "    }" + ("," if index + 1 < len(config["families"]) else ""),
        ]
    out.append("  ]" + ("," if "facade_shadow_modules" in config else ""))
    if "facade_shadow_modules" in config:
        # T3433: moved batches whose ROOTS keep shadowing facade lane ROOTS in tools.rust_coverage.
        out.append(f'  "facade_shadow_modules": {json.dumps(config["facade_shadow_modules"])}')
    out.append("}")
    return "\n".join(out) + "\n"


def add_mod_line(path: Path, module: str) -> None:
    text = path.read_text()
    line = f"pub mod {module};"
    if line not in text.splitlines():
        # Generated C2R source is never rustfmt-gated (preflight skips /c2r/); keep `cargo fmt
        # --check` of the family crate clean when the module moves in.
        path.write_text(text.rstrip() + "\n#[rustfmt::skip]\n" + line + "\n")


def ensure_state_wire(lib_path: Path) -> None:
    """T3605: a family lib that receives a `crate::state_wire` user re-exports the shared runtime instance."""
    text = lib_path.read_text()
    if re.search(r"\b(?:pub use tsfp_port_runtime::state_wire|pub mod state_wire)\b", text):
        return
    marker = "pub mod c2r;"
    if marker not in text:
        raise SystemExit(f"{lib_path}: no `pub mod c2r;` to anchor the state_wire re-export")
    lib_path.write_text(
        text.replace(marker, "pub use tsfp_port_runtime::state_wire;\n\n" + marker, 1)
    )


def ensure_wire(crate_dir: Path) -> None:
    """T3610: a family that receives a tsfp-port-wire user depends on the wiring crate and re-exports its modules."""
    manifest = crate_dir / "Cargo.toml"
    text = manifest.read_text()
    if WIRE_PACKAGE not in text:
        dep = f'{WIRE_PACKAGE} = {{ path = "../{WIRE_PACKAGE}" }}\n'
        dev = f'{WIRE_PACKAGE} = {{ path = "../{WIRE_PACKAGE}", features = ["thread-retire"] }}\n'
        runtime = 'tsfp-port-runtime = { path = "../tsfp-port-runtime" }\n'
        text = text.replace(runtime, runtime + dep, 1)
        if "[dev-dependencies]" in text:
            text = text.replace("[dev-dependencies]\n", "[dev-dependencies]\n" + dev, 1)
        else:
            text = text.replace("[workspace]", "[dev-dependencies]\n" + dev + "\n[workspace]", 1)
        manifest.write_text(text)
    lib_path = crate_dir / "src/lib.rs"
    lib = lib_path.read_text()
    if "tsfp_port_wire" not in lib:
        marker = "pub mod c2r;"
        lib_path.write_text(
            lib.replace(
                marker,
                "// T3610: the wiring layer is hosted once in tsfp-port-wire; batches keep their `crate::<module>` paths.\n"
                "pub use tsfp_port_wire::{\n    contract_views, hud_metrics, leaf_wire, menu_state, missions_system, missions_wire, player_wire, views,\n};\n\n"
                + marker,
                1,
            )
        )


def apply(root: Path, groups: dict[str, tuple[str, list[str]]]) -> list[str]:
    config_path = root / FAMILIES
    config = json.loads(config_path.read_text())
    by_package = {family["package"]: family for family in config["families"]}
    new_packages = []
    for package in sorted(groups):
        subsystem, modules = groups[package][0], sorted(groups[package][1])
        crate_dir = root / "crates" / package
        if package not in by_package:
            if (crate_dir / "Cargo.toml").exists():
                raise SystemExit(f"{crate_dir} exists but is not a registered family")
            family = {"subsystem": subsystem, "package": package, "modules": []}
            config["families"].append(family)
            by_package[package] = family
            new_packages.append(package)
            (crate_dir / "src/c2r").mkdir(parents=True)
            (crate_dir / "Cargo.toml").write_text(CARGO_TEMPLATE.format(package=package))
            lib = LIB_TEMPLATE.format(subsystem=subsystem)
            (crate_dir / "src/lib.rs").write_text(lib)
            (crate_dir / "src/c2r/mod.rs").write_text(C2R_MOD_TEMPLATE)
        family = by_package[package]
        ensure_state_wire(crate_dir / "src/lib.rs")
        checks = []
        for module in modules:
            source = root / C2R / f"{module}.rs"
            text = source.read_text()
            subprocess.run(
                [
                    "git",
                    "mv",
                    str(source.relative_to(root)),
                    f"crates/{package}/src/c2r/{module}.rs",
                ],
                cwd=root,
                check=True,
            )
            add_mod_line(crate_dir / "src/c2r/mod.rs", module)
            family["modules"].append(module)
            if uses_wire(text):
                ensure_wire(crate_dir)
                family["wire"] = True
            roots = ROOTS.search(text)[1].strip()
            count = len(re.findall(r"0x[0-9a-fA-F]+", roots))
            checks.append(f"        assert_eq!(super::c2r::{module}::ROOTS.len(), {count});")
        if package in new_packages:
            lib_path = crate_dir / "src/lib.rs"
            body = TEST_TEMPLATE.format(checks="\n".join(checks))
            lib_path.write_text(lib_path.read_text() + body)
    shadow = set(config.get("facade_shadow_modules", []))
    shadow.update(m for _, ms in groups.values() for m in ms)
    config["facade_shadow_modules"] = sorted(shadow)
    config_path.write_text(render_config(config))

    # tsfp-ports c2r/mod.rs: `pub mod m;` -> `pub use <crate>::c2r::m;`
    moved = {m: package.replace("-", "_") for package, (_, ms) in groups.items() for m in ms}
    mod_path = root / C2R / "mod.rs"
    lines = []
    for line in mod_path.read_text().splitlines():
        match = re.fullmatch(r"pub mod ([a-z0-9_]+);", line)
        if match and match[1] in moved:
            line = f"pub use {moved[match[1]]}::c2r::{match[1]};"
        lines.append(line)
    mod_path.write_text("\n".join(lines) + "\n")

    # tsfp-ports dependencies and root workspace exclude.
    manifest = root / PORTS / "Cargo.toml"
    text = manifest.read_text()
    deps = "".join(f'\n{package} = {{ path = "../{package}" }}' for package in new_packages)
    text = text.replace("\n\n[dev-dependencies]", f"{deps}\n\n[dev-dependencies]", 1)
    manifest.write_text(text)
    workspace = root / "Cargo.toml"
    wtext = workspace.read_text()
    match = re.search(r"^exclude = \[(.*)\]$", wtext, re.M)
    entries = [e.strip().strip('"') for e in match[1].split(",") if e.strip()]
    for package in new_packages:
        if f"crates/{package}" not in entries:
            entries.append(f"crates/{package}")
    new_line = "exclude = [" + ", ".join(f'"{e}"' for e in entries) + "]"
    workspace.write_text(wtext[: match.start()] + new_line + wtext[match.end() :])
    return new_packages


def refresh_locks(root: Path) -> None:
    """Add the new path packages to every tracked Cargo.lock that pins tsfp-ports.
    Run under `python3 -m tools.wt run` so CARGO_HOME and TSFP_BEVY_CARGO_HOME are set."""
    listed = subprocess.run(
        ["git", "grep", "-l", 'name = "tsfp-ports"', "--", "*Cargo.lock"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.split()
    for lock in listed:
        env = dict(os.environ)
        if "TSFP_BEVY_CARGO_HOME" in env and lock != "Cargo.lock":
            env["CARGO_HOME"] = env["TSFP_BEVY_CARGO_HOME"]
        subprocess.run(
            ["cargo", "metadata", "--offline", "--format-version", "1"],
            cwd=root / Path(lock).parent,
            env=env,
            stdout=subprocess.DEVNULL,
            check=True,
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument(
        "--subsystems",
        type=Path,
        default=None,
        help="function_subsystems.csv (gitignored; default tools/data/ of --root)",
    )
    parser.add_argument(
        "--crate-lines",
        type=int,
        default=DEFAULT_CRATE_LINES,
        help="fill a family crate up to this many non-test lines, then open <package>-2, -3, ...",
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--plan", action="store_true")
    mode.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    root = args.root
    groups, kept = plan(
        root, load_subsystems(args.subsystems or root / SUBSYSTEMS), args.crate_lines
    )
    total = 0
    for package in sorted(groups):
        modules = groups[package][1]
        size = sum((root / C2R / f"{m}.rs").stat().st_size for m in modules)
        lines = sum(module_lines(root, m) for m in modules)
        total += size
        print(f"{package}\t{len(modules)}\t{size}\t{lines} lines\t{' '.join(sorted(modules))}")
    print(f"total\t{sum(len(ms) for _, ms in groups.values())}\t{total}")
    reasons = Counter(reason.split(" ")[0] + " " + reason.split(" ")[1] for reason in kept.values())
    print("kept in facade:", len(kept), dict(reasons))
    if args.apply:
        print("new crates:", " ".join(apply(root, groups)) or "none")
        refresh_locks(root)
        for tool in ("tools.portreg", "tools.rust_coverage"):
            subprocess.run(
                [sys.executable, "-m", tool, "--write"],
                cwd=root,
                stdout=subprocess.DEVNULL,
                check=True,
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
