"""Authored-only diagnostic mask and fail-closed identity regressions."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest.mock import patch

import numpy as np
from osgeo import gdal

SPEC = importlib.util.spec_from_file_location('full_scenes', Path(__file__).parents[1] / 'viewer-acceptance-full-scenes.py')
full = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(full)


def pack(planes, discard):
    selected = [planes[0]] if discard == 0 else planes
    return b''.join(np.packbits(p[c].ravel(), bitorder='little').tobytes()
                    for c in range(planes[0].shape[0]) for p in selected)


class Masks(unittest.TestCase):
    def test_nonzero_masks_odd_edges_and_distinct_band_populations(self):
        native = np.full((3, 5, 7), 191, dtype='u1')
        native[0, 0, 0] = 0
        native[1, -1, -1] = 0
        native[2] = 0
        for discard in range(7):
            actual = full.reduce_mask(native, discard)
            scale = 1 << discard
            wanted = [np.empty_like(actual[0]), np.empty_like(actual[1])]
            for c in range(3):
                for y in range(actual[0].shape[1]):
                    for x in range(actual[0].shape[2]):
                        block = native[c, y*scale:(y+1)*scale, x*scale:(x+1)*scale]
                        wanted[0][c, y, x] = (block != 0).all()
                        wanted[1][c, y, x] = (block != 0).any()
            for expected, observed in zip(wanted, full.unpack_mask(pack(actual, discard), 3, *actual[0].shape[1:], discard)):
                np.testing.assert_array_equal(expected, observed)
        self.assertFalse(full.reduce_mask(native, 6)[0].any())
        self.assertEqual(full.reduce_mask(native, 6)[1].sum(), 2)

    def test_padding_length_and_all_implies_any_fail_closed(self):
        for data, d, message in [(b'\xff\xff', 0, 'padding'), (b'\x01\x00', 1, 'all without any'), (b'', 0, 'length')]:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                full.unpack_mask(data, 1, 3 if d == 0 else 1, 3 if d == 0 else 1, d)

    def test_false_valid_and_false_invalid_are_actual_counts(self):
        expected = np.array([[1, 0, 1, 0]], bool)
        actual = np.array([[0, 1, 0, 0]], bool)
        result = full.differences(expected, actual)
        self.assertEqual((result['false_valid'], result['false_invalid']), (1, 2))
        self.assertNotEqual(result['expected_sha256'], result['actual_sha256'])

    def test_original_gdal_mask_unaligned_global_reduced_region(self):
        gdal.UseExceptions()
        ds = gdal.GetDriverByName('MEM').Create('', 519, 517, 3, gdal.GDT_UInt16)
        for c in range(3):
            band = ds.GetRasterBand(c + 1)
            values = np.full((517, 519), 99, dtype='u2')
            values[:, 512 + c] = 191
            values[-1, -1] = 191
            band.WriteArray(values)
            band.SetNoDataValue(191)
        asset = {'bands': [3, 1, 2]}
        request = {'x': 507, 'y': 505, 'width': 12, 'height': 12, 'discard': 2, 'components': [0, 2]}
        actual = full.regional_expected(ds, asset, request)
        self.assertEqual(actual.shape, (3, 3))
        native = np.stack([ds.GetRasterBand(b).GetMaskBand().ReadAsArray(508, 508, 11, 9) != 0 for b in [3, 2]])
        np.testing.assert_array_equal(actual, full.reduce_mask(native, 2)[0].all(axis=0))
        self.assertTrue(actual.any())
        self.assertTrue((~actual).any())

    def test_repeated_selections_and_source_edge_windows_all_levels(self):
        asset = {'image': {'width': 1027, 'height': 1031}, 'bands': [5, 3, 2],
                 'views': [{'x': 3, 'y': 5, 'width': 256, 'height': 256}]}
        requests = full.regional_requests(asset, {0: [512, 517]})
        self.assertEqual({r['discard'] for r in requests}, set(range(7)))
        for i in range(0, len(requests), 2):
            self.assertEqual(requests[i], requests[i + 1])
            self.assertEqual(requests[i]['components'], sorted(set(requests[i]['components'])))
        self.assertTrue(any(r['x'] + r['width'] == 1027 and r['y'] + r['height'] == 1031 for r in requests))


class Guards(unittest.TestCase):
    def test_source_hash_and_byte_length_changes_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'authored.bin'
            path.write_bytes(b'1234')
            record = full.identity(path)
            full.check_identity(record)
            for changed in [b'1235', b'12345']:
                path.write_bytes(changed)
                with self.assertRaisesRegex(ValueError, 'identity changed'):
                    full.check_identity(record)

    def test_fixed_cohort_has_no_screen_sweep_or_selected_configuration(self):
        self.assertEqual(len(full.COHORT), 5)
        for name, (rate, bands) in full.COHORT.items():
            self.assertEqual(rate, 12 if name.endswith('RGB16') else 4)
            self.assertEqual(bands, [5, 3, 2] if name.endswith('RGB16') else [1, 2, 3] if name.endswith('RGB8') else [1])
        self.assertEqual(full.DISPOSITION, 'quality-rejected-diagnostic-only')

    def test_grant_rejects_selection_receipt_and_identity_drift(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = argparse.Namespace(grant=root / 'grant.json', source_coverage=root / 'coverage.json',
                build_identity=root / 'build.json', protocol_commit='a' * 40,
                asset=next(iter(full.COHORT)), output_name='viewer-acceptance-authored')
            args.source_coverage.write_text('{}')
            args.build_identity.write_text('{}')
            bindings = {'runner.py': 'b' * 64}
            build = {'source_revision': full.REVISION, 'linked_codec_revision': full.CODEC}
            grant = {'schema': 'viewer-acceptance-full-scenes-grant/1', 'disposition': full.DISPOSITION,
                'protocol_commit': args.protocol_commit, 'protocol_files': bindings,
                'build_sha256': full.digest(args.build_identity), 'asset': args.asset, 'output_name': args.output_name,
                'source_coverage_sha256': full.digest(args.source_coverage), 'max_preparations': 1, 'operation_owner': 'authored'}
            args.grant.write_text(json.dumps(grant))
            full.validate_grant(args, build, bindings)
            for key, changed in [('schema', 'viewer-acceptance-quality-selection/1'), ('max_preparations', 2),
                                 ('disposition', 'accepted'), ('output_name', 'elsewhere'), ('protocol_files', {}),
                                 ('build_sha256', '0' * 64), ('operation_owner', '')]:
                candidate = {**grant, key: changed}
                args.grant.write_text(json.dumps(candidate))
                with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'grant'):
                    full.validate_grant(args, build, bindings)

    def test_uncommitted_protocol_bytes_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'runner.py'
            path.write_bytes(b'changed')
            with patch.object(full, 'REPO', Path(temporary)), patch.object(full, 'frozen_files', return_value=[path]), \
                 patch.object(full.subprocess, 'check_output', side_effect=['a' * 40 + '\n', b'committed']):
                with self.assertRaisesRegex(ValueError, 'not committed'):
                    full.committed_inputs('a' * 40)


@unittest.skipUnless(os.environ.get('EMUELLA_FULL_SCENES_AUTHORED') == '1', 'explicit authored native/GDAL probe')
class NativeAuthored(unittest.TestCase):
    def test_complete_odd_scene_native_service_masks_and_persistent_bytes(self):
        # Only authored arrays, in the registered build root; no RarePlanes read.
        with tempfile.TemporaryDirectory(dir=full.BUILD, prefix='authored-') as temporary:
            root = Path(temporary)
            original = root / 'authored.tif'
            ds = gdal.GetDriverByName('GTiff').Create(str(original), 643, 645, 3, gdal.GDT_Byte)
            for c in range(3):
                values = np.full((645, 643), 64 + c * 23, dtype='u1')
                values[:, 512 + c] = 191
                values[511, :] = 191
                values[-1, -1] = 191
                values[:4, :4] = 0  # Valid zero must remain valid.
                ds.GetRasterBand(c + 1).WriteArray(values)
                ds.GetRasterBand(c + 1).SetNoDataValue(191)
            ds = None
            asset = {'id': '105_104001002F92BB00-RGB8', 'bands': [1, 2, 3],
                     'image': {'width': 643, 'height': 645, 'precision': 8},
                     'source_sha256': full.digest(original), 'statistics': [{'nodata': 191}] * 3,
                     'views': [{'x': 257, 'y': 255, 'width': 256, 'height': 256}]}
            representation = root / 'representation'
            command = [str(full.BUILD / 'cargo/release/emuella-viewer-tools'), 'prepare']
            options = {'input': original, 'output': representation, 'gdal-library': full.GDAL,
                       'bands': '1,2,3', 'mask-input': original, 'mask-bands': '1,2,3', 'bits': 8,
                       'bpp': 4, 'tile': 512, 'levels': 6, 'codec-revision': full.CODEC, 'retain-incomplete': 'true'}
            for key, value in options.items():
                command.extend(['--' + key, str(value)])
            prepared = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
            self.assertEqual(prepared.returncode, 0, prepared.stderr.decode())
            ds = gdal.Open(str(original))
            masks = full.check_masks(ds, asset, representation, root)
            self.assertEqual((masks['tiles'], masks['levels'], masks['false_valid'], masks['false_invalid']), (4, 7, 0, 0))
            requests = full.regional_requests(asset, masks['boundary_points'])
            full.write_json(root / 'requests.json', requests)
            subprocess.run([str(full.BUILD / 'helper-cargo/release/full-scenes-native'), str(representation),
                            str(root / 'requests.json'), str(root)], check=True, timeout=180)
            native = json.loads((root / 'native.json').read_text())
            full.check_regions(ds, asset, root, requests, native)
            quality = full.module('viewer-acceptance-quality')
            (root / 'source').mkdir()
            asset['source_path'] = '../authored.tif'
            asset['stretches'] = {'authored': [[0, 255]] * 3}
            coverage = quality.full_coverage(argparse.Namespace(store=root,
                tool=full.BUILD / 'cargo/release/emuella-viewer-tools'), asset, representation, root, {'scales': [1, 4]})
            for cell in coverage['cells']:
                mask = ds.GetRasterBand(cell['source_band']).GetMaskBand().ReadAsArray() != 0
                all_valid, any_valid = full.reduce_mask(mask[None], 0 if cell['scale'] == 1 else 2)
                self.assertEqual(cell['metrics']['samples'], int(any_valid.sum()))
                partial = int((any_valid & ~all_valid).sum())
                self.assertEqual(0 if cell['partial_valid'] is None else cell['partial_valid']['samples'], partial)
            storage = full.persistent_storage(quality, representation, asset)
            self.assertEqual(native['checked_mask_bytes'], storage['actual_bytes']['masks'])
            self.assertEqual(storage['total_bytes'], sum(storage['actual_bytes'].values()))
            self.assertTrue(storage['mask_eligibility'])
            self.assertTrue(storage['total_eligible'])
            (representation / 'unexpected.bin').write_bytes(b'authored')
            with self.assertRaisesRegex(ValueError, 'unexpected persistent'):
                full.persistent_storage(quality, representation, asset)
            self.assertEqual(full.digest(original), asset['source_sha256'])


if __name__ == '__main__':
    unittest.main()
