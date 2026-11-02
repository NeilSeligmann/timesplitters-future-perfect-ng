# SPDX-License-Identifier: GPL-3.0-or-later
"""Compiled-code digests of port VAs in a capture host (T3839 item 2, I-T3839-4).

t2-continuous re-checked every port VA of a crate (and of its dependents) when any file of the
crate changed (crate_graph). tsfp-ports gains a file nearly every pass, so each pass re-checked
~1800 VAs whose verdict could not move and re-promoted ~727 rows unchanged. A source-level
narrowing cannot bound Rust's name scope here (hub modules, `#[path]` includes, impls of foreign
types), so the dependency is read where it is exact: the machine code the capture host runs.

`digests(host, entries)`: per VA, a SHA-256 over the normalized disassembly of its entry
functions (the `module::fn` symbols the port registry and the dispatch arm name, else every
symbol of the port's module) and of every function they reach by direct call or jump,
transitively. Normalization makes the digest independent of where the linker put things:
direct targets and RIP-relative operands become `symbol+offset` (symbol hashes stripped), data
operands without a symbol contribute their first 16 bytes with pointer-sized words that land in
a symbol named and jump-table words that land in the function made function-relative. Indirect
calls (the offline host's nested-call hook, trait objects) end the walk: the compare stubs those
calls. A VA whose entry symbols are not found gets no digest (the caller keeps it selected).

Confidence INFERRED until the measurement in docs/t3738-capture-design.md "Dependency-precise
selection (T3839)".
"""

from __future__ import annotations

import bisect
import hashlib
import re
import struct
import subprocess
from collections.abc import Iterable
from dataclasses import dataclass, field
from pathlib import Path

DIGEST_VERSION = 1  # bump when normalization changes: every stored digest is then unknown
HASH_SUFFIX = re.compile(r"::h[0-9a-f]{16}(?=$|[^0-9a-f])")
TEXT_TYPES = {"t", "T", "W", "w", "i"}
DATA_PEEK = 16
ARM = re.compile(r"\b0x([0-9A-Fa-f_]+)\s*=>\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(")


@dataclass
class Symbol:
    address: int
    size: int
    name: str  # demangled, hash suffix stripped


@dataclass
class Image:
    """The host ELF: functions by address and a reader over its loaded segments."""

    path: Path
    symbols: list[Symbol] = field(default_factory=list)  # sorted by address, sized only
    starts: list[int] = field(default_factory=list)
    segments: list[tuple[int, int, int, int]] = field(
        default_factory=list
    )  # vaddr, filesz, off, memsz
    _data: bytes = b""

    def at(self, address: int) -> Symbol | None:
        index = bisect.bisect_right(self.starts, address) - 1
        if index >= 0:
            symbol = self.symbols[index]
            if symbol.address <= address < symbol.address + max(symbol.size, 1):
                return symbol
        return None

    def read(self, address: int, size: int) -> bytes:
        for vaddr, filesz, offset, _ in self.segments:
            if vaddr <= address < vaddr + filesz:
                start = offset + address - vaddr
                return self._data[start : start + min(size, vaddr + filesz - address)]
        return b""


def load(path: Path) -> Image:
    """Read the ELF64 program headers and the sized function symbols (`nm -S -C`)."""
    data = path.read_bytes()
    if data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1:
        raise ValueError(f"{path}: not a little-endian ELF64 file")
    (phoff,) = struct.unpack_from("<Q", data, 0x20)
    phentsize, phnum = struct.unpack_from("<HH", data, 0x36)
    segments = []
    for index in range(phnum):
        kind, _, offset, vaddr, _, filesz, memsz, _ = struct.unpack_from(
            "<IIQQQQQQ", data, phoff + index * phentsize
        )
        if kind == 1:  # PT_LOAD
            segments.append((vaddr, filesz, offset, memsz))
    done = subprocess.run(
        ["nm", "--defined-only", "-S", "-C", str(path)],
        capture_output=True, text=True, timeout=600, check=False,
    )  # fmt: skip
    if done.returncode:
        raise ValueError(f"nm {path}: {done.stderr.strip()[-300:]}")
    symbols = {}
    for line in done.stdout.splitlines():
        parts = line.split(" ", 3)
        if len(parts) != 4 or parts[2] not in TEXT_TYPES:
            continue
        address, size = int(parts[0], 16), int(parts[1], 16)
        if size:
            symbols.setdefault(address, Symbol(address, size, HASH_SUFFIX.sub("", parts[3])))
    ordered = sorted(symbols.values(), key=lambda s: s.address)
    image = Image(path, ordered, [s.address for s in ordered], segments)
    image._data = data
    return image


class Digester:
    """Per-function normalized text and callees, memoized; per-VA closure digests."""

    def __init__(self, image: Image) -> None:
        import capstone

        self.image = image
        self.engine = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        self.engine.detail = True
        self._done: dict[int, tuple[str, tuple[int, ...]]] = {}
        self._by_name: dict[str, list[Symbol]] | None = None

    def by_prefix(self, prefixes: Iterable[str]) -> list[Symbol]:
        """Function symbols whose demangled path (generic arguments cut, a leading `<` of an
        impl path dropped) starts with one of the `a::b` prefixes, segment by segment."""
        if self._by_name is None:
            self._by_name = {}
            for symbol in self.image.symbols:
                path = re.split(r"::<|<| as ", symbol.name.lstrip("<"), maxsplit=1)[0]
                segments = path.split("::")
                for count in range(1, len(segments) + 1):
                    self._by_name.setdefault("::".join(segments[:count]), []).append(symbol)
        found: dict[int, Symbol] = {}
        for prefix in prefixes:
            for symbol in self._by_name.get(prefix, []):
                found[symbol.address] = symbol
        return list(found.values())

    def _operand(self, symbol: Symbol, target: int) -> str:
        hit = self.image.at(target)
        if hit is not None:
            return f"{hit.name}+{target - hit.address:#x}"
        words = []
        raw = self.image.read(target, DATA_PEEK)
        for at in range(0, len(raw) - 3, 4):
            (word,) = struct.unpack_from("<i", raw, at)
            if symbol.address <= target + word < symbol.address + symbol.size:
                words.append(f"L{target + word - symbol.address:#x}")  # a jump-table entry
                continue
            words.append(raw[at : at + 4].hex())
        for at in range(0, len(raw) - 7, 8):
            (pointer,) = struct.unpack_from("<Q", raw, at)
            hit = self.image.at(pointer)
            if hit is not None:
                words[at // 4 : at // 4 + 2] = [f"{hit.name}+{pointer - hit.address:#x}", ""]
        return "data[" + ",".join(w for w in words if w) + "]"

    def function(self, symbol: Symbol) -> tuple[str, tuple[int, ...]]:
        """(sha256 of the normalized text, direct callee addresses) of one function."""
        if symbol.address in self._done:
            return self._done[symbol.address]
        from capstone import x86

        code = self.image.read(symbol.address, symbol.size)
        lines = []
        callees: set[int] = set()
        for insn in self.engine.disasm(code, symbol.address):
            ops = []
            for op in insn.operands:
                if op.type == x86.X86_OP_IMM:
                    value = op.imm
                    if insn.group(1) or insn.group(2) or insn.group(7):  # jump, call, rel branch
                        if symbol.address <= value < symbol.address + symbol.size:
                            ops.append(f"L{value - symbol.address:#x}")
                            continue
                        hit = self.image.at(value)
                        if hit is not None:
                            callees.add(hit.address)
                            ops.append(f"{hit.name}+{value - hit.address:#x}")
                            continue
                    ops.append(hex(value))
                elif op.type == x86.X86_OP_MEM:
                    if op.mem.base == x86.X86_REG_RIP:
                        target = insn.address + insn.size + op.mem.disp
                        ops.append(f"[{self._operand(symbol, target)}]")
                    else:
                        ops.append(
                            f"[{insn.reg_name(op.mem.base) if op.mem.base else ''}"
                            f"+{insn.reg_name(op.mem.index) if op.mem.index else ''}"
                            f"*{op.mem.scale}+{op.mem.disp:#x}:{op.size}]"
                        )
                elif op.type == x86.X86_OP_REG:
                    ops.append(insn.reg_name(op.reg))
            lines.append(f"{insn.mnemonic} {','.join(ops)}")
        digest = hashlib.sha256("\n".join(lines).encode()).hexdigest()
        self._done[symbol.address] = (digest, tuple(sorted(callees)))
        return self._done[symbol.address]

    def closure(self, entries: list[Symbol]) -> str:
        """SHA-256 over (name, digest) of every function reachable from `entries`."""
        seen: dict[int, Symbol] = {}
        stack = [s.address for s in entries]
        by_address = {s.address: s for s in entries}
        while stack:
            address = stack.pop()
            if address in seen:
                continue
            symbol = by_address.get(address) or self.image.at(address)
            if symbol is None:
                continue
            seen[address] = symbol
            _, callees = self.function(symbol)
            stack.extend(c for c in callees if c not in seen)
        rows = sorted(f"{s.name}\t{self.function(s)[0]}" for s in seen.values())
        return hashlib.sha256(
            f"v{DIGEST_VERSION}\n".encode() + "\n".join(rows).encode()
        ).hexdigest()


def digests(host: Path, entries: dict[str, list[str]]) -> dict[str, str]:
    """VA -> closure digest over the union of the symbols of its entries. An entry is
    `fn-path|module-path` alternatives, the first that names symbols wins (an inlined function
    has none: its module's dispatch `run` holds its code). A VA with an entry that names no
    symbol gets no digest (its code is not where the registry says)."""
    digester = Digester(load(host))
    out = {}
    for va, prefixes in entries.items():
        symbols: list[Symbol] = []
        for entry in prefixes:
            found = next(
                (hit for hit in (digester.by_prefix([p]) for p in entry.split("|")) if hit), []
            )
            if not found:
                symbols = []
                break
            symbols += found
        if symbols:
            out[va] = digester.closure(symbols)
    return out


def lib_name(root: Path, crate_dir: str) -> str | None:
    """The Rust path name of a crate (`[lib] name`, else the package name, `-` as `_`)."""
    import tomllib

    try:
        manifest = tomllib.loads((root / crate_dir / "Cargo.toml").read_text())
    except (OSError, ValueError):
        return None
    name = (manifest.get("lib") or {}).get("name") or (manifest.get("package") or {}).get("name")
    return str(name).replace("-", "_") if name else None


def module_path(root: Path, source: str) -> str | None:
    """`crates/x-y/src/a/b.rs` -> `x_y::a::b` (`mod.rs` is its directory, `lib.rs` the crate
    root); None outside `src/` (bins, build scripts, `#[path]` includes under another root)."""
    if "/src/" not in source or not source.endswith(".rs"):
        return None
    crate_dir, inner = source.split("/src/", 1)
    name = lib_name(root, crate_dir)
    if name is None:
        return None
    parts = inner[: -len(".rs")].split("/")
    if parts[-1] == "mod":
        parts = parts[:-1]
    if parts in (["lib"], ["main"]) or (parts and parts[0] == "bin"):
        return name if parts == ["lib"] else None
    return "::".join([name, *parts])


def entry_prefixes(
    root: Path, registry: dict, only: set[str] | None = None
) -> dict[str, list[str]]:
    """VA -> symbol path prefixes of its port code: per registry entry the dispatch arm's
    function (`0x<va> => name(`, through a `pub use path::name` re-export when there is one),
    else the entry's module; crate-root entries only when the VA has no other entry. `only`:
    the 0x%08X VAs wanted (sources of the others are not read)."""
    texts: dict[str, str] = {}
    arms: dict[str, dict[int, str]] = {}  # source -> VA -> dispatch arm function
    out: dict[str, list[str]] = {}
    for key, rows in (registry.get("ports") or {}).items():
        va = int(str(key), 16)
        if only is not None and f"0x{va:08X}" not in only:
            continue
        prefixes: list[str] = []
        roots: list[str] = []
        for row in rows:
            source = str(row.get("source") or "")
            module = module_path(root, source)
            if module is None:
                prefixes = []
                roots = ["?"]
                break
            if source.endswith("/src/lib.rs"):
                roots.append(module)
                continue
            if source not in texts:
                try:
                    texts[source] = (root / source).read_text(errors="replace")
                except OSError:
                    texts[source] = ""
                arms[source] = {}
                for value, name in ARM.findall(texts[source]):
                    arms[source].setdefault(int(value, 16), name)
            function = arms[source].get(va)
            if function is None:
                prefixes.append(module)
                continue
            used = re.search(rf"\bpub\s+use\s+([A-Za-z_][\w:]*)::{function}\s*;", texts[source])
            target = f"{used.group(1)}::{function}" if used else f"{module}::{function}"
            prefixes.append(f"{target}|{module}")
        if "?" in roots:
            continue
        chosen = prefixes or roots
        if chosen:
            out[f"0x{va:08X}"] = sorted(set(chosen))
    return out


def host_binary(host: Path) -> Path:
    """The capture host ELF: the prebuilt dir's `tsfp-bevy`, or the path itself."""
    return host / "tsfp-bevy" if host.is_dir() else host


def main(argv: list[str] | None = None) -> int:
    import argparse
    import json

    parser = argparse.ArgumentParser(prog="python3 -m tools.t2capture.code_digest")
    parser.add_argument("--host", type=Path, required=True, help="capture host (dir or ELF)")
    parser.add_argument("--registry", type=Path, default=Path("docs/data/port-registry.json"))
    parser.add_argument("--vas", type=Path, help="JSON list of VAs (default: every port VA)")
    parser.add_argument("--out", type=Path, required=True, help="JSON {va: digest}")
    args = parser.parse_args(argv)
    wanted = None
    if args.vas is not None:
        wanted = {f"0x{int(str(va), 16):08X}" for va in json.loads(args.vas.read_text())}
    entries = entry_prefixes(Path("."), json.loads(args.registry.read_text()), wanted)
    found = digests(host_binary(args.host), entries)
    doc = {"version": DIGEST_VERSION, "vas": dict(sorted(found.items()))}
    args.out.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n")
    print(f"code_digest: {len(found)} of {len(entries)} VA(s) digested")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
