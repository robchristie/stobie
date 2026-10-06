import assert from 'node:assert/strict';
import test from 'node:test';
import { resultSelectionConfirmed } from '../lab-result-selection.mjs';

function snapshot(result, rows = []) {
  return { ui_snapshot: { nodes: [
    { id: 'pane.7.selection', domain_reference: { kind: 'result', value: result } },
    { id: 'pane.1.viewport', description: `selected result: result ${result}` },
    ...rows,
  ] } };
}
const row = result => ({ role: 'result_row', selected: true,
  domain_reference: { kind: 'result', value: result } });

test('physical selection remains confirmed when its action bar clips the clicked row', () => {
  const state = snapshot(63, [{ role: 'result_row', selected: false,
    domain_reference: { kind: 'result', value: 64 } }]);
  assert.equal(resultSelectionConfirmed(63, state), true);
  assert.equal(resultSelectionConfirmed(63, snapshot(63, [row(63)])), true);
});

test('confirmation requires the clicked identity in both Inspector and primary view', () => {
  assert.equal(resultSelectionConfirmed(63, snapshot(64)), false);
  const wrongViewport = snapshot(63);
  wrongViewport.ui_snapshot.nodes[1].description = 'selected result: result 64';
  assert.equal(resultSelectionConfirmed(63, wrongViewport), false);
  wrongViewport.ui_snapshot.nodes[1].description = 'selected result: result 630';
  assert.equal(resultSelectionConfirmed(63, wrongViewport), false);
  assert.equal(resultSelectionConfirmed(63, snapshot(63, [row(64)])), false);
  const missingInspector = snapshot(63);
  missingInspector.ui_snapshot.nodes.shift();
  assert.equal(resultSelectionConfirmed(63, missingInspector), false);
  assert.equal(resultSelectionConfirmed(63, snapshot(null)), false);
  assert.equal(resultSelectionConfirmed(undefined, snapshot(null)), false);
});
