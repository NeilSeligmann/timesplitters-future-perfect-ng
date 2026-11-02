#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""T1850: build and run the Rust-vs-lifted differential for the pose/control and inventory ports
(tests/c/test_rust_invpose.c + tests/c/invpose_fixtures.inc) at lifted O0/O3 x Rust O0/O3, edge
coverage (--cover) and generated mutation controls (--mutations).

Adapted from tools/t1831_weapons_diff.py (not modified). The lifted sources are private and
ignored, so pass --gen-dir (the MAIN checkout's generated/lifted/gen from a worktree).
Output goes to --out-dir (default tmp/t1850).

Mutation controls are generated, not hand written: every conditional jump, comparison,
constant offset, memory store and arithmetic call of a root's translated function is mutated one
at a time (see `mutants`). A mutant that is not KILLED must be explained in EQUIVALENT.
"""

# ruff: noqa: E501
from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from tools.datadir import output_path
from tools.play.rust_originals import preserve

# group name -> (roots, source file holding them)
GROUPS: dict[str, tuple[tuple[int, ...], str]] = {
    "pose": (
        (
            0x426B0,
            0x426E0,
            0x427B0,
            0x42C30,
            0x42CE0,
            0x431C0,
            0x43A90,
            0x43AF0,
            0x59D70,
            0x59F10,
            0x59F50,
            0x5A1E0,
            0xB2D20,
            0xC4560,
            0xC9BB0,
            0xD0CA0,
        ),
        "pose.rs",
    ),
    "inventory": (
        (
            0x1B6B40,
            0x1B68F0,
            0x1B66E0,
            0x1B6C90,
            0x1B9D30,
            0x1BA130,
            0x1BA220,
            0x1C1D90,
            0x1BAC50,
            0x1BFC00,
            0xD7BC0,
        ),
        "inventory.rs",
    ),
    "select": ((0x1BA410, 0x1BA7B0), "inventory_select.rs"),
    "ammo": (
        (
            0x1B67E0,
            0x1B6890,
            0x1B6950,
            0x1B6D10,
            0x1B6E20,
            0x1B6EC0,
            0x1B9760,
            0x1BA2B0,
            0x1BADD0,
            0x1BAE40,
            0x1BAEB0,
            0x1BFE60,
            0x1C1A90,
            0x1C0080,
            0x1B99B0,
            0x1B9CF0,
        ),
        "inventory_ammo.rs",
    ),
    "slots": (
        (
            0x1B9AB0,
            0x1B9AD0,
            0x1B6BB0,
            0x1B6BF0,
            0x1BA1B0,
            0x1BFA60,
            0x1BA8F0,
            0x1BA9B0,
            0x1BAA70,
            0x1BAAF0,
            0x1BABA0,
            0x1BACA0,
            0x1BFEC0,
        ),
        "inventory_slots.rs",
    ),
    "pickup": (
        (0xBE090, 0xBFB40, 0xBFF70, 0xCEF30, 0x357160, 0x357320, 0x25EAD0, 0x25EC70),
        "inventory_pickup.rs",
    ),
    "leaf": (
        (
            0x2CBCE0,
            0x259DF0,
            0xD1E20,
            0xD2D20,
            0x426D0,
            0x42710,
            0x42760,
            0x42830,
            0x42870,
            0x428D0,
            0x42920,
            0x42A30,
            0x42CA0,
            0x42D00,
            0x59CF0,
            0x59D90,
            0x59DD0,
            0x59E00,
            0x5BA00,
            0x64650,
            0x646A0,
            0x8EA20,
            0x949C0,
            0xC6140,
            0xC63F0,
            0xC7950,
            0xC9BE0,
            0xC9C00,
            0xC9CC0,
            0xC9F50,
            0xCFEC0,
            0xD6920,
            0xF8AD0,
            0xF8AE0,
            0x147080,
            0x190E80,
            0x22DD90,
            0x246D10,
            0x246DC0,
            0x2474E0,
            0x258870,
            0x25A080,
            0x25B720,
            0x2E86A0,
            0x2E8C00,
            0x42C50,
            0x585B0,
            0x61430,
            0x61610,
            0x61630,
            0x660C0,
            0x66100,
            0x66220,
            0x115F10,
            0x259800,
            0x25EF30,
            0x3381D0,
            0x2CE120,
            0x126260,
            0x238460,
            0x238510,
            0x2385B0,
            0x238610,
        ),
        "pose_leaf.rs",
    ),
    "anim": ((0x606E0, 0x60740, 0x65340, 0x66280), "pose_anim.rs"),
    "grant": ((0x1BFAA0, 0x1BFCA0, 0xC0310), "inventory_grant.rs"),
}
RUSTC = (
    "rustc",
    "--edition=2024",
    "--target",
    "x86_64-unknown-linux-gnu",
    "--crate-name",
    "tsfp_ports",
    "--crate-type",
    "staticlib",
    "-Cpanic=abort",
    "-Coverflow-checks=off",
    "-Crelocation-model=pic",
)
# CFG edges no input can take, with the reason (checked by reading the lifted original)
UNREACHABLE: dict[tuple[int, int, int], str] = {
    (
        0x1B68F0,
        0x1B6930,
        0x1B6900,
    ): "the inner loop always runs exactly 70 entries (0x4FA128 + 70 * 0x3C == 0x4FB190), so ECX is 71 at the outer test and the outer loop never repeats",
    (
        0x1BA410,
        0x1BA51A,
        0x1BA55D,
    ): "EDI holds the constant 1 on every path to 0x1BA51A, so the compare EDI == 0 never holds",
    (
        0x1BA410,
        0x1BA568,
        0x1BA5AA,
    ): "the compare is (ext + 0x1D4) == 0, true only for the invalid pointer 0xFFFFFE2C",
    (
        0x1B67E0,
        0x1B680A,
        0x1B6839,
    ): "the compare is (ext + 0xA4) == 0, true only for the invalid pointer 0xFFFFFF5C",
}
# mutants that cannot change observable behaviour, with the reason (name -> reason)
EQUIVALENT: tuple[tuple[str, str], ...] = (
    # (regex over "<mutant name> | <original line>", reason); each was read in the lifted original and the Rust
    (
        r"^(59d70|c9bb0):\d+ reg eax xor1 \| eax = 0;?$",
        "the xor on a zeroed EAX is overwritten by the following SETcc low-byte write",
    ),
    (
        r"^(59d70|c9bb0):\d+ const 0xff->0x103 ",
        "the low-byte mask 0xff vs 0x103 sees only a 0 or 1 SETcc value",
    ),
    (
        r"^43a90:\d+ reg edx xor1 \| edx = \(edx & 0xffff_ff00\) \| \(\(\(\(m\.read8\(ecx\)",
        "bit 0 of the loaded byte is cleared by the next statement (& 0xFE)",
    ),
    (
        r"^c4560:\d+ drop store \| m\.write32\(edx, ecx\);",
        "the zero store at +0 is overwritten by the later store of -1 to the same address (edx == eax)",
    ),
    (
        r"^606e0:\d+ reg eax xor1 \| eax = eax\.wrapping_add\(edx\);",
        "bit 0 is shifted out by the following arithmetic shift right by 2",
    ),
    (
        r"^65340:\d+ reg eax xor1 \| eax = edx;",
        "bit 0 is shifted out by the following arithmetic shift right by 5",
    ),
    (
        r"^1b6b40:\d+ drop store \| m\.write32\(0x75a024, ecx\);",
        "the initial -1 is overwritten by the final count store before the function returns",
    ),
    (
        r"^1b68f0:\d+ op < \| if \(fas < fbs\) \{ pc = 0x1b6900",
        "that edge is proven unreachable (the outer loop never repeats)",
    ),
    (
        r"^1b66e0:\d+ reg ecx xor1 \| ecx = m\.read32\(esp\.wrapping_add\(0x14\)\);",
        "the kind is compared with 0 and 1 and both take the same path; n <-> n^1 keeps the branch for n >= 2",
    ),
    (
        r"^1b6c90:\d+ reg edx xor1 \| edx = m\.read32\(ecx\.wrapping_add\(eax\)\.wrapping_add\(0x68\)\);",
        "only the sign bit of the entry flags is tested, then EDX is overwritten",
    ),
    (
        r"^1ba130:\d+ reg ecx xor1 \| ecx = m\.read32\(0x7de458\);",
        "only bits 8..15 are tested, then ECX is overwritten",
    ),
    (
        r"^1ba130:\d+ reg ebx xor1 \| ebx = \(ebx & 0xffff_ff00\)",
        "only the sign of the low byte is tested and EBX is restored by the epilogue pop",
    ),
    (
        r"^d7bc0:\d+ reg eax xor1 \| eax = m\.read32\(0x7de458\);",
        "bit 0 is shifted out by the following shift right by 13",
    ),
    (
        r"^d7bc0:\d+ (const 0x44->0x48|reg ecx xor1) \| ecx = 0x44;",
        "the extra iterations only rewrite bytes that the function zeroes or overwrites again",
    ),
)


def run(
    cmd: list[str], cwd: Path | None = None, timeout: int = 1800, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    merged = {**os.environ, **env} if env else None
    return subprocess.run(
        cmd, cwd=cwd, text=True, capture_output=True, timeout=timeout, check=False, env=merged
    )


# T1944: a seed whose ORIGINAL runs past the per-seed CPU budget (T1944_ORIGINAL_MS, default 100 ms) loops
# forever on its fixture (e.g. a ring walk over an order table with no selectable entry) and is skipped by
# the C watchdog, which names root and seed. More than this share of a root's seeds skipped means the
# fixture is degenerate for that root, so the run fails instead of passing on the few seeds left.
LOOP_SKIP_LIMIT = 0.25
RUN_TIMEOUT = (
    900  # seconds per exe run (2,000 seeds take about 1 minute per group, see docs/t1944-hang.md)
)
ROOT_LINE = re.compile(r"^root 0x([0-9a-f]+) seeds (\d+) .* loops-skipped (\d+)\b", re.MULTILINE)


def loop_skip_report(
    output: str, limit: float = LOOP_SKIP_LIMIT
) -> tuple[dict[int, int], list[str]]:
    """Per-root watchdog skips parsed from a harness run, and the roots that skipped over `limit`."""
    skipped: dict[int, int] = {}
    excessive: list[str] = []
    for match in ROOT_LINE.finditer(output):
        va, seeds, loops = int(match.group(1), 16), int(match.group(2)), int(match.group(3))
        skipped[va] = loops
        if seeds and loops / seeds > limit:
            excessive.append(
                f"root {va:#x} skipped {loops} of {seeds} seeds (original loops forever)"
            )
    return skipped, excessive


def roots_macro(roots: tuple[int, ...]) -> str:
    return " ".join(f"X({va:08x},0x{va:x})" for va in roots)


def build_and_run(
    root: Path,
    src: Path,
    out: Path,
    gen: Path,
    group: str,
    lifted_opt: str,
    rust_opt: str,
    seeds: int,
    cover: bool = False,
    only: tuple[int, ...] | None = None,
    run_timeout: int | None = None,
    env: dict[str, str] | None = None,
) -> tuple[bool, str]:
    run_timeout = run_timeout or RUN_TIMEOUT
    out.mkdir(parents=True, exist_ok=True)
    roots = only or GROUPS[group][0]
    # T1874: T1850_LANE=1 runs the Rust side through the production lane entry (tsfp_rust_try_lane,
    # runtime.rs selection, lane_inv.rs register-file marshalling, rust_lane_call.c) instead of the
    # ExtRegisters differential entry. The seven VAs that earlier lanes already own are skipped.
    lane = os.environ.get("T1850_LANE") == "1"
    if lane:
        from tools.replace.rust_lane import INV_HOST_ROOTS

        roots = tuple(va for va in roots if va in INV_HOST_ROOTS)
        if not roots:
            return True, "no lane roots in this group"
    archive = out / "libtsfp_ports.a"
    extra = ["--cfg", "tsfp_cover"] if cover else []
    result = run(
        [*RUSTC, f"-Copt-level={rust_opt}", *extra, str(src / "lib.rs"), "-o", str(archive)]
    )
    if result.returncode:
        return False, "rust build failed\n" + result.stderr[-2000:]
    originals = out / "originals.c"
    preserve(gen, originals, roots)
    exe = out / "test_rust_invpose"
    cc = [
        "cc",
        f"-O{lifted_opt}",
        "-fno-strict-aliasing",
        "-w",
        "-DTSFP_RUST_PORTS=1",
        *(["-DLANE_BRIDGE=1"] if lane else []),
        f"-DROOTS={roots_macro(roots)}",
        "-I",
        str(root / "src/game"),
        "-I",
        str(root / "src/host"),
        "-I",
        str(root / "tests/c"),
        "-I",
        str(gen),
        str(root / "tests/c/test_rust_invpose.c"),
        str(root / "src/host/rust_ext_bridge.c"),
        str(
            root / "src/host/rust_lane_call.c"
        ),  # tsfp_lane_guest_call: every binary linking libtsfp_ports.a needs it
        str(originals),
        str(archive),
        "-lm",
        "-o",
        str(exe),
    ]
    result = run(cc)
    if result.returncode:
        return False, "C build failed\n" + result.stderr[-3000:]
    try:
        result = run([str(exe), str(seeds)], timeout=run_timeout, env=env)
    except subprocess.TimeoutExpired as expired:
        partial = expired.stdout or ""
        partial = partial.decode(errors="replace") if isinstance(partial, bytes) else partial
        done = {int(m.group(1), 16) for m in ROOT_LINE.finditer(partial)}
        stuck = next((va for va in roots if va not in done), None)
        where = f"root {stuck:#x}" if stuck is not None else "after the last root"
        return (
            False,
            f"TIMEOUT after {run_timeout}s at {where} (a loop the per-seed watchdog did not break)",
        )
    text = result.stdout + result.stderr
    _, excessive = loop_skip_report(text)
    if excessive:
        return False, "\n".join(excessive) + "\n" + text
    return result.returncode == 0, text


def function_spans(text: str) -> dict[int, tuple[int, int]]:
    """VA -> (start, end) char span of each translated function body (found by its cover hook)."""
    spans: dict[int, tuple[int, int]] = {}
    for m in re.finditer(r"(?m)^pub fn \w+<", text):
        end = text.find("\n}\n", m.start())
        hook = re.search(r"crate::pose::cover\(0x([0-9a-f]+),", text[m.start() : end])
        if hook:
            spans[int(hook.group(1), 16)] = (m.start(), end)
    return spans


def function_body(text: str, va: int) -> str:
    start, end = function_spans(text)[va]
    return text[start:end]


def edge_report(text: str, output: str, roots: tuple[int, ...]) -> tuple[bool, str]:
    """Compare the edges taken (printed by the harness built with --cfg tsfp_cover) with every
    static `pc = X; continue` edge of the translated functions of `roots`."""
    spans = function_spans(text)
    seen = {
        (int(m.group(1), 16), int(m.group(2), 16), int(m.group(3), 16))
        for m in re.finditer(r"^edge ([0-9a-f]+) ([0-9a-f]+) ([0-9a-f]+)$", output, re.MULTILINE)
    }
    problems = []
    totals = []
    for va in roots:
        if va not in spans:
            problems.append(f"{va:x}: no translated body")
            continue
        body = text[spans[va][0] : spans[va][1]]
        arms = re.split(r"\n            (0x[0-9a-f]+) => \{\n", body)
        static = set()
        blocks = set()
        for index in range(1, len(arms), 2):
            label = int(arms[index], 16)
            blocks.add(label)
            for target in re.findall(r"pc = (0x[0-9a-f]+); continue", arms[index + 1]):
                static.add((label, int(target, 16)))
        taken = {(a, b) for f, a, b in seen if f == va}
        entered = {b for _, b in taken} | {min(blocks)}
        waived = {edge for edge in static - taken if (va, *edge) in UNREACHABLE}
        missing = sorted(static - taken - waived)
        unentered = sorted(blocks - entered)
        totals.append(
            f"{va:x}: blocks {len(blocks) - len(unentered)}/{len(blocks)} edges {len(static & taken)}/{len(static)}"
            + (f" (+{len(waived)} unreachable, proven)" if waived else "")
        )
        if missing or unentered:
            problems.append(
                f"{va:x}: missing edges {[(hex(a), hex(b)) for a, b in missing[:12]]}{' ...' if len(missing) > 12 else ''} unentered {[hex(b) for b in unentered[:12]]}"
            )
    return not problems, "; ".join(totals) + ("\n" + "\n".join(problems) if problems else "")


REGS = ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")


def dead_register_definitions(body: str) -> set[int]:
    """Line numbers (within `body`) of single-register assignments whose value no path can read before
    it is overwritten (register liveness over the translated block CFG). Registers are live at calls,
    tail jumps and the function exit (EAX/ECX/EDX and the callee-saved set are all compared)."""
    lines = body.split("\n")
    arms: list[tuple[int, int, int]] = []  # (label, first line, last line exclusive)
    start = None
    label = 0
    for number, line in enumerate(lines):
        m = re.match(r"\s{12}(0x[0-9a-f]+) => \{$", line)
        if m:
            start, label = number + 1, int(m.group(1), 16)
        elif start is not None and re.match(r"\s{12}\}$", line):
            arms.append((label, start, number))
            start = None
    index = {label: i for i, (label, _, _) in enumerate(arms)}
    succ = [
        [
            index[int(t, 16)]
            for t in set(re.findall(r"pc = (0x[0-9a-f]+);", "\n".join(lines[a:b])))
            if int(t, 16) in index
        ]
        for _, a, b in arms
    ]

    def effect(line: str) -> tuple[str | None, set[str]]:
        text = line.strip()
        if re.search(r"\b(call!|call_to!|tail_to!)\(|break 'run", text):
            return None, set(REGS)
        m = re.match(r"(eax|ecx|edx|ebx|esi|edi|ebp) = ([^;]*);(.*)$", text)
        if m:
            used = {r for r in REGS if re.search(rf"\b{r}\b", m.group(2) + m.group(3))}
            return m.group(1), used
        return None, {r for r in REGS if re.search(rf"\b{r}\b", text)}

    live_in = [set() for _ in arms]
    changed = True
    while changed:
        changed = False
        for i in reversed(range(len(arms))):
            live = set().union(*(live_in[j] for j in succ[i])) if succ[i] else set(REGS)
            for number in range(arms[i][2] - 1, arms[i][1] - 1, -1):
                define, used = effect(lines[number])
                if define:
                    live.discard(define)
                live |= used
            if live != live_in[i]:
                live_in[i] = live
                changed = True
    dead: set[int] = set()
    for i, (_, first, last) in enumerate(arms):
        live = set().union(*(live_in[j] for j in succ[i])) if succ[i] else set(REGS)
        for number in range(last - 1, first - 1, -1):
            define, used = effect(lines[number])
            if define and define not in live:
                dead.add(number)
            if define:
                live.discard(define)
            live |= used
    return dead


def mutants(text: str, root_va: int) -> list[tuple[str, str, str]]:
    """Single-site mutations of one translated function: (name, old line, new line) per site.
    Sites: jump conditions (negated and boundary-shifted), constants in memory addresses (+1/-4),
    stores (dropped), wrapping adds/subs (swapped) and register assignments (value changed)."""
    spans = function_spans(text)
    start, end = spans[root_va]
    body = text[start:end]
    dead = dead_register_definitions(body)
    out: list[tuple[str, str, str]] = []
    lines = body.split("\n")
    for number, line in enumerate(lines):
        stripped = line.strip()
        label = f"{root_va:x}:{number}"
        if "cover(" in stripped or stripped.startswith(
            ("pub fn", "#[", "let ", "macro_rules", "*regs", "//")
        ):
            continue
        if m := re.match(r"if (.*) \{ pc = (0x[0-9a-f]+); continue; \}$", stripped):
            cond = m.group(1)
            out.append((f"{label} negate", line, line.replace(f"if {cond} ", f"if !({cond}) ", 1)))
            for old, new in (
                ("==", "!="),
                ("!=", "=="),
                (">=", "<"),
                ("<=", ">"),
                ("<", "<="),
                (">", ">="),
                ("&&", "||"),
                ("||", "&&"),
            ):
                if re.search(rf" {re.escape(old)} ", cond):
                    out.append((f"{label} op {old}", line, line.replace(f" {old} ", f" {new} ", 1)))
        if "m.write32(" in stripped or "m.write8(" in stripped or "write16(m," in stripped:
            out.append((f"{label} drop store", line, re.sub(r"\S.*", "();", line, count=1)))
        for m in re.finditer(r"(0x[0-9a-f]+)(u32)?", stripped):
            token = m.group(1)
            if "pc = " in stripped or token in ("0x0",):
                continue
            value = int(token, 16)
            if value & (value + 1) == 0 or value in (0xFFFFFF00, 0xFFFF0000, 0x80000000):
                continue  # all-ones width/shift masks: mutated by the separate `reg xor1` and operator sites
            new_value = ((value + 4) if value >= 4 else (value + 1)) & 0xFFFFFFFF
            replaced = (
                stripped[: m.start()]
                + f"0x{new_value:x}"
                + (m.group(2) or "")
                + stripped[m.end() :]
            )
            out.append(
                (
                    f"{label} const {token}->{new_value:#x}",
                    line,
                    line.replace(stripped, replaced, 1),
                )
            )
        if (
            ".wrapping_add(" in stripped
            and "m.read" in stripped
            or ".wrapping_add(" in stripped
            and "m.write" in stripped
        ):
            out.append(
                (f"{label} add->sub", line, line.replace(".wrapping_add(", ".wrapping_sub(", 1))
            )
        if m := re.match(r"(eax|ecx|edx|ebx|esi|edi|ebp) = ([^;]*);(.*)$", stripped):
            tag = " [dead]" if number in dead else ""
            out.append(
                (
                    f"{label} reg {m.group(1)} xor1{tag}",
                    line,
                    line.replace(
                        f"{m.group(1)} = {m.group(2)};", f"{m.group(1)} = ({m.group(2)}) ^ 1;", 1
                    ),
                )
            )
    seen = set()
    unique = []
    for name, old, new in out:
        if old == new or (old, new) in seen:
            continue
        seen.add((old, new))
        unique.append((name, old, new))
    return unique


def main() -> int:
    global RUN_TIMEOUT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--gen-dir", type=Path, required=True, help="private lifted generated/lifted/gen"
    )
    parser.add_argument("--out-dir", type=output_path, default=output_path("tmp/t1850"))
    parser.add_argument("--seeds", type=int, default=2000)
    parser.add_argument("--group", choices=sorted(GROUPS), default="pose")
    parser.add_argument(
        "--root", action="append", default=[], help="restrict to this hex VA (repeatable)"
    )
    parser.add_argument(
        "--cover", action="store_true", help="edge coverage run (Rust O0, lifted O0)"
    )
    parser.add_argument(
        "--mutations", action="store_true", help="also run the generated mutation controls (O0/O0)"
    )
    parser.add_argument(
        "--lane",
        action="store_true",
        help="T1874: enter the Rust side through the production lane entry (tsfp_rust_try_lane)",
    )
    parser.add_argument("--run-timeout", type=int, default=RUN_TIMEOUT, help="seconds per exe run")
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--mutation-seeds", type=int, default=4000)
    parser.add_argument("--only", default="", help="run only mutations whose name contains this")
    parser.add_argument(
        "--skip-combos", action="store_true", help="with --mutations: skip the 4 combination runs"
    )
    parser.add_argument(
        "--root-dir",
        type=Path,
        default=None,
        help="tree holding crates/, src/ and tests/c (default: cwd)",
    )
    parser.add_argument(
        "--max-per-root",
        type=int,
        default=0,
        help="cap the mutants per root (evenly spaced sample, 0 = all)",
    )
    parser.add_argument(
        "--names-file",
        type=Path,
        default=None,
        help="run only the mutants whose name is listed (one per line), e.g. the survivors of an earlier run",
    )
    parser.add_argument(
        "--list-mutants", action="store_true", help="print the mutant names and exit"
    )
    args = parser.parse_args()
    RUN_TIMEOUT = args.run_timeout
    if args.lane:
        os.environ["T1850_LANE"] = "1"
    root = (args.root_dir or Path.cwd()).resolve()
    src = root / "crates/tsfp-ports/src"
    roots = tuple(int(v, 16) for v in args.root) or GROUPS[args.group][0]
    source_name = GROUPS[args.group][1]
    failed = False
    if args.list_mutants:
        text = (src / source_name).read_text()
        for va in roots:
            for name, _, _ in mutants(text, va):
                print(name)
        return 0
    if args.cover:
        ok, out = build_and_run(
            root,
            src,
            args.out_dir / "cover",
            args.gen_dir,
            args.group,
            "0",
            "0",
            args.seeds,
            True,
            roots,
        )
        covered, report = edge_report((src / source_name).read_text(), out, roots)
        print(
            f"== coverage run: {'PASS' if ok else 'FAIL'}\n{out.splitlines()[0] if out else ''}\n{report}"
        )
        failed |= not (ok and covered)
    elif not args.skip_combos:
        for lifted_opt in ("0", "3"):
            for rust_opt in ("0", "3"):
                ok, out = build_and_run(
                    root,
                    src,
                    args.out_dir / f"l{lifted_opt}r{rust_opt}",
                    args.gen_dir,
                    args.group,
                    lifted_opt,
                    rust_opt,
                    args.seeds,
                    False,
                    roots,
                )
                first = lifted_opt == "0" and rust_opt == "0"
                lines = out.splitlines()
                shown = (
                    out
                    if (not ok or first)
                    else "\n".join(
                        line
                        for line in lines
                        if " divergences " in line and not line.endswith(" 0") or "DISAGREE" in line
                    )
                )
                looped = sum(loop_skip_report(out)[0].values())
                total = sum(int(m.group(1)) for m in re.finditer(r" divergences (\d+) ", out))
                print(
                    f"== lifted O{lifted_opt} rust O{rust_opt}: {'PASS' if ok else 'FAIL'} ({len(re.findall('^root ', out, re.MULTILINE))} roots, {total} divergences, {looped} watchdog-skipped seeds)\n{shown}"
                )
                failed |= not ok
    if args.mutations:
        text = (src / source_name).read_text()
        wanted = set(args.names_file.read_text().splitlines()) if args.names_file else None
        work_items = []
        for va in roots:
            chosen = [
                item
                for item in mutants(text, va)
                if args.only in item[0] and (wanted is None or item[0] in wanted)
            ]
            if args.max_per_root and len(chosen) > args.max_per_root:
                step = len(chosen) / args.max_per_root
                chosen = [chosen[int(i * step)] for i in range(args.max_per_root)]
            work_items.extend((va, item) for item in chosen)

        def mutate(entry: tuple[int, tuple[int, tuple[str, str, str]]]) -> tuple[str, str, bool]:
            index, (va, (name, old, new)) = entry
            if name.endswith("[dead]"):
                return (
                    name,
                    "EQUIVALENT (dead assignment: the register is overwritten before any read on every path, register liveness)",
                    False,
                )
            work = args.out_dir / f"mut{index}"
            shutil.rmtree(work, ignore_errors=True)
            # sibling crates are reached through `#[path = "../../tsfp-engine-*/..."]`, so the
            # mutated copy keeps the same crates/<name>/src layout with symlinked siblings
            mutated = work / "crates/tsfp-ports/src"
            mutated.parent.mkdir(parents=True)
            for sibling in (root / "crates").iterdir():
                if sibling.name != "tsfp-ports":
                    (work / "crates" / sibling.name).symlink_to(sibling.resolve())
            shutil.copytree(src, mutated)
            target = mutated / source_name
            body = target.read_text()
            if body.count(old) < 1:
                return name, "ANCHOR DRIFTED", False
            # replace only the occurrence inside this function's span
            start, end = function_spans(body)[va]
            at = body.find(old, start, end)
            if at < 0:
                return name, "ANCHOR DRIFTED", False
            target.write_text(body[:at] + new + body[at + len(old) :])
            body_text = function_body(body, va)
            call_free = not re.search(r"\b(call!|call_to!|tail_to!)\(", body_text)
            strict = {"T1850_COMPARE_SCRATCH": "1"} if call_free else None
            ok, output = build_and_run(
                root,
                mutated,
                work,
                args.gen_dir,
                args.group,
                "0",
                "0",
                min(args.seeds, args.mutation_seeds),
                False,
                (va,),
                90,
                strict,
            )
            shutil.rmtree(work, ignore_errors=True)
            first = next((line for line in output.splitlines() if "DISAGREE" in line), "")
            if "build failed" in output:
                return name, "INVALID MUTANT (build failed)", True
            reason = next(
                (
                    why
                    for pattern, why in EQUIVALENT
                    if re.search(pattern, f"{name} | {old.strip()}")
                ),
                None,
            )
            if ok and reason:
                return name, f"EQUIVALENT (explained: {reason})", False
            return name, ("SURVIVED" if ok else "killed") + f" {first}", ok

        survivors = []
        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            for name, verdict, survived in pool.map(mutate, list(enumerate(work_items))):
                print(f"mutation {name}: {verdict}", flush=True)
                if survived or verdict.startswith("ANCHOR"):
                    survivors.append(name)
        failed |= bool(survivors)
        print(f"mutations {len(work_items)} survivors {len(survivors)}: {survivors}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
