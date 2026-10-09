from pathlib import Path
import subprocess
import tempfile
import unittest

LIFECYCLE = Path(__file__).resolve().parents[1] / "native-smoke-lifecycle.sh"


class WindowReadinessTests(unittest.TestCase):
    def run_fixture(self, body):
        with tempfile.TemporaryDirectory() as temporary:
            return subprocess.run(["bash", "-c", '''
set -euo pipefail
POLYORAMA_USE_SYSTEM_UI_LIBS=1
source "$1"
''' + body, "fixture", str(LIFECYCLE)], cwd=temporary, text=True,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=4)

    def test_delayed_visibility_is_observed_without_a_fixed_startup_sleep(self):
        result = self.run_fixture('''
xdo() {
  n=$(cat calls 2>/dev/null || echo 0)
  echo "$((n + 1))" > calls
  if (( n < 2 )); then return 1; fi
  printf '123\\n456\\n'
}
owned_wait_window 'application title' "$$" 2
''')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "123\n")

    def test_missing_window_fails_at_the_bound(self):
        result = self.run_fixture('xdo() { return 1; }; owned_wait_window app "$$" 0')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("window did not appear", result.stderr)

    def test_application_exit_fails_before_waiting_for_visibility(self):
        result = self.run_fixture('''
sleep 0 &
fixture_pid=$!
wait "$fixture_pid"
xdo() { echo 'unexpected search' >&2; return 1; }
owned_wait_window app "$fixture_pid" 2
''')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("application exited", result.stderr)
        self.assertNotIn("unexpected search", result.stderr)


if __name__ == "__main__":
    unittest.main()
