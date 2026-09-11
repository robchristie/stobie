"""Check the frozen comparison method and reject missing independent trials."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('pacing_compare', Path(__file__).resolve().parents[1]/'viewer-pacing-compare.py')
pacing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pacing)


class PacingCompareTests(unittest.TestCase):
    def test_conservative_marginal_interval(self):
        result = pacing.interval([10-2**.5, 10, 10, 10, 10+2**.5])
        self.assertEqual(result['mean'], 10)
        self.assertGreaterEqual(result['high']-10, 5.5975683670755/5**.5)
        self.assertEqual(pacing.interval([10]*20)['low'], 10)

    def test_rejects_missing_invalid_or_partial_trials(self):
        for values in ([10]*19, [10]*4, [10]*19+[float('nan')], [10]*19+[0]):
            with self.assertRaises(ValueError):
                pacing.interval(values)
        with self.assertRaises(ValueError):
            pacing.assess({'pairs': []})
