"""T3877 dispatch gate: current real calls and receipts before allocating a fix lane.

Only requested VAs are read from the corpus index; never scan replay runs or load
the corpus wholesale. No game replay or comparison runs inside queue validation.
"""

from __future__ import annotations

import functools
import json
import sqlite3
from pathlib import Path

from tools import datadir, t2capture_promote
from tools.t2capture import live_targets, requests, runner
from tools.t2corpus.spool import MAX_RECORD_BYTES, Record, decode_record, decode_regs
from tools.t2corpus.store import Corpus

FIX_KINDS = {"diagnose", "diverged_gated"}
MAX_RECEIPT_BYTES = 1024 * 1024
PREVIEW_BYTES = 2048


class Prerequisites:
    def __init__(
        self,
        root: Path,
        *,
        store: Path | None = None,
        receipt_root: Path | None = None,
        request_dir: Path | None = None,
        currents: t2capture_promote.Currents | None = None,
    ) -> None:
        self.root = root
        self.store = store if store is not None else datadir.data_dir(live_targets.STORE)
        self.receipt_root = runner.receipt_root(receipt_root)
        self.requests = request_dir
        current = currents or t2capture_promote.real_currents(root)
        # A refill may inspect several functions from the same port module.
        self.compare = functools.cache(current.compare_logic)
        self.lift = functools.cache(current.lifted_tree_hash)
        self.commit = functools.cache(current.port_source_commit)

    def check(self, spec: dict, *, route: bool = False) -> list[str]:
        failures = []
        functions = spec.get("functions")
        if not isinstance(functions, list) or not functions:
            return ["malformed-functions"]
        for function in functions:
            if not isinstance(function, dict):
                failures.append("malformed-function")
                continue
            function.pop("comparator", None)  # never trust an earlier dispatch's evidence
            try:
                va = int(str(function["va"]), 16)
            except (KeyError, TypeError, ValueError):
                failures.append("malformed-va")
                continue
            receipt_path = runner.receipt_path(self.receipt_root, va)
            try:
                reason, action, context = self.inspect(va, spec.get("kind", ""), receipt_path)
            except (OSError, ValueError, KeyError, TypeError, sqlite3.Error) as exc:
                reason = f"unreadable-prerequisite:{type(exc).__name__}"
                action, context = "refresh", None
            if reason:
                failures.append(f"{function['va']}:{reason}")
                if route:
                    requests.submit(
                        action, va, str(spec["id"]), reason, receipt_path, self.requests
                    )
            elif context is not None:
                function["comparator"] = context
        return failures

    def inspect(self, va: int, kind: str, path: Path) -> tuple[str, str, dict | None]:
        tree = self.lift()
        if not tree:
            return "current-lift-unknown", "refresh", None
        try:
            corpus = Corpus(self.store, readonly=True)
        except (OSError, sqlite3.Error):
            return "capture-missing", "capture", None
        try:
            rows = corpus.entry_rows(tree, va)
            usable = [row for row in rows if not row["flags"]]
            if not usable:
                return "capture-missing-or-incomplete", "capture", None
            # Check the actual shard, not just a dangling sqlite row.
            try:
                sample = self.record(corpus, usable[0])
            except (OSError, ValueError):
                return "capture-unreadable", "capture", None
            if sample.va != va:
                return "capture-va-mismatch", "capture", None
            if not path.is_file():
                return "receipt-missing", "refresh", None
            if path.stat().st_size > MAX_RECEIPT_BYTES:
                return "receipt-oversized", "refresh", None
            receipt = json.loads(path.read_text())
            if not isinstance(receipt, dict):
                return "receipt-invalid", "refresh", None
            if (
                receipt.get("schema") != runner.SCHEMA
                or receipt.get("kind") != runner.KIND
                or int(str(receipt.get("va")), 16) != va
            ):
                return "receipt-invalid", "refresh", None
            identities = [
                ("compare_logic", self.compare()),
                ("lifted_tree_hash", tree),
                ("corpus_digest", runner.corpus_digest([row["digest"] for row in usable])),
            ]
            # An unported function still needs a fresh lifted/no-ABI receipt, but
            # there is no Rust source commit until its first port exists.
            if kind != "port" or receipt.get("port_module"):
                identities.append(
                    ("port_source_commit", self.commit(receipt.get("port_module", "")))
                )
            for key, value in identities:
                if not value or receipt.get(key) != value:
                    return f"receipt-stale-{key}", "refresh", None
            context = {
                "receipt": str(path),
                "receipt_sha256": requests.digest(path),
                "status": receipt.get("status"),
            }
            first = receipt.get("first_divergence")
            if kind in FIX_KINDS and receipt.get("status") not in {"diverged", "equal"}:
                return "receipt-needs-reclassification", "refresh", None
            if receipt.get("status") == "diverged":
                if not isinstance(first, dict) or not first.get("detail"):
                    return "divergence-missing", "refresh", None
                digest = first.get("provenance", {}).get("digest")
                row = next((row for row in usable if row["digest"] == digest), None)
                if row is None:
                    return "divergence-capture-missing", "refresh", None
                record = self.record(corpus, row)
                context.update(
                    {
                        "first_divergence": first,
                        "args": {
                            "entry_registers": decode_regs(record.entry_regs),
                            "ext_registers_hex": record.entry_regs.hex(),
                            "stack_arguments": "captured stack bytes are in reads (ESP above)",
                        },
                        "reads": self.runs(record.read_runs),
                        "writes": self.runs(record.write_runs),
                    }
                )
            return "", "", context
        finally:
            corpus.close()

    @staticmethod
    def record(corpus: Corpus, row: sqlite3.Row) -> Record:
        if not 0 < row["shard_length"] <= MAX_RECORD_BYTES:
            raise ValueError("capture record length out of bounds")
        return decode_record(corpus.raw_record(row))

    @staticmethod
    def runs(runs: list) -> list[dict]:
        return [{"address": f"0x{address:08X}", "bytes": data.hex()} for address, data in runs]


def brief_evidence(function: dict) -> list[str]:
    context = function.get("comparator")
    if not context:
        return []
    lines = [
        f"Comparator {function['va']}: {context['receipt']} "
        f"sha256={context['receipt_sha256']} status={context['status']}"
    ]
    first = context.get("first_divergence")
    if context["status"] == "equal":
        lines.append(
            "Latest comparison is EQUAL; validate/promote this receipt before changing code."
        )
    if first:
        lines.append("First differing field (verbatim comparator): " + first["detail"])
        lines.append("Call provenance: " + json.dumps(first, sort_keys=True))
        for field in ("args", "reads", "writes"):
            value = json.dumps(context[field], sort_keys=True)
            if len(value) > PREVIEW_BYTES:
                value = value[:PREVIEW_BYTES] + " … [full value in evidence artifact]"
            lines.append(f"{field}: {value}")
        if context.get("artifact"):
            lines.append("Exact captured inputs and expected writes: " + context["artifact"])
    return lines


def save_evidence(spec: dict, specs: Path) -> None:
    for function in spec["functions"]:
        context = function.get("comparator")
        if context and context.get("first_divergence"):
            path = specs / "evidence" / f"{spec['id']}-{int(function['va'], 16):08X}.json"
            requests.atomic_json(path, context)
            context["artifact"] = str(path.resolve())
