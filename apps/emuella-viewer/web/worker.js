import { beginResponse } from './response.js';
import init, { WorkerClient } from './pkg/emuella_viewer.js';
let wasm;
const ready = init().then(value => { wasm = value; });
let client, server, active, pending, running = false;
const transport = { aborted: 0, retries: 0, cache_hits: 0, elapsed_ms: 0, transferred_sample_bytes: 0 };
const cancelled = new Set();
const pacingNow = () => performance.timeOrigin + performance.now();
const key = request => JSON.stringify(request.token);
const limit = 8 * 1024 * 1024;
async function bounded(response, maximum, consume) {
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const reader = response.body.getReader();
  const chunks = []; let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > maximum) throw new Error('HTTP response exceeds byte bound');
      if (consume) consume(value); else chunks.push(value);
    }
  } finally { await reader.cancel(); }
  if (consume) return;
  const result = new Uint8Array(size); let offset = 0;
  for (const c of chunks) { result.set(c, offset); offset += c.length; }
  return result;
}
function metrics() { return {...(client?.metrics() ?? {}), ...transport, wasm_linear_bytes: wasm.memory.buffer.byteLength}; }
function emit(value) {
  for (const kind of ['Completed', 'Cancelled', 'Failed']) {
    const timing = value[kind]?.metrics.timing;
    if (timing) timing.published_ms = pacingNow();
  }
  const buffer=value.Completed?.pixels.samples.buffer;
  const validity = value.Completed?.pixels.validity.buffer;
  postMessage(value, buffer ? [buffer, validity] : []);
}
async function work(job) {
  const controller = new AbortController(); active = { job, controller };
  const stopped = () => { if (cancelled.has(key(job.request))) throw new Error('cancelled'); };
  const get = path => fetch(server + path, { signal: controller.signal });
  const started = performance.now(), started_ms = pacingNow();
  let finished_ms;
  const timedMetrics = () => ({...metrics(), timing: {started_ms, finished_ms: finished_ms ?? pacingNow(), published_ms: 0, received_ms: null}});
  try {
    stopped(); client.register(job.manifest);
    // Admission can discard an earlier descriptor; recover the bounded working set.
    for (let round=0;round<3;round++) {
    for (const tile of client.missing(job)) {
      stopped();
      const bytes = await bounded(await get(`/descriptor/${job.manifest.target}/${tile}?tid=${job.manifest.tid}`), limit);
      stopped(); client.descriptor(job.manifest.tid, tile, bytes);
    }
      if (!client.missing(job).length) break;
    }
    if (client.missing(job).length) throw new Error('regional descriptors exceed admitted metadata budget');
    stopped(); client.begin_request(job);
    for (const tile of client.missing_masks(job)) {
      stopped();
      const discard = job.request.key.reduction;
      const bytes = await bounded(await get(`/mask/${job.manifest.target}/${discard}/${tile}?tid=${job.manifest.tid}`), limit);
      stopped(); client.mask(job.manifest.tid, tile, discard, bytes);
    }
    if (client.missing_masks(job).length) throw new Error('regional masks exceed admitted budget');
    let pixels;
    if (client.ready(job)) { pixels = client.decode(job); transport.cache_hits++; }
    for (let attempt = 0; !pixels && attempt < 64; attempt++) {
      stopped();
      if (attempt > 0) transport.retries++;
      const response = await get(`/jpip?${client.query(job)}`);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      stopped();
      beginResponse(client, job.manifest.tid, response.headers);
      await bounded(response, limit, bytes => { stopped(); client.receive(bytes); });
      client.finish(); stopped();
      if (client.ready(job)) pixels = client.decode(job);
    }
    if (!pixels) throw new Error('regional continuation exhausted after 64 rounds');
    finished_ms = pacingNow();
    // Yield after synchronous WASM decoding so queued cancellation is seen before publishing.
    await new Promise(resolve => setTimeout(resolve, 0)); stopped();
    transport.elapsed_ms += performance.now() - started;
    transport.transferred_sample_bytes += pixels.samples.byteLength + pixels.validity.byteLength;
    client.end_request();
    const m = timedMetrics();
    emit({ Completed: { request: job.request, pixels, metrics: m } });
  } catch (error) {
    client.end_request();
    if (cancelled.has(key(job.request))) { transport.aborted++; emit({ Cancelled: { request: job.request, metrics: timedMetrics() } }); }
    else emit({ Failed: { request: job.request, error: String(error), metrics: timedMetrics() } });
  } finally { client.end_request(); client.abandon_response(); cancelled.delete(key(job.request)); active = undefined; }
}
self.onmessage = async ({ data }) => {
  await ready;
  if (data.kind === 'init') {
    server = data.server.replace(/\/$/, ''); client = new WorkerClient(data.compressed);
    try {
      const bytes = await bounded(await fetch(server + '/catalogue'), 16 << 20);
      const catalogue = JSON.parse(new TextDecoder().decode(bytes));
      if (catalogue.length > 32) throw new Error('catalogue capacity');
      for (const manifest of catalogue) client.register(manifest);
      emit({ Catalogue: catalogue });
    } catch (error) { emit({ Failed: { request: null, error: String(error), metrics: metrics() } }); }
  } else if (data.kind === 'cancel') {
    // A completion already posted to the UI will release its reservation there.
    // Do not retain a cancellation tombstone for a job this worker has finished.
    if (active && key(active.job.request) === key(data.request)) {
      cancelled.add(key(data.request)); active.controller.abort();
    }
  } else if (data.kind === 'job') {
    if (pending || running) { emit({ Failed: { request: data.job.request, error: 'worker queue capacity', metrics: metrics() } }); return; }
    pending = data.job; running = true;
    const job = pending; pending = undefined; await work(job); running = false;
  }
};
