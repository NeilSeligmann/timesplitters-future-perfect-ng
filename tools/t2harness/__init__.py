# SPDX-License-Identifier: GPL-3.0-or-later
"""Constructed-call Tier 2 harness for class d functions (T3796,
docs/t3796-constructed-call-tier2.md).

Class d of the T3793 audit (`tools.t2reach`) are never-executed port functions whose code closure is
small and self-contained. This package fabricates their inputs (`generate`), runs the ORIGINAL
lifted code on them in a capture host to record reference corpus entries (`construct`, host mode
`--t2-construct`), and judges the ports with the unchanged `tools.t2capture` runner. Receipts carry
`tier2_source: "constructed-call"` and `inputs: {"kind": "fabricated", ...}` (`receipts`):
FABRICATED inputs prove agreement on the constructed domain only, never recording evidence (rule
9(b) label).
"""
