# SPDX-License-Identifier: GPL-3.0-or-later
"""T3765: derive crates/tsfp-ports/lanes.json rows for c2r roots from authoritative data.

A c2r function listed in a family crate's `ROOTS` table has no host ABI entry until it has a
lanes.json row (capture Tier 2 receipts say `no_abi_entry`, tmp/t2round-breakdown.md). This tool
emits those rows for the c2r lane crate (`tools.lanes.C2R_CRATE`):

* the owner is the family crate whose `src/c2r/**.rs` module lists the address in `ROOTS` and has
  a `run` arm for it (tsfp-ports itself is not an owner: the leaf crate cannot depend on it);
* `ret_pops`, `stack_args`, `calls`, `x87`, `sse` come from the lifted body
  (`tools.replace.invpose_meta.describe`, the same derivation as every existing row). A body
  without any `ret N` (it leaves through tail jumps) takes its cleanup from the lift's
  disassembly instead (`tools.disasm_cleanup`, T3785; the row then carries `"ret_source":
  "disasm"`).

Nothing is guessed: an address without exactly one owner, without a lifted body, with no or
more than one `ret N` cleanup, or already registered on any host table is skipped with a reason.

    python3 -m tools.lane_rows --va 0x76150 --va 0x77470            # dry run, JSON report
    python3 -m tools.lane_rows --va-file vas.txt --apply --regen    # add rows, regenerate
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

from tools import lanes

ROOTS_TABLE = re.compile(r"pub const ROOTS: \[u32; \d+\] = \[(.*?)\];", re.S)
LANE_MODULE = "c2r_lane"


def default_gen_dir(root: Path) -> Path:
    """generated/lifted/gen of the primary checkout (worktrees have no lifted tree)."""
    local = root / "generated/lifted/gen"
    if local.is_dir():
        return local
    common = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    ).stdout.strip()
    return Path(common).parent / "generated/lifted/gen"


def family_crates(root: Path) -> set[str]:
    """No_std family crates (the only crates the archive build links for the lane crate)."""
    from tools.c2rust.split import FAMILY_SPECS

    return {
        family["package"]
        for family in FAMILY_SPECS
        if (root / "crates" / family["package"]).is_dir()
    }


def c2r_owners(root: Path, crates: set[str]) -> dict[int, list[str]]:
    """{va: [`<crate>/<module path>`]} for every `ROOTS` entry with a `run` arm."""
    found: dict[int, list[str]] = {}
    for crate in sorted(crates):
        base = root / "crates" / crate / "src/c2r"
        for path in sorted(base.rglob("*.rs")):
            text = path.read_text()
            table = ROOTS_TABLE.search(text)
            if table is None:
                continue
            relative = path.relative_to(base).with_suffix("")
            parts = relative.parts[:-1] if relative.name == "mod" else relative.parts
            owner = f"{crate}/{'/'.join(parts)}"
            arms = text[table.end() :]
            for token in re.findall(r"0x([0-9a-fA-F_]+)", table.group(1)):
                va = int(token.replace("_", ""), 16)
                if re.search(rf"\b0x{va:x}\s*(?:=>|\|)", arms):
                    found.setdefault(va, []).append(owner)
    return found


FACADE_USE = re.compile(r"pub use super::(\w+)::\{(.*?)\};", re.S)
FACADE_ARM = re.compile(
    r"(?m)^\s*0x([0-9a-f]+) => ([a-z_][a-z0-9_]*\((?:m|guest|regs)(?:, (?:m|guest|regs))*\)),$"
)


def facade_calls(
    root: Path, packages: dict[str, str]
) -> tuple[dict[int, tuple[str, str]], dict[int, str]]:
    """tsfp-ports c2r facades (T3433 splits): {va: (owner, call)} and {va: reason} for refused arms.

    `packages` maps a split module name to its family crate. An arm is usable when it calls a
    function the facade re-exports from a split module; a pre-hook (`matches!(va, ..)` before the
    `match`) or a body defined in the facade itself keeps the root on tsfp-ports.
    """
    found: dict[int, tuple[str, str]] = {}
    refused: dict[int, str] = {}
    for path in sorted((root / "crates/tsfp-ports/src/c2r").glob("*.rs")):
        text = path.read_text()
        start = text.find("\npub fn run<")
        if ROOTS_TABLE.search(text) is None or start < 0:
            continue
        exported = {
            name.strip(): module
            for module, names in FACADE_USE.findall(text)
            for name in names.split(",")
            if name.strip()
        }
        body = text[start:]
        head, _, arms = body.partition("    match va {")
        hooked = {int(v, 16) for v in re.findall(r"0x([0-9a-f]+)", head)}
        for va_text, call in FACADE_ARM.findall(arms.split("\n    }\n", 1)[0]):
            va = int(va_text, 16)
            module = exported.get(call.split("(", 1)[0])
            if va in hooked:
                refused[va] = f"tsfp-ports/c2r/{path.stem} pre-hook keeps it on tsfp-ports"
            elif module is None or module not in packages:
                refused[va] = f"tsfp-ports/c2r/{path.stem} body is not in a family crate"
            else:
                found[va] = (f"{packages[module]}/{module}", call)
    return found, refused


def offline_facade_owners(root: Path, bindings: dict[str, str]) -> dict[int, list[str]]:
    """Discover unhooked facade run arms compiled into explicit offline-only crates."""
    found: dict[int, list[str]] = {}
    for module, package in sorted(bindings.items()):
        if not re.fullmatch(r"[a-z][a-z0-9_]*", module) or not re.fullmatch(
            r"tsfp-t2-[a-z0-9-]+", package
        ):
            raise ValueError(f"invalid offline facade binding: {module} {package}")
        if not (root / "crates" / package / "Cargo.toml").is_file():
            raise ValueError(f"offline facade crate missing: {package}")
        path = root / "crates/tsfp-ports/src/c2r" / f"{module}.rs"
        text = path.read_text()
        table = ROOTS_TABLE.search(text)
        start = text.find("\npub fn run<")
        if table is None or start < 0:
            raise ValueError(f"offline facade lacks ROOTS or run: {module}")
        head, marker, arms = text[start:].partition("    match va {")
        if not marker:
            raise ValueError(f"offline facade has no recognizable dispatch: {module}")
        # Any run prelude might alter dispatch or state; refuse the whole module.
        if not head.partition("{")[1] or head.partition("{")[2].strip():
            continue
        roots = {
            int(v.replace("_", ""), 16) for v in re.findall(r"0x([0-9a-fA-F_]+)", table.group(1))
        }
        for value, _ in FACADE_ARM.findall(arms.split("\n    }\n", 1)[0]):
            va = int(value, 16)
            if va in roots:
                found.setdefault(va, []).append(f"{package}/{module}")
    return found


def split_packages() -> dict[str, str]:
    from tools.c2rust.split import SPLIT_MODULES

    return {module: crate.replace("_", "-") for module, crate in SPLIT_MODULES.items()}


def host_owned(root: Path) -> dict[int, str]:
    """{va: owner} of addresses whose `sub_<VA>` the host build already owns outside the lane
    tables: src/game replacements and the ui draw hooks. A lane row there fails configure
    (`tools.play.ui_draw_hooks`: already owned, I-T3785-2)."""
    from tools.play.ui_draw_hooks import HOOKS
    from tools.replace.scan import registered_in_any_variant

    owned = {va: "src/game replacement" for va in registered_in_any_variant(root / "src/game")}
    owned.update({va: "tools.play.ui_draw_hooks hook" for va in HOOKS})
    return owned


def registered(root: Path) -> set[int]:
    """Every address the host already resolves (lane, legacy, ext, weapon roots)."""
    manifest = json.loads((root / lanes.MANIFEST).read_text())
    known = {int(row["va"], 16) for lane in manifest["lanes"] for row in lane["roots"]}
    for group in manifest["legacy"].values():
        known |= {int(row["va"], 16) for row in group["roots"]}
    ext = lanes.EXT_TABLE.search((root / lanes.EXT_SOURCE).read_text())
    if ext is None:
        raise ValueError(f"EXT_ROOTS table not found in {lanes.EXT_SOURCE}")
    known |= {int(v, 16) for v in re.findall(r"0x([0-9a-fA-F]+)", ext.group(1))}
    from tools.play.rust_weapons import lane_roots

    known |= set(lane_roots(root))
    return known


REGISTRY = Path("docs/data/port-registry.json")


def typed_wired(root: Path) -> set[int]:
    """Roots a `tools.typed_roots` adapter (tsfp-port-lanes-typed) dispatches to their readable
    port (T3785): their lane row is that port's ABI entry."""
    from tools import typed_roots

    spec = typed_roots.load(root)
    return {int(row["va"], 16) for module in spec["modules"].values() for row in module["roots"]}


def hand_ports(root: Path) -> dict[int, str]:
    """{va: module} of ports outside any c2r module (port registry). A lane row there would be a
    second port of the same function (tools.portreg duplicate check, T3766)."""
    registry = json.loads((root / REGISTRY).read_text())["ports"]
    wired = typed_wired(root)
    found: dict[int, str] = {}
    for va, entries in registry.items():
        if int(va, 16) in wired:
            continue  # the typed adapter is that hand port's ABI entry, not a second port
        for entry in entries:
            module = entry.get("module", "")
            if "/c2r/" not in module and not module.startswith(f"{lanes.C2R_CRATE}/"):
                found.setdefault(int(va, 16), module)
    return found


def derive(
    va: int,
    owners: dict[int, list[str]],
    known: set[int],
    describe: Callable[[int], dict],
    facades: tuple[dict[int, tuple[str, str]], dict[int, str]] = ({}, {}),
    hand: dict[int, str] | None = None,
    disasm: Callable[[int], tuple[list[int] | None, str]] | None = None,
    owned: dict[int, str] | None = None,
) -> tuple[dict | None, str]:
    """(row, "") or (None, reason). `describe(va)` returns the lifted metadata or raises;
    `disasm(va)` returns the disassembly cleanup (`tools.disasm_cleanup`)."""
    if va in known:
        return None, "already registered on a host table"
    if owned and va in owned:
        return None, f"host owns sub_{va:08X} ({owned[va]})"
    if hand and va in hand:
        return None, f"hand port in {hand[va]}"
    candidates = owners.get(va, [])
    extra = {}
    if not candidates and va in facades[0]:
        owner, call = facades[0][va]
        candidates, extra = [owner], {"c2r_call": call}
    elif not candidates and va in facades[1]:
        return None, facades[1][va]
    if len(candidates) != 1:
        return None, f"needs exactly one c2r owner, found {len(candidates)} {candidates}"
    try:
        meta = describe(va)
    except SystemExit as error:
        return None, f"no lifted body ({error})"
    pops = meta["ret_pops"]
    if not pops and disasm is not None:
        found, why = disasm(va)
        if found is None:
            return None, f"callee cleanup not derivable (lifted ret_pops [], disassembly: {why})"
        meta, extra = {**meta, "ret_pops": found}, {**extra, "ret_source": "disasm"}
        pops = found
    if len(pops) != 1:
        return None, f"callee cleanup not derivable from the lifted body (ret_pops {pops})"
    row = {"va": f"{va:08X}", **{key: meta[key] for key in lanes.FIELDS[2:]}, "c2r": candidates[0]}
    return {**row, **extra}, ""


def apply(manifest: dict, rows: list[dict]) -> int:
    """Append rows to the c2r lane (created on first use); returns the number added."""
    lane = next((lane for lane in manifest["lanes"] if lane["module"] == LANE_MODULE), None)
    if lane is None:
        lane = {"module": LANE_MODULE, "crate": lanes.C2R_CRATE, "roots": []}
        manifest["lanes"].append(lane)
    present = {row["va"] for row in lane["roots"]}
    added = [row for row in rows if row["va"] not in present]
    lane["roots"].extend(added)
    return len(added)


def parse_va(text: str) -> int:
    return int(text, 16)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--va", type=parse_va, action="append", default=[], help="0x.. (repeatable)"
    )
    parser.add_argument("--va-file", type=Path, help="one 0x.. address per line")
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--gen-dir", type=Path, help="lifted C (default: primary checkout generated/lifted/gen)"
    )
    parser.add_argument(
        "--apply", action="store_true", help="append the derived rows to lanes.json"
    )
    parser.add_argument(
        "--disasm-dir",
        type=Path,
        help="lift disassembly for tail-jump cleanup (default: newest complete lift)",
    )
    parser.add_argument("--regen", action="store_true", help="run tools.lanes regen after --apply")
    parser.add_argument(
        "--offline",
        action="store_true",
        help="derive capture-only rows without the live host symbol ownership restriction",
    )
    parser.add_argument("--report", type=Path, help="write the JSON report here as well")
    args = parser.parse_args(argv)
    if args.offline and (args.apply or args.regen):
        parser.error("--offline only produces a report, never live lanes")
    vas = list(args.va)
    if args.va_file:
        vas += [
            parse_va(line.split()[0])
            for line in args.va_file.read_text().splitlines()
            if line.strip()
        ]
    if not vas:
        parser.error("give --va or --va-file")
    root = args.root.resolve()
    gen = args.gen_dir or default_gen_dir(root)
    if not (gen / "recomp_dispatch.c").is_file():
        parser.error(f"no lifted tree at {gen}")
    from tools.replace.invpose_meta import describe

    families = family_crates(root) - {"tsfp-ports"}
    owners = c2r_owners(root, families)
    if args.offline:
        bindings = json.loads((root / "tools/data/offline_facade_sources.json").read_text())
        for va, extra in offline_facade_owners(root, bindings).items():
            if va not in owners:
                owners[va] = extra
    packages = {module: crate for module, crate in split_packages().items() if crate in families}
    facades = facade_calls(root, packages)
    known = registered(root)
    hand = hand_ports(root)
    owned = {} if args.offline else host_owned(root)
    from tools import disasm_cleanup

    disasm_dir = args.disasm_dir or disasm_cleanup.default_dir(root)
    disasm = None
    if disasm_dir is None or not (disasm_dir / "functions.json").is_file():
        print(
            f"lane_rows: no lift disassembly ({disasm_dir}); tail-jump cleanup is refused",
            file=sys.stderr,
        )
    else:
        disasm = disasm_cleanup.Disassembly(disasm_dir).cleanup
    rows, skipped = [], {}
    for va in dict.fromkeys(vas):
        row, reason = derive(
            va, owners, known, lambda value: describe(gen, value), facades, hand, disasm, owned
        )
        if row is None:
            skipped[f"0x{va:08X}"] = reason
        else:
            rows.append(row)
    report = {"rows": rows, "skipped": skipped}
    if args.apply and rows:
        path = root / lanes.MANIFEST
        manifest = json.loads(path.read_text())
        report["added"] = apply(manifest, rows)
        path.write_text(json.dumps(manifest, indent=1))  # the manifest's own layout
        if args.regen:
            report["regen"] = lanes.run(root, check=False, registry=False)
    text = json.dumps(report, indent=1)
    if args.report:
        args.report.write_text(text + "\n")
    print(
        text
        if len(text) < 6000
        else json.dumps(
            {k: (len(v) if isinstance(v, (list, dict)) else v) for k, v in report.items()}
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
