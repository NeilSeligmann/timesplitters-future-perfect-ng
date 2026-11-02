# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871: map a `cargo kani` run of one equivalence harness to a verdict.

- `proved_equal`: every check passed, unwinding assertions included, and every `t2s-equal`
  check was reached, so the two bodies agree on every input the harness admits;
- `counterexample`: a `t2s-equal <what>` check failed (a candidate port bug);
- `bound_exceeded`: only unwinding assertions failed (a loop can run past `--unwind`);
- `port_fault`: another check failed (a Rust panic, overflow, pointer or harness assertion), or
  a `t2s-equal` check was unreachable (a vacuous proof is never reported as equal);
- `timeout`: the run hit its time budget or the solver's memory;
- `unsupported`: the harness did not build (C front end, Rust compile, missing body).
"""

from __future__ import annotations

import re

PROVED_EQUAL = "proved_equal"
COUNTEREXAMPLE = "counterexample"
BOUND_EXCEEDED = "bound_exceeded"
PORT_FAULT = "port_fault"
TIMEOUT = "timeout"
UNSUPPORTED = "unsupported"
VERDICTS = (PROVED_EQUAL, COUNTEREXAMPLE, BOUND_EXCEEDED, PORT_FAULT, TIMEOUT, UNSUPPORTED)
EQUAL_PREFIX = "t2s-equal "
# CBMC gave up for lack of memory (the 6 GB cap of tools.t2symbolic.limits) or crashed.
RESOURCE_MARKERS = ("CBMC failed", "run out of memory", "ran out of memory")

_CHECK = re.compile(r'^Check \d+: \S+\n\t - Status: (\w+)\n\t - Description: "(.*)"$', re.M)
_ERROR = re.compile(r"^(error(?:\[\w+\])?: .*)$", re.M)
_PLAYBACK_VALUE = re.compile(r"^\s*// (.+)\n\s*vec!\[([^\]]*)\]", re.M)

# kani::any() order in templates/harness.rs `check`: one playback value per scalar element.
PLAYBACK_FIELDS = (
    ["span", "eax", "ecx", "edx", "esp", "ebx", "ebp", "esi", "edi", "eflags"]
    + [f"fpu.stack[{i}]" for i in range(8)]
    + ["fpu.top", "fpu.control"]
    + [f"xmm{r}.lane{i}" for r in range(8) for i in range(4)]
    + ["seh_ebp", "fp_cmp", "fp_cc"]
    + [f"arg{i}" for i in range(6)]
    + ["probe"]
)


def checks(output: str) -> list[tuple[str, str]]:
    """(status, description) of every property, in Kani's order."""
    return _CHECK.findall(output)


def failed_checks(output: str) -> list[str]:
    """FAILURE checks, then ERROR ones (checks CBMC could not decide)."""
    found = checks(output)
    failed = [description for status, description in found if status == "FAILURE"]
    return failed + [f"error: {description}" for status, description in found if status == "ERROR"]


def classify(output: str, timed_out: bool) -> dict[str, object]:
    if timed_out:
        return {"verdict": TIMEOUT}
    if any(marker in output for marker in RESOURCE_MARKERS):
        return {
            "verdict": TIMEOUT,
            "reason": "solver resource limit (CBMC failed or out of memory)",
        }
    if "VERIFICATION:- SUCCESSFUL" in output:
        equal = [
            status for status, description in checks(output) if description.startswith(EQUAL_PREFIX)
        ]
        if not equal or any(status != "SUCCESS" for status in equal):
            return {"verdict": PORT_FAULT, "failed": ["t2s-equal check unreachable (vacuous)"]}
        return {"verdict": PROVED_EQUAL}
    if "VERIFICATION:- FAILED" in output:
        failed = failed_checks(output)
        differs = sorted(
            {d.removeprefix(EQUAL_PREFIX) for d in failed if d.startswith(EQUAL_PREFIX)}
        )
        if differs:
            return {"verdict": COUNTEREXAMPLE, "differs": differs}
        if failed and all("unwinding assertion" in d for d in failed):
            return {"verdict": BOUND_EXCEEDED}
        return {"verdict": PORT_FAULT, "failed": failed[:5]}
    errors = _ERROR.findall(output)
    reason = errors[0] if errors else (output.strip().splitlines() or ["no output"])[-1]
    return {"verdict": UNSUPPORTED, "reason": reason[:300]}


def playback_values(output: str) -> dict[str, str]:
    """The register-side inputs of a Kani concrete-playback test, keyed by harness field.

    Initial guest memory is unconstrained C-side state and is not part of the playback; the
    fuzz replay or a capture supplies concrete bytes for it."""
    values = [comment.strip() for comment, _ in _PLAYBACK_VALUE.findall(output)]
    return dict(zip(PLAYBACK_FIELDS, values, strict=False))
