# SPDX-License-Identifier: GPL-3.0-or-later
# ruff: noqa: E501
"""python -m tools.ledger generate | compare A B | rollup | next."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from collections import Counter
from collections.abc import Sequence
from pathlib import Path

from tools import verification
from tools.ledger import dispositions, exclusions, execution, freshness
from tools.ledger.compare import compare, has_regression, render
from tools.ledger.model import (
    OBLIGATION_IDS,
    Facts,
    build_ledger,
    fmt_va,
    mentions_from_docs,
    parse_va,
)
from tools.ledger.rollup import rank, rollup
from tools.ledger.store import read_snapshot, write_snapshot

DEFAULT_OUT = Path("generated/ledger")


def main_checkout() -> Path:
    common = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    return Path(common).parent if common else Path.cwd()


def tier_inputs(args: argparse.Namespace) -> list[Path]:
    """Every file a snapshot's tiers derive from (T3449: includes the verification tiers and the
    Tier 2 evidence, so a credited run makes the snapshot stale instead of staying invisible)."""
    return [
        Path(args.registry),
        Path(args.c2rust_status),
        Path(args.verification),
        Path(args.tier2_evidence),
        # I-T3744-3: capture promotions (rule 43) credit Tier 2 too; without this input a
        # promotion round never made the snapshot stale and `ledger tier2` kept saying 0.
        Path(getattr(args, "tier2_capture_evidence", verification.DEFAULT_CAPTURE_PATH)),
    ]


def resolve_snapshot(path: str) -> Path:
    """Fall back to the main checkout's snapshot: `generated/` is gitignored, so a worktree lacks it."""
    snapshot = Path(path)
    if snapshot.exists() or snapshot.is_absolute():
        return snapshot
    common = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    if common:
        candidate = Path(common).parent / snapshot
        if candidate.exists():
            return candidate
    return snapshot


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_names(path: Path) -> dict[int, tuple[str, str, str]]:
    with path.open(encoding="utf-8", newline="") as handle:
        return {
            parse_va(r["entry_va"]): (r["name"], r["confidence"], r["evidence"])
            for r in csv.DictReader(handle)
        }


def load_subsystems(path: Path) -> dict[int, str]:
    with path.open(encoding="utf-8", newline="") as handle:
        return {parse_va(r["address"]): r["subsystem"] for r in csv.DictReader(handle)}


def load_edges(path: Path | None) -> dict[int, tuple[int, ...]]:
    if path is None:
        return {}
    edges: dict[int, set[int]] = {}
    with path.open(encoding="utf-8", newline="") as handle:
        for r in csv.DictReader(handle):
            edges.setdefault(parse_va(r["caller"]), set()).add(parse_va(r["callee"]))
    return {k: tuple(sorted(v)) for k, v in edges.items()}


def load_hybrid_roots(hybrid: Path) -> dict[int, str]:
    if not hybrid.is_file():
        return {}
    data = json.loads(hybrid.read_text(encoding="utf-8"))
    return {
        parse_va(
            root["va"]
        ): f"{hybrid.as_posix()} 15-root integer pilot, original-vs-Rust differential"
        for root in data["retained_originals"]["roots"]
    }


def load_rust_ports(
    hybrid: Path,
    overlay: dict[str, object],
    registry: Path | None = None,
    records: dict[int, dict] | None = None,
) -> tuple[dict[int, str], dict[int, str]]:
    """(ports, weak): VA -> evidence. Hybrid pilot, overlay, c2rust, then port-registry (T1997).

    Earlier readers win. A registry VA declared only by an address-labelled doc comment is `weak`:
    not counted as a port (T1997, docs/t-obligation-ledger.md)."""
    ports = load_hybrid_roots(hybrid)
    for va, evidence in overlay.get("rust_ports", {}).items():  # type: ignore[union-attr]
        ports[parse_va(va)] = str(evidence)
    for va, evidence in load_c2rust_ports(C2RUST_STATUS).items():
        ports.setdefault(va, evidence)
    strong, weak = load_registry_ports(registry, records or {}) if registry else ({}, {})
    for va, evidence in strong.items():
        ports.setdefault(va, evidence)
    return ports, {va: note for va, note in weak.items() if va not in ports}


def load_registry_ports(
    path: Path, records: dict[int, dict]
) -> tuple[dict[int, str], dict[int, str]]:
    """T1997: docs/data/port-registry.json. The tier is the recorded one (verification-tiers.json
    or c2rust-status.csv), Tier 0 when there is no evidence: never higher than the evidence."""
    strong: dict[int, str] = {}
    weak: dict[int, str] = {}
    if not path.is_file():
        return strong, weak
    for key, modules in json.loads(path.read_text(encoding="utf-8"))["ports"].items():
        va = parse_va(key)
        modules = sorted(modules, key=lambda m: (m["crate"], m["module"]))
        where = ", ".join(sorted({f"{m['source']}:{m['line']}" for m in modules})[:3])
        if not any(set(m["forms"]) - {"doc"} for m in modules):
            weak[va] = f"port-registry doc-leader only, not counted: {where}"
            continue
        tier = records.get(va, {}).get("tier")
        label = f"Tier {tier}" if isinstance(tier, int) else "Tier 0 (no recorded evidence)"
        strong[va] = f"{label}: {path.as_posix()} {modules[0]['module']} ({where})"
    return strong, weak


C2RUST_STATUS = Path("docs/data/c2rust-status.csv")


def load_c2rust_ports(path: Path) -> dict[int, str]:
    """tools.c2rust output (T1878): translated functions with their verification tier (rule 10)."""
    from tools.c2rust.split import split_package

    ports: dict[int, str] = {}
    if not path.is_file():
        return ports
    with path.open(encoding="utf-8", newline="") as handle:
        for row in csv.DictReader(handle):
            if row["status"] != "translated" or row.get("tier") not in {"0", "1"}:
                continue
            batch = row["batch"]
            package = split_package(batch)
            source = (
                f"crates/{package.replace('_', '-')}/src/c2r/{batch}.rs"
                if package
                else f"crates/tsfp-ports/src/c2r/{batch}.rs"
            )
            ports[parse_va(row["va"])] = (
                f"Tier {row['tier']}: {source} (tools.c2rust, {row['evidence']}; not admitted)"
            )
    return ports


def load_facts(args: argparse.Namespace) -> tuple[Facts, dict[str, str]]:
    overlay = json.loads(Path(args.overlay).read_text(encoding="utf-8"))
    proof_path = Path(args.proofs)
    proofs = {
        parse_va(f["va"]): f
        for f in json.loads(proof_path.read_text(encoding="utf-8"))["functions"]
    }
    names = load_names(Path(args.names))
    subsystems = load_subsystems(Path(args.subsystems))
    disposition_table = dispositions.load(Path(args.dispositions))
    tier2_functions = dispositions.port_vas(subsystems, disposition_table)
    docs = [
        (p.as_posix(), p.read_text(encoding="utf-8", errors="replace"))
        for p in sorted(Path(args.docs).glob("*.md"))
    ]
    pins = {
        Path(p).name: sha256(Path(p))
        for p in (args.names, args.subsystems, args.proofs, args.overlay)
    }
    tier2_path = None if args.ignore_tier2_evidence else Path(args.tier2_evidence)
    sources = [args.verification, args.c2rust_status, args.registry]
    if tier2_path is not None:
        sources.append(args.tier2_evidence)
    for source in sources:
        if Path(source).exists():
            pins[Path(source).name] = sha256(Path(source))
    callers = dict(overlay.get("callers", {}))
    callers.update(
        {
            fmt_va(va): "docs/data/t1815-rust-hybrid.json caller-site audit eligible (111 sites)"
            for va in load_hybrid_roots(Path(args.hybrid))
        }
    )
    overlay["callers"] = callers
    records = verification.load_records(
        Path(args.verification),
        c2rust_status=Path(args.c2rust_status),
        tier2_path=tier2_path,
        tier2_functions=tier2_functions,
        tier2_capture_path=Path(args.tier2_capture_evidence),
    )
    rust_ports, weak_ports = load_rust_ports(
        Path(args.hybrid), overlay, Path(args.registry), records
    )
    facts = Facts(
        verification_records=records,
        names=names,
        subsystems=subsystems,
        proofs=proofs,
        doc_mentions=mentions_from_docs(docs, set(names) | set(subsystems)),
        overlay=overlay,
        edges=load_edges(Path(args.edges) if args.edges else None),
        rust_ports=rust_ports,
        weak_ports=weak_ports,
    )
    return facts, pins


def add_inputs(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--c2rust-status", default="docs/data/c2rust-status.csv")
    parser.add_argument("--registry", default="docs/data/port-registry.json")
    parser.add_argument("--verification", default=str(verification.DEFAULT_PATH))
    parser.add_argument("--tier2-evidence", default=str(verification.DEFAULT_TIER2_PATH))
    parser.add_argument("--tier2-capture-evidence", default=str(verification.DEFAULT_CAPTURE_PATH))
    parser.add_argument(
        "--ignore-tier2-evidence",
        action="store_true",
        help="baseline comparison only: do not import bulk Tier 2 receipt evidence",
    )
    parser.add_argument("--dispositions", default=str(dispositions.DEFAULT_PATH))
    parser.add_argument("--names", default="tools/data/function_names.csv")
    parser.add_argument(
        "--subsystems",
        default="tools/data/function_subsystems.csv",
        help="gitignored placement table (python -m tools.subsystems seed); pass the main checkout copy from a worktree",  # noqa: E501
    )
    parser.add_argument("--proofs", default="docs/data/replace-proof-snapshot.json")
    parser.add_argument("--hybrid", default="docs/data/t1815-rust-hybrid.json")
    parser.add_argument("--overlay", default="tools/data/ledger_facts.json")
    parser.add_argument("--docs", default="docs")
    parser.add_argument("--edges", help="optional CSV caller,callee (hex VAs) for dependency edges")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python -m tools.ledger", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    gen = sub.add_parser("generate", help="write the paginated ledger snapshot")
    add_inputs(gen)
    gen.add_argument("--out", default=str(DEFAULT_OUT))
    gen.add_argument("--page-size", type=int, default=500)
    gen.add_argument("--include-library", action="store_true")
    ref = sub.add_parser(
        "refresh",
        help="regenerate the shared main-checkout generated/ledger via temp dir + rename (T2134)",
    )
    add_inputs(ref)
    ref.add_argument("--out", help="default: generated/ledger of the main checkout")
    ref.add_argument("--page-size", type=int, default=500)
    ref.add_argument("--include-library", action="store_true")
    ref.add_argument(
        "--if-stale",
        action="store_true",
        help="T3449: do nothing when the snapshot is newer than every tier input "
        "(registry, c2rust status, verification tiers, Tier 2 evidence)",
    )
    cmp_ = sub.add_parser("compare", help="diff two snapshots; exit 1 on a regression")
    cmp_.add_argument("old")
    cmp_.add_argument("new")
    cmp_.add_argument("--json", action="store_true")
    for name, text in (
        ("rollup", "per subsystem tree and per slice"),
        ("next", "rank what to port next"),
    ):
        cmd = sub.add_parser(name, help=text)
        cmd.add_argument("--snapshot", default=str(DEFAULT_OUT))
        cmd.add_argument("--format", choices=("text", "json", "md"), default="text")
        if name == "next":
            cmd.add_argument("--limit", type=int, default=25)
            cmd.add_argument("--slice")
            cmd.add_argument("--subsystem")
            cmd.add_argument(
                "--weight",
                choices=("exec", "rate", "none"),
                default="exec",
                help="exec = calls/frame x readiness, rate = calls/frame, none = readiness only",
            )
            cmd.add_argument("--weights", default=str(execution.DEFAULT_PATH))
            cmd.add_argument(
                "--lease",
                metavar="N|all",
                help="atomically lease the N top ranked free functions (or all) to --task "
                "(T1912); leased functions of other tasks are never listed",
            )
            cmd.add_argument("--task", help="task id holding the lease, e.g. T1912")
            cmd.add_argument(
                "--include-replace",
                action="store_true",
                help="also list replace systems (ranked last); drop systems are always skipped",
            )
            cmd.add_argument("--dispositions", default=str(dispositions.DEFAULT_PATH))
            cmd.add_argument("--owner", help="session owner recorded on the lease")
            cmd.add_argument("--system", help="alias of --subsystem (whole system lease)")
            cmd.add_argument("--va-range", help="restrict to 0xLO-0xHI and lease that range")
    bar = sub.add_parser(
        "breadth", help="progress against the breadth-phase done bar (80%% calls, 60%% Tier 1)"
    )
    bar.add_argument("--snapshot", default=str(DEFAULT_OUT))
    bar.add_argument("--weights", default=str(execution.DEFAULT_PATH))
    bar.add_argument(
        "--scope",
        choices=("port", "all"),
        default="port",
        help="port: only port-disposition systems count (owner rule 14); all: every game function",
    )
    bar.add_argument("--dispositions", default=str(dispositions.DEFAULT_PATH))
    bar.add_argument("--json", action="store_true")
    bar.add_argument("--c2rust-status", default="docs/data/c2rust-status.csv")
    bar.add_argument("--strict", action="store_true", help="exit 1 when the snapshot is stale")
    bar.add_argument("--registry", default="docs/data/port-registry.json")
    bar.add_argument("--verification", default=str(verification.DEFAULT_PATH))
    bar.add_argument("--tier2-evidence", default=str(verification.DEFAULT_TIER2_PATH))
    bar.add_argument("--tier2-capture-evidence", default=str(verification.DEFAULT_CAPTURE_PATH))
    bar.add_argument(
        "--out", help="also write the JSON result here (docs/data/breadth-snapshot.json)"
    )
    tier2 = sub.add_parser("tier2", help="Tier 2 progress by port subsystem and overall")
    tier2.add_argument("--snapshot", default=str(DEFAULT_OUT))
    tier2.add_argument("--weights", default=str(execution.DEFAULT_PATH))
    tier2.add_argument("--dispositions", default=str(dispositions.DEFAULT_PATH))
    tier2.add_argument(
        "--baseline-snapshot",
        help="also include a before-promotion Tier 2 metric from a second ledger snapshot",
    )
    tier2.add_argument(
        "--exclusions",
        default=str(exclusions.DEFAULT_PATH),
        help="per-function MEASURED-unreachable exclusions (T3793); a missing file excludes none",
    )
    tier2.add_argument("--json", action="store_true")
    tier2.add_argument("--out", help="write the JSON metric snapshot here")
    disp = sub.add_parser("dispositions", help="docs/t-system-dispositions.md (owner rule 14)")
    disp.add_argument("--snapshot", default=str(DEFAULT_OUT))
    disp.add_argument("--weights", default=str(execution.DEFAULT_PATH))
    disp.add_argument("--dispositions", default=str(dispositions.DEFAULT_PATH))
    disp.add_argument("--write", help="rewrite the generated part of this markdown file")
    for name in ("debt", "audit"):
        cmd = sub.add_parser(
            name,
            help="verification debt"
            if name == "debt"
            else "queue random Tier 0/1 rechecks without promoting achieved tiers",
        )
        cmd.add_argument("--snapshot", default=str(DEFAULT_OUT))
        cmd.add_argument("--subsystem")
        if name == "audit":
            cmd.add_argument("--count", type=int, default=10)
            cmd.add_argument("--seed", type=int, required=True)
            cmd.add_argument("--target-tier", type=int, choices=(1, 2, 3), default=2)
            cmd.add_argument("--out", help="write reviewable JSON audit queue")
    col = sub.add_parser("exec-collect", help="aggregate host census files into the weights file")
    col.add_argument("roots", nargs="+", help="session folders or census.txt.* files")
    col.add_argument("--subsystems", required=True, help="placement table (tools.subsystems seed)")
    col.add_argument("--out", default=str(execution.DEFAULT_PATH))
    col.add_argument(
        "--profile",
        nargs="+",
        default=[],
        metavar="FILE",
        help="T1888: profile.txt* cpu sampler files of runs of --exe (second source, `samples` column)",
    )
    col.add_argument("--exe", help="the tsfp_host build the --profile files were sampled from")
    mrg = sub.add_parser(
        "exec-merge",
        help="T2417: add one entry-counted run (TSFP_CALL_COUNT_OUT file) to the weights file, keeping every row",
    )
    mrg.add_argument("calls", help="TSFP_CALL_COUNT_OUT dump (`function 0xVA calls N` lines)")
    mrg.add_argument("--frames", type=int, required=True, help="presented frames of that run")
    mrg.add_argument("--subsystems", required=True, help="placement table")
    mrg.add_argument("--weights", default=str(execution.DEFAULT_PATH))
    mrg.add_argument(
        "--profile", nargs="+", default=[], metavar="FILE", help="cpu sampler files of the same run"
    )
    mrg.add_argument("--exe", help="host build the --profile files were sampled from")
    return parser


def registry_modules(path: Path) -> dict[int, str]:
    if not path.is_file():
        return {}
    ports = json.loads(path.read_text(encoding="utf-8"))["ports"]
    return {parse_va(va): sorted(m["module"] for m in ms)[0] for va, ms in ports.items()}


def render_side_by_side(result: dict[str, object]) -> str:
    table = result.get("side_by_side")
    if not table:
        return ""
    labels = {
        "ported_any_tier": "ported (any tier)",
        "tier_ge_1": "Tier>=1",
        "tier_ge_2": "Tier>=2",
        "doc_leader_only_not_counted": "doc-leader only (not counted)",
    }
    lines = [
        f"executed game volume: {result['executed_game_calls']} calls, "
        f"{result['executed_game_functions']} functions",
        f"{'':32}{'calls':>12}{'call share':>12}{'functions':>11}{'fn share':>10}",
    ]
    for key, label in labels.items():
        row = table[key]  # type: ignore[index]
        lines.append(
            f"{label:32}{row['calls']:>12}{row['call_share']:>12.1%}"
            f"{row['functions']:>11}{row['function_share']:>10.1%}"
        )
    total = len(result.get("verification_debt_functions", []))  # type: ignore[arg-type]
    lines.append(
        f"verification debt: {total} executed ports with no recorded tier; top modules (executed calls):"
    )
    for item in result["verification_debt_modules"]:  # type: ignore[union-attr]
        lines.append(f"  {item['module']:46}{item['calls']:>12} {item['executed_functions']} fn")
    return "\n".join(lines)


def render_dispositions(result: dict[str, object]) -> str:
    table = result.get("by_disposition")
    if not table:
        return ""
    lines = [
        "by disposition (owner rule 14):",
        f"  {'class':11}{'functions':>10}{'ported':>8}{'tier>=1':>9}{'calls':>13}{'call share':>12}",
    ]
    for name, row in table.items():  # type: ignore[union-attr]
        lines.append(
            f"  {name:11}{row['functions']:>10}{row['ported']:>8}{row['tier1']:>9}"
            f"{row['calls']:>13}{row['call_share']:>12.1%}"
        )
    undecided = result.get("undecided_subsystems") or []
    if undecided:
        lines.append("  undecided (excluded from the port bar): " + ", ".join(undecided))  # type: ignore[arg-type]
    return "\n".join(lines)


def rows_of(snapshot: str | Path) -> list[dict[str, object]]:
    if not Path(snapshot).exists():
        raise SystemExit(
            f"ledger snapshot {snapshot} not found (generated/ is gitignored, so a fresh "
            "checkout or worktree has none): run `python -m tools.ledger generate` first"
        )
    rows = read_snapshot(Path(snapshot))
    return [rows[va] for va in sorted(rows)]


def render_rollup(result: dict[str, dict[str, dict[str, object]]], fmt: str) -> str:
    if fmt == "json":
        return json.dumps(result, indent=2, sort_keys=True)
    lines = []
    for group in ("tree", "slices"):
        if fmt == "md":
            lines += [
                "",
                f"### {group}",
                "",
                "| node | functions | complete | " + " | ".join(OBLIGATION_IDS) + " |",
                "|---|---:|---:|" + "---:|" * len(OBLIGATION_IDS),
            ]
        else:
            lines.append(f"== {group} (done/applicable per obligation) ==")
        for node, bucket in result[group].items():
            table = bucket["obligations"]
            cells = []
            for name in OBLIGATION_IDS:
                done, applicable = (
                    table[name]["done"],
                    sum(v for k, v in table[name].items() if k != "na"),
                )  # type: ignore[index,union-attr]
                cells.append(f"{done}/{applicable}")
            if fmt == "md":
                lines.append(
                    f"| {node} | {bucket['functions']} | {bucket['complete']} | "
                    + " | ".join(cells)
                    + " |"
                )
            else:
                lines.append(
                    f"{node:34} fn={bucket['functions']:<5} complete={bucket['complete']:<4} "
                    + " ".join(cells)
                )
    return "\n".join(lines)


def render_next(items: list[dict[str, object]], fmt: str) -> str:
    if fmt == "json":
        return json.dumps(items, indent=2, sort_keys=True)
    lines = []
    weighted = bool(items) and "calls_per_frame" in items[0]
    extra = " calls/frame | ready | score |" if weighted else ""
    if fmt == "md":
        lines += [
            "| rank | va | name | subsystem | slices | open | unported callees | open obligations |"  # noqa: E501
            + extra,
            "|---:|---|---|---|---|---:|---:|---|" + ("---:|---:|---:|" if weighted else ""),
        ]
    for number, item in enumerate(items, start=1):
        fields = (
            item["va"],
            item["name"],
            item["subsystem"],
            ",".join(item["slices"]),
            item["open"],  # type: ignore[arg-type]
            item["unported_callees"],
            ",".join(item["open_obligations"]),
        )  # type: ignore[arg-type]
        if weighted:
            fields += (
                f"{item['calls_per_frame']:.4f}",
                f"{item['ready']:.2f}",
                f"{item['score']:.4f}",
            )
        lines.append(
            f"| {number} | " + " | ".join(str(f) for f in fields) + " |"
            if fmt == "md"
            else f"{number:3} " + " ".join(str(f) for f in fields)
        )
    return "\n".join(lines)


def print_next(args: argparse.Namespace, rows: list, weights: object) -> int:
    """`next`: rank, hide VAs leased by other tasks, optionally lease the top N (T1912)."""
    from tools.agents import leases

    if args.lease and not args.task:
        print("--lease needs --task T<n>", file=sys.stderr)
        return 1
    lease_all = args.lease == "all"
    if args.lease and not lease_all and not args.lease.isdigit():
        print("--lease takes a count or 'all'", file=sys.stderr)
        return 1
    span = leases.parse_range(args.va_range) if args.va_range else None
    skipped: dict[str, int] = {}
    limit = 10**9  # hide leased functions before truncating to --limit
    ranked = rank(
        rows,
        limit=limit,
        slice_name=args.slice,
        subsystem=args.system or args.subsystem,
        weights=weights,  # type: ignore[arg-type]
        weight_mode=args.weight if weights else "none",
        table=dispositions.load(Path(args.dispositions)),
        include_replace=args.include_replace,
        stats=skipped,
    )
    if skipped.get("drop"):
        print(
            f"warning: {skipped['drop']} function(s) skipped: drop systems (rule 14)",
            file=sys.stderr,
        )
    if skipped.get("replace"):
        print(
            f"{skipped['replace']} function(s) in replace systems left out (--include-replace)",
            file=sys.stderr,
        )
    if span:
        ranked = [r for r in ranked if span[0] <= parse_va(str(r["va"])) <= span[1]]
    try:
        holder = leases.holder_lookup(args.task)
    except (OSError, subprocess.CalledProcessError):  # no repo: leases unavailable, still rank
        holder = None
    if holder is not None:
        hidden = [r for r in ranked if holder(parse_va(str(r["va"])))]
        ranked = [r for r in ranked if r not in hidden]
        if hidden:
            print(f"{len(hidden)} function(s) hidden: leased by other tasks", file=sys.stderr)
    if args.lease:
        count = None if lease_all else int(args.lease)
        if count is None and not (args.system or args.subsystem or span):
            print("--lease all needs --system or --va-range", file=sys.stderr)
            return 1
        chosen = leases.acquire_lease(
            args.task,
            args.owner or args.task,
            [parse_va(str(r["va"])) for r in ranked],
            count,
            system=args.system or args.subsystem or "",
            ranges=[span] if span else None,
        )
        ranked = [r for r in ranked if parse_va(str(r["va"])) in set(chosen)]
        print(f"leased {len(chosen)} function(s) to {args.task}", file=sys.stderr)
    else:
        ranked = ranked[: args.limit]
    print(render_next(ranked, args.format))
    return 0


def profile_samples(files: list[Path], exe: Path) -> dict[int, int]:
    """T1888: samples per lifted function VA from cpu sampler files, resolved with addr2line on `exe`."""
    import subprocess

    parsed = [
        execution.parse_profile(p.read_text(encoding="utf-8", errors="ignore")) for p in files
    ]
    offsets = sorted({rip - base for base, rips in parsed for rip in rips if rip >= base})
    names: dict[int, str] = {}
    for start in range(0, len(offsets), 4096):
        chunk = offsets[start : start + 4096]
        out = subprocess.run(
            ["addr2line", "-f", "-e", str(exe), *(f"0x{o:x}" for o in chunk)],
            capture_output=True,
            text=True,
            check=True,
            timeout=600,
        ).stdout.splitlines()
        for offset, name in zip(chunk, out[0::2], strict=False):
            names[offset] = name
    total: dict[int, int] = {}
    for base, rips in parsed:
        for va, count in execution.samples_by_va(base, rips, names).items():
            total[va] = total.get(va, 0) + count
    return total


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "generate":
        facts, pins = load_facts(args)
        rows = build_ledger(facts, include_library=args.include_library)
        index = write_snapshot(Path(args.out), rows, page_size=args.page_size, pins=pins)
        print(f"{index['functions']} functions in {len(index['pages'])} pages -> {args.out}")  # type: ignore[arg-type]
        return 0
    if args.command == "refresh":
        target = Path(args.out) if args.out else main_checkout() / DEFAULT_OUT
        if args.if_stale:
            _, stale_warnings = freshness.check(target, tier_inputs(args))
            if not stale_warnings:
                print(f"{target} is newer than every tier input, nothing to refresh")
                return 0
        tmp = target.with_name(target.name + f".tmp{os.getpid()}")
        shutil.rmtree(tmp, ignore_errors=True)
        facts, pins = load_facts(args)
        rows = build_ledger(facts, include_library=args.include_library)
        index = write_snapshot(tmp, rows, page_size=args.page_size, pins=pins)
        freshness.swap_in(tmp, target)
        print(f"{index['functions']} functions refreshed -> {target}")
        return 0
    if args.command == "compare":
        result = compare(read_snapshot(Path(args.old)), read_snapshot(Path(args.new)))
        print(json.dumps(result, indent=2, sort_keys=True) if args.json else render(result))
        return 1 if has_regression(result) else 0
    if args.command == "exec-merge":
        base = execution.load(Path(args.weights)) or execution.Weights()
        subs = load_subsystems(Path(args.subsystems))
        execution.merge_run(
            base,
            execution.parse_calls(Path(args.calls).read_text(encoding="utf-8", errors="ignore")),
            args.frames,
            subs,
        )
        if args.profile:
            if not args.exe:
                print("--profile needs --exe (addr2line target)", file=sys.stderr)
                return 2
            for va, n in profile_samples([Path(f) for f in args.profile], Path(args.exe)).items():
                base.samples[va] = base.samples.get(va, 0) + n
                base.subsystem.setdefault(va, subs.get(va, "unknown"))
        Path(args.weights).write_text(execution.dumps(base), encoding="utf-8")
        print(
            f"{len(base.calls)} executed functions, {base.total_calls()} calls, {base.frames} frames -> {args.weights}"
        )
        return 0
    if args.command == "exec-collect":
        weights = execution.collect(
            [Path(r) for r in args.roots], load_subsystems(Path(args.subsystems))
        )
        if args.profile:
            if not args.exe:
                print("--profile needs --exe (addr2line target)", file=sys.stderr)
                return 2
            weights.samples = profile_samples([Path(f) for f in args.profile], Path(args.exe))
            for va in weights.samples:
                weights.subsystem.setdefault(va, "unknown")
        Path(args.out).write_text(execution.dumps(weights), encoding="utf-8")
        print(
            f"{len(weights.calls)} executed functions, {weights.total_calls()} calls, "
            f"{weights.frames} frames in {weights.files} files / {weights.sessions} sessions"
            f" -> {args.out}"
        )
        return 0
    stale = False
    if args.command == "breadth":
        used = resolve_snapshot(args.snapshot)
        age, warns = freshness.check(used, tier_inputs(args))
        print(
            f"snapshot: {used}" + (f" (age {age / 3600:.1f} h)" if age is not None else ""),
            file=sys.stderr,
        )
        for line in warns:
            print(f"WARNING: {line}", file=sys.stderr)
        stale = bool(warns)
    rows = rows_of(resolve_snapshot(args.snapshot))
    if args.command == "tier2":
        snapshot_path = resolve_snapshot(args.snapshot)
        index_path = snapshot_path / "index.json" if snapshot_path.is_dir() else snapshot_path
        if index_path.name != "index.json" and index_path.is_file():
            index_path = index_path.parent / "index.json"
        index = json.loads(index_path.read_text(encoding="utf-8")) if index_path.is_file() else {}
        weights = execution.load(Path(args.weights))
        excluded = exclusions.validate(
            exclusions.load(Path(args.exclusions)),
            args.exclusions,
            executed={va for va, calls in (weights.calls if weights else {}).items() if calls},
            tier2={
                int(str(row["va"]), 16)
                for row in rows
                if verification.is_tier2(row.get("verification_tier"))
            },
        )
        for va, why in sorted(excluded.refused.items()):
            print(f"WARNING: exclusion {va:#x} refused, counted normally: {why}", file=sys.stderr)
        result = execution.tier2_metrics(
            rows, weights, dispositions.load(Path(args.dispositions)), excluded.accepted
        )
        from tools import t2capture_promote

        # I-T3784-8: credit kept while its port changed and a re-check is pending; a new key,
        # the existing ones are unchanged (claude-1 audit reads them).
        result["stale_port_changed_recheck_pending"] = len(
            t2capture_promote.stale_port_entries(Path("."))
        )
        # I-T3784-9: harness credit is re-checked by its harness lane, counted apart.
        result["stale_harness_rerun_pending"] = len(
            t2capture_promote.stale_port_entries(Path("."), harness=True)
        )
        result["exclusions"] = {
            "path": excluded.path,
            "accepted": [f"{va:#x}" for va in sorted(excluded.accepted)],
            "refused": {f"{va:#x}": why for va, why in sorted(excluded.refused.items())},
        }
        if args.baseline_snapshot:
            baseline_path = resolve_snapshot(args.baseline_snapshot)
            baseline_rows = rows_of(baseline_path)
            result["baseline"] = execution.tier2_metrics(
                baseline_rows,
                execution.load(Path(args.weights)),
                dispositions.load(Path(args.dispositions)),
                excluded.accepted,
            )
            baseline_index = (
                baseline_path / "index.json" if baseline_path.is_dir() else baseline_path
            )
            if baseline_index.name != "index.json" and baseline_index.is_file():
                baseline_index = baseline_index.parent / "index.json"
            result["baseline_snapshot_sha256"] = (
                sha256(baseline_index) if baseline_index.is_file() else ""
            )
        result["inputs"] = {
            "snapshot_sha256": sha256(index_path) if index_path.is_file() else "",
            "snapshot_pins": index.get("pins", {}),
            "weights": str(args.weights),
            "weights_sha256": sha256(Path(args.weights)),
            "dispositions": str(args.dispositions),
            "dispositions_sha256": sha256(Path(args.dispositions)),
            "exclusions": str(args.exclusions),
            "exclusions_sha256": sha256(Path(args.exclusions))
            if Path(args.exclusions).is_file()
            else "",
        }
        if args.out:
            Path(args.out).write_text(
                json.dumps(result, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
                newline="\n",
            )
        if args.json:
            print(json.dumps(result, indent=2, sort_keys=True))
        else:
            total = result["overall"]
            print(
                "Tier 2 port progress: "
                f"{total['tier2_functions']}/{total['functions']} functions "
                f"({total['tier2_function_share']:.1%}); "
                f"{total['tier2_executed_functions']}/{total['executed_functions']} executed functions "
                f"({total['tier2_executed_function_share']:.1%}); "
                f"{total['tier2_calls']}/{total['executed_calls']} calls "
                f"({total['tier2_call_share']:.1%}); "
                f"{total['tier2_unweighted_functions']} promoted but not in the weights corpus"
            )
            print(
                "Tier 2 stale (port changed, recheck pending): "
                f"{result['stale_port_changed_recheck_pending']} capture credit(s); "
                "harness stale (harness lane rerun pending): "
                f"{result['stale_harness_rerun_pending']}"
            )
            print(
                f"Tier 2 {total['tier2_functions']}/{total['functions']}, "
                f"{total['excluded_unreachable']} excluded as unreachable "
                f"(evidence file {excluded.path}; {len(excluded.refused)} refused)"
            )
            print(
                "Tier 2 by source: "
                f"whole-game {total['tier2_whole_game_functions']} functions "
                f"({total['tier2_whole_game_calls']} calls); "
                f"capture {total['tier2_capture_functions']} functions "
                f"({total['tier2_capture_calls']} calls); "
                f"constructed-call {total['tier2_constructed_call_functions']} functions "
                f"({total['tier2_constructed_call_calls']} calls); "
                f"harness-service {total['tier2_harness_service_functions']} functions "
                f"({total['tier2_harness_service_calls']} calls); "
                f"symbolic {total['tier2_symbolic_functions']} functions "
                f"({total['tier2_symbolic_calls']} calls)"
            )
            labels = total.get("tier2_by_source") or {}
            print(
                "Tier 2 by source label: "
                + (", ".join(f"{k} {v['functions']}" for k, v in sorted(labels.items())) or "none")
            )
            print("system | tier2/functions | tier2/executed | tier2 calls/executed calls")
            for system, item in result["by_system"].items():
                print(
                    f"{system} | {item['tier2_functions']}/{item['functions']} "
                    f"({item['tier2_function_share']:.1%}) | "
                    f"{item['tier2_executed_functions']}/{item['executed_functions']} "
                    f"({item['tier2_executed_function_share']:.1%}) | "
                    f"{item['tier2_calls']}/{item['executed_calls']} "
                    f"({item['tier2_call_share']:.1%})"
                    + (
                        f" | {item['excluded_unreachable']} excluded as unreachable"
                        if item["excluded_unreachable"]
                        else ""
                    )
                )
        return 0
    if args.command == "dispositions":
        body = dispositions.render_doc(
            rows, execution.load(Path(args.weights)), dispositions.load(Path(args.dispositions))
        )
        if not args.write:
            print(body, end="")
            return 0
        target = Path(args.write)
        tail = ""
        if target.is_file():
            old = target.read_text(encoding="utf-8")
            if dispositions.GENERATED_END in old:
                tail = old.split(dispositions.GENERATED_END, 1)[1].lstrip("\n")
        target.write_text(body + ("\n" + tail if tail else ""), encoding="utf-8", newline="\n")
        return 0
    if args.command == "breadth":
        result = execution.breadth_bar(
            rows,
            execution.load(Path(args.weights)),
            registry_modules(Path(args.registry)),
            scope=args.scope,
            table=dispositions.load(Path(args.dispositions)),
        )
        if args.out:
            Path(args.out).write_text(
                json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
            )
        if args.json:
            print(json.dumps(result, indent=2, sort_keys=True))
        else:
            print(
                f"breadth bar ({result['scope']} scope): calls {result['call_share']:.1%} of {result['call_target']:.0%} "
                f"({result['ported_calls']}/{result['game_calls']}), Tier>=1 "
                f"{result['tier1_share']:.1%} of {result['tier1_target']:.0%} "
                f"({result['tier1_functions']}/{result['functions']}), "
                f"{'MET' if result['met'] else 'not met'}"
                + ("" if result["weights_present"] else " (no execution weights file)")
            )
            print(render_side_by_side(result))
            print(render_dispositions(result))
        return 1 if (args.strict and stale) else 0
    if args.command in ("debt", "audit"):
        if args.subsystem:
            rows = [
                r
                for r in rows
                if r["subsystem"] == args.subsystem
                or r["subsystem"].startswith(args.subsystem + "/")
            ]
        if args.command == "audit":
            result = {
                "seed": args.seed,
                "functions": verification.sample(
                    rows, count=args.count, seed=args.seed, target_tier=args.target_tier
                ),
            }
        else:
            result = {
                **verification.summary(rows),
                "functions": [
                    r
                    for r in rows
                    if r.get("verification_tier") is None or r["verification_tier"] < 2
                ],
            }
        output = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.command == "audit" and args.out:
            Path(args.out).write_text(output, encoding="utf-8")
        print(output, end="")
        return 0
    if args.command == "rollup":
        print(render_rollup(rollup(rows), args.format))
        tiers = Counter(
            m.group(1)
            for row in rows
            if (
                m := re.match(
                    r"Tier (\d)", str(row["obligations"]["rust_port"].get("evidence", ""))
                )
            )  # type: ignore[index,union-attr]
        )
        if tiers:
            print(
                "rust_port verification tiers (AGENTS.md rule 10): "
                + ", ".join(f"tier {k}: {v}" for k, v in sorted(tiers.items()))
            )
    else:
        weights = execution.load(Path(args.weights)) if args.weight != "none" else None
        if args.weight != "none" and weights is None:
            print(
                f"no weights file {args.weights}: ranking without execution weight", file=sys.stderr
            )
        return print_next(args, rows, weights)
    return 0


if __name__ == "__main__":
    sys.exit(main())
