# SPDX-License-Identifier: GPL-3.0-or-later
"""Capture-based Tier 2 runner and receipts (T3743, docs/t3738-capture-design.md section 3).

Per VA: export the unflagged corpus entries, run the offline host
(`<host> --t2-offline <entries.bin> --va 0x<VA> --out <results.jsonl>`), aggregate its per-entry
rows into `receipts/t2capture/<va>.json`. `lifted` is the fidelity control: an entry whose lifted
result is not "equal" is `capture_unfaithful` and its rust result is ignored.
"""

from __future__ import annotations

import csv
import hashlib
import importlib
import inspect
import json
import re
import socket
import subprocess
import tempfile
from concurrent.futures import ThreadPoolExecutor
from datetime import UTC, datetime
from pathlib import Path

from tools import gate_data
from tools.t2capture import callee_arity, fpo_leaf, inlined_callees, ret_liveness
from tools.t2corpus.spool import flag_names
from tools.t2corpus.store import Corpus

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = 1
KIND = "t2capture"
WEIGHTS_PATH = Path("docs/data/t1884-exec-weights.csv")
REGISTRY_PATH = Path("docs/data/port-registry.json")
SUMMARY_PATH = Path("docs/data/t2capture-summary.json")
DEFAULT_TIMEOUT = 600
EXIT_HARNESS_ERROR = 2
EXIT_NO_ABI = 3
LIFTED_CLASSES = frozenset({"equal", "exit_regs", "write_set", "callee_mismatch", "fault"})
RUST_CLASSES = LIFTED_CLASSES | {"fp_ulp", "no_abi_entry"}
SKIPPED = "skipped"  # T3742: both sides, reason in detail (flagged, other VA, malformed record)
DIVERGED_CLASSES = frozenset({"exit_regs", "write_set", "callee_mismatch", "fault"})
# v9 (T3832, I-T3832-1): a receipt with size-flagged entries is never EQUAL. Those are the calls
# with the most work (read-set or write-set over 64 KiB, or over 256 callees: the 240 to 267-node
# skeletons of 0x60B30), exactly where a capped port goes wrong, so excluding them could certify a
# capped port. They stay excluded from the counts and make the status `incomplete_size`.
INCOMPLETE_SIZE = "incomplete_size"
SIZE_FLAG = "size"
# T3740: a record that takes the whole host down (abort or exit in host code reached from the run)
# becomes a `fault` row and the host resumes after it (`--from`); more deaths than this per VA is an
# error for the VA (a broken host, not a few bad records).
MAX_HOST_DEATHS = 16

# Code that decides a receipt verdict; its digest keys the receipt (as gate.compare_logic_digest).
COMPARE_FUNCTIONS = (
    ("tools.t2capture.runner", "parse_results"),
    ("tools.t2capture.runner", "aggregate"),
    ("tools.t2capture.runner", "_first_flag"),
    ("tools.t2capture.runner", "_provenance"),
    ("tools.t2capture.runner", "corpus_digest"),
    ("tools.t2capture.ret_liveness", "ret_live"),
    ("tools.t2capture.ret_liveness", "caller_live_ebp"),
    ("tools.t2capture.ret_liveness", "dead_regs"),
)

COMPARE_RUST_SOURCES = tuple(
    f"crates/tsfp-t2-offline/src/{name}.rs"
    for name in ("compare", "stub", "frames", "arena", "runner", "digest")
)


def compare_logic_version() -> int:
    """`COMPARE_LOGIC_VERSION` of crates/tsfp-t2-offline/src/compare.rs (0 when absent)."""
    text = (ROOT / COMPARE_RUST_SOURCES[0]).read_text()
    match = re.search(r"COMPARE_LOGIC_VERSION:\s*u32\s*=\s*(\d+)", text)
    return int(match.group(1)) if match else 0


def compare_logic_digest() -> str:
    """sha256 prefix over the comparison and aggregation functions (resolved at call time)."""
    digest = hashlib.sha256()
    for relative in COMPARE_RUST_SOURCES:  # T3742: the harness half of the comparison
        digest.update(relative.encode() + b"\0" + (ROOT / relative).read_bytes())
    digest.update(f"COMPARE_LOGIC_VERSION={compare_logic_version()}".encode())
    table = ret_liveness.table_path()  # v3: which VAs skip EAX/EDX is part of the verdict
    digest.update(b"ret_liveness\0" + (table.read_bytes() if table.exists() else b""))
    leaf = fpo_leaf.table_path()  # v4: which roots accept the lifted seh_ebp exit EBP
    digest.update(b"fpo_leaf\0" + (leaf.read_bytes() if leaf.exists() else b""))
    inlined = inlined_callees.table_path()  # v6: declared native callees per root
    digest.update(b"inlined_callees\0" + (inlined.read_bytes() if inlined.exists() else b""))
    arity = callee_arity.table_path()  # v8: proven callee stack arity (lift-derived)
    digest.update(b"callee_arity\0" + (arity.read_bytes() if arity.exists() else b""))
    digest.update(
        b"callee_arity_overrides\0"
        + json.dumps(callee_arity._KNOWN_ARITY_OVERRIDES, sort_keys=True).encode()
    )
    for module, name in COMPARE_FUNCTIONS:
        source = inspect.getsource(getattr(importlib.import_module(module), name))
        digest.update(f"{module}.{name}".encode() + b"\0" + source.encode())
    return digest.hexdigest()[:16]


def va_text(va: int) -> str:
    return f"0x{va:08X}"


def _first_flag(flags: int) -> str:
    return flag_names(flags)[0]


def corpus_digest(digests: list[str]) -> str:
    return hashlib.sha256("\n".join(sorted(digests)).encode()).hexdigest()


def _provenance(row: dict) -> dict:
    return {
        "recording_sha256": row["recording_sha256"],
        "poll": row["poll"],
        "guest_thread": row["guest_thread"],
        "caller_va": va_text(row["caller_va"]),
        "return_address": va_text(row["return_address"]),
        "digest": row["digest"],
    }


def parse_results(text: str, expected: int) -> list[dict]:
    """Host JSON lines in input order; raises ValueError on a short, reordered or bad output."""
    rows = [json.loads(line) for line in text.splitlines() if line.strip()]
    if len(rows) != expected:
        raise ValueError(f"host returned {len(rows)} rows for {expected} entries")
    for position, row in enumerate(rows):
        if row.get("index") != position:
            raise ValueError(f"row {position} has index {row.get('index')}")
        lifted, rust = row.get("lifted"), row.get("rust")
        if lifted == SKIPPED and rust == SKIPPED:
            continue
        if (lifted is not None and lifted not in LIFTED_CLASSES) or (
            rust is not None and rust not in RUST_CLASSES
        ):
            raise ValueError(
                f"row {position} has unknown class {row.get('lifted')}/{row.get('rust')}"
            )
    return rows


def aggregate(rows: list[dict], results: list[dict]) -> dict:
    """Receipt status, counts, warnings and first divergence from index rows and host results.

    `rows` are all stored entries of the VA in export order; `results` cover the unflagged ones.
    """
    excluded: dict[str, int] = {}
    sent: list[dict] = []
    for row in rows:
        if row["flags"]:
            key = f"capture_incomplete:{_first_flag(row['flags'])}"
            excluded[key] = excluded.get(key, 0) + 1
        else:
            sent.append(row)
    counts = {"compared": len(rows), "equal": 0, "diverged": 0, "fp_ulp": 0}
    fidelity = {"lifted_equal": 0, "capture_unfaithful": 0}
    warnings = {"extra_read": 0, "dead_stack_writes": 0}
    first: dict | None = None
    no_abi = 0
    size_flagged = sum(1 for row in rows if row["flags"] and SIZE_FLAG in flag_names(row["flags"]))

    def diverge(row: dict, result: dict, rust: str) -> None:
        nonlocal first
        counts["fp_ulp" if rust == "fp_ulp" else "diverged"] += 1
        if first is None:
            first = {
                "entry": result["index"],
                "class": rust,
                "detail": str(result.get("detail", ""))[:300],
                "provenance": _provenance(row),
            }

    for row, result in zip(sent, results, strict=True):
        # v9: a port that did nothing where the original wrote memory or had callees is DIVERGED
        # on every run entry, also where the lifted control is unfaithful (never hidden).
        if result.get("no_work") and result["lifted"] not in (SKIPPED, None):
            if result["lifted"] != "equal":
                fidelity["capture_unfaithful"] += 1
            else:
                fidelity["lifted_equal"] += 1
            diverge(row, result, "no_work")
            continue
        if result["lifted"] == SKIPPED:
            detail = str(result.get("detail", ""))
            key = detail if detail.startswith("capture_incomplete:") else "skipped:host"
            excluded[key] = excluded.get(key, 0) + 1
            continue
        if result["lifted"] is None:  # rust-only run: no fidelity control, nothing certified
            excluded["lifted_not_run"] = excluded.get("lifted_not_run", 0) + 1
            continue
        if result["lifted"] != "equal":
            fidelity["capture_unfaithful"] += 1
            excluded["capture_unfaithful"] = excluded.get("capture_unfaithful", 0) + 1
            continue
        fidelity["lifted_equal"] += 1
        warnings["extra_read"] += int(result.get("extra_read", 0))
        warnings["dead_stack_writes"] += int(result.get("dead_stack_writes", 0))
        rust = result["rust"]
        if rust is None:  # lifted-only run: fidelity checked, port not judged
            excluded["rust_not_run"] = excluded.get("rust_not_run", 0) + 1
            continue
        if rust == "no_abi_entry":
            no_abi += 1
        elif rust == "equal":
            counts["equal"] += 1
        else:
            diverge(row, result, rust)
    counts["excluded"] = excluded
    counts["fidelity"] = fidelity
    if not rows:
        status = "no_samples"
    elif not sent:
        status = "capture_incomplete"
    elif no_abi:
        status = "no_abi_entry"
    elif fidelity["lifted_equal"] == 0:
        status = "error"
    elif counts["diverged"] or counts["fp_ulp"]:
        status = "diverged"
    elif counts["equal"] == 0:  # nothing judged (lifted-only or every entry skipped)
        status = "no_samples"
    elif size_flagged:
        status = INCOMPLETE_SIZE
    else:
        status = "equal"
    return {
        "status": status,
        "counts": counts,
        "warnings": warnings,
        "first_divergence": first,
        "corpus_digest": corpus_digest([row["digest"] for row in sent]),
    }


def port_identity(va: int, registry: Path | None = None, root: Path = ROOT) -> tuple[str, str]:
    """(port module, last commit touching its source) for a VA, empty strings when unknown."""
    path = registry or root / REGISTRY_PATH
    ports = json.loads(path.read_text()).get("ports", {}) if path.exists() else {}
    entries = {int(key, 16): val for key, val in ports.items()}.get(va) or []
    if not entries:
        return "", ""
    module, source = entries[0].get("module", ""), entries[0].get("source", "")
    commit = ""
    if source:
        done = subprocess.run(
            ["git", "log", "-1", "--format=%H", "--", source],
            cwd=root, capture_output=True, text=True, check=False, timeout=60,
        )  # fmt: skip
        commit = done.stdout.strip()
    return module, commit


def default_build_input_hash() -> str:
    try:
        from tools import prebuilt_select

        return prebuilt_select.build_input_hash(ROOT, "HEAD")
    except Exception:  # best effort: the receipt still names the host path
        return ""


def receipt_root(root: Path | None = None) -> Path:
    if root is not None:
        return Path(root)
    from tools.trace_store import main_root

    return main_root()


def receipt_path(root: Path, va: int) -> Path:
    return gate_data.receipt_store(root, KIND) / f"{va_text(va)}.json"


def run_va(
    corpus: Corpus,
    tree: str,
    va: int,
    host: Path | None,
    *,
    timeout: float = DEFAULT_TIMEOUT,
    min_samples: int = 0,
    build_input_hash: str = "",
    registry: Path | None = None,
) -> dict:
    """Run one VA end to end and return its receipt dict (not yet written)."""
    db_rows = [dict(row) for row in corpus.entry_rows(tree, va)]
    module, commit = port_identity(va, registry)
    # v3: EAX/EDX are compared only when some caller may read them (absent = strict).
    live = ret_liveness.ret_live(va, ret_liveness.load())
    # v4: the lifted fpo_leaf root returns EBP = entry seh_ebp (absent or other tree = strict).
    leaf = va in fpo_leaf.load(tree)
    # v6: callees this root's port runs natively (absent = strict).
    inlined = inlined_callees.flag_value(va, inlined_callees.load())
    # v8: callee words at or above the proven arity are not compared (no table = strict).
    arity = callee_arity.load(tree)
    receipt: dict = {
        "schema": SCHEMA,
        "kind": KIND,
        "va": va_text(va),
        "compare_logic": compare_logic_digest(),
        "corpus_digest": "",
        "lifted_tree_hash": tree,
        "port_module": module,
        "port_source_commit": commit,
        "build_input_hash": build_input_hash,
        "host": socket.gethostname(),
        "created": datetime.now(UTC).isoformat(timespec="seconds"),
        "status": "error",
        "counts": {},
        "warnings": {"extra_read": 0, "dead_stack_writes": 0},
        "first_divergence": None,
        "min_samples_policy": min_samples,
        "ret_live": live,
        "fpo_leaf": leaf,
        "inlined_callees": inlined,
        "callee_arity": len(arity),
    }
    usable = [row for row in db_rows if not row["flags"]]
    if usable and host is not None:
        outcome = _invoke_host(
            corpus,
            tree,
            va,
            host,
            timeout,
            usable,
            ret_liveness.dead_regs(live),
            leaf,
            inlined,
            arity,
        )
    else:
        outcome = ("results", [])
    if usable and host is None:
        outcome = ("error", "no host executable")
    if outcome[0] == "results":
        receipt.update(aggregate(db_rows, outcome[1]))
    elif outcome[0] == "no_abi_entry":
        # T3767: the host still ran the lifted control on every entry; count what it measured
        # (before, every entry of a function without a port was reported lifted-equal).
        rows = outcome[1] if outcome[1] is not None else [_no_abi(row) for row in usable]
        receipt.update(aggregate(db_rows, rows))
        receipt["status"] = "no_abi_entry"
    else:
        receipt["counts"] = {"compared": len(db_rows)}
        receipt["error"] = str(outcome[1])[:300]
    return receipt


def _no_abi(row: dict) -> dict:
    return {"index": 0, "lifted": "equal", "rust": "no_abi_entry", "detail": "no dispatch entry"}


def _invoke_host(
    corpus: Corpus,
    tree: str,
    va: int,
    host: Path,
    timeout: float,
    usable: list[dict],
    ret_dead: list[str] | None = None,
    leaf: bool = False,
    inlined: str = "",
    arity: dict[int, int] | None = None,
) -> tuple[str, object]:
    from tools import datadisk

    with tempfile.TemporaryDirectory(prefix="t2capture-", dir=datadisk.data_tmp()) as scratch:
        entries, out = Path(scratch) / "entries.bin", Path(scratch) / "results.jsonl"
        corpus.export(va, tree, entries, skip_flagged=True)
        cmd = [str(host), "--t2-offline", str(entries), "--va", f"0x{va:08x}", "--out", str(out)]
        if ret_dead:
            cmd += ["--ret-dead", ",".join(ret_dead)]
        if leaf:
            cmd.append("--fpo-leaf")
        if inlined:
            cmd += ["--inlined-callees", inlined]
        if arity:
            table = Path(scratch) / "callee-arity.txt"
            table.write_text(callee_arity.flag_text(arity))
            cmd += ["--callee-arity", str(table)]
        return _run_isolated(cmd, out, len(usable), timeout)


def _die_with_parent() -> None:
    """Child preexec: the host gets SIGKILL when this runner dies. A runner killed by an outer
    timeout left a --t2-offline host spinning for 9.3 h, holding the local game slot (I-T3879)."""
    import ctypes
    import signal

    ctypes.CDLL(None, use_errno=True).prctl(1, signal.SIGKILL, 0, 0, 0)  # PR_SET_PDEATHSIG


def _run_isolated(cmd: list[str], out: Path, expected: int, timeout: float) -> tuple[str, object]:
    """Run the host; a record that kills it is a `fault` row and the run resumes after it."""
    lines: list[str] = []
    deaths = 0
    start = 0
    while True:
        args = [*cmd, "--from", str(start)] if start else cmd
        try:
            done = subprocess.run(
                args,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
                preexec_fn=_die_with_parent,
            )
        except subprocess.TimeoutExpired:
            return "error", f"host timed out after {timeout}s"
        except OSError as exc:
            return "error", f"host not runnable: {exc}"
        if done.returncode == EXIT_NO_ABI:
            try:
                rows = parse_results(out.read_text(), expected) if start == 0 else None
            except (OSError, ValueError):
                rows = None
            return "no_abi_entry", rows
        try:
            produced = [line for line in out.read_text().splitlines() if line.strip()]
        except OSError:
            produced = []
        if done.returncode == 0:
            lines += produced
            break
        if done.returncode == EXIT_HARNESS_ERROR or start + len(produced) >= expected:
            return "error", f"host exit {done.returncode}: {done.stderr.strip()[-200:]}"
        deaths += 1
        if deaths > MAX_HOST_DEATHS:
            return (
                "error",
                f"host died {deaths} times, last exit {done.returncode}: "
                f"{done.stderr.strip()[-160:]}",
            )
        lines += produced
        dead = start + len(produced)
        detail = f"host died (exit {done.returncode}): {done.stderr.strip()[-160:]}"
        lines.append(json.dumps({"index": dead, "digest": "", "lifted": "fault", "rust": "fault",
                                 "detail": detail, "extra_read": 0,
                                 "dead_stack_writes": 0, "no_work": False}))  # fmt: skip
        start = dead + 1
        if start >= expected:
            break
    try:
        return "results", parse_results("\n".join(lines), expected)
    except ValueError as exc:
        return "error", f"bad host output: {exc}"


class HostRefused(RuntimeError):
    """The host cannot run the lifted fidelity control (T3767: not a capture build)."""


def preflight(host: Path, timeout: float = 120) -> None:
    """Run the host once on an empty spool; raise HostRefused when it refuses the run.

    T3767: rounds 0 and 1 ran the checker on the gate prebuilt, whose lifted control ran real
    callees (retained originals without the call hook, unhooked tail calls): 489 functions all
    `capture_unfaithful` and 127 host deaths. The host now refuses that (exit 2 before reading any
    input); this probe turns the refusal into one error for the whole run instead of one per VA.
    """
    from tools import datadisk
    from tools.t2corpus.spool import write_spool

    keys = ("recording_sha256", "build_input_hash", "lifted_tree_hash", "created", "tool_version")
    header = {key: "preflight" for key in keys}
    with tempfile.TemporaryDirectory(prefix="t2preflight-", dir=datadisk.data_tmp()) as scratch:
        spool, out = Path(scratch) / "empty.bin", Path(scratch) / "out.jsonl"
        write_spool(spool, header, [])
        cmd = [str(host), "--t2-offline", str(spool), "--va", "0x0", "--out", str(out)]
        try:
            done = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, check=False)
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise HostRefused(f"host preflight failed: {exc}") from exc
    if done.returncode not in (0, EXIT_NO_ABI):
        raise HostRefused(
            f"host refused the preflight (exit {done.returncode}): {done.stderr.strip()[-300:]}"
        )


def write_receipt(root: Path, receipt: dict) -> Path:
    path = receipt_path(root, int(receipt["va"], 16))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(receipt, indent=1, sort_keys=True) + "\n")
    return path


def exec_weights(path: Path | None = None) -> dict[int, int]:
    weights = Path(path) if path else ROOT / WEIGHTS_PATH
    out: dict[int, int] = {}
    if not weights.exists():
        return out
    with weights.open(newline="") as src:
        for row in csv.DictReader(line for line in src if not line.startswith("#")):
            out[int(row["va"], 16)] = int(row["calls"] or 0)
    return out


def ordered_vas(corpus: Corpus, tree: str, weights: dict[int, int]) -> list[int]:
    """Corpus VAs, heaviest executed-call weight first (ties by VA)."""
    vas = {row["va"] for row in corpus.entry_rows(tree)}
    return sorted(vas, key=lambda va: (-weights.get(va, 0), va))


def run_all(
    corpus_root: Path,
    tree: str,
    host: Path | None,
    root: Path,
    *,
    jobs: int = 1,
    min_samples: int = 0,
    weights: dict[int, int] | None = None,
    **kw: object,
) -> list[dict]:
    """Run every corpus VA (weight order); VAs with fewer usable entries than min_samples skip."""
    with_corpus = Corpus(corpus_root)
    try:
        weight = weights if weights is not None else exec_weights()
        vas = ordered_vas(with_corpus, tree, weight)
        usable = {
            va: sum(1 for r in with_corpus.entry_rows(tree, va) if not r["flags"]) for va in vas
        }
    finally:
        with_corpus.close()
    todo = [va for va in vas if usable[va] >= min_samples]
    return run_vas(corpus_root, tree, todo, host, root, jobs=jobs, min_samples=min_samples, **kw)


def run_vas(
    corpus_root: Path,
    tree: str,
    todo: list[int],
    host: Path | None,
    root: Path,
    *,
    jobs: int = 1,
    min_samples: int = 0,
    **kw: object,
) -> list[dict]:
    """Run the given VAs on `jobs` threads, writing each receipt (T3784: continuous re-check)."""
    if host is not None and todo:
        preflight(host)

    def one(va: int) -> dict:
        local = Corpus(corpus_root)
        try:
            receipt = run_va(local, tree, va, host, min_samples=min_samples, **kw)  # type: ignore[arg-type]
        finally:
            local.close()
        write_receipt(root, receipt)
        return receipt

    with ThreadPoolExecutor(max_workers=max(1, jobs)) as pool:
        return list(pool.map(one, todo))


def build_summary(root: Path, weights: dict[int, int], top: int = 20) -> dict:
    """Per-status counts and top divergences by call weight; no raw bytes, no timestamps."""
    by_status: dict[str, int] = {}
    functions = []
    for path in sorted(gate_data.receipt_store(root, KIND).glob("*.json")):
        receipt = json.loads(path.read_text())
        va = int(receipt["va"], 16)
        by_status[receipt["status"]] = by_status.get(receipt["status"], 0) + 1
        first = receipt.get("first_divergence") or {}
        functions.append(
            {
                "va": receipt["va"],
                "status": receipt["status"],
                "calls": weights.get(va, 0),
                "compared": receipt["counts"].get("compared", 0),
                "equal": receipt["counts"].get("equal", 0),
                "diverged": receipt["counts"].get("diverged", 0),
                "fp_ulp": receipt["counts"].get("fp_ulp", 0),
                "class": first.get("class", ""),
                "port_module": receipt.get("port_module", ""),
            }
        )
    diverged = [fn for fn in functions if fn["status"] == "diverged"]
    diverged.sort(key=lambda fn: (-fn["calls"], fn["va"]))
    return {
        "schema": 1,
        "task": "T3743",
        "note": "counts and classes only; receipts live in the receipt store",
        "status_counts": dict(sorted(by_status.items())),
        "top_divergences": diverged[:top],
        "functions": sorted(functions, key=lambda fn: fn["va"]),
    }
