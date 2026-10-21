#!/usr/bin/env python3
"""Run one Linux smoke process tree; terminate and reap every owned descendant.

usage: owned-process.py [--pass-fd N] COMMAND [ARGUMENT...]

--pass-fd keeps the inherited descriptor N open in COMMAND, such as the
descriptor Xvfb's -displayfd writes its display number to.
"""

import ctypes
import os
import signal
import subprocess
import sys
import time


def main():
    # Adopt orphaned sandbox/app grandchildren so cleanup cannot leave zombies.
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(36, 1, 0, 0, 0) != 0:  # PR_SET_CHILD_SUBREAPER
        raise OSError(ctypes.get_errno(), "enable child subreaper")
    interrupted = 0

    def stop(signum, _frame):
        nonlocal interrupted
        interrupted = signum

    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, stop)
    command = sys.argv[1:]
    passed = ()
    if command[:1] == ["--pass-fd"]:
        passed = (int(command[1]),)
        command = command[2:]
    child = subprocess.Popen(command, start_new_session=True, pass_fds=passed)
    while child.poll() is None and not interrupted:
        time.sleep(0.02)
    result = 128 + interrupted if interrupted else child.returncode
    # The session is private to this launch, including bwrap and its app.
    for signum, grace in ((signal.SIGTERM, 0.5), (signal.SIGKILL, 2.0)):
        try:
            os.killpg(child.pid, signum)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + grace
        while time.monotonic() < deadline:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                return result
            if pid == 0:
                time.sleep(0.02)
    print("owned smoke descendants did not terminate within cleanup deadline", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
