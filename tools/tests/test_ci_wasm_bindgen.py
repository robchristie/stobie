"""Check that CI validates the executable it will use before verification."""

import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
INSTALLER = ROOT / "tools" / "ensure-wasm-bindgen-cli.sh"


class WasmBindgenCiTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.bin_dir = Path(self.temporary.name)
        self.calls = self.bin_dir / "cargo-calls"
        cargo = self.bin_dir / "cargo"
        cargo.write_text(
            """#!/bin/sh
printf '%s\\n' "$@" > "$MOCK_CARGO_CALLS"
if [ "${MOCK_CARGO_FAIL:-0}" = 1 ]; then exit 23; fi
cat > "$MOCK_BIN/wasm-bindgen" <<EOF
#!/bin/sh
printf 'wasm-bindgen %s\\n' "$MOCK_INSTALL_VERSION"
exit "$MOCK_INSTALLED_EXIT"
EOF
chmod +x "$MOCK_BIN/wasm-bindgen"
""",
            encoding="utf-8",
        )
        cargo.chmod(0o755)

    def _binary(self, version, exit_code=0):
        binary = self.bin_dir / "wasm-bindgen"
        binary.write_text(
            f"#!/bin/sh\nprintf 'wasm-bindgen {version}\\n'\nexit {exit_code}\n",
            encoding="utf-8",
        )
        binary.chmod(0o755)

    def _run(self, *, installed="0.2.127", fail=False, installed_exit=0):
        env = os.environ.copy()
        env.update(
            PATH=f"{self.bin_dir}:/usr/bin:/bin",
            MOCK_BIN=str(self.bin_dir),
            MOCK_CARGO_CALLS=str(self.calls),
            MOCK_INSTALL_VERSION=installed,
            MOCK_INSTALLED_EXIT=str(installed_exit),
            MOCK_CARGO_FAIL="1" if fail else "0",
        )
        return subprocess.run(
            ["bash", str(INSTALLER)], capture_output=True, text=True, env=env, check=False
        )

    def test_pinned_cli_is_reused_without_installation(self):
        self._binary("0.2.127")
        result = self._run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Using wasm-bindgen 0.2.127", result.stdout)
        self.assertFalse(self.calls.exists())

    def test_missing_cli_is_installed_at_pinned_version(self):
        result = self._run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls.read_text(encoding="utf-8").splitlines(),
            ["install", "--locked", "--force", "wasm-bindgen-cli", "--version", "0.2.127"],
        )

    def test_wrong_cli_is_replaced(self):
        self._binary("0.2.126")
        result = self._run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("observed wasm-bindgen 0.2.126", result.stdout)
        self.assertTrue(self.calls.exists())
        self.assertEqual(
            subprocess.check_output([self.bin_dir / "wasm-bindgen", "--version"], text=True).strip(),
            "wasm-bindgen 0.2.127",
        )

    def test_failing_cli_with_expected_output_is_replaced(self):
        self._binary("0.2.127", exit_code=7)
        result = self._run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.calls.exists())

    def test_incorrect_install_result_fails(self):
        result = self._run(installed="0.2.126")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Expected wasm-bindgen CLI 0.2.127", result.stderr)

    def test_failing_installed_cli_with_expected_output_fails(self):
        result = self._run(installed_exit=7)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Expected wasm-bindgen CLI 0.2.127", result.stderr)

    def test_install_failure_fails(self):
        result = self._run(fail=True)
        self.assertNotEqual(result.returncode, 0)

    def test_cli_version_matches_workspace_package(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        script = INSTALLER.read_text(encoding="utf-8")
        version = re.search(r'^expected_version="([^"]+)"$', script, re.MULTILINE)
        self.assertIsNotNone(version)
        self.assertEqual(version.group(1), manifest["workspace"]["dependencies"]["wasm-bindgen"])


if __name__ == "__main__":
    unittest.main()
