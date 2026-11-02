# SPDX-License-Identifier: GPL-3.0-or-later
"""Recapture every owner recording on remote workers (T3740, rule 43): orchestration only.

Each capture is `tools.wt replay submit --remote --t2-capture-host <host>` (remote staging through
the private input cache, remote compaction, bundle pull and ingest are in tools.wt.remote_replay),
restricted to one worker through TSFP_REPLAY_REMOTE_HOSTS and into a fresh record-format-2 store
through TSFP_T2_CORPUS_NAME. This module adds the recording list (shortest first), one capture per
host with at most `--hosts` running at once, a disk check before each capture, per-recording
measurements from the store, and a coord status line every 15 minutes. Never captures locally.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import queue
import subprocess
import sys
import threading
import time
from pathlib import Path

from tools import datadir

CAPTURE_CAPABILITY = "capture"


def workers(registry_path: Path | None = None) -> dict[str, dict[str, object]]:
    """Capture hosts from tools/host_registry (rule 45): every non-retired host with the
    `capture` capability, as {name: target, port, data_path}. No hand-kept host list."""
    from tools import host_registry
    from tools.wt import retired_hosts

    found: dict[str, dict[str, object]] = {}
    for entry in retired_hosts.active(host_registry.hosts_or_empty(registry_path)):
        if CAPTURE_CAPABILITY not in entry.get("capabilities", []):
            continue
        paths = entry.get("data_paths") or [entry.get("data_root", "~")]
        found[entry["name"]] = {
            "target": f"{entry['user']}@{entry['address']}",
            "port": int(entry.get("port", 22)),
            "data_path": paths[0],
        }
    return found


RETIRED_HOSTS_FILE = Path(__file__).resolve().parents[1] / "data" / "remote_hosts.json"
STORE_NAME = "t2corpus-f2"
STATUS_CHECK_S = 60.0
STATUS_MIN_GAP_S = 1800.0
"""T3794: claude-1 is woken by every coord line: a status goes out only when a recording finished or
failed since the last one, and at most once per STATUS_MIN_GAP_S."""


def should_post(
    last: tuple[int, int] | None, last_at: float, done: int, failed: int, now: float
) -> bool:
    """True when (done, failed) changed since the last post and the minimum gap has passed."""
    if last == (done, failed):
        return False
    return last is None and (done or failed) > 0 or now - last_at >= STATUS_MIN_GAP_S


def default_slots(target: str) -> int:
    """Parallel captures for a host named without `:N`: its remote replay cap
    (`remote_replay.REMOTE_HOST_CAPS`, the same bound every replay dispatch applies), else 1.

    T3798: a bare `--hosts laptop` meant ONE capture at a time, so 44 recordings queued behind
    one 29 min recording while the laptop had 4 replay exec dirs and >30 GB free (rule 45)."""
    from tools.wt import remote_replay

    return max(1, remote_replay.REMOTE_HOST_CAPS.get(target.rsplit("@", 1)[-1], 1))


def usable_slots(hosts_arg: str, retired: frozenset[str]) -> dict[str, int]:
    """`--hosts` as {host: slots}, dropping retired and unknown hosts with a notice (T3753).
    A host without `:N` gets `default_slots` (T3798)."""
    known = workers()
    slots: dict[str, int] = {}
    for item in filter(None, hosts_arg.split(",")):
        name, _, count = item.partition(":")
        if name in retired:
            print(f"recapture: host {name} is retired, skipped", file=sys.stderr)
        elif name not in known:
            print(f"recapture: unknown host {name}, skipped", file=sys.stderr)
        else:
            slots[name] = int(count) if count else default_slots(str(known[name]["target"]))
    return slots


def recordings(directory: Path, skip: set[str]) -> list[Path]:
    found = [p for p in directory.glob("*.rec") if p.stem not in skip]
    return sorted(found, key=lambda p: (p.stat().st_size, p.name))


def file_sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def free_gb(host: str, path: str | None = None) -> float | None:
    spec = workers()[host]
    cmd = ["ssh", "-p", str(spec["port"]), "-o", "BatchMode=yes", "-o", "ConnectTimeout=20",
           str(spec["target"]), f"df -BG --output=avail {path or spec['data_path']} | tail -1"]  # fmt: skip  # noqa: E501
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=60, check=False).stdout
        return float(out.strip().rstrip("G"))
    except (ValueError, subprocess.TimeoutExpired, OSError):
        return None


def disk_ok(host: str) -> str | None:
    """None when the capture may start, else the breached floor (root 3 GB, data per host)."""
    checks = [("data", free_gb(host), floor_gb(host)), ("root", free_gb(host, "/"), ROOT_FLOOR_GB)]
    for name, free, floor in checks:
        if free is None or free < floor:
            return f"{name} free {free} GB < floor {floor}"
    return None


ROOT_FLOOR_GB = 3.0


def floor_gb(host: str) -> float:
    """Data floor: the host's disk_floor_gb (tools.datadisk, as TSFP_DISK_PROFILE=remote_worker
    resolves it) plus two capture-run caps."""
    from tools import datadisk

    budget = json.loads(Path("tools/data/disk_budget.json").read_text())
    return datadisk.remote_disk_floor(host) + 2 * float(budget["capture_run_max_gb"])


def store_facts(sha: str) -> dict[str, int]:
    """What one recording contributed to the fresh store (MEASURED from the store itself)."""
    from tools.t2corpus.store import Corpus

    corpus = Corpus(datadir.data_dir(STORE_NAME))
    try:
        like = sha + "#%"
        records, kept, units = corpus.db.execute(
            "SELECT COALESCE(SUM(record_count),0), COALESCE(SUM(kept_count),0), COUNT(*)"
            " FROM recordings WHERE recording_sha256 LIKE ?",
            (like,),
        ).fetchone()
        functions, flagged = corpus.db.execute(
            "SELECT COUNT(DISTINCT va), COALESCE(SUM(flags != 0),0) FROM entries"
            " WHERE recording_sha256 = ?",
            (sha,),
        ).fetchone()
        return {"units": units, "records": records, "kept": kept, "functions": functions,
                "flagged": flagged}  # fmt: skip
    finally:
        corpus.close()


def caps_suffix(seed: Path | None) -> str:
    """T3836: the store's `@caps<b>x<c>` unit suffix a seed's raised size caps produce, or ''."""
    if seed is None:
        return ""
    from tools.t2capture.budget_seed import directives_env
    from tools.t2corpus.spool import SizeCaps

    env = directives_env(Path(seed).read_text())
    caps = SizeCaps(
        int(env.get("TSFP_T2_CAPTURE_MAX_SET_BYTES", SizeCaps().max_set_bytes)),
        int(env.get("TSFP_T2_CAPTURE_MAX_CALLEES", SizeCaps().max_callees)),
    )
    return "" if caps.is_default else f"@caps{caps.max_set_bytes}x{caps.max_callees}"


def recaptured(sha: str, version: int | None = None, suffix: str = "") -> bool:
    """T3767: the store holds an ingest unit of this recording made by capture runtime `version`
    (default: the current one). A recording captured by an older runtime is captured again; the
    store then supersedes its older entries (tools/t2corpus/store.py). T3836: with `suffix`
    (raised size caps) only a unit captured with those caps counts."""
    from tools.t2corpus.store import CAPTURE_VERSION_CURRENT, CAPTURE_VERSION_DEFAULT, Corpus

    wanted = CAPTURE_VERSION_CURRENT if version is None else version
    pattern = (
        sha
        + "#%"
        + (f"@v{wanted}" if wanted != CAPTURE_VERSION_DEFAULT else "")
        + ("%" if suffix else "")
    )
    corpus = Corpus(datadir.data_dir(STORE_NAME))
    try:
        rows = corpus.db.execute(
            "SELECT recording_sha256 FROM recordings WHERE recording_sha256 LIKE ?", (pattern,)
        ).fetchall()
    finally:
        corpus.close()
    if suffix:
        return any(row[0].endswith(suffix) for row in rows)
    rows = [row for row in rows if "@caps" not in row[0]]
    if wanted == CAPTURE_VERSION_DEFAULT:
        return any("@v" not in row[0] for row in rows)
    return bool(rows)


def store_totals() -> str:
    from tools.t2corpus.store import Corpus

    corpus = Corpus(datadir.data_dir(STORE_NAME))
    try:
        entries = corpus.db.execute("SELECT COUNT(*) FROM entries").fetchone()[0]
        functions = corpus.db.execute("SELECT COUNT(DISTINCT va) FROM entries").fetchone()[0]
        return f"{functions} functions, {entries} entries"
    finally:
        corpus.close()


def budget_seed_path(choice: str, results: Path) -> Path | None:
    """The T3803 seed file for every capture of this run: generated once from STORE_NAME (auto)."""
    if choice == "none":
        return None
    if choice != "auto":
        return Path(choice).resolve()
    from tools.t2capture import budget_seed

    out = results.parent.resolve() / "budget-seed.txt"
    tree, count = budget_seed.write(out, datadir.data_dir(STORE_NAME))
    print(f"recapture: budget seed {out}: {count} VAs, tree {tree}", file=sys.stderr)
    return out


BUSY_BACKOFF_S = 60.0
BUSY_MARK = "replay exec dirs on "


def host_busy(entry: dict[str, object]) -> bool:
    """The capture was refused only because every exec dir of the host was in use (T3794)."""
    return (
        entry.get("rc") != 0
        and BUSY_MARK in str(entry.get("tail", ""))
        and "are busy" in str(entry.get("tail", ""))
    )


def submit_receipt(stdout: str) -> dict[str, object]:
    """The last JSON object line `tools.wt replay submit` printed (its receipt), or {}."""
    for line in reversed((stdout or "").splitlines()):
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict):
            return value
    return {}


def route_end_facts(receipt: dict[str, object], rec: Path) -> dict[str, object]:
    """T3794: did the capture replay reach the recording's route end? Reads the trace.json meta
    normal replays write (route_end_poll, recorded_route_end_poll). A missing poll is not reached.
    Independent of the capture budget: budget-skipped entries never shorten the replay itself."""
    meta: dict[str, object] = {}
    run_dir = receipt.get("run_dir")
    if run_dir:
        try:
            loaded = json.loads((Path(str(run_dir)) / "trace.json").read_text())
            meta = loaded.get("meta") or {} if isinstance(loaded, dict) else {}
        except (OSError, ValueError):
            meta = {}
    reached, recorded = meta.get("route_end_poll"), meta.get("recorded_route_end_poll")
    if not isinstance(recorded, int):
        try:
            from tools.rust_trace import native_input_route_end_poll

            recorded = native_input_route_end_poll(rec)
        except Exception as error:  # noqa: BLE001 - degrade loudly: no recorded poll = short
            print(f"recapture: no recorded route end for {rec.stem}: {error}", file=sys.stderr)
            recorded = None
    reached = reached if isinstance(reached, int) else None
    recorded = recorded if isinstance(recorded, int) else None
    ok = reached is not None and recorded is not None and reached >= recorded
    # T3819: a capture whose game state left the owner's path (desync_poll set) is partial too
    desync = meta.get("desync_poll")
    desync = desync if isinstance(desync, int) else None
    hint = (meta.get("desync") or {}).get("desync_hint") is True
    return {"route_end_poll": reached, "recorded_route_end_poll": recorded,
            "reached_route_end": ok, "desync_poll": desync, "desync_hint": hint}  # fmt: skip


DESYNC_NOTE = (
    "partial: replay desynced from the owner's session at poll {poll} (T3819); calls after it are "
    "real original-code calls on a wrong game state; ingested"
)
SHORT_NOTE = "partial: capture stopped short of the recorded route end (or has no poll); ingested"


def outcome(entry: dict[str, object]) -> str:
    """done / short / failed. Policy (T3794): an rc-0 capture that did not reach the recorded
    route end is ingested with a 'partial' note and counted short (never done), not requeued."""
    if entry.get("rc") != 0:
        return "failed"
    if entry.get("desync_poll") is not None or entry.get("desync_hint") is True:
        return "short"
    return "done" if entry.get("reached_route_end") is True else "short"


def release_failed_claim(entry: dict[str, object], claim: Path) -> None:
    """T3836 (I-T3836-2): a failed capture gives its claim back, so the next run retries it. A
    kept claim made a rerun skip the recording without a result line."""
    if outcome(entry) == "failed":
        claim.unlink(missing_ok=True)


def outcome_counts(entries: list[dict[str, object]]) -> dict[str, int]:
    counts = {"done": 0, "short": 0, "failed": 0}
    for entry in entries:
        counts[outcome(entry)] += 1
    return counts


def summary_line(entries: list[dict[str, object]], total: int) -> str:
    counts = outcome_counts(entries)
    return (f"recapture SUMMARY done={counts['done']}/{total} short={counts['short']} "
            f"failed={counts['failed']}")  # fmt: skip


def capture(host: str, rec: Path, args: argparse.Namespace, slots: int = 1) -> dict[str, object]:
    spec = workers()[host]
    # T3760: no `repo`: the tree goes to the host's exec dir (tools/data/remote_hosts.json).
    remote_workers = [{"name": host, "target": spec["target"], "port": spec["port"],
                "max_slots": slots}]  # fmt: skip
    env = {**__import__("os").environ, "TSFP_REPLAY_REMOTE_HOSTS": json.dumps(remote_workers),
           "TSFP_T2_CORPUS_NAME": STORE_NAME}  # fmt: skip
    command = [sys.executable, "-m", "tools.wt", "replay", "submit", "--route", str(rec),
               "--switches", "off", "--commit", args.commit, "--t2-capture-host", args.host,
               "--remote", "--no-cache"]  # fmt: skip
    if getattr(args, "seed_path", None):  # T3803: VAs the store already covers are not re-recorded
        command += ["--t2-budget-seed", str(args.seed_path)]
    started = time.monotonic()
    done = subprocess.run(command, env=env, capture_output=True, text=True, check=False,
                          timeout=args.timeout)  # fmt: skip
    sha = file_sha(rec)
    result: dict[str, object] = {"recording": rec.stem, "host": host,
                                 "wall_s": round(time.monotonic() - started), "rc": done.returncode}  # fmt: skip  # noqa: E501
    if done.returncode:
        result["tail"] = ((done.stdout or "") + (done.stderr or ""))[-800:]
    else:
        result.update(store_facts(sha))
        result.update(route_end_facts(submit_receipt(done.stdout), rec))
        if not result["reached_route_end"]:
            result["note"] = SHORT_NOTE
        elif result.get("desync_poll") is not None:
            result["note"] = DESYNC_NOTE.format(poll=result["desync_poll"])
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--hosts", default="laptop", help="comma list host[:slots] (no :N = the host's replay cap)"
    )
    parser.add_argument("--longest-first", action="store_true")
    parser.add_argument("--host", required=True, help="local copy of the capture tsfp-bevy")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--recordings", default="tmp/owner/recordings")
    parser.add_argument("--skip", default="session-c-20261025", help="comma list of stems")
    parser.add_argument("--only", default="", help="T3767: comma list of stems to capture (all)")
    parser.add_argument("--limit", type=int, default=0, help="stop after N recordings (0 = all)")
    parser.add_argument("--timeout", type=float, default=7200)
    parser.add_argument("--results", default="tmp/t2recap/results.jsonl")
    parser.add_argument(
        "--budget-seed",
        default="auto",
        help="T3803: per-VA capture budget seed: auto (from the store, next to --results), none, "
        "or a file from tools.t2capture.budget_seed",
    )
    args = parser.parse_args(argv)
    args.seed_path = budget_seed_path(args.budget_seed, Path(args.results))
    from tools.wt import retired_hosts

    slot_map = usable_slots(args.hosts, retired_hosts.retired_names(RETIRED_HOSTS_FILE))
    if not slot_map:
        print("recapture: no usable host left in --hosts", file=sys.stderr)
        return 2
    hosts = [h for h, n in slot_map.items() for _ in range(n)]
    work: queue.Queue[Path] = queue.Queue()
    pending = recordings(Path(args.recordings), set(filter(None, args.skip.split(","))))
    only = set(filter(None, args.only.split(",")))
    if only:
        missing = only - {rec.stem for rec in pending}
        if missing:
            parser.error(f"--only names unknown recordings: {', '.join(sorted(missing))}")
        pending = [rec for rec in pending if rec.stem in only]
    if args.longest_first:
        pending.reverse()
    suffix = caps_suffix(args.seed_path)
    for rec in pending:
        if not recaptured(
            file_sha(rec), suffix=suffix
        ):  # captured by the current runtime already: skipped
            work.put(rec)
            if args.limit and work.qsize() >= args.limit:
                break
    total, finished = work.qsize(), []
    lock = threading.Lock()
    host_failures: dict[str, int] = {}
    results = Path(args.results)
    results.parent.mkdir(parents=True, exist_ok=True)

    def worker(host: str) -> None:
        slots = slot_map[host]
        failures = 0
        while (
            failures < 2 and host_failures.get(host, 0) < 2
        ):  # an infrastructure fault twice in a row stops this host (rule 33)
            try:
                rec = work.get_nowait()
            except queue.Empty:
                return
            claim = Path(args.results).parent / "claims" / rec.stem
            claim.parent.mkdir(parents=True, exist_ok=True)
            try:  # cross-process claim: two drivers (droplets, laptop) never take the same recording  # noqa: E501
                claim.open("x").close()
            except FileExistsError:  # T3836: said, never silent (a leftover claim skips it)
                print(f"recapture: {rec.stem} skipped, claim {claim} exists", file=sys.stderr)
                continue
            breach = disk_ok(host)
            if breach:
                with lock:
                    entry: dict[str, object] = {"recording": rec.stem, "host": host,
                                                "skipped": breach}  # fmt: skip
                    results.open("a").write(json.dumps(entry) + "\n")
                claim.unlink()
                work.put(rec)  # another host may take it
                return
            try:
                entry = capture(host, rec, args, slots)
            except subprocess.TimeoutExpired:
                entry = {"recording": rec.stem, "host": host, "rc": "timeout"}
            if host_busy(entry):
                # T3794: other replays held every exec dir; not a host fault, so requeue the
                # recording and back off instead of counting it (two counted busies stopped
                # every laptop thread).
                claim.unlink(missing_ok=True)
                work.put(rec)
                time.sleep(BUSY_BACKOFF_S)
                continue
            failures = 0 if entry.get("rc") == 0 else failures + 1
            host_failures[host] = 0 if entry.get("rc") == 0 else host_failures.get(host, 0) + 1
            with lock:
                finished.append(entry)
                results.open("a").write(json.dumps(entry) + "\n")
            release_failed_claim(entry, claim)
            print(json.dumps(entry), flush=True)

    def status() -> None:
        last: tuple[int, int] | None = None
        last_at = time.monotonic()
        while any(t.is_alive() for t in threads):
            time.sleep(STATUS_CHECK_S)
            counts = outcome_counts(finished)
            done, failed, short = counts["done"], counts["failed"], counts["short"]
            if not should_post(last, last_at, done + short, failed, time.monotonic()):
                continue
            last, last_at = (done + short, failed), time.monotonic()
            line = (f"REA recapture STATUS hosts={','.join(hosts)} done {done}/{total} "
                    f"corpus {store_totals()} failed={failed} short={short}")  # fmt: skip
            subprocess.run([sys.executable, "-m", "tools.agents.coord", "say", "--owner",
                            os.environ.get("TSFP_SESSION", "claude-1"), "--to", "claude-1", line],
                           check=False, timeout=120)  # fmt: skip

    threads = [threading.Thread(target=worker, args=(h,)) for h in hosts]
    for thread in threads:
        thread.start()
    threading.Thread(target=status, daemon=True).start()
    for thread in threads:
        thread.join()
    print(summary_line(finished, total), flush=True)
    return 0 if all(outcome(e) == "done" for e in finished) else 1


if __name__ == "__main__":
    sys.exit(main())
