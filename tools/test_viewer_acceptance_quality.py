"""Synthetic regressions for quality coverage, tile support and attribution."""
import importlib.util
from pathlib import Path
import struct
import json
from types import SimpleNamespace
import tempfile
import unittest

import numpy as np

spec = importlib.util.spec_from_file_location('probe', Path(__file__).with_name('viewer-acceptance-quality.py'))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class QualityProtocolTests(unittest.TestCase):
    def test_screen_preserves_partial_original_edge(self):
        asset = {'image': {'width': 1213, 'height': 688}, 'views': [{'x': 448, 'y': 432, 'width': 256, 'height': 256}]}
        rect = probe.screen_rect(asset)
        self.assertEqual(rect, (0, 0, 1024, 688))
        self.assertEqual(probe.tile_supports(asset, rect)[-1], [512, 512, 512, 176])
        with self.assertRaises(ValueError):
            probe.tile_supports(asset, (0, 0, 1024, 600))
        with self.assertRaises(ValueError):
            probe.tile_supports(asset, (4, 0, 512, 512))

    def test_partial_box_edges_include_every_sample(self):
        values = np.arange(35).reshape(1, 5, 7)
        actual = probe.box_average(values, 4)
        expected = np.array([[[values[0, :4, :4].mean(), values[0, :4, 4:].mean()], [values[0, 4:, :4].mean(), values[0, 4:, 4:].mean()]]])
        np.testing.assert_equal(actual, expected)

    def test_histogram_matches_frozen_higher_quantile(self):
        values = np.array([0] * 98 + [4, 12, 255])
        result = probe.histogram_metrics(np.bincount(values, minlength=256))
        self.assertEqual(result['p99_absolute'], np.quantile(values, .99, method='higher'))
        self.assertAlmostEqual(result['rmse'], np.sqrt(np.mean(values.astype(float) ** 2)))
        self.assertIsNone(probe.histogram_metrics(np.zeros(256, dtype=np.int64)))

    def test_reserved_rate_and_binary_identity_are_bound(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            tool, gdal, receipt = root / 'tool', root / 'gdal', root / 'selection.json'
            tool.write_bytes(b'tool')
            gdal.write_bytes(b'gdal')
            build = {'tool': {'sha256': probe.source.digest(tool)}, 'gdal': {'sha256': probe.source.digest(gdal)},
                     'cargo_lock_sha256': probe.source.digest(Path(__file__).resolve().parents[1] / 'Cargo.lock')}
            args = SimpleNamespace(tool=tool, gdal_library=gdal, asset='106_10400100413CDF00-RGB16', bpp=8, phase='full', selection=None)
            with self.assertRaises(ValueError):
                probe.validate_invocation(args, build, 'views')
            receipt.write_text(json.dumps({'schema': 'viewer-acceptance-quality-selection/1', 'views_sha256': 'views',
                                          'development_complete_passed': True, 'tool_sha256': build['tool']['sha256'], 'rgb16_bpp': 8}))
            args.selection = receipt
            probe.validate_invocation(args, build, 'views')
            args.bpp = 12
            with self.assertRaises(ValueError):
                probe.validate_invocation(args, build, 'views')
            args.bpp = 8
            args.phase = 'screen'
            with self.assertRaises(ValueError):
                probe.validate_invocation(args, build, 'views')
            args.phase = 'full'
            with self.assertRaises(ValueError):
                probe.validate_invocation(args, build, 'different views')
            tool.write_bytes(b'changed')
            with self.assertRaises(ValueError):
                probe.validate_invocation(args, build, 'views')
            tool.write_bytes(b'tool')
            gdal.write_bytes(b'changed')
            with self.assertRaises(ValueError):
                probe.validate_invocation(args, build, 'views')
            gdal.write_bytes(b'gdal')
            build['cargo_lock_sha256'] = 'wrong lock'
            with self.assertRaises(ValueError):
                probe.validate_invocation(args, build, 'views')

    def test_reduced_validity_retains_partial_cells(self):
        valid = np.ones((1, 4, 8), dtype=bool)
        valid[0, 0, 0] = False
        fraction = probe.box_average(valid, 4)[0]
        np.testing.assert_equal(fraction > 0, [[True, True]])
        np.testing.assert_equal(fraction == 1, [[False, True]])
        np.testing.assert_equal((fraction > 0) & (fraction < 1), [[True, False]])

    def test_source_valid_gate_keeps_valid_side_ringing_and_partial_cells(self):
        original = np.array([[[0, 8], [8, 8]]], dtype=np.uint16)
        decoded = np.array([[[200, 0], [8, 8]]], dtype=np.uint16)
        original_display = original.transpose(1, 2, 0).astype(np.uint8)
        decoded_display = decoded.transpose(1, 2, 0).astype(np.uint8)
        population = probe.quality.display_populations(original, original_display, decoded_display, [{'nodata': 0}], 1)
        self.assertEqual(population['all_pixels'][0]['samples'], 4)
        self.assertEqual(population['any_valid'][0]['samples'], 3)
        self.assertEqual(population['any_valid'][0]['max_absolute'], 8)
        # The decoded zero at an originally valid sample stays in the error population.
        self.assertGreater(population['any_valid'][0]['rmse'], 0)
        orig_reduced = np.rint(probe.box_average(original, 2)).transpose(1, 2, 0).astype(np.uint8)
        dec_reduced = np.rint(probe.box_average(decoded, 2)).transpose(1, 2, 0).astype(np.uint8)
        reduced = probe.quality.display_populations(original, orig_reduced, dec_reduced, [{'nodata': 0}], 2)
        self.assertEqual(reduced['any_valid'][0]['samples'], 1)
        self.assertEqual(reduced['partial_valid'][0]['samples'], 1)
        self.assertIsNone(reduced['all_valid'][0])
        self.assertGreater(reduced['partial_valid'][0]['rmse'], 0)

    def test_descriptor_packet_attribution_and_truncation(self):
        profile = {'components': 3, 'decomposition_levels': 2}
        qcd = b'\xff\x5c\x00\x11\x62' + b'\x38\x01' * 7
        data = b'EHTIDX01' + struct.pack('<HQI', 5, 200, len(qcd)) + qcd
        for _ in range(3):
            for component in range(3):
                data += struct.pack('<II', 1, component + 2) + b'\x00'
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / 'descriptor'
            path.write_bytes(data)
            result = probe.descriptor_attribution(path, profile)
            self.assertEqual(result['tile'], 5)
            self.assertEqual([c['packet_body_bytes'] for c in result['components']], [6, 9, 12])
            self.assertEqual(result['qcd_steps'][0], {'exponent': 7, 'mantissa': 1})
            path.write_bytes(data[:-1])
            with self.assertRaises((ValueError, struct.error)):
                probe.descriptor_attribution(path, profile)


if __name__ == '__main__':
    unittest.main()
