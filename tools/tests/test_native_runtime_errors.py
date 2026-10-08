"""Exercise the actual native runtime-log guard and scanner exit contract."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BASH = shutil.which("bash")
GUARD = ROOT / "tools" / "native-runtime-errors.sh"


@unittest.skipUnless(sys.platform == "linux", "Linux native runtime log guard")
class NativeRuntimeErrorsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.log = self.root / "runtime.log"
        self.log.write_text("native application started\n", encoding="utf-8")

    def run_guard(self, *, scanner=None):
        env = os.environ.copy()
        if scanner is not None:
            env["PATH"] = str(self.root)
            if scanner != "absent":
                executable = self.root / "rg"
                executable.write_text(f"#!/bin/sh\nexit {scanner}\n", encoding="utf-8")
                executable.chmod(0o755)
        return subprocess.run([BASH, "-c", 'source "$1"; assert_clean_native_runtime_log "$2" "native fixture"',
            "guard-test", str(GUARD), str(self.log)], env=env, capture_output=True, text=True, check=False)

    def test_clean_log_passes(self):
        result = self.run_guard()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_each_existing_runtime_error_pattern_fails(self):
        for text in ["panicked at test", "WGPU error: test", "Exiting because of error"]:
            with self.subTest(text=text):
                self.log.write_text(text, encoding="utf-8")
                result = self.run_guard()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("observed an application failure", result.stderr)

    def test_missing_log_fails(self):
        self.log.unlink()
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("scan failed", result.stderr)

    def test_unreadable_log_fails(self):
        self.log.chmod(0)
        try:
            result = self.run_guard()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("scan failed", result.stderr)
        finally:
            self.log.chmod(0o600)

    def test_unavailable_scanner_fails(self):
        result = self.run_guard(scanner="absent")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("scan failed (127)", result.stderr)

    def test_scanner_failure_fails(self):
        result = self.run_guard(scanner=23)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("scan failed (23)", result.stderr)

    def test_scanner_one_is_clean_and_zero_is_error(self):
        self.assertEqual(self.run_guard(scanner=1).returncode, 0)
        self.assertNotEqual(self.run_guard(scanner=0).returncode, 0)
