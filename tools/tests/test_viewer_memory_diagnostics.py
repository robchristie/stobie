"""Authored proc fixtures: identity, accounting boundaries and bounded retention."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    'viewer_memory_diagnostics', Path(__file__).resolve().parents[1] / 'viewer-memory-diagnostics.py')
MEMORY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MEMORY)


def stat(pid=42, start=100, parent=1):
    return f'{pid} (authored ) worker) S {parent} ' + '0 ' * 17 + str(start) + ' 0 0\n'


def mapping(path='', start=4096, rss=3):
    return (f'{start:08x}-{start + 4096:08x} rw-p 00000000 00:00 0 {path}\n'
            f'Size: 4 kB\nRss: {rss} kB\nPss: 2 kB\nPrivate_Clean: 0 kB\n'
            'Private_Dirty: 1 kB\nShared_Clean: 2 kB\nShared_Dirty: 0 kB\n'
            'Anonymous: 1 kB\nSwap: 0 kB\nSwapPss: 0 kB\nVmFlags: rd wr\n')


def fixture(root, pid=42, start=100, rss=7, hwm=9):
    directory = root / str(pid)
    directory.mkdir(exist_ok=True)
    (directory / 'stat').write_text(stat(pid, start))
    (directory / 'status').write_text(f'Name: authored\nPPid: 1\nVmRSS: {rss} kB\nVmHWM: {hwm} kB\n')
    (directory / 'smaps_rollup').write_text(mapping('[rollup]'))
    (directory / 'smaps').write_text(mapping('[heap]') + mapping('/authored/libfake.so.1', start=8192))
    return directory


def marker(sequence=0, at=2_000_000_000, start=100):
    return {'schema': 'viewer_memory_phase_marker/1', 'clock': 'linux_monotonic',
            'pid': 42, 'start_time_ticks': start, 'monotonic_ns': at,
            'sequence': sequence, 'phase_label': 'cycle-01', 'kind': 'cycle-revisited'}


class MemoryParserTests(unittest.TestCase):
    def test_stat_with_parentheses_uses_field_22(self):
        self.assertEqual(MEMORY.parse_stat(stat(), 42),
                         {'pid': 42, 'ppid': 1, 'start_time_ticks': 100})
        for raw, pid in [(stat(), 43), ('42 (short) S 1', 42), (stat(start=-1), 42)]:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                MEMORY.parse_stat(raw, pid)

    def test_missing_fields_are_not_zero_and_units_are_checked(self):
        values = MEMORY.parse_kib_fields('VmRSS: 0 kB\n', ('VmRSS', 'VmHWM'))
        self.assertEqual(values['VmRSS']['value'], 0)
        self.assertIsNone(values['VmHWM']['value'])
        self.assertEqual(values['VmHWM']['unavailable_reason'], 'field absent')
        for raw in ('VmRSS: 2 MB', 'VmRSS: -1 kB', 'VmRSS: 2', 'VmRSS: 1 kB\nVmRSS: 2 kB'):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                MEMORY.parse_kib_fields(raw, ('VmRSS',))

    def test_categories_distinguish_virtual_and_resident_bytes_without_paths(self):
        paths = ['[heap]', '[stack]', '', '/authored/libfake.so.1',
                 '/authored/private-input', '/dev/dri/renderD128', '[vdso]']
        raw = ''.join(mapping(path, start=(i + 1) * 4096) for i, path in enumerate(paths))
        result = MEMORY.parse_smaps(raw)
        for category in MEMORY.CATEGORIES:
            self.assertEqual(result[category]['mapping_count'], 1)
            self.assertEqual(result[category]['virtual_bytes'], 4096)
            self.assertEqual(result[category]['fields_bytes']['Rss']['value'], 3072)
        self.assertNotIn('/authored', json.dumps(result))
        self.assertNotIn('renderD128', json.dumps(result))

    def test_missing_mapping_field_does_not_report_partial_sum(self):
        raw = mapping('[heap]') + mapping('[heap]', 8192).replace('Pss: 2 kB\n', '')
        values = MEMORY.parse_smaps(raw)['heap']['fields_bytes']
        self.assertEqual(values['Rss']['value'], 6144)
        self.assertIsNone(values['Pss']['value'])
        self.assertIn('at least one mapping', values['Pss']['unavailable_reason'])

    def test_mapping_count_and_malformed_ranges_fail_closed(self):
        for raw, limit in [(mapping() * 2, 1), ('', 1),
                           (mapping().replace('00001000-00002000', '00002000-00001000'), 1),
                           (mapping() + 'bad-range rw-p 00000000 00:00 0\n', 2),
                           ('Rss: 2 kB\n', 1)]:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                MEMORY.parse_smaps(raw, max_mappings=limit)

    def test_read_bound_and_permission_reason_do_not_leak_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'authored'
            path.write_bytes(b'12345')
            self.assertEqual(MEMORY.read_bounded(path, 5), ('12345', None))
            value, reason = MEMORY.read_bounded(path, 4)
            self.assertIsNone(value)
            self.assertIn('byte limit exceeded', reason)
            with patch.object(Path, 'open', side_effect=PermissionError(13, 'sensitive path')):
                self.assertEqual(MEMORY.read_bounded(path, 4), (None, 'permission denied'))


class MemoryIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.directory = fixture(self.root)

    def test_current_highwater_rollup_and_mapping_boundaries(self):
        row = MEMORY.read_process(self.root, 42, deep=True, clock=lambda: 123)
        self.assertEqual(row['identity']['value']['start_time_ticks'], 100)
        self.assertEqual(row['status']['value']['VmRSS']['value'], 7168)
        self.assertEqual(row['status']['value']['VmHWM']['value'], 9216)
        self.assertEqual(row['smaps_rollup']['value']['Rss']['value'], 3072)
        self.assertEqual(row['mapping_categories']['value']['heap']['fields_bytes']['Rss']['value'], 3072)

    def test_pid_reuse_or_exit_during_read_discards_mixed_observation(self):
        initial = {'pid': 42, 'ppid': 1, 'start_time_ticks': 100}
        reused = {**initial, 'start_time_ticks': 200}
        for second in [(reused, None), (None, 'process exited')]:
            with self.subTest(second=second), patch.object(MEMORY, 'read_identity', side_effect=[(initial, None), second]):
                row = MEMORY.read_process(self.root, 42, deep=True)
                for key in ('identity', 'status', 'smaps_rollup', 'mapping_categories'):
                    self.assertIsNone(row[key]['value'])
                    self.assertTrue(row[key]['unavailable_reason'])

    def test_absent_identity_prevents_unattributed_memory(self):
        row = MEMORY.read_process(self.root, 999, deep=True)
        self.assertIsNone(row['identity']['value'])
        self.assertIn('identity unavailable', row['status']['unavailable_reason'])

    def test_missing_rollup_and_oversized_smaps_keep_status(self):
        (self.directory / 'smaps_rollup').unlink()  # Authored fixture only.
        with patch.object(MEMORY, 'MAX_SMAPS_BYTES', 8):
            row = MEMORY.read_process(self.root, 42, deep=True)
        self.assertIsNotNone(row['status']['value'])
        self.assertIn('absent', row['smaps_rollup']['unavailable_reason'])
        self.assertIn('byte limit', row['mapping_categories']['unavailable_reason'])

    def test_pid_reuse_between_rounds_retains_separate_lifetimes(self):
        now = [0]
        collector = MEMORY.MemoryDiagnostics(self.root, clock=lambda: now[0])
        collector.sample({42})
        fixture(self.root, start=200, hwm=3)
        now[0] = MEMORY.INTERVAL_NS
        collector.sample({42})
        self.assertEqual(len(collector.identities), 2)
        self.assertEqual(collector.identities[(42, 100)]['observed_high_water_bytes']['value'], 9216)
        self.assertEqual(collector.identities[(42, 200)]['observed_high_water_bytes']['value'], 3072)

    def test_round_pid_row_and_identity_limits_are_explicit(self):
        fixture(self.root, pid=43)
        fixture(self.root, pid=44)
        now = [0]
        collector = MEMORY.MemoryDiagnostics(self.root, clock=lambda: now[0])
        with patch.multiple(MEMORY, MAX_ROUNDS=2, MAX_PIDS_PER_ROUND=2, MAX_ROWS=1):
            collector.sample({42, 43, 44})
            collector.sample({42, 43, 44})  # Same interval: no additional work.
            self.assertEqual(collector.rounds, 1)
            now[0] += MEMORY.INTERVAL_NS
            collector.sample({42, 43, 44})
            now[0] += MEMORY.INTERVAL_NS
            collector.sample({42})
        self.assertEqual(len(collector.samples), 1)
        self.assertEqual(collector.skipped, {'pid_slots': 2, 'row_slots': 3, 'identity_slots': 0, 'rounds': 1})
        collector = MEMORY.MemoryDiagnostics(self.root)
        with patch.object(MEMORY, 'MAX_IDENTITIES', 1):
            collector.sample({42, 43})
        self.assertEqual(len(collector.identities), 1)
        self.assertEqual(len(collector.samples), 1)
        self.assertEqual(collector.skipped['identity_slots'], 1)

    def test_only_one_deep_read_per_round(self):
        fixture(self.root, pid=43)
        collector = MEMORY.MemoryDiagnostics(self.root)
        collector.sample({42, 43})
        self.assertEqual(sum(row['mapping_categories']['value'] is not None for row in collector.samples), 1)
        self.assertIn('not selected', collector.samples[1]['smaps_rollup']['unavailable_reason'])

    def test_deep_rotation_covers_pids_across_bounded_batches(self):
        for pid in range(43, 46):
            fixture(self.root, pid=pid)
        now = [0]
        collector = MEMORY.MemoryDiagnostics(self.root, clock=lambda: now[0])
        with patch.object(MEMORY, 'MAX_PIDS_PER_ROUND', 2):
            for _ in range(4):
                collector.sample(set(range(42, 46)))
                now[0] += MEMORY.INTERVAL_NS
        deep = [row['pid'] for row in collector.samples if row['mapping_categories']['value'] is not None]
        self.assertEqual(deep, [42, 43, 44, 45])


class MemoryPhaseTests(unittest.TestCase):
    def test_markers_require_identity_clock_order_and_bounds(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'markers.jsonl'
            path.write_text(json.dumps(marker()) + '\n')
            self.assertEqual(MEMORY.read_markers(path)['value'], [marker()])
            for item in [{**marker(), 'clock': 'app_elapsed'}, {**marker(), 'pid': True},
                         {**marker(), 'phase_label': '/sensitive/path'}, {**marker(), 'monotonic_ns': -1}]:
                path.write_text(json.dumps(item) + '\n')
                self.assertIsNone(MEMORY.read_markers(path)['value'])
            path.write_text(json.dumps(marker()) + '\n' + json.dumps(marker()) + '\n')
            self.assertIsNone(MEMORY.read_markers(path)['value'])
            with patch.object(MEMORY, 'MAX_MARKERS', 1):
                self.assertIn('count limit', MEMORY.read_markers(path)['unavailable_reason'])
            with patch.object(MEMORY, 'MAX_MARKER_BYTES', 8):
                self.assertIn('byte limit', MEMORY.read_markers(path)['unavailable_reason'])
            path.write_text('[' * 2000 + '0' + ']' * 2000)
            self.assertIsNone(MEMORY.read_markers(path)['value'])

    def test_phase_brackets_match_identity_and_never_interpolate(self):
        def row(start, begin, end):
            return {'identity': MEMORY.available({'pid': 42, 'start_time_ticks': start}),
                    'read_started_monotonic_ns': begin, 'read_finished_monotonic_ns': end}
        rows = [row(100, 900_000_000, 1_000_000_000),
                row(200, 1_700_000_000, 1_800_000_000),
                row(100, 1_900_000_000, 2_100_000_000),
                row(100, 2_900_000_000, 3_000_000_000)]
        result = MEMORY.attach_markers([marker()], rows)[0]
        self.assertEqual(result['before_sample']['value'], 0)
        self.assertEqual(result['after_sample']['value'], 3)
        self.assertIsNone(result['at_marker_rss_bytes']['value'])
        distant = MEMORY.attach_markers([marker(at=20_000_000_000)], rows)[0]
        self.assertIsNone(distant['before_sample']['value'])
        self.assertIsNone(distant['after_sample']['value'])

    def test_old_stages_report_missing_clock_without_copying_events(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            collector = MEMORY.MemoryDiagnostics(root)
            report = collector.report(root / 'missing', [
                {'phase_label': 'first-view', 'phase_started_ms': 12,
                 'process_peak_rss_bytes': 1234, 'events': [{'raw': 'must not export'}]}])
        stage = report['application_phase_observations'][0]
        self.assertEqual(stage['snapshot_high_water_bytes']['value'], 1234)
        self.assertIn('no external monotonic anchor', stage['external_memory']['unavailable_reason'])
        self.assertIsNone(report['allocation_lifetime_attribution']['value'])
        self.assertIsNone(report['phase_markers']['value'])
        self.assertNotIn('must not export', json.dumps(report))


if __name__ == '__main__':
    unittest.main()
