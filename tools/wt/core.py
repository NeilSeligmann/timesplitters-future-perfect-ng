# ruff: noqa: E501
"""Bootstrap and doctor logic for fresh git worktrees (T1883). Never copies owner files into tracked paths."""

from __future__ import annotations

import hashlib
import os
import re
import shlex
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from tools import owner_files
from tools.wt import cache_keys

FILE, DIR, ENTRIES, GLOB_DIRS = "file", "dir", "entries", "glob-dirs"


@dataclass(frozen=True)
class Link:
    path: str
    kind: str
    required: bool
    why: str


# Everything here is gitignored; the worktree gets symlinks to the MAIN checkout's copy.
LINKS = (
    Link(
        "src/xbox/xdk_abi.inc", FILE, True, "gitignored XDK surface (host build exits 2 without it)"
    ),
    Link("src/xbox/xdk_surface.c", FILE, True, "gitignored XDK surface"),
    Link("src/xbox/xdk_surface.h", FILE, True, "gitignored XDK surface"),
    Link(
        "build/default.xbe",
        FILE,
        True,
        "retail XBE the host and lifter bind to (host exits 1 without it)",
    ),
    Link(
        "tools/data/function_subsystems.csv", FILE, False, "generated ledger data some tests read"
    ),
    Link(
        "generated",
        ENTRIES,
        False,
        "generated retail inputs (children linked, new files stay local)",
    ),
    Link("tmp/spikes-cargo-home", DIR, False, "offline wgpu/Bevy cargo cache"),
    Link("tmp/rust", DIR, True, "offline rustup/cargo home (CARGO_HOME, RUSTUP_HOME)"),
    Link("tmp/Xbox-Emulator-Files", DIR, False, "owner bios/mcpx/hdd for xemu (never copied)"),
    Link(
        "tmp/private-host/lift-*",
        GLOB_DIRS,
        False,
        "content-keyed private lifts (key = source hash, path independent)",
    ),
    Link("tmp/oxm-extract", DIR, False, "owner extract some oracle tests read"),
    Link("tmp/play", DIR, False, "gpu replay modules for the live host"),
    Link("tmp/recorded-input*", GLOB_DIRS, False, "recorded owner input sessions for replays"),
    Link(".venv", DIR, False, "project Python environment"),
)
CACHE_REL = "tmp/wt-cache"
ENV_REL = "tmp/wt-env.sh"


def git_out(root: Path, *args: str) -> str:
    done = subprocess.run(
        ["git", *args], cwd=root, capture_output=True, text=True, timeout=60, check=False
    )
    return done.stdout.strip()


def worktree_root(path: Path | None) -> Path:
    from tools.wt import source_identity

    start = (path or Path.cwd()).resolve()
    exec_tree = source_identity.exec_root(start)  # T3760: a git-less remote exec tree
    if exec_tree is not None:
        return exec_tree  # asked first: no git call on a host (T3775 guard)
    top = git_out(start, "rev-parse", "--show-toplevel")
    if not top:
        raise SystemExit(
            f"{start} is not inside a git worktree nor an exec tree with a source identity"
        )
    return Path(top)


def main_root(root: Path) -> Path:
    return owner_files.main_checkout(root)


def cargo_target_root(main: Path) -> Path:
    """Use the local host's registered target volume when `main` is that host's data root."""
    default = main / CACHE_REL / "cargo-target"
    from tools import host_registry

    try:
        registry = host_registry.load(host_registry.path_in(main))
        resolved_main = main.expanduser().resolve()
        for host in registry.hosts():
            data_root = host.get("data_root")
            target_dir = host.get("target_dir")
            if not isinstance(data_root, str) or not isinstance(target_dir, str) or not target_dir:
                continue
            if Path(data_root).expanduser().resolve() != resolved_main:
                continue
            target = Path(target_dir).expanduser()
            return target if target.is_absolute() else main / target
    except (OSError, host_registry.RegistryError):
        pass
    return default


def cargo_family_key(root: Path, family: str, command: list[str] | None = None) -> str:
    """Stable cache key for a toolchain/build-input family, shared across worker tasks.

    Worker labels are aliases for the normal family. A caller that genuinely needs a
    distinct crate graph can use a semantic family such as ``default-rt-host``.
    Cargo itself separates feature/profile/target fingerprints inside the target directory;
    we additionally include those requested inputs here to avoid needless cross-family churn.
    """
    if family.startswith("w-"):
        semantic = "default-rt-host" if family.endswith("-rt-host") else "default"
    else:
        semantic = family
    toolchain = hashlib.sha256()
    found = False
    for name in ("rust-toolchain.toml", "rust-toolchain"):
        path = root / name
        if path.is_file():
            toolchain.update(name.encode() + b"\0" + path.read_bytes() + b"\0")
            found = True
    words = list(command or [])
    # Shell hook routes `sh -c CMD` through tools.wt run; inspect CMD just as for cargo argv.
    for index, word in enumerate(words[:-1]):
        if Path(word).name in {"sh", "bash", "zsh"} and words[index + 1] == "-c":
            try:
                words = [*words[:index], *shlex.split(words[index + 2])]
            except (ValueError, IndexError):
                pass
            break
    override = next((word[1:] for word in words if word.startswith("+") and len(word) > 1), "")
    toolchain.update(
        b"override\0" + (override or os.environ.get("RUSTUP_TOOLCHAIN", "default")).encode() + b"\0"
    )
    if not found:
        toolchain.update(os.environ.get("RUSTUP_TOOLCHAIN", "default").encode())
    features = set(filter(None, os.environ.get("TSFP_CARGO_FEATURES", "").split(",")))
    no_default = False
    all_features = False
    release = False
    target = ""
    for index, word in enumerate(words):
        if word in ("--features", "-F") and index + 1 < len(words):
            features.update(filter(None, re.split(r"[,\s]", words[index + 1])))
        elif word.startswith("--features=") or word.startswith("-F") and word != "-F":
            value = word.split("=", 1)[1] if word.startswith("--features=") else word[2:]
            features.update(filter(None, re.split(r"[,\s]", value)))
        elif word == "--no-default-features":
            no_default = True
        elif word == "--all-features":
            all_features = True
        elif word == "--release":
            release = True
        elif word == "--target" and index + 1 < len(words):
            target = words[index + 1]
        elif word.startswith("--target="):
            target = word.partition("=")[2]
    flags = "\0".join(
        (
            os.environ.get("RUSTFLAGS", ""),
            os.environ.get("CARGO_ENCODED_RUSTFLAGS", ""),
            os.environ.get("CARGO_BUILD_TARGET", ""),
            os.environ.get("CARGO_PROFILE", ""),
        )
    )
    # Incremental targets remain separate because Cargo's incremental artifacts cannot be
    # reused by non-incremental builds; the worker count and target directory stay excluded.
    shared = cache_keys.make_key(
        "cargo-target-family",
        files={"toolchain-pin-and-override": toolchain.hexdigest()},
        # Probe rustc through the project toolchain, never the caller's ~/.rustup (T3836).
        tools=cache_keys.tool_versions(
            root, env=toolchain_env(main_root(root)), names=("rustc",), include_pin=False
        ),
        env={},
        extra={
            "semantic-family": semantic,
            "features": sorted(features),
            "no-default-features": no_default,
            "all-features": all_features,
            "release": release,
            "target": target,
            "flags": flags,
            "incremental-target": os.environ.get("TSFP_CARGO_INCREMENTAL") == "1",
        },
    )
    digest = shared.key[:20]
    label = re.sub(r"[^a-zA-Z0-9_.-]+", "-", semantic).strip("-.")[:20] or "default"
    return f"v2-{label}-{digest}"


def cargo_profile_dir(target_dir: Path, command: list[str] | None = None) -> Path:
    """Map a Cargo invocation to the exact output profile directory held against pruning."""
    words = list(command or [])
    for index, word in enumerate(words[:-1]):
        if Path(word).name in {"sh", "bash", "zsh"} and words[index + 1] == "-c":
            try:
                words = [*words[:index], *shlex.split(words[index + 2])]
            except (ValueError, IndexError):
                pass
            break
    profile = "release" if "--release" in words else "debug"
    for index, word in enumerate(words[:-1]):
        if word == "--profile":
            profile = words[index + 1]
        elif word.startswith("--profile="):
            profile = word.partition("=")[2]
    target = ""
    for index, word in enumerate(words[:-1]):
        if word == "--target":
            target = words[index + 1]
        elif word.startswith("--target="):
            target = word.partition("=")[2]
    return target_dir / target / profile if target else target_dir / profile


def link_one(source: Path, target: Path, dry_run: bool) -> str:
    """Create target -> source. Never replaces a real file or dir. Returns a status word."""
    if target.is_symlink():
        if target.resolve() == source.resolve():
            return "ok"
        if not dry_run:
            target.unlink()
    elif target.exists():
        return "kept-local"
    if dry_run:
        return "would-link"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.symlink_to(source)
    return "linked"


def plan(root: Path, main: Path, files_dir: Path | None) -> list[tuple[Link, Path, Path | None]]:
    """(spec, target, source) triples; source None means missing in the main checkout."""
    items: list[tuple[Link, Path, Path | None]] = []
    for spec in LINKS:
        if spec.kind == GLOB_DIRS:
            parent, pattern = spec.path.rsplit("/", 1)
            found = sorted(
                p
                for p in (main / parent).glob(pattern)
                if p.exists() and not p.name.endswith((".pid", ".log"))
            )
            items.extend((spec, root / parent / p.name, p) for p in found)
            continue
        source = main / spec.path
        if spec.path == "tmp/Xbox-Emulator-Files" and files_dir:
            source = files_dir.resolve()
        items.append((spec, root / spec.path, source if source.exists() else None))
    # ISO images are owner-provided run inputs, kept in tmp/owner. Link them into
    # each worktree's tmp/ so existing relative-path recipes continue to work.
    owner_dir = main / "tmp" / "owner"
    iso_spec = Link("tmp/*.iso", FILE, False, "owner ISO linked from tmp/owner (never copied)")
    if owner_dir.is_dir():
        items.extend(
            (iso_spec, root / "tmp" / source.name, source)
            for source in sorted(owner_dir.glob("*.iso"))
            if source.is_file()
        )
    return items


# T3461: git >= 2.47 runs auto-maintenance detached after commit, merge and fetch. Under a
# PID 1 that does not reap (this container) every detached child stays a zombie: MEASURED
# 10 commits = +10 zombie git processes, 58k of them before the fix.
GIT_HYGIENE = {
    "maintenance.autoDetach": "false",
    "gc.autoDetach": "false",
    # Scheduled maintenance in tools.wt.hygiene replaces this repo-wide stopgap.
    "gc.auto": "6700",
}


def git_local_config(root: Path, key: str) -> str | None:
    """The repository-level (common config) value of `key`, or None when unset."""
    done = subprocess.run(
        ["git", "config", "--local", "--get", key],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return done.stdout.strip() if done.returncode == 0 else None


def set_git_hygiene(root: Path) -> list[str]:
    """Write GIT_HYGIENE into the shared repository config (idempotent); return what changed."""
    changed = []
    for key, value in GIT_HYGIENE.items():
        if git_local_config(root, key) == value:
            continue
        for attempt in range(20):
            done = subprocess.run(
                ["git", "config", "--local", key, value], cwd=root, capture_output=True, timeout=30
            )
            if done.returncode == 0:
                break
            # Concurrent bootstraps collide on the common config lock (exit 255): retry.
            if done.returncode != 255 or attempt == 19:
                raise subprocess.CalledProcessError(done.returncode, done.args, done.stderr)
            time.sleep(min(0.05 * (attempt + 1), 0.25))
        changed.append(f"{key}={value}")
    return changed


# T3662 (I-T3662-3): the shared reference-transaction hook runs tools/ci/primary-ref-guard.sh,
# which refuses moving primary main to a commit origin lacks. It is a no-op in checkouts whose
# tree has no guard script, and merge_task fast-forwards primary only after its push.
PRIMARY_GUARD_MARK = "primary-ref-guard"
PRIMARY_GUARD_HOOK = (
    "#!/bin/sh\n"
    "# T3662: primary main may only move to commits origin holds (tools/ci/primary-ref-guard.sh)\n"
    '[ -x ./tools/ci/primary-ref-guard.sh ] && exec ./tools/ci/primary-ref-guard.sh "$@"\n'
    "cat >/dev/null\n"
)


def hooks_dir(root: Path) -> Path:
    """The hooks directory git uses for `root` (core.hooksPath, else <common dir>/hooks)."""
    configured = git_local_config(root, "core.hooksPath")
    if configured:
        path = Path(configured).expanduser()
        return path if path.is_absolute() else root / path
    common = subprocess.run(
        ["git", "rev-parse", "--git-common-dir"],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    ).stdout.strip()
    path = Path(common)
    return (path if path.is_absolute() else root / path) / "hooks"


def install_primary_guard(root: Path) -> str:
    """Install the shared reference-transaction hook (idempotent): installed, updated, present
    or kept. A guard hook of another revision (claude-1's interim one fell back to an untracked
    copy hooks/primary-ref-guard.sh) is replaced and the copy removed, so only the tracked
    script runs; a foreign hook is never overwritten (HEALTH primary-guard-missing reports it)."""
    directory = hooks_dir(root)
    hook = directory / "reference-transaction"
    try:
        current = hook.read_text()
    except FileNotFoundError:
        current = None
    if current is not None and PRIMARY_GUARD_MARK not in current:
        return "kept foreign hook"
    copy = directory / "primary-ref-guard.sh"
    if current == PRIMARY_GUARD_HOOK and not copy.exists():
        return "present"
    directory.mkdir(parents=True, exist_ok=True)
    scratch = hook.with_name(f".{hook.name}.{os.getpid()}.tmp")
    scratch.write_text(PRIMARY_GUARD_HOOK)
    scratch.chmod(0o755)
    os.replace(scratch, hook)  # atomic: concurrent bootstraps write the same text
    copy.unlink(missing_ok=True)
    return "installed" if current is None else "updated"


def bootstrap(root: Path, files_dir: Path | None, family: str, dry_run: bool) -> list[str]:
    main = main_root(root)
    lines: list[str] = []
    if not dry_run:
        from tools.wt import pyheal

        main_result = pyheal.ensure(main, expected_version=pyheal.pinned_version(root))
        lines.append(f"python   main .venv {main_result}")
        if root.resolve() != main.resolve():
            worktree_result = pyheal.ensure_worktree(root, main)
            lines.append(f"python   worktree .venv {worktree_result}")
    if root.resolve() == main.resolve():
        lines.append("note: this is the main checkout, nothing to link")
    else:
        for spec, target, source in plan(root, main, files_dir):
            rel = target.relative_to(root)
            if source is None:
                lines.append(f"missing  {rel}  (not in main checkout; {spec.why})")
            elif spec.kind == ENTRIES:
                lines.append(f"{entries_link(source, target, dry_run):<10} {rel}/ children")
            else:
                lines.append(f"{link_one(source, target, dry_run):<10} {rel}")
    env_text = env_script(root, main, family)
    from tools.wt import source_identity

    identity = source_identity.load(root)  # T3760: a remote exec tree has no repository
    if not dry_run:
        if identity is not None:
            lines.append(f"git      none (exec tree of {identity.commit or identity.tree})")
        else:
            from tools.rea_index_merge import register

            register(root)  # shared git config: REA index merge driver (T3386)
            from tools.t2capture import evidence_merge

            evidence_merge.register(root)  # Tier 2 evidence per-VA merge driver (T3784)
            for setting in set_git_hygiene(root):  # shared git config: no zombies (T3461)
                lines.append(f"git      config {setting}")
            lines.append(f"git      reference-transaction guard {install_primary_guard(root)}")
        env_path = root / ENV_REL
        env_path.parent.mkdir(parents=True, exist_ok=True)
        env_path.write_text(env_text)
        (main / CACHE_REL / "cargo-target" / cargo_family_key(root, family)).mkdir(
            parents=True, exist_ok=True
        )
    lines.append(f"env      {ENV_REL}  (source it, or use `python3 -m tools.wt run -- CMD`)")
    if identity is not None:
        lines.append("prebuilt exec tree: no primary branch, builds name their commit")
        return lines
    from tools.wt.prebuilt import expected_path

    lines.append(
        f"prebuilt {expected_path(root)} (shared host for this worktree's primary merge-base)"
    )
    return lines


def entries_link(source: Path, target: Path, dry_run: bool) -> str:
    if target.is_symlink() and not dry_run:
        target.unlink()  # a whole-dir link would write generated files into main
    if not dry_run:
        target.mkdir(parents=True, exist_ok=True)
    made = 0
    for child in sorted(source.iterdir()):
        if link_one(child, target / child.name, dry_run) in ("linked", "would-link"):
            made += 1
    return f"+{made}"


def shared_rustup_home(main: Path) -> Path:
    """The project RUSTUP_HOME every tools.wt build exports (hygiene sweeps its tmp, T3834)."""
    return main / "tmp" / "rust" / "rustup"


def shared_cargo_home(main: Path) -> Path:
    """The project CARGO_HOME every tools.wt build exports."""
    return main / "tmp" / "rust" / "cargo"


def toolchain_env(main: Path) -> dict[str, str]:
    """os.environ with the project Rust toolchain (homes, PATH, no auto-install), T3836.

    Version probes run under this so a rustup proxy never resolves ~/.rustup.
    """
    cargo_home = shared_cargo_home(main)
    return {
        **os.environ,
        "CARGO_HOME": str(cargo_home),
        "RUSTUP_HOME": str(shared_rustup_home(main)),
        "RUSTUP_AUTO_INSTALL": "0",
        "PATH": f"{cargo_home / 'bin'}{os.pathsep}{os.environ.get('PATH', '')}",
    }


def env_vars(
    root: Path, main: Path, family: str, command: list[str] | None = None
) -> dict[str, str]:
    from tools.wt.rustup_health import check

    check(main)
    cargo_home = shared_cargo_home(main)
    family_key = cargo_family_key(root, family, command)
    # T3432 can safely share a target only when tools.wt also synchronizes the sources
    # into its fixed canonical checkout. The full-host entrypoint now has the same
    # contract. Other callers (cargo run, arbitrary shell scripts, compound commands,
    # or an explicit canonical-build opt-out) must still get a target
    # isolated to this worktree: Cargo freshness relies on mtimes and a shared target can
    # otherwise accept another checkout's newer artifact as fresh. Immutable source
    # paths do not prevent this: Cargo can reuse a binary from another checkout.
    from tools.wt import canonical_build, host_family

    canonical_command = canonical_build.cargo_argv(command or [])
    host_command = host_family.argv(command or [], root)
    if host_command is not None and canonical_build.enabled(os.environ):
        family_key = host_family.PREFIX + family_key
    elif canonical_command is None or not canonical_build.enabled(os.environ):
        worktree = hashlib.sha256(str(root.resolve()).encode()).hexdigest()[:16]
        family_key = f"{family_key}-wt-{worktree}"
    env = {
        "CARGO_NET_OFFLINE": "true",
        # T3445: TSFP_CARGO_INCREMENTAL=1 opts an edit loop into incremental local crates
        # (MEASURED tsfp-ports re-edit 115 s -> 28 s, cold +15 %, target 2.6 -> 9.5 GB).
        "CARGO_INCREMENTAL": "1" if os.environ.get("TSFP_CARGO_INCREMENTAL") == "1" else "0",
        "CARGO_TARGET_DIR": str(cargo_target_root(main) / family_key),
        "CARGO_HOME": str(cargo_home),
        "RUSTUP_HOME": str(shared_rustup_home(main)),
        # Never let a rustup proxy repair a missing shared install implicitly. That
        # default-profile install used ~/.rustup and raced the pinned fleet install.
        "RUSTUP_AUTO_INSTALL": "0",
        "PATH": f"{cargo_home / 'bin'}{os.pathsep}{os.environ.get('PATH', '')}",
        "TSFP_FILES_DIR": str(owner_files.xbox_files_dir(root)),
    }
    from tools.wt import compiler_cache

    cache_env = {**os.environ, **env}
    configured = compiler_cache.configure(root, main, cache_env)
    for key in (
        "TSFP_SCCACHE",
        "RUSTC_WRAPPER",
        "CMAKE_C_COMPILER_LAUNCHER",
        "SCCACHE_DIR",
        "SCCACHE_CACHE_SIZE",
    ):
        if key in cache_env and configured:
            env[key] = cache_env[key]
    if not configured and os.environ.get("TSFP_NO_CCACHE") != "1" and shutil.which("ccache"):
        env["CMAKE_C_COMPILER_LAUNCHER"] = "ccache"
        env["CCACHE_DIR"] = str(main / CACHE_REL / "ccache")
    # T3745: tools' temporary files on the data disk, never /tmp on the host root disk
    tmpdir = main / "tmp" / "tmpdir"
    tmpdir.mkdir(parents=True, exist_ok=True)
    env["TMPDIR"] = str(tmpdir)
    env["GIT_CEILING_DIRECTORIES"] = str(tmpdir)  # temp dirs are inside the checkout: no discovery
    return env


def apply_rust_env(root: Path, main: Path | None = None) -> None:
    """Apply the repository's pinned Rust homes to Python tool subprocesses.

    Command wrappers use this before invoking rustfmt/cargo directly; `tools.wt run`
    uses the richer `env_vars` path above for build-specific cache routing.
    """
    main = main or main_root(root)
    from tools.wt.rustup_health import check

    check(main)
    rust = main / "tmp" / "rust"
    cargo_home = rust / "cargo"
    os.environ.update(
        {
            "CARGO_HOME": str(cargo_home),
            "RUSTUP_HOME": str(shared_rustup_home(main)),
            "RUSTUP_AUTO_INSTALL": "0",
            "PATH": f"{cargo_home / 'bin'}{os.pathsep}{os.environ.get('PATH', '')}",
        }
    )


def _reads_pass_file(wrapper: Path) -> bool:
    try:
        return "TSFP_SCCACHE_PASS_FILE" in wrapper.read_text()
    except OSError:
        return False


def apply_shared_sccache(root: Path, main: Path, env: dict[str, str]) -> bool:
    """T3472: route this build's rustc and C compiles to the shared WebDAV sccache store.

    Only when the env already uses sccache through the wrapper (the wrapper reads the
    password file); otherwise, or when the tunnel is down, the local-disk cache stays."""
    if env.get("TSFP_NO_CCACHE") == "1":
        env.pop("CMAKE_C_COMPILER_LAUNCHER", None)
        env.pop("RUSTC_WRAPPER", None)
        return False
    wrapper = env.get("RUSTC_WRAPPER", "")
    if not wrapper.endswith("sccache-rustc.sh"):
        return False
    if not _reads_pass_file(Path(wrapper)):
        # An older main checkout's wrapper would start the shared daemon without the
        # password; use this worktree's wrapper, or stay on the local-disk cache.
        own = root / "tools/wt/sccache-rustc.sh"
        if not _reads_pass_file(own):
            return False
        wrapper = env["RUSTC_WRAPPER"] = str(own)
    from tools.wt import sccache_shared

    shared = sccache_shared.client_env(main, env)
    if not shared:
        return False
    env.update(shared)
    env["CMAKE_C_COMPILER_LAUNCHER"] = wrapper
    return True


def env_script(root: Path, main: Path, family: str) -> str:
    rows = ["# generated by python3 -m tools.wt bootstrap (T1883), do not commit"]
    rows += [f'export {k}="{v}"' for k, v in env_vars(root, main, family).items()]
    return "\n".join(rows) + "\n"


def doctor(root: Path, files_dir: Path | None) -> tuple[list[str], int]:
    main = main_root(root)
    lines, bad = [], 0

    def row(ok: bool, name: str, detail: str, required: bool = True) -> None:
        nonlocal bad
        mark = "ok     " if ok else ("MISSING" if required else "absent ")
        if not ok and required:
            bad += 1
        lines.append(f"{mark} {name}  {detail}".rstrip())

    from tools.wt import pyheal

    main_python = pyheal.ensure(main, expected_version=pyheal.pinned_version(root))
    row(
        main_python in ("ok", "no-venv") or main_python.startswith("ok ("),
        "main Python",
        main_python,
    )
    if root.resolve() != main.resolve():
        worktree_python = pyheal.ensure_worktree(root, main)
        row(
            worktree_python in ("ok", "skip") or worktree_python.startswith("relinked"),
            "worktree Python",
            worktree_python,
        )

    globbed = [t for s, t, _ in plan(root, main, files_dir) if s.kind == GLOB_DIRS]
    if globbed:
        have = sum(1 for t in globbed if t.exists())
        row(have == len(globbed), "tmp/private-host/lift-* links", f"{have}/{len(globbed)}", False)
    for spec, target, _source in plan(root, main, files_dir):
        if spec.kind == GLOB_DIRS:
            continue
        usable = target.exists() and (target.stat().st_size > 0 or target.is_dir())
        row(
            usable,
            str(target.relative_to(root)),
            "" if usable else f"({spec.why}; run tools.wt bootstrap)",
            spec.required,
        )
    for key, value in GIT_HYGIENE.items():
        current = git_local_config(root, key)
        row(
            current == value,
            f"git {key}={value}",
            "" if current == value else f"(is {current!r}; run tools.wt bootstrap, T3461 zombies)",
        )
    env = env_vars(root, main, "default")
    row((root / ENV_REL).exists(), ENV_REL, "(run tools.wt bootstrap)", False)
    row(Path(env["CARGO_HOME"], "bin", "cargo").exists(), "cargo", env["CARGO_HOME"])
    for tool, required in (
        ("clang", True),
        ("cmake", True),
        ("ninja", True),
        ("Xvfb", True),
        ("xvfb-run", False),
        ("ccache", False),
        ("sccache", False),
    ):
        found = shutil.which(tool)
        row(
            bool(found),
            tool,
            found or "(sudo apt-get install; list in .claude/missing_packages.txt)",
            required,
        )
    files = Path(env["TSFP_FILES_DIR"]) if not files_dir else files_dir
    for sub in ("bios", "mcpx", "hdd"):
        row((files / sub).exists(), f"owner files {sub}", "", False)
    lines.append(
        "RESULT " + ("all required items present" if not bad else f"{bad} required item(s) missing")
    )
    return lines, bad
