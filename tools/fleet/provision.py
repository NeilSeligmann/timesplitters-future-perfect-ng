"""One provisioning entry point that makes any worker host identical (T3626).

    python3 -m tools.fleet provision <host> [--apply] [--only STEP[,STEP]] [--list]
    [--allow-disruptive]

Replaces tmp/codexapi/server-provision.sh and deck-provision.sh. Every step is a read-only remote
check plus an idempotent apply. Without `--apply` only checks run (no scp, no apply command). All
remote effects go through the injectable `Runner` (tools.fleet.core), so tests use a recording fake.

Result states: `ok` (check passed), `fixed` (applied, re-check passed), `would-fix` (dry run),
`failed`, `skipped` (aborted by an unreachable host, or disruptive without --allow-disruptive).
Credentials (codex login, the git deploy key) are copied with scp only: their contents are never
read, printed or put on a command line.
"""

from __future__ import annotations

import argparse
import functools
import hashlib
import json
import re
import shlex
import subprocess
import tempfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

from tools import host_registry
from tools.fleet.config import REPO_ROOT, FleetConfig, HostConfig
from tools.fleet.core import CmdResult, Runner, local_runner, quote, scp_argv, ssh_argv
from tools.fleet.runner import lane_gate_text, render_runner, rgrc_text
from tools.wt import build_container, owner_links
from tools.wt.remote_git import NO_LAZY_EXPORT

REMOTE_DIR = Path(__file__).resolve().parent / "remote"
# T3755: the workers' state root is a registry fact (remote_hosts.json worker_state_root)
STATE_ROOT = str(host_registry.load().document["worker_state_root"])
SSH_GIT = (
    "ssh -i ~/.ssh-git/id_rsa_git -o IdentitiesOnly=yes -o StrictHostKeyChecking=no "
    "-o UserKnownHostsFile=/dev/null"
    # T3682: one multiplexed connection per host; the loaded git server reset ~30% of new
    # connections (2026-10-24), multiplexing took 25 of 25 through.
    " -o ControlMaster=auto -o ControlPath=~/.ssh-git/cm-%r@%h:%p -o ControlPersist=600"
    " -o ServerAliveInterval=30"
)
# The main checkout (local host, both containers share its .git): same multiplexing.
LOCAL_SSH_GIT = (
    "ssh -o ControlMaster=auto -o ControlPath=/tmp/tsfp-git-%r@%h:%p -o ControlPersist=600"
    " -o ServerAliveInterval=30"
)
SCCACHE_SIZE_BYTES = 20 * 1024**3
DISRUPTIVE_NOTE = "disruptive: pass --allow-disruptive"
PATH_PREFIX = 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.codex-cli/bin:$PATH"; '

# --- remote shell, one constant per step (@TOKEN@ placeholders are filled per host) ---
REACHABLE_CHECK = "true"
SUDO_CHECK = "sudo -n true"
LAYOUT_CHECK_STATE_ROOT = (
    'for d in csup rea-codex2; do [ -L ~/$d ] && [ "$(readlink ~/$d)" = @P@/$d ] || exit 1; done; '
    "test -d @P@/csup/codexapi/bin && test -d @P@/csup/state && test -d @P@/rea-codex2"
)
LAYOUT_APPLY_STATE_ROOT = (
    "mkdir -p @P@/csup/codexapi/briefs @P@/csup/codexapi/bin @P@/csup/state @P@/rea-codex2; "
    "for d in csup rea-codex2; do [ -L ~/$d ] || "
    "{ [ -e ~/$d ] && mv ~/$d ~/$d.old.$(date +%s); ln -s @P@/$d ~/$d; }; done"
)
LAYOUT_CHECK_HOME = "test -d ~/csup/codexapi/bin && test -d ~/csup/state && test -d ~/rea-codex2"
LAYOUT_APPLY_HOME = "mkdir -p ~/csup/codexapi/briefs ~/csup/codexapi/bin ~/csup/state ~/rea-codex2"
TOOLCHAIN_CHECK = (
    'for t in git uv node npm codex; do command -v $t >/dev/null || { echo "missing $t"; exit 1; '
    "}; done; "
    'rustc --version 2>&1 | grep -q "^rustc @RUST@" || { echo "rust @RUST@ not active"; exit 1; }'
)
TOOLCHAIN_APPLY = (
    "command -v rustup >/dev/null || { echo 'rustup missing: install rustup by hand' >&2; exit 1; "
    "}; "
    "rustup toolchain install @RUST@ --profile minimal"
)
UV_VERSION = "0.13.5"
UV_VERSION_CHECK = (
    f'uv --version | grep -Eq "^uv {UV_VERSION}( |$)"'  # uv prints "uv X.Y.Z (target)"
)
# uv installed without a receipt refuses `self update` (server, 2026-10-24): fall back to the
# pinned official installer into the same directory.
UV_VERSION_APPLY = (
    f"uv self update {UV_VERSION} || curl -LsSf https://astral.sh/uv/{UV_VERSION}/install.sh "
    '| env UV_NO_MODIFY_PATH=1 UV_INSTALL_DIR="$(dirname "$(command -v uv)")" sh'
)
CODEX_SYMLINK_CHECK = "command -v codex >/dev/null && test -L /usr/local/bin/codex"
CODEX_SYMLINK_APPLY = (
    "T=; for c in ~/.local/bin/codex ~/.codex-cli/bin/codex; do [ -e $c ] && { T=$c; break; }; "
    "done; "
    "[ -n \"$T\" ] || { echo 'no codex binary under ~/.local/bin or ~/.codex-cli/bin' >&2; exit 1; "
    "}; "
    'sudo -n ln -sfn "$T" /usr/local/bin/codex'
)
CODEX_LOGIN_CHECK = "test -s ~/.codex/auth.json"
CODEX_LOGIN_PREPARE = "mkdir -p ~/.codex && chmod 700 ~/.codex"
CODEX_LOGIN_FINISH = "chmod 600 ~/.codex/auth.json"
GIT_CONFIG_CHECK = (
    NO_LAZY_EXPORT + 'test "$(git config --global maintenance.autoDetach)" = false '
    '&& test "$(git config --global gc.autoDetach)" = false '
    "&& test -s ~/.ssh-git/id_rsa_git && { R=~/rea-codex2/repo; [ ! -d $R/.git ] || { "
    'test "$(git -C $R config maintenance.autoDetach)" = false && test "$(git -C $R config '
    'gc.autoDetach)" = false '
    "&& git -C $R config --get remote.origin.url >/dev/null && "
    'test "$(git -C $R config --get core.sshCommand)" = "@SSHGIT@"; }; }'
)
GIT_KEY_PREPARE = "mkdir -p ~/.ssh-git && chmod 700 ~/.ssh-git"
GIT_KEY_FINISH = "chmod 600 ~/.ssh-git/id_rsa_git"
GIT_CONFIG_APPLY = (
    NO_LAZY_EXPORT
    + "git config --global maintenance.autoDetach false; git config --global gc.autoDetach false; "
    "R=~/rea-codex2/repo; [ -d $R/.git ] && { git -C $R config maintenance.autoDetach false; "
    "git -C $R config gc.autoDetach false; "
    "git -C $R config --get remote.origin.url >/dev/null || git -C $R config remote.origin.url "
    "@URL@; "
    "git -C $R config --get remote.origin.fetch >/dev/null || "
    'git -C $R config remote.origin.fetch "+refs/heads/main:refs/remotes/origin/main"; '
    'git -C $R config core.sshCommand "@SSHGIT@"; }; true'
)
REPO_CHECK = (
    NO_LAZY_EXPORT
    + "R=~/rea-codex2/repo; test -d $R/.git && git -C $R rev-parse -q --verify HEAD >/dev/null "
    '&& test "$(git -C $R rev-parse --is-shallow-repository)" = false'
)
GIT_STORAGE_CHECK = (
    NO_LAZY_EXPORT + 'R="$HOME/rea-codex2/repo"; P=$(readlink -f "$R"); '
    'test "$(git -C "$R" config --get remote.origin.promisor)" = true '
    '&& test "$(git -C "$R" config --get remote.origin.partialclonefilter)" = blob:none '
    # git stores maintenance.repo resolved; ~/rea-codex2 is a symlink on the LAN hosts (2026-10-24)
    '&& git config --global --get-all maintenance.repo | grep -Fxq "$(cd "$R" && pwd -P)"'
)
GIT_STORAGE_APPLY = (
    NO_LAZY_EXPORT + 'R="$HOME/rea-codex2/repo"; git -C "$R" config remote.origin.promisor true; '
    'git -C "$R" config remote.origin.partialclonefilter blob:none; '
    'git -C "$R" fetch --filter=blob:none --no-tags origin main; '
    'git -C "$R" maintenance register; git -C "$R" maintenance start --scheduler=auto'
)
NO_LAZY_SCRIPTS = ("csup/lclean.sh", "csup/codexapi/bin/git")  # capguard.sh is git-free
NO_LAZY_PROBE_SECONDS = 5
NO_LAZY_CHECK = (
    "for f in ~/csup/lclean.sh ~/csup/codexapi/bin/git; do "
    "[ -e $f ] || continue; grep -q '^ *export GIT_NO_LAZY_FETCH=1' $f || exit 1; done; "
    'R="$HOME/rea-codex2/repo"; [ -d "$R/.git" ] || exit 0; '
    'S=$(date +%s); GIT_NO_LAZY_FETCH=1 timeout 5 git -C "$R" cat-file -e '
    "0000000000000000000000000000000000000000 >/dev/null 2>&1; rc=$?; "
    "[ $rc -ne 124 ] && [ $rc -ne 0 ] && [ $(( $(date +%s) - S )) -lt 5 ]"
)
REPO_APPLY = (
    NO_LAZY_EXPORT
    + 'export GIT_SSH_COMMAND="@SSHGIT@"; mkdir -p ~/rea-codex2; cd ~/rea-codex2 || exit 1; '
    "R=~/rea-codex2/repo; "
    "if [ -d $R/.git ] && ! git -C $R rev-parse -q --verify HEAD >/dev/null 2>&1; then "
    "mv $R $R.broken-$(date +%Y%m%d%H%M); fi; "
    "if [ ! -d $R/.git ]; then timeout 1700 git clone -q --single-branch --branch main @URL@ repo "
    '&& git -C repo config core.sshCommand "@SSHGIT@" && mkdir -p repo/tmp/t2368-v4; '
    "else busy=0; RR=$(readlink -f $R); for d in /proc/[0-9]*; do "
    '[ "$(readlink $d/cwd 2>/dev/null)" = "$RR" ] && [ "$(cat $d/comm 2>/dev/null)" = git ] '
    '&& busy=1; done; ps -eo args | grep -Eq "^git -C $R " && busy=1; '
    "[ $busy = 0 ] && find $R/.git -maxdepth 1 -name '*.lock' -mmin +60 -delete; "
    "timeout 1700 git -C $R fetch -q --unshallow origin "
    "+refs/heads/main:refs/remotes/origin/main "
    # a worker updating origin/main meanwhile fails the ref update after the objects arrived
    '|| test "$(git -C $R rev-parse --is-shallow-repository)" = false; fi'
)
PYTHON_VERSION = (
    'V=$(cat ~/rea-codex2/repo/.python-version 2>/dev/null || echo @PYTHON@); V=$(echo "$V" | tr '
    '-d " \\r\\n"); '
    "C=~/rea-codex2/repo/.venv/pyvenv.cfg; "
)
PYTHON_CHECK = (
    PYTHON_VERSION
    + 'uv python find "$V" >/dev/null 2>&1 && grep -Eq "^version_info *= *$V(\\.|$)" $C '
    '|| { echo "python $V not in repo venv"; exit 1; }'
)
PYTHON_APPLY = (
    PYTHON_VERSION + 'uv python install "$V" && cd ~/rea-codex2/repo && uv sync -q --python "$V"'
)
SCCACHE_CHECK = (
    "python3 - <<'TSFP_CACHE_CHECK'\n"
    "import pathlib, tomllib\n"
    "try:\n"
    "    c = tomllib.loads((pathlib.Path.home() / '.config/sccache/config').read_text())\n"
    "    d = c['cache']['disk']\n"
    "    if not (isinstance(d['dir'], str) and d['dir']\n"
    "            and type(d['size']) is int and d['size'] > 0):\n"
    "        raise SystemExit(1)\n"
    "except (OSError, ValueError, KeyError, TypeError):\n"
    "    raise SystemExit(1)\n"
    "TSFP_CACHE_CHECK"
)
SCCACHE_APPLY = (
    "mkdir -p @SCDIR@ && python3 ~/.config/sccache/tsfp-config.py "
    "--path ~/.config/sccache/config --directory @SCDIR@ --size @SCSIZE@"
)
CONTEXT_MODE_CHECK = "test -x ~/.local/bin/context-mode"
CONTEXT_MODE_APPLY = "timeout 300 npm i -g --prefix ~/.local context-mode@1.0.169 >/dev/null 2>&1"
MEMGUARD_LOCK = "~/csup/memguard.lock"
# Anchored to the interpreter: an unanchored '[m]emguard.py' matched the provisioning shell's own
# command line (it contains ~/csup/memguard.py), so the guard never started (I-T3865-3).
MEMGUARD_PGREP = "'^python3 [^ ]*[m]emguard[.]py'"
MEMGUARD_CHECK = (
    f"if test -e {MEMGUARD_LOCK} && ! flock -n {MEMGUARD_LOCK} true; then echo held; "
    f"elif pgrep -f {MEMGUARD_PGREP} >/dev/null; then echo legacy; else exit 1; fi"
)
MEMGUARD_APPLY = (
    f"if test -e {MEMGUARD_LOCK} && ! flock -n {MEMGUARD_LOCK} true; then exit 0; fi; "
    f"pgrep -f {MEMGUARD_PGREP} >/dev/null && exit 0; cd ~/csup && "
    # setsid -f forks a new session now; `setsid nohup ... &` died with the ssh (I-T3678-18).
    f"setsid -f flock -n {MEMGUARD_LOCK} python3 ~/csup/memguard.py @MGARGS@ --max-pause-min 20 "
    "--log ~/csup/memguard.log </dev/null >/dev/null 2>&1"
)
LOWPRI_CHECK = (
    'ps -u "$(id -un)" -o etimes=,ni=,comm= '
    "| awk '$1 > 120 && $2 < 19 && $3 !~ /^(sshd|ssh)$/ {n++} END {exit n > 0}'"
)
LOWPRI_APPLY = (
    'renice -n 19 -u "$(id -un)" >/dev/null 2>&1; '
    'for p in $(pgrep -u "$(id -un)"); do ionice -c3 -p $p 2>/dev/null; done; true'
)
TUNNEL_NOTE = "echo 'tunnel policy @TUNNEL@: owned by the controller, not started here'"


@dataclass
class Ctx:
    config: FleetConfig
    host: HostConfig
    runner: Runner
    scp: Runner
    apply: bool
    allow_disruptive: bool
    home: Path
    subst: dict[str, str] = field(default_factory=dict)
    prefix: str = PATH_PREFIX

    def remote(self, command: str, timeout: float = 60) -> CmdResult:
        argv = ssh_argv(self.host.target, self.host.port, self.prefix + command)
        return self.runner(argv, timeout)

    def copy(
        self, sources: Sequence[str | Path], destination: str, timeout: float = 300
    ) -> CmdResult:
        return self.scp(scp_argv(self.host.target, self.host.port, sources, destination), timeout)

    def render(self, template: str) -> str:
        for key, value in self.subst.items():
            template = template.replace(f"@{key}@", value)
        return template


@dataclass(frozen=True)
class Step:
    name: str
    doc: str
    check_cmd: str | None = None
    apply_cmd: str | None = None
    check_fn: Callable[[Ctx], CmdResult] | None = None
    apply_fn: Callable[[Ctx], CmdResult] | None = None
    disruptive: bool = False
    local_prereq: Callable[[Ctx], str | None] | None = None
    applies: Callable[[Ctx], bool] | None = None
    check_timeout: float = 60
    apply_timeout: float = 120


@dataclass(frozen=True)
class StepResult:
    step: str
    state: str
    detail: str = ""

    def line(self) -> str:
        return f"{self.step:<22} {self.state}" + (f"  {self.detail}" if self.detail else "")


# --- local helpers ---
def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _tail(result: CmdResult) -> str:
    text = (result.err or result.out).strip().splitlines()
    return text[-1][:200] if text else f"rc={result.rc}"


def _state_root(ctx: Ctx) -> bool:
    return str(ctx.host.registry.get("canon_root", "")).startswith(STATE_ROOT)


def payload_entries(ctx: Ctx) -> tuple[list[tuple[Path, str]], list[str]]:
    """(local file, remote path relative to $HOME) for the whole payload, plus missing local
    sources."""
    codexapi = ctx.config.codexapi_dir
    entries: list[tuple[Path, str]] = []
    missing: list[str] = []

    def add(path: Path, remote: str, required: bool = True) -> None:
        if path.is_file():
            entries.append((path, remote))
        elif required:
            missing.append(str(path))

    for name in ctx.config.provision.get("payload_from_codexapi", []):
        deck = codexapi / "deck-files" / name
        add(deck if deck.is_file() else codexapi / name, f"csup/codexapi/{name}")
    add(codexapi / "deck-files" / "wt-setup.sh", "csup/wt-setup.sh")
    for name in (
        "capguard.sh",
        "lclean.sh",
        "memguard.py",
        "reaper.py",
        "rotate-codex-sessions.sh",
    ):
        add(REMOTE_DIR / name, f"csup/{name}")
    for path, remote in CAPGUARD_SHIPPED:
        add(path, remote)
    add(REMOTE_DIR / "bin" / "git", "csup/codexapi/bin/git")
    for name in ("rg", "cargo", "rustfmt"):
        add(codexapi / "bin" / name, f"csup/codexapi/bin/{name}", required=name == "rg")
    add(REMOTE_DIR / "cargo-slot", "csup/codexapi/bin/cargo-slot")
    return entries, missing


def provisioned_script_entries(ctx: Ctx) -> tuple[list[tuple[Path, str]], list[str]]:
    """Tracked script payload copied to remote hosts, excluding accompanying data files."""
    entries, missing = payload_entries(ctx)
    scripts = [
        (source, remote)
        for source, remote in entries
        if source.suffix in {".sh", ".py"} or "/bin/" in remote
    ]
    return scripts, missing


SERVER_CARGO_ADMISSION_CHECK = (
    "test -L ~/.local/bin/cargo && "
    'test "$(readlink ~/.local/bin/cargo)" = "$HOME/csup/codexapi/bin/cargo-slot" && '
    "grep -Fqx 'export PATH=\"$HOME/.local/bin:$PATH\"' ~/.profile"
)
SERVER_CARGO_ADMISSION_APPLY = (
    "mkdir -p ~/.local/bin; "
    "test -x ~/csup/codexapi/bin/cargo-slot || "
    '{ echo "cargo admission wrapper missing" >&2; exit 1; }; '
    "if [ -e ~/.local/bin/cargo ] && [ ! -L ~/.local/bin/cargo ]; then "
    'echo "refusing to replace existing ~/.local/bin/cargo" >&2; exit 1; fi; '
    'ln -sfn "$HOME/csup/codexapi/bin/cargo-slot" ~/.local/bin/cargo; '
    "if ! grep -Fqx 'export PATH=\"$HOME/.local/bin:$PATH\"' ~/.profile 2>/dev/null; then "
    'if [ -L ~/.profile ]; then echo "refusing to edit symlinked ~/.profile" >&2; exit 1; fi; '
    "printf '%s\\n' 'export PATH=\"$HOME/.local/bin:$PATH\"' >> ~/.profile; fi; "
    'export PATH="$HOME/.local/bin:$PATH"'
)


def _sha_check(entries: Sequence[tuple[Path, str]]) -> str:
    items = " ".join(f'"{_sha256(path)}  $HOME/{remote}"' for path, remote in entries)
    return f"printf '%s\\n' {items} | sha256sum -c --quiet - >/dev/null 2>&1"


def _install(ctx: Ctx, entries: Sequence[tuple[Path, str]]) -> CmdResult:
    """Install staged files atomically, keeping the prior version as `.bak`."""
    groups: dict[str, list[tuple[Path, str]]] = {}
    for path, remote in entries:
        destination, _, name = remote.rpartition("/")
        groups.setdefault(destination, []).append((path, name))
    for index, (destination, files) in enumerate(groups.items()):
        stage = f"csup/.fleet-stage/{index}"
        result = ctx.remote(f"mkdir -p ~/{stage} ~/{destination}")
        if not result.ok:
            return result
        result = ctx.copy([path for path, _ in files], f"{stage}/")
        if not result.ok:
            return result
        moves_list: list[str] = []
        for _, name in files:
            staged = f"~/{stage}/{name}"
            target = f"~/{destination}/{name}"
            if name.endswith(".sh"):
                moves_list.append(f"sh -n {staged} || exit; chmod +x {staged}")
            elif name.endswith(".py"):
                moves_list.append(
                    "python3 -c 'import pathlib,sys; "
                    "compile(pathlib.Path(sys.argv[1]).read_bytes(),"
                    f'sys.argv[1], "exec")\' {staged} || exit; chmod +x {staged}'
                )
            elif destination.endswith("/bin"):
                moves_list.append(f"chmod +x {staged}")
            moves_list.append(
                f"if [ -e {target} ]; then cp -p {target} {target}.bak || exit; fi; "
                f"mv -f {staged} {target} || exit"
            )
        moves = "; ".join(moves_list)
        result = ctx.remote(f"{moves}; rmdir ~/{stage} 2>/dev/null; true")
        if not result.ok:
            return result
    return CmdResult(0)


# --- step implementations ---
def _sudo_apply(ctx: Ctx) -> CmdResult:
    return CmdResult(
        1,
        "",
        "passwordless sudo missing: add 'NOPASSWD' for the worker user on the host (never prompts)",
    )


def _codex_login_prereq(ctx: Ctx) -> str | None:
    return (
        None
        if (ctx.home / ".codex" / "auth.json").is_file()
        else "local ~/.codex/auth.json missing"
    )


def _codex_login_apply(ctx: Ctx) -> CmdResult:
    result = ctx.remote(CODEX_LOGIN_PREPARE)
    if not result.ok:
        return result
    names = [
        n
        for n in ctx.config.provision.get("codex_login_files", ["auth.json"])
        if (ctx.home / ".codex" / n).is_file()
    ]
    result = ctx.copy([ctx.home / ".codex" / n for n in names], ".codex/")
    return result if not result.ok else ctx.remote(CODEX_LOGIN_FINISH)


def _git_config_apply(ctx: Ctx) -> CmdResult:
    if not ctx.remote("test -s ~/.ssh-git/id_rsa_git").ok:
        key = ctx.home / ".ssh" / "id_rsa"
        if not key.is_file():
            return CmdResult(1, "", "local ~/.ssh/id_rsa missing (deploy key source)")
        for step in (
            lambda: ctx.remote(GIT_KEY_PREPARE),
            lambda: ctx.copy([key], ".ssh-git/id_rsa_git", 60),
            lambda: ctx.remote(GIT_KEY_FINISH),
        ):
            result = step()
            if not result.ok:
                return result
    return ctx.remote(ctx.render(GIT_CONFIG_APPLY))


def _gitshim_entries() -> list[tuple[Path, str]]:
    return [(REMOTE_DIR / "bin" / "git", "csup/codexapi/bin/git")]


def _gitshim_check(ctx: Ctx) -> CmdResult:
    return ctx.remote(_sha_check(_gitshim_entries()))


def _payload_check(ctx: Ctx) -> CmdResult:
    entries, missing = payload_entries(ctx)
    if missing:
        return CmdResult(
            1, "", "local payload missing: " + ", ".join(Path(m).name for m in missing)
        )
    command = _sha_check(entries)
    return ctx.remote(command)


def _script_payload_check(ctx: Ctx) -> CmdResult:
    entries, missing = provisioned_script_entries(ctx)
    if missing:
        return CmdResult(
            1, "", "local script payload missing: " + ", ".join(Path(m).name for m in missing)
        )
    return ctx.remote(_sha_check(entries))


def _payload_apply(ctx: Ctx) -> CmdResult:
    entries, missing = payload_entries(ctx)
    if missing:
        return CmdResult(
            1, "", "local payload missing: " + ", ".join(Path(m).name for m in missing)
        )
    result = _install(ctx, entries)
    return result


def _sccache_dir(ctx: Ctx) -> str:
    canon = ctx.host.registry.get("canon_root")
    return f"{canon}/sccache" if canon else "$HOME/.cache/sccache"


def _sccache_check(ctx: Ctx) -> CmdResult:
    result = ctx.remote(ctx.render(SCCACHE_CHECK))
    route = ctx.host.registry.get("sccache", "none")
    if not result.ok or route not in ("server", "forward", "reverse") or not ctx.apply:
        return result
    tool = ctx.runner(
        [
            "env",
            "-C",
            str(REPO_ROOT),
            "python3",
            "-m",
            "tools.wt.sccache_hosts",
            "--host",
            ctx.host.name,
        ],
        120,
    )
    return CmdResult(tool.rc, "", "" if tool.ok else "sccache route: " + _tail(tool))


def _sccache_apply(ctx: Ctx) -> CmdResult:
    ready = ctx.remote("mkdir -p ~/.config/sccache")
    if not ready.ok:
        return ready
    copied = ctx.copy([REMOTE_DIR / "sccache_config.py"], "~/.config/sccache/tsfp-config.py")
    if not copied.ok:
        return copied
    return ctx.remote(ctx.render(SCCACHE_APPLY))


#: Standalone stdlib scripts capguard.sh runs from ~/csup (exec-only hosts have no repo, rule 47).
CAPGUARD_SHIPPED = (
    (REMOTE_DIR / "diskprune.py", "csup/diskprune.py"),
    (REMOTE_DIR / "ownerevict.py", "csup/ownerevict.py"),
    (REMOTE_DIR / "ownerscan.py", "csup/ownerscan.py"),
    (REPO_ROOT / "tools" / "wt" / "replay_input_cache.py", "csup/replay_input_cache.py"),
    (REPO_ROOT / "tools" / "fleet" / "integrity.py", "csup/integrity.py"),
)


def _guard_entries(name: str) -> list[tuple[Path, str]]:
    entries = [(REMOTE_DIR / name, f"csup/{name}")]
    if (
        name == "capguard.sh"
    ):  # capguard runs these (T3749, cx-capguard-execonly): ship and hash together
        entries.extend(CAPGUARD_SHIPPED)
    return entries


#: capguard.env variable -> remote_hosts.json key (cx-capguard-execonly). Required keys fail closed.
CAPGUARD_ENV_KEYS = (
    ("DATA_ROOT", "data_root", True),
    ("REPLAY_CACHE_DIR", "replay_cache_dir", True),
    ("EXEC_WORKDIR", "workdir", True),
    ("TARGET_DIR", "target_dir", False),
    ("CARGO_TARGET_CAP_GB", "target_cap_gb", True),
    ("CAP_GB", "data_cap_gb", True),
    ("MIN_FREE_GB", "disk_floor_gb", True),
    ("REPLAY_INPUT_CACHE_GB", "replay_input_cache_gb", False),
    ("CANON_ROOT", "canon_root", False),
    ("SCCACHE_MODE", "sccache", False),
    ("LIFT_SHIP_KEEP", "lift_ship_keep", False),
)


def capguard_env_text(ctx: Ctx) -> str:
    """~/csup/capguard.env for this host: every path and cap capguard.sh uses, from the registry.
    `${X:-value}` lets a caller (balancer, tests) override one value. Raises on a missing key."""
    registry = ctx.host.registry
    missing = [key for _, key, required in CAPGUARD_ENV_KEYS if required and key not in registry]
    if missing:
        raise ValueError(f"registry host {ctx.host.name} lacks {', '.join(missing)} for capguard")
    lines = [
        "# Rendered by tools.fleet provision (capguard step) from tools/data/remote_hosts.json.",
        f"CAPGUARD_HOST=${{CAPGUARD_HOST:-{shlex.quote(ctx.host.name)}}}",
    ]
    for variable, key, _ in CAPGUARD_ENV_KEYS:
        if key in registry:
            lines.append(f"{variable}=${{{variable}:-{shlex.quote(str(registry[key]))}}}")
    # Owner 2026-10-28: a host flagged owner_files holds them in <data_root>/tmp/owner (I-T3879).
    owner_inputs = 1 if _owner_input_host(ctx) else 0
    lines.append(f"OWNER_INPUTS=${{OWNER_INPUTS:-{owner_inputs}}}")
    return "\n".join(lines) + "\n"


def _capguard_stage(ctx: Ctx, directory: Path) -> list[tuple[Path, str]]:
    env = directory / "capguard.env"
    env.write_text(capguard_env_text(ctx))
    return [*_guard_entries("capguard.sh"), (env, "csup/capguard.env")]


def _capguard_check(ctx: Ctx) -> CmdResult:
    """capguard.sh, its shipped scripts and the rendered capguard.env match by sha256."""
    with tempfile.TemporaryDirectory() as stage:
        try:
            entries = _capguard_stage(ctx, Path(stage))
        except ValueError as error:
            return CmdResult(1, "", str(error))
        return ctx.remote(_sha_check(entries))


def _capguard_apply(ctx: Ctx) -> CmdResult:
    with tempfile.TemporaryDirectory() as stage:
        try:
            entries = _capguard_stage(ctx, Path(stage))
        except ValueError as error:
            return CmdResult(1, "", str(error))
        return _install(ctx, entries)


# A host-local timer continues enforcing the budget if the fleet controller is down.
# Preserve unrelated crontab entries and replace only our marked entry.
CAPGUARD_CRON_ENTRY = (
    '*/15 * * * * flock -n "$HOME/csup/capguard-timer.lock" '
    'sh "$HOME/csup/capguard.sh" >> "$HOME/csup/capguard.log" 2>&1 # tsfp-capguard-timer'
)
CAPGUARD_CRON_CHECK = (
    "pgrep -x cron >/dev/null && crontab -l 2>/dev/null | grep -Fqx "
    + shlex.quote(CAPGUARD_CRON_ENTRY)
)
CAPGUARD_CRON_APPLY = (
    "if ! command -v crontab >/dev/null 2>&1; then "
    "sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -q cron || exit 1; fi; "
    "pgrep -x cron >/dev/null || sudo -n service cron start || exit 1; "
    "(crontab -l 2>/dev/null | grep -v '# tsfp-capguard-timer$'; printf '%s\\n' "
    + shlex.quote(CAPGUARD_CRON_ENTRY)
    + ") | crontab -"
)


def _guard_check(name: str) -> Callable[[Ctx], CmdResult]:
    """sha256 against the tracked script (T3708: `sh -n` let a hand-patched copy pass)."""

    def check(ctx: Ctx) -> CmdResult:
        entries = _guard_entries(name)
        if not entries[0][0].is_file():
            return CmdResult(1, "", f"local payload missing: {name}")
        return ctx.remote(_sha_check(entries))

    return check


def _guard_apply(name: str) -> Callable[[Ctx], CmdResult]:
    def apply(ctx: Ctx) -> CmdResult:
        return _install(ctx, _guard_entries(name))

    return apply


def _no_lazy_apply(ctx: Ctx) -> CmdResult:
    """Re-install the remote scripts (the tracked copies carry the export), then re-check."""
    entries: list[tuple[Path, str]] = []
    if ctx.host.lclean:
        entries.append((REMOTE_DIR / "lclean.sh", "csup/lclean.sh"))
    entries.append((REMOTE_DIR / "bin" / "git", "csup/codexapi/bin/git"))
    return _install(ctx, entries)


def _runner_stage(ctx: Ctx, directory: Path) -> list[tuple[Path, str]]:
    """Render run.sh, rgrc and lane_gate.sh for this host into `directory` (file, remote path)."""
    files = {
        "run.sh": render_runner(ctx.config, ctx.host.name),
        "rgrc": rgrc_text(),
        "lane_gate.sh": lane_gate_text(),
    }
    entries = []
    for name, text in files.items():
        path = directory / name
        path.write_text(text)
        entries.append((path, f"csup/codexapi/{name}"))
    return entries


def runner_check(ctx: Ctx) -> CmdResult:
    """Installed runner files equal the render by sha256 (also used by the HEALTH detector)."""
    with tempfile.TemporaryDirectory() as stage:
        return ctx.remote(_sha_check(_runner_stage(ctx, Path(stage))))


def _runner_apply(ctx: Ctx) -> CmdResult:
    """`_install` stages, syntax-checks, keeps `.bak` and renames atomically (mv -f)."""
    with tempfile.TemporaryDirectory() as stage:
        return _install(ctx, _runner_stage(ctx, Path(stage)))


def _gitshim_apply(ctx: Ctx) -> CmdResult:
    return _install(ctx, _gitshim_entries())


PYTHON_DEPS_STAMP = "csup/python-deps.sha256"
PYTHON_DEPS_FILE = "csup/python-deps.txt"
DEFAULT_HOST_VENV = "~/rea-codex2/repo/.venv"
"""Fallback for a registry host without `host_venv` (cx-execlayout: every host names it)."""


def host_layout(ctx: Ctx) -> dict[str, str]:
    """cx-execlayout: the registry's exec-only layout (`host_registry.HOST_LAYOUT_KEYS`)."""
    return {key: str(ctx.host.registry.get(key) or "") for key in host_registry.HOST_LAYOUT_KEYS}


def host_venv_python(ctx: Ctx) -> str:
    """The registry `host_venv` interpreter as a shell word (`~/` kept unquoted)."""
    venv = (host_layout(ctx)["host_venv"] or DEFAULT_HOST_VENV).rstrip("/") + "/bin/python"
    return "~/" + shlex.quote(venv[2:]) if venv.startswith("~/") else shlex.quote(venv)


def _shell_dir(path: str) -> str:
    return '"$HOME"/' + shlex.quote(path[2:]) if path.startswith("~/") else shlex.quote(path)


def exec_layout_dirs(ctx: Ctx) -> list[str]:
    """The plain dirs a new exec-only host needs (toolchain, replay cache), no git repo."""
    layout = host_layout(ctx)
    return [_shell_dir(layout[key]) for key in ("toolchain_dir", "replay_cache_dir") if layout[key]]


def exec_layout_check(ctx: Ctx) -> CmdResult:
    """Every layout key is set in the registry and its dir exists on the host."""
    missing = [key for key, value in host_layout(ctx).items() if not value]
    if missing:
        return CmdResult(1, "", f"registry lacks {', '.join(missing)}")
    tests = " && ".join(f"test -d {path}" for path in exec_layout_dirs(ctx))
    return ctx.remote(f"{tests} || {{ echo 'exec layout dir missing'; exit 1; }}")


def _exec_layout_apply(ctx: Ctx) -> CmdResult:
    """mkdir -p the layout dirs; python-deps creates `host_venv`, toolchain fills `tmp/rust`."""
    return ctx.remote("mkdir -p " + " ".join(exec_layout_dirs(ctx)))


def _host_keeps_git(ctx: Ctx) -> bool:
    """Rule 47 (T3760): only the owner PC holds a git repo; remote hosts are exec-only."""
    return ctx.host.is_local


@functools.lru_cache(maxsize=4)
def _export_lock(root: str, lock_mtime_ns: int) -> str:
    result = subprocess.run(
        ["uv", "export", "--frozen", "--no-hashes", "--no-emit-project", "--all-groups"],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"uv export failed: {result.stderr.strip()[-200:]}")
    return "\n".join(line for line in result.stdout.splitlines() if not line.startswith("#")) + "\n"


def lock_requirements(ctx: Ctx) -> str:
    """Requirements exported from the controller's tracked uv.lock (comment lines dropped)."""
    root = ctx.config.root
    return _export_lock(str(root), (root / "uv.lock").stat().st_mtime_ns)


def python_deps_digest(ctx: Ctx) -> str:
    return hashlib.sha256(lock_requirements(ctx).encode()).hexdigest()


def exec_venv(ctx: Ctx) -> str | None:
    """T3760: the host's exec venv (`remote_exec.exec_venv`, beside the exec dirs, outside any
    git repo), or None for a host missing from tools/data/remote_hosts.json."""
    from tools.wt import remote_exec

    try:
        return remote_exec.exec_venv(remote_exec.find_host(ctx.host.name))
    except remote_exec.RemoteExecError:
        return None


def python_deps_check(ctx: Ctx) -> CmdResult:
    """The host stamp equals the sha256 of the current lock export and the exec venv exists
    (also the HEALTH detector)."""
    digest = python_deps_digest(ctx)
    venv = exec_venv(ctx)
    has_venv = f" && test -x {shlex.quote(venv + '/bin/python')}" if venv else ""
    return ctx.remote(
        f'test "$(cat ~/{PYTHON_DEPS_STAMP} 2>/dev/null)" = {digest}{has_venv} '
        f"|| {{ echo 'python deps stamp differs from uv.lock or exec venv missing'; exit 1; }}"
    )


def _python_deps_apply(ctx: Ctx) -> CmdResult:
    """Install-only (no removals, editable installs stay) then write the stamp."""
    text = lock_requirements(ctx)
    digest = hashlib.sha256(text.encode()).hexdigest()
    with tempfile.TemporaryDirectory() as stage:
        path = Path(stage) / "python-deps.txt"
        path.write_text(text)
        result = ctx.remote("mkdir -p ~/csup")
        if not result.ok:
            return result
        result = ctx.copy([path], PYTHON_DEPS_FILE)
        if not result.ok:
            return result
    # T3760: remote jobs run from exec dirs with the exec venv; the registry `host_venv`
    # (cx-execlayout) is kept current while it exists.
    host_python = host_venv_python(ctx)
    install = f"uv pip install -q --python {host_python} -r ~/{PYTHON_DEPS_FILE}"
    command = f"{{ [ ! -x {host_python} ] || {install}; }}"
    venv = exec_venv(ctx)
    if venv:
        pinned = ctx.config.root / ".python-version"
        version = pinned.read_text().strip() if pinned.is_file() else ctx.render("@PYTHON@")
        python = shlex.quote(venv + "/bin/python")
        create = f"uv venv -q --python {shlex.quote(version)} {shlex.quote(venv)}"
        command += (
            f" && {{ [ -x {python} ] || {create}; }}"
            f" && uv pip install -q --python {python} -r ~/{PYTHON_DEPS_FILE}"
        )
    return ctx.remote(f"{command} && printf '%s\\n' {digest} > ~/{PYTHON_DEPS_STAMP}", 1200)


EXPECTED_TOOLCHAIN = REPO_ROOT / "tools" / "data" / "expected_toolchain.json"
TOOLCHAIN_TOOLS = ("cc", "cmake", "python3")
MEDIA_BUILD_LIBRARIES = (
    "libavcodec",
    "libavformat",
    "libavutil",
    "libavfilter",
    "libavdevice",
    "libswresample",
    "libswscale",
)
MEDIA_BUILD_PACKAGES = tuple(f"{name}-dev" for name in MEDIA_BUILD_LIBRARIES)
MEDIA_BUILD_CHECK = "pkg-config --exists " + " ".join(MEDIA_BUILD_LIBRARIES)
MEDIA_BUILD_APPLY = (
    "command -v apt-get >/dev/null && sudo -n env DEBIAN_FRONTEND=noninteractive "
    "apt-get install -y -q --no-upgrade --no-remove --no-install-recommends "
    + " ".join(MEDIA_BUILD_PACKAGES)
)
# Same PATH the remote build runs under (tools.wt env), same probe as tools/wt/remote_toolchain.py.
TOOLCHAIN_PATH = 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"; '
PARITY_APPLY = (
    TOOLCHAIN_PATH + "set -e; mkdir -p ~/.local/bin; "
    # cmake: the upstream release tarball, SHA-256 checked against the release's own list.
    'if [ "$(cmake --version 2>&1 | head -n 1)" != "cmake version @CMAKE@" ]; then '
    "T=cmake-@CMAKE@-linux-$(uname -m); "
    "U=https://github.com/Kitware/CMake/releases/download/v@CMAKE@; D=$(mktemp -d); "
    "cd $D && curl -fsSLO $U/$T.tar.gz && curl -fsSLO $U/cmake-@CMAKE@-SHA-256.txt && "
    'grep -F " $T.tar.gz" cmake-@CMAKE@-SHA-256.txt | sha256sum -c - && '
    "tar -xzf $T.tar.gz -C ~/.local && ln -sfn ~/.local/$T/bin/cmake ~/.local/bin/cmake && "
    "cd ~ && rm -rf $D; fi; "
    # python3: uv's managed CPython of exactly that release behind ~/.local/bin/python3.
    'if [ "$(python3 --version 2>&1 | head -n 1)" != "Python @PYTHON@" ]; then '
    'uv python install @PYTHON@ && ln -sfn "$(uv python find @PYTHON@)" ~/.local/bin/python3; '
    "fi; "
    # cc: only a Debian host can report the Debian clang vendor string (docs/t-toolchain-parity.md).
    'if [ "$(cc --version 2>&1 | head -n 1)" != @CC@ ]; then '
    '. /etc/os-release; if [ "$ID" = debian ]; then '
    "sudo -n apt-get install -y clang-@CLANGMAJOR@ && "
    'ln -sfn "$(command -v clang-@CLANGMAJOR@)" ~/.local/bin/cc; '
    'else echo "cc: $ID cannot report the Debian clang vendor string"; fi; fi; true'
)


def expected_toolchain() -> dict[str, str]:
    """The tracked first `--version` line per compared tool (tools/data/expected_toolchain.json)."""
    data = json.loads(EXPECTED_TOOLCHAIN.read_text())
    return {tool: str(data[tool]) for tool in TOOLCHAIN_TOOLS}


def toolchain_check_command(expected: dict[str, str]) -> str:
    lines = [TOOLCHAIN_PATH + "rc=0"]
    for tool in TOOLCHAIN_TOOLS:
        lines.append(
            f"have=$({tool} --version 2>&1 | head -n 1); "
            f'[ "$have" = {shlex.quote(expected[tool])} ] || '
            f'{{ echo "{tool}: $have vs {expected[tool]}"; rc=1; }}'
        )
    lines.append("exit $rc")
    return "; ".join(lines)


BUILD_SH = Path(__file__).resolve().parents[2] / "tools/dev/build.sh"


def build_sh_required_tools(build_sh: Path = BUILD_SH) -> list[str]:
    """The tools build.sh refuses to run without (its `required="..."` line), so a host check
    never drifts from the script (rule 45). I-T3865-8: the laptop lost ~/.local/bin/clang in a
    disk cleanup and every remote prebuild failed with "missing tools: clang"."""
    match = re.search(r'^required="([^"]+)"', build_sh.read_text(), re.MULTILINE)
    if not match:
        raise ValueError(f'{build_sh} has no required="..." tool line')
    return match.group(1).split()


def build_tools_check_command(tools: Sequence[str]) -> str:
    names = " ".join(shlex.quote(tool) for tool in tools)
    return (
        f'm=; for t in {names}; do command -v "$t" >/dev/null 2>&1 || m="$m $t"; done; '
        '[ -z "$m" ] || { echo "missing build tools:$m"; exit 1; }'
    )


def build_tools_apply_command(tools: Sequence[str]) -> str:
    """Link a missing tool to its newest versioned binary (clang -> clang-19); anything else
    stays missing and the re-check names it."""
    names = " ".join(shlex.quote(tool) for tool in tools)
    return (
        f'mkdir -p ~/.local/bin; for t in {names}; do command -v "$t" >/dev/null 2>&1 && continue; '
        'v=$(ls /usr/bin/"$t"-[0-9]* 2>/dev/null | sort -V | tail -n 1); '
        '[ -n "$v" ] && ln -sfn "$v" ~/.local/bin/"$t"; done; true'
    )


def _build_tools_check(ctx: Ctx) -> CmdResult:
    return ctx.remote(build_tools_check_command(build_sh_required_tools()))


def _build_tools_apply(ctx: Ctx) -> CmdResult:
    return ctx.remote(build_tools_apply_command(build_sh_required_tools()))


DEFAULT_DATA_ROOT = "~/rea-codex2/repo"


def _owner_input_host(ctx: Ctx) -> bool:
    flags = ctx.host.registry.get("owner_files") or {}
    return bool(flags) and all(flags.values())


def owner_input_sources(root: Path = REPO_ROOT) -> list[Path]:
    """The owner inputs a gate/replay host holds: disc image, recordings, routes, saves (owner
    2026-10-28: "You can keep whatever you want on the remotes")."""
    from tools import owner_files

    owner = root / "tmp/owner"
    sources = [owner_files.disc_image(root)]
    sources += [owner / name for name in ("recordings", "routes") if (owner / name).is_dir()]
    sources += sorted(owner.glob("saves-*"))
    return sources


def _owner_dest(ctx: Ctx) -> str:
    root = str(ctx.host.registry.get("data_root") or DEFAULT_DATA_ROOT)
    return root.removeprefix("~/") + "/tmp/owner"


def _owner_inputs_check(ctx: Ctx) -> CmdResult:
    dest = _owner_dest(ctx)
    names = [shlex.quote(f"{dest}/{path.name}") for path in owner_input_sources()]
    store = owner_links.store_dir(str(ctx.host.registry.get("data_root") or DEFAULT_DATA_ROOT))
    names += [shlex.quote(f"{store}/{rel}") for rel, _ in owner_links.sources(REPO_ROOT)]
    return ctx.remote(
        "cd ~ && for p in "
        + " ".join(names)
        + '; do [ -e "$p" ] || { echo "missing $p"; exit 1; }; done'
    )


def _owner_inputs_apply(ctx: Ctx) -> CmdResult:
    """Restore missing owner inputs (I-T3865-19: a container restart wiped the laptop's tmp/owner
    and the registry still said it held the disc)."""
    dest = _owner_dest(ctx)
    made = ctx.remote(f"mkdir -p ~/{shlex.quote(dest)}")
    if not made.ok:
        return made
    ssh = f"ssh -o BatchMode=yes -p {ctx.host.port}"
    sources = [str(path) for path in owner_input_sources()]
    done = ctx.runner(
        ["rsync", "-a", "--partial", "-e", ssh, *sources, f"{ctx.host.target}:{dest}/"], 1800
    )
    return done if not done.ok else _owner_links_apply(ctx, ssh, dest)


def _owner_links_apply(ctx: Ctx, ssh: str, dest: str) -> CmdResult:
    """Tests need the retail XBE and XDK surface inside exec trees: store them under the data
    root, `verify_script` links them (`tools.wt.owner_links`)."""
    store = owner_links.store_dir(str(ctx.host.registry.get("data_root") or DEFAULT_DATA_ROOT))
    out = CmdResult(0, "", "")
    for rel, path in owner_links.sources(REPO_ROOT):
        made = ctx.remote(f"mkdir -p ~/{shlex.quote(store + '/' + str(Path(rel).parent))}")
        if not made.ok:
            return made
        out = ctx.runner(
            ["rsync", "-a", "-e", ssh, str(path), f"{ctx.host.target}:{store}/{rel}"], 600
        )
        if not out.ok:
            return out
    return out


def _toolchain_check(ctx: Ctx) -> CmdResult:
    return ctx.remote(toolchain_check_command(expected_toolchain()))


def _toolchain_apply(ctx: Ctx) -> CmdResult:
    expected = expected_toolchain()
    cmake = re.search(r"\d+(?:\.\d+)+", expected["cmake"])
    python = re.search(r"\d+(?:\.\d+)+", expected["python3"])
    clang = re.search(r"clang version (\d+)", expected["cc"])
    if not (cmake and python and clang):
        return CmdResult(1, "", "expected_toolchain.json has unparseable versions")
    script = (
        PARITY_APPLY.replace("@CMAKE@", cmake.group(0))
        .replace("@PYTHON@", python.group(0))
        .replace("@CLANGMAJOR@", clang.group(1))
        .replace("@CC@", shlex.quote(expected["cc"]))
    )
    return ctx.remote(script, 1500)


BUILD_IMAGE_DIR = ".cache/tsfp-build-image"
BUILD_IMAGE_PREPARE = (
    "set -e; if ! command -v docker >/dev/null 2>&1; then "
    "sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -q docker.io; fi; "
    "sudo -n systemctl enable --now docker >/dev/null 2>&1 || true; "
    f"mkdir -p ~/{BUILD_IMAGE_DIR}"
)


def build_image_check_command(expected: dict[str, str], image: dict[str, str]) -> str:
    """Docker present, the pinned image ID present, and the probe INSIDE it equals `expected`."""
    docker = build_container.DOCKER
    name, image_id = shlex.quote(image["name"]), shlex.quote(image["id"])
    probe = build_container.wrap(toolchain_check_command(expected), image=image["id"])
    return (
        f"{docker} --version >/dev/null 2>&1 || {{ echo 'docker missing'; exit 1; }}; "
        f"have=$({docker} image inspect {name} --format '{{{{.Id}}}}' 2>&1); "
        f'[ "$have" = {image_id} ] || '
        f'{{ echo "image {image["name"]}: $have vs {image["id"]}"; exit 1; }}; ' + probe
    )


def _build_image_check(ctx: Ctx) -> CmdResult:
    try:
        command = build_image_check_command(expected_toolchain(), build_container.build_image())
    except ValueError as error:
        return CmdResult(1, "", str(error))
    return ctx.remote(command, 120)


BUILD_IMAGE_ARCHIVE = REPO_ROOT / "tmp" / "build-image" / "image.tar.zst"


def build_image_load_command() -> str:
    """Load a `docker save | zstd` copy of the pinned image (see docs/t-toolchain-parity.md)."""
    return f"zstd -dc ~/{BUILD_IMAGE_DIR}/image.tar.zst | {build_container.DOCKER} load"


def build_image_build_command(image: dict[str, str]) -> str:
    """Build the image from the copied Dockerfile and require the pinned ID (else fail loudly)."""
    docker = build_container.DOCKER
    name, image_id = shlex.quote(image["name"]), shlex.quote(image["id"])
    message = (
        f"built $have, pinned {image['id']}: rebuilds are not bit-identical, "
        "docker save | docker load the pinned image from a host that has it "
        "(docs/t-toolchain-parity.md)"
    )
    return (
        f"cd ~/{BUILD_IMAGE_DIR} && {docker} build -q -t {name} . >/dev/null && "
        f"have=$({docker} image inspect {name} --format '{{{{.Id}}}}') && "
        f'[ "$have" = {image_id} ] || {{ echo "{message}"; exit 1; }}'
    )


def _build_image_apply(ctx: Ctx) -> CmdResult:
    try:
        image = build_container.build_image()
    except ValueError as error:
        return CmdResult(1, "", str(error))
    prepared = ctx.remote(BUILD_IMAGE_PREPARE, 600)
    if not prepared.ok:
        return prepared
    have = ctx.remote(
        f"{build_container.DOCKER} image inspect {shlex.quote(image['name'])} "
        "--format '{{.Id}}'",
        60,
    )
    if have.ok and have.stdout.strip() == image["id"]:
        return have
    # A rebuild is not bit-identical (apt moves), so a saved copy of the pinned image wins.
    if BUILD_IMAGE_ARCHIVE.is_file():
        copied = ctx.copy([BUILD_IMAGE_ARCHIVE], f"{BUILD_IMAGE_DIR}/image.tar.zst", 1500)
        if not copied.ok:
            return copied
        loaded = ctx.remote(build_image_load_command(), 900)
        if loaded.ok:
            return loaded
    copied = ctx.copy([build_container.DOCKERFILE], f"{BUILD_IMAGE_DIR}/Dockerfile")
    if not copied.ok:
        return copied
    return ctx.remote(build_image_build_command(image), 1500)


STEPS: tuple[Step, ...] = (
    Step("reachable", "ssh true; failure aborts the rest", REACHABLE_CHECK, check_timeout=30),
    Step("sudo", "passwordless sudo (cannot self-fix)", SUDO_CHECK, apply_fn=_sudo_apply),
    Step(
        "persistent-state-layout",
        "csup and rea-codex2 live on the persistent volume",
        LAYOUT_CHECK_STATE_ROOT,
        LAYOUT_APPLY_STATE_ROOT,
    ),
    Step(
        "toolchain",
        "git uv node npm codex and the pinned rust",
        TOOLCHAIN_CHECK,
        TOOLCHAIN_APPLY,
        apply_timeout=900,
    ),
    Step(
        "uv-version",
        f"uv {UV_VERSION} supports the pinned Python release",
        UV_VERSION_CHECK,
        UV_VERSION_APPLY,
        apply_timeout=300,
    ),
    Step("codex-symlink", "/usr/local/bin/codex symlink", CODEX_SYMLINK_CHECK, CODEX_SYMLINK_APPLY),
    Step(
        "codex-login",
        "~/.codex/auth.json copied (scp only)",
        CODEX_LOGIN_CHECK,
        apply_fn=_codex_login_apply,
        local_prereq=_codex_login_prereq,
    ),
    Step(
        "git-config",
        "autoDetach off, origin restored, deploy key",
        GIT_CONFIG_CHECK,
        GIT_CONFIG_APPLY,
        apply_fn=_git_config_apply,
    ),
    Step(
        "gitshim",
        "worker git guard equals tools/fleet/remote/bin/git",
        check_fn=_gitshim_check,
        apply_fn=_gitshim_apply,
    ),
    Step(
        "repo-full-history",
        "full-history repo checkout",
        REPO_CHECK,
        REPO_APPLY,
        check_timeout=60,
        apply_timeout=1800,
        applies=_host_keeps_git,
    ),
    Step(
        "git-storage",
        "blobless origin fetches and scheduled Git maintenance",
        GIT_STORAGE_CHECK,
        GIT_STORAGE_APPLY,
        check_timeout=30,
        apply_timeout=900,
        applies=_host_keeps_git,
    ),
    Step(
        "python-pin",
        "uv python and repo venv match .python-version",
        PYTHON_CHECK,
        PYTHON_APPLY,
        disruptive=True,
        apply_timeout=1800,
        applies=_host_keeps_git,
    ),
    Step(
        "exec-layout",
        "registry host_venv, toolchain_dir, replay_cache_dir set and their dirs exist, no git "
        "repo (rule 47, cx-execlayout)",
        check_fn=exec_layout_check,
        apply_fn=_exec_layout_apply,
        applies=lambda ctx: not ctx.host.is_local,
        check_timeout=30,
    ),
    Step(
        "python-deps",
        "exec venv (and the legacy repo venv) install the uv.lock requirements "
        "(stamp ~/csup/python-deps.sha256)",
        check_fn=python_deps_check,
        apply_fn=_python_deps_apply,
        applies=lambda ctx: not ctx.host.is_local,
        check_timeout=90,
        apply_timeout=1300,
    ),
    Step(
        "media-build-deps",
        "FFmpeg development metadata for native movie decoder builds (no package upgrades)",
        MEDIA_BUILD_CHECK,
        MEDIA_BUILD_APPLY,
        applies=lambda ctx: not ctx.host.is_local,
        check_timeout=30,
        apply_timeout=900,
    ),
    Step(
        "sccache-config",
        "local disk cache config and the shared store route",
        check_fn=_sccache_check,
        apply_fn=_sccache_apply,
        check_timeout=180,
    ),
    Step(
        "build-image",
        "Debian 13 build container: docker, the pinned image, and the toolchain inside it equal "
        "tools/data/expected_toolchain.json (build_container hosts)",
        check_fn=_build_image_check,
        apply_fn=_build_image_apply,
        applies=lambda ctx: ctx.host.build_container,
        check_timeout=150,
        apply_timeout=2400,
    ),
    Step(
        "owner-inputs",
        "hosts flagged owner_files hold the disc, recordings, routes and saves (restored if lost)",
        check_fn=_owner_inputs_check,
        apply_fn=_owner_inputs_apply,
        applies=lambda ctx: not ctx.host.is_local and _owner_input_host(ctx),
        check_timeout=60,
        apply_timeout=1800,
    ),
    Step(
        "build-tools",
        "every tool tools/dev/build.sh requires is on PATH (remote prebuild runs build.sh)",
        check_fn=_build_tools_check,
        apply_fn=_build_tools_apply,
        applies=lambda ctx: not ctx.host.is_local and not ctx.host.build_container,
    ),
    Step(
        "toolchain-parity",
        "cc, cmake, python3 versions equal tools/data/expected_toolchain.json (builder hosts)",
        check_fn=_toolchain_check,
        apply_fn=_toolchain_apply,
        # a build_container host builds inside the image (step build-image), not on its own PATH
        applies=lambda ctx: ctx.host.toolchain_parity and not ctx.host.build_container,
        apply_timeout=1500,
    ),
    Step(
        "payload",
        "run scripts, guards, rg",
        check_fn=_payload_check,
        apply_fn=_payload_apply,
        apply_timeout=600,
    ),
    Step(
        "context-mode",
        "~/.local/bin/context-mode",
        CONTEXT_MODE_CHECK,
        CONTEXT_MODE_APPLY,
        apply_timeout=400,
    ),
    Step("memguard", "a process holds the memguard flock", MEMGUARD_CHECK, MEMGUARD_APPLY),
    Step(
        "capguard",
        "~/csup/capguard.sh, its shipped scripts and the registry-rendered capguard.env match",
        check_fn=_capguard_check,
        apply_fn=_capguard_apply,
    ),
    Step(
        "capguard-timer",
        "host-local capguard cron runs every 15 minutes",
        CAPGUARD_CRON_CHECK,
        CAPGUARD_CRON_APPLY,
        applies=lambda ctx: ctx.host.capguard is not None,
    ),
    Step(
        "lclean",
        "~/csup/lclean.sh matches its tracked source hash",
        check_fn=_guard_check("lclean.sh"),
        apply_fn=_guard_apply("lclean.sh"),
        applies=lambda ctx: ctx.host.lclean,
    ),
    Step(
        "no-lazy-fetch",
        "host scripts export GIT_NO_LAZY_FETCH=1 and a missing object fails fast (T3678)",
        NO_LAZY_CHECK,
        apply_fn=_no_lazy_apply,
        check_timeout=30,
    ),
    Step(
        "codex-runner",
        "csup/codexapi run.sh, rgrc, lane_gate.sh equal the tools/fleet/runner render (T3708)",
        check_fn=runner_check,
        apply_fn=_runner_apply,
        applies=lambda ctx: not ctx.host.is_local,
    ),
    Step(
        "low-priority",
        "nice 19 and ionice idle (nice_all hosts)",
        LOWPRI_CHECK,
        LOWPRI_APPLY,
        applies=lambda ctx: ctx.host.nice_all,
    ),
    Step(
        "server-cargo-admission",
        "direct cargo builds on the server acquire the T3631 build slot",
        SERVER_CARGO_ADMISSION_CHECK,
        SERVER_CARGO_ADMISSION_APPLY,
        applies=lambda ctx: ctx.host.name == "server",
    ),
    Step(
        "tunnel-note",
        "tunnel hosts: reachability note only",
        TUNNEL_NOTE,
        applies=lambda ctx: bool(ctx.host.tunnel),
    ),
)


def describe_steps() -> list[str]:
    return [f"{s.name:<24} {'[disruptive] ' if s.disruptive else ''}{s.doc}" for s in STEPS]


def _make_ctx(
    config: FleetConfig,
    host: HostConfig,
    runner: Runner,
    apply: bool,
    allow_disruptive: bool,
    scp: Runner | None,
    home: Path | None,
) -> Ctx:
    env = host.registry.get("env", {})
    rust = env.get("RUSTUP_TOOLCHAIN") or config.provision.get("rust_toolchain", "stable")
    ctx = Ctx(config, host, runner, scp or runner, apply, allow_disruptive, home or Path.home())
    ctx.subst = {
        "P": STATE_ROOT,
        "RUST": rust,
        "URL": quote(config.provision.get("repo_url", "")),
        "SSHGIT": SSH_GIT,
        "PYTHON": str(config.provision.get("python", "3")),
        "SCDIR": _sccache_dir(ctx),
        "SCSIZE": str(SCCACHE_SIZE_BYTES),
        "MGARGS": " ".join(quote(a) for a in shlex.split(host.memguard)),
        "TUNNEL": host.tunnel or "",
    }
    ctx.prefix = (
        PATH_PREFIX
        + f'export RUSTUP_TOOLCHAIN="{rust}"; '
        + "".join(f'export {k}="{v}"; ' for k, v in env.items())
    )
    return ctx


def _check(step: Step, ctx: Ctx) -> CmdResult:
    if step.check_fn:
        return step.check_fn(ctx)
    command = step.check_cmd
    if step.name == "persistent-state-layout" and not _state_root(ctx):
        command = LAYOUT_CHECK_HOME
    return ctx.remote(ctx.render(command or "true"), step.check_timeout)


def _apply(step: Step, ctx: Ctx) -> CmdResult:
    if step.local_prereq and (problem := step.local_prereq(ctx)):
        return CmdResult(1, "", problem)
    if step.apply_fn:
        return step.apply_fn(ctx)
    command = step.apply_cmd
    if step.name == "persistent-state-layout" and not _state_root(ctx):
        command = LAYOUT_APPLY_HOME
    if not command:
        return CmdResult(1, "", "cannot be fixed automatically")
    return ctx.remote(ctx.render(command), step.apply_timeout)


def provision(
    config: FleetConfig,
    host: str | HostConfig,
    runner: Runner,
    apply: bool,
    only: Sequence[str] | None = None,
    scp: Runner | None = None,
    allow_disruptive: bool = False,
    home: Path | None = None,
) -> list[StepResult]:
    """Check (and with `apply`, fix) every step on `host`. Idempotent; checks are read-only."""
    host_config = config.host(host) if isinstance(host, str) else host
    names = {s.name for s in STEPS}
    unknown = [n for n in (only or []) if n not in names]
    if unknown:
        raise ValueError(f"unknown step(s) {unknown}; known: {sorted(names)}")
    ctx = _make_ctx(config, host_config, runner, apply, allow_disruptive, scp, home)
    results: list[StepResult] = []
    aborted = False
    for step in STEPS:
        if only and step.name not in only:
            continue
        if step.applies and not step.applies(ctx):
            continue
        if aborted:
            results.append(StepResult(step.name, "skipped", "host unreachable"))
            continue
        result = _run_step(step, ctx)
        results.append(result)
        if step.name == "reachable" and result.state != "ok":
            aborted = True
    return results


def _run_step(step: Step, ctx: Ctx) -> StepResult:
    first = _check(step, ctx)
    if first.ok:
        return StepResult(
            step.name, "ok", first.out.strip().splitlines()[-1] if first.out.strip() else ""
        )
    drift = _tail(first)
    if not ctx.apply:
        return StepResult(step.name, "would-fix", drift)
    if step.disruptive and not ctx.allow_disruptive:
        return StepResult(step.name, "skipped", f"{DISRUPTIVE_NOTE} (drift: {drift})")
    applied = _apply(step, ctx)
    if not applied.ok:
        return StepResult(step.name, "failed", _tail(applied))
    again = _check(step, ctx)
    if again.ok:
        return StepResult(step.name, "fixed", drift)
    return StepResult(step.name, "failed", f"re-check failed: {_tail(again)}")


# --- CLI glue (wired into tools/fleet/__main__.py by the orchestrator) ---
def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("host", nargs="?", help="fleet host name (see tools/data/fleet.json)")
    parser.add_argument("--apply", action="store_true", help="fix drift (default: checks only)")
    parser.add_argument("--only", help="comma-separated step names")
    parser.add_argument("--list", action="store_true", help="list the steps and exit")
    parser.add_argument(
        "--allow-disruptive",
        action="store_true",
        help="also run steps that disturb running workers",
    )


def run_cli(args: argparse.Namespace, config: FleetConfig, runner: Runner | None = None) -> int:
    if args.list:
        print("\n".join(describe_steps()))
        return 0
    if not args.host:
        print("provision: host required (or --list)")
        return 2
    try:
        host = config.host(args.host)
        if host.is_local:
            print("provision: the local host is not provisioned over ssh")
            return 2
        only = [n.strip() for n in args.only.split(",") if n.strip()] if args.only else None
        results = provision(
            config,
            host,
            runner or local_runner(),
            args.apply,
            only,
            allow_disruptive=args.allow_disruptive,
        )
    except (KeyError, ValueError) as error:
        print(f"provision: {error}")
        return 2
    mode = "apply" if args.apply else "dry-run"
    print(f"provision {host.name} ({mode})")
    for result in results:
        print(result.line())
    return 1 if any(r.state == "failed" for r in results) else 0
