#!/usr/bin/env python3
# ruff: noqa: E501
"""T1879 pure logic of the Rust trace oracle: dump parsing, trace comparison, noise masks, delta-debugging bisect, reports.

No host, no I/O beyond reading dump files, so every function is unit tested on synthetic traces (tests/test_rust_trace.py).
The capture and CLI live in tools/rust_trace.py.

Trace model: a trace is a list of samples, each sample is `{"key": str, "poll": int, "regions": {name: bytes}}`.
Samples of two runs are matched by key (the host's deterministic poll based dump events), the shorter run bounds the compare.
Evidence class of anything built on this: MEASURED retained-original versus Rust A/B at the sampled regions only.
"""

from __future__ import annotations

import math
import re
import struct
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

HEX_LINE = re.compile(r"^([0-9A-Fa-f]{8}):((?: [0-9A-Fa-f]{2})+)\s*$")


@dataclass(frozen=True)
class Region:
    """One configured region: `spec` is the host --dump-guest-range grammar, `float_ranges` are (offset, dword count) read as f32."""

    name: str
    spec: str
    float_ranges: tuple[tuple[int, int], ...] = ()
    ignore_ranges: tuple[
        tuple[int, int], ...
    ] = ()  # (offset, byte count) never compared (known nondeterminism)


def parse_int(value: object) -> int:
    return value if isinstance(value, int) else int(str(value), 0)


def array_specs(array: dict) -> list[str]:
    """Expand a bounded array into existing host ranges; no new dump wire format."""
    base, stride, count, length = (
        parse_int(array[key]) for key in ("base", "stride", "count", "length")
    )
    if not 1 <= count <= 16:
        raise ValueError("array count must be 1 to 16")
    if not 0 <= base < 2**32 or not 0 <= stride < 2**32:
        raise ValueError("array base and stride must be unsigned 32-bit values")
    if not 1 <= length <= 0x04000000:
        raise ValueError("array length must be 1 to 64 MiB")
    offsets = array.get("offsets")
    if offsets is not None:
        offsets = [parse_int(offset) for offset in offsets]
        if not 1 <= len(offsets) <= 4:
            raise ValueError("array pointer chain needs 1 to 4 offsets")
        if any(not 0 <= offset <= 0x04000000 for offset in offsets):
            raise ValueError("array pointer offsets must be 0 to 64 MiB")
        if offsets[-1] + length > 0x04000000:
            raise ValueError("array final offset+length exceeds 64 MiB")
    specs = []
    for index in range(count):
        address = base + index * stride
        if offsets is None:
            if address + length > 2**32:
                raise ValueError("array target range wraps past 0xFFFFFFFF")
            specs.append(f"=0x{address:X}:0x{length:X}")
        else:
            if address + 4 > 0x04000000:
                raise ValueError("array pointer slot leaves the low 64 MiB")
            suffix = "".join(f"+0x{offset:X}" for offset in offsets)
            specs.append(f"{'*' * len(offsets)}0x{address:X}{suffix}:0x{length:X}")
    return specs


def load_regions(config: dict) -> list[Region]:
    """Expand optional arrays; enforce host's 16-range limit and unique names."""
    regions = []
    for item in config["regions"]:
        if ("spec" in item) == ("array" in item):
            raise ValueError("region needs exactly one of spec or array")
        specs = array_specs(item["array"]) if "array" in item else [item["spec"]]
        for index, spec in enumerate(specs):
            regions.append(
                Region(
                    name=f"{item['name']}[{index}]" if "array" in item else item["name"],
                    spec=spec,
                    float_ranges=tuple(
                        (parse_int(o), parse_int(n)) for o, n in item.get("float_ranges", [])
                    ),
                    ignore_ranges=tuple(
                        (parse_int(o), parse_int(n)) for o, n in item.get("ignore_ranges", [])
                    ),
                )
            )
    names = [region.name for region in regions]
    if len(names) != len(set(names)):
        raise ValueError("duplicate region names")
    if not 1 <= len(regions) <= 1024:
        raise ValueError("the host dumps 1 to 1024 ranges")
    return regions


def parse_dump(text: str, names: Sequence[str]) -> dict[str, bytes]:
    """Region bytes of one guestdump file, in the order of the configured ranges (`range A N` header then hex lines).

    An unreadable or null pointer range has no `range` header body and yields b"" for that region.
    """
    blocks: list[bytearray] = []
    pending = (
        False  # an `indirect` header whose `range` line (and so its block) has not been seen yet
    )
    for line in text.splitlines():
        if line.startswith("indirect"):
            blocks.append(bytearray())
            pending = True
        elif line.startswith("unreadable"):
            pending = False
        elif line.startswith("range "):
            if pending:
                pending = False
            else:
                blocks.append(bytearray())
        else:
            match = HEX_LINE.match(line)
            if match and blocks:
                blocks[-1].extend(int(byte, 16) for byte in match.group(2).split())
    if len(blocks) != len(names):
        raise ValueError(f"dump has {len(blocks)} ranges, config has {len(names)}")
    return {name: bytes(block) for name, block in zip(names, blocks, strict=True)}


@dataclass
class Policy:
    """Tolerance policy.  ints exact, floats within max(abs_eps, rel_eps * |a|); NaN equals NaN; masked words skipped."""

    abs_eps: float = 1e-4
    rel_eps: float = 1e-4
    max_samples_missing: int = (
        0  # allowed difference in sample count (a longer run is only compared on the common prefix)
    )
    noise: dict[str, set[int]] = field(
        default_factory=dict
    )  # region -> dword indexes nondeterministic in most samples (heap pointers)
    noise_samples: dict[str, dict[str, set[int]]] = field(
        default_factory=dict
    )  # sample key -> region -> jitter words of that sample


def float_close(left: float, right: float, policy: Policy) -> bool:
    if math.isnan(left) or math.isnan(right):
        return math.isnan(left) and math.isnan(right)
    if math.isinf(left) or math.isinf(right):
        return left == right
    return abs(left - right) <= max(policy.abs_eps, policy.rel_eps * abs(left))


def region_diffs(
    region: Region, left: bytes, right: bytes, policy: Policy, key: str = ""
) -> list[dict]:
    """Word level differences of one region under the policy (list of {offset, left, right, kind})."""
    diffs: list[dict] = []
    if len(left) != len(right):
        return [{"offset": 0, "kind": "length", "left": len(left), "right": len(right)}]
    floats = set()
    for offset, count in region.float_ranges:
        floats.update(range(offset // 4, offset // 4 + count))
    ignored = set()
    for offset, count in region.ignore_ranges:
        ignored.update(range(offset // 4, (offset + count + 3) // 4))
    noise = policy.noise.get(region.name, set()) | policy.noise_samples.get(key, {}).get(
        region.name, set()
    )
    for index in range(len(left) // 4):
        if index in ignored or index in noise:
            continue
        a_bytes = left[index * 4 : index * 4 + 4]
        b_bytes = right[index * 4 : index * 4 + 4]
        if a_bytes == b_bytes:
            continue
        if index in floats:
            a_val = struct.unpack("<f", a_bytes)[0]
            b_val = struct.unpack("<f", b_bytes)[0]
            if float_close(a_val, b_val, policy):
                continue
            diffs.append({"offset": index * 4, "kind": "f32", "left": a_val, "right": b_val})
        else:
            diffs.append(
                {
                    "offset": index * 4,
                    "kind": "u32",
                    "left": struct.unpack("<I", a_bytes)[0],
                    "right": struct.unpack("<I", b_bytes)[0],
                }
            )
    return diffs


def compare_traces(
    regions: Sequence[Region],
    left: list[dict],
    right: list[dict],
    policy: Policy,
    max_listed: int = 8,
) -> dict:
    """Compare two traces sample by sample (matched by key).  Returns the verdict and the first divergence."""
    left_by_key = {sample["key"]: sample for sample in left}
    right_by_key = {sample["key"]: sample for sample in right}
    keys = [sample["key"] for sample in left if sample["key"] in right_by_key]
    result: dict = {
        "samples_left": len(left),
        "samples_right": len(right),
        "samples_compared": len(keys),
        "equal": True,
        "first_divergence": None,
        "diverging_samples": 0,
        "diverging_regions": {},
        "reason": "",
    }
    if abs(len(left) - len(right)) > policy.max_samples_missing and not keys:
        result["equal"] = False
        result["reason"] = "no common samples"
        return result
    if not keys:
        result["equal"] = False
        result["reason"] = "no common samples (empty trace is not acceptance)"
        return result
    for key in keys:
        a_sample, b_sample = left_by_key[key], right_by_key[key]
        sample_diverged = False
        for region in regions:
            diffs = region_diffs(
                region,
                a_sample["regions"].get(region.name, b""),
                b_sample["regions"].get(region.name, b""),
                policy,
                key,
            )
            if not diffs:
                continue
            sample_diverged = True
            result["diverging_regions"][region.name] = (
                result["diverging_regions"].get(region.name, 0) + 1
            )
            if result["first_divergence"] is None:
                result["first_divergence"] = {
                    "key": key,
                    "poll": a_sample.get("poll"),
                    "region": region.name,
                    "word_count": len(diffs),
                    "words": diffs[:max_listed],
                }
        result["diverging_samples"] += sample_diverged
    result["equal"] = result["diverging_samples"] == 0
    if not result["equal"]:
        result["reason"] = "state divergence"
    return result


def noise_masks(
    regions: Sequence[Region], first: list[dict], second: list[dict], global_fraction: float = 0.5
) -> tuple[dict[str, set[int]], dict[str, dict[str, set[int]]]]:
    """Nondeterministic words between two runs of the SAME configuration (T1858/T1860 jitter, host heap pointers).

    A word that differs in at least `global_fraction` of the compared samples is global noise (a pointer into the host heap);
    a word that differs in fewer samples is masked only in those samples (poll jitter), so a real divergence elsewhere stays visible.
    """
    second_by_key = {sample["key"]: sample for sample in second}
    counts: dict[str, dict[int, int]] = {}
    per_sample: dict[str, dict[str, set[int]]] = {}
    compared = 0
    for sample in first:
        other = second_by_key.get(sample["key"])
        if other is None:
            continue
        compared += 1
        for region in regions:
            a_bytes = sample["regions"].get(region.name, b"")
            b_bytes = other["regions"].get(region.name, b"")
            for index in range(min(len(a_bytes), len(b_bytes)) // 4):
                if a_bytes[index * 4 : index * 4 + 4] != b_bytes[index * 4 : index * 4 + 4]:
                    counts.setdefault(region.name, {}).setdefault(index, 0)
                    counts[region.name][index] += 1
                    per_sample.setdefault(sample["key"], {}).setdefault(region.name, set()).add(
                        index
                    )
    global_mask: dict[str, set[int]] = {}
    for name, words in counts.items():
        for index, count in words.items():
            if count >= global_fraction * compared:
                global_mask.setdefault(name, set()).add(index)
    local = {
        key: {name: indexes - global_mask.get(name, set()) for name, indexes in regs.items()}
        for key, regs in per_sample.items()
    }
    return global_mask, local


def noise_masks_multi(
    regions: Sequence[Region], runs: Sequence[list[dict]], global_fraction: float = 0.5
) -> tuple[dict[str, set[int]], dict[str, dict[str, set[int]]]]:
    """Volatile words learned across 3+ runs of the SAME configuration (T2067).

    The first run is compared with every other run and the masks are unioned, so a word that varies in any pair
    (for example a per-run counter such as player_stats+0x12c) is masked. A word global in any pair stays global,
    the per-sample jitter of the rest is unioned per sample key. With two runs this equals `noise_masks`.
    """
    global_mask: dict[str, set[int]] = {}
    local: dict[str, dict[str, set[int]]] = {}
    for other in runs[1:]:
        g, per = noise_masks(regions, runs[0], other, global_fraction)
        for name, words in g.items():
            global_mask.setdefault(name, set()).update(words)
        for key, regs in per.items():
            for name, words in regs.items():
                local.setdefault(key, {}).setdefault(name, set()).update(words)
    local = {
        key: {name: words - global_mask.get(name, set()) for name, words in regs.items()}
        for key, regs in local.items()
    }
    return global_mask, local


def noise_mask(
    regions: Sequence[Region], first: list[dict], second: list[dict]
) -> dict[str, set[int]]:
    """Every differing dword index (no fraction split), kept for tests and callers that want the union."""
    mask: dict[str, set[int]] = {}
    for name, per in noise_masks(regions, first, second, global_fraction=0.0)[0].items():
        mask[name] = set(per)
    return mask


def ddmin(items: Sequence[str], fails: Callable[[Sequence[str]], bool]) -> list[str]:
    """Zeller delta debugging: a 1-minimal subset of `items` for which `fails(subset)` is still true.

    `fails(items)` must hold for the full set.  Each probe is one host run, so the caller memoises through the callback.
    """
    current = list(items)
    granularity = 2
    while len(current) >= 2:
        chunk = max(1, math.ceil(len(current) / granularity))
        subsets = [current[i : i + chunk] for i in range(0, len(current), chunk)]
        reduced = False
        for subset in subsets:
            if fails(subset):
                current, granularity, reduced = subset, 2, True
                break
        if not reduced:
            for subset in subsets:
                complement = [item for item in current if item not in subset]
                if complement and fails(complement):
                    current, granularity, reduced = complement, max(granularity - 1, 2), True
                    break
        if not reduced:
            if granularity >= len(current):
                break
            granularity = min(len(current), granularity * 2)
    return current


def parse_counters(report: str) -> dict[str, dict[str, int]]:
    """`rust port 0x00012820 rust=N lifted=M` lines of the stop report -> {hex va: {rust, lifted}}.

    `rust ext port` lines (typed ext bridge roots such as the frame-loop roots) are summed in; a root that is
    both a lane and an ext root prints one line of each.
    """
    counters: dict[str, dict[str, int]] = {}
    for match in re.finditer(
        r"rust (?:ext )?port\s+0x([0-9A-Fa-f]+)\s+rust=(\d+)\s+lifted=(\d+)", report
    ):
        entry = counters.setdefault(f"{int(match.group(1), 16):08X}", {"rust": 0, "lifted": 0})
        entry["rust"] += int(match.group(2))
        entry["lifted"] += int(match.group(3))
    return counters


def reached_roots(counters: dict[str, dict[str, int]]) -> list[str]:
    return sorted(va for va, count in counters.items() if count["rust"] > 0)


def render_markdown(report: dict) -> str:
    lines = [f"# Rust trace oracle: {report['title']}", ""]
    lines.append(f"- verdict: **{'EQUAL within tolerance' if report['equal'] else 'DIVERGED'}**")
    lines.append(f"- left: `{report['left_label']}`  right: `{report['right_label']}`")
    compared = report["comparison"]
    lines.append(
        f"- samples: left {compared['samples_left']}, right {compared['samples_right']}, compared {compared['samples_compared']}"
    )
    lines.append(f"- noise words masked: {report.get('noise_words', 0)}")
    first = compared["first_divergence"]
    if first:
        lines += ["", "## First divergence", ""]
        lines.append(
            f"sample `{first['key']}` (poll {first['poll']}), region `{first['region']}`, {first['word_count']} word(s)"
        )
        lines.append("")
        lines.append("| offset | kind | left | right |")
        lines.append("|---|---|---|---|")
        for word in first["words"]:
            left, right = word["left"], word["right"]
            if word["kind"] == "u32":
                left, right = f"{left:#010x}", f"{right:#010x}"
            lines.append(f"| {word['offset']:#x} | {word['kind']} | {left} | {right} |")
        lines.append("")
        lines.append(
            "diverging samples per region: "
            + ", ".join(f"{k}={v}" for k, v in compared["diverging_regions"].items())
        )
    attribution = report.get("attribution")
    if attribution:
        lines += ["", "## Root attribution (delta debugging over --rust-ports)", ""]
        lines.append(
            f"- offending root(s): {', '.join('0x' + root for root in attribution['roots'])}"
        )
        lines.append(f"- probes (host runs): {attribution['probes']}")
        lines.append(f"- candidate set size: {attribution['candidates']}")
    counters = report.get("counters")
    if counters:
        lines += ["", "## Rust call counters (right run)", ""]
        reached = reached_roots(counters)
        lines.append(
            f"{len(reached)} of {len(counters)} roots reached: "
            + ", ".join(f"{va}={counters[va]['rust']}" for va in reached)
        )
    for note in report.get("notes", []):
        lines.append(f"\nNote: {note}")
    return "\n".join(lines) + "\n"


def read_dump_file(path: Path, names: Sequence[str]) -> dict[str, bytes]:
    return parse_dump(path.read_text(errors="replace"), names)
