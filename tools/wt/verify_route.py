# SPDX-License-Identifier: GPL-3.0-or-later
"""T3582: split a worker `--verify` string into the cargo steps a remote verify host can run.

`tools.wt.remote_fallback.remote_form` only knew one command with one leading wrapper. The
strings workers actually submit are chains:

    python3 -m tools.wt run -- cargo test --manifest-path A && python3 -m tools.wt run -- env
    CARGO_HOME=/workspace/tmp/x cargo test ... && export TSFP_LIFTED_DIR=/workspace/... && ...

Every chain failed the portability test (a second `python3 -m tools.wt run` wrapper, an absolute
`CARGO_HOME`, a lift path) and compiled on the owner's PC while the verify hosts sat idle. This
module parses such a chain (`&&` only, no pipes, redirects, `;`, subshells or substitutions) into
segments, strips the host-local wrappers and environment of each (`tools.wt run`, `canon-run`,
`nice`, `env`, CARGO_HOME, CARGO_BUILD_JOBS, the lift directory), carries `export NAME=value`
forward and renders one remote command per cargo segment, so the verify host runs the same steps
in the same order and stops at the first failure. A c-diff test that reads the private lift
(`TSFP_LIFTED_DIR`) is marked `needs_bundle`; the caller turns it into a lifted bundle command
(tools.wt.cdiff_bundle, T3501) or keeps the whole chain local.
"""

from __future__ import annotations

import re
import shlex
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field

CDIFF_CRATE = "tsfp-c-diff"
LIFT_ENV = "TSFP_LIFTED_DIR"
# Host-local settings: the verify host picks its own cargo home, jobs, target and wrapper.
HOST_LOCAL = frozenset(
    {
        "CARGO_HOME",
        "CARGO_BUILD_JOBS",
        "CARGO_TARGET_DIR",
        "RUSTC_WRAPPER",
        "TSFP_WT_FAMILY",
        "TSFP_BUILD_SLOT_HELD",
        "TSFP_MERGE_URGENT",
        LIFT_ENV,
    }
)
ASSIGN = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$", re.S)
CARGO_VERB = re.compile(r"^(?:build|b|check|c|test|t|clippy|bench|doc|fmt)$")
PYTHON = re.compile(r"^python3?$")

Bundler = Callable[["Segment"], str]


@dataclass
class Segment:
    """One `&&`-separated step: the command that remains once the wrappers are stripped."""

    tokens: list[str]
    env: dict[str, str] = field(default_factory=dict)
    """Assignments in force for this step (earlier exports plus the step's own)."""

    @property
    def verb(self) -> str | None:
        """The cargo subcommand (`test`, `check`, ...), or None for a non-cargo step."""
        rest = self.tokens[1:]
        if not self.tokens or self.tokens[0] != "cargo":
            return None
        if rest and rest[0].startswith("+"):
            rest = rest[1:]
        return rest[0] if rest and CARGO_VERB.match(rest[0]) else None

    @property
    def targets_cdiff(self) -> bool:
        return any(CDIFF_CRATE in token for token in self.tokens)

    def needs_bundle(self, process_env: Mapping[str, str] | None = None) -> bool:
        """A c-diff test that reads the private lift: it needs a lifted bundle remotely."""
        if "TSFP_CDIFF_LIFT" in self.env:
            return False  # already carries its bundle (tools.wt build built it)
        lifted = LIFT_ENV in self.env or bool((process_env or {}).get(LIFT_ENV))
        return self.verb in ("test", "t") and self.targets_cdiff and lifted

    def lift_env(self, process_env: Mapping[str, str] | None = None) -> dict[str, str]:
        """The environment a local run would see: the process one plus this step's assignments."""
        return {**(process_env or {}), **self.env}

    def render(self) -> str:
        """The step as a remote shell command, host-local assignments dropped."""
        pairs = [
            f"{name}={shlex.quote(value)}"
            for name, value in self.env.items()
            if name not in HOST_LOCAL
        ]
        return " ".join([*pairs, shlex.join(self.tokens)])


def tokenize(command: str) -> list[str] | None:
    """Shell words of `command` with `&&` as its own token; None for any other operator."""
    if "\n" in command or "`" in command or "$(" in command:
        return None
    lexer = shlex.shlex(command, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    try:
        words = list(lexer)
    except ValueError:
        return None
    if any(set(word) <= set("();<>|&") and word != "&&" for word in words):
        return None
    return words


def _strip_wrappers(words: list[str], carried: dict[str, str]) -> tuple[Segment, bool] | None:
    """Peel `VAR=val`, `export`, `nice`, `env`, `tools.wt run` and `canon-run` off one step."""
    env = dict(carried)
    index = 0
    while index < len(words):
        word = words[index]
        if word == "export" and index == 0:
            pairs = [ASSIGN.match(item) for item in words[1:]]
            if not pairs or not all(pairs):
                return None
            carried.update({m.group(1): m.group(2) for m in pairs if m})
            return Segment([], carried.copy()), True
        match = ASSIGN.match(word)
        if match:
            env[match.group(1)] = match.group(2)
            index += 1
        elif word == "env":
            index += 1
            if index < len(words) and words[index].startswith("-"):
                return None  # env -i and friends change the meaning
        elif word == "nice":
            index += 1
            if index < len(words) and words[index] in ("-n", "--adjustment"):
                index += 2
            elif index < len(words) and re.match(r"^-\d+$", words[index]):
                index += 1
        elif PYTHON.match(word) and words[index + 1 : index + 3] == ["-m", "tools.wt"]:
            if words[index + 3 : index + 4] != ["run"]:
                return None
            index += 4
            while index < len(words) and words[index] != "--":
                if words[index] not in ("--family", "--worktree") or index + 1 >= len(words):
                    return None
                index += 2
            index += 1  # the `--`
        elif PYTHON.match(word) and words[index + 1 : index + 3] == [
            "-m",
            "tools.agents.verify_profiles",
        ]:
            if words[index + 3 : index + 5] != ["canon-run", "--"]:
                return None
            index += 5
        else:
            break
    return Segment(words[index:], env), False


def split_steps(command: str) -> list[str] | None:
    """The `&&`-separated steps of `command` as exact source text, or None.

    Only a pure `&&` list splits: `;`, `||`, `&`, subshells, substitutions and newlines make the
    meaning depend on precedence, so such a command is never split (it stays whole and local).
    Pipes and redirects stay inside their step."""
    if "\n" in command or "`" in command:
        return None
    steps: list[str] = []
    start = 0
    quote = ""
    index = 0
    while index < len(command):
        char = command[index]
        if quote:
            if char == "\\" and quote == '"':
                index += 1
            elif char == quote:
                quote = ""
        elif char in "'\"":
            quote = char
        elif char == "\\":
            index += 1
        elif command.startswith("$(", index) or char in ";()":
            return None
        elif command.startswith("||", index):
            return None
        elif command.startswith("&&", index):
            steps.append(command[start:index].strip())
            start = index + 2
            index += 1
        elif char == "&" and command[index - 1 : index] not in (">", "<"):
            return None
        index += 1
    if quote:
        return None
    steps.append(command[start:].strip())
    return steps if all(steps) else None


def is_export(step: str) -> bool:
    words = tokenize(step)
    return bool(words) and words[0] == "export"


def parse(command: str) -> list[Segment] | None:
    """The executable steps of an `&&` chain (exports folded into later steps), or None."""
    words = tokenize(command)
    if not words:
        return None
    steps: list[list[str]] = [[]]
    for word in words:
        if word == "&&":
            steps.append([])
        else:
            steps[-1].append(word)
    carried: dict[str, str] = {}
    segments: list[Segment] = []
    for step in steps:
        if not step:
            return None
        parsed = _strip_wrappers(step, carried)
        if parsed is None:
            return None
        segment, export_only = parsed
        if not export_only:
            if not segment.tokens:
                return None
            segments.append(segment)
    return segments or None


def chain_forms(
    command: str,
    *,
    process_env: Mapping[str, str] | None = None,
    bundler: Bundler | None = None,
    check: Callable[[str], bool] = lambda _text: True,
) -> list[str] | None:
    """One remote command per step of `command`, or None when any step must stay local.

    `check(text)` is the caller's per-step portability test (cargo verb, no absolute path).
    A step that needs the private lift is rendered by `bundler(segment)` (the lifted-bundle
    command); without a bundler such a chain is not portable. A fmt step rides along with a
    real cargo step only."""
    segments = parse(command)
    if segments is None:
        return None
    forms: list[str] = []
    heavy = False
    for segment in segments:
        verb = segment.verb
        if verb is None:
            return None
        if segment.needs_bundle(process_env):
            if bundler is None:
                return None
            text = bundler(segment)
        elif LIFT_ENV in segment.env:
            return None  # reads the private lift but is no c-diff test: main host only
        else:
            text = segment.render()
        if verb != "fmt":
            heavy = True
            if not check(text):
                return None
        elif not check(text.replace("cargo fmt", "cargo check", 1)):
            return None
        forms.append(text)
    return forms if heavy else None
