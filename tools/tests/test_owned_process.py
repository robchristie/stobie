"""Exercise the actual shell lifecycle with nested, TERM-resistant processes."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

TOOLS = Path(__file__).resolve().parents[1]
TREE = '''
import os, signal, sys, time
from pathlib import Path
signal.signal(signal.SIGTERM, signal.SIG_IGN)
child = os.fork()
if child == 0:
    child = os.fork()
    if child == 0:
        with open(sys.argv[1], 'a') as output:
            output.write(str(os.getpid()) + '\\n')
        while True: time.sleep(.02)
    os._exit(0)
with open(sys.argv[1], 'a') as output:
    output.write(str(os.getpid()) + '\\n')
while not Path(sys.argv[2]).exists(): time.sleep(.02)
sys.exit(int(sys.argv[3]))
'''


@unittest.skipUnless(sys.platform == "linux", "Linux native smoke lifecycle")
class LifecycleTests(unittest.TestCase):
    def test_success_failure_restart_and_interrupt(self):
        sentinel = subprocess.Popen(["sleep", "60"])
        self.addCleanup(lambda: (sentinel.terminate(), sentinel.wait()))
        for mode in ("success", "success", "failure", "restart", "TERM", "INT"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                tree = root / "tree.py"
                tree.write_text(TREE)
                pids = root / "pids"
                finish = root / "finish"
                script = '''
set -euo pipefail
ROOT="$1"
POLYORAMA_USE_SYSTEM_UI_LIBS=1
source "$ROOT/tools/native-smoke-lifecycle.sh"
owned_start APP_PID "$2" "$3" "$4" "$5" "$6"
while [[ ! -f "$5" ]]; do sleep .02; done
if [[ "$7" == restart ]]; then
  owned_stop APP_PID
  owned_start APP_PID "$2" "$3" "$4" "$5" 0
fi
wait "$APP_PID"
'''
                process = subprocess.Popen(["bash", "-c", script, "lifecycle", str(TOOLS.parent),
                                            sys.executable, str(tree), str(pids), str(finish),
                                            "7" if mode == "failure" else "0", mode])
                try:
                    deadline = time.monotonic() + 5
                    while not pids.exists() or len(pids.read_text().splitlines()) < 2:
                        if time.monotonic() > deadline:
                            self.fail("fixture did not start")
                        time.sleep(.02)
                    if mode in ("TERM", "INT"):
                        process.send_signal(getattr(signal, "SIG" + mode))
                    else:
                        finish.touch()
                    self.assertEqual(process.wait(timeout=6), {"failure": 7, "TERM": 143, "INT": 130}.get(mode, 0))
                    for pid in pids.read_text().splitlines():
                        self.assertFalse(Path(f"/proc/{pid}").exists(), f"owned descendant {pid} survived {mode}")
                    self.assertIsNone(sentinel.poll(), "unrelated process was terminated")
                finally:
                    if process.poll() is None:
                        process.terminate()
                        process.wait(timeout=6)


if __name__ == "__main__":
    unittest.main()
