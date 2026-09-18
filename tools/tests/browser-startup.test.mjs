import test from 'node:test';
import assert from 'node:assert/strict';
import { installStartup, installWorkerStartup, startApplication } from '../browser-startup.js';

function environment() {
  let now = 10, next = 0;
  const callbacks = new Map(), timers = new Map(), timerDelays = new Map(), listeners = new Map(), documentListeners = new Map(), marks = [];
  const classes = new Set();
  const loading = { textContent: '' };
  class GPUQueue { submit() { now += 1; } }
  const scope = {
    performance: { timeOrigin: 1000, now: () => now++, mark: name => marks.push(name), getEntriesByType: () => [] },
    navigator: { gpu: {} }, GPUQueue,
    document: { hidden: false, addEventListener: (name, fn) => documentListeners.set(name, fn), removeEventListener: name => documentListeners.delete(name), getElementById: () => loading, body: { classList: { add: name => classes.add(name), remove: name => classes.delete(name) } } },
    setTimeout: (fn, delay) => { timers.set(++next, fn); timerDelays.set(next, delay); return next; }, clearTimeout: id => { timers.delete(id); timerDelays.delete(id); },
    requestAnimationFrame: fn => { callbacks.set(++next, fn); return next; }, cancelAnimationFrame: id => callbacks.delete(id),
    addEventListener: (name, fn) => listeners.set(name, fn), removeEventListener: name => listeners.delete(name),
  };
  return { scope, marks, classes, loading, timers, timerDelays, listeners, documentListeners, callbacks, advance(ms) { now += ms; }, visibility(hidden) { scope.document.hidden = hidden; documentListeners.get('visibilitychange')?.(); }, frame() { const work = [...callbacks.values()]; callbacks.clear(); work.forEach(fn => fn()); } };
}

test('records only the first workspace submission, restores the queue and schedules one proxy', () => {
  const env = environment();
  const original = env.scope.GPUQueue.prototype.submit;
  const startup = installStartup({ app: 'test', scope: env.scope });
  const queue = new env.scope.GPUQueue();
  queue.submit();
  assert.equal(startup.report.milestones.workspace_frame_submitted, undefined);
  startup.mark('workspace_ui_complete');
  queue.submit();
  assert.ok(startup.report.milestones.workspace_frame_submitted);
  assert.equal(env.scope.GPUQueue.prototype.submit, original);
  assert.equal(startup.report.milestones.rendering_opportunity_proxy, undefined);
  assert.equal(env.callbacks.size, 1);
  const first = startup.report.milestones.workspace_frame_submitted;
  assert.equal(startup.mark('workspace_frame_submitted'), false);
  assert.deepEqual(startup.report.milestones.workspace_frame_submitted, first);
  startup.setMemory({ buffer: { byteLength: 65536 } });
  startup.mark('first_useful_content');
  assert.equal(startup.report.status, 'starting');
  env.frame();
  assert.equal(startup.report.status, 'ready');
  assert.equal(env.callbacks.size, 0);
  assert.equal(env.timers.size, 0);
  assert.equal(env.listeners.size, 0);
  assert.equal(startup.report.memory.main.first_useful_content.wasmLinearBytes, 65536);
  assert.equal(startup.fail('late', 'ignored'), false);
});

test('missing queue observation retains an honest CPU handoff/proxy fallback', () => {
  const env = environment(); delete env.scope.GPUQueue;
  const startup = installStartup({ app: 'test', scope: env.scope });
  startup.mark('workspace_ui_complete'); startup.mark('first_useful_content'); env.frame();
  assert.equal(startup.report.status, 'ready');
  assert.equal(startup.report.submissionObservation, 'unavailable');
  assert.equal(startup.report.milestones.workspace_frame_submitted, undefined);
  assert.ok(startup.report.milestones.rendering_opportunity_proxy);
});

test('failure is terminal, phase-labelled, restores observation and keeps the error shell visible', () => {
  const env = environment(), original = env.scope.GPUQueue.prototype.submit;
  const startup = installStartup({ app: 'test', scope: env.scope });
  startup.mark('workspace_ui_complete');
  startup.fail('worker:wasm_init', new Error('missing asset'));
  assert.equal(startup.report.status, 'failed');
  assert.equal(startup.report.failure.phase, 'worker:wasm_init');
  assert.match(env.loading.textContent, /worker:wasm_init.*missing asset/);
  assert.equal(env.classes.has('ready'), false);
  assert.equal(env.scope.GPUQueue.prototype.submit, original);
  assert.equal(startup.mark('first_useful_content'), false);
  assert.equal(startup.fail('later', 'overwrite'), false);
  assert.equal(env.timers.size, 0);
});

test('worker envelopes use their own clock/memory and leave decode messages untouched', () => {
  const env = environment();
  const startup = installStartup({ app: 'test', workerUrl: 'https://example.test/a/worker.js', scope: env.scope });
  assert.equal(env.scope.__polyoramaWorkerUrl(), 'https://example.test/a/worker.js');
  const worker = { timeOrigin: 2000, milestones: { worker_ready: { atMs: 4, epochMs: 2004 } }, memory: { worker_ready: { wasmLinearBytes: 131072 } }, resources: [] };
  assert.equal(env.scope.__polyoramaStartupWorkerMessage({ Completed: {} }), false);
  assert.equal(env.scope.__polyoramaStartupWorkerMessage({ __polyoramaStartup: worker }), true);
  assert.equal(startup.report.worker.timeOrigin, 2000);
  assert.deepEqual(startup.report.memory.worker, worker.memory);
  assert.deepEqual(startup.report.memory.main, {});
  assert.ok(startup.report.milestones.worker_ready);
  startup.fail('test', 'finish');
});

test('module/init and unsupported WebGPU errors become reported terminal failures', async () => {
  for (const missingGpu of [false, true]) {
    const env = environment();
    if (missingGpu) delete env.scope.navigator.gpu;
    const startup = await startApplication({ app: 'test', scope: env.scope, canvasId: 'canvas', handleName: 'handle',
      load: async () => {
        if (!missingGpu) throw new Error('module unavailable');
        return { default: async () => ({ memory: { buffer: { byteLength: 65536 } } }) };
      }, start() { assert.fail('must not start'); },
    });
    assert.equal(startup.report.status, 'failed');
    assert.equal(startup.report.failure.phase, missingGpu ? 'framework_start' : 'wasm_init');
  }
});

test('worker publishes bounded one-shot timing snapshots and terminal errors', () => {
  const env = environment(), messages = [];
  env.scope.postMessage = value => messages.push(value);
  const worker = installWorkerStartup({ scope: env.scope });
  worker.mark('wasm_init_begin'); worker.setMemory({ buffer: { byteLength: 131072 } });
  worker.mark('wasm_init_end'); worker.mark('worker_ready'); worker.mark('worker_ready');
  worker.mark('first_decode_complete'); worker.mark('first_decode_complete');
  assert.equal(messages.length, 2);
  assert.equal(messages[0].__polyoramaStartup.milestones.first_decode_complete, undefined);
  assert.equal(messages[1].__polyoramaStartup.memory.first_decode_complete.wasmLinearBytes, 131072);
  worker.fail('decode', 'bad input'); worker.fail('again', 'ignored'); worker.mark('later');
  assert.equal(messages.length, 3);
  assert.equal(messages[2].__polyoramaStartup.failure.phase, 'decode');
});

test('a worker startup failure prevents readiness and cannot be overwritten', () => {
  const env = environment();
  const startup = installStartup({ app: 'test', workerUrl: 'worker.js', scope: env.scope });
  env.scope.__polyoramaStartupWorkerMessage({ __polyoramaStartup: { memory: {}, failure: { phase: 'catalogue', message: 'HTTP 404' } } });
  assert.equal(startup.report.failure.phase, 'worker:catalogue');
  assert.equal(startup.mark('worker_ready'), false);
});

test('main and worker streaming observers count real calls and restore the original API', async () => {
  for (const worker of [false, true]) {
    const env = environment();
    const original = async () => ({ instance: {} });
    env.scope.WebAssembly = { instantiateStreaming: original };
    env.scope.postMessage = () => {};
    const startup = worker ? installWorkerStartup({ scope: env.scope }) : installStartup({ app: 'test', scope: env.scope });
    const observation = env.scope.WebAssembly.instantiateStreaming;
    assert.notEqual(observation, original);
    await env.scope.WebAssembly.instantiateStreaming(Promise.resolve({}));
    startup.setMemory({ buffer: { byteLength: 65536 } });
    assert.equal(env.scope.WebAssembly.instantiateStreaming, original);
    if (!worker) {
      assert.deepEqual(startup.report.instantiateStreaming, { available: true, observed: true, attempts: 1, completed: 1, failures: 0 });
      startup.fail('test', 'finish');
    }
  }
});

test('a timeout and an uncaught construction error retain their terminal phase', () => {
  for (const timeout of [false, true]) {
    const env = environment();
    const startup = installStartup({ app: 'test', scope: env.scope });
    startup.mark('application_construct_begin');
    if (timeout) [...env.timers.values()][0]();
    else env.listeners.get('error')({ error: new Error('construction failed') });
    assert.equal(startup.report.failure.phase, 'application_construct');
    assert.equal(startup.report.status, 'failed');
    assert.equal(env.listeners.size, 0);
  }
});

test('an asynchronous terminal failure stops bootstrap before framework construction', async () => {
  const env = environment();
  const startup = await startApplication({ app: 'test', scope: env.scope,
    load: async () => ({ default: async () => {
      env.scope.__polyoramaStartupFail('worker', 'failed during init');
      return { memory: { buffer: { byteLength: 65536 } } };
    }, WebHandle: class { constructor() { assert.fail('must not construct'); } } }),
  });
  assert.equal(startup.report.failure.phase, 'worker');
  assert.equal(startup.report.milestones.wasm_init_end, undefined);
});

test('failure after framework construction releases the handle outside the Rust callback', async () => {
  const env = environment();
  const pending = []; env.scope.queueMicrotask = fn => pending.push(fn);
  let destroyed = 0;
  const startup = await startApplication({ app: 'test', scope: env.scope,
    load: async () => ({ default: async () => ({ memory: { buffer: { byteLength: 65536 } } }), WebHandle: class { destroy() { destroyed++; } } }),
    start: async () => {},
  });
  startup.fail('worker', 'failed before useful content');
  assert.equal(destroyed, 0);
  assert.equal(pending.length, 1);
  pending[0]();
  assert.equal(destroyed, 1);
  assert.equal(startup.report.status, 'failed');
});

function workspaceFrame(env, startup) {
  startup.mark('workspace_ui_complete');
  new env.scope.GPUQueue().submit();
  env.frame();
}

function workerEnvelope(milestones, memory = {}) {
  return { __polyoramaStartup: { timeOrigin: 2000, milestones, memory, resources: [] } };
}

test('a usable saved layout without imagery clears the watchdog and never destroys its handle', async () => {
  const env = environment(), deferred = [];
  env.scope.queueMicrotask = fn => deferred.push(fn);
  let destroyed = 0;
  const originalSubmit = env.scope.GPUQueue.prototype.submit;
  const startup = await startApplication({ app: 'lab', workerUrl: '/worker.js', scope: env.scope,
    load: async () => ({ default: async () => ({ memory: { buffer: { byteLength: 65536 } } }), WebHandle: class { destroy() { destroyed++; } } }),
    start: async () => {},
  });
  const staleTimer = [...env.timers.values()][0];
  workspaceFrame(env, startup);
  assert.equal(startup.report.status, 'starting', 'worker readiness is still required');
  env.scope.__polyoramaStartupWorkerMessage(workerEnvelope({ worker_ready: { atMs: 1, epochMs: 2001 } }));
  assert.equal(startup.report.status, 'ready');
  assert.equal(startup.report.milestones.first_useful_content, undefined);
  assert.equal(startup.report.memory.main.first_useful_content, undefined);
  assert.equal(env.timers.size, 0);
  assert.equal(env.listeners.size, 0);
  assert.equal(env.documentListeners.size, 0);
  assert.equal(env.scope.GPUQueue.prototype.submit, originalSubmit);
  env.advance(120000); staleTimer();
  assert.equal(deferred.length, 0);
  assert.equal(destroyed, 0);
  assert.equal(startup.report.status, 'ready');
  assert.equal(startup.report.failure, null);
});

test('first content can arrive once after readiness and captures memory at that later boundary', () => {
  const env = environment();
  const startup = installStartup({ app: 'test', scope: env.scope });
  const memory = { buffer: { byteLength: 65536 } };
  startup.setMemory(memory);
  workspaceFrame(env, startup);
  assert.equal(startup.report.status, 'ready');
  memory.buffer = { byteLength: 262144 };
  env.advance(1000);
  assert.equal(startup.mark('first_useful_content'), true);
  const first = structuredClone(startup.report.milestones.first_useful_content);
  const sample = structuredClone(startup.report.memory.main.first_useful_content);
  assert.equal(sample.wasmLinearBytes, 262144);
  assert.ok(first.atMs > startup.report.milestones.rendering_opportunity_proxy.atMs);
  memory.buffer = { byteLength: 524288 };
  assert.equal(startup.mark('first_useful_content'), false);
  assert.deepEqual(startup.report.milestones.first_useful_content, first);
  assert.deepEqual(startup.report.memory.main.first_useful_content, sample);
  assert.equal(startup.mark('unexpected_late_milestone'), false);
  assert.equal(env.timers.size, 0);
});

test('the worker first-decode report survives readiness without turning runtime errors into startup failure', () => {
  const env = environment(); let failures = 0;
  const startup = installStartup({ app: 'lab', workerUrl: '/worker.js', scope: env.scope, onFailure: () => failures++ });
  const ready = { worker_ready: { atMs: 1, epochMs: 2001 } };
  env.scope.__polyoramaStartupWorkerMessage(workerEnvelope(ready));
  workspaceFrame(env, startup);
  assert.equal(startup.report.status, 'ready');
  env.scope.__polyoramaStartupWorkerMessage({ __polyoramaStartup: { failure: { phase: 'decode', message: 'runtime error' } } });
  assert.equal(startup.report.worker.failure, undefined);
  const decoded = { ...ready, first_decode_complete: { atMs: 10, epochMs: 2010 } };
  const memory = { first_decode_complete: { wasmLinearBytes: 131072 } };
  env.scope.__polyoramaStartupWorkerMessage(workerEnvelope(decoded, memory));
  assert.deepEqual(startup.report.worker.milestones, decoded);
  assert.deepEqual(startup.report.memory.worker, memory);
  env.scope.__polyoramaStartupWorkerMessage(workerEnvelope(decoded, { changed: true }));
  assert.deepEqual(startup.report.memory.worker, memory, 'first decode sample remains one-shot');
  assert.equal(startup.fail('worker_transport', 'runtime transport error'), false);
  assert.equal(failures, 0);
  assert.equal(startup.report.failure, null);
});

test('a hidden page starts with no watchdog and resumes its foreground budget on visibility', () => {
  const env = environment(); env.scope.document.hidden = true;
  const startup = installStartup({ app: 'test', scope: env.scope, timeoutMs: 1000 });
  assert.equal(env.timers.size, 0);
  assert.equal(env.documentListeners.size, 1);
  env.advance(120000);
  env.visibility(false);
  assert.equal(env.timers.size, 1);
  assert.equal([...env.timerDelays.values()][0], 1000);
  env.advance(200);
  env.visibility(true);
  assert.equal(env.timers.size, 0);
  env.advance(120000);
  env.visibility(false);
  const remaining = [...env.timerDelays.values()][0];
  assert.ok(remaining >= 790 && remaining <= 800);
  workspaceFrame(env, startup);
  assert.equal(startup.report.status, 'ready');
  assert.equal(env.timers.size, 0);
  assert.equal(env.documentListeners.size, 0);
});

test('an expired foreground watchdog still fails startup and removes visibility observation', () => {
  const env = environment(); let failures = 0;
  const startup = installStartup({ app: 'test', scope: env.scope, onFailure: () => failures++ });
  [...env.timers.values()][0]();
  assert.equal(startup.report.status, 'failed');
  assert.match(startup.report.failure.message, /foreground time/);
  assert.equal(failures, 1);
  assert.equal(env.documentListeners.size, 0);
  assert.equal(env.timers.size, 0);
});

test('a delayed watchdog callback seeing a hidden page cannot destroy or spend its unknown background time', () => {
  const env = environment();
  const startup = installStartup({ app: 'test', scope: env.scope, timeoutMs: 1000 });
  const callback = [...env.timers.values()][0];
  env.scope.document.hidden = true;
  env.advance(120000);
  callback();
  assert.equal(startup.report.status, 'starting');
  assert.equal(env.timers.size, 0);
  env.visibility(false);
  assert.equal([...env.timerDelays.values()][0], 1000);
  workspaceFrame(env, startup);
  assert.equal(startup.report.status, 'ready');
});
