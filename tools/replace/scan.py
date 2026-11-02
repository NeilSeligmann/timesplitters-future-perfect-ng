# SPDX-License-Identifier: GPL-3.0-or-later
"""Find the `GAME_REPLACE(...)` registrations in src/game/*.c.

A source scan is used ONLY to decide which lifted symbols must be made weak before the
lifted chunks are compiled, which has to happen before any binary containing the
registry exists. It is never evidence that a replacement exists: that is the registry
dumped from a linked binary (`tools/replace/manifest.py`), and the two are cross-checked
so a registration hidden behind a comment or an `#if 0` shows up as a disagreement.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

from .input_abi import INPUT_SETS, validate_inputs

CONVENTIONS = ("cdecl", "stdcall", "thiscall", "fastcall")
RETURNS = ("u32", "void")

#: The eight hex digits are UPPER CASE because they are pasted into the lifter's symbol
#: name, which is upper case (`sub_0003C330`). Matching only upper case is deliberate: a
#: lower-case registration would compile, define a symbol nothing calls, and silently
#: replace nothing.
REGISTRATION = re.compile(
    r"\bGAME_REPLACE(?P<mode>_EXACT_INPUTS|_EXACT)?\(\s*(?P<va>[0-9A-F]{8})\s*,\s*(?P<cc>"
    + "|".join(CONVENTIONS)
    + r")\s*,\s*(?P<count>\d+)\s*,\s*(?P<ret>"
    + "|".join(RETURNS)
    + r")\s*,\s*(?:(?P<inputs>ecx_esi|ecx|esi)\s*,\s*)?(?P<fn>[A-Za-z_]\w*)\s*\)"
)
ANY_REGISTRATION = re.compile(r"\bGAME_REPLACE(?:_EXACT(?:_INPUTS)?)?\s*\(")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.DOTALL)
LINE_COMMENT = re.compile(r"//[^\n]*")
#: The macro's own definition lives in game_replace.h and its usage example sits in a
#: comment, so only `.c` files are scanned and comments are removed first.
SOURCE_SUFFIX = ".c"
#: `game_manifest.c` and `game_registry.c` are infrastructure, never registrations.
INFRASTRUCTURE = frozenset({"game_manifest.c", "game_registry.c"})


#: Build-variant switches (T2048 opt-in Rust allocator paths, `-DTSFP_RUST_GAME_*`). A
#: conditional made only of `defined(TSFP_RUST_GAME_*)` tests, `!`, `&&`, `||` and parentheses
#: is evaluated against the build's defines, so a registration in a variant block that the
#: default build does not compile is not reported as hidden. Every other conditional,
#: `#if 0` included, stays transparent: the scan keeps seeing it, so a registration hidden
#: there still shows up as a disagreement with the linked registry.
DIRECTIVE = re.compile(r"^[ \t]*#[ \t]*(if|ifdef|ifndef|elif|else|endif)\b(.*)$")
CONDITION_TOKEN = re.compile(r"\s*(defined\s*\(\s*TSFP_RUST_GAME_\w+\s*\)|&&|\|\||!|\(|\))")


def _variant_value(condition: str, defines: frozenset[str]) -> bool | None:
    """Value of a build-variant condition, or None when it is not purely a variant test."""
    tokens: list[str] = []
    position = 0
    condition = condition.strip()
    while position < len(condition):
        match = CONDITION_TOKEN.match(condition, position)
        if match is None:
            return None
        tokens.append(re.sub(r"\s+", "", match.group(1)))
        position = match.end()
        while position < len(condition) and condition[position].isspace():
            position += 1
    if not tokens:
        return None
    index = 0

    def parse_or() -> bool:
        nonlocal index
        value = parse_and()
        while index < len(tokens) and tokens[index] == "||":
            index += 1
            value = parse_and() or value
        return value

    def parse_and() -> bool:
        nonlocal index
        value = parse_not()
        while index < len(tokens) and tokens[index] == "&&":
            index += 1
            value = parse_not() and value
        return value

    def parse_not() -> bool:
        nonlocal index
        if index >= len(tokens):
            raise ScanError(f"unparsable variant condition: {condition!r}")
        token = tokens[index]
        index += 1
        if token == "!":
            return not parse_not()
        if token == "(":
            value = parse_or()
            if index >= len(tokens) or tokens[index] != ")":
                raise ScanError(f"unbalanced variant condition: {condition!r}")
            index += 1
            return value
        if token.startswith("defined("):
            return token[len("defined(") : -1] in defines
        raise ScanError(f"unexpected {token!r} in variant condition: {condition!r}")

    value = parse_or()
    if index != len(tokens):
        raise ScanError(f"unparsable variant condition: {condition!r}")
    return value


def strip_variant_blocks(code: str, defines: frozenset[str] = frozenset()) -> str:
    """Blank the lines of `TSFP_RUST_GAME_*` variant blocks that `defines` does not compile."""
    # A frame is (variant conditional?, current branch active, a branch was taken, parent active).
    stack: list[tuple[bool, bool, bool, bool]] = []
    out: list[str] = []
    active = True
    for line in code.split("\n"):
        match = DIRECTIVE.match(line)
        if match is None:
            out.append(line if active else re.sub(r"\S", " ", line))
            continue
        kind, rest = match.group(1), match.group(2)
        out.append(line)
        if kind in ("if", "ifdef", "ifndef"):
            value = _variant_value(rest, defines) if kind == "if" else None
            variant = value is not None
            branch = value if value is not None else True
            stack.append((variant, branch, branch, active))
            active = active and branch
        elif kind == "endif" and stack:
            active = stack.pop()[3]
        elif kind in ("elif", "else") and stack and stack[-1][0]:
            _, _, taken, parent = stack.pop()
            value = True if kind == "else" else bool(_variant_value(rest, defines))
            branch = value and not taken
            stack.append((True, branch, taken or value, parent))
            active = parent and branch
    return "\n".join(out)


class ScanError(ValueError):
    """A registration that looks like one but cannot be accepted."""


@dataclass(frozen=True)
class Registration:
    va: int
    convention: str
    stack_args: int
    returns: str
    function: str
    source: str
    exact_registers: bool = False
    register_inputs: tuple[str, ...] | None = None

    @property
    def symbol(self) -> str:
        return f"sub_{self.va:08X}"


def strip_comments(text: str) -> str:
    """Source with comments blanked, keeping line structure."""

    def blank(match: re.Match[str]) -> str:
        return re.sub(r"[^\n]", " ", match.group(0))

    return LINE_COMMENT.sub(blank, BLOCK_COMMENT.sub(blank, text))


def scan_text(text: str, source: str, defines: frozenset[str] = frozenset()) -> list[Registration]:
    code = strip_variant_blocks(strip_comments(text), defines)
    found = []
    for match in REGISTRATION.finditer(code):
        explicit = match["mode"] == "_EXACT_INPUTS"
        if explicit != (match["inputs"] is not None):
            raise ScanError(f"{source}: missing or conflicting explicit input metadata")
        inputs = INPUT_SETS[match["inputs"]] if explicit else None
        try:
            validate_inputs(inputs, match["cc"], int(match["count"]), ())
        except ValueError as error:
            raise ScanError(f"{source}: {error}") from error
        found.append(
            Registration(
                va=int(match["va"], 16),
                convention=match["cc"],
                stack_args=int(match["count"]),
                returns=match["ret"],
                function=match["fn"],
                source=source,
                exact_registers=match["mode"] is not None,
                register_inputs=inputs,
            )
        )
    uses = len(ANY_REGISTRATION.findall(code))
    if uses != len(found):
        raise ScanError(
            f"{source}: {uses} GAME_REPLACE use(s) but {len(found)} well-formed. A VA must be "
            "eight UPPER CASE hex digits without 0x, then a convention, a stack argument "
            "count, u32 or void, and the function name."
        )
    return found


def scan_directory(game_dir: Path, defines: frozenset[str] = frozenset()) -> list[Registration]:
    """Every registration in `game_dir`, sorted by address, duplicates refused.

    `defines` are the `TSFP_RUST_GAME_*` variant switches the scanned build compiles with.
    The default (none) is the default build that the default manifest binary reflects."""
    found: list[Registration] = []
    for path in sorted(game_dir.glob(f"*{SOURCE_SUFFIX}")):
        if path.name in INFRASTRUCTURE:
            continue
        found.extend(scan_text(path.read_text(encoding="utf-8"), path.name, defines))
    # T1510: registrations in an `.inc` file are scanned too (x87_roots.inc is included by
    # x87_roots.c unconditionally, so it is part of every build's registry).
    for path in sorted(game_dir.glob("*.inc")):
        found.extend(scan_text(path.read_text(encoding="utf-8"), path.name, defines))
    seen: dict[int, Registration] = {}
    for registration in found:
        if registration.va in seen:
            other = seen[registration.va]
            raise ScanError(
                f"{registration.va:#010x} is registered twice: {other.source} "
                f"({other.function}) and {registration.source} ({registration.function}). "
                "Two strong definitions of one symbol cannot both link."
            )
        seen[registration.va] = registration
    return sorted(found, key=lambda registration: registration.va)


VARIANT_NAME = re.compile(r"TSFP_RUST_GAME_\w+")


def scan_any_variant(game_dir: Path) -> list[Registration]:
    """Every registration that at least one build variant of `game_dir` compiles, by address.

    The default scan hides the registrations of the `TSFP_RUST_GAME_*` variant blocks (T3596). A
    build step that must treat a root any variant replaces as taken needs this view: the wiring that
    weakens the lifted bodies (`tools.replace wire`) and the generators that skip roots src/game
    already replaces (the rust adapters, weapon, HUD gate and UI draw hooks). With the default scan
    the gate prebuilt (`TSFP_BUILD_GAME_ARENA=1`, ...) failed to link: the arena, pool and ledger
    twins were neither weakened in the lifted chunks nor skipped by the adapter generators, 20
    duplicate `sub_<VA>` symbols (T3600).
    """
    names = frozenset(
        name
        for path in (*game_dir.glob(f"*{SOURCE_SUFFIX}"), *game_dir.glob("*.inc"))
        for name in VARIANT_NAME.findall(path.read_text(encoding="utf-8"))
    )
    merged: dict[int, Registration] = {}
    for defines in (frozenset(), names):
        for registration in scan_directory(game_dir, defines):
            merged.setdefault(registration.va, registration)
    return sorted(merged.values(), key=lambda registration: registration.va)


def registered_in_any_variant(game_dir: Path) -> set[int]:
    """Addresses that at least one build variant of `game_dir` registers."""
    return {registration.va for registration in scan_any_variant(game_dir)}
