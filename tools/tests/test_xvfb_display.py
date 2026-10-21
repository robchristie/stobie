"""Each smoke's Xvfb reports a free display rather than taking a fixed one."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SYSROOT = ROOT / ".tools/sysroot"

# A stand-in for Xvfb: it reports display 42 on the -displayfd descriptor, as
# Xvfb does once it accepts connections, records its arguments and waits.
FAKE_XVFB = """#!/usr/bin/env python3
import os, sys, time
arguments = sys.argv[1:]
with open(os.environ["FAKE_XVFB_ARGUMENTS"], "w") as record:
    record.write("\\n".join(arguments))
mode = os.environ.get("FAKE_XVFB_MODE", "report")
if mode == "exit":
    print("Fatal server error: no screens found", file=sys.stderr)
    sys.exit(1)
descriptor = int(arguments[arguments.index("-displayfd") + 1])
if mode == "report":
    os.write(descriptor, b"42\\n")
elif mode == "garbage":
    os.write(descriptor, b"not a display\\n")
while True:
    time.sleep(0.05)
"""


def lifecycle(body, *, directory, environment=None, timeout=10):
    script = """
set -euo pipefail
ROOT="$1" SYSROOT="$1/.tools/sysroot" SMOKE_TMP="$2" EVIDENCE_DIR="$2"
source "$ROOT/tools/native-smoke-lifecycle.sh"
""" + body
    env = {name: value for name, value in os.environ.items() if name != "STOBIE_XVFB_DISPLAY"}
    env.update(STOBIE_TARGET_DIR="unused", STOBIE_RECORD_DESK_TARGET_DIR="unused", **(environment or {}))
    return subprocess.run(["bash", "-c", script, "fixture", str(ROOT), str(directory)], env=env,
                          text=True, capture_output=True, timeout=timeout)


@unittest.skipUnless(sys.platform == "linux", "Linux native smoke lifecycle")
class FakeXvfbTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        fake = self.directory / "Xvfb"
        fake.write_text(FAKE_XVFB)
        fake.chmod(0o755)
        self.arguments = self.directory / "arguments"
        self.environment = {"STOBIE_USE_SYSTEM_UI_LIBS": "1", "XVFB": str(fake),
                            "FAKE_XVFB_ARGUMENTS": str(self.arguments)}

    def run_xvfb(self, mode, extra=None, body='owned_start_xvfb "$SMOKE_TMP/xvfb.log" -screen 0 64x64x24\n'
                 'echo "display=$DISPLAY"\nkill -0 "$XVFB_PID"\n'):
        return lifecycle(body, directory=self.directory,
                         environment={**self.environment, "FAKE_XVFB_MODE": mode, **(extra or {})})

    def test_the_reported_display_is_exported_and_the_server_is_owned(self):
        result = self.run_xvfb("report", body='owned_start_xvfb "$SMOKE_TMP/xvfb.log" -screen 0 64x64x24\n'
                               'echo "display=$DISPLAY pid=$XVFB_PID"\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("display=:42", result.stdout)
        self.assertEqual(self.arguments.read_text().split("\n"), ["-displayfd", "3", "-screen", "0", "64x64x24"])
        pid = result.stdout.split("pid=")[1].split()[0]
        self.assertFalse(Path(f"/proc/{pid}").exists(), "Xvfb outlived the smoke")

    def test_a_fixed_display_is_passed_through(self):
        result = self.run_xvfb("report", {"STOBIE_XVFB_DISPLAY": ":99"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.arguments.read_text().split("\n")[:3], [":99", "-displayfd", "3"])

    def test_a_server_that_exits_fails_with_its_log(self):
        result = self.run_xvfb("exit")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Xvfb exited before it accepted connections", result.stderr)
        self.assertIn("no screens found", result.stderr)

    def test_an_unreadable_display_fails(self):
        result = self.run_xvfb("garbage")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Xvfb reported display "not a display"', result.stderr)


@unittest.skipUnless(sys.platform == "linux" and (SYSROOT / "usr/bin/Xvfb").exists()
                     and shutil.which("bwrap"), "the repository UI sysroot's Xvfb")
class SandboxedXvfbTests(unittest.TestCase):
    def test_concurrent_servers_take_different_displays(self):
        # Each smoke's sandbox has a private /tmp, so only the abstract socket
        # namespace is shared; three at once must still find three displays.
        runs = []
        with tempfile.TemporaryDirectory(dir=ROOT / ".tools/runtime") as directory:
            for index in range(3):
                smoke = Path(directory, str(index))
                (smoke / ".X11-unix").mkdir(parents=True)
                smoke.chmod(0o1777)
                (smoke / ".X11-unix").chmod(0o1777)
                script = """
set -euo pipefail
ROOT="$1" SYSROOT="$1/.tools/sysroot" SMOKE_TMP="$2" EVIDENCE_DIR="$2"
export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
XVFB="$SYSROOT/usr/bin/Xvfb"
source "$ROOT/tools/native-smoke-lifecycle.sh"
owned_start_xvfb "$SMOKE_TMP/xvfb.log" -screen 0 64x64x24 -nolisten tcp
echo "$DISPLAY"
read -r _
"""
                env = {**os.environ, "STOBIE_TARGET_DIR": "unused", "STOBIE_RECORD_DESK_TARGET_DIR": "unused"}
                env.pop("STOBIE_USE_SYSTEM_UI_LIBS", None)
                env.pop("STOBIE_XVFB_DISPLAY", None)
                runs.append(subprocess.Popen(["bash", "-c", script, "fixture", str(ROOT), str(smoke)], env=env,
                                             text=True, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                             stderr=subprocess.PIPE))
            try:
                displays = [run.stdout.readline().strip() for run in runs]
                for run, display in zip(runs, displays):
                    self.assertRegex(display, r"^:\d+$", run.stderr.read() if not display else "")
                self.assertEqual(len(set(displays)), 3, displays)
            finally:
                for run in runs:
                    run.communicate(timeout=10)


if __name__ == "__main__":
    unittest.main()
