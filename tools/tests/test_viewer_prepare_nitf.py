"""Source syscall evidence must remain distinct from callbacks and device I/O."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    'viewer_prepare_nitf', Path(__file__).resolve().parents[1] / 'viewer-prepare-nitf.py')
HARNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS)


class SourceTraceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'authored-source.bin'
        self.source.write_bytes(bytes(range(256)) * 16)

    def parse(self, lines):
        trace = self.root / 'trace.1'
        trace.write_text('\n'.join(lines) + '\n')
        return HARNESS.parse_source_trace([trace], self.source)

    def test_positive_zero_and_error_reads_have_distinct_counts(self):
        result = self.parse([
            f'read(3<{self.source}>, "", 4096) = 4096',
            f'pread64(3<{self.source}>, "", 16, 0) = 16',
            f'readv(3<{self.source}>, [{{iov_base="", iov_len=7}}], 1) = 7',
            f'read(3<{self.source}>, "", 4096) = 0',
            f'read(3<{self.source}>, "", 1) = -1 EIO (Input/output error)',
            'read(4</other-file>, "", 1000) = 1000',
            '+++ exited with 0 +++',
        ])
        self.assertEqual(result['read_bytes'], 4119)
        self.assertEqual(result['read_operations'], 3)
        self.assertEqual(result['zero_read_operations'], 1)
        self.assertEqual(result['error_read_operations'], 1)

    def test_negative_evidence_is_never_reported_as_zero(self):
        cases = [
            ['read(3, "", 12) = 12'],
            [f'read(3<{self.source}>,  <unfinished ...>'],
            ['<... read resumed>"", 12) = 12'],
            [f'read(3<{self.source} (deleted)>, "", 12) = 12'],
            [f'mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, 3<{self.source}>, 0) = 0x1234'],
            [f'sendfile(4</output>, 3<{self.source}>, NULL, 12) = 12'],
            ['io_uring_setup(32, {}) = 3<anon_inode:[io_uring]>'],
            [f'read(3<{self.source}>, "", 12) = ? ERESTARTSYS'],
            ['read(4</other-file>, "", 12) = 12'],
            [f'read(3<{self.source}>, "", 12) = 12', '+++ killed by SIGKILL +++'],
        ]
        for lines in cases:
            with self.subTest(lines=lines), self.assertRaises(ValueError):
                self.parse(lines + ['+++ exited with 0 +++'])

    def test_missing_task_termination_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'did not exit'):
            self.parse([f'read(3<{self.source}>, "", 12) = 12'])

    @unittest.skipUnless(sys.platform == 'linux' and shutil.which('strace'), 'Linux strace required')
    def test_real_syscalls_against_authored_file(self):
        # Exercise the observer against actual read, pread and readv syscalls,
        # including a child process. This proves observation, not NITF decoding.
        code = '''import os, sys
fd = os.open(sys.argv[1], os.O_RDONLY)
assert len(os.read(fd, 13)) == 13
assert len(os.pread(fd, 17, 1)) == 17
assert os.readv(fd, [bytearray(5), bytearray(7)]) == 12
assert os.pread(fd, 1, 4096) == b''
pid = os.fork()
if pid == 0:
    assert len(os.pread(fd, 19, 0)) == 19
    os._exit(0)
assert os.waitpid(pid, 0)[1] == 0
os.close(fd)
'''
        command = HARNESS.trace_command(shutil.which('strace'), self.root / 'actual',
                                        [sys.executable, '-c', code, str(self.source)])
        subprocess.run(command, check=True, capture_output=True)
        result = HARNESS.parse_source_trace(sorted(self.root.glob('actual.*')), self.source)
        self.assertEqual(result['read_bytes'], 61)
        self.assertEqual(result['read_operations'], 4)
        self.assertEqual(result['zero_read_operations'], 1)
        self.assertEqual(result['traced_tasks'], 2)
        traces = ''.join(path.read_text() for path in self.root.glob('actual.*'))
        self.assertNotIn('\\1\\2\\3', traces)

    def test_existing_output_fails_before_tool_invocation(self):
        output = self.root / 'existing'
        output.mkdir()
        sentinel = output / 'untouched'
        sentinel.write_text('original')
        args = []
        for key in ('tool', 'gdal-library', 'plugin', 'codec-library', 'input'):
            args.extend(['--' + key, str(self.source)])
        for key in ('tool', 'gdal', 'plugin', 'codec'):
            args.extend(['--' + key + '-revision', 'a' * 40])
        args.extend(['--output', str(output), '--build-record', str(self.source),
                     '--measurement-mode', 'observer', '--target', 'test',
                     '--width', '256', '--height', '256', '--bits', '11', '--bands', '1',
                     '--tile', '256', '--levels', '2', '--bpp', '2'])
        with self.assertRaises(FileExistsError):
            HARNESS.main(args)
        self.assertEqual(list(output.iterdir()), [sentinel])
        self.assertEqual(sentinel.read_text(), 'original')


class FrozenBoundTests(unittest.TestCase):
    def setUp(self):
        self.report = dict(started_unix_ms=20, source={'sha256': 'source'}, bands=[1],
                           measurement_mode='performance', wall_ms=15,
                           source_cache_state='uncontrolled',
                           representation=dict(profile={'bits_per_sample': 11}, encoded_bytes=100,
                                               metrics={'peak_rss_kib': 99, 'descriptor_bytes': 10}),
                           source_syscalls=None)
        self.limits = dict(schema='viewer-nitf-preparation-thresholds/1', frozen_unix_ms=10,
                           source_sha256='source', bands=[1], measurement_mode='performance',
                           source_cache_state='uncontrolled',
                           profile={'bits_per_sample': 11}, bounds={'peak_rss_kib': {'maximum': 100}})

    def test_matching_separate_freeze_checks_actual_bound(self):
        self.assertEqual(HARNESS.evaluate_bounds(self.limits, self.report), [])
        self.limits['bounds']['peak_rss_kib']['maximum'] = 98
        self.assertEqual(len(HARNESS.evaluate_bounds(self.limits, self.report)), 1)

    def test_wrong_mode_source_time_or_missing_observation_rejected(self):
        for key, value in [('source_sha256', 'other'), ('measurement_mode', 'observer'),
                           ('source_cache_state', 'cold-os'),
                           ('frozen_unix_ms', 20), ('schema', 'synthetic-preparation/1'),
                           ('bounds', {'source_read_bytes': {'maximum': 100}}),
                           ('bounds', {'wall_ms': {'maximum': float('nan')}}), ('bounds', {})]:
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                HARNESS.evaluate_bounds(dict(self.limits, **{key: value}), self.report)


@unittest.skipUnless(sys.platform == 'linux', 'Linux source residency required')
class SourceCacheTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'authored-cache-source.bin'
        self.page_size = os.sysconf('SC_PAGE_SIZE')
        with self.source.open('wb') as stream:
            stream.write(b'x' * (self.page_size * 4 + 37))
            stream.flush()
            os.fsync(stream.fileno())

    def test_real_warm_pass_and_bounded_residency_windows(self):
        evidence = {}
        with mock.patch.object(HARNESS, 'CACHE_WINDOW_BYTES', self.page_size * 2):
            HARNESS.condition_source_cache(self.source, 'warm-os', evidence)
        self.assertEqual(evidence['conditioning_read_bytes'], self.source.stat().st_size)
        self.assertEqual(evidence['before']['pages'], 5)
        self.assertEqual(evidence['before']['resident_pages'], 5)
        self.assertEqual(evidence['before']['mapping_windows'], 3)
        self.assertLessEqual(evidence['before']['maximum_mapping_bytes'], self.page_size * 2)

    def test_real_cold_request_observed_or_rejected_never_assumed(self):
        evidence = {}
        try:
            HARNESS.condition_source_cache(self.source, 'cold-os', evidence)
        except ValueError as error:
            # tmpfs and some filesystems do not honour eviction. The portable
            # test requires explicit rejection there, never an invented cold pass.
            self.assertIn('source pages remain resident', str(error))
            self.assertGreater(evidence['before']['resident_pages'], 0)
        else:
            self.assertEqual(evidence['before']['resident_pages'], 0)
            self.assertEqual(HARNESS.source_residency(self.source)['resident_pages'], 0)
        self.assertEqual(evidence['advised_bytes'], self.page_size * 5)

    def test_ineffective_eviction_and_partial_warmth_fail_admission(self):
        snapshot = dict(available=True, pages=5, resident_pages=2)
        for state in ('cold-os', 'warm-os'):
            evidence = {}
            with self.subTest(state=state), mock.patch.object(HARNESS, 'source_residency',
                                                             return_value=snapshot):
                with self.assertRaisesRegex(ValueError, 'admission failed'):
                    HARNESS.condition_source_cache(self.source, state, evidence)
            self.assertEqual(evidence['before']['resident_pages'], 2)

    def test_missing_residency_fails_explicit_state_but_records_uncontrolled(self):
        with mock.patch.object(HARNESS, 'source_residency', side_effect=OSError('unavailable')):
            for state in ('cold-os', 'warm-os'):
                evidence = {}
                with self.subTest(state=state), self.assertRaisesRegex(ValueError, 'unavailable'):
                    HARNESS.condition_source_cache(self.source, state, evidence)
                self.assertFalse(evidence['before']['available'])
            evidence = {}
            HARNESS.condition_source_cache(self.source, 'uncontrolled', evidence)
            self.assertFalse(evidence['before']['available'])

    def test_unowned_or_empty_sources_do_not_supply_residency_proof(self):
        with mock.patch.object(HARNESS.os, 'geteuid', return_value=os.geteuid() + 1):
            with self.assertRaisesRegex(ValueError, 'caller-owned'):
                HARNESS.source_residency(self.source)
        self.source.write_bytes(b'')
        with self.assertRaisesRegex(ValueError, 'non-empty regular file'):
            HARNESS.source_residency(self.source)


class ImmutableResultTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / 'payload.j2c').write_bytes(b'test payload')
        (self.root / 'descriptors').mkdir()
        (self.root / 'descriptors/0.bin').write_bytes(b'test descriptor')
        self.args = SimpleNamespace(width=256, height=256, bits=11, bands=[1], tile=256,
                                    levels=2, bpp=2, codec_revision='a' * 40, target='u11')
        profile = dict(width=256, height=256, bits_per_sample=11, components=1,
                       tile_edge=256, decomposition_levels=2, bits_per_pixel=2)
        self.source = dict(sha256='b' * 64, bytes=123)
        manifest = dict(identity=dict(profile=profile, bands=[1], codec_revision='a' * 40,
                                     source_sha256='b' * 64,
                                     payload_sha256=HARNESS.digest(self.root / 'payload.j2c')),
                        encoded_bytes=12, target='u11', tid='test-only',
                        descriptor_sha256=[HARNESS.digest(self.root / 'descriptors/0.bin')])
        metrics = dict(source_outer_driver='NITF', source_nitf_ic='C8', source_index_required=True,
                       source_hash_read_bytes=123, peak_rss_kib=100)
        self.result = [manifest, metrics]
        (self.root / 'manifest.json').write_text(json.dumps(manifest))
        (self.root / 'preparation.json').write_text(json.dumps(metrics))

    def test_manifest_payload_and_descriptor_are_content_bound(self):
        result = HARNESS.validate_result(self.result, self.root, self.args, self.source)
        self.assertEqual(result['encoded_bytes'], 12)
        self.assertEqual(len(result['descriptors']), 1)
        (self.root / 'descriptors/0.bin').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'descriptor digest'):
            HARNESS.validate_result(self.result, self.root, self.args, self.source)

    def test_source_precision_and_stored_metrics_must_match(self):
        self.args.bits = 16
        with self.assertRaisesRegex(ValueError, 'identity mismatch'):
            HARNESS.validate_result(self.result, self.root, self.args, self.source)
        self.args.bits = 11
        self.result[1]['peak_rss_kib'] = 1
        with self.assertRaisesRegex(ValueError, 'stored preparation metrics'):
            HARNESS.validate_result(self.result, self.root, self.args, self.source)

    def test_legacy_decoder_cannot_qualify_indexed_preparation(self):
        self.result[1]['source_index_required'] = False
        (self.root / 'preparation.json').write_text(json.dumps(self.result[1]))
        with self.assertRaisesRegex(ValueError, 'retained source index'):
            HARNESS.validate_result(self.result, self.root, self.args, self.source)


if __name__ == '__main__':
    unittest.main()
