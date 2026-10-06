import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:net';
import { ApplicationClient, ApplicationError, SocketTransport, BrowserTransport,
  BrowserPhysicalAdapter, NativePhysicalAdapter, matchesSelector, describeApplicationError } from '../application-client.mjs';

const rootRect = { min_x: 10, min_y: 20, max_x: 410, max_y: 320 };
const targetRect = { min_x: 30, min_y: 40, max_x: 90, max_y: 80 };
const domain = { kind: 'external', value: { namespace: 'records', id: '23' } };
const node = (overrides = {}) => ({ id: 'apply', role: 'button', name: 'Apply', enabled: true,
  focused: false, selected: false, pane: 7, domain_reference: domain, actions: ['record.apply'], rect: targetRect, ...overrides });
const observation = (targets = [node()], id = 1, scale = 1) => ({ id,
  snapshot: { frame: id, root: 'root', pixels_per_point: scale, nodes: [node({ id: 'root', role: 'application', actions: [], rect: rootRect }), ...targets], text: [], text_audit: [], semantic_audit: [] },
  collection: { viewport: 'main', boundary: 'completed_ui_pass', collections: [] }, facts: {} });
const target = { id: 'apply', capability: 'record.apply', pane: 7, domain, meaning: 'apply:23', observation: 1 };
const fakeClock = () => ({ elapsed: 0, now() { return this.elapsed; }, async sleep(ms, signal) {
  if (signal?.aborted) throw new ApplicationError('cancelled', 'Cancelled'); this.elapsed += ms;
} });

function fixture(handler = () => ({ kind: 'observe', observation: observation() }), scheduler = fakeClock()) {
  const requests = [];
  const transport = { kind: 'fake', async request(raw, options) {
    const request = JSON.parse(raw); requests.push(request);
    const result = request.operation.op === 'hello' ? { kind: 'hello', application: { name: 'Test', build: { version: '1', source_revision: 'abc' } }, limits: {} }
      : await handler(request.operation, request, options);
    if (typeof result === 'string') return result;
    return JSON.stringify({ version: 1, request_id: request.request_id, instance: 'instance-1', ...result?.error ? result : { result } });
  } };
  const client = new ApplicationClient(transport, { scheduler });
  return { client, requests, scheduler };
}
async function bound(handler, scheduler) { const result = fixture(handler, scheduler); await result.client.hello(); return result; }
function adapter(geometry = { x: 100, y: 200, width: 400, height: 300, device_scale: 1 }) {
  const native = new NativePhysicalAdapter(17);
  const calls = [];
  return { calls, geometry: async () => geometry, point: (target, box) => native.point(target, box),
    click: async point => calls.push({ input: 'click', point }), key: async key => calls.push({ input: 'key', key }),
    text: async value => calls.push({ input: 'text', value }), capture: async path => calls.push({ input: 'capture', path }) };
}

test('hello binds build and instance; subsequent requests carry it', async () => {
  const { client, requests } = await bound();
  await client.observe();
  assert.equal(client.application.build.source_revision, 'abc');
  assert.equal(requests[0].instance, undefined);
  assert.equal(requests[1].instance, 'instance-1');
});

test('unbound reads and unknown selector fields fail before dispatch', async () => {
  const { client, requests } = fixture();
  await assert.rejects(client.observe(), { code: 'unbound' });
  await client.hello();
  await assert.rejects(client.query({ typo: 'apply' }), { code: 'invalid_request' });
  assert.equal(requests.length, 1);
});

test('selectors combine exact identity, capability, pane and structural domain equality', () => {
  assert.equal(matchesSelector(node(), { id: 'apply', capability: 'record.apply', pane: 7,
    domain: { value: { id: '23', namespace: 'records' }, kind: 'external' } }), true);
  for (const selector of [{ id: 'other' }, { pane: 9 }, { capability: 'other' }, { domain: { kind: 'pane', value: 7 } }]) {
    assert.equal(matchesSelector(node(), selector), false);
  }
});

test('bounded query passes the cursor and rejects excessive limits', async () => {
  const { client, requests } = await bound(() => ({ kind: 'query', total: 0, observation: 1, nodes: [], next_cursor: null }));
  const cursor = { observation: 1, offset: 12 };
  await client.query({}, { limit: 12, cursor });
  assert.deepEqual(requests.at(-1).operation, { op: 'query', selector: {}, limit: 12, cursor });
  await assert.rejects(client.query({}, { limit: 257 }), { code: 'invalid_request' });
});

test('old instance responses refuse restart and retain the original binding', async () => {
  const { client } = await bound((_, request) => JSON.stringify({ version: 1, request_id: request.request_id, instance: 'instance-2', result: { kind: 'observe', observation: observation() } }));
  await assert.rejects(client.observe(), { code: 'wrong_instance' });
  assert.equal(client.instance, 'instance-1');
});

test('unanswered reads time out and cancellation interrupts an in-flight wait', async () => {
  const { client } = await bound(() => new Promise(() => {}));
  await assert.rejects(client.observe({ timeoutMs: 15 }), { code: 'timeout' });
  const controller = new AbortController();
  const waiting = client.wait({ condition: 'enabled', selector: { id: 'apply' } }, { signal: controller.signal });
  controller.abort();
  await assert.rejects(waiting, { code: 'cancelled' });
});

test('cancelled invocation before dispatch sends nothing', async () => {
  const { client, requests } = await bound();
  const controller = new AbortController(); controller.abort();
  await assert.rejects(client.invoke({ capability: 'record.apply' }, { expected: target, signal: controller.signal }), { code: 'cancelled' });
  assert.equal(requests.length, 1);
});

test('a missing first observation remains diagnosable at the monotonic deadline', async () => {
  const { client, scheduler } = await bound(() => ({ error: { code: 'missing_observation', message: 'No completed pass' } }));
  await assert.rejects(client.wait({ condition: 'present', selector: { id: 'apply' } }, { timeoutMs: 120 }), error => {
    assert.equal(error.code, 'timeout'); assert.equal(error.last_error.code, 'missing_observation'); return true;
  });
  assert.equal(scheduler.elapsed, 120);
});

test('waits use completed idle observations without a new frame', async () => {
  const { client, requests } = await bound();
  assert.equal((await client.wait({ condition: 'enabled', selector: { id: 'apply' } })).observation.id, 1);
  assert.equal((await client.target({ id: 'apply' })).observation.id, 1);
  assert.equal(requests.filter(request => request.operation.op === 'observe').length, 4);
  assert.ok(requests.every(request => ['hello', 'observe'].includes(request.operation.op)));
});

test('checked waits observe toggle state and reject non-boolean expectations', async () => {
  const { client } = await bound(() => ({ kind: 'observe', observation: observation([node({ checked: true })]) }));
  assert.equal((await client.wait({ condition: 'checked', selector: { id: 'apply' }, value: true })).node.checked, true);
  await assert.rejects(client.wait({ condition: 'checked', selector: { id: 'apply' }, value: 'true' }), { code: 'invalid_request' });
});

test('an enabled capability without a rendered control satisfies an idle wait', async () => {
  const { client } = await bound(operation => operation.op === 'discover'
    ? { kind: 'discover', observation: 1, total: 1, capabilities: [{ availability: { state: 'enabled' }, target }] }
    : { kind: 'observe', observation: observation([]) });
  const result = await client.wait({ condition: 'capability_enabled', selector: { capability: 'record.apply' } });
  assert.equal(result.capability.availability.state, 'enabled');
  assert.equal(result.capability_observation, 1);
  assert.equal(result.node, null);
});

test('disabled physical targets retain the latest observation and reason for CLI diagnostics', async () => {
  const { client } = await bound(() => ({ kind: 'observe', observation: observation([node({ enabled: false, disabled_reason: 'No draft changes' })]) }));
  await assert.rejects(client.target({ id: 'apply' }, { timeoutMs: 120 }), error => {
    const diagnostic = describeApplicationError(error);
    assert.equal(diagnostic.code, 'timeout');
    assert.equal(diagnostic.observation.id, 1);
    assert.equal(diagnostic.last_reason, 'No draft changes');
    assert.equal(diagnostic.cause, undefined);
    return true;
  });
});

test('transport timeout after a valid wait observation retains that observation', async () => {
  let reads = 0;
  const { client } = await bound(() => {
    if (reads++ === 0) return { kind: 'observe', observation: observation() };
    throw new ApplicationError('timeout', 'Host read exceeded the remaining deadline');
  });
  await assert.rejects(client.wait({ condition: 'present', selector: { id: 'absent' } }), error => {
    assert.equal(error.code, 'timeout');
    assert.equal(error.observation.id, 1);
    assert.equal(error.last_error.code, 'timeout');
    return true;
  });
});

test('metadata write failure preserves observations and a captured image report', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'application-capture-'));
  try {
    const { client } = await bound();
    const physical = { geometry: async () => ({ width: 400, height: 300, device_scale: 1 }), capture: async () => {} };
    const report = await client.capture(physical, { path: join(directory, 'image.png'), metadataPath: directory });
    assert.equal(report.status, 'partial');
    assert.equal(report.capture.status, 'captured');
    assert.equal(report.before.observation.id, 1);
    assert.equal(report.after.observation.id, 1);
    assert.equal(report.diagnostics[0].stage, 'metadata');
  } finally { await rm(directory, { recursive: true }); }
});

test('name/status and changed selection waits need no application parser', async () => {
  let selected = 'one';
  const scheduler = fakeClock();
  scheduler.sleep = async function(ms) { this.elapsed += ms; selected = 'two'; };
  const { client } = await bound(() => ({ kind: 'observe', observation: observation([
    node({ id: 'one', selected: selected === 'one' }), node({ id: 'two', selected: selected === 'two' }),
    node({ id: 'status', role: 'status', name: 'Saved' }),
  ]) }), scheduler);
  await client.wait({ condition: 'status', selector: { id: 'status' }, value: 'Saved' });
  await client.wait({ condition: 'name', selector: { id: 'one' }, value: 'Apply' });
  const result = await client.wait({ condition: 'selection_changed', selector: { role: 'button' } });
  assert.equal(result.observation.snapshot.nodes.find(candidate => candidate.id === 'two').selected, true);
});

test('fact waits compare published primitive values and keep absent facts pending', async () => {
  const scheduler = fakeClock();
  const { client } = await bound(() => ({ kind: 'observe', observation: { ...observation(), facts:
    scheduler.elapsed >= 50 ? { undo_entries: 1, draft_dirty: false } : {} } }), scheduler);
  await client.wait({ condition: 'fact', key: 'undo_entries', value: 1 });
  assert.equal(scheduler.elapsed, 50);
  await client.wait({ condition: 'fact', key: 'draft_dirty', value: false });
});

test('ambiguous semantic physical selectors fail instead of selecting the first action', async () => {
  const { client } = await bound(() => ({ kind: 'observe', observation: observation([node(), node({ id: 'other' })]) }));
  await assert.rejects(client.target({ capability: 'record.apply' }), { code: 'ambiguous' });
  assert.equal((await client.target({ id: 'apply', capability: 'record.apply' })).node.id, 'apply');
});

test('moving, disappearing and disabled targets reset the 100 ms confirmation window', async () => {
  for (const scenario of ['moving', 'missing', 'disabled']) {
    const scheduler = fakeClock();
    const { client } = await bound(() => {
      const elapsed = scheduler.elapsed;
      const targets = scenario === 'missing' && elapsed === 50 ? [] : [node({
        enabled: !(scenario === 'disabled' && elapsed === 50),
        rect: scenario === 'moving' && elapsed < 100 ? { ...targetRect, min_x: elapsed / 10 + 20 } : targetRect,
      })];
      return { kind: 'observe', observation: observation(targets) };
    }, scheduler);
    assert.equal((await client.target({ id: 'apply' })).observed_ms, 200, scenario);
  }
});

test('missing/disabled and continuously moving targets respect the total budget', async () => {
  for (const mode of ['missing', 'disabled', 'moving']) {
    const scheduler = fakeClock();
    const { client } = await bound(() => ({ kind: 'observe', observation: observation(mode === 'missing' ? [] : [node({
      enabled: mode !== 'disabled', rect: { ...targetRect, min_x: mode === 'moving' ? 20 + scheduler.elapsed % 100 / 5 : 30 },
    })]) }), scheduler);
    await assert.rejects(client.target({ id: 'apply' }, { timeoutMs: 250 }), { code: 'timeout' });
    assert.equal(scheduler.elapsed, 250);
  }
});

test('invalid, clipped or non-finite geometry and scale never dispatch input', async () => {
  for (const [targets, scale, code] of [
    [[node({ rect: { ...targetRect, min_x: NaN } })], 1, 'invalid_geometry'],
    [[node({ rect: { ...targetRect, max_y: 400 } })], 1, 'clipped_target'],
    [[node()], 0, 'invalid_geometry'],
  ]) {
    const { client } = await bound(() => ({ kind: 'observe', observation: observation(targets, 1, scale) }));
    const physical = adapter();
    await assert.rejects(client.click({ id: 'apply' }, physical), { code });
    assert.equal(physical.calls.length, 0);
  }
});

test('physical coordinates account for logical root origin and scale', async () => {
  const { client } = await bound(() => ({ kind: 'observe', observation: observation([node()], 1, 2) }));
  const physical = adapter({ x: 100, y: 200, width: 800, height: 600, device_scale: 1 });
  const receipt = await client.click({ id: 'apply' }, physical);
  assert.deepEqual(receipt.point, { x: 200, y: 280 });
  assert.equal(receipt.route, 'physical');
  assert.equal(receipt.outcome, 'unverified');
  assert.equal(physical.calls.length, 1);
});

test('host geometry must agree with the observed viewport and scale', async () => {
  const { client } = await bound();
  await assert.rejects(client.click({ id: 'apply' }, adapter({ x: 0, y: 0, width: 500, height: 300, device_scale: 1 })), { code: 'invalid_geometry' });
});

test('physical recheck refuses movement, disappearance, disabled state or host movement', async () => {
  for (const mode of ['move', 'missing', 'disabled', 'host_move']) {
    let reads = 0;
    const { client } = await bound(() => {
      reads += 1;
      const final = reads === 4;
      return { kind: 'observe', observation: observation(final && mode === 'missing' ? [] : [node({
        enabled: !(final && mode === 'disabled'), rect: final && mode === 'move' ? { ...targetRect, min_x: 40 } : targetRect,
      })]) };
    });
    const physical = adapter();
    let geometryReads = 0;
    physical.geometry = async () => ({ x: mode === 'host_move' && ++geometryReads === 4 ? 101 : 100,
      y: 200, width: 400, height: 300, device_scale: 1 });
    await assert.rejects(client.click({ id: 'apply' }, physical), { code: 'stale_target' }, mode);
    assert.equal(physical.calls.length, 0);
  }
});

test('semantic invocation discovers one target and dispatches once while reading queued receipts', async () => {
  let receiptReads = 0;
  const { client, requests } = await bound(op => {
    if (op.op === 'discover') return { kind: 'discover', total: 1, capabilities: [{ target }] };
    return { kind: 'receipt', receipt: { request_id: 'mutation-1', route: 'semantic', state:
      op.op === 'invoke' || ++receiptReads === 1 ? 'queued' : 'completed' } };
  });
  const receipt = await client.invoke({ capability: 'record.apply', pane: 7 }, { requestId: 'mutation-1' });
  assert.equal(receipt.state, 'completed');
  assert.equal(requests.filter(request => request.operation.op === 'invoke').length, 1);
  assert.deepEqual(requests.find(request => request.operation.op === 'invoke').operation.expected, target);
});

test('ambiguous discovery prevents semantic invocation', async () => {
  const { client, requests } = await bound(() => ({ kind: 'discover', total: 2, capabilities: [{ target }, { target }] }));
  await assert.rejects(client.invoke({ capability: 'record.apply' }), { code: 'ambiguous' });
  assert.equal(requests.filter(request => request.operation.op === 'invoke').length, 0);
});

test('slow discovery cannot consume the deadline then dispatch a mutation', async () => {
  const scheduler = fakeClock();
  const { client, requests } = await bound(() => {
    scheduler.elapsed = 200;
    return { kind: 'discover', total: 1, capabilities: [{ target }] };
  }, scheduler);
  await assert.rejects(client.invoke({ id: 'apply' }, { timeoutMs: 100 }), { code: 'timeout' });
  assert.equal(requests.filter(request => request.operation.op === 'invoke').length, 0);
});

test('lost mutation reply retains its lookup ID and does not retry', async () => {
  const { client, requests } = await bound(() => new Promise(() => {}));
  await assert.rejects(client.invoke({ capability: 'record.apply' }, { expected: target, requestId: 'uncertain-1', timeoutMs: 15 }), error => {
    assert.equal(error.code, 'uncertain_completion'); assert.equal(error.request_id, 'uncertain-1'); return true;
  });
  assert.equal(requests.filter(request => request.operation.op === 'invoke').length, 1);
});

test('queued mutation timeout/cancellation remain uncertain and never repeat the mutation', async () => {
  const { client, requests } = await bound(() => ({ kind: 'receipt', receipt: { request_id: 'queued-1', state: 'queued' } }));
  await assert.rejects(client.invoke({ id: 'apply' }, { expected: target, requestId: 'queued-1', timeoutMs: 120 }), { code: 'uncertain_completion', request_id: 'queued-1' });
  assert.equal(requests.filter(request => request.operation.op === 'invoke').length, 1);
  assert.equal(requests.filter(request => request.operation.op === 'receipt').length, 3);
});

test('authoritative validation rejection is definitive and no retry is issued', async () => {
  const { client, requests } = await bound(() => ({ error: { code: 'validation_failed', message: 'Draft invalid' } }));
  await assert.rejects(client.invoke({ id: 'apply' }, { expected: target }), { code: 'validation_failed' });
  assert.equal(requests.filter(request => request.operation.op === 'invoke').length, 1);
});

test('unsupported arguments reach authoritative validation without a client setter path', async () => {
  const { client, requests } = await bound(() => ({ error: { code: 'invalid_arguments', message: 'Only no-argument actions' } }));
  await assert.rejects(client.invoke({ id: 'apply' }, { expected: target, arguments: { bad: 1 } }), { code: 'invalid_arguments' });
  assert.deepEqual(requests.at(-1).operation.arguments, { bad: 1 });
});

test('keyboard/text input requires the selected focused target and retains its observation', async () => {
  const { client } = await bound(() => ({ kind: 'observe', observation: observation([node({ focused: true, role: 'text_input' })]) }));
  const physical = adapter();
  const evidence = await client.text('Revised', physical, { selector: { id: 'apply', pane: 7 } });
  assert.equal(evidence.node, 'apply'); assert.equal(evidence.observation, 1);
  assert.equal(evidence.focus[0].role, 'text_input'); assert.equal(evidence.outcome, 'unverified');
  await assert.rejects(client.key('Enter', physical, { selector: { id: 'absent' } }), { code: 'unavailable' });
  assert.equal(physical.calls.length, 1);
});

test('keyboard transport timeout/cancellation cannot claim an input result', async () => {
  const { client } = await bound();
  const physical = adapter(); physical.key = () => new Promise(() => {});
  await assert.rejects(client.key('Enter', physical, { timeoutMs: 15 }), { code: 'uncertain_physical_input', cause_code: 'timeout' });
  const controller = new AbortController();
  const input = client.key('Enter', physical, { signal: controller.signal });
  setTimeout(() => controller.abort(), 1);
  await assert.rejects(input, { code: 'uncertain_physical_input', cause_code: 'cancelled' });
});

test('bounds and invalid/null selector values fail before any transport dispatch', async () => {
  const { client, requests } = await bound();
  for (const selected of [{ id: null }, { pane: -1 }, { domain: null }]) {
    await assert.rejects(client.query(selected), { code: 'invalid_request' });
  }
  await assert.rejects(client.query({ name: 'x'.repeat(256 * 1024) }), { code: 'size_limit' });
  assert.equal(requests.length, 1);
});

test('capture preserves bracketed semantic/text observations and honest failure diagnostics', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'polyorama-evidence-'));
  try {
    const { client } = await bound();
    const physical = adapter();
    physical.capture = async () => { throw new Error('ImageMagick unavailable'); };
    const path = join(directory, 'host.png');
    const metadataPath = join(directory, 'host.json');
    const evidence = await client.capture(physical, { path, metadataPath, inputRoute: 'physical_pointer' });
    assert.equal(evidence.status, 'partial'); assert.equal(evidence.capture.status, 'failed');
    assert.equal(evidence.capture.exact_observation_frame, false);
    assert.equal(evidence.before.observation.id, 1); assert.equal(evidence.after.observation.id, 1);
    assert.equal(evidence.application.build.source_revision, 'abc');
    assert.deepEqual(JSON.parse(await readFile(metadataPath, 'utf8')), evidence);
    assert.equal(evidence.diagnostics[0].stage, 'capture');
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('capture keeps a successful screenshot when a bracketing observation fails', async () => {
  let reads = 0;
  const { client } = await bound(() => ++reads === 1 ? { error: { code: 'missing_observation', message: 'Not ready' } }
    : { kind: 'observe', observation: observation() });
  const directory = await mkdtemp(join(tmpdir(), 'polyorama-evidence-'));
  try {
    const evidence = await client.capture(adapter(), { path: join(directory, 'partial.png') });
    assert.equal(evidence.status, 'partial'); assert.equal(evidence.capture.status, 'captured');
    assert.equal(evidence.before.error.code, 'missing_observation'); assert.equal(evidence.after.observation.id, 1);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('native adapter uses executable argument arrays, only the configured window and literal text', async () => {
  const commands = [];
  const execute = async (program, args) => { commands.push({ program, args }); return { stdout: 'X=4\nY=5\nWIDTH=400\nHEIGHT=300\n' }; };
  const physical = new NativePhysicalAdapter(42, { execute });
  assert.equal((await physical.geometry()).x, 4);
  await physical.text('$(touch /tmp/unsafe); `example`');
  await physical.key('Control+ArrowLeft');
  await physical.capture('/tmp/capture.png');
  assert.deepEqual(commands.find(command => command.args[0] === 'type').args, ['type', '--clearmodifiers', '--', '$(touch /tmp/unsafe); `example`']);
  assert.deepEqual(commands.find(command => command.args[0] === 'key').args, ['key', '--clearmodifiers', 'ctrl+Left']);
  assert.deepEqual(commands.at(-1), { program: 'import', args: ['-window', '42', 'png:/tmp/capture.png'] });
  assert.throws(() => new NativePhysicalAdapter('42; shell'), { code: 'invalid_request' });
});

test('browser hook and canvas access are encapsulated and CSS coordinates respect device scale', async () => {
  const calls = [];
  const page = { evaluate: async (fn, input) => {
    calls.push(input); return input ? JSON.stringify({ received: input }) : 2;
  }, locator: () => ({ count: async () => 1, boundingBox: async () => ({ x: 3, y: 5, width: 400, height: 300 }) }) };
  const transport = new BrowserTransport(page);
  assert.deepEqual(JSON.parse(await transport.request('{"operation":{}}')), { received: '{"operation":{}}' });
  const physical = new BrowserPhysicalAdapter(page);
  const geometry = await physical.geometry();
  assert.deepEqual(physical.point({ root: { rect: rootRect }, node: node(), scale: 2 }, geometry), { x: 53, y: 45 });
  page.locator = () => ({ count: async () => 2 });
  await assert.rejects(physical.geometry(), { code: 'ambiguous' });
});

test('browser startup readiness belongs to the shared hello transport', async () => {
  const calls = [];
  const transport = new BrowserTransport({
    waitForFunction: async () => calls.push('ready'),
    evaluate: async (_fn, request) => { calls.push('request'); return request; },
  });
  const request = '{"operation":{"op":"hello"}}';
  assert.equal(await transport.request(request), request);
  assert.deepEqual(calls, ['ready', 'request']);
});

test('native pointer placement supports an unchanged position and dispatches one click', async () => {
  const calls = [];
  const physical = new NativePhysicalAdapter(17, { execute: async (_program, args) => {
    calls.push(args);
    return { stdout: args[0] === 'getmouselocation' ? 'X=73\nY=23\nSCREEN=0\nWINDOW=17\n' : '' };
  } });
  await physical.click({ x: 73.1, y: 23.2 });
  assert.deepEqual(calls.find(args => args[0] === 'mousemove'), ['mousemove', '73', '23']);
  assert.equal(calls.filter(args => args[0] === 'click').length, 1);
  const misplaced = new NativePhysicalAdapter(17, { execute: async (_program, args) => {
    assert.notEqual(args[0], 'click');
    return { stdout: 'X=0\nY=0\n' };
  } });
  await assert.rejects(misplaced.click({ x: 73, y: 23 }), { code: 'invalid_geometry' });
});

test('socket transport handles fragmented replies and connection loss with no retry', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'polyorama-socket-'));
  const path = join(directory, 'application.sock');
  let connections = 0;
  const server = createServer(stream => {
    connections += 1;
    stream.once('data', () => {
      if (connections === 1) { stream.write('{"value":'); stream.end('1}\n'); }
      else stream.end();
    });
  });
  try {
    await new Promise(resolve => server.listen(path, resolve));
    const transport = new SocketTransport(path);
    assert.deepEqual(JSON.parse(await transport.request('{}')), { value: 1 });
    await assert.rejects(transport.request('{}'), { code: 'disconnected' });
    assert.equal(connections, 2);
  } finally {
    await new Promise(resolve => server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
});
