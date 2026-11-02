# SPDX-License-Identifier: GPL-3.0-or-later
"""Reject vacuous WSOLA A/B runs from opt-in sink activity reports."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

REPORT = re.compile(r"^WSOLA activity backend=(c|rust|invalid) calls=(\d+) primes=(\d+)$")


def check(text: str, expected: str) -> tuple[bool, str]:
    if expected not in {"c", "rust"}:
        return False, "unsupported expected backend"
    lines = [line for line in text.splitlines() if line.startswith("WSOLA activity ")]
    if any(REPORT.fullmatch(line) is None for line in lines):
        return False, "malformed activity report"
    reports = [REPORT.fullmatch(line).groups() for line in lines]
    if not reports:
        return False, "missing activity report"
    if any(backend != expected for backend, _, _ in reports):
        return False, "unexpected backend in activity report"
    if not any(
        int(calls) > 0 and int(primes) > 0 and int(primes) <= int(calls)
        for _, calls, primes in reports
    ):
        return False, "no successful prime and operation"
    if any(int(primes) > int(calls) for _, calls, primes in reports):
        return False, "invalid activity counts"
    return True, f"{expected}: {len(reports)} engine report(s), active"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--backend", choices=("c", "rust"), required=True)
    args = parser.parse_args()
    try:
        ok, reason = check(args.log.read_text(errors="replace"), args.backend)
    except OSError:
        ok, reason = False, "log unavailable"
    print(("PASS: " if ok else "INVALID: ") + reason)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
