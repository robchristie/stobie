import assert from 'node:assert/strict';
import test from 'node:test';
import { assertFrozenResultTarget, firstResultRowTarget, resolveResultRowTarget,
  resultSelectionConfirmed, sameResultTarget, waitForStableResultTarget } from '../lab-result-selection.mjs';

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

const canvas = { x: 0, y: 0, width: 600, height: 400 };
const rootRect = { min_x: 0, min_y: 0, max_x: 600, max_y: 400 };
const scrollRect = { min_x: 200, min_y: 20, max_x: 600, max_y: 380 };
function target(result, frame = 1, minY = 20, wheel = 1) {
  const rect = { min_x: 200, min_y: minY, max_x: 600, max_y: minY + 28 };
  return firstResultRowTarget({ frame_number: frame, physical_wheel_events: wheel,
    virtualisation: { visible_rows: [result, result + 14] },
    ui_snapshot: { root: 'root', nodes: [{ id: 'root', rect: rootRect }] },
    ui_geometry: { root: rootRect, results_scroll: scrollRect, result_rows: [{ result, rect }] },
  }, canvas);
}
function clock() {
  let elapsed = 0;
  return { now: () => elapsed, advance: ms => { elapsed += ms; }, wait: async ms => { elapsed += ms; } };
}

test('first-row selector retains a clipped first row and records its edge distances', () => {
  const state = { frame_number: 1, physical_wheel_events: 1,
    virtualisation: { visible_rows: [63, 77] },
    ui_snapshot: { root: 'root', nodes: [{ id: 'root', rect: rootRect }] },
    ui_geometry: { root: rootRect, results_scroll: scrollRect, result_rows: [
      { result: 63, rect: { min_x: 200, min_y: 20, max_x: 600, max_y: 20.5 } },
      { result: 64, rect: { min_x: 200, min_y: 20.5, max_x: 600, max_y: 48.5 } },
    ] },
  };
  const first = firstResultRowTarget(state, canvas);
  assert.equal(first.result, 63);
  assert.equal(first.hit_height, 0.5);
  assert.equal(first.clip_edge_distances.top, 0.25);
});

test('settling accepts the current identity after 63 becomes 64 at the same coordinates', async () => {
  const before = target(0, 1, 20, 0);
  let sample = 0;
  const time = clock();
  const settled = await waitForStableResultTarget(() => {
    sample++;
    return target(sample === 1 ? 63 : 64, sample + 1);
  }, time.wait, before, time.now);
  assert.equal(settled.target.result, 64);
  assert.equal(settled.stable_ms, 200);
  assert.equal(settled.observed_ms, time.now());
  assert.deepEqual(settled.observations.slice(0, 2).map(s => s.target.result), [63, 64]);
});

test('moving rectangles reset stability while unrelated frame and diagnostic churn do not', async () => {
  let sample = 0;
  const time = clock();
  const settled = await waitForStableResultTarget(() => {
    sample++;
    return { ...target(64, sample + 1, sample < 4 ? 50 - sample : 46), diagnostic_churn: sample };
  }, time.wait, target(0, 1, 20, 0), time.now);
  assert.equal(settled.target.rect.min_y, 46);
  assert(settled.observed_ms >= 300, 'moving geometry must not be accepted as stable');
  assert(settled.observed_ms <= 3500);
});

test('scroll clip, root geometry and canvas transforms are part of the stable signature', () => {
  const frozen = target(64);
  for (const [field, property] of [['scroll_rect', 'min_y'], ['root_rect', 'max_x'], ['canvas', 'width']]) {
    const moved = structuredClone(frozen);
    moved[field][property] += 1;
    assert.equal(sameResultTarget(frozen, moved), false, field);
    assert.throws(() => assertFrozenResultTarget(frozen, moved), /moved or disappeared/);
  }
  assert.equal(sameResultTarget(frozen, { ...frozen, frame: 100, physical_wheel_events: 2 }), true);
});

test('continuous geometry movement fails at the fixed settling budget', async () => {
  let sample = 0;
  const time = clock();
  await assert.rejects(waitForStableResultTarget(() => target(64, ++sample, 20 + sample % 20),
    time.wait, target(0, 1, 20, 0), time.now), error => {
    assert.match(error.message, /did not settle within 3500 ms/);
    assert.equal(error.observations.at(-1).observed_ms, 3500);
    return true;
  });
  assert.equal(time.now(), 3500);
});

test('settling requires processed physical wheel input and changed Results geometry', async () => {
  const before = target(0, 1, 20, 0);
  for (const unchanged of [target(64, 2, 20, 0), target(0, 2, 20, 1)]) {
    const time = clock();
    await assert.rejects(waitForStableResultTarget(async () => unchanged, time.wait, before, time.now), /did not settle/);
  }
});

test('a delayed wheel receipt starts the stable window when input is actually processed', async () => {
  const time = clock();
  let sample = 0;
  const settled = await waitForStableResultTarget(() => {
    sample++;
    return target(64, sample + 1, 20, sample < 3 ? 0 : 1);
  }, time.wait, target(0, 1, 20, 0), time.now);
  assert.equal(settled.observed_ms, 300);
  assert.equal(settled.stable_ms, 200);
});

test('the deadline includes slow reads and rejects a stable target returned too late', async () => {
  const time = clock();
  let frame = 1;
  await assert.rejects(waitForStableResultTarget(async () => {
    time.advance(1800);
    return target(64, ++frame);
  }, time.wait, target(0, 1, 20, 0), time.now), error => {
    assert.match(error.message, /did not settle within 3500 ms/);
    assert.equal(error.observations.length, 2);
    assert.equal(error.observations.at(-1).observed_ms, 3650);
    return true;
  });
});

test('final resolution preserves the frozen identity or fails before dispatch', () => {
  const frozen = target(64);
  const state = { frame_number: 3, physical_wheel_events: 1,
    virtualisation: { visible_rows: [64, 78] },
    ui_snapshot: { root: 'root', nodes: [{ id: 'root', rect: rootRect }] },
    ui_geometry: { root: rootRect, results_scroll: scrollRect, result_rows: [
      { result: 65, rect: { min_x: 200, min_y: 20, max_x: 600, max_y: 48 } },
    ] },
  };
  const missing = resolveResultRowTarget(state, canvas, 64);
  assert.equal(missing, null);
  assert.throws(() => assertFrozenResultTarget(frozen, missing), /Frozen Results target 64/);
  const moved = { ...frozen, rect: { ...frozen.rect, max_y: frozen.rect.max_y - 1 } };
  assert.throws(() => assertFrozenResultTarget(frozen, moved), /moved or disappeared/);
  assert.doesNotThrow(() => assertFrozenResultTarget(frozen, { ...frozen, frame: 100 }));
});
