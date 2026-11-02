# SPDX-License-Identifier: GPL-3.0-or-later
"""Disposition-aware progress: function treemap and platform layers (T3632, T3881).

    python3 -m tools.port_heatmap --write    # render both image pairs and the platform table
    python3 -m tools.port_heatmap --check    # exit 1 when the image is stale versus the data
    python3 -m tools.port_heatmap --stats    # print stage counts only

Reads owner metadata and the gitignored subsystem classification (docs/port-heatmap.md).
No disc data or replay cache is read. Each image pair embeds a digest of its input table,
so the title date and commit never make --check flaky.
Rendering needs matplotlib and squarify (dependency group `heatmap` in pyproject.toml).
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import json
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = Path("docs/img")
SVG_NAME = "port-progress.svg"
PNG_NAME = "port-progress.png"
DIGEST_RE = re.compile(rb"port-heatmap-digest:([0-9a-f]{64})")
DIGEST_TAG = "port-heatmap-digest:"
SCHEMA = 3
STYLE_VERSION = 3
LAYOUT_VERSION = 3
SUBSYSTEMS = "tools/data/function_subsystems.csv"
SIZE_CAP = 2048  # bytes; one huge function must not dominate its block
SIZE_MIN = 16
SIZE_TAIL = 64

# (key, label, colour). One hue family per category, lightness stepping up with progress so stages
# read at a glance: analysis greys, PORT tier blues, adoption green, REPLACE oranges, C retired ink.
# Hatching is reserved for code that will not be implemented (DROP, SPEC, LIBRARY).
STAGES = (
    ("unknown", "Unknown (no name)", "#f2f2ef"),  # analysis: palest grey
    ("named", "Named (inferred name)", "#bdbdb7"),  # analysis: mid grey
    ("analyzed", "Analyzed (measured name or documented model)", "#77776f"),  # analysis: dark grey
    ("tier0", "Ported, Tier 0 (translated, smoke test)", "#c6dbef"),  # rust: palest blue
    ("tier1", "Tier 1 (differential vs lifted)", "#6baed6"),  # rust: light blue
    ("tier2", "Tier 2 (captured calls / whole-system EQUAL)", "#2171b5"),  # rust: blue
    ("tier3", "Tier 3 (byte/functional comparison)", "#08306b"),  # rust: navy
    ("default_on", "Default-on / adopted (C remains live)", "#2ca25f"),  # adoption: green
    ("retired", "C retired (shared terminal state)", "#101820"),  # ink
)
PORT_STAGES = STAGES
REPLACE_STAGES = (
    ("replace_pending", "Replacement not recorded", "#fee6ce"),
    ("replace_rust", "Rust exists", "#fdae6b"),
    ("replace_default", "Rust default-on", "#e6550d"),
    ("replace_equal", "Gate EQUAL (cited)", "#8c2d04"),
    ("retired", "C retired (shared terminal state)", "#101820"),
)
OTHER_STAGES = (
    ("decision", "Decision recorded", "#ffffff"),
    ("unassigned", "No implementation disposition", "#ffffff"),
)
STAGES = PORT_STAGES[:-1] + REPLACE_STAGES[:-1] + OTHER_STAGES + PORT_STAGES[-1:]
STAGE_KEYS = [key for key, _, _ in STAGES]
TIER_STAGE = {0: "tier0", 1: "tier1", 2: "tier2", 3: "tier3"}
ROWS_NAME = "progress-rows.json"  # snapshot for tools.progress_history, not an image
IMAGE_NAMES = (SVG_NAME, PNG_NAME, "platform-progress.svg", "platform-progress.png")

# Hatch colour is the edge colour in matplotlib, so not-implemented categories are pale cells whose
# texture carries the meaning; undecided is a loud solid magenta because it needs a decision.
CATEGORY_STYLES = {
    "port": {"hatch": None, "face": None, "edge": "#ffffff"},
    "replace": {"hatch": None, "face": None, "edge": "#ffffff"},
    "drop": {"hatch": "xx", "face": "#ffffff", "edge": "#c0392b"},
    "spec": {"hatch": "..", "face": "#ffffff", "edge": "#6a51a3"},
    "library": {"hatch": "////", "face": "#f4f4f2", "edge": "#9a9a94"},
    "undecided": {"hatch": None, "face": "#e7298a", "edge": "#ffffff"},
}


def category_style(disposition: str, stage: str) -> tuple[str, str | None, str]:
    """Return fill, texture and outline for one visible disposition/stage combination."""
    style = CATEGORY_STYLES[disposition]
    colours = {key: value for key, _, value in STAGES}
    return style["face"] or colours[stage], style["hatch"], style["edge"]


def category_title(disposition: str, count: int) -> str:
    descriptions = {
        "port": "PORT · tier evidence",
        "replace": "REPLACE · host replacement",
        "drop": "DROP · will not be implemented",
        "spec": "SPEC · not implemented, specification-only",
        "library": "LIBRARY · not implemented, external",
        "undecided": "UNDECIDED · not classified",
    }
    return f"{descriptions[disposition]}  ({count:,})"


def disposition_stage(disposition: str, port_stage: str, equal: bool = False) -> str:
    """Evaluate progress against its goal; a port tier is not a replacement gate."""
    if disposition in {"drop", "spec"}:
        return "decision"
    if disposition in {"library", "undecided"}:
        return "unassigned"
    if disposition != "replace":
        return port_stage
    if port_stage == "retired":
        return "retired"
    if equal:
        return "replace_equal"
    if port_stage == "default_on":
        return "replace_default"
    if port_stage.startswith("tier"):
        return "replace_rust"
    return "replace_pending"


def legend_stages(disposition: str) -> tuple:
    if disposition == "port":
        return PORT_STAGES
    if disposition == "replace":
        return REPLACE_STAGES
    if disposition == "drop":
        return (("decision", "Decision recorded; no implementation planned", "#ffffff"),)
    if disposition == "spec":
        return (("decision", "Specification-only; decision complete", "#ffffff"),)
    if disposition == "library":
        return (("unassigned", "External/runtime library code", "#f4f4f2"),)
    return (("unassigned", "Awaiting classification", "#e7298a"),)


def parse_va(text: str) -> int:
    return int(text, 16)


def read_csv(root: Path, rel: str) -> list[dict[str, str]]:
    with (root / rel).open(newline="") as handle:
        return list(csv.DictReader(handle))


class MissingInput(Exception):
    """The gitignored per-function subsystem table is not available in this checkout."""


def subsystem_table(root: Path) -> Path:
    """tools/data/function_subsystems.csv is gitignored (bulk per-address dump, rule 8 hook), so a
    linked worktree reads the main checkout's copy; rebuild: `python -m tools.subsystems seed`."""
    rel = Path(SUBSYSTEMS)
    if (root / rel).exists():
        return root / rel
    common = git_text(root, "rev-parse", "--path-format=absolute", "--git-common-dir")
    if common:
        main = Path(common).parent
        if (main / rel).exists():
            return main / rel
    raise MissingInput(
        f"{SUBSYSTEMS} not found (gitignored; build it with python3 -m tools.subsystems seed)"
    )


def registry_tiers(root: Path) -> tuple[dict[int, int], set[int]]:
    """Best Rust tier per VA (port registry) and the VAs that only have a documented model."""
    ports = json.loads((root / "docs/data/port-registry.json").read_text())["ports"]
    best: dict[int, int] = {}
    documented: set[int] = set()
    for address, entries in ports.items():
        va = parse_va(address)
        for entry in entries:
            if entry.get("tier") is None:
                documented.add(va)
            else:
                best[va] = max(best.get(va, 0), int(entry["tier"]))
    return best, documented


def c2r_tiers(root: Path) -> dict[int, int]:
    best: dict[int, int] = {}
    for row in read_csv(root, "docs/data/c2rust-status.csv"):
        if row["status"] == "translated" and row["tier"].strip():
            va = parse_va(row["va"])
            best[va] = max(best.get(va, 0), int(row["tier"]))
    return best


def port_class_vas(root: Path, subsystems: list[dict[str, str]]) -> set[int]:
    """VAs whose subsystem has the `port` disposition (the ledger's Tier 2 scope)."""
    from tools.ledger import dispositions

    table = dispositions.load(root / dispositions.DEFAULT_PATH)
    return {
        parse_va(row["address"])
        for row in subsystems
        if dispositions.disposition_of(row["subsystem"], table) == "port"
    }


def shared_tier2(root: Path, port_vas: set[int]) -> tuple[dict[int, str], set[int]]:
    """(all-scope Tier 2 VA -> evidence, port-class Tier 2 VAs), via tools.verification.

    Same sources as `python3 -m tools.ledger tier2`: a VA is Tier 2 only when that shared
    function says so, never from the port registry's own tier field (T3702).
    """
    from tools import verification

    sources = {
        "verification_path": root / verification.DEFAULT_PATH,
        "c2rust_status": root / "docs/data/c2rust-status.csv",
        "tier2_path": root / verification.DEFAULT_TIER2_PATH,
        "tier2_capture_path": root / verification.DEFAULT_CAPTURE_PATH,
    }
    everywhere = verification.tier2_functions(**sources)
    ported = verification.tier2_functions(**sources, port_vas=port_vas)
    return everywhere, set(ported) & port_vas


def tier2_source_counts(root: Path, port_vas: set[int]) -> dict[str, int]:
    """Port-class Tier 2 VAs by credit source (whole-game / capture), ledger scope."""
    from tools import verification

    counts = {}
    for source in (verification.WHOLE_GAME, *verification.CAPTURE_FILE_SOURCES):
        found = verification.tier2_functions(
            verification_path=root / verification.DEFAULT_PATH,
            c2rust_status=root / "docs/data/c2rust-status.csv",
            tier2_path=root / verification.DEFAULT_TIER2_PATH,
            tier2_capture_path=root / verification.DEFAULT_CAPTURE_PATH,
            port_vas=port_vas,
            source=source,
        )
        counts[source] = len(set(found) & port_vas)
    return counts


def system_vas(root: Path) -> tuple[set[int], set[int]]:
    """(adopted VAs, retired VAs) from the rule 24 retirement config and records."""
    sys.path.insert(0, str(root))
    from tools import retire

    config = json.loads((root / "tools/data/retirement_systems.json").read_text())["systems"]
    retired_path = root / "docs/data/retirements.json"
    retired_names = {r["system"] for r in json.loads(retired_path.read_text()).get("retired", [])}
    adopted: set[int] = set()
    retired: set[int] = set()
    for name, entry in config.items():
        try:
            full, _partial = retire.enumerate_vas(root, entry)
        except FileNotFoundError as err:
            print(
                f"port_heatmap: WARNING skipping system {name}: missing source {err}",
                file=sys.stderr,
            )
            continue
        (retired if name in retired_names else adopted).update(full)
    return adopted, retired


def classify(
    va: int,
    named: dict[int, str],
    tiers: dict[int, int],
    documented: set[int],
    adopted: set[int],
    retired: set[int],
) -> str:
    """Stage key, strongest evidence first."""
    if va in retired:
        return "retired"
    if va in adopted:
        return "default_on"
    if va in tiers:
        return TIER_STAGE[min(tiers[va], 3)]
    if va in documented or named.get(va) == "MEASURED":
        return "analyzed"
    if va in named:
        return "named"
    return "unknown"


def compute(root: Path) -> list[dict]:
    """One row per function: va, subsystem, stage, size (bytes, address gap, capped)."""
    with subsystem_table(root).open(newline="") as handle:
        subsystems = list(csv.DictReader(handle))
    named = {
        parse_va(r["entry_va"]): r["confidence"]
        for r in read_csv(root, "tools/data/function_names.csv")
    }
    registry, documented = registry_tiers(root)
    tiers = dict(c2r_tiers(root))
    for va, tier in registry.items():
        tiers[va] = max(tiers.get(va, 0), tier)
    # Tier 2 and above come only from the ledger's shared evidence (T3702); registry and
    # c2r tiers are capped at Tier 1 here so the two metrics cannot drift.
    tiers = {va: min(tier, 1) for va, tier in tiers.items()}
    tier2_all, _ = shared_tier2(root, port_class_vas(root, subsystems))
    for va in tier2_all:
        tiers[va] = max(tiers.get(va, 0), 2)
    from tools.agents.default_on_guard import ADOPTIONS, SNAPSHOT, parse_snapshot
    from tools.ledger import dispositions

    table = dispositions.load(root / dispositions.DEFAULT_PATH)
    if not table:
        raise MissingInput("system dispositions missing or empty")
    adopted, retired = system_vas(root)
    _, defaults = parse_snapshot((root / SNAPSHOT).read_text())
    adopted.update(defaults)
    # These are tracked citations, not a re-validation of live receipt stores.
    equal = {
        parse_va(entry["va"])
        for entry in json.loads((root / ADOPTIONS).read_text())["adoptions"]
        if entry.get("verdict") == "EQUAL" and entry.get("receipt")
    }
    for entry in json.loads((root / "docs/data/retirements.json").read_text())["retired"]:
        retired.update(parse_va(va) for va in entry.get("retired_vas", []))
    ordered = sorted(subsystems, key=lambda row: parse_va(row["address"]))
    rows = []
    for index, row in enumerate(ordered):
        va = parse_va(row["address"])
        gap = (
            parse_va(ordered[index + 1]["address"]) - va if index + 1 < len(ordered) else SIZE_TAIL
        )
        disposition = dispositions.disposition_of(row["subsystem"], table)
        port_stage = classify(va, named, tiers, documented, adopted, retired)
        rows.append(
            {
                "va": va,
                "subsystem": row["subsystem"] or "unclassified",
                "disposition": disposition,
                "stage": disposition_stage(disposition, port_stage, va in equal),
                "rust": va in tiers or va in adopted or va in retired or va in equal,
                "default_on": va in adopted or va in retired,
                "gate_equal": va in equal,
                "size": max(SIZE_MIN, min(gap, SIZE_CAP)),
            }
        )
    return rows


def digest(rows: list[dict]) -> str:
    hasher = hashlib.sha256(
        json.dumps(
            [SCHEMA, STYLE_VERSION, LAYOUT_VERSION, STAGES, CATEGORY_STYLES], sort_keys=True
        ).encode()
    )
    hasher.update(json.dumps(rows, sort_keys=True).encode())
    return hasher.hexdigest()


def stage_counts(rows: list[dict]) -> Counter:
    return Counter(row["stage"] for row in rows)


def git_text(root: Path, *args: str) -> str:
    try:
        done = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, timeout=20)
        return done.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def squarify_cells(values: list[float], x: float, y: float, w: float, h: float) -> list[dict]:
    import squarify

    if not values:
        return []
    sizes = squarify.normalize_sizes(values, w, h)
    return squarify.squarify(sizes, x, y, w, h)


def layout(
    rows: list[dict], width: float, height: float
) -> tuple[list[tuple], list[tuple], list[tuple]]:
    """Cells (x, y, dx, dy, stage, disposition), leaf blocks, and top-level group rectangles."""
    leaves: dict[str, list[dict]] = defaultdict(list)
    for row in rows:
        leaves[row["subsystem"]].append(row)
    groups: dict[str, list[str]] = defaultdict(list)
    for leaf in leaves:
        groups[leaf.split("/")[0]].append(leaf)
    weight = {leaf: float(sum(r["size"] for r in members)) for leaf, members in leaves.items()}
    group_order = sorted(groups, key=lambda g: -sum(weight[leaf] for leaf in groups[g]))
    group_rects = squarify_cells(
        [sum(weight[leaf] for leaf in groups[g]) for g in group_order], 0, 0, width, height
    )
    cells: list[tuple] = []
    blocks: list[tuple] = []
    group_boxes: list[tuple] = []
    for group, grect in zip(group_order, group_rects, strict=True):
        group_boxes.append((grect["x"], grect["y"], grect["dx"], grect["dy"], group))
        names = sorted(groups[group], key=lambda leaf: -weight[leaf])
        pad = min(grect["dx"], grect["dy"]) * 0.012
        inner = (
            grect["x"] + pad,
            grect["y"] + pad,
            max(grect["dx"] - 2 * pad, 0.1),
            max(grect["dy"] - 2 * pad, 0.1),
        )
        for leaf, lrect in zip(
            names, squarify_cells([weight[n] for n in names], *inner), strict=True
        ):
            blocks.append((lrect["x"], lrect["y"], lrect["dx"], lrect["dy"], leaf))
            members = sorted(
                leaves[leaf], key=lambda r: (-STAGE_KEYS.index(r["stage"]), -r["size"], r["va"])
            )
            for member, crect in zip(
                members,
                squarify_cells(
                    [float(m["size"]) for m in members],
                    lrect["x"],
                    lrect["y"],
                    lrect["dx"],
                    lrect["dy"],
                ),
                strict=True,
            ):
                cells.append(
                    (
                        crect["x"],
                        crect["y"],
                        crect["dx"],
                        crect["dy"],
                        member["stage"],
                        member.get("disposition", "port"),
                    )
                )
    return cells, blocks, group_boxes


def render(rows: list[dict], root: Path, out_dir: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.patheffects as effects
    import matplotlib.pyplot as plt
    from matplotlib.collections import PatchCollection
    from matplotlib.patches import Patch, PathPatch, Rectangle
    from matplotlib.path import Path
    from matplotlib.transforms import Affine2D

    matplotlib.rcParams["svg.fonttype"] = "none"
    matplotlib.rcParams["svg.hashsalt"] = "tsfp"
    total = len(rows)
    width, height = 160.0, 90.0
    cells, blocks, group_boxes = layout(rows, width, height)

    fig = plt.figure(figsize=(17.0, 11.7), dpi=110)
    ax = fig.add_axes([0.005, 0.34, 0.99, 0.57])
    for disposition in ("port", "replace", "drop", "spec", "library", "undecided"):
        for stage in STAGE_KEYS:
            # Group by category and stage to preserve compact SVG paths and category texture.
            paths = [
                Path.unit_rectangle().transformed(Affine2D().scale(dx, dy).translate(x, y))
                for x, y, dx, dy, cell_stage, cell_disposition in cells
                if cell_stage == stage and cell_disposition == disposition
            ]
            if not paths:
                continue
            face, hatch, edge = category_style(disposition, stage)
            ax.add_patch(
                PathPatch(
                    Path.make_compound_path(*paths),
                    facecolor=face,
                    edgecolor=edge,
                    linewidth=0.16 if hatch else 0.12,
                    hatch=hatch,
                )
            )
    ax.add_collection(
        PatchCollection(
            [Rectangle((x, y), dx, dy) for x, y, dx, dy, _ in blocks],
            facecolor="none",
            edgecolor="#222222",
            linewidth=0.5,
        )
    )
    ax.add_collection(
        PatchCollection(
            [Rectangle((x, y), dx, dy) for x, y, dx, dy, _ in group_boxes],
            facecolor="none",
            edgecolor="black",
            linewidth=1.6,
        )
    )
    halo = [effects.withStroke(linewidth=2.2, foreground="white")]
    for x, y, dx, dy, leaf in blocks:
        if dx * dy < 55 or dx < 11 or dy < 3.2:
            continue
        label = leaf.split("/", 1)[1] if "/" in leaf else leaf
        size = max(5.5, min(10.0, dx * 0.55 / max(len(label) / 6.0, 1)))
        ax.text(
            x + dx / 2,
            y + dy / 2,
            label,
            ha="center",
            va="center",
            fontsize=size,
            color="black",
            path_effects=halo,
        )
    for x, y, dx, dy, group in group_boxes:
        if dx > 14 and dy > 5:
            ax.text(
                x + 0.6,
                y + 0.4,
                group.upper(),
                ha="left",
                va="top",
                fontsize=8.5,
                fontweight="bold",
                color="black",
                path_effects=halo,
            )
    ax.set_xlim(0, width)
    ax.set_ylim(height, 0)
    ax.axis("off")

    commit = git_text(root, "rev-parse", "--short", "HEAD")
    today = dt.date.today().isoformat()
    fig.text(
        0.01,
        0.975,
        "TimeSplitters: Future Perfect disposition progress, one cell per function",
        fontsize=16,
        fontweight="bold",
        va="top",
    )
    fig.text(
        0.01,
        0.945,
        f"{total} functions; colours follow each disposition's goal. "
        f"Cell area = function size, blocks = subsystems. Generated {today}, base commit {commit}.",
        fontsize=10.5,
        va="top",
        color="#333333",
    )
    dispositions = ("port", "replace", "drop", "spec", "library", "undecided")
    for index, disposition in enumerate(dispositions):
        counts = Counter(r["stage"] for r in rows if r.get("disposition", "port") == disposition)
        handles = [
            Patch(
                facecolor=category_style(disposition, k)[0],
                edgecolor=category_style(disposition, k)[2],
                hatch=category_style(disposition, k)[1],
                linewidth=0.8,
                label=f"{label}: {counts[k]}",
            )
            for k, label, _ in legend_stages(disposition)
        ]
        total_count = sum(counts.values())
        if index == 0:
            x, y, width, height, columns = 0.015, 0.125, 0.475, 0.205, 2
        elif index == 1:
            x, y, width, height, columns = 0.51, 0.125, 0.475, 0.205, 2
        else:
            x = 0.015 + (index - 2) * 0.247
            y, width, height, columns = 0.0, 0.095, 0.22, 1
        fig.legend(
            handles=handles,
            title=category_title(disposition, total_count),
            loc="upper left",
            bbox_to_anchor=(x, y, width, height),
            fontsize=8.5 if index < 2 else 8.0,
            title_fontsize=9.5 if index < 2 else 8.7,
            frameon=True,
            fancybox=False,
            framealpha=0.9,
            edgecolor="#b8c0c8",
            facecolor="#ffffff",
            handlelength=2.2,
            handleheight=1.1,
            labelspacing=0.24,
            borderpad=0.65,
            mode="expand" if index < 2 else None,
            ncol=columns,
        )
    tag = f"{DIGEST_TAG}{digest(rows)}"
    out_dir.mkdir(parents=True, exist_ok=True)
    fig.savefig(
        out_dir / SVG_NAME,
        format="svg",
        metadata={"Description": tag, "Date": None, "Creator": "tools.port_heatmap"},
    )
    svg = out_dir / SVG_NAME  # matplotlib leaves trailing blanks; `git diff --check` rejects them
    svg.write_text("".join(line.rstrip() + "\n" for line in svg.read_text().splitlines()))
    fig.savefig(
        out_dir / PNG_NAME,
        format="png",
        dpi=110,
        metadata={"Description": tag, "Software": "tools.port_heatmap"},
    )
    plt.close(fig)


def embedded_digest(path: Path) -> str | None:
    if not path.exists():
        return None
    found = DIGEST_RE.search(path.read_bytes())
    return found.group(1).decode() if found else None


def stale_files(rows: list[dict], out_dir: Path) -> list[str]:
    want = digest(rows)
    return [name for name in (SVG_NAME, PNG_NAME) if embedded_digest(out_dir / name) != want]


def venv_python(root: Path) -> str:
    candidate = root / ".venv/bin/python"
    return str(candidate) if candidate.exists() else sys.executable


HEATMAP_ENV = "tmp/venvs/heatmap"


def heatmap_home() -> tuple[Path, Path]:
    """(project dir whose uv.lock defines the group, environment dir): this module's own checkout,
    so a publisher rendering an older candidate tree still syncs the group its code expects; the
    environment lives in the main checkout so every worktree shares one."""
    project = Path(__file__).resolve().parents[1]
    try:
        common = subprocess.run(
            ["git", "-C", str(project), "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        ).stdout.strip()
        main = Path(common).parent
    except (OSError, subprocess.SubprocessError):
        main = project
    return project, main / HEATMAP_ENV


def render_python(root: Path) -> str:
    """An interpreter with the `heatmap` dependency group (matplotlib, squarify): the caller's, the
    project venv, else a dedicated environment synced from uv.lock. The main venv's `uv sync` prunes
    the group, which broke every progress publish on 2026-10-28 (I-T3881-2)."""
    for python in dict.fromkeys([sys.executable, venv_python(root)]):
        if _has_heatmap_deps(python):
            return python
    project, env = heatmap_home()
    try:
        subprocess.run(
            ["uv", "sync", "--frozen", "--only-group", "heatmap", "--no-install-project"],
            cwd=project,
            capture_output=True,
            timeout=900,
            check=True,
            env=dict(os.environ, UV_PROJECT_ENVIRONMENT=str(env)),
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise SystemExit(
            f"port_heatmap: cannot sync the heatmap environment {env}: {error}"
        ) from error
    return str(env / "bin/python")


def _has_heatmap_deps(python: str) -> bool:
    probe = [python, "-c", "import matplotlib, squarify"]
    try:
        return subprocess.run(probe, capture_output=True, timeout=60).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def land(root: Path, out_rel: str, dry_run: bool) -> int:
    """Regenerate through a scratch worktree and land it with merge_task (hourly hygiene path).

    The main checkout must stay clean, so the images are never written there. Only this path
    commits them: feature branches never touch the binary images, so they cannot conflict."""
    try:
        rows = compute(root)
    except MissingInput as error:
        print(f"port_heatmap: skipped, {error}")
        return 0
    from tools import port_heatmap_platform as platform

    layers = platform.compute(root, rows)
    stale = stale_files(rows, root / out_rel) + platform.stale_files(layers, root / out_rel)
    if (
        platform.update_doc((root / platform.DOC).read_text(), layers)
        != (root / platform.DOC).read_text()
    ):
        stale.append(platform.DOC)
    if not stale:
        print("port_heatmap: images are current, nothing to land")
        return 0
    print(f"port_heatmap: stale {', '.join(stale)}")
    if dry_run:
        return 0
    stamp = dt.datetime.now().strftime("%Y%m%d%H%M%S")
    branch = f"task/heatmap-auto-{stamp}"
    tree = root / "tmp/wt" / f"heatmap-auto-{stamp}"
    python = venv_python(root)

    def run(cmd: list[str], cwd: Path, timeout: int, env_extra: dict | None = None) -> int:
        import os

        env = {**os.environ, **(env_extra or {})}
        return subprocess.run(cmd, cwd=cwd, env=env, timeout=timeout).returncode

    if run(["git", "fetch", "-q", "origin", "main"], root, 120) != 0:
        return 5
    if (
        run(["git", "worktree", "add", "-q", str(tree), "-b", branch, "origin/main"], root, 120)
        != 0
    ):
        return 5
    renderer = render_python(root)
    code = run([renderer, "-m", "tools.port_heatmap", "--write", "--out-dir", out_rel], tree, 300)
    if code == 0:
        run(["git", "add", out_rel, platform.DOC], tree, 60)
        code = run(
            [
                "git",
                "commit",
                "-q",
                "-m",
                "chore(T3632): regenerate port-progress heatmap (tools.port_heatmap --land)",
            ],
            tree,
            60,
        )
    if code == 0:
        verify = f"{python} -m pytest -q tests/test_port_heatmap.py -p no:cacheprovider"
        code = run(
            [
                sys.executable,
                "-m",
                "tools.agents.merge_task",
                branch,
                "--lock-timeout",
                "900",
                "--verify",
                verify,
            ],
            root,
            1700,
            {"TSFP_BUILD_SLOT_HELD": "1"},
        )
    if code == 0:
        run([sys.executable, "-m", "tools.wt", "remove", str(tree)], root, 300)
        run(["git", "branch", "-d", branch], root, 60)
    else:
        print(
            f"port_heatmap: landing failed (exit {code}); scratch tree kept at {tree}",
            file=sys.stderr,
        )
    return code


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--root", default=".", help="repository root (relative)")
    parser.add_argument("--dry-run", action="store_true", help="with --land: only report staleness")
    parser.add_argument(
        "--filter", default="", help="subsystem prefix; writes into a detail subdirectory"
    )
    parser.add_argument(
        "--out-dir", default=str(OUT_DIR), help="image directory, relative to the root"
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true", help="render the images")
    mode.add_argument("--check", action="store_true", help="exit 1 when an image is stale")
    mode.add_argument(
        "--land",
        action="store_true",
        help="regenerate in a scratch worktree and land it through merge_task when stale",
    )
    mode.add_argument("--stats", action="store_true", help="print stage counts")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()
    out_dir = root / args.out_dir
    if args.filter:
        if args.land:
            parser.error("--filter cannot be used with --land")
        detail = re.sub(r"[^a-zA-Z0-9_-]+", "-", args.filter).strip("-") or "detail"
        out_dir = out_dir / f"filter-{detail}"
    try:
        rows = compute(root)
    except (MissingInput, OSError, ValueError) as error:
        print(f"port_heatmap: {error}", file=sys.stderr)
        return 2
    if args.filter:
        rows = [row for row in rows if row["subsystem"].startswith(args.filter)]
    if not rows:
        print(f"port_heatmap: no functions match {args.filter!r}", file=sys.stderr)
        return 2
    if args.land:
        return land(root, args.out_dir, args.dry_run)
    if args.stats:
        for disposition in ("port", "replace", "drop", "spec", "library", "undecided"):
            counts = Counter(r["stage"] for r in rows if r["disposition"] == disposition)
            for key, label, _ in legend_stages(disposition):
                print(f"{disposition:10} {key:16} {counts[key]:6}  {label}")
        print(f"total       {len(rows):6}")
        port_vas = {r["va"] for r in rows if r["disposition"] == "port"}
        _, port_tier2 = shared_tier2(root, port_vas)
        print(f"tier2 port-class (ledger scope): {len(port_tier2)}/{len(port_vas)}")
        split = tier2_source_counts(root, port_vas)
        print(
            f"  by source: whole-game {split['whole-game']}, capture {split['capture']}, "
            f"constructed-call {split['constructed-call']}, "
            f"harness-service {split['harness-service']}, symbolic {split['symbolic']}"
        )
        return 0
    from tools import port_heatmap_platform as platform

    try:
        layers = platform.compute(root, rows, args.filter)
    except (OSError, ValueError) as error:
        print(f"port_heatmap: {error}", file=sys.stderr)
        return 2
    doc = root / platform.DOC
    if args.check:
        stale = stale_files(rows, out_dir) + platform.stale_files(layers, out_dir)
        if not args.filter and platform.update_doc(doc.read_text(), layers) != doc.read_text():
            stale.append(platform.DOC)
        for name in stale:
            print(f"port_heatmap: {args.out_dir}/{name} is stale; run tools.port_heatmap --write")
        return 1 if stale else 0
    render(rows, root, out_dir)
    platform.render(layers, out_dir)
    if not args.filter:
        (out_dir / ROWS_NAME).write_text(json.dumps(rows, separators=(",", ":")))
        doc.write_text(platform.update_doc(doc.read_text(), layers))
    print(f"port_heatmap: wrote {out_dir} (four images; {len(rows)} functions)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
