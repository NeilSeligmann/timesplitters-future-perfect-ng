# SPDX-License-Identifier: GPL-3.0-or-later
"""Disposition-owned platform summary for tools.port_heatmap (T3881).

Guest milestones are counts, never blanket claims about a host replacement layer.
Host retirement profiles are separate citations; no replay caches are read.
"""

from __future__ import annotations

import hashlib
import json
import textwrap
from collections import Counter
from pathlib import Path

from tools.ledger import dispositions, execution

PLAN = "docs/t-platform-replacement-plan.md"
DOC = "docs/port-heatmap.md"
START = "<!-- platform-progress:start -->"
END = "<!-- platform-progress:end -->"
NAMES = ("platform-progress.svg", "platform-progress.png")
LAYOUT_VERSION = 3
PLATFORM_STYLE_VERSION = 1


def owner_prefix(subsystem: str, table: dict[str, str]) -> str | None:
    parts = subsystem.split("/")
    return next(
        ("/".join(parts[:n]) for n in range(len(parts), 0, -1) if "/".join(parts[:n]) in table),
        None,
    )


def plan_steps(text: str, layers: list[str]) -> dict[str, str]:
    """Read the owner's summary table by header, not line number or duplicated stage data."""
    header: list[str] = []
    result: dict[str, str] = {}
    for line in text.splitlines():
        if not line.startswith("|"):
            header = []
            continue
        cells = [part.strip() for part in line.strip("|").split("|")]
        if "Layer" in cells and "Remaining steps" in cells:
            header = cells
            continue
        if not header or len(cells) != len(header):
            continue
        values = dict(zip(header, cells, strict=True))
        for layer in layers:
            if layer in values["Layer"]:
                result[layer] = values["Remaining steps"]
    return result


def compute(root: Path, rows: list[dict], prefix: str = "") -> list[dict]:
    full = dispositions.load_full(root / dispositions.DEFAULT_PATH)
    table = {key: entry["d"] for key, entry in full["systems"].items()}
    layers = sorted(key for key, d in table.items() if d == "replace")
    steps = plan_steps((root / PLAN).read_text(), layers)
    weights = execution.load(root / execution.DEFAULT_PATH)
    if weights is None:
        raise ValueError(f"missing execution weights: {execution.DEFAULT_PATH}")
    total_calls = weights.total_calls()
    config = json.loads((root / "tools/data/retirement_systems.json").read_text())["systems"]
    retired = {
        r["system"]
        for r in json.loads((root / "docs/data/retirements.json").read_text())["retired"]
    }
    result = []
    for layer in layers:
        if prefix and not (layer.startswith(prefix) or prefix.startswith(layer + "/")):
            continue
        members = [r for r in rows if owner_prefix(r["subsystem"], table) == layer]
        host = []
        for name, entry in sorted(config.items()):
            if entry.get("kind") != "replace" or entry.get("subsystem") != layer:
                continue
            evidence = entry["replacement"]
            run = evidence.get("verified_180s", {})
            host.append(
                {
                    "profile": name,
                    "retired": name in retired,
                    "routes": len(evidence.get("required_routes", [])),
                    "run_status": run.get("status", "not recorded"),
                    "run_receipt": run.get("receipt", ""),
                    "gate": evidence.get("current_head_gate", ""),
                    "next": evidence.get("remove_after", "not recorded"),
                    "c_lines": sum(
                        len((root / unit["path"]).read_bytes().splitlines())
                        for unit in evidence.get("translation_units", [])
                        if (root / unit["path"]).is_file()
                    ),
                    "c_missing": [
                        unit["path"]
                        for unit in evidence.get("translation_units", [])
                        if name not in retired and not (root / unit["path"]).is_file()
                    ],
                }
            )
        counts = Counter(r["stage"] for r in members)
        calls = sum(weights.calls.get(r["va"], 0) for r in members)
        result.append(
            {
                "layer": layer,
                "functions": len(members),
                "calls": calls,
                "call_share": 100 * calls / total_calls if total_calls else 0,
                "contract": full.get("contracts", {}).get(layer, "contract not recorded"),
                "rust": sum(bool(r["rust"]) for r in members),
                "default_on": sum(bool(r["default_on"]) for r in members),
                "gate_equal": sum(bool(r["gate_equal"]) for r in members),
                "retired": counts["retired"],
                "counts": dict(sorted(counts.items())),
                "next": steps.get(layer, "Not mapped in platform plan; update its Layer column"),
                "host": host,
            }
        )
    return result


def digest(rows: list[dict]) -> str:
    from tools.port_heatmap import REPLACE_STAGES, SCHEMA, STYLE_VERSION

    return hashlib.sha256(
        json.dumps(
            [
                SCHEMA,
                STYLE_VERSION,
                LAYOUT_VERSION,
                PLATFORM_STYLE_VERSION,
                REPLACE_STAGES,
                "solid-stage-bars",
                rows,
            ],
            sort_keys=True,
        ).encode()
    ).hexdigest()


def host_text(host: list[dict]) -> str:
    return (
        "; ".join(
            f"{h['profile']}: {'C retired' if h['retired'] else 'C retained'}; "
            f"{h['routes']} required route citations; 180 s {h['run_status']}; "
            f"current-head gate {'cited' if h['gate'] else 'not cited'}"
            for h in host
        )
        or "No mapped host retirement profile"
    )


def host_brief(host: list[dict]) -> str:
    """Visible host-layer stage per profile for the image (the guest bars cannot show it)."""
    if not host:
        return "Host: no retirement profile mapped"
    parts = []
    for h in host:
        stage = "C RETIRED" if h["retired"] else f"C retained ({h['c_lines']:,} C lines left)"
        if h[
            "c_missing"
        ]:  # rule 45: a live profile listing absent C units is a stale entry, say so
            stage += f", {len(h['c_missing'])} listed C file(s) MISSING"
        parts.append(
            f"{h['profile']}: {stage}, {h['routes']}/3 EQUAL routes, 180 s {h['run_status']}"
        )
    return "Host: " + "; ".join(parts)


HOST_STAGE_COLOURS = {
    "retired": "#1b7a3d",
    "partial": "#9a6a00",
    "retained": "#24313d",
    "none": "#24313d",
}


def host_stage(host: list[dict]) -> str:
    """retired: every host profile of the layer has its C retired; partial: some have."""
    if not host:
        return "none"
    done = sum(h["retired"] for h in host)
    return "retired" if done == len(host) else "partial" if done else "retained"


def row_label(row: dict) -> str:
    stage = host_stage(row["host"])
    if stage == "none":
        return row["layer"]
    names = ", ".join(h["profile"] for h in row["host"] if h["retired"])
    mark = {
        "retired": "HOST C RETIRED",
        "partial": f"host C retired: {names}",
        "retained": "host C retained",
    }
    return f"{row['layer']}\n{mark[stage]}"


def markdown(rows: list[dict]) -> str:
    lines = [
        START,
        "",
        "Guest-function milestones are counts (they may overlap); host evidence is separate.",
        "Call share uses all calls in the tracked execution census. "
        "Gate EQUAL means a tracked citation,",
        "not a fresh gate. Contract text including TBD is reproduced from the disposition owner.",
        "",
        "| Replace layer | Functions | Calls (% of all) | Contract | Rust exists | Default-on "
        "| Gate EQUAL | C retired | Host retirement evidence | Next step (owner plan) |",
        "|---|---:|---:|---|---:|---:|---:|---:|---|---|",
    ]
    for row in rows:
        cells = [
            row["layer"],
            row["functions"],
            f"{row['calls']:,} ({row['call_share']:.3f}%)",
            row["contract"],
            row["rust"],
            row["default_on"],
            row["gate_equal"],
            row["retired"],
            host_text(row["host"]),
            row["next"],
        ]
        lines.append(
            "| " + " | ".join(str(c).replace("|", "\\|").replace("\n", " ") for c in cells) + " |"
        )
    return "\n".join([*lines, "", END])


def update_doc(text: str, rows: list[dict]) -> str:
    block = markdown(rows)
    if START not in text:
        return text.rstrip() + "\n\n" + block + "\n"
    before, rest = text.split(START, 1)
    _, after = rest.split(END, 1)
    return before + block + after


def stale_files(rows: list[dict], out_dir: Path) -> list[str]:
    from tools.port_heatmap import embedded_digest

    return [name for name in NAMES if embedded_digest(out_dir / name) != digest(rows)]


def render(rows: list[dict], out_dir: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.patches import Patch

    from tools.port_heatmap import DIGEST_TAG, REPLACE_STAGES

    matplotlib.rcParams["svg.fonttype"] = "none"
    matplotlib.rcParams["svg.hashsalt"] = "tsfp-platform"
    fig, ax = plt.subplots(figsize=(20, max(7.0, len(rows) * 1.55 + 3.0)))
    fig.subplots_adjust(left=0.14, right=0.50, top=0.82, bottom=0.13)
    ax.set_facecolor("#fbfcfd")
    ax.set_axisbelow(True)
    for index, row in enumerate(rows):
        left = 0.0
        for stage, _, colour in REPLACE_STAGES:
            count = row["counts"].get(stage, 0)
            width = count / row["functions"] if row["functions"] else 0
            ax.barh(
                index,
                width,
                left=left,
                color=colour,
                height=0.68,
                edgecolor="#ffffff",
                linewidth=0.85,
            )
            if width > 0.07:
                ax.text(
                    left + width / 2,
                    index,
                    str(count),
                    ha="center",
                    va="center",
                    color="#ffffff"
                    if stage in {"replace_default", "replace_equal", "retired"}
                    else "#24313d",
                    fontsize=9,
                    fontweight="bold",
                )
            left += width
        detail = (
            f"{row['functions']:,} guest functions  ·  {row['call_share']:.3f}% of calls\n"
            + textwrap.fill(f"Contract: {row['contract']}", 78)
            + "\n"
            + textwrap.fill(f"Next: {row['next']}", 78)
            + "\n"
            + textwrap.fill(host_brief(row["host"]), 78)
        )
        ax.text(
            1.02,
            index,
            detail,
            fontsize=9.1,
            va="center",
            color="#24313d",
            linespacing=1.25,
            clip_on=False,
        )
    for index in range(len(rows) - 1):
        ax.axhline(index + 0.5, color="#d5dce2", linewidth=0.8, zorder=0)
    ax.set_yticks(range(len(rows)), [row_label(r) for r in rows])
    ax.tick_params(axis="y", length=0, pad=12, labelsize=10)
    for label, row in zip(ax.get_yticklabels(), rows, strict=True):
        label.set_fontweight("bold")
        # The bars count guest functions; host retirement would be invisible there, so the
        # row label carries it in colour (owner 2026-10-31).
        label.set_color(HOST_STAGE_COLOURS[host_stage(row["host"])])
    ax.invert_yaxis()
    ax.set_xlim(0, 1)
    ax.set_xticks([0, 0.25, 0.5, 0.75, 1], ["0%", "25%", "50%", "75%", "100%"])
    ax.tick_params(axis="x", labelsize=9)
    ax.set_xlabel("Share of guest functions at each recorded milestone", fontsize=10)
    fig.suptitle("Platform replacement progress", fontsize=20, fontweight="bold", y=0.96)
    fig.text(
        0.14,
        0.90,
        "Milestones count guest functions; they do not certify a whole host layer. "
        "Adopted means default-on while C remains live; ink marks C removed.",
        fontsize=10.5,
        color="#344454",
    )
    fig.text(
        0.515,
        0.855,
        "LAYER CONTRACT AND NEXT STEP",
        fontsize=10,
        fontweight="bold",
        color="#344454",
    )
    fig.legend(
        handles=[
            Patch(
                facecolor=c,
                edgecolor="#65717e",
                linewidth=0.7,
                label=label,
            )
            for _, label, c in REPLACE_STAGES
        ],
        title="REPLACE MILESTONES",
        loc="lower left",
        bbox_to_anchor=(0.13, 0.015, 0.84, 0.09),
        ncol=5,
        frameon=True,
        fancybox=False,
        framealpha=0.95,
        edgecolor="#b8c0c8",
        facecolor="#ffffff",
        fontsize=9,
        title_fontsize=9,
        borderpad=0.65,
    )
    out_dir.mkdir(parents=True, exist_ok=True)
    tag = DIGEST_TAG + digest(rows)
    for name in NAMES:
        metadata = {"Description": tag}
        if name.endswith("svg"):
            metadata["Date"] = None
        fig.savefig(out_dir / name, metadata=metadata)
    svg = out_dir / NAMES[0]
    svg.write_text("\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n")
    plt.close(fig)
