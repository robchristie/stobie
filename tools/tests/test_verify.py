import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("verify", Path(__file__).resolve().parents[1] / "verify.py")
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


class ScopeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Test")
        self.write("README.md")
        self.write("src/main.rs")
        self.git("add", ".")
        self.git("commit", "-qm", "Initial")
        self.base = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.root), *args], text=True)

    def write(self, path, text="initial\n"):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text)

    def scoped(self):
        return verify.documentation_only(self.root, self.base)

    def test_prose_committed_staged_dirty_and_untracked(self):
        self.write("README.md", "changed\n")
        self.assertTrue(self.scoped())
        self.git("add", ".")
        self.assertTrue(self.scoped())
        self.git("commit", "-qm", "Prose")
        self.write("docs/closeout-plan.md")
        self.assertTrue(self.scoped())

    def test_clean_scope_without_hidden_index_flags(self):
        self.assertTrue(self.scoped())
        self.write("README.md", "changed\n")
        self.assertTrue(self.scoped())

    def test_hidden_index_flags_fail_closed(self):
        self.write("README.md", "changed\n")
        for flag in ("assume-unchanged", "skip-worktree"):
            for path in ("src/main.rs", "README.md"):
                with self.subTest(flag=flag, path=path):
                    self.git("update-index", "--" + flag, path)
                    self.assertFalse(self.scoped(), "flag presence alone must select full verification")
                    self.write(path, "hidden modified bytes\n")
                    self.assertFalse(self.scoped())
                    self.git("update-index", "--no-" + flag, path)
                    self.write(path, "changed\n" if path == "README.md" else "initial\n")
                    self.assertTrue(self.scoped())

    def test_sparse_checkout_fails_closed(self):
        self.git("sparse-checkout", "set", "--no-cone", "/README.md")
        self.assertFalse((self.root / "src/main.rs").exists())
        self.assertFalse(self.scoped())
        self.git("sparse-checkout", "disable")
        self.assertTrue(self.scoped())
        # Reject sparse mode even when every tracked file is materialised.
        self.git("config", "--worktree", "core.sparseCheckout", "true")
        self.assertFalse(self.scoped())

    def test_mixed_and_hidden_staged_code(self):
        self.write("README.md", "changed\n")
        self.write("src/main.rs", "changed\n")
        self.assertFalse(self.scoped())
        self.git("add", ".")
        self.write("src/main.rs")
        self.assertFalse(self.scoped())

    def test_nonallowlisted_paths(self):
        for path in ("AGENTS.md", "docs/AGENTS.md", "Cargo.lock", "tools/check.py",
                     ".github/workflows/verify.yml", "docs/ui-snapshots/README.md",
                     "docs/example-evidence/result.md", "docs/qualification-report.md",
                     "new.rs", "docs/ui-evaluation-seed.md"):
            with self.subTest(path=path):
                self.write(path)
                self.assertFalse(self.scoped())
                (self.root / path).unlink()

    def test_rename_checks_both_paths(self):
        (self.root / "docs").mkdir()
        self.git("mv", "src/main.rs", "docs/moved-plan.md")
        self.assertFalse(self.scoped())
        self.git("reset", "--hard", "-q")
        (self.root / "docs").mkdir(exist_ok=True)
        self.git("mv", "README.md", "docs/readme-plan.md")
        self.assertTrue(self.scoped())

    def test_deletions(self):
        self.git("rm", "-q", "README.md")
        self.assertTrue(self.scoped())
        self.git("rm", "-q", "src/main.rs")
        self.assertFalse(self.scoped())

    def test_missing_unknown_and_empty_base(self):
        for base in (None, "", 123, "invalid", "0" * 40, "--help"):
            self.assertFalse(verify.documentation_only(self.root, base))

    def test_symlink_and_mode_change(self):
        (self.root / "README.md").unlink()
        (self.root / "README.md").symlink_to("src/main.rs")
        self.assertFalse(self.scoped())
        self.git("checkout", "--", "README.md")
        (self.root / "README.md").chmod(0o755)
        self.assertFalse(self.scoped())

    def test_untracked_executable_prose(self):
        self.write("docs/new-plan.md")
        (self.root / "docs/new-plan.md").chmod(0o755)
        self.assertFalse(self.scoped())

    def test_ci_event_comparison_bases(self):
        event_path = self.root / ".git/event.json"
        for event_name, event, expected in (
            ("pull_request", {"pull_request": {"base": {"sha": self.base}}}, self.base),
            ("push", {"before": self.base}, self.base),
            ("pull_request", {"pull_request": {"base": {"sha": "missing"}}}, None),
            ("push", {}, None),
            ("push", [], None),
            ("workflow_dispatch", {}, None),
        ):
            with self.subTest(event_name=event_name, event=event):
                event_path.write_text(json.dumps(event))
                with patch.dict("os.environ", {"GITHUB_EVENT_PATH": str(event_path), "GITHUB_EVENT_NAME": event_name}):
                    self.assertEqual(verify.event_base(self.root), expected)
        event_path.write_text("invalid json")
        with patch.dict("os.environ", {"GITHUB_EVENT_PATH": str(event_path), "GITHUB_EVENT_NAME": "push"}):
            self.assertIsNone(verify.event_base(self.root))
            event_path.unlink()
            self.assertIsNone(verify.event_base(self.root))

    def test_unmerged_state(self):
        blob = self.git("rev-parse", "HEAD:README.md").strip()
        self.git("update-index", "--force-remove", "README.md")
        subprocess.run(["git", "-C", str(self.root), "update-index", "--index-info"],
                       input=f"100644 {blob} 1\tREADME.md\n100644 {blob} 2\tREADME.md\n", text=True, check=True)
        self.assertFalse(self.scoped())


if __name__ == "__main__":
    unittest.main()
