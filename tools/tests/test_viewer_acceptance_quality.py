"""Include the frozen quality regressions in canonical test discovery."""
import importlib.util
from pathlib import Path


def load_tests(loader, tests, pattern):
    path = Path(__file__).resolve().parents[1] / 'test_viewer_acceptance_quality.py'
    spec = importlib.util.spec_from_file_location('frozen_viewer_quality_tests', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return loader.loadTestsFromModule(module)
