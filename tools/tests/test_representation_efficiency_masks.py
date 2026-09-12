"""Authored compact oracle and frozen regional selection invariants."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import numpy as np

SPEC = importlib.util.spec_from_file_location('compact_masks', Path(__file__).parents[1] / 'representation-efficiency-masks.py')
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class CompactOracle(unittest.TestCase):
    def test_uniform_and_mixed_reordered_original_bands_and_corrupt_oracle(self):
        for state in (0, 1, 2):
            with tempfile.TemporaryDirectory() as temp:
                root, old = Path(temp) / 'new', Path(temp) / 'old'
                for directory in (root, old):
                    (directory / 'masks/0').mkdir(parents=True)
                values = np.array([[state == 1] * 9] * 2, dtype=np.uint8)
                if state == 2:
                    values[0, 2] = 1
                    values[1, 4:] = 1
                legacy = b''.join(np.packbits(v, bitorder='little').tobytes() for v in values)
                compact = bytes([state]) + (legacy if state == 2 else b'')
                (old / 'masks/0/0.bin').write_bytes(legacy)
                (root / 'masks/0/0.bin').write_bytes(compact)
                (root / 'manifest.json').write_text(json.dumps({'identity': {
                    'profile': {'width': 3, 'height': 3, 'tile_edge': 256},
                    'validity': {'bands': [3, 1], 'tile_sha256': [[f'{state}:{hashlib.sha256(compact).hexdigest()}']]}}}))
                records = []
                for band, values_for_band in zip((3, 1), values):
                    for plane in ('all', 'any'):
                        hashed = hashlib.sha256(values_for_band).hexdigest()
                        records.append(dict(tile=0, discard=0, band=band, plane=plane,
                                            expected_sha256=hashed, actual_sha256=hashed,
                                            valid=int(values_for_band.sum()), samples=9))
                result = module.oracle(root, old, records)
                self.assertEqual(result['original_gdal_oracle_plane_cells'], 36)
                self.assertEqual(result['false_invalid'], 0)
                records[0]['expected_sha256'] = '0' * 64
                with self.assertRaisesRegex(ValueError, 'oracle mismatch'):
                    module.oracle(root, old, records)

    def test_frozen_selection_preserves_first_and_last_each_level_and_band_set(self):
        requests = [dict(discard=d, components=c) for _ in range(3)
                    for d in range(7) for c in ([0], [1], [0, 1, 2])]
        actual = module.indices(requests)
        self.assertEqual(actual, list(range(21)) + list(range(42, 63)))

    def test_required_checks_do_not_depend_on_python_assertions(self):
        with self.assertRaises(ValueError):
            module.require(False, 'binding')
