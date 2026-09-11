"""Authored protocol freeze checks; no app, assets, service or timing run."""
import hashlib
import json
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SchedulingProtocolTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        text = (ROOT / 'docs/viewer-acceptance-scheduling.md').read_text()
        capsules = re.findall(r'```json\n(.*?)\n```', text, re.DOTALL)
        if len(capsules) != 1:
            raise AssertionError('exactly one scheduling protocol capsule required')
        cls.protocol = json.loads(capsules[0])

    def test_one_opt_in_candidate_with_fixed_bounds(self):
        p = self.protocol
        self.assertEqual(p['schema'], 'viewer-acceptance-scheduling/1')
        self.assertEqual(p['candidates'], 1)
        self.assertIs(p['native_only'], True)
        self.assertIs(p['default_completion_pump'], False)
        self.assertEqual((p['turn_budget_ms'], p['completion_limit']), (4, 64))

    def test_predeclared_pair_orders_and_no_extra_runs(self):
        p = self.protocol
        self.assertEqual(p['screen_pairs'], ['AB', 'BA', 'AB', 'BA', 'AB'])
        self.assertEqual(p['confirmation_pairs'], ['AB', 'BA'] * 10)
        self.assertEqual(p['conditioning_journeys_per_phase'], 1)
        self.assertEqual(p['conditioning_arm'], 'A')
        self.assertEqual((p['warmups'], p['retries']), (0, 0))
        self.assertEqual((p['confidence_percent'], p['practical_percent']), (99, 5))

    def test_normal_workload_and_all_native_pressure_recovery_gates_are_frozen(self):
        expected = {
            'apps/emuella-viewer/real-scene-workload.json',
            'apps/emuella-viewer/qualification/real-scene-native-thresholds.json',
            'apps/emuella-viewer/qualification/real-scene-pressure-thresholds.json',
            'apps/emuella-viewer/qualification/real-scene-recovery-thresholds.json',
        }
        self.assertEqual(set(self.protocol['inputs']), expected)
        for name, digest in self.protocol['inputs'].items():
            with self.subTest(name=name):
                self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest)


if __name__ == '__main__':
    unittest.main()
