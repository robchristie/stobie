import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "normalise-ci-toolchain.sh"


class ToolchainTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.99.0"\n')
        (self.root / "installed").write_text('1.98.1-host\n1.99.0-host\n')
        (self.root / "rustup").write_text('''#!/usr/bin/env python3
import os, sys
from pathlib import Path
args = sys.argv[1:]
if args == ["show", "active-toolchain"]:
    print(os.environ.get("TEST_ACTIVE", "1.99.0-host") + " (overridden)")
elif args == ["toolchain", "list"]:
    print(Path("installed").read_text(), end="")
elif args[:3] == ["toolchain", "uninstall", "--"]:
    with open("removed", "a") as log: log.write(args[3] + "\\n")
    path = Path("installed")
    path.write_text("".join(line + "\\n" for line in path.read_text().splitlines() if line != args[3]))
else:
    raise SystemExit(2)
''')
        (self.root / "rustc").write_text('#!/usr/bin/env bash\necho "rustc 1.99.0 (test)"\n')
        for name in ("rustup", "rustc"):
            (self.root / name).chmod(0o755)
        self.env = {**os.environ, "PATH": f"{self.root}:{os.environ['PATH']}",
                    "GITHUB_ACTIONS": "true", "RUNNER_ENVIRONMENT": "github-hosted"}

    def run_script(self):
        return subprocess.run(["bash", str(SCRIPT)], cwd=self.root, env=self.env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    def test_only_unused_toolchains_are_removed_on_hosted_runner(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "removed").read_text(), "1.98.1-host\n")
        self.assertEqual((self.root / "installed").read_text(), "1.99.0-host\n")

    def test_developer_and_self_hosted_toolchains_are_preserved(self):
        for actions, runner in (("false", "github-hosted"), ("true", "self-hosted")):
            self.env.update(GITHUB_ACTIONS=actions, RUNNER_ENVIRONMENT=runner)
            self.assertNotEqual(self.run_script().returncode, 0)
            self.assertFalse((self.root / "removed").exists())

    def test_wrong_active_compiler_fails_before_any_uninstall(self):
        self.env["TEST_ACTIVE"] = "1.98.1-host"
        self.assertNotEqual(self.run_script().returncode, 0)
        self.assertFalse((self.root / "removed").exists())


if __name__ == "__main__":
    unittest.main()
