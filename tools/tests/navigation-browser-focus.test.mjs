import assert from 'node:assert/strict';
import test from 'node:test';
import { focusNavigationDestination } from '../navigation-browser-focus.mjs';
import { focusedId } from '../browser-keyboard-focus.mjs';

// A Tab cycle of `controls` controls, with focus starting on the first; the
// target is focused after `reachedAfter` Tabs.
function fixture(reachedAfter, controls = Infinity) {
  const state = { dispatches: 0, waits: [], observations: 0 };
  const callbacks = {
    target: async destination => {
      assert.equal(destination, 'tasks');
      state.observations++;
      const focused = state.waits.length >= reachedAfter;
      return { node: { focused }, focused_id: focused ? 'tasks' : `control-${state.waits.length % controls}`, tab_input_epoch: state.dispatches };
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

test('a target behind hundreds of controls needs no limit', async () => {
  const { state, callbacks } = fixture(600);
  await focusNavigationDestination('tasks', callbacks);
  assert.equal(state.dispatches, 600);
  assert.deepEqual(state.waits, Array(600).fill(60));
  assert.equal(state.observations, 601);
});

test('a target outside the Tab cycle fails once focus returns to a visited control', async () => {
  const { state, callbacks } = fixture(Infinity, 70);
  await assert.rejects(focusNavigationDestination('tasks', callbacks), /Tab did not reach tasks: focus returned to control-0 after 70 controls/);
  assert.equal(state.dispatches, 70);
  assert.equal(state.observations, 71);
});

test('a brief unfocused wrap continues the walk', async () => {
  const { state, callbacks } = fixture(5);
  const observe = callbacks.target;
  callbacks.target = async destination => {
    const observation = await observe(destination);
    return state.waits.length === 2 || state.waits.length === 3 ? { ...observation, focused_id: null } : observation;
  };
  await focusNavigationDestination('tasks', callbacks);
  assert.equal(state.dispatches, 5);
});

test('focus that stays on one control fails after the stall limit', async () => {
  const { state, callbacks } = fixture(Infinity);
  const observe = callbacks.target;
  callbacks.target = async destination => ({ ...(await observe(destination)), focused_id: 'control-0' });
  await assert.rejects(focusNavigationDestination('tasks', callbacks), /focus stayed on control-0 for 8 steps/);
  assert.equal(state.dispatches, 8);
});

test('controls outside the observed tree neither stall nor count', async () => {
  // Hundreds of unobserved controls (such as a catalogue) before the target.
  const { state, callbacks } = fixture(400);
  const observe = callbacks.target;
  callbacks.target = async destination => {
    const observation = await observe(destination);
    return observation.node.focused ? observation : { ...observation, focused_id: null };
  };
  await focusNavigationDestination('tasks', callbacks);
  assert.equal(state.dispatches, 400);
});

test('a cycle with unobserved controls between observed ones still fails on a repeat', async () => {
  const { state, callbacks } = fixture(Infinity, 10);
  const observe = callbacks.target;
  callbacks.target = async destination => {
    const observation = await observe(destination);
    return observation.focused_id === 'control-3' ? observation : { ...observation, focused_id: null };
  };
  await assert.rejects(focusNavigationDestination('tasks', callbacks), /focus returned to control-3 after 1 controls/);
  assert.equal(state.dispatches, 13);
});

test('focus that never reaches an observed control stops at the safety limit', async () => {
  const { state, callbacks } = fixture(Infinity);
  const observe = callbacks.target;
  callbacks.target = async destination => ({ ...(await observe(destination)), focused_id: null });
  await assert.rejects(focusNavigationDestination('tasks', { ...callbacks, safetyLimit: 40 }), /within 40 steps/);
  assert.equal(state.dispatches, 40);
});

test('a walk whose focus never repeats stops at the safety limit', async () => {
  const { state, callbacks } = fixture(Infinity);
  await assert.rejects(focusNavigationDestination('tasks', { ...callbacks, safetyLimit: 30 }), /within 30 steps/);
  assert.equal(state.dispatches, 30);
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

test('caller pacing is kept for every Tab', async () => {
  const { state, callbacks } = fixture(4);
  await focusNavigationDestination('tasks', { ...callbacks, settleMs: 50 });
  assert.deepEqual(state.waits, Array(4).fill(50));
});

test('the focused ID comes from the focused node', () => {
  assert.equal(focusedId([{ id: 'a', focused: false }, { id: 'b', focused: true }]), 'b');
  assert.equal(focusedId([{ id: 'a', focused: false }]), null);
});
