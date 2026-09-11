// Authored Worker control-flow tests: no browser, WASM, network, scene or GPU launch.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('../worker.js', import.meta.url), 'utf8')
  .replace(/^import .*;\n/gm, '');
const responseSource = readFileSync(new URL('../response.js', import.meta.url), 'utf8')
  .replace('export function', 'function');
const job = sequence => ({ manifest: {target: 'authored', tid: 'authored-exact-tid'},
  request: {token: {generation: 4, epoch: 2, sequence}, key: {reduction: 0}, max_decoded_bytes: 1024} });
const clone = value => JSON.parse(JSON.stringify(value));
async function harness(options = {}) {
  const events = [], transfers = [];
  const state = {scopes: 0, releases: 0, decodes: 0, queries: 0, responses: 0, received: 0, readers: 0, masks: 0};
  let context;
  class WorkerClient {
    register() {}
    missing() { return []; }
    begin_request() { assert.equal(state.scopes, 0); state.scopes++; }
    end_request() { if (state.scopes) {state.releases++; state.scopes = 0;} state.readers = 0; }
    missing_masks() { return options.mask && !state.masks ? [0] : []; }
    mask() { if (options.maskError) throw new Error('mask digest mismatch'); state.masks++; }
    ready() { return Boolean(options.immediate); }
    decode() {
      state.decodes++;
      if (options.decodeError) throw new Error('authored decode error');
      return {samples: new Uint16Array([17]), validity: new Uint8Array([1])};
    }
    query() { state.queries++; return 'authored-query'; }
    begin(tid, headers) {
      assert.equal(tid, 'authored-exact-tid');
      assert.equal(new Headers(headers).get('JPIP-tid'), tid);
      state.responses++; state.readers++;
    }
    receive(bytes) { assert.equal(state.readers, 1); state.received += bytes.length; }
    finish() { assert.equal(state.readers, 1); state.readers = 0; }
    abandon_response() { state.readers = 0; }
    metrics() { return {decode_count: state.decodes, request_working_set_bytes: state.scopes * 128, request_pin_metadata_bytes: state.scopes * 64}; }
  }
  const fetch = async (url, args) => {
    if (url.endsWith('/catalogue')) return new Response('[]');
    if (options.onFetch) await options.onFetch({url, args, send: data => context.self.onmessage({data})});
    return new Response(new Uint8Array([0, 2, 0]), {headers: {'JPIP-tid': 'authored-exact-tid', 'JPIP-fsiz': '1,1', 'JPIP-roff': '0,0', 'JPIP-rsiz': '1,1'}});
  };
  context = vm.createContext({WorkerClient, init: async () => ({memory: {buffer: new ArrayBuffer(64)}}),
    fetch, Headers, TextDecoder, Uint8Array, AbortController, self: {},
    performance: {timeOrigin: 0, now: () => 0},
    setTimeout: callback => Promise.resolve().then(async () => {
      if (options.onYield) await options.onYield(data => context.self.onmessage({data}));
      callback();
    }),
    postMessage: (value, buffers) => {events.push(value); transfers.push(buffers);},
  });
  vm.runInContext(responseSource + '\n' + source, context);
  const send = data => context.self.onmessage({data});
  await send({kind: 'init', server: 'http://authored.invalid', compressed: 1024});
  events.length = 0; transfers.length = 0;
  return {send, events, transfers, state};
}
function terminal(h, kind, request) {
  assert.equal(h.events.length, 1);
  assert.deepEqual(clone(h.events[0][kind].request), request);
  assert.equal(h.state.scopes, 0);
  assert.equal(h.state.readers, 0);
  assert.equal(h.state.releases, 1);
  assert.equal(h.events[0][kind].metrics.request_pin_metadata_bytes, 0);
}
test('non-ready immediate transport exhausts exactly 64 rounds with one matching Failed and no decode or arrays', async () => {
  const h = await harness(); const j = job(1);
  await h.send({kind: 'job', job: j});
  terminal(h, 'Failed', j.request);
  assert.match(h.events[0].Failed.error, /^Error: regional continuation exhausted after 64 rounds$/);
  assert.equal(h.state.queries, 64); assert.equal(h.state.responses, 64);
  assert.equal(h.events[0].Failed.metrics.retries, 63);
  assert.equal(h.state.decodes, 0);
  assert.equal(h.events[0].Failed.metrics.transferred_sample_bytes, 0);
  assert.equal(h.transfers.flat().length, 0);
  assert.equal(h.events[0].Failed.pixels, undefined);
});
test('ready cache hit completes once and releases before transferring both arrays', async () => {
  const h = await harness({immediate: true}); const j = job(2);
  await h.send({kind: 'job', job: j}); terminal(h, 'Completed', j.request);
  assert.equal(h.state.queries, 0); assert.equal(h.state.decodes, 1);
  assert.equal(h.transfers[0].length, 2);
  assert.equal(h.events[0].Completed.metrics.transferred_sample_bytes, 3);
});
test('mask error releases once with no decode or transfers', async () => {
  const h = await harness({mask: true, maskError: true}); const j = job(3);
  await h.send({kind: 'job', job: j}); terminal(h, 'Failed', j.request);
  assert.match(h.events[0].Failed.error, /mask digest mismatch/);
  assert.equal(h.state.queries + h.state.decodes, 0);
  assert.equal(h.transfers.flat().length, 0);
});
test('matching cancellation during transport releases once and publishes no arrays', async () => {
  const j = job(4);
  const h = await harness({onFetch: async ({args, send}) => {
    await send({kind: 'cancel', request: j.request}); assert.equal(args.signal.aborted, true);
  }});
  await h.send({kind: 'job', job: j}); terminal(h, 'Cancelled', j.request);
  assert.equal(h.state.decodes, 0); assert.equal(h.transfers.flat().length, 0);
});
test('cancellation after synchronous decode discards output before transfer', async () => {
  const j = job(5);
  const h = await harness({immediate: true, onYield: send => send({kind: 'cancel', request: j.request})});
  await h.send({kind: 'job', job: j}); terminal(h, 'Cancelled', j.request);
  assert.equal(h.state.decodes, 1); assert.equal(h.transfers.flat().length, 0);
  assert.equal(h.events[0].Cancelled.metrics.transferred_sample_bytes, 0);
});
test('stale generation cancellation cannot stop the current request or retain tombstones', async () => {
  const j = job(6); const old = clone(j.request); old.token.generation--;
  const h = await harness({immediate: true, onYield: send => send({kind: 'cancel', request: old})});
  await h.send({kind: 'job', job: j}); terminal(h, 'Completed', j.request);
  await h.send({kind: 'cancel', request: j.request});
  h.events.length = 0; h.state.releases = 0;
  const next = job(7); await h.send({kind: 'job', job: next}); terminal(h, 'Completed', next.request);
});
test('decode error releases the working set before Failed', async () => {
  const h = await harness({immediate: true, decodeError: true}); const j = job(8);
  await h.send({kind: 'job', job: j}); terminal(h, 'Failed', j.request);
  assert.match(h.events[0].Failed.error, /authored decode error/);
  assert.equal(h.transfers.flat().length, 0);
});
