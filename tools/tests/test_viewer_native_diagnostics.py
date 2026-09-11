import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('native_diagnostics', ROOT / 'tools/viewer-native-diagnostics.py')
DIAG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAG)


class NativeDiagnosticTests(unittest.TestCase):
    def test_authored_catalogue_is_explicit_bounded_and_representative(self):
        catalogue = DIAG.catalogue()
        self.assertEqual([(m['identity']['profile']['components'], m['identity']['profile']['bits_per_sample']) for m in catalogue], [(1, 16), (3, 16), (3, 8)])
        self.assertLess(len(DIAG.compact(catalogue)), 16 << 20)
        self.assertEqual(len({m['tid'] for m in catalogue}), 3)
        for m in catalogue:
            self.assertEqual(m['identity']['source_sha256'], DIAG.AUTHORED)
            self.assertEqual(m['identity']['encoding_contract'], DIAG.AUTHORED)
            self.assertNotIn('validity', m['identity'])
            self.assertEqual(len(m['descriptor_sha256']), 84 * 84)

    def test_cycle_workload_preserves_all_ordinary_phases_and_exactly_ten_resets(self):
        base = json.loads((ROOT / 'apps/emuella-viewer/real-scene-workload.json').read_text())
        diagnostic = json.loads((ROOT / 'apps/emuella-viewer/native-memory-workload.json').read_text())
        self.assertEqual(diagnostic[:len(base)], base)
        self.assertEqual(len(diagnostic), len(base) + 10)
        self.assertEqual([s['diagnostic_cycle'] for s in diagnostic[len(base):]], list(range(1, 11)))
        self.assertTrue(all(s['intent'] == {'kind': 'clear_display_cache'} for s in diagnostic[len(base):]))
        self.assertLessEqual(len(diagnostic), 64)

    def test_memory_threshold_binding_preserves_every_inherited_gate(self):
        folder = ROOT / 'apps/emuella-viewer/qualification'
        original = json.loads((folder / 'real-scene-native-thresholds.json').read_text())
        diagnostic = json.loads((folder / 'native-memory-thresholds.json').read_text())
        for key in ['schema', 'bounds', 'required_events', 'require_hardware_gpu', 'baseline_evidence_sha256']:
            self.assertEqual(diagnostic[key], original[key])
        workload = ROOT / 'apps/emuella-viewer/native-memory-workload.json'
        self.assertEqual(diagnostic['workload_sha256'], hashlib.sha256(workload.read_bytes()).hexdigest())
        self.assertEqual(diagnostic['bounds']['process_peak_rss_bytes']['maximum'], 243269632)

    def test_audit_requires_actual_paired_cycles_and_allocation_records(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            phases = ['empty-client-overview'] + [f'cycle-{i:02}' for i in range(1, 11)]
            boundaries = [('startup', 'startup'), ('graphics-ready', 'graphics')]
            for phase in phases:
                boundaries.append(('phase-start', phase))
                if phase.startswith('cycle-'):
                    boundaries.append(('cycle-evicted', phase))
                boundaries.append(('phase-settled', phase))
                if phase.startswith('cycle-'):
                    boundaries.append(('cycle-revisited', phase))
            markers = [dict(schema='viewer_memory_phase_marker/1', clock='linux_monotonic', pid=42,
                            start_time_ticks=100, monotonic_ns=1000+i, sequence=i,
                            kind=kind, phase_label=phase) for i, (kind, phase) in enumerate(boundaries)]
            marker_path = root / 'memory-phase-markers.jsonl'
            marker_path.write_text(''.join(json.dumps(m)+'\n' for m in markers))
            (root / 'app.json.stages.json').write_text(json.dumps([dict(phase_label=p) for p in phases]))
            (root / 'app.json').write_text(json.dumps(dict(script_complete=True, errors=[], diagnostic_cycles_completed=10, instrument='native-completion-memory-v1')))
            allocations = [dict(schema='viewer_native_allocation_marker/1', marker=m, observation_finished_monotonic_ns=m['monotonic_ns'], allocator={'unavailable': 'authored fixture'}) for m in markers]
            path = root / 'memory-phase-markers.allocations.jsonl'
            path.write_text(''.join(json.dumps(a)+'\n' for a in allocations))
            result = DIAG.audit(root)
            self.assertEqual(result['cycle_count'], 10)
            self.assertEqual(result['allocator_unavailable'], len(markers))
            path.write_text(''.join(json.dumps(a)+'\n' for a in allocations[:-1]))
            with self.assertRaisesRegex(ValueError, 'allocation/marker coverage mismatch'):
                DIAG.audit(root)
            marker_path.write_text(''.join(json.dumps(m)+'\n' for m in markers[:-1]))
            with self.assertRaisesRegex(ValueError, 'actual cycle marker coverage mismatch'):
                DIAG.audit(root)

    def test_audit_rejects_absent_markers_and_oversized_inputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(ValueError):
                DIAG.audit(root)
            path = root / 'large.json'
            path.write_text('[1,2,3]')
            with self.assertRaises(ValueError):
                DIAG.bounded_json(path, 2)


if __name__ == '__main__':
    unittest.main()
