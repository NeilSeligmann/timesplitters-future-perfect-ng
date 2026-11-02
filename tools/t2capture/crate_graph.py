# SPDX-License-Identifier: GPL-3.0-or-later
"""Crate path-dependency graph for the t2-continuous dependent selection (T3784).

A change anywhere in a crate can change the verdict of every port function compiled into that
crate or into any crate that depends on it (a shared bridge such as
tsfp-port-lanes-typed/src/cpu_bridge.rs). The graph is derived from `cargo metadata --offline
--no-deps` over every tracked manifest (rule 45: no hand list): one call per workspace, the root
workspace first, then each standalone crate not already covered. Edges are normal and build
path dependencies; dev-dependencies never reach the shipped code.

Keys are repository-relative crate directories with a trailing slash ("crates/tsfp-ports/").
"""

from __future__ import annotations

import json
import subprocess
from collections.abc import Callable, Iterable
from pathlib import Path

Graph = dict[str, set[str]]  # crate dir -> crate dirs it depends on (path deps)
Metadata = Callable[[Path, Path], dict]  # (root, manifest) -> cargo metadata JSON


class GraphError(RuntimeError):
    """cargo metadata failed for a workspace: the graph is unknown."""


def _relative(root: Path, path: Path) -> str | None:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix().rstrip("/") + "/"
    except ValueError:
        return None


def parse_metadata(root: Path, metadata: dict) -> Graph:
    """Local packages of one `cargo metadata --no-deps` document and their path dependencies."""
    graph: Graph = {}
    for package in metadata.get("packages") or []:
        if package.get("source"):
            continue  # registry crates are pinned by Cargo.lock
        own = _relative(root, Path(package["manifest_path"]).parent)
        if own is None:
            continue
        deps = graph.setdefault(own, set())
        for dependency in package.get("dependencies") or []:
            if not dependency.get("path") or dependency.get("kind") == "dev":
                continue
            target = _relative(root, Path(dependency["path"]))
            if target is not None and target != own:
                deps.add(target)
    return graph


def real_metadata(root: Path, manifest: Path) -> dict:
    done = subprocess.run(
        ["cargo", "metadata", "--offline", "--no-deps", "--format-version", "1",
         "--manifest-path", str(manifest)],
        cwd=root, capture_output=True, text=True, timeout=120, check=False,
    )  # fmt: skip
    if done.returncode:
        raise GraphError(f"cargo metadata {manifest}: {done.stderr.strip()[-300:]}")
    return json.loads(done.stdout)


def toml_metadata(root: Path, manifest: Path) -> dict:
    """Fallback for a manifest cargo refuses outside its workspace (a worktree nested under
    another checkout makes cargo walk up to the outer workspace): the same package shape, read
    from the manifest's own [dependencies], [build-dependencies] and target tables."""
    import tomllib

    data = tomllib.loads(manifest.read_text())
    tables = [data.get("dependencies") or {}, data.get("build-dependencies") or {}]
    for target in (data.get("target") or {}).values():
        tables += [target.get("dependencies") or {}, target.get("build-dependencies") or {}]
    deps = [
        {"name": name, "path": str((manifest.parent / spec["path"]).resolve()), "kind": None}
        for table in tables
        for name, spec in table.items()
        if isinstance(spec, dict) and spec.get("path")
    ]
    name = (data.get("package") or {}).get("name", manifest.parent.name)
    return {"packages": [{"name": name, "manifest_path": str(manifest), "dependencies": deps}]}


def manifests(root: Path) -> list[Path]:
    """The root workspace manifest first (covers its members in one call), then the rest."""
    from tools.agents import verify_profiles

    rest = verify_profiles.crate_manifests(root)
    head = [root / "Cargo.toml"] if (root / "Cargo.toml").is_file() else []
    return head + rest


def load(
    root: Path,
    metadata: Metadata = real_metadata,
    fallbacks: list[str] | None = None,
) -> Graph:
    """Graph over every tracked manifest. A manifest cargo refuses is read with toml_metadata
    and named in `fallbacks`; GraphError when neither can read it."""
    graph: Graph = {}
    for manifest in manifests(root):
        own = _relative(root, manifest.parent)
        if own in graph:
            continue  # already covered by an earlier workspace's package list
        try:
            document = metadata(root, manifest)
        except (GraphError, OSError, subprocess.SubprocessError, ValueError) as error:
            try:
                document = toml_metadata(root, manifest)
            except (OSError, ValueError) as toml_error:
                raise GraphError(f"{manifest}: {error}; toml: {toml_error}") from toml_error
            if fallbacks is not None:
                fallbacks.append(str(manifest.relative_to(root)))
        for crate, deps in parse_metadata(root, document).items():
            graph.setdefault(crate, set()).update(deps)
    return graph


def owning_crate(path: str, graph: Graph) -> str | None:
    """Deepest crate directory containing `path` (nested crates win)."""
    best = None
    for crate in graph:
        if path.startswith(crate) and (best is None or len(crate) > len(best)):
            best = crate
    return best


def dependents(graph: Graph, crates: Iterable[str]) -> set[str]:
    """`crates` plus every crate that depends on one of them, transitively."""
    reverse: Graph = {}
    for crate, deps in graph.items():
        for dep in deps:
            reverse.setdefault(dep, set()).add(crate)
    seen: set[str] = set()
    stack = list(crates)
    while stack:
        crate = stack.pop()
        if crate in seen:
            continue
        seen.add(crate)
        stack.extend(reverse.get(crate, ()))
    return seen


def affected_crates(graph: Graph, changed_files: Iterable[str]) -> set[str]:
    """Crates whose compiled code may differ after `changed_files` (owners plus dependents)."""
    owners = {crate for path in changed_files if (crate := owning_crate(path, graph))}
    return dependents(graph, owners)
