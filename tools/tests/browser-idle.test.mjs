import assert from 'node:assert/strict';
import test from 'node:test';
import { observeWarmedIdle } from '../browser-idle.mjs';

function probe(frameAt, options) {
  let elapsed = 0;
  return observeWarmedIdle(() => frameAt(elapsed), async (ms) => { elapsed += ms; }, options);
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

test('one late burst after the quiet window settles again and is recorded', async () => {
  const result = await probe((elapsed) => elapsed < 1000 ? 10 : 11);
  assert.deepEqual(result.late_bursts, [{ from: 10, to: 11 }]);
  assert.equal(result.frame_before, 11);
  assert.equal(result.frame_after, 11);
  assert.equal(result.settle_observed_ms, 1400);
});

test('presentation in the idle window fails without a late-burst allowance', async () => {
  await assert.rejects(probe((elapsed) => elapsed < 1000 ? 10 : 11, { lateBurstLimit: 0 }), /repainted after settling \(10 -> 11\)$/);
});

test('a second repaint after settling again still fails', async () => {
  await assert.rejects(
    probe((elapsed) => elapsed < 1000 ? 10 : elapsed < 2500 ? 11 : 12),
    /repainted after settling \(11 -> 12\), after a late burst \(10 -> 11\)/,
  );
});

test('a repaint every second fails although each settles first', async () => {
  await assert.rejects(probe((elapsed) => Math.floor(elapsed / 1000)), /repainted after settling/);
});

test('missing or malformed observations cannot establish idle', async () => {
  for (const frame of [undefined, null, NaN, -1, 1.5]) {
    await assert.rejects(probe(() => frame), /Invalid presentation frame counter/);
  }
});
