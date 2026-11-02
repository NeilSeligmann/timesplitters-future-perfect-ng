# SPDX-License-Identifier: GPL-3.0-or-later
"""Rust source quality inventory and non-growing lint/format gates (T1959)."""

import argparse
import hashlib
import json
import os
import re
import subprocess
import tomllib
from bisect import bisect_right
from collections import Counter
from pathlib import Path
from typing import Any

from tools.generated_text import write_lf_text

ROOT = Path(__file__).resolve().parents[1]
BASELINE = Path("tools/data/rust_quality_allowlist.json")
SUMMARY = Path("docs/t-rust-quality.md")
# Lexical estimates, deliberately not advertised as Rust AST metrics. Strings and
# comments are tokens so braces/keywords inside them cannot change the estimates.
TOKEN = re.compile(
    r'//[^\n]*|/\*[\s\S]*?\*/|r(?P<hash>\#*)"[\s\S]*?"(?P=hash)|'
    r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'|[A-Za-z_][A-Za-z_0-9]*|'
    r"0x[\da-fA-F_]+|\d[\w.]*|&&|\|\||=>|[^\s]"
)


def tokens(source: str) -> list[tuple[str, int]]:
    return [
        (m.group(), m.start())
        for m in TOKEN.finditer(source)
        if not m.group().startswith(("//", "/*"))
    ]


def inspect_source(source: str) -> dict[str, Any]:
    newlines = [m.start() for m in re.finditer("\n", source)]
    ts = tokens(source)
    words = [t for t, _ in ts]
    macros = Counter(
        words[i]
        for i in range(len(words) - 1)
        if words[i + 1] == "!"
        and i + 2 < len(words)
        and words[i + 2] in ("(", "[", "{")
        and re.fullmatch(r"\w+", words[i])
    )
    unsafe = sum(words[i : i + 2] == ["unsafe", "{"] for i in range(len(words)))
    raw = sum(bool(re.fullmatch(r"0x[\da-fA-F_]+", w)) for w in words)
    functions = []
    for i, word in enumerate(words):
        if word != "fn" or i + 1 == len(words) or not re.fullmatch(r"[A-Za-z_]\w*", words[i + 1]):
            continue
        j = i + 2
        while j < len(words) and words[j] not in ("{", ";"):
            j += 1
        if j == len(words) or words[j] == ";":
            continue
        start = j
        depth = 1
        j += 1
        while j < len(words) and depth:
            depth += (words[j] == "{") - (words[j] == "}")
            j += 1
        if depth:
            continue
        body = words[start + 1 : j - 1]
        # Exact token-normalized bodies: ignore whitespace/comments only, preserve
        # identifiers and literals so semantic differences are not hidden.
        digest = hashlib.sha256("\0".join(body).encode()).hexdigest()
        functions.append(
            {
                "name": words[i + 1],
                "line": bisect_right(newlines, ts[i][1]) + 1,
                "length": bisect_right(newlines, ts[j - 1][1])
                - bisect_right(newlines, ts[i][1])
                + 1,
                "complexity": 1
                + sum(w in ("if", "while", "for", "&&", "||", "?", "=>") for w in body),
                "hash": digest,
                "tokens": len(body),
            }
        )
    # Candidate raw guest accesses: numeric literals in memory accessor calls.
    raw_access = 0
    for i, word in enumerate(words):
        if re.match(r"(?:read|write|load|store|ptr|addr)\w*", word):
            nearby = words[i + 1 : i + 24]
            if "(" in nearby and any(re.fullmatch(r"0x[\da-fA-F_]+", w) for w in nearby):
                raw_access += 1
    return {
        "unsafe": unsafe,
        "macros": dict(sorted(macros.items())),
        "hex_literals": raw,
        "raw_access_candidates": raw_access,
        "functions": functions,
    }


def crates(root: Path) -> list[tuple[str, Path]]:
    return [
        (tomllib.loads(p.read_text())["package"]["name"], p)
        for p in sorted((root / "crates").glob("*/Cargo.toml"))
    ]


def scan(root: Path) -> dict[str, Any]:
    result = {}
    for name, manifest in crates(root):
        files = []
        for path in sorted(manifest.parent.rglob("*.rs")):
            if "target" in path.parts:
                continue
            row = inspect_source(path.read_text())
            row["path"] = str(path.relative_to(root))
            files.append(row)
        funcs = [dict(f, path=row["path"]) for row in files for f in row["functions"]]
        hashes = Counter(f["hash"] for f in funcs if f["tokens"] >= 10)
        macros = Counter()
        for row in files:
            macros.update(row["macros"])
        result[name] = {
            "files": len(files),
            "functions": len(funcs),
            "unsafe": sum(r["unsafe"] for r in files),
            "max_length": max((f["length"] for f in funcs), default=0),
            "max_complexity": max((f["complexity"] for f in funcs), default=0),
            "raw_access_candidates": sum(r["raw_access_candidates"] for r in files),
            "hex_literals": sum(r["hex_literals"] for r in files),
            "duplicate_bodies": sum(n - 1 for n in hashes.values()),
            "macros": dict(sorted(macros.items())),
            "details": files,
        }
    return result


def lint_counts(output: str, package: str) -> dict[str, int]:
    counts = Counter()
    seen = set()
    for line in output.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if row.get("reason") != "compiler-message":
            continue
        msg = row["message"]
        if msg.get("level") != "warning" or row.get("package_id") != package:
            continue
        key = json.dumps(msg, sort_keys=True)
        if key in seen:
            continue
        seen.add(key)
        code = msg.get("code") or {}
        counts[code.get("code", "uncoded")] += 1
    return dict(sorted(counts.items()))


def growth(actual: dict[str, int], allowed: dict[str, int]) -> list[str]:
    return [
        f"{key}: {count} > {allowed.get(key, 0)}"
        for key, count in actual.items()
        if count > allowed.get(key, 0)
    ]


def budget_growth(current: dict[str, Any], previous: dict[str, Any]) -> list[str]:
    """Reject edited budgets above the published primary-branch allowance."""
    return [
        f"{name}: allowlist {error}"
        for name, row in current.items()
        for error in growth(row.get("warnings", {}), previous.get(name, {}).get("warnings", {}))
    ]


def run(command: list[str], root: Path) -> tuple[int, str]:
    env = dict(os.environ, CARGO_TARGET_DIR=str(root / "tmp/T1959-target"), CARGO_BUILD_JOBS="4")
    proc = subprocess.run(
        ["timeout", "--kill-after=5s", "120", "nice", "-n", "10", *command],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout + proc.stderr


def gates(
    root: Path, baseline: dict[str, Any], initialize: bool = False
) -> tuple[dict[str, Any], list[str]]:
    observed = {}
    errors = []
    for name, manifest in crates(root):
        old = baseline.get(name, {})
        fmt, _ = run(["cargo", "fmt", "--manifest-path", str(manifest), "--check"], root)
        # Only library targets: host binaries may need private C/disc artifacts.
        code, output = run(
            [
                "cargo",
                "clippy",
                "--offline",
                "--manifest-path",
                str(manifest),
                "--lib",
                "--message-format=json",
                "--",
                "-W",
                "clippy::needless_borrow",
                "-W",
                "clippy::redundant_closure",
                "-W",
                "clippy::manual_let_else",
            ],
            root,
        )
        package_ids = set()
        for line in output.splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                continue
            target = row.get("target", {})
            if target.get("name", "").replace("_", "-") == name:
                package_ids.add(row.get("package_id"))
        counts = Counter()
        for package in package_ids:
            counts.update(lint_counts(output, package))
        observed[name] = {
            "fmt_gate": fmt == 0 if initialize else old.get("fmt_gate", False),
            "fmt_pass": fmt == 0,
            "clippy_status": "ok" if code == 0 else "unavailable",
            "warnings": dict(counts),
        }
        log = root / "generated/rust-quality" / f"{name}.log"
        write_lf_text(log, output)
        if not initialize:
            if old.get("fmt_gate") and fmt:
                errors.append(f"{name}: cargo fmt --check regressed")
            if old.get("clippy_status") == "ok" and code:
                errors.append(f"{name}: clippy could not complete")
            if code == 0:
                errors.extend(f"{name}: {e}" for e in growth(counts, old.get("warnings", {})))
    return observed, errors


def render(metrics: dict[str, Any], baseline: dict[str, Any]) -> str:
    lines = [
        "<!-- generated by python3 -m tools.rust_quality; aggregate only -->",
        "| Crate | Functions | Unsafe blocks | Max lines | Max complexity | Raw candidates | Duplicate bodies | tail_to! | RECOMP_PARITY8 | Clippy warnings | Fmt gate |",  # noqa: E501
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|",
    ]
    for name, row in sorted(metrics.items()):
        gate = baseline.get(name, {})
        warning = (
            str(sum(gate.get("warnings", {}).values()))
            if gate.get("clippy_status") == "ok"
            else "unavailable"
        )
        lines.append(
            f"| {name} | {row['functions']} | {row['unsafe']} | {row['max_length']} | {row['max_complexity']} | {row['raw_access_candidates']} | {row['duplicate_bodies']} | {row['macros'].get('tail_to', 0)} | {row['macros'].get('RECOMP_PARITY8', 0)} | {warning} | {'yes' if gate.get('fmt_gate') else 'debt'} |"  # noqa: E501
        )
    lint_totals = Counter()
    for gate in baseline.values():
        if gate.get("clippy_status") == "ok":
            lint_totals.update(gate["warnings"])
    lines += ["", "| Lint | Baseline warnings |", "|---|---:|"]
    lines += [f"| {lint} | {n} |" for lint, n in sorted(lint_totals.items())]
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="compare tracked aggregate without writing it"
    )
    parser.add_argument(
        "--gates", action="store_true", help="run fmt/clippy and reject warning growth"
    )
    parser.add_argument(
        "--initialize", action="store_true", help="create initial allowlist; refuses overwrite"
    )
    parser.add_argument(
        "--ratchet", action="store_true", help="lower existing lint budgets; never increase"
    )
    args = parser.parse_args()
    if args.initialize and (ROOT / BASELINE).exists():
        parser.error("allowlist already exists; use --ratchet to shrink")
    bulk = ROOT / "generated/rust-quality"
    bulk.mkdir(parents=True, exist_ok=True)
    baseline = json.loads((ROOT / BASELINE).read_text()) if (ROOT / BASELINE).exists() else {}
    errors = []
    if args.check:
        published = subprocess.run(
            ["git", "show", f"origin/main:{BASELINE.as_posix()}"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        if published.returncode == 0:
            errors.extend(budget_growth(baseline, json.loads(published.stdout)))
    if args.gates or args.initialize or args.ratchet:
        actual, gate_errors = gates(ROOT, baseline, args.initialize)
        errors.extend(gate_errors)
        if args.initialize:
            baseline = actual
        elif args.ratchet and not errors:
            for name, row in actual.items():
                if row["clippy_status"] == "ok":
                    baseline[name]["warnings"] = row["warnings"]
        if (args.initialize or args.ratchet) and not errors:
            write_lf_text(ROOT / BASELINE, json.dumps(baseline, indent=2, sort_keys=True) + "\n")
    metrics = scan(ROOT)
    write_lf_text(bulk / "inventory.json", json.dumps(metrics, indent=2, sort_keys=True) + "\n")
    table = render(metrics, baseline)
    path = ROOT / SUMMARY
    current = path.read_text() if path.exists() else ""
    begin, end = "<!-- quality:begin -->\n", "<!-- quality:end -->"
    if begin in current:
        prefix, rest = current.split(begin, 1)
        _, suffix = rest.split(end, 1)
        expected = prefix + begin + table + end + suffix
    else:
        expected = "# Rust quality baseline\n\n" + begin + table + end + "\n"
    if args.check:
        if current != expected:
            errors.append("Rust quality aggregate is stale; run python3 -m tools.rust_quality")
    else:
        write_lf_text(path, expected)
    for error in errors:
        print(error)
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
