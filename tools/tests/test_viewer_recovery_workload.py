"""Keep the real-scene pressure choice explicit and reject unsupported selections."""
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class RecoveryWorkloadTests(unittest.TestCase):
    def test_selection_and_pan_admission(self):
        subprocess.run(['node', '--input-type=module', '-e', """
import assert from 'node:assert/strict';
import {pressureWorkload,panSweep} from './tools/viewer-recovery-workload.mjs';
assert.equal(pressureWorkload([]),'image-gallery');
assert.equal(pressureWorkload(['--pressure-workload','real-scene-pan-sweep']),'real-scene-pan-sweep');
for (const args of [['PAN'],['--pressure-workload'],['--pressure-workload','unknown'],['--pressure-workload','image-gallery','extra']]) {
  assert.throws(()=>pressureWorkload(args));
}
const m=(width,height,components)=>({identity:{profile:{width,height,components}}});
assert.throws(()=>panSweep([]));
assert.throws(()=>panSweep([m(2048,2048,3)]));
assert.throws(()=>panSweep([m(256,256,1)]));
const plan=panSweep([m(8192,8192,3),m(4851,2752,1),m(1024,1024,1)]);
assert.equal(plan.panIndex,1);
assert.ok(Math.abs(plan.viewWidth-512)<1e-10);
assert.equal(plan.factor,512/4851);
"""], cwd=ROOT, check=True)
