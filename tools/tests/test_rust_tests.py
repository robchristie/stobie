import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("rust_tests", Path(__file__).resolve().parents[1] / "test-rust.py")
rust_tests = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rust_tests)


class RustTestRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        binary = self.root / "bin"
        binary.mkdir()
        cargo = binary / "cargo"
        cargo.write_text("""#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys
arguments = sys.argv[1:]
root = Path(os.environ['HARNESS_ROOT'])
with (root / 'calls.jsonl').open('a') as calls:
    calls.write(json.dumps(arguments) + '\\n')
with (root / 'cwd.jsonl').open('a') as calls:
    calls.write(json.dumps(str(Path.cwd())) + '\\n')
if arguments[:2] == ['nextest', 'show-config']:
    sys.exit(int(os.environ.get('PREFLIGHT_EXIT', '0')))
if arguments[:2] == ['nextest', 'run']:
    selection = arguments[arguments.index('--profile') + 1]
    report = root / '.tools/runtime/verification-evidence/nextest' / selection / 'junit.xml'
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text('<testsuites name="' + selection + '"/>')
    sys.exit(int(os.environ.get('TEST_EXIT', '0')))
sys.exit(int(os.environ.get('DOC_EXIT', '0')))
""")
        cargo.chmod(0o755)
        environment = {**os.environ, "PATH": str(binary) + os.pathsep + os.environ["PATH"],
                       "HARNESS_ROOT": str(self.root)}
        self.environment = patch.dict(os.environ, environment, clear=True)
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def report(self, selection):
        return self.root / rust_tests.REPORTS / selection / "junit.xml"

    def calls(self):
        return [json.loads(line) for line in (self.root / "calls.jsonl").read_text().splitlines()]

    def test_selections_retain_features_and_separate_doctests_and_reports(self):
        rust_tests.run("workspace", self.root)
        workspace_report = self.report("workspace").read_text()
        rust_tests.run("ui-no-default", self.root)
        self.assertEqual(self.report("workspace").read_text(), workspace_report)
        self.assertNotEqual(self.report("ui-no-default").read_text(), workspace_report)
        self.assertEqual(self.calls(), [
            ["nextest", "show-config", "version"],
            ["nextest", "run", "--profile", "workspace", "--workspace"],
            ["test", "--doc", "--workspace"],
            ["nextest", "show-config", "version"],
            ["nextest", "run", "--profile", "ui-no-default", "-p", "polyorama-ui-egui", "--no-default-features"],
            ["test", "--doc", "-p", "polyorama-ui-egui", "--no-default-features"],
        ])

    def test_prepare_removes_only_the_three_owned_reports(self):
        for selection in ("workspace", "ui-no-default", "record-desk", "other"):
            self.report(selection).parent.mkdir(parents=True, exist_ok=True)
            self.report(selection).write_text("old report")
        unrelated = self.report("workspace").parent / "diagnostic.txt"
        unrelated.write_text("keep")
        rust_tests.clear_reports(self.root)
        self.assertFalse(self.report("workspace").exists())
        self.assertFalse(self.report("ui-no-default").exists())
        self.assertFalse(self.report("record-desk").exists())
        self.assertEqual(self.report("other").read_text(), "old report")
        self.assertEqual(unrelated.read_text(), "keep")

    def test_preflight_failure_removes_stale_report_without_running_tests(self):
        self.report("workspace").parent.mkdir(parents=True)
        self.report("workspace").write_text("stale success")
        with patch.dict(os.environ, {"PREFLIGHT_EXIT": "2"}):
            with self.assertRaises(subprocess.CalledProcessError) as failure:
                rust_tests.run("workspace", self.root)
        self.assertEqual(failure.exception.returncode, 2)
        self.assertFalse(self.report("workspace").exists())
        self.assertEqual(self.calls(), [["nextest", "show-config", "version"]])

    def test_test_failure_propagates_and_keeps_the_failed_report(self):
        with patch.dict(os.environ, {"TEST_EXIT": "100"}):
            with self.assertRaises(subprocess.CalledProcessError) as failure:
                rust_tests.run("workspace", self.root)
        self.assertEqual(failure.exception.returncode, 100)
        self.assertTrue(self.report("workspace").exists())
        self.assertEqual(len(self.calls()), 2)

    def test_doctest_failure_propagates_and_keeps_both_reports(self):
        rust_tests.run("workspace", self.root)
        with patch.dict(os.environ, {"DOC_EXIT": "101"}):
            with self.assertRaises(subprocess.CalledProcessError) as failure:
                rust_tests.run("ui-no-default", self.root)
        self.assertEqual(failure.exception.returncode, 101)
        self.assertTrue(self.report("workspace").exists())
        self.assertTrue(self.report("ui-no-default").exists())

    def test_consumer_uses_its_own_locked_workspace_and_distinct_report(self):
        consumer = self.root / "consumers/record-desk"
        consumer.mkdir(parents=True)
        rust_tests.run("workspace", self.root)
        rust_tests.run("ui-no-default", self.root)
        rust_tests.run("record-desk", self.root)
        self.assertEqual(self.calls()[-3:], [
            ["nextest", "show-config", "version"],
            ["nextest", "run", "--profile", "record-desk", "--locked"],
            ["test", "--doc", "--locked"],
        ])
        self.assertEqual([json.loads(line) for line in (self.root / "cwd.jsonl").read_text().splitlines()][-3:],
                         [str(consumer)] * 3)
        rust_tests.clear_reports(self.root, ["record-desk"])
        self.assertFalse(self.report("record-desk").exists())
        self.assertTrue(self.report("workspace").exists())
        self.assertTrue(self.report("ui-no-default").exists())


if __name__ == "__main__":
    unittest.main()
