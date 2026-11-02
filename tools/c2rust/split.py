# SPDX-License-Identifier: GPL-3.0-or-later
"""Routing for generated modules moved to per-subsystem crates."""

from __future__ import annotations

import json
import re
from pathlib import Path

FAMILY_CONFIG = Path(__file__).resolve().parents[2] / "tools/data/c2r_family_crates.json"
FAMILY_SPECS = json.loads(FAMILY_CONFIG.read_text())["families"]
FAMILY_CRATES = {family["package"].replace("-", "_"): family for family in FAMILY_SPECS}
SPLIT_MODULES = {
    module: crate for crate, family in FAMILY_CRATES.items() for module in family["modules"]
}


def family_packages(root: Path) -> list[tuple[str, Path]]:
    """Return each no_std family crate once, in its stable config order."""
    packages: list[tuple[str, Path]] = []
    seen: set[str] = set()
    for family in FAMILY_SPECS:
        package = family["package"]
        crate = package.replace("-", "_")
        if crate not in seen:
            packages.append((crate, root / "crates" / package))
            seen.add(crate)
    return packages


def wire_package(root: Path) -> tuple[str, Path]:
    """T3610: the S3 wiring crate, built after the runtime and before the family crates."""
    return "tsfp_port_wire", root / "crates/tsfp-port-wire"


def split_package(module: str) -> str | None:
    return SPLIT_MODULES.get(module)


def split_source(root: Path, module: str) -> Path:
    package = split_package(module)
    if package is None:
        raise KeyError(module)
    return root / "crates" / package.replace("_", "-") / "src/c2r" / f"{module}.rs"


def ensure_family_module(root: Path, module: str) -> None:
    package = split_package(module)
    if package is None:
        raise KeyError(module)
    registry = root / "crates" / package.replace("_", "-") / "src/c2r/mod.rs"
    text = (
        registry.read_text()
        if registry.exists()
        else "// SPDX-License-Identifier: GPL-3.0-or-later\n"
    )
    declaration = f"pub mod {module};"
    if declaration not in text.splitlines():
        registry.write_text(text.rstrip() + "\n\n" + declaration + "\n")


def split_stub(module: str, generated: str) -> str:
    package = split_package(module)
    if package is None:
        raise KeyError(module)
    match = re.search(r"pub const ROOTS: \[u32; (\d+)\] = \[([^\]]*)\]", generated)
    if match is None:
        raise ValueError(f"split module {module} has no generated ROOTS array")
    return f"pub use {package}::c2r::{module}::*;\n"


WIRE_REF = re.compile(
    r"\bcrate::(?:contract_views|hud_metrics|leaf_wire|menu_state|missions_system|missions_wire|player_wire|views)\b"
)


def ensure_family_wire(root: Path, module: str, text: str) -> None:
    """T3610: a split module naming a wire module needs its family to depend on the wire crate."""
    package = split_package(module)
    if package is None or not WIRE_REF.search(text):
        return
    from tools.dev import split_ports

    split_ports.ensure_wire(root / "crates" / package.replace("_", "-"))
    config_path = root / "tools/data/c2r_family_crates.json"
    config = json.loads(config_path.read_text())
    for family in config["families"]:
        if family["package"].replace("-", "_") == package and not family.get("wire"):
            family["wire"] = True
            config_path.write_text(split_ports.render_config(config))
