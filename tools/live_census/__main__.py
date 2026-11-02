# SPDX-License-Identifier: GPL-3.0-or-later
"""Live hit census CLI (T3824, docs/t3824-live-hit-census.md).

python3 -m tools.live_census gate            ON or OFF: build.sh's TSFP_BUILD_LIVE_CENSUS default
python3 -m tools.live_census persist RUN_DIR  copy RUN_DIR/census-hits.bin into tmp/live-census
python3 -m tools.live_census runs             one line per live run (lift key, build, ports, hits)
python3 -m tools.live_census report           ran-live-uncaptured vs never-ran, per subsystem
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from tools.live_census import gate, report, store


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.live_census", description=__doc__.splitlines()[0]
    )
    sub = parser.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("gate", help="print ON or OFF from tools/data/live_census.json")
    g.add_argument("--config", type=Path, default=None)
    g.add_argument("--explain", action="store_true")
    p = sub.add_parser("persist", help="copy a run's census-hits.bin into the durable store")
    p.add_argument("run_dir", type=Path)
    for name in ("runs", "report"):
        s = sub.add_parser(name)
        s.add_argument("--lift-key", default=None, help="refuse files of any other lift")
        s.add_argument("--build-sha", default=None, help="only runs of this build")
        s.add_argument("--port-set", default=None, help="only runs of this port set (see `runs`)")
    args = parser.parse_args(argv)

    if args.cmd == "gate":
        config = gate.load(args.config)
        print(
            gate.explain(config) if args.explain else ("ON" if gate.default_on(config) else "OFF")
        )
        return 0
    if args.cmd == "persist":
        target = store.persist(args.run_dir)
        print(
            f"live census: {target}"
            if target
            else f"live census: {args.run_dir} wrote no {store.RUN_FILE}"
        )
        return 0

    collected = store.collect(expected_lift_key=args.lift_key)
    for line in collected.refused:
        print(f"live census: refused {line}", file=sys.stderr)
    runs = [
        c
        for c in collected.runs
        if (args.build_sha is None or c.build_sha == args.build_sha)
        and (args.port_set is None or c.port_set == args.port_set)
    ]
    if not runs:
        print(
            f"live census: 0 live runs yet (store {store.store_dir()}, run dirs {store.play_dir()})"
        )
        return 0
    if args.cmd == "runs":
        for c in runs:
            hits = len(c.hit_vas())
            print(
                f"{Path(c.source).name} lift {c.lift_key} build {c.build_sha[:12]} "
                f"ports {c.port_set} "
                f"{hits}/{len(c.vas)} hit ({c.reason})"
            )
        return 0
    from tools.t2reach.audit import execution_evidence, port_rows

    rows = port_rows()
    replay_executed, _where, _counts = execution_evidence(rows, include_live=False)
    live = store.union(runs)
    print(report.render(report.breakdown(rows, frozenset(replay_executed), live), len(runs)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
