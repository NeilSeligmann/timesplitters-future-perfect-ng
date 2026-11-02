"""T3459: run heavy verify commands on a remote host when the local build slots are full.

Local merge verifies and worker checks hold one host-wide build slot (tools.wt.buildslots).
When every slot is taken they used to wait, sometimes for most of an hour. With the fallback
on, the cargo commands of such a verify run instead on the least-loaded eligible remote host
(tools/data/remote_hosts.json, capability `verify`: server, deck, droplet, and the laptop only
while `tmp/codexapi/laptop.off` is absent) through `tools.wt.remote_exec.run_verify`:

* exact tree: the committed candidate tree is archived (`git archive` of the tree hash), copied
  and SHA-256 checked on the host; a dirty tracked tree never goes remote;
* host-wide slot: the remote run takes `~/.cache/tsfp-remote-exec/slot.lock` non-blocking, the
  same lock remote_prebuilt builds take, so a busy host is skipped, never queued on;
* honest verdicts: exit code and log tail come back. Anything infrastructural (ssh, refused
  probe, busy slot, staging, `cargo fetch`, missing tool or toolchain) is not a verdict: the
  caller falls back to waiting for a local slot. A remote run never reports a pass unless every
  command ran and exited 0.

Only commands that are portable run remotely: a cargo build/check/test/clippy with relative
paths only (no absolute path, no python tool). The rest stay local. T3461: the rust profile's
`canon-run` wrapper, `nice` and the host-local CARGO_HOME/CARGO_BUILD_JOBS are unwrapped, so
profile cargo steps are portable too; measured headroom orders hosts (VERIFY_ORDER breaks ties);
`admit` is the one admission entry point for run_admitted and a standalone canon-run, and every
decision is logged to tmp/verify-admission.jsonl (`python3 -m tools.wt.remote_fallback --stats`).

Switch TSFP_REMOTE_VERIFY_FALLBACK: unset or `first` = remote first (owner 2026-10-21, the local
host is the owner's PC), `1` = local first but at most LOCAL_WAIT_S (60 s) of slot wait, then
remote (T3461), `0` = off.
"""

from __future__ import annotations

import fcntl
import os
import re
import shlex
import signal
import subprocess
import tarfile
import tempfile
import threading
import time
from collections.abc import Callable, Collection, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path

from tools.fleet.headroom import score as headroom_score
from tools.wt import buildslots, cdiff_bundle, host_budget, host_health, remote_exec, verify_route

ENV = "TSFP_REMOTE_VERIFY_FALLBACK"
DEFAULT_ON = True
CARGO = re.compile(
    r"(?:^|[\s;&|(])cargo\s+(?:\+\S+\s+)?(?:build|b|check|c|test|t|clippy|bench|doc)\b"
)
WT_RUN = re.compile(r"^(?:python3?\s+-m\s+tools\.wt\s+run\s+--\s+)")
# T3461: the rust profile wraps cargo as `[CARGO_HOME=..] [CARGO_BUILD_JOBS=N] nice -n 15
# python3 -m tools.agents.verify_profiles canon-run -- cargo ...`; the wrapper only picks the
# local canonical tree, CARGO_HOME and the job count are host-local. Unwrapped, the command is
# the same cargo step, so it may run on a verify host (it waited 1763 s for a local slot).
CANON_RUN = re.compile(r"python3?\s+-m\s+tools\.agents\.verify_profiles\s+canon-run\s+--\s+")
CARGO_HOME = re.compile(r"(?:^|(?<=\s))CARGO_HOME=\S+\s+")
# T3461 (coordinator): verify host preference, used only for equal headroom since T3878.
# T3582: the server shares its machine with the owner's Forgejo and Postgres (host_health max 1
# build), so it is the last resort; the laptop is the owner's but idle most of the day.
VERIFY_ORDER = ("droplet", "droplet2", "laptop", "deck", "server")
LOCAL_WAIT_S = 60.0  # fallback mode: wait this long for a local slot before going remote
REMOTE_QUEUE_S = 60.0  # under merge lock or --urgent, keep the queue bounded (T3582)
REMOTE_POLL_S = 10.0
# T3732: set by a terminating signal (merge_task SIGTERM/SIGINT). The host loop runs in a
# worker thread that the signal's SystemExit never reaches: it kept spawning rsync/ssh to the
# next host for minutes after SIGTERM (5 of 10 merge_task groups, 2026-10-24/25).
STOP = threading.Event()
REMOTE_CHILDREN = frozenset({"ssh", "rsync", "scp"})
# T3582: an unlocked candidate (pre-lock verify, a worker check) waits this long for a remote
# slot before it takes the owner's PC; under the merge lock or --urgent the short wait stays.
UNLOCKED_QUEUE_S = 300.0
QUEUE_ENV = "TSFP_REMOTE_QUEUE_S"
STATS_REL = Path("tmp/verify-admission.jsonl")
# T3782 (I-T3782-2): admission-log reasons of local runs that are local by design, not because
# remote-first failed: T3760 cost routing keeps Python test sets local (`route_cost`). They are
# left out of `remote_eligible` and its local share (HEALTH verify-local-share).
REASON_COST = "cost"
REASON_NOT_PORTABLE = "not_portable"
LOCAL_BY_DESIGN = frozenset({REASON_COST})
NOT_PORTABLE = re.compile(r"(?:^|[\s=:'\"])(?:/|~)|\bpython|\btools\.|\$\(|`|\.\.")
MANIFEST = re.compile(r"--manifest-path[ =](\S+)")
# T3472: the private lift (w4_lifted and similar features, TSFP_LIFTED_DIR) exists only on the
# main host; remotely such tests select nothing and would "pass" with zero tests.
LOCAL_ONLY = re.compile(r"lifted|TSFP_LIFTED", re.I)
# T3501: a c-diff command carrying a lifted-bundle id is portable; run_verify ships the bundle.
CDIFF_BUNDLE = re.compile(r"\bTSFP_CDIFF_LIFT=[0-9a-f]{32}-[0-9a-f]{16}\b")
TESTS_PASSED = re.compile(r"^test result: ok\. (\d+) passed", re.M)
# T3798: the remote script counts every `test result:` line of a command's FULL log (the log
# section it prints is only the last `tail_lines`, which cut the passing line of crates with
# many test binaries and read as zero tests).
PASSED_MARK = re.compile(r"^@@PASSED \d+ (\d+)$", re.M)
CARGO_TEST = re.compile(r"(?:^|[\s;&|(])cargo\s+(?:\+\S+\s+)?(?:test|t)\b")
NO_RUN = re.compile(r"(?:^|\s)--no-run(?:\s|$)")
LIST_ONLY = re.compile(r"(?:^|\s)--list(?:\s|$)")

PYTEST_MODULES = frozenset(("pytest", "tools.agents.pytest_retry"))


Log = Callable[[str], object]


@dataclass
class Outcome:
    status: str  # pass | fail | infra
    host: str | None = None
    failed: str | None = None
    returncode: int = 0
    output: str = ""
    seconds: float = 0.0
    notes: list[str] = field(default_factory=list)
    health: list[str] = field(default_factory=list)
    """T3516: "<host>:<reason>" of every host-health refusal met on the way."""
    passed: list[str] = field(default_factory=list)
    """T3798: caller commands that fully passed on a host whose run then ended without a
    verdict (stopped, zero tests in a later step, infrastructure exit): only the rest is
    verified locally."""


def record_notes(notes: Sequence[str], limit: int = 12) -> list[str]:
    """Admission-log form of `notes`: the LAST note per host (or bare note), in first-seen order.

    claude-w1: `notes[:4]` kept only the first re-probes (four `server:host_health_backoff`
    lines) and hid the host that actually held a 300 s queue wait. Repeats collapse to
    `<note> (xN)`."""
    last: dict[str, str] = {}
    count: dict[str, int] = {}
    for note in notes:
        key = note.split(":", 1)[0] if ":" in note else note
        last[key] = note
        count[key] = count.get(key, 0) + 1
    return [
        last[key] + (f" (x{count[key]})" if count[key] > 1 else "") for key in list(last)[:limit]
    ]


def _mode(env: Mapping[str, str] | None) -> str:
    value = (os.environ if env is None else env).get(ENV, "").strip().lower()
    if not value:
        return "first" if DEFAULT_ON else "off"
    if value in ("0", "off", "no", "false"):
        return "off"
    return "first" if value == "first" else "fallback"


def enabled(env: Mapping[str, str] | None = None) -> bool:
    """Remote verify allowed at all (`first` or `fallback`)."""
    return _mode(env) != "off"


def remote_first(env: Mapping[str, str] | None = None) -> bool:
    """Owner 2026-10-21: the local host is the owner's PC, so portable builds go to the remote
    slots first by default and only run locally when no remote host gives a verdict.
    TSFP_REMOTE_VERIFY_FALLBACK=1 keeps the older fallback-only policy (remote only when local
    slots are full), 0 turns remote verify off."""
    return _mode(env) == "first"


def _legacy_form(command: str) -> str | None:
    """A single cargo step with at most one leading wrapper, as remote hosts run it."""
    stripped = CARGO_HOME.sub("", CANON_RUN.sub("", WT_RUN.sub("", command.strip())))
    local_only = LOCAL_ONLY.search(stripped) and not CDIFF_BUNDLE.search(stripped)
    if not CARGO.search(stripped) or NOT_PORTABLE.search(stripped) or local_only:
        return None
    return stripped


PLACEHOLDER = cdiff_bundle.Bundle("0" * 32 + "-" + "0" * 16, Path("."), False, 0.0)


def _placeholder(segment: verify_route.Segment, env: Mapping[str, str]) -> str:
    """The shape a lifted c-diff step takes remotely, for classification only."""
    return cdiff_bundle.remote_command(segment.tokens, PLACEHOLDER, segment.lift_env(env))


def _portable_manifest(command: str, root: Path | None) -> str:
    """Rewrite an absolute manifest under this checkout to its remote-relative path."""
    if root is None:
        return command
    words = verify_route.tokenize(command)
    if not words:
        return command
    checkout = root.resolve()
    changed = False
    for index, word in enumerate(words):
        value = word.partition("=")[2] if word.startswith("--manifest-path=") else None
        path_index = index if value is not None else index + 1 if word == "--manifest-path" else -1
        if path_index < 0 or path_index >= len(words):
            continue
        raw = value if value is not None else words[path_index]
        path = Path(raw)
        if not path.is_absolute():
            continue
        try:
            relative = path.resolve().relative_to(checkout).as_posix()
        except (OSError, ValueError):
            continue
        words[path_index] = f"--manifest-path={relative}" if value is not None else relative
        changed = True
    return (
        " ".join(word if word == "&&" else shlex.quote(word) for word in words)
        if changed
        else command
    )


def remote_form(
    command: str, env: Mapping[str, str] | None = None, *, root: Path | None = None
) -> str | None:
    """The command as it runs on a remote host, or None when it is not portable.

    T3582: a chain of cargo steps (`tools.wt run -- cargo ... && export X=.. && ...`) is portable
    when every step is (tools.wt.verify_route); the steps are joined with `&&`. With `env`
    (the process environment of the caller) a c-diff test that reads the private lift counts as
    portable too: `remote_forms` ships it as a lifted bundle."""
    forms = _classify(_portable_manifest(command, root), env)
    return " && ".join(forms) if forms else None


def _pytest_form(command: str) -> str | None:
    """Normalize a local venv pytest invocation to the host's project Python command."""
    words = verify_route.tokenize(command)
    if not words or len(words) < 4:
        return None
    try:
        module_at = words.index("-m")
    except ValueError:
        return None
    if module_at + 1 >= len(words) or words[module_at + 1] not in PYTEST_MODULES:
        return None
    args = words[module_at + 2 :]
    if not args or any(arg.startswith(("/", "~")) or ".." in Path(arg).parts for arg in args):
        return None
    if names_git_repo_test(args):
        return None  # T3782: runs git on the checkout; exec trees have no .git (rule 47)
    return shlex.join(["python3", "-m", words[module_at + 1], *args])


def names_git_repo_test(args: Sequence[str], root: Path | None = None) -> bool:
    """A pytest argument names (or globs over) a test routed local by `tools.agents.git_tests`.

    T3782: a test that runs git against the checkout fails on an execution-only host (rule 47,
    tests/test_datadir.py on the deck). The routed list is tools/data/git_repo_tests.json."""
    import fnmatch

    from tools.agents import git_tests

    routed = git_tests.routed(root or Path(__file__).resolve().parents[2])
    for arg in args:
        if arg.startswith("-"):
            continue
        path = arg.split("::", 1)[0]
        if path in routed or any(fnmatch.fnmatch(test, path) for test in routed):
            return True
        if path.rstrip("/") in ("tests", "."):
            return True  # the whole suite includes them
    return False


NO_STD_PLACEHOLDER = "tsfp-no-std-archive-check"
"""What `remote_form` reports for the no_std archive check: the shape only, the script is
rendered per tree by `remote_forms` (it lists that tree's family crates)."""


def is_no_std(command: str) -> bool:
    """T3585: `<python> -m tools.agents.no_std_archive`, alone or behind host-local wrappers."""
    from tools.agents import no_std_archive

    segments = verify_route.parse(command)
    return bool(segments) and len(segments) == 1 and no_std_archive.is_command(segments[0].tokens)


def _classify(command: str, env: Mapping[str, str] | None) -> list[str] | None:
    process_env = {} if env is None else env
    words = verify_route.tokenize(command)
    if words is None:
        # A pipe, `;`, `||`, redirect or substitution: the host's `bash -c` has no pipefail and
        # no `-e`, so a failure could be masked or a second command run. Always local (T3582).
        return None
    if is_no_std(command):
        return [NO_STD_PLACEHOLDER]  # T3585: a python tool, but its rustc steps render portably
    pytest_form = _pytest_form(command)
    if pytest_form:
        return [pytest_form]
    segments = verify_route.parse(command)
    if segments is not None and any(seg.needs_bundle(process_env) for seg in segments):
        return verify_route.chain_forms(
            command,
            process_env=process_env,
            bundler=lambda seg: _placeholder(seg, process_env),
            check=lambda text: _legacy_form(text) is not None,
        )
    single = _legacy_form(command) if "&&" not in words else None
    if single is not None:
        return [single]
    return verify_route.chain_forms(
        command, process_env=process_env, check=lambda text: _legacy_form(text) is not None
    )


def split_mixed(
    command: str, env: Mapping[str, str] | None = None, *, root: Path | None = None
) -> list[str] | None:
    """Split a mixed `&&` chain into (cargo steps for a verify host, the rest for here) (T3582).

    Worker `--verify` strings mix portable cargo steps with host-local ones (`python3 -m
    tools.agents.roadmap --check`, a piped `--list | rg`, `tools.wt build`). Wholly local they
    compiled on the owner's PC. The checks are independent, so the cargo steps run remotely and
    the remainder locally, concurrently, and the candidate needs every part to pass (what `&&`
    requires). `export` steps ride with both parts. Returns [remote, local] as command strings
    or None when the chain is wholly portable, wholly local, or not a pure `&&` list."""
    steps = verify_route.split_steps(_portable_manifest(command, root))
    if steps is None or len(steps) < 2:
        return None
    exports: list[str] = []
    remote_steps: list[str] = []
    local_steps: list[str] = []
    for step in steps:
        if verify_route.is_export(step):
            exports.append(step)
            continue
        if _classify(" && ".join([*exports, step]), env):
            remote_steps.append(step)
        else:
            local_steps.append(step)
    if not remote_steps or not local_steps:
        return None
    return [" && ".join([*exports, *remote_steps]), " && ".join([*exports, *local_steps])]


def without_cargo(command: str, env: Mapping[str, str] | None = None) -> str:
    """`command` minus its portable cargo steps (T3582): "" when nothing else remains, the
    non-cargo remainder of a mixed `&&` chain, or `command` itself when it cannot be split."""
    if remote_form(command, env):
        return ""
    parts = split_mixed(command, env)
    return parts[1] if parts else command


def remote_forms(
    command: str,
    *,
    root: Path | None = None,
    main: Path | None = None,
    bundle_root: Path | None = None,
    env: Mapping[str, str] | None = None,
    log: Log | None = None,
) -> list[str] | None:
    """The remote commands of `command`, one per cargo step (T3582), or None (stay local).

    A c-diff test that reads the private lift gets its lifted bundle here (T3501 follow-up:
    `cdiff_bundle.ensure` builds it once per inputs key on this host, `remote_exec.run_verify` ships
    it); a bundle that cannot be built keeps the step local, since the host would run the
    test without the lifted reference."""
    process_env = os.environ if env is None else env
    if is_no_std(command):
        if root is None:
            return None
        from tools.agents import no_std_archive

        try:
            return [no_std_archive.remote_script(root)]
        except (OSError, ValueError, no_std_archive.BuildError) as error:
            if log:
                log(f"verify: no_std remote script not rendered ({str(error)[:200]}), stays local")
            return None
    command = _portable_manifest(command, root)
    segments = verify_route.parse(command)
    if segments is None or not any(seg.needs_bundle(process_env) for seg in segments):
        forms = _classify(command, process_env)
        return forms
    if root is None or main is None:
        return None

    def bundler(segment: verify_route.Segment) -> str:
        step_env = segment.lift_env(process_env)
        source_root = bundle_root or root
        if source_root != root:
            lift = cdiff_bundle.lift_dir(root, process_env)
            if lift is not None:
                step_env["TSFP_LIFTED_DIR"] = str(lift)
        bundle = cdiff_bundle.ensure(source_root, main, step_env, log=log)
        return cdiff_bundle.remote_command(segment.tokens, bundle, step_env)

    try:
        return verify_route.chain_forms(
            command,
            process_env=process_env,
            bundler=bundler,
            check=lambda text: _legacy_form(text) is not None,
        )
    except (cdiff_bundle.BundleError, OSError, subprocess.SubprocessError) as error:
        if log:
            log(f"verify: no c-diff lifted bundle ({str(error)[:200]}), step stays local")
        return None


def tests_ran(output: str) -> int:
    """Tests that passed in a log section: the remote `@@PASSED` count of the whole command
    log when present (T3798), else cargo's `test result:` lines in the section."""
    marked = PASSED_MARK.findall(output)
    if marked:
        return sum(int(count) for count in marked)
    return sum(int(count) for count in TESTS_PASSED.findall(output))


def expects_tests(form: str) -> bool:
    """True for a cargo test step that must run at least one test to be a verdict.

    T3798: `cargo test --no-run` (T3660 integration-test build check) only compiles; it never
    prints a passing `test result:` line, so every such step read as zero_tests_remote and the
    whole remote job was verified again on the owner PC. Arguments after a bare `--` belong to
    the test binaries, not cargo; `-- --list` (the harness lists tests and runs none, 13 wt-build
    jobs in the first hour after the fix) is no test run either."""
    if not CARGO_TEST.search(form):
        return False
    parts = re.split(r"\s--(?:\s|$)", form, maxsplit=1)
    cargo_args, harness_args = parts[0], parts[1] if len(parts) > 1 else ""
    return not NO_RUN.search(cargo_args) and not LIST_ONLY.search(harness_args)


def fetch_manifests(commands: Sequence[str]) -> list[str]:
    """Manifests whose dependencies the host fetches before `--offline` commands run."""
    if not any(CARGO.search(command) for command in commands):
        return []  # T3585: a rustc-only step (no_std archive) needs no cargo fetch
    manifests = ["Cargo.toml"]
    for command in commands:
        for match in MANIFEST.finditer(command):
            manifests.append(shlex.split(match.group(1))[0])
    return list(dict.fromkeys(manifests))


def queue_wait(*, locked: bool, urgent: bool, env: Mapping[str, str] | None = None) -> float:
    """Seconds to wait for a free remote verify slot before building locally (T3582)."""
    env = os.environ if env is None else env
    try:
        return max(0.0, float(env[QUEUE_ENV]))
    except (KeyError, ValueError):
        pass
    return REMOTE_QUEUE_S if locked or urgent else UNLOCKED_QUEUE_S


def _admission_off(env: Mapping[str, str]) -> bool:
    return env.get("TSFP_BUILD_ADMISSION", "").lower() in ("off", "0", "no") or bool(
        env.get("TSFP_BUILD_SLOT_HELD")
    )


def local_slots_full(main: Path, env: Mapping[str, str] | None = None) -> bool:
    """True when buildslots.acquire (shared pool) would wait right now (probe only)."""
    env = os.environ if env is None else env
    if _admission_off(env):
        return False
    directory = main / buildslots.SLOTS_REL
    if not directory.is_dir():
        return False
    count = buildslots.slot_count(buildslots.mem_total_mb(), directory)
    held = buildslots.held_slots(directory, count)
    if held >= count:
        return True
    available = buildslots.mem_available_mb()
    need = buildslots.env_mb("TSFP_BUILD_MEM_RESERVE_MB", buildslots.MEM_RESERVE_MB)
    need += buildslots.env_mb("TSFP_BUILD_RSS_MB", buildslots.BUILD_RSS_MB)
    return bool(held) and available is not None and available < need


def wait_local_slot(
    main: Path,
    *,
    merge: bool,
    env: Mapping[str, str] | None = None,
    wait_s: float | None = None,
    poll_s: float = 1.0,
) -> int | None:
    """A local slot within `wait_s` (default LOCAL_WAIT_S), else None (T3461)."""
    deadline = time.monotonic() + (LOCAL_WAIT_S if wait_s is None else wait_s)
    while True:
        fd = try_local_slot(main, merge=merge, env=env)
        if fd is not None or time.monotonic() >= deadline:
            return fd
        time.sleep(poll_s)


def try_local_slot(main: Path, *, merge: bool, env: Mapping[str, str] | None = None) -> int | None:
    """A local slot fd if one is free now, else None (never waits, logs nothing)."""
    if _admission_off(os.environ if env is None else env):
        return None
    return buildslots.acquire(main, wait_s=0, log=lambda _text: None, merge=merge)


def owner_load_per_core() -> float | None:
    """Current owner-PC load per logical core; None means the load probe failed."""
    try:
        load1 = os.getloadavg()[0]
        cores = os.cpu_count() or 0
        return load1 / cores if cores else None
    except (AttributeError, OSError):
        return None


def owner_load_threshold() -> float:
    from tools.dispatch.hosts import policy

    return policy().owner_max_load_per_core


def owner_verify_capacity(main: Path) -> tuple[bool, str]:
    """Whether another local verify fits the dispatch cap, counting shared build slots.

    merge_task and worker_check already hold buildslots leases while running local cargo work.
    Dispatcher owner-PC leases are included as well; counting both is conservative when a
    dispatched verify also owns a build slot.
    """
    from tools.dispatch import hosts as dispatch_hosts
    from tools.dispatch import queue as dispatch_queue

    cap = dispatch_hosts.policy().owner_max_jobs
    state = dispatch_queue.state_dir()
    active = dispatch_queue.live_entries(state / "active", dispatch_queue.LEASE)
    dispatch_jobs = sum(1 for row in active if row.get("host") == "local")
    directory = main / buildslots.SLOTS_REL
    slots = buildslots.slot_count(buildslots.mem_total_mb(), directory)
    held = buildslots.held_slots(directory, slots) if directory.is_dir() else 0
    return dispatch_jobs + held < cap, f"{dispatch_jobs + held}/{cap} verify jobs"


def try_owner_verify_slot(
    main: Path, *, merge: bool, env: Mapping[str, str]
) -> tuple[int | None, str]:
    """Serialize owner capacity check plus slot acquisition among owner-first verifiers."""
    directory = main / buildslots.SLOTS_REL
    directory.mkdir(parents=True, exist_ok=True)
    with (directory / "owner-verify-admission.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        try:
            capacity, note = owner_verify_capacity(main)
            if not capacity:
                return None, note
            return try_local_slot(main, merge=merge, env=env), note
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def record(main: Path, entry: dict[str, object]) -> None:
    """Append one admission decision to tmp/verify-admission.jsonl (telemetry, never fatal)."""
    import json

    try:
        path = main / STATS_REL
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps({"ts": round(time.time(), 3), **entry}, sort_keys=True) + "\n")
    except OSError:
        pass


def admit(
    main: Path,
    commands: Sequence[str],
    *,
    root: Path,
    merge: bool,
    caller: str,
    env: Mapping[str, str] | None = None,
    log: Log | None = None,
    remote: Callable[..., Outcome] | None = None,
    queue_s: float | None = None,
) -> tuple[int | None, Outcome | None]:
    """T3461 admission for a heavy cargo step: a local slot, or a remote verify verdict.

    `first` (default, owner: the local host is the owner's PC): portable steps go to a verify
    host at once. `fallback` (TSFP_REMOTE_VERIFY_FALLBACK=1): wait at most LOCAL_WAIT_S for a
    local slot (memory below the reserve also waits), then go remote. Returns (slot fd, None)
    to build locally, or (None, outcome) after a remote attempt: "pass" is final; "fail" and
    "infra" mean build locally (a remote failure is confirmed locally, infra falls back).
    Both None: admission does not apply (held by an outer merge_task, or off)."""
    env = os.environ if env is None else env
    say = log or (lambda _text: None)
    if not enabled(env) or _admission_off(env):
        return None, None
    started = time.monotonic()
    portable = [c for c in commands if remote_form(c, env, root=root)]
    urgent = env.get("TSFP_MERGE_URGENT") == "1"  # T3461: urgent goes remote first, never waits
    if queue_s is None:
        queue_s = queue_wait(locked=False, urgent=urgent, env=env)
    if (remote_first(env) or urgent) and len(portable) == len(commands):
        threshold = owner_load_threshold()
        load = owner_load_per_core()
        if load is not None and load < threshold:
            fd, capacity_note = try_owner_verify_slot(main, merge=merge, env=env)
            if fd is not None:
                waited = round(time.monotonic() - started, 1)
                say(f"verify: owner PC admitted at load/core {load:.3f} ({capacity_note})")
                record(
                    main,
                    {
                        "caller": caller,
                        "decision": "local",
                        "local_wait_s": waited,
                        "owner_load_per_core": round(load, 4),
                        "owner_capacity": capacity_note,
                    },
                )
                return fd, None
    if (not remote_first(env) and not urgent) or len(portable) != len(commands):
        if portable:
            fd = wait_local_slot(main, merge=merge, env=env)
        else:
            fd = buildslots.acquire(main, log=say, merge=merge, wait_s=0 if urgent else None)
        waited = round(time.monotonic() - started, 1)
        if fd is not None or len(portable) != len(commands):
            record(main, {"caller": caller, "decision": "local", "local_wait_s": waited})
            return fd, None
        say(f"verify: no local build slot within {LOCAL_WAIT_S:.0f} s, going remote")
    waited = round(time.monotonic() - started, 1)
    outcome = (remote or run_remote)(
        portable, root=root, log=say, main=main, queue_s=queue_s, env=env
    )
    record(
        main,
        {
            "caller": caller,
            "decision": "remote",
            "local_wait_s": waited,
            "remote_status": outcome.status,
            "remote_host": outcome.host,
            "remote_s": outcome.seconds,
            "notes": record_notes(outcome.notes),
            "host_health": outcome.health[:8],
            "no_std": any(is_no_std(c) for c in portable),  # T3585
            # T3798: steps that passed remotely before a no-verdict end are not redone locally
            "kept_passed": len(outcome.passed) if outcome.status == "infra" else 0,
        },
    )
    return None, outcome


def record_health_refusal(
    main: Path | None, host: str, reasons: Sequence[str], *, stage: str
) -> None:
    """T3516: one row per host-health refusal (`stage` probe or run) in the admission log.

    The supervisor's health-check.sh flags a host with more than 20 of them in an hour."""
    if main is None:
        return
    host_health.note_refusal(host, root=main, reasons=reasons)  # T3588: no probe for the back-off
    record(
        main,
        {
            "decision": host_health.DECISION,
            "host": host,
            "code": host_health.HEALTH_EXIT,
            "stage": stage,
            "reasons": list(reasons)[:6],
        },
    )


INFRA_CAUSES = (
    ("stopped", re.compile(r"^stopped_by_signal")),
    ("zero_tests", re.compile(r":zero_tests_remote")),
    ("exec_dir_in_use", re.compile(r":exec_dir_in_use|lease: the dir stayed in use")),
    ("degraded", re.compile(r":degraded\(")),
    ("infra_exit", re.compile(r":infra_exit_")),
    ("host_health", re.compile(r":host_health")),
    ("slot_race", re.compile(r":slot_taken_(?:before|while)_staging|:slot_busy_at_reserve")),
    ("budget_full", re.compile(r":budget_full")),
    ("slot_busy", re.compile(r":slot_busy")),
    ("not_portable", re.compile(r"^not_portable")),
)
"""T3798: no-verdict causes, most specific first; a row counts under its first match."""


DISABLED_NOTE = ":disabled"
"""`remote_exec.ranked_hosts` reason of an owner-disabled host, never a no-verdict reason."""


def budget_admits(host: remote_exec.HostSpec) -> bool:
    """T3837: the host's shared budget fits one more verify (an unbudgeted host admits; a
    budgeted host without `job_costs` raises, guarded by tests/test_host_budget.py)."""
    return remote_exec.budget_admits_verify(host)


def infra_cause(notes: Sequence[str]) -> str:
    """The root-cause class of one no-verdict admission row (`INFRA_CAUSES`, else `other`)."""
    for name, pattern in INFRA_CAUSES:
        if any(pattern.search(note) for note in notes):
            return name
    return "other"


def infra_causes(rows: Sequence[Mapping[str, object]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for row in rows:
        if row.get("remote_status") == "infra":
            notes = row.get("notes")
            cause = infra_cause([str(n) for n in notes] if isinstance(notes, list) else [])
            counts[cause] = counts.get(cause, 0) + 1
    return dict(sorted(counts.items(), key=lambda item: -item[1]))


def stats(main: Path, since: float, *, until: float | None = None) -> dict[str, object]:
    """Local waits versus remote dispatches and remote failures that fell back (T3461)."""
    import json

    path = main / STATS_REL
    rows = []
    if path.is_file():
        with path.open(encoding="utf-8") as source:
            for line in source:
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if not isinstance(row, dict) or not isinstance(row.get("ts"), (int, float)):
                    continue
                stamp = row["ts"]
                if stamp >= since and (until is None or stamp < until):
                    rows.append(row)
    refusals: dict[str, int] = {}
    for row in rows:
        if row.get("decision") == host_health.DECISION:
            refusals[str(row.get("host"))] = refusals.get(str(row.get("host")), 0) + 1
    rows = [r for r in rows if r.get("decision") != host_health.DECISION]
    # T3798: `tools.wt build` logs its local fallback after a remote attempt as a second row; the
    # remote row (status != pass) already counts that compile as local, so one job counted twice.
    followups = [r for r in rows if r.get("after_remote")]
    rows = [r for r in rows if not r.get("after_remote")]
    local = [r for r in rows if r.get("decision") == "local"]
    remote_rows = [r for r in rows if r.get("decision") == "remote"]

    def p50(values: list[float]) -> float | None:
        return sorted(values)[len(values) // 2] if values else None

    hosts: dict[str, int] = {}
    for row in remote_rows:
        if row.get("remote_status") == "pass":
            hosts[str(row.get("remote_host"))] = hosts.get(str(row.get("remote_host")), 0) + 1
    # T3582: where the compile ran. A remote row that gave no pass built on this host after all.
    ran_local = len(local) + sum(1 for r in remote_rows if r.get("remote_status") != "pass")
    # T3782: the share that measures remote-first (cargo builds, replays): by-design local rows
    # (Python test sets under cost routing) are not remote-eligible.
    eligible = [r for r in rows if r.get("reason") not in LOCAL_BY_DESIGN]
    eligible_local = sum(
        1 for r in eligible if r.get("decision") == "local" or r.get("remote_status") != "pass"
    )
    callers: dict[str, dict[str, int]] = {}
    for row in rows:
        entry = callers.setdefault(str(row.get("caller")), {"local": 0, "remote": 0})
        passed = row.get("decision") == "remote" and row.get("remote_status") == "pass"
        entry["remote" if passed else "local"] += 1
    no_std = [r for r in remote_rows if r.get("no_std")]  # T3585: the archive check's own jobs
    return {
        "no_std_archive": {
            "remote_jobs": len(no_std),
            "remote_pass": sum(1 for r in no_std if r.get("remote_status") == "pass"),
            "compiled_locally_after_remote": sum(
                1 for r in no_std if r.get("remote_status") != "pass"
            ),
            "remote_s_p50": p50([float(r.get("remote_s") or 0) for r in no_std]),
            # I-T3797-1: a remote verdict "fail" here is a compile error on the host; when the
            # local re-run then passes, HEALTH no-std-remote-skew names it.
            "remote_fail": sum(1 for r in no_std if r.get("remote_status") == "fail"),
            "remote_infra": sum(1 for r in no_std if r.get("remote_status") == "infra"),
        },
        "decisions": len(rows),
        "local_share": round(ran_local / len(rows), 3) if rows else None,
        "local_by_design": len(rows) - len(eligible),
        "remote_eligible": len(eligible),
        "remote_eligible_local_share": (
            round(eligible_local / len(eligible), 3) if eligible else None
        ),
        "by_caller": callers,
        "local": len(local),
        "local_wait_p50": p50([float(r.get("local_wait_s") or 0) for r in local]),
        "remote": len(remote_rows),
        "remote_pass": sum(1 for r in remote_rows if r.get("remote_status") == "pass"),
        "remote_fail_confirmed_locally": sum(
            1 for r in remote_rows if r.get("remote_status") == "fail"
        ),
        "remote_infra_fallback": sum(1 for r in remote_rows if r.get("remote_status") == "infra"),
        "remote_infra_causes": infra_causes(remote_rows),
        "remote_kept_passed": sum(int(r.get("kept_passed") or 0) for r in remote_rows),
        "local_after_remote_rows": len(followups),
        "remote_s_p50": p50([float(r.get("remote_s") or 0) for r in remote_rows]),
        "remote_pass_hosts": hosts,
        "host_health_refusals": refusals,
    }


def request_stop(
    pid: int | None = None,
    proc: Path = Path("/proc"),
    kill: Callable[[int, int], None] = os.kill,
    killpg: Callable[[int, int], None] = os.killpg,
) -> list[int]:
    """T3732: stop every remote verify of this process: the host loops end at their next check
    (STOP) and the current ssh/rsync children (and their process groups) get SIGTERM, so a
    blocked subprocess.run returns now. Other children (git, local verify commands) are left
    to their own cleanup. Returns the signalled child pids."""
    STOP.set()
    me = os.getpid() if pid is None else pid
    signalled: list[int] = []
    for stat in proc.glob("[0-9]*/stat"):
        try:
            text = stat.read_text()
            comm = text[text.index("(") + 1 : text.rindex(")")]
            fields = text.rsplit(")", 1)[1].split()
            child, parent, group = int(stat.parent.name), int(fields[1]), int(fields[2])
        except (OSError, IndexError, ValueError):
            continue
        if parent != me or comm not in REMOTE_CHILDREN:
            continue
        try:
            if group == child:
                killpg(child, signal.SIGTERM)  # its own group (start_new_session children)
            else:
                kill(child, signal.SIGTERM)
        except (ProcessLookupError, PermissionError):
            continue
        signalled.append(child)
    return signalled


def stopped(outcome: Outcome, started: float, say: Log) -> Outcome:
    """The host loop's answer after request_stop(): no verdict, try no further host."""
    outcome.notes.append("stopped_by_signal")
    outcome.seconds = round(time.monotonic() - started, 1)
    say("remote verify: stopped by a signal, trying no further host")
    return outcome


def section(log: str, index: int) -> str:
    """The log tail printed for remote command `index` (between @@BEGIN and @@RC)."""
    match = re.search(rf"^@@BEGIN {index}\n(.*?)^@@RC {index} ", log, re.S | re.M)
    return match.group(1) if match else log[-4000:]


def command_tests(ran: Mapping[int, remote_exec.CommandResult], log: str, index: int) -> int:
    """Use structured evidence retained before the bounded diagnostic tail (T3878).

    Legacy results need their own section: another command's tail is never test evidence.
    """
    item = ran.get(index)
    if item is not None and item.passed_tests is not None:
        return item.passed_tests
    match = re.search(rf"^@@BEGIN {index}\n(.*?)^@@RC {index} ", log, re.S | re.M)
    return tests_ran(match.group(1)) if match else 0


def _tree_clean(root: Path) -> bool:
    result = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    return result.returncode == 0 and not result.stdout.strip()


def run_remote(
    commands: Sequence[str],
    *,
    root: Path,
    commit: str = "HEAD",
    log: Log | None = None,
    hosts: Sequence[remote_exec.HostSpec] | None = None,
    main: Path | None = None,
    ssh: remote_exec.SshRunner | None = None,
    copy: remote_exec.Copier | None = None,
    command_timeout_s: int = 1800,
    tail_lines: int = 60,
    queue_s: float | None = None,
    env: Mapping[str, str] | None = None,
    budget_reservation: host_budget.Reservation | None = None,
) -> Outcome:
    """Run portable `commands` on the first eligible host that is free; see module doc.

    T3582: a chained command runs as one remote command per step (verdict and zero-test check
    per step, `commands` in the outcome stay the caller's), a lifted c-diff step carries its
    bundle, and `queue_s` (default REMOTE_QUEUE_S) is how long to wait for a free remote slot."""
    say = log or (lambda _text: None)
    started = time.monotonic()
    if hosts is None and os.environ.get("PYTEST_CURRENT_TEST"):
        return Outcome("infra", notes=["under_pytest"])  # tests never reach real hosts
    snapshot = None
    bundle_root = None
    prepared_tree = False
    if any(cdiff_bundle.CRATE in command for command in commands):
        try:
            if not _tree_clean(root):
                commit = remote_exec.worktree_tree(root)
                say(f"remote verify: shipping the uncommitted working tree as {commit[:12]}")
            else:
                commit = remote_exec.tree_of(root, commit)
            snapshot = tempfile.TemporaryDirectory(prefix="tsfp-cdiff-tree-")
            archive = Path(snapshot.name) / "tree.tar"
            remote_exec.archive_tree(root, commit, archive)
            bundle_root = Path(snapshot.name) / "tree"
            bundle_root.mkdir()
            with tarfile.open(archive) as source:
                source.extractall(bundle_root)
            prepared_tree = True
        except (OSError, tarfile.TarError, remote_exec.RemoteExecError) as error:
            if snapshot is not None:
                snapshot.cleanup()
            return Outcome("infra", notes=[f"cdiff_snapshot:{str(error)[:180]}"])
    forms: list[str | None] = []
    owners: list[str] = []  # the caller's command behind each remote form
    for command in commands:
        steps = remote_forms(
            command, root=root, main=main, bundle_root=bundle_root, env=env, log=say
        )
        for step in steps or [None]:
            forms.append(step)
            owners.append(command)
    if snapshot is not None:
        snapshot.cleanup()
    if not commands or any(form is None for form in forms):
        return Outcome("infra", notes=["not_portable"])
    commands = owners
    if hosts is None:
        from tools.dispatch import remote_evidence
        from tools.dispatch.hosts import policy

        key = remote_evidence.command_key(list(commands), root)
        duration = remote_evidence.estimate(key)
        hosts = [
            host
            for host in remote_exec.load_registry()
            if (
                (host.automatic_capabilities is None or "verify" in host.automatic_capabilities)
                and key in remote_evidence.validated_keys(host)
                and duration >= policy().remote_min_seconds
            )
        ]
        if not hosts:
            return Outcome(
                "infra",
                notes=[
                    "automatic_offload_unvalidated_or_short: use local capacity or an explicit "
                    "remote_exec verify --host HOST smoke check"
                ],
            )
    if not prepared_tree and not _tree_clean(root):
        # T3461: ship the working state as an exact tree instead of refusing.
        try:
            commit = remote_exec.worktree_tree(root)
        except remote_exec.RemoteExecError as error:
            return Outcome("infra", notes=[f"dirty_tree:{error}"[:200]])
        say(f"remote verify: shipping the uncommitted working tree as {commit[:12]}")
    outcome = Outcome("infra")
    order = {name: index for index, name in enumerate(VERIFY_ORDER)}
    tried: set[str] = set()
    busy_retry = False
    deadline = time.monotonic() + (REMOTE_QUEUE_S if queue_s is None else queue_s)
    while True:
        if STOP.is_set():
            return stopped(outcome, started, say)
        if busy_retry:  # a slot was taken while we staged: re-probe after a short pause
            if time.monotonic() >= deadline:
                break
            if STOP.wait(REMOTE_POLL_S):
                return stopped(outcome, started, say)
            busy_retry = False
        refused: list[str] = []
        loaded: list[tuple[remote_exec.HostSpec, remote_exec.Probe]] = []
        ranked = remote_exec.ranked_hosts(
            "verify", hosts=hosts, ssh=ssh, root=main, skipped=refused, waiting=loaded
        )
        for reason in refused:
            if reason.endswith(DISABLED_NOTE):
                continue  # T3837: the owner switched the host off; that is no failure to report
            if reason not in outcome.notes:
                outcome.notes.append(reason)
                say(f"remote verify: skipped {reason}")
        # Headroom decides; the historical preference only breaks an exact tie (T3878).
        ranked.sort(
            key=lambda pair: (
                headroom_score(pair[1].load1, pair[1].cores, pair[1].mem_available_mb / 1024),
                order.get(pair[0].name, len(order)),
            )
        )
        # T3837: a host whose shared budget (verify + build + capture) has no room for a verify
        # waits like a host with no free slot, instead of staging into an overloaded machine.
        full = {
            h.name
            for h, p in ranked
            if p.free_slots != 0
            and not (
                budget_reservation is not None
                and budget_reservation.covers(h.name, "verify", remote_exec.verify_cost(h))
            )
            and not budget_admits(h)
        }
        free = [
            (h, p)
            for h, p in ranked
            if p.free_slots != 0 and h.name not in full and h.name not in tried
        ]
        busy = [
            h.name
            for h, p in ranked
            if (p.free_slots == 0 or h.name in full) and h.name not in tried
        ]
        for h, p in loaded:  # T3516: too loaded to take a build, back to the wait window
            reasons = [r.removeprefix(host_health.HEALTH_PREFIX) for r in p.reasons]
            record_health_refusal(main, h.name, reasons, stage="probe")
            outcome.health += [f"{h.name}:{r}" for r in reasons]
            if h.name not in tried:
                busy.append(h.name)
        if not free:
            if busy and time.monotonic() < deadline:
                # T3461: the local host is the owner's PC; a remote slot is worth a short wait.
                say(f"remote verify: every verify host busy ({', '.join(busy)}), waiting")
                STOP.wait(REMOTE_POLL_S)
                continue
            outcome.notes += [
                f"{name}:budget_full" if name in full else f"{name}:slot_busy" for name in busy
            ]
            break
        for host, probe in free:
            if STOP.is_set():
                return stopped(outcome, started, say)
            tried.add(host.name)
            verdict = _try_host(
                host,
                probe,
                forms,
                commands,
                root=root,
                commit=commit,
                log=say,
                ssh=ssh,
                copy=copy,
                command_timeout_s=command_timeout_s,
                tail_lines=tail_lines,
                notes=outcome.notes,
                started=started,
                main=main,
                health=outcome.health,
                passed=outcome.passed,
                **(
                    {"budget_reservation": budget_reservation}
                    if budget_reservation is not None
                    else {}
                ),
            )
            if STOP.is_set():  # the verdict of a killed ssh is no verdict
                return stopped(outcome, started, say)
            if verdict is not None and verdict.status == "busy":
                tried.discard(host.name)  # re-probe it in the queue window
                busy_retry = True
                continue
            if verdict is not None:
                return verdict
    outcome.seconds = round(time.monotonic() - started, 1)
    if not outcome.notes:
        outcome.notes.append("no_eligible_host")
    say(f"remote verify: no remote verdict ({'; '.join(outcome.notes)}), waiting locally")
    return outcome


def keep_passed(
    passed: list[str],
    forms: Sequence[str | None],
    commands: Sequence[str],
    ran: Mapping[int, remote_exec.CommandResult],
    log: str,
    skip: Collection[int] = (),
) -> None:
    """T3798: add to `passed` each caller command whose every remote step exited 0 (and ran
    tests when it is a test step). Steps run in order on the exact tree, so a step that passed
    before the run was stopped or ended in an infrastructure exit is a valid verdict; a 600 s
    remote job yielded at 26 of 29 steps was otherwise verified again from step 1 locally."""
    steps: dict[str, list[int]] = {}
    for index, owner in enumerate(commands):
        steps.setdefault(owner, []).append(index)
    for owner, indexes in steps.items():
        if owner in passed:
            continue
        if all(
            index in ran
            and ran[index].returncode == 0
            and index not in skip
            and not (expects_tests(forms[index] or "") and not command_tests(ran, log, index))
            for index in indexes
        ):
            passed.append(owner)


def _try_host(
    host: remote_exec.HostSpec,
    probe: remote_exec.Probe,
    forms: Sequence[str | None],
    commands: Sequence[str],
    *,
    root: Path,
    commit: str,
    log: Log,
    ssh: remote_exec.SshRunner | None,
    copy: remote_exec.Copier | None,
    command_timeout_s: int,
    tail_lines: int,
    notes: list[str],
    started: float,
    main: Path | None = None,
    health: list[str] | None = None,
    passed: list[str] | None = None,
    budget_reservation: host_budget.Reservation | None = None,
) -> Outcome | None:
    """One host: a pass or fail Outcome, or None (infrastructure, reason added to notes).

    T3798: on None, `passed` gets the caller commands whose every step passed (with tests)."""
    say = log
    passed = [] if passed is None else passed
    outcome = Outcome("infra", notes=notes, health=health if health is not None else [])
    for host, probe in [(host, probe)]:  # noqa: B020 - one pass of the former host loop
        say(f"remote verify: trying {host.name} ({probe.mem_available_mb} MB available)")
        try:
            result = remote_exec.run_verify(
                host,
                root,
                commit,
                [form for form in forms if form],
                command_timeout_s=command_timeout_s,
                ssh=ssh,
                copy=copy,
                probe_result=probe,
                nonblocking=True,
                fetch=fetch_manifests([form for form in forms if form]),
                tail_lines=tail_lines,
                **(
                    {"budget_reservation": budget_reservation}
                    if budget_reservation is not None
                    else {}
                ),
            )
        except remote_exec.RemoteExecBusy as error:
            outcome.notes.append(str(error)[:200])
            return Outcome("busy", host=host.name, notes=outcome.notes, health=outcome.health)
        except (remote_exec.RemoteExecError, *remote_exec.TRANSPORT_ERRORS) as error:
            # T3472 p3: ssh/rsync timeouts and connection errors are infrastructure, never a
            # failed check (T2408v/T2407v crashed on a TimeoutExpired from the Deck copy).
            outcome.notes.append(
                f"{host.name}:{error}"[:200] or f"{host.name}:{type(error).__name__}"
            )
            continue
        ran = {item.index: item for item in result.commands}
        if result.returncode == 0:
            empty = [
                index
                for index, form in enumerate(forms)
                if form and expects_tests(form) and not command_tests(ran, result.log, index)
            ]
            if empty:
                # Zero tests remotely is no verdict (missing private inputs, filtered out).
                outcome.notes.append(f"{host.name}:zero_tests_remote")
                keep_passed(passed, forms, commands, ran, result.log, skip=set(empty))
                continue
            if len(ran) == len(forms) and all(item.returncode == 0 for item in ran.values()):
                say(
                    f"remote verify: passed on {host.name} tree {result.tree[:12]} "
                    f"(total {result.seconds}s, transfer {result.transfer_seconds}s, commands "
                    + ", ".join(f"{i.seconds}s/{i.peak_rss_kb} KB" for i in result.commands)
                    + ")"
                )
                return Outcome(
                    "pass",
                    host=host.name,
                    output="\n".join(section(result.log, index) for index in sorted(ran)),
                    seconds=round(time.monotonic() - started, 1),
                    notes=outcome.notes,
                    health=outcome.health,
                )
            outcome.notes.append(f"{host.name}:incomplete_results")
            continue
        if remote_exec.infrastructural(result) or not ran:
            keep_passed(passed, forms, commands, ran, result.log)
            from tools.dispatch import remote_evidence

            tail = remote_evidence.diagnostic(result.log)
            outcome.output = result.log[-12000:]
            if result.returncode == remote_exec.HEALTH_EXIT:
                # T3516: the run-time host-health gate (after the slot lock) found the host too
                # loaded. Not a verdict and not a transport fault: requeue the host in the wait
                # window like a busy slot, never mark it degraded.
                reasons = host_health.refusal_in(result.log) or ["unreadable"]
                outcome.notes.append(f"{host.name}:host_health_at_run:{','.join(reasons)}"[:200])
                outcome.health += [f"{host.name}:{reason}" for reason in reasons]
                record_health_refusal(main, host.name, reasons, stage="run")
                return Outcome("busy", host=host.name, notes=outcome.notes, health=outcome.health)
            if result.returncode == remote_exec.BUSY_EXIT:
                # T3461: exit 75 = @@LOCK_BUSY from slot_lock_command: every verify slot of the
                # host was taken between the probe and the run (the staging takes 40-150 s,
                # other dispatchers and remote prebuilt builds take slots meanwhile). The
                # archive stays as the host's rsync basis, so a retry is cheap: requeue it.
                outcome.notes.append(f"{host.name}:slot_taken_while_staging")
                return Outcome("busy", host=host.name, notes=outcome.notes)
            outcome.notes.append(f"{host.name}:infra_exit_{result.returncode}:{tail}"[:500])
            if result.returncode in {
                101,
                127,
                remote_exec.FETCH_EXIT,
            } and remote_exec.INFRA_LOG.search(result.log):
                outcome.notes.append(f"environment:{host.name}:{tail}"[:500])
            if getattr(result, "diagnostic_path", None):
                outcome.notes.append(f"diagnostic:{result.diagnostic_path}")
            continue
        failed = [index for index, item in ran.items() if item.returncode]
        if not failed:
            # The remote wrapper can fail after reporting successful command rows (for example,
            # while collecting its final receipt). That is incomplete evidence, not a command
            # failure; do not crash while trying to name a failed command.
            outcome.notes.append(f"{host.name}:incomplete_results")
            continue
        index = max(failed)
        say(f"remote verify: FAILED on {host.name} exit {ran[index].returncode}")
        return Outcome(
            "fail",
            host=host.name,
            failed=commands[index],
            returncode=ran[index].returncode,
            output=section(result.log, index),
            seconds=round(time.monotonic() - started, 1),
            notes=outcome.notes,
            health=outcome.health,
        )
    return None


def main() -> int:
    import argparse
    import json

    from tools.wt import core

    parser = argparse.ArgumentParser(description="T3461 verify admission statistics")
    parser.add_argument("--stats", action="store_true", help="print local/remote decisions")
    parser.add_argument("--hours", type=float, default=6.0)
    parser.add_argument("--until", type=float, help="exclusive window end, Unix seconds (T3878)")
    args = parser.parse_args()
    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, timeout=60
        ).stdout.strip()
        or "."
    )
    until = time.time() if args.until is None else args.until
    summary = stats(core.main_root(root), until - args.hours * 3600, until=until)
    summary["window"] = {"since": until - args.hours * 3600, "until": until}
    print(json.dumps(summary, indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
