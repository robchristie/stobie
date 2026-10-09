import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("ci", Path(__file__).resolve().parents[1] / "ci.py")
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


class GateTests(unittest.TestCase):
    def setUp(self):
        self.needs = {"scope": {"result": "success", "outputs": {"mode": "full"}},
                      "qualification": {"result": "success"}, "documentation": {"result": "skipped"}}
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.evidence = Path(self.temporary.name)
        for stage, apps, hosts in (("native-lab", ["lab"], ["native"]),
                                   ("browser", ["lab"], ["browser"]),
                                   ("record-desk", ["record-desk"], ["browser", "native"])):
            path = self.evidence / f"interface-{stage}/application-interface/summary.json"
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({"revision": "candidate", "dirty": False, "slice_only": False,
                                       "selection": {"apps": apps, "hosts": hosts},
                                       "reports": [{"app": app, "host": host} for app in apps for host in hosts]}))

    def test_full_requires_all_lanes_and_attributed_complete_journeys(self):
        self.assertEqual(ci.gate(self.needs, self.evidence, "candidate"), "full")
        for job in ("scope", "qualification", "documentation"):
            for result in ("failure", "cancelled", "skipped", "success"):
                if result == self.needs[job]["result"]:
                    continue
                needs = copy.deepcopy(self.needs)
                needs[job]["result"] = result
                with self.subTest(job=job, result=result), self.assertRaises(ValueError):
                    ci.gate(needs, self.evidence, "candidate")
        with self.assertRaises(ValueError):
            ci.gate(self.needs, self.evidence, "another-revision")
        with self.assertRaises(ValueError):
            ci.gate(self.needs)

    def test_missing_or_incomplete_journey_is_not_a_pass(self):
        path = self.evidence / "interface-record-desk/application-interface/summary.json"
        summary = json.loads(path.read_text())
        for reports in (summary["reports"][:1], summary["reports"] * 2):
            path.write_text(json.dumps({**summary, "reports": reports}))
            with self.assertRaises(ValueError):
                ci.gate(self.needs, self.evidence, "candidate")
        for key, value in (("dirty", True), ("slice_only", True)):
            path.write_text(json.dumps({**summary, key: value}))
            with self.assertRaises(ValueError):
                ci.gate(self.needs, self.evidence, "candidate")
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            ci.gate(self.needs, self.evidence, "candidate")

    def test_only_successful_docs_classification_can_use_scoped_route(self):
        needs = copy.deepcopy(self.needs)
        needs["scope"]["outputs"]["mode"] = "docs"
        needs["qualification"]["result"] = "skipped"
        needs["documentation"]["result"] = "success"
        self.assertEqual(ci.gate(needs), "docs")
        for mode in ("", "unknown", None):
            needs["scope"]["outputs"]["mode"] = mode
            with self.assertRaises(ValueError):
                ci.gate(needs)

    def test_inventory_has_unique_stages_and_boolean_setup_requirements(self):
        stages = ci.matrix()["include"]
        self.assertEqual(len(stages), len({stage["name"] for stage in stages}))
        self.assertTrue(all(isinstance(stage["browser"], bool) for stage in stages))


if __name__ == "__main__":
    unittest.main()
