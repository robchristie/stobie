"""Smokes find binaries wherever Cargo builds them, not only in target/."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

from tools.target_directory import target_directory

ROOT = Path(__file__).resolve().parents[2]
RECORD_DESK = ROOT / "consumers/record-desk/Cargo.toml"


class TargetDirectoryTests(unittest.TestCase):
    def test_cargo_target_dir_moves_both_workspaces(self):
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": directory}):
            self.assertEqual(target_directory(), Path(directory))
            self.assertEqual(target_directory(RECORD_DESK), Path(directory))

    def test_the_smoke_lifecycle_exports_what_cargo_reports(self):
        with tempfile.TemporaryDirectory() as directory:
            script = """
set -euo pipefail
ROOT="$1" SYSROOT=unused SMOKE_TMP="$2" EVIDENCE_DIR="$2" STOBIE_USE_SYSTEM_UI_LIBS=1
source "$ROOT/tools/native-smoke-lifecycle.sh"
printf '%s\\n%s\\n' "$STOBIE_TARGET_DIR" "$STOBIE_RECORD_DESK_TARGET_DIR"
"""
            environment = {name: value for name, value in os.environ.items()
                           if name not in ("STOBIE_TARGET_DIR", "STOBIE_RECORD_DESK_TARGET_DIR")}
            environment["CARGO_TARGET_DIR"] = directory
            result = subprocess.run(["bash", "-c", script, "fixture", str(ROOT), directory],
                                    env=environment, text=True, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.splitlines(), [directory, directory])


if __name__ == "__main__":
    unittest.main()
