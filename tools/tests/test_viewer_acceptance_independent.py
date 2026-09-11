"""Synthetic protocol regressions; no codec invocation or protected input access."""
from decimal import Decimal
import importlib.util
import io
import json
from pathlib import Path
import struct
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import numpy as np

spec = importlib.util.spec_from_file_location('independent', Path(__file__).resolve().parents[1] / 'viewer-acceptance-independent.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


def marker(code, body):
    return bytes([255, code]) + struct.pack('>H', len(body) + 2) + body


def cod(**changes):
    fields = dict(flags=0, progression=0, layers=1, mct=0, levels=6, bw=4, bh=4, style=64, transform=0)
    fields.update(changes)
    return struct.pack('>BBHBBBBBB', *fields.values())


def stream(precision=16, override=b'', parts=1):
    siz = struct.pack('>H8IH', 0x4000, 512, 176, 0, 0, 512, 512, 0, 0, 3) + bytes([precision - 1, 1, 1]) * 3
    main = b'\xff\x4f' + marker(0x51, siz) + marker(0x52, cod()) + marker(0x5c, b'\x62' + b'\x38\x01' * 19)
    tile = marker(0x90, struct.pack('>HIBB', 0, 17 + len(override), 0, parts)) + override + b'\xff\x93' + b'abc'
    return main + tile + b'\xff\xd9'


class IndependentTests(unittest.TestCase):
    def test_inclusive_exact_byte_gate(self):
        for size in [9900, 10000, 10100]:
            self.assertTrue(probe.matched(size, 10000))
        for size in [0, 9899, 10101]:
            self.assertFalse(probe.matched(size, 10000))
        self.assertTrue(probe.matched(1056940 + 10569, 1056940))
        self.assertFalse(probe.matched(1056940 + 10570, 1056940))

    def test_frozen_geometric_sequence_and_first_match(self):
        visited = []
        def invoke(ordinal, qstep):
            visited.append(qstep)
            return {'status': 'completed', 'payload_bytes': [1200, 800, 1005][ordinal - 1]}
        result = probe.rate_search(1000, invoke)
        self.assertEqual(result['selected'], 3)
        self.assertEqual(visited, ['2.2360679774997897E-3', '3.3437015248821101E-2', '8.6468167010213086E-3'])

    def test_eight_invocation_cap_without_endpoint_or_quality_feedback(self):
        visited = []
        def invoke(ordinal, qstep):
            visited.append((ordinal, Decimal(qstep)))
            return {'status': 'completed', 'payload_bytes': 2000}
        result = probe.rate_search(1000, invoke)
        self.assertEqual(result['status'], 'unmatched')
        self.assertIsNone(result['selected'])
        self.assertEqual(len(visited), 8)
        self.assertTrue(all(Decimal('.00001') < q < Decimal('.5') for _, q in visited))
        self.assertEqual(visited, sorted(visited, key=lambda v: v[1]))

    def test_failure_consumes_invocation_and_stops(self):
        calls = []
        def invoke(ordinal, qstep):
            calls.append(ordinal)
            return {'status': 'failed'}
        self.assertEqual(probe.rate_search(1000, invoke)['status'], 'unsupported')
        self.assertEqual(calls, [1])

    def inspect(self, data, precision=16):
        path = SimpleNamespace(stat=lambda: SimpleNamespace(st_size=len(data)), open=lambda _: io.BytesIO(data))
        return probe.inspect_stream(path, {'width': 512, 'height': 176, 'precision': precision})

    def test_bounded_marker_guard_and_unspecified_tilepart_count(self):
        for precision in [8, 16]:
            for parts in [0, 1]:
                result = self.inspect(stream(precision, parts=parts), precision)
                self.assertEqual(len(result['tiles']), 1)
                self.assertTrue(result['contract_verified'])
                self.assertEqual(result['tiles'][0]['bytes'], 17)

    def test_marker_guard_rejects_precision_overrides_and_truncation(self):
        invalid = [stream(8), stream(override=marker(0x52, cod(mct=1))),
                   stream(override=marker(0x53, b'\x00' * 10)), stream(parts=2),
                   stream()[:-1], stream() + b'extra']
        for data in invalid:
            with self.subTest(length=len(data)), self.assertRaises(ValueError):
                self.inspect(data)

    def test_each_material_cod_contract_field_is_enforced(self):
        for field, value in [('flags', 1), ('progression', 2), ('layers', 2), ('mct', 1),
                             ('levels', 5), ('bw', 3), ('bh', 3), ('style', 0), ('transform', 1)]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                probe.cod_contract(cod(**{field: value}))

    def test_cli_contract_has_no_stretched_or_colour_transformed_input(self):
        command = probe.encoder_command(Path('selected.tif'), Path('trial.j2c'), '0.001')
        opts = dict(zip(command[1::2], command[2::2]))
        self.assertEqual(opts['-colour_trans'], 'false')
        self.assertEqual(opts['-reversible'], 'false')
        self.assertEqual(opts['-num_decomps'], '6')
        self.assertEqual(opts['-prog_order'], 'LRCP')
        self.assertEqual(opts['-tile_size'], '{512,512}')
        self.assertEqual(opts['-block_size'], '{64,64}')

    def test_grant_requires_exact_revision_protocol_and_exclusive_window(self):
        grant = dict(schema='viewer-acceptance-independent-grant/1', revision='a' * 40,
                     protocol_sha256='b' * 64, output_group=probe.GROUP,
                     exclusive_measurement_window=True, measurement_owner='bounded-context')
        probe.check_grant(grant, 'a' * 40, 'b' * 64)
        for key in grant:
            invalid = dict(grant)
            invalid[key] = None
            with self.subTest(field=key), self.assertRaises(ValueError):
                probe.check_grant(invalid, 'a' * 40, 'b' * 64)

    def test_uncommitted_or_changed_protocol_rejected(self):
        with patch.object(probe.subprocess, 'check_output', side_effect=['a' * 40, b'committed']), patch.object(Path, 'read_bytes', return_value=b'changed'):
            with self.assertRaises(ValueError):
                probe.check_revision('a' * 40, ['tools/example.py'])
        with patch.object(probe.subprocess, 'check_output', return_value='b' * 40):
            with self.assertRaises(ValueError):
                probe.check_revision('a' * 40, [])

    def test_scoring_uses_existing_any_valid_gate_and_native_input(self):
        original = np.full((3, 4, 4), 1000, dtype=np.uint16)
        original[:, 0, 0] = 0
        decoded = original.copy()
        decoded[:, 0, 0] = 5000
        decoded[:, 0, 1] = 0  # Lossy zero at a valid source sample must count.
        def dataset(values):
            return SimpleNamespace(GetRasterBand=lambda c: SimpleNamespace(ReadAsArray=lambda *args: values[c - 1]))
        asset = {'views': [{'x': 0, 'y': 0, 'width': 4, 'height': 4}], 'bands': [1, 2, 3],
                 'statistics': [{'nodata': 0}] * 3, 'stretches': {'frozen': [[0, 5000]] * 3}}
        with patch.object(probe.gdal, 'Open', return_value=dataset(original)), patch.object(probe, 'read_decoded', return_value=dataset(decoded)):
            result = probe.score_views(Path('original'), Path('decoded'), asset, [1, 4])
        first = result['cells'][0]['populations']
        self.assertEqual(first['any_valid']['samples'], 15)
        self.assertEqual(first['any_valid']['max_absolute'], 51)
        self.assertEqual(first['all_pixels']['max_absolute'], 255)
        reduced = result['cells'][3]['populations']
        self.assertEqual(reduced['partial_valid']['samples'], 1)
        self.assertIsNone(reduced['all_valid'])
        self.assertFalse(result['passed'])

    def test_selected_rgb16_bands_preserve_all_words_across_strips(self):
        values = np.arange(5 * 130 * 4, dtype=np.uint16).reshape(5, 130, 4) + 1000
        copied = np.zeros((3, 130, 4), dtype=np.uint16)
        writes = []
        def dataset(array):
            def band(c):
                def write(chunk, x, y):
                    writes.append((c, y, chunk.shape[0]))
                    array[c - 1, y:y + chunk.shape[0], x:x + chunk.shape[1]] = chunk
                return SimpleNamespace(DataType=probe.gdal.GDT_UInt16, GetScale=lambda: None,
                                       GetOffset=lambda: None,
                                       ReadAsArray=lambda x, y, w, h: array[c - 1, y:y + h, x:x + w],
                                       WriteArray=write)
            return SimpleNamespace(RasterXSize=4, RasterYSize=130, GetRasterBand=band)
        asset = {'bands': [5, 3, 2], 'image': {'width': 4, 'height': 130, 'precision': 16}}
        with patch.object(probe.gdal, 'Open', side_effect=[dataset(values), dataset(copied)]), \
                patch.object(probe.gdal, 'GetDriverByName', return_value=SimpleNamespace(Create=lambda *a, **k: dataset(copied))), \
                patch.object(probe, 'identity', return_value={}):
            record = probe.selected_input(Path('crop'), asset, Path('selected'))
        np.testing.assert_array_equal(copied, values[[4, 2, 1]])
        self.assertEqual([h for _, _, h in writes], [128, 128, 128, 2, 2, 2])
        self.assertTrue(record['exact_samples_verified'])
        self.assertFalse(record['stretch_applied'])

    def test_comparison_refuses_different_population(self):
        baseline = {'records': [{'display': [{'scale': 1, 'stretch': 'fixed', 'source_any_valid': [
            {'samples': 2, 'rmse': 4, 'p99_absolute': 13}]}]}]}
        independent = {'cells': [{'view': 0, 'scale': 1, 'stretch': 'fixed', 'source_band': 5,
                                  'populations': {'any_valid': {'samples': 2, 'rmse': 2, 'p99_absolute': 10}}}]}
        result = probe.compare_quality(independent, baseline, {'bands': [5]})
        self.assertEqual(result[0]['rmse_delta'], -2)
        self.assertEqual(result[0]['p99_delta'], -3)
        independent['cells'][0]['populations']['any_valid']['samples'] = 1
        with self.assertRaises(ValueError):
            probe.compare_quality(independent, baseline, {'bands': [5]})

    def test_frozen_cohort_targets_and_scope(self):
        protocol = json.loads(probe.PROTOCOL.read_text())
        self.assertEqual([p['target_bytes'] for p in protocol['products']], [1056940, 1573332])
        self.assertEqual([p['asset']['bands'] for p in protocol['products']], [[5, 3, 2], [1, 2, 3]])
        self.assertEqual([p['asset']['image']['precision'] for p in protocol['products']], [16, 8])
        self.assertEqual(protocol['rate_algorithm']['maximum_encoder_invocations_per_product'], probe.MAX_TRIALS)


if __name__ == '__main__':
    unittest.main()
