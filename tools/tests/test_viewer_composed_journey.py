"""Protect recovery measurements across fresh browser counter resets."""
import importlib.util
from pathlib import Path
import unittest


SPEC = importlib.util.spec_from_file_location(
    'viewer_composed_journey', Path(__file__).resolve().parents[1] / 'viewer-composed-journey.py')
JOURNEY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(JOURNEY)


def state(context, requests, body_bytes, samples, resource_bytes, dropped):
    return {'context_id': context, 'page_id': context + '-page', 'snapshot': {
        'events_dropped': dropped,
        'worker': {'requests': requests, 'received_jpp_bytes': body_bytes,
                   'synthesis_output_samples': samples,
                   'peak_compressed_bytes': resource_bytes,
                   'wasm_linear_bytes': resource_bytes * 2}}}


class RecoveryMeasurementTests(unittest.TestCase):
    def test_context_resets_add_work_but_not_resource_peaks(self):
        states = [state('transport', 3, 90, 100, 200, 2),
                  state('transport', 5, 150, 180, 240, 4),
                  state('pressure', 1, 20, 30, 100, 0),
                  state('pressure', 2, 60, 80, 120, 1)]
        # Include the duplicated final snapshot used by the exporter: it must
        # neither double-count the last context nor hide the previous context.
        snapshots = [states[-1]['snapshot']] + [s['snapshot'] for s in states]
        for field, expected in [('requests', 7), ('received_jpp_bytes', 210),
                                ('synthesis_output_samples', 260)]:
            with self.subTest(field=field):
                value, boundary = JOURNEY.worker_observation(field, snapshots, {'states': states})
                self.assertEqual(value, expected)
                self.assertIn('sum of per-context sampled cumulative maxima', boundary)
        for field, expected in [('peak_compressed_bytes', 240), ('wasm_linear_bytes', 480)]:
            with self.subTest(field=field):
                value, boundary = JOURNEY.worker_observation(field, snapshots, {'states': states})
                self.assertEqual(value, expected)
                self.assertIn('maximum across sampled sequential fresh browser contexts', boundary)
        self.assertEqual(JOURNEY.recovery_counter(states, 'events_dropped', worker=False), 5)
        # Normal mode retains both its value and frozen boundary text.
        self.assertEqual(JOURNEY.worker_observation('requests', snapshots),
                         (5, 'shared worker requests'))
        self.assertEqual(JOURNEY.worker_observation('peak_compressed_bytes', snapshots),
                         (240, 'shared worker peak_compressed_bytes'))

    def test_unidentified_or_unobserved_context_is_not_a_partial_total(self):
        first = state('transport', 5, 150, 180, 240, 4)
        second = state('pressure', 2, 60, 80, 120, 1)
        del second['snapshot']['worker']['requests']
        self.assertIsNone(JOURNEY.recovery_counter([first, second], 'requests'))
        del first['context_id']
        self.assertIsNone(JOURNEY.recovery_counter([first], 'received_jpp_bytes'))
        self.assertIsNone(JOURNEY.recovery_counter([], 'requests'))

    def test_recovery_browser_attribution_uses_actual_recovery_record(self):
        runtime, workers, boundary, _ = JOURNEY.browser_attribution(
            'recovery', {}, {'browser_version': '131.0.6778.33', 'workers': 3})
        self.assertEqual(runtime, 'recovery 131.0.6778.33')
        self.assertEqual(workers, 3)
        self.assertIn('total Playwright Worker creation events', boundary)
        self.assertIn('not concurrent workers', boundary)
        missing_runtime, missing_workers, _, missing_reason = JOURNEY.browser_attribution('recovery', {}, {})
        self.assertEqual(missing_runtime, 'recovery browser version unavailable')
        self.assertIsNone(missing_workers)
        self.assertNotIn('native', missing_reason)
        self.assertEqual(JOURNEY.browser_attribution('browser', {'browser_version': '131', 'workers': 1}, {})[:3],
                         ('browser 131', 1, 'Playwright Worker creation events'))
        self.assertEqual(JOURNEY.browser_attribution('native', {}, {})[0], 'native native eframe/wgpu')


if __name__ == '__main__':
    unittest.main()
