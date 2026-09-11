"""Authored/mock checks only: never open a display or launch an application."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location('cohort', Path(__file__).resolve().parents[1] / 'viewer-acceptance-native-cohort.py')
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)


def authored_protocol(path):
    """Rebase copied repository inputs without rewriting historical evidence."""
    protocol = copy.deepcopy(C.load(path))
    historical_root = Path(protocol['workload']['path']).parents[2]
    for key in ('workload', 'thresholds', 'inherited_thresholds', 'build_receipt'):
        record = protocol[key]
        record['path'] = str(C.ROOT / Path(record['path']).relative_to(historical_root))
        C.verify(record)
    for record in protocol['frozen_helpers']:
        record['path'] = str(C.ROOT / Path(record['path']).relative_to(historical_root))
    return protocol


class CohortTests(unittest.TestCase):
    def test_default_and_explicit_repaired_protocol(self):
        original_fixture = authored_protocol(C.PROTOCOL)
        repaired_fixture = authored_protocol(C.REPAIRED_PROTOCOL)
        receipt_path = Path(repaired_fixture['build_receipt']['path'])
        receipt = C.load(receipt_path)
        # Reconstruct only committed manifest metadata, never build outputs.
        # Its historical digest is still checked by select_protocol's real verify.
        manifest = [dict(row, path=str(Path(row['path']).relative_to(repaired_fixture['web_root'])))
                    for row in repaired_fixture['web_files']]
        real_load = C.load
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = Path(temporary) / 'web-manifest.json'
            manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
            receipt['web']['manifest']['path'] = str(manifest_path)
            fixtures = {C.PROTOCOL: original_fixture, C.REPAIRED_PROTOCOL: repaired_fixture,
                        receipt_path: receipt}

            def load(path, *args):
                if Path(path) in fixtures:
                    return copy.deepcopy(fixtures[Path(path)])
                return real_load(path, *args)

            with patch.object(C, 'load', side_effect=load):
                path, original = C.select_protocol()
                repaired_path, repaired = C.select_protocol(C.REPAIRED_PROTOCOL)
        self.assertEqual(path, C.PROTOCOL)
        self.assertEqual(original, original_fixture)
        self.assertEqual(repaired_path, C.REPAIRED_PROTOCOL)
        self.assertEqual(repaired['runtime_revision'], '8633754fa28f2ca34f159a7368e0b8e7e953d205')
        for field in original.keys() - C.IDENTITY_FIELDS - {'frozen_helpers'}:
            self.assertEqual(repaired[field], original[field], field)
        self.assertEqual(repaired['frozen_helpers'][1:], original['frozen_helpers'][1:])
        self.assertEqual(repaired['frozen_helpers'][0]['path'], original['frozen_helpers'][0]['path'])
        with self.assertRaisesRegex(ValueError, 'explicitly pinned'):
            C.select_protocol('/authored/arbitrary-protocol.json')

    def test_alternate_rejects_every_policy_leaf_change(self):
        repaired = C.load(C.REPAIRED_PROTOCOL)
        real_load = C.load

        def leaves(value, prefix=()):
            if isinstance(value, dict):
                for key, child in value.items():
                    yield from leaves(child, (*prefix, key))
            elif isinstance(value, list):
                for key, child in enumerate(value):
                    yield from leaves(child, (*prefix, key))
            else:
                yield prefix, value

        for field in repaired.keys() - C.IDENTITY_FIELDS:
            for keys, value in leaves(repaired[field], (field,)):
                changed = copy.deepcopy(repaired)
                target = changed
                for key in keys[:-1]:
                    target = target[key]
                target[keys[-1]] = value + 1 if type(value) in (int, float) else 'authored drift'
                with self.subTest(keys=keys), patch.object(C, 'load', side_effect=lambda path:
                        changed if path == C.REPAIRED_PROTOCOL else real_load(path)):
                    with self.assertRaisesRegex(ValueError, 'original policy drift'):
                        C.select_protocol(C.REPAIRED_PROTOCOL)

    def test_alternate_rejects_original_replacement_and_new_fields(self):
        repaired = C.load(C.REPAIRED_PROTOCOL)
        with patch.object(C, 'identity', return_value={'sha256': '0' * 64}):
            with self.assertRaisesRegex(ValueError, 'original protocol identity drift'):
                C.select_protocol(C.REPAIRED_PROTOCOL)
        real_load = C.load
        repaired['extra_budget'] = 99
        with patch.object(C, 'load', side_effect=lambda path:
                repaired if path == C.REPAIRED_PROTOCOL else real_load(path)):
            with self.assertRaisesRegex(ValueError, 'fields drift'):
                C.select_protocol(C.REPAIRED_PROTOCOL)

    def test_repaired_grant_binds_path_digest_and_fixed_output(self):
        grant = dict(schema='viewer_native_cohort_grant/1', protocol_commit='a' * 40,
                     output_name=C.REPAIRED_OUTPUT, display=':177', port=8194,
                     execute=True, authored_probe_owner_stopped=True,
                     granted_by='authored coordinator', granted_utc='authored timestamp',
                     protocol_path=str(C.REPAIRED_PROTOCOL.relative_to(C.ROOT)),
                     protocol_sha256=C.identity(C.REPAIRED_PROTOCOL)['sha256'])
        args = ('a' * 40, C.REPAIRED_OUTPUT, ':177', 8194, C.REPAIRED_PROTOCOL)
        C.validate_grant(grant, *args)
        for key in ('protocol_path', 'protocol_sha256'):
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'selected protocol'):
                C.validate_grant(dict(grant, **{key: 'wrong'}), *args)
        with self.assertRaisesRegex(ValueError, 'fixed repaired output'):
            C.validate_grant(dict(grant, output_name='different'), 'a' * 40,
                             'different', ':177', 8194, C.REPAIRED_PROTOCOL)

    def test_repaired_committed_gate_covers_receipt_and_both_protocols(self):
        names = []

        def git(_root, *args):
            if args == ('rev-parse', 'HEAD'):
                return b'abc\n'
            names.append(args[1].split(':', 1)[1])
            return b'committed'

        with patch.object(C, 'git', side_effect=git), patch.object(C, 'read', return_value=b'committed'), \
                patch.object(C, 'identity', return_value={}):
            C.committed('abc', C.REPAIRED_PROTOCOL)
        self.assertEqual(set(names), set(C.OWNED) | {
            'docs/viewer-acceptance-native-repaired-cohort.json',
            'docs/viewer-acceptance-native-repaired-cohort.md',
            'docs/viewer-acceptance-repaired-build.json'})
        with patch.object(C, 'git', side_effect=git), patch.object(C, 'read', side_effect=lambda path:
                b'changed' if path == C.REPAIRED_PROTOCOL else b'committed'):
            with self.assertRaisesRegex(ValueError, 'uncommitted protocol'):
                C.committed('abc', C.REPAIRED_PROTOCOL)

    def test_five_slots_survive_each_failed_start_without_retries(self):
        calls = []

        def run(index):
            calls.append(index)
            if index in (1, 3, 5):
                raise OSError('authored failed app start')
            return dict(index=index, harness_exit=4, failures=['authored incomplete cycles'])

        rows = C.five_slots(run)
        self.assertEqual(calls, [1, 2, 3, 4, 5])
        self.assertEqual([r['index'] for r in rows], calls)
        self.assertTrue(all(r['failures'] for r in rows))

    def test_grant_requires_committed_identity_and_stopped_owner(self):
        grant = dict(schema='viewer_native_cohort_grant/1', protocol_commit='a' * 40,
                     output_name='viewer-acceptance-native-cohort-authored', display=':177',
                     port=8194, execute=True, authored_probe_owner_stopped=True,
                     granted_by='authored coordinator', granted_utc='authored timestamp')
        args = ('a' * 40, grant['output_name'], ':177', 8194)
        C.validate_grant(grant, *args)
        for key in grant:
            with self.subTest(key=key):
                changed = {k: v for k, v in grant.items() if k != key}
                with self.assertRaises(ValueError):
                    C.validate_grant(changed, *args)

    def test_committed_gate_rejects_uncommitted_runner(self):
        with patch.object(C, 'git', side_effect=[b'abc\n', b'committed']), \
                patch.object(C, 'read', return_value=b'changed'):
            with self.assertRaisesRegex(ValueError, 'uncommitted protocol'):
                C.committed('abc')

    def test_contract_preserves_every_inherited_number_and_cycles(self):
        protocol = authored_protocol(C.PROTOCOL)
        C.validate_contract(protocol)
        for key in protocol['budget']:
            changed = copy.deepcopy(protocol)
            changed['budget'][key] += 1
            with self.assertRaisesRegex(ValueError, 'budget drift'):
                C.validate_contract(changed)
        changed = copy.deepcopy(protocol)
        changed['captures']['offset_seconds'] = [1, 5]
        with self.assertRaisesRegex(ValueError, 'capture schedule'):
            C.validate_contract(changed)

    def test_native_arguments_bind_existing_harness_and_fixed_workload(self):
        protocol = C.load(C.PROTOCOL)
        repaired = C.load(C.REPAIRED_PROTOCOL)
        for index in range(1, 6):
            args = C.journey_arguments(protocol, Path('/authored/native'), 'http://127.0.0.1:8194', index)
            alternate = C.journey_arguments(repaired, Path('/authored/native'), 'http://127.0.0.1:8194', index)
            for flag in ('--native-bin', '--web-root'):
                alternate[alternate.index(flag) + 1] = args[args.index(flag) + 1]
            self.assertEqual(alternate, args)
            self.assertEqual(args[args.index('--mode') + 1], 'native')
            self.assertEqual(args[args.index('--workload') + 1], protocol['workload']['path'])
            self.assertEqual(args[args.index('--thresholds') + 1], protocol['thresholds']['path'])
            self.assertIn('--memory-diagnostics', args)
            self.assertNotIn('--authored-immediate-completion', args)
            self.assertNotIn('--completion-pump', args)
            self.assertEqual(args[args.index('--server-cache-state') + 1],
                             'uncontrolled-first-observation' if index == 1 else 'warm-server')

    def test_observer_launches_actual_binary_once_and_reports_clock_interval(self):
        connection = Mock()
        observer = C.LaunchObserver(connection, '/authored/native')
        child = Mock(pid=42)
        with patch.object(C.subprocess, 'Popen', return_value=child) as popen, \
                patch.object(C, 'process_identity', return_value=dict(pid=42, start_time_ticks=7, process_group=42)), \
                patch.object(C.time, 'monotonic_ns', side_effect=[100, 110]), \
                patch.object(C.time, 'time_ns', return_value=1000):
            self.assertIs(observer.Popen(['/authored/native'], start_new_session=True), child)
            with self.assertRaisesRegex(ValueError, 'one native launch'):
                observer.Popen(['/authored/native'])
            popen.assert_called_once_with(['/authored/native'], start_new_session=True)
        record = connection.send.call_args.args[0]
        self.assertEqual((record['launch_monotonic_ns'], record['popen_return_monotonic_ns']), (100, 110))

    def test_capture_targets_owned_root_with_hard_bounds(self):
        command = C.capture_command('/authored/import', ':177', Path('/approved/run/capture.png'))
        self.assertEqual(command[:5], ['/authored/import', '-display', ':177', '-window', 'root'])
        self.assertIn('PNG24:/approved/run/capture.png', command)
        self.assertEqual(command[command.index('-snaps') + 1], '1')
        with patch.object(C.resource, 'setrlimit') as limit:
            C.capture_limits()
        self.assertIn(((C.resource.RLIMIT_FSIZE, (8 << 20, 8 << 20)),),
                      [(call.args,) for call in limit.call_args_list])

    def test_exited_native_does_not_capture_stale_display_or_retry(self):
        with patch.object(C, 'alive', return_value=False), patch.object(C.subprocess, 'Popen') as launch:
            for offset in C.OFFSETS:
                row, process = C.begin_capture({}, Path('/authored'), ':177',
                    dict(identity={'pid': 42}, launch_monotonic_ns=0), offset)
                self.assertIsNone(process)
                self.assertIn('no retry', row['failure'])
            launch.assert_not_called()

    def test_capture_timeout_retains_partial_image_and_does_not_retry(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'partial.png'
            path.write_bytes(b'authored partial')
            row = dict(attempted_monotonic_ns=0, capture_identity={'pid': 42}, image=str(path))
            child = Mock()
            child.poll.return_value = None
            child.wait.return_value = -9
            with patch.object(C.time, 'monotonic_ns', return_value=2_000_000_000), \
                    patch.object(C, 'kill_owned') as kill:
                self.assertTrue(C.finish_capture(row, child))
                kill.assert_called_once_with({'pid': 42})
            self.assertEqual(path.read_bytes(), b'authored partial')
            self.assertEqual(row['exit_code'], -9)
            self.assertIn('deadline', row['failure'])

    def test_cleanup_refuses_pid_reuse_or_unowned_group(self):
        record = dict(pid=42, start_time_ticks=7, process_group=42)
        with patch.object(C, 'process_identity', return_value=dict(record, start_time_ticks=8)), \
                patch.object(C, 'read', return_value=b'42 (authored) S 1 42'), \
                patch.object(C.os, 'killpg') as kill:
            C.kill_owned(record)
            kill.assert_not_called()
        with patch.object(C, 'alive', return_value=True), patch.object(C.os, 'killpg') as kill:
            with self.assertRaisesRegex(ValueError, 'unowned group'):
                C.kill_owned(dict(record, process_group=1))
            kill.assert_not_called()

    def test_zombie_native_is_unavailable_for_capture(self):
        with patch.object(C, 'read', return_value=b'42 (authored) Z 1 42'), \
                patch.object(C, 'process_identity') as identity:
            self.assertFalse(C.alive({'pid': 42}))
            identity.assert_not_called()

    def test_outer_deadline_kills_native_at_660_without_extra_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            now, living, kills = [0], [True], []
            native = dict(pid=42, start_time_ticks=7, process_group=42)
            launch = dict(identity=native, launch_monotonic_ns=0)
            receiver, sender, worker, context = Mock(), Mock(), Mock(), Mock()
            receiver.poll.return_value = True
            receiver.recv.return_value = launch
            worker.is_alive.side_effect = lambda: now[0] < 665 * 10**9
            worker.exitcode = 4
            context.Pipe.return_value = (receiver, sender)
            context.Process.return_value = worker

            def advance(_seconds):
                now[0] += 10**9

            def kill(_identity, *args):
                if living[0]:
                    kills.append(now[0])
                    living[0] = False

            def capture(_protocol, _path, _display, _launch, offset):
                return dict(offset_seconds=offset, failure='authored capture unavailable'), None

            with patch.object(C.multiprocessing, 'get_context', return_value=context), \
                    patch.object(C.time, 'monotonic_ns', side_effect=lambda: now[0]), \
                    patch.object(C.time, 'sleep', side_effect=advance), \
                    patch.object(C, 'alive', side_effect=lambda _: living[0]), \
                    patch.object(C, 'kill_owned', side_effect=kill), \
                    patch.object(C, 'begin_capture', side_effect=capture) as captures, \
                    patch.object(C, 'mapped_files', return_value={'unavailable': 'authored'}):
                result = C.one_run(C.load(C.PROTOCOL), Path(temporary), 1, ':177', 'http://authored')
            self.assertEqual(kills, [660 * 10**9])
            self.assertEqual([c.args[-1] for c in captures.call_args_list], [2, 5])
            worker.start.assert_called_once()
            self.assertEqual(result['harness_exit'], 4)
            self.assertIn('660-second', result['failures'][0])
    def test_no_missing_trace_is_manufactured_for_benchmark(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(C.subprocess, 'run') as run:
            group = Path(temporary)
            report = C.assess({'benchmark': {'path': '/authored/benchmark'}}, group, 1)
            run.assert_not_called()
            self.assertIn('no synthetic', report['benchmark']['unavailable'])
            self.assertIsNone(report['current_growth'])
            self.assertIsNone(report['rss']['process_peak_rss_bytes']['within_ceiling'])

    def test_growth_requires_all_ten_bracketed_cycles_and_keeps_rss_gate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            samples, attachments = [], []
            for i in range(10):
                for j in range(2):
                    samples.append(dict(identity={'value': {'pid': 42, 'start_time_ticks': 7}},
                        status={'value': {'VmRSS': {'value': 1000 + i * 10 + j}, 'VmHWM': {'value': 999999}}},
                        read_started_monotonic_ns=i * 10 + j, read_finished_monotonic_ns=i * 10 + j,
                        smaps_rollup={'value': None}, mapping_categories={'value': None}))
                attachments.append(dict(marker=dict(kind='cycle-revisited', phase_label=f'cycle-{i+1:02}',
                    pid=42, start_time_ticks=7), before_sample={'value': 2*i}, after_sample={'value': 2*i+1}))
            diagnostic = dict(samples=samples, phase_attachments=attachments)
            (root / 'process-memory-diagnostics.json').write_text(json.dumps(diagnostic))
            (root / 'composed-trace.json').write_text(json.dumps(dict(observations={
                'process_peak_rss_bytes': {'value': 278794240},
                'process_group_sampled_peak_rss_bytes': {'value': 250000000},
                'observed_process_high_water_sum_bytes': {'value': 280000000}})))
            report = C.summarise(root)
            self.assertEqual(report['current_growth']['last_minus_first_bytes'], 90)
            self.assertEqual(len(report['cycle_brackets']), 10)
            self.assertTrue(all(v['within_ceiling'] is False for v in report['rss'].values()))
            self.assertIn('never add or subtract', report['boundary'])
            attachments[4]['before_sample'] = dict(value=None, unavailable_reason='authored gap')
            (root / 'process-memory-diagnostics.json').write_text(json.dumps(diagnostic))
            self.assertIsNone(C.summarise(root)['current_growth'])

    def test_failed_trace_export_still_reports_actual_sampler_rss(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'process-memory.json').write_text(json.dumps(dict(
                samples=[{'rss_bytes': 250000000}, {'rss_bytes': 210000000}],
                observed_pid_high_water_bytes={'42': 278794240})))
            (root / 'app.json').write_text(json.dumps(dict(process_peak_rss_bytes=278794240)))
            result = C.summarise(root)
            self.assertEqual(result['rss']['process_group_sampled_peak_rss_bytes']['bytes'], 250000000)
            self.assertEqual(result['rss']['observed_process_high_water_sum_bytes']['bytes'], 278794240)
            self.assertTrue(all(g['within_ceiling'] is False for g in result['rss'].values()))
            self.assertFalse((root / 'composed-trace.json').exists())


if __name__ == '__main__':
    unittest.main()
