# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871: coverage-guided differential fuzzing of one VA (method B).

Builds, next to the symbolic harness:
- the harness crate as a staticlib with SanitizerCoverage on the Rust side (`sancov-module`,
  inline 8-bit counters, compare tracing), so libFuzzer steers by the port's branches too;
- `glue_fuzz.c` (lifted body, page memory model) and `templates/fuzz_main.c` with
  `clang -fsanitize=fuzzer`, so the lifted side's branches and compares are instrumented as well.

Runs libFuzzer for a fixed time with a fixed seed. A difference aborts the input, which libFuzzer
keeps as `crash-<sha1>`; the run reports it as `counterexample` with the mismatch line.
"""

from __future__ import annotations

import os
import re
import subprocess
import time
from pathlib import Path

from tools.t2symbolic import limits, runner, verdict

SANCOV_RUSTFLAGS = " ".join(
    [
        "-C passes=sancov-module",
        "-C llvm-args=-sanitizer-coverage-level=3",
        "-C llvm-args=-sanitizer-coverage-inline-8bit-counters",
        "-C llvm-args=-sanitizer-coverage-pc-table",
        "-C llvm-args=-sanitizer-coverage-trace-compares",
        "-C debug-assertions=off",
        "-C overflow-checks=off",
    ]
)
NO_DIFFERENCE = "no_difference"
# An input ran past the per-input limit on either side (a guest loop over fabricated memory
# that never ends, e.g. a cyclic list): neither a difference nor evidence.
HANG = "hang"
INPUT_TIMEOUT_SECONDS = 5
SEED = 1
_STATS = re.compile(r"stat::(number_of_executed_units|new_units_added|peak_rss_mb):\s*(\d+)")
_COVERAGE = re.compile(r"cov: (\d+) ft: (\d+)")
_MISMATCH = re.compile(r'^(\{"t2s_mismatch".*\})$', re.M)


def build(harness_dir: Path, va: int, timeout: int = 1200) -> Path:
    """The libFuzzer binary for `va` (harness_dir from runner.prepare)."""
    target = harness_dir.parent.parent / "target-fuzz" / harness_dir.name
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target)
    env["RUSTFLAGS"] = SANCOV_RUSTFLAGS
    subprocess.run(
        [
            "cargo",
            "rustc",
            "--manifest-path",
            str(harness_dir / "Cargo.toml"),
            "--lib",
            "--release",
            "--crate-type",
            "staticlib",
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=timeout,
        env=env,
        cwd=harness_dir,
    )
    # The harness package is `t2s-harness-<crate>` (runner.render_manifest).
    archive = target / "release" / f"libt2s_harness_{harness_dir.name.replace('-', '_')}.a"
    if not archive.is_file():
        raise FileNotFoundError(f"no staticlib {archive}")
    binary = harness_dir / f"fuzz_{va:08x}"
    main = Path(__file__).resolve().parent / "templates" / "fuzz_main.c"
    subprocess.run(
        [
            "clang",
            "-O1",
            "-g",
            "-fsanitize=fuzzer",
            f"-DT2S_VA={va:#x}u",
            f"-I{harness_dir}",
            str(harness_dir / "glue_fuzz.c"),
            str(main),
            str(archive),
            "-lm",
            "-lpthread",
            "-ldl",
            "-o",
            str(binary),
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=timeout,
        cwd=harness_dir,
    )
    return binary


def parse(
    output: str, crashed: bool, artifacts: list[Path], timed_out: bool = False
) -> dict[str, object]:
    stats = {key: int(value) for key, value in _STATS.findall(output)}
    coverage = _COVERAGE.findall(output)
    result: dict[str, object] = {
        "executions": stats.get("number_of_executed_units", 0),
        "corpus_added": stats.get("new_units_added", 0),
        "coverage_edges": int(coverage[-1][0]) if coverage else 0,
        "features": int(coverage[-1][1]) if coverage else 0,
    }
    mismatch = _MISMATCH.findall(output)
    if timed_out or "libFuzzer: timeout" in output:
        result["verdict"] = HANG
        result["artifact"] = str(artifacts[0]) if artifacts else None
    elif crashed or artifacts:
        result["verdict"] = verdict.COUNTEREXAMPLE if mismatch else verdict.PORT_FAULT
        result["mismatch"] = mismatch[0] if mismatch else None
        result["artifact"] = str(artifacts[0]) if artifacts else None
    else:
        result["verdict"] = NO_DIFFERENCE
    return result


def run(harness_dir: Path, case: object, seconds: int) -> dict[str, object]:
    """Fuzz one VA for `seconds` (fixed seed); never grants anything, only reports."""
    va = int(case.va)  # type: ignore[attr-defined]
    started = time.monotonic()
    try:
        binary = build(harness_dir, va)
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        return {"verdict": verdict.UNSUPPORTED, "reason": f"fuzz build: {str(detail)[-300:]}"}
    build_seconds = time.monotonic() - started
    corpus = harness_dir / f"corpus_{va:08x}"
    corpus.mkdir(exist_ok=True)
    prefix = harness_dir / f"artifact_{va:08x}_"
    command = [
        str(binary),
        f"-seed={SEED}",
        f"-max_total_time={seconds}",
        "-print_final_stats=1",
        "-max_len=4096",
        "-rss_limit_mb=4096",
        f"-timeout={INPUT_TIMEOUT_SECONDS}",
        f"-artifact_prefix={prefix}",
        str(corpus),
    ]
    run_started = time.monotonic()
    # Same host limits as the solver (one slot, 6 GB address space, peak RSS measured).
    output, timed_out = runner.run_bounded(command, seconds + 120, None, harness_dir)
    crashed = not timed_out and ("==ERROR: libFuzzer" in output or "deadly signal" in output)
    (harness_dir / f"fuzz_{va:08x}.log").write_text(output)
    artifacts = sorted(harness_dir.glob(f"artifact_{va:08x}_*"))
    result = parse(output, crashed, artifacts, timed_out)
    result["build_seconds"] = round(build_seconds, 2)
    result["peak_rss_mb"] = limits.peak_rss_mb(output)
    result["seconds"] = round(time.monotonic() - run_started, 2)
    return result
