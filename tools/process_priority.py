"""Run background tooling politely on shared developer machines.

Call ``lower_priority`` once at a command entry point. Children inherit its CPU and
I/O scheduling priorities, including grandchildren started by shell runners.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

MIN_NICE = 10


def lower_priority() -> int:
    """Set every existing thread and this process to nice >= 10 and idle I/O."""
    # Linux nice is per-thread. PRIO_PROCESS with pid 0 only affects the calling
    # thread, so a thread created before this entry-point call can keep its old
    # priority and spawn children that escape the intended inheritance.
    try:
        task_ids = {os.getpid()}
        task_ids.update(int(path.name) for path in Path("/proc/self/task").iterdir())
    except (OSError, ValueError):
        task_ids = {os.getpid()}

    highest = MIN_NICE
    for task_id in sorted(task_ids):
        try:
            current = os.getpriority(os.PRIO_PROCESS, task_id)
            target = max(MIN_NICE, current)
            highest = max(highest, target)
            if current < target:
                os.setpriority(os.PRIO_PROCESS, task_id, target)
        except (AttributeError, OSError):
            continue

    # A second pass catches threads that appeared while the first pass ran.
    # Callers invoke this before creating their own worker threads.
    try:
        for path in Path("/proc/self/task").iterdir():
            task_id = int(path.name)
            current = os.getpriority(os.PRIO_PROCESS, task_id)
            target = max(MIN_NICE, current)
            highest = max(highest, target)
            if current < target:
                os.setpriority(os.PRIO_PROCESS, task_id, target)
    except (OSError, ValueError):
        pass

    ionice = shutil.which("ionice")
    if ionice:
        for task_id in task_ids:
            try:
                subprocess.run(
                    [ionice, "-c", "3", "-p", str(task_id)],
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    check=False,
                    timeout=2,
                )
            except (OSError, subprocess.SubprocessError):
                continue
    return highest
