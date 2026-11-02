"""`python3 -m tools.fleet.runner render --host NAME [--out FILE] [--check FILE] [--what run|rgrc]`.

--check FILE compares the render with FILE and exits 1 on a sha256 mismatch (drift).
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from tools.fleet.config import ConfigError, load_config
from tools.fleet.runner import lane_gate_text, render_runner, rgrc_text, sha256_text


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.fleet.runner", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    render = sub.add_parser("render", help="render the runner for one host")
    render.add_argument("--host", required=True, help="fleet host name (local, laptop, ...)")
    render.add_argument("--what", choices=("run", "rgrc", "lane_gate"), default="run")
    render.add_argument("--out", help="write the render here (default: stdout)")
    render.add_argument("--check", metavar="FILE", help="report drift of FILE against the render")
    args = parser.parse_args(argv)
    try:
        config = load_config()
        renders = {
            "run": lambda: render_runner(config, args.host),
            "rgrc": rgrc_text,
            "lane_gate": lane_gate_text,
        }
        text = renders[args.what]()
    except (ConfigError, KeyError, ValueError) as error:
        print(f"render failed: {error}", file=sys.stderr)
        return 2
    if args.check:
        path = Path(args.check)
        have = sha256_text(path.read_text()) if path.is_file() else "missing"
        want = sha256_text(text)
        if have != want:
            print(f"DRIFT {args.check}: sha256 {have} != rendered {want} ({args.host})")
            return 1
        print(f"ok {args.check} sha256 {want} ({args.host})")
        return 0
    if args.out:
        Path(args.out).write_text(text)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
