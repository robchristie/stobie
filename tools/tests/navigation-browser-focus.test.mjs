import assert from 'node:assert/strict';
import test from 'node:test';
import { focusNavigationDestination } from '../navigation-browser-focus.mjs';

function fixture(reachedAfter) {
  const state = { dispatches: 0, waits: [], observations: 0 };
  const callbacks = {
    target: async destination => {
      assert.equal(destination, 'tasks');
      state.observations++;
      return { node: { focused: state.waits.length >= reachedAfter }, tab_input_epoch: state.dispatches };
    },
    pressTab: async () => { state.dispatches++; },
    waitForTab: async epoch => { assert.equal(state.dispatches, epoch + 1); },
    wait: async ms => { state.waits.push(ms); },
  };
  return { state, callbacks };
}

test('target already focused uses no keys or waits', async () => {
  const { state, callbacks } = fixture(0);
  await focusNavigationDestination('tasks', callbacks);
  assert.deepEqual(state, { dispatches: 0, waits: [], observations: 1 });
});

test('earlier focus stops before another key', async () => {
  const { state, callbacks } = fixture(3);
  await focusNavigationDestination('tasks', callbacks);
  assert.equal(state.dispatches, 3);
  assert.deepEqual(state.waits, [60, 60, 60]);
  assert.equal(state.observations, 4);
});

test('focus after final allowed action succeeds without a forty-sixth key', async () => {
  const { state, callbacks } = fixture(45);
  await focusNavigationDestination('tasks', callbacks);
  assert.equal(state.dispatches, 45);
  assert.deepEqual(state.waits, Array(45).fill(60));
  assert.equal(state.observations, 46);
});

test('unreachable target fails after final observation with unchanged action/pacing bounds', async () => {
  const { state, callbacks } = fixture(Infinity);
  await assert.rejects(focusNavigationDestination('tasks', callbacks), /Tab did not reach tasks/);
  assert.equal(state.dispatches, 45);
  assert.deepEqual(state.waits, Array(45).fill(60));
  assert.equal(state.observations, 46);
});

test('target observation rejection propagates without another traversal', async () => {
  const { state, callbacks } = fixture(Infinity);
  const failure = new Error('missing destination node');
  callbacks.target = async () => { throw failure; };
  await assert.rejects(focusNavigationDestination('tasks', callbacks), error => error === failure);
  assert.equal(state.dispatches, 0);
  assert.equal(state.waits.length, 0);
});

test('dispatch rejection propagates without a wait or another key', async () => {
  const { state, callbacks } = fixture(Infinity);
  const failure = new Error('keyboard dispatch failed');
  callbacks.pressTab = async () => { state.dispatches++; throw failure; };
  await assert.rejects(focusNavigationDestination('tasks', callbacks), error => error === failure);
  assert.equal(state.dispatches, 1);
  assert.equal(state.waits.length, 0);
});

test('wait rejection propagates without another key', async () => {
  const { state, callbacks } = fixture(Infinity);
  const failure = new Error('page closed during wait');
  callbacks.wait = async ms => { state.waits.push(ms); throw failure; };
  await assert.rejects(focusNavigationDestination('tasks', callbacks), error => error === failure);
  assert.equal(state.dispatches, 1);
  assert.deepEqual(state.waits, [60]);
});

test('delayed input receipt prevents a second queued Tab', async () => {
  const state = { dispatched: 0, received: 0 };
  let acknowledge;
  const completion = focusNavigationDestination('tasks', {
    target: async () => ({ node: { focused: state.received === 1 }, tab_input_epoch: state.received }),
    pressTab: async () => { state.dispatched++; },
    waitForTab: () => new Promise(resolve => { acknowledge = () => { state.received++; resolve(); }; }),
    wait: async () => {},
  });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(state.dispatched, 1);
  assert.equal(state.received, 0);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(state.dispatched, 1);
  acknowledge();
  await completion;
  assert.equal(state.dispatched, 1);
});

test('missing receipt fails without another key or a settling delay', async () => {
  const { state, callbacks } = fixture(Infinity);
  const failure = new Error('application did not process Tab');
  callbacks.waitForTab = async () => { throw failure; };
  await assert.rejects(focusNavigationDestination('tasks', callbacks), error => error === failure);
  assert.equal(state.dispatches, 1);
  assert.deepEqual(state.waits, []);
});

test('caller traversal bounds retain final-action observation and pacing', async () => {
  for (const [maxTabs, settleMs] of [[24, 50], [40, 60]]) {
    const { state, callbacks } = fixture(maxTabs);
    await focusNavigationDestination('tasks', { ...callbacks, maxTabs, settleMs });
    assert.equal(state.dispatches, maxTabs);
    assert.deepEqual(state.waits, Array(maxTabs).fill(settleMs));
    assert.equal(state.observations, maxTabs + 1);
  }
});
