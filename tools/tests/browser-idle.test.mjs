import assert from 'node:assert/strict';
import test from 'node:test';
import { observeWarmedIdle } from '../browser-idle.mjs';

function probe(frameAt) {
  let elapsed = 0;
  return observeWarmedIdle(() => frameAt(elapsed), async (ms) => { elapsed += ms; });
}

test('stable presentation has a separate settling and idle observation', async () => {
  const result = await probe(() => 10);
  assert.equal(result.frame_before, result.frame_after);
  assert.equal(result.settle_observed_ms, 700);
  assert.equal(result.idle_observed_ms, 700);
});

test('one deferred interaction frame settles before the unchanged idle gate', async () => {
  const result = await probe((elapsed) => elapsed < 300 ? 10 : 11);
  assert.equal(result.settle_initial_frame, 10);
  assert.equal(result.settle_final_frame, 11);
  assert.equal(result.frame_before, 11);
  assert.equal(result.frame_after, 11);
  assert.equal(result.settle_observed_ms, 1000);
});

test('continuous presentation cannot satisfy bounded settling', async () => {
  await assert.rejects(probe((elapsed) => elapsed / 100), /did not settle within 3500 ms/);
});

test('periodic presentation cannot satisfy bounded settling', async () => {
  await assert.rejects(probe((elapsed) => Math.floor(elapsed / 500)), /did not settle within 3500 ms/);
});

test('presentation during the final idle window still fails', async () => {
  await assert.rejects(probe((elapsed) => elapsed < 1000 ? 10 : 11), /repainted after settling/);
});

test('missing or malformed observations cannot establish idle', async () => {
  for (const frame of [undefined, null, NaN, -1, 1.5]) {
    await assert.rejects(probe(() => frame), /Invalid presentation frame counter/);
  }
});
