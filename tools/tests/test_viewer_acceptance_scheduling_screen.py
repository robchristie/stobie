"""Authored screen orchestration checks; no application, service, GPU or protected inputs."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('screen', ROOT / 'tools/viewer-acceptance-scheduling-screen.py')
screen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(screen)


class ScreenTests(unittest.TestCase):
    def setUp(self):
        self.protocol = copy.deepcopy(screen.load(screen.PROTOCOL))
        # Rebase only copied authored inputs; retain and verify their frozen hashes.
        historical_root = Path(self.protocol['workload']['path']).parents[2]
        for key in ('workload', 'thresholds'):
            record = self.protocol[key]
            path = ROOT / Path(record['path']).relative_to(historical_root)
            record['path'] = str(path)
            record['resolved_path'] = str(path.resolve())
            screen.cohort.verify(record)

    def rows(self):
        return [dict(**s, absolute_pass=True,
            observations={'warm_compressed_ms': dict(value=100 if s['arm'] == 'A' else 94, unit='ms'),
                          'first_useful_ms': dict(value=10, unit='ms')}, rss={'process_peak_rss_bytes': 100})
                for s in screen.slots()]

    def test_contract_and_same_binary_explicit_arms(self):
        screen.validate_contract(self.protocol)
        self.assertEqual(''.join(s['arm'] for s in screen.slots()), 'AABBAABBAAB')
        for slot in screen.slots():
            args = screen.arguments(self.protocol, Path('/authored'), slot, 'http://127.0.0.1:8199')
            self.assertEqual(args[args.index('--native-bin') + 1], self.protocol['native']['path'])
            self.assertEqual(args[-2:], ['--completion-pump', slot['completion_pump']])
            self.assertEqual(args[args.index('--workload') + 1], self.protocol['workload']['path'])
            self.assertEqual(args[args.index('--server-cache-state') + 1],
                             'warm-server' if slot['pair'] else 'uncontrolled-first-observation')

    def test_every_failed_slot_retained_without_retries(self):
        calls, retained = [], []
        def fail(slot):
            calls.append(slot['slot'])
            raise ValueError('authored launch/export failure')
        rows = screen.frozen_slots(fail, retained.append)
        self.assertEqual(len(rows), 11)
        self.assertEqual(calls, [s['slot'] for s in screen.slots()])
        self.assertEqual(rows, retained)
        self.assertTrue(all(r['launch'] is None and r['failures'] for r in rows))

    def test_mixed_outcomes_preserve_later_slots(self):
        def run(slot):
            if slot['slot'] == 'pair-01-B':
                raise OSError('authored failure')
            return dict(**slot, launch={'authored': True}, failures=[])
        rows = screen.frozen_slots(run, lambda _: None)
        self.assertEqual(len(rows), 11)
        self.assertTrue(rows[2]['failures'])
        self.assertTrue(rows[-1]['launch'])

    def test_grant_must_bind_committed_screen_and_completed_builds(self):
        p = self.protocol
        grant = dict(schema='viewer_scheduling_screen_grant/1', protocol_commit='full-commit',
            screen_id=p['screen_id'], execute=True, no_concurrent_builds=True, resource_compile_complete=True,
            authored_probe_owner_stopped=True, granted_by='authored main', granted_utc='authored time',
            output_name='viewer-acceptance-scheduling-screen-authored', display=':199', url='http://127.0.0.1:8199')
        screen.validate_grant(grant, 'full-commit', p)
        for field in ('protocol_commit', 'screen_id', 'execute', 'no_concurrent_builds', 'resource_compile_complete'):
            altered = dict(grant, **{field: None})
            with self.subTest(field=field), self.assertRaises(ValueError):
                screen.validate_grant(altered, 'full-commit', p)

    def test_preflight_identity_failure_never_launches(self):
        with patch.object(screen, 'validate_contract'), patch.object(screen, 'validate_grant'), \
                patch.object(screen.cohort, 'git', return_value=b'wrong-head\n'), \
                patch.object(screen, 'one_cell') as launch, patch.object(screen.cohort, 'module') as source:
            with self.assertRaisesRegex(ValueError, 'commit before execution'):
                screen.execute(self.protocol, {}, 'expected-head', Path('/authored/grant'))
            launch.assert_not_called()
            source.assert_not_called()
        with patch.object(screen, 'validate_grant'), patch.object(screen, 'OWNED', []), \
                patch.object(screen.cohort, 'git', side_effect=[b'expected-head\n', b'']), \
                patch.object(screen.cohort, 'verify', side_effect=ValueError('identity drift')), \
                patch.object(screen, 'one_cell') as launch, patch.object(screen.cohort, 'module') as source:
            with self.assertRaisesRegex(ValueError, 'identity drift'):
                screen.execute(self.protocol, {}, 'expected-head', Path('/authored/grant'))
            launch.assert_not_called()
            source.assert_not_called()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'authored-binary'
            path.write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'identity drift'):
                screen.cohort.verify(dict(path=str(path), sha256='0' * 64))

    def test_strict_improvement_recovery_remains_unproved(self):
        rows = self.rows()
        result = screen.decision(rows, {'bounds': {}})
        self.assertTrue(result['native_screen_promising'])
        self.assertFalse(result['acceptance'])
        self.assertFalse(result['confirmation_authorised'])
        for value in (95, 96):
            for row in rows:
                if row['arm'] == 'B':
                    row['observations']['warm_compressed_ms']['value'] = value
            self.assertEqual(screen.decision(rows, {'bounds': {}})['disposition'], 'reject')

    def test_failures_missing_cells_and_regressions_reject(self):
        for case in ('conditioning-failed', 'cell-missing', 'latency-missing', 'detail-regression', 'resource-regression'):
            rows = self.rows()
            if case == 'conditioning-failed':
                rows[0]['absolute_pass'] = False
            elif case == 'cell-missing':
                rows.pop()
            elif case == 'latency-missing':
                rows[2]['observations']['first_useful_ms']['value'] = None
            elif case == 'detail-regression':
                for row in rows:
                    row['observations']['target_detail_ms'] = dict(value=106 if row['arm'] == 'B' else 100, unit='ms')
            else:
                for row in rows:
                    row['rss']['process_peak_rss_bytes'] = 106 if row['arm'] == 'B' else 100
            with self.subTest(case=case):
                self.assertEqual(screen.decision(rows, {'bounds': {}})['disposition'], 'reject')

    def test_diagnostic_differences_keep_negative_and_missing_values(self):
        event = dict(token={'authored': 1}, pacing=dict(dispatch_ms=2, worker_started_ms=1,
            worker_finished_ms=3, published_ms=4, received_ms=3, ui_drained_ms=5))
        stage = dict(runtime_epoch=1, phase_label='authored', events=[event])
        result = screen.diagnostic_summary([stage, copy.deepcopy(stage)])
        self.assertEqual(len(result['retained_same_clock_differences']), 1)
        differences = result['retained_same_clock_differences'][0]['differences_ms']
        self.assertEqual(differences['dispatch_to_worker_ms'], -1)
        self.assertEqual(differences['publish_to_receive_ms'], -1)


if __name__ == '__main__':
    unittest.main()
