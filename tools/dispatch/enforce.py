"""Owner-PC enforcement of the dispatcher (T3885 step 4, rule 48).

On the owner PC (`tools.local_game_slots.is_owner_pc`, the one detection) no heavy command
starts without a dispatcher placement token (`TSFP_DISPATCH_TOKEN`, a live owner-PC lease):

* cargo build/test through `tools.wt run` (`tools.wt.cli.dispatch_heavy`): placed first, run
  remotely or here under the lease;
* game host launches through `tools.local_game_slots.hold` (every `hostresources.prepare` argv
  and every direct spawner, guarded by tests/test_local_game_slots.py): a `game` placement is
  held with the slot; interactive owner play is exempt (its display is the owner's);
* pytest with more than PYTEST_WORKERS xdist workers (tests/conftest.py) without a token: cut
  to PYTEST_WORKERS workers before xdist starts them (larger runs: `tools.dispatch pytest`).

A launcher without a token takes its placement here and waits in the dispatcher's queue under
the owner PC's cap; there is no path that runs the work without one. Static guard against
direct launches that bypass these entry points: tests/test_t3885_dispatch.py (allowlist only
shrinks).

Under pytest nothing is dispatched for real unless a test injects a dispatcher (`inject`), so a
test never touches the live queue or hosts (rule 39).
"""

from __future__ import annotations

import contextlib
import os
import sys
from collections.abc import Iterator, Mapping
from pathlib import Path
from typing import Any

from tools.dispatch.queue import TOKEN_ENV, Dispatcher, Lease, token_valid

PYTEST_WORKERS = 4
"""xdist workers a pytest session may use on the owner PC without a placement."""
GAME_CORES = 4.0
"""A local game host (software GL on Xvfb) uses about 5 cores at full speed (T3823)."""

_injected: list[Dispatcher] = []


def inject(dispatcher: Dispatcher | None) -> None:
    """Tests: route enforcement placements to `dispatcher` (None removes it)."""
    _injected.clear()
    if dispatcher is not None:
        _injected.append(dispatcher)


def placed(env: Mapping[str, str] | None = None) -> bool:
    """This process already runs under a live owner-PC placement."""
    token = (os.environ if env is None else env).get(TOKEN_ENV, "")
    return bool(token) and token_valid(token)


def _dispatcher() -> Dispatcher | None:
    if _injected:
        return _injected[0]
    if os.environ.get("PYTEST_CURRENT_TEST"):
        return None
    from tools import dispatch
    from tools.datadir import main_root

    return dispatch.default(main_root(), log=lambda text: print(text, file=sys.stderr, flush=True))


def owner_pc(root: Path | None = None) -> bool:
    from tools import local_game_slots

    return local_game_slots.is_owner_pc(root or local_game_slots.ROOT)


@contextlib.contextmanager
def placement(
    kind: str, cores: float, *, label: str, reason: str, root: Path | None = None
) -> Iterator[Lease | None]:
    """Hold an owner-PC placement for a heavy local launch; a no-op when already placed, off
    the owner PC, or under pytest without an injected dispatcher. The token is exported to
    this process's environment for its children and restored afterwards."""
    if placed() or not owner_pc(root):
        yield None
        return
    dispatcher = _dispatcher()
    if dispatcher is None:
        yield None
        return
    from tools import dispatch

    spec = dispatch.job(kind, cores, local_only=True, local_reason=reason, label=label)
    lease = dispatcher.acquire(spec)
    previous = os.environ.get(TOKEN_ENV)
    os.environ[TOKEN_ENV] = lease.id
    try:
        yield lease
    finally:
        if previous is None:
            os.environ.pop(TOKEN_ENV, None)
        else:
            os.environ[TOKEN_ENV] = previous
        dispatcher.release(lease)


def xdist_workers(config: Any) -> int:
    """Workers xdist will start for this session (its expanded launch list, else -n)."""
    tx = getattr(config.option, "tx", None)
    if tx:
        return len(tx)
    requested = getattr(config.option, "numprocesses", None)
    return requested if isinstance(requested, int) else 0


def pytest_session_start(config: Any) -> None:
    """tests/conftest.py pytest_configure: on the owner PC a session without a placement runs
    at most PYTEST_WORKERS xdist workers; more are cut before xdist starts them.

    A queued placement for the whole session (the first version) starved landings: a waiting
    `pytest -n 12` needs the owner PC to itself. Larger runs go through `python3 -m
    tools.dispatch pytest`, which places them on a remote host."""
    if hasattr(config, "workerinput") or placed() or not owner_pc():
        return
    workers = xdist_workers(config)
    if workers <= PYTEST_WORKERS:
        return
    tx = getattr(config.option, "tx", None)
    if tx:
        config.option.tx = list(tx)[:PYTEST_WORKERS]
    if isinstance(getattr(config.option, "numprocesses", None), int):
        config.option.numprocesses = PYTEST_WORKERS
    if getattr(config.option, "maxprocesses", None):
        config.option.maxprocesses = min(config.option.maxprocesses, PYTEST_WORKERS)
    print(
        f"T3885: pytest -n {workers} on the owner PC capped to {PYTEST_WORKERS} workers; "
        "use `python3 -m tools.dispatch pytest --workers N <tests>` for larger runs",
        file=sys.stderr,
        flush=True,
    )


def pytest_session_end(config: Any) -> None:
    """Kept for tests/conftest.py (nothing is held for a session)."""
