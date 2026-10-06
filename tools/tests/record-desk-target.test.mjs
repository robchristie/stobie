import assert from 'node:assert/strict';
import test from 'node:test';
import { waitForClickTarget } from '../record-desk-target.mjs';

const rect = { min_x: 10, min_y: 20, max_x: 80, max_y: 50 };
function state(targets) {
  return { ui: { root: 'root', nodes: [
    { id: 'root', rect: { min_x: 0, min_y: 0, max_x: 1080, max_y: 760 }, actions: [] },
    ...targets,
  ] } };
}
const target = (overrides = {}) => ({ id: 'choice.option.Ideas', rect, enabled: true, actions: ['choice'], ...overrides });
async function probe(at, id = 'choice.option.Ideas') {
  let elapsed = 0;
  const result = await waitForClickTarget(() => at(elapsed), async ms => { elapsed += ms; }, id);
  return { ...result, elapsed };
}

test('transient popup absence resets confirmation before a physical click', async () => {
  const result = await probe(ms => state(ms === 50 ? [] : [target()]));
  assert.equal(result.elapsed, 200);
  assert.equal(result.node.id, 'choice.option.Ideas');
});

test('moving popup geometry must stabilise before it supplies the click target', async () => {
  const moving = await probe(ms => state([target({ rect: { ...rect, min_y: ms < 100 ? ms / 10 : 20 } })]));
  assert.equal(moving.elapsed, 200);
  assert.deepEqual(moving.node.rect, rect);
});

test('an exact semantic identity takes priority over another control sharing its action', async () => {
  const result = await probe(() => state([target({ id: 'other', actions: ['choice.option.Ideas'] }), target()]));
  assert.equal(result.node.id, 'choice.option.Ideas');
  assert.equal(result.elapsed, 100); // An idle snapshot needs no extra repaint.
});

test('geometry that keeps moving cannot satisfy bounded confirmation', async () => {
  await assert.rejects(probe(ms => state([target({ rect: { ...rect, min_x: ms % 100 === 0 ? 0 : 10 } })])), /within 15000 ms/);
});

test('missing or disabled targets fail within the observation budget', async () => {
  for (const targets of [[], [target({ enabled: false })]]) {
    await assert.rejects(probe(() => state(targets)), /within 15000 ms/);
  }
});

test('invalid geometry fails visibly instead of supplying a physical target', async () => {
  for (const invalid of [null, { ...rect, min_x: NaN }, { ...rect, max_x: 10 }]) {
    await assert.rejects(probe(() => state([target({ rect: invalid })])), /Invalid target geometry/);
  }
});
