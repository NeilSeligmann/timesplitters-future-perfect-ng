# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871: recording-free verification of port functions (docs/t-symbolic-equivalence-plan.md).

A) bounded symbolic equivalence: the lifted C body and the Rust port run in one Kani/CBMC product
   program over a shared symbolic guest memory (`harness`, `verdict`);
B) coverage-guided differential fuzzing: the same pair under libFuzzer (`fuzz`).

`inventory` measures which never-executed port functions are leaves and picks the pilot set.
Results are a report only: no Tier 2 credit (rule 43 stays the credit rule).
"""
