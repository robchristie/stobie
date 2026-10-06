import { randomUUID } from 'node:crypto';
import { connect } from 'node:net';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { performance } from 'node:perf_hooks';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

const runFile = promisify(execFile);
const REQUEST_BYTES = 256 * 1024;
const RESPONSE_BYTES = 2 * 1024 * 1024;
const SELECTOR_KEYS = new Set(['id', 'role', 'name', 'capability', 'pane', 'domain']);
const clock = { now: () => performance.now(), sleep: sleep };

export class ApplicationError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = 'ApplicationError';
    this.code = code;
    Object.assign(this, details);
  }
}

/** Preserve bounded client diagnostics without serialising internal causes. */
export function describeApplicationError(error) {
  const description = { code: error.code ?? 'cli_error', message: error.message };
  for (const key of ['request_id', 'cause_code', 'observation', 'last_error', 'last_capability', 'capability_observation', 'target', 'last_reason', 'ids', 'node', 'point']) {
    if (error[key] !== undefined) description[key] = error[key];
  }
  return description;
}

function failure(code, message, details) { return new ApplicationError(code, message, details); }
function aborted(signal) {
  if (signal?.aborted) throw failure('cancelled', 'The local operation was cancelled');
}
function timeout(value) {
  if (!Number.isFinite(value) || value <= 0 || value > 60000) {
    throw failure('invalid_request', 'timeoutMs must be positive and at most 60000');
  }
  return value;
}
function interval(value) {
  if (!Number.isFinite(value) || value < 1 || value > 1000) throw failure('invalid_request', 'intervalMs must be 1..1000');
  return value;
}
function sleep(ms, signal) {
  return new Promise((fulfil, reject) => {
    aborted(signal);
    const timer = setTimeout(done, ms);
    function done() { signal?.removeEventListener('abort', cancel); fulfil(); }
    function cancel() {
      clearTimeout(timer);
      signal?.removeEventListener('abort', cancel);
      reject(failure('cancelled', 'The local wait was cancelled'));
    }
    signal?.addEventListener('abort', cancel, { once: true });
  });
}
async function hostCall(work, milliseconds, signal) {
  aborted(signal);
  if (milliseconds <= 0) throw failure('timeout', 'The host operation reached its monotonic deadline');
  const cancellation = new AbortController();
  let timer;
  let cancel;
  try {
    const deadline = new Promise((_, reject) => {
      timer = setTimeout(() => { reject(failure('timeout', 'The host operation reached its deadline')); cancellation.abort(); }, milliseconds);
      cancel = () => { reject(failure('cancelled', 'The local host operation was cancelled')); cancellation.abort(); };
      signal?.addEventListener('abort', cancel, { once: true });
    });
    aborted(signal);
    return await Promise.race([work(cancellation.signal), deadline]);
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener('abort', cancel);
  }
}
function selector(value = {}) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw failure('invalid_request', 'A selector must be an object');
  }
  for (const key of Object.keys(value)) {
    if (!SELECTOR_KEYS.has(key)) throw failure('invalid_request', `Unknown selector field ${key}`);
    if (['id', 'role', 'name', 'capability'].includes(key) && typeof value[key] !== 'string') {
      throw failure('invalid_request', `Selector ${key} must be an exact string`);
    }
    if (key === 'pane' && (!Number.isSafeInteger(value[key]) || value[key] < 0)) throw failure('invalid_request', 'Selector pane must be a non-negative integer');
    if (key === 'domain' && (!value[key] || typeof value[key] !== 'object' || Array.isArray(value[key]))) throw failure('invalid_request', 'Selector domain must be a tagged domain object');
  }
  return value;
}
function same(a, b) {
  if (a === b) return true;
  if (a == null || b == null || typeof a !== 'object' || typeof b !== 'object') return false;
  const keys = Object.keys(a).sort();
  return sameKeys(keys, Object.keys(b).sort()) && keys.every(key => same(a[key], b[key]));
}
function sameKeys(a, b) { return a.length === b.length && a.every((value, index) => value === b[index]); }
export function matchesSelector(node, selected) {
  selector(selected);
  return Object.entries(selected).every(([key, value]) => {
    if (key === 'capability') return node.actions.includes(value);
    if (key === 'domain') return same(node.domain_reference, value);
    return same(node[key], value);
  });
}
function nodesFor(observation, selected) {
  const nodes = observation?.snapshot?.nodes;
  if (!Array.isArray(nodes) || nodes.length > 4096) throw failure('invalid_response', 'Invalid bounded observation nodes');
  return nodes.filter(node => matchesSelector(node, selected));
}
function oneNode(observation, selected) {
  const nodes = nodesFor(observation, selected);
  if (nodes.length > 1) throw failure('ambiguous', 'The selector matches several nodes; add an exact identity, pane or domain', { ids: nodes.map(node => node.id) });
  return nodes[0];
}
function rect(value, label) {
  if (!value || !['min_x', 'min_y', 'max_x', 'max_y'].every(key => Number.isFinite(value[key]))
    || value.max_x <= value.min_x || value.max_y <= value.min_y) {
    throw failure('invalid_geometry', `Invalid ${label} geometry`);
  }
  return value;
}
function physicalTarget(observation, node) {
  const snapshot = observation.snapshot;
  const roots = snapshot.nodes.filter(candidate => candidate.id === snapshot.root);
  if (roots.length !== 1) throw failure('invalid_geometry', 'The observation requires exactly one root');
  const root = roots[0];
  rect(root.rect, 'root');
  rect(node.rect, 'target');
  if (!(Number.isFinite(snapshot.pixels_per_point) && snapshot.pixels_per_point > 0)) {
    throw failure('invalid_geometry', 'Invalid viewport scale');
  }
  if (node.rect.min_x < root.rect.min_x || node.rect.min_y < root.rect.min_y
    || node.rect.max_x > root.rect.max_x || node.rect.max_y > root.rect.max_y) {
    throw failure('clipped_target', `Target ${node.id} is outside the completed root rectangle`);
  }
  return { observation, node, root, scale: snapshot.pixels_per_point };
}
function signature(target, geometry) {
  const { node, root, scale } = target;
  return JSON.stringify([node.id, node.role, node.name, node.pane, node.domain_reference,
    node.actions, node.enabled, node.rect, root.rect, scale, geometry]);
}
function boundedJson(value, bound, label) {
  const json = typeof value === 'string' ? value : JSON.stringify(value);
  if (Buffer.byteLength(json) > bound) throw failure('size_limit', `${label} exceeds ${bound} bytes`);
  return json;
}

/** One request per local Unix stream. No connection or mutation is retried. */
export class SocketTransport {
  constructor(path = process.env.POLYORAMA_AUTOMATION_SOCKET) {
    if (process.platform === 'win32') throw failure('unsupported_host', 'The local socket adapter requires Unix');
    if (typeof path !== 'string' || !path || path.includes('\0')) {
      throw failure('invalid_request', 'Supply the explicit POLYORAMA_AUTOMATION_SOCKET path');
    }
    this.path = path;
    this.kind = 'native_unix';
  }
  request(json, { signal } = {}) {
    boundedJson(json, REQUEST_BYTES, 'Request');
    return new Promise((fulfil, reject) => {
      aborted(signal);
      const stream = connect(this.path);
      const chunks = [];
      let length = 0;
      let done = false;
      const finish = (error, reply) => {
        if (done) return;
        done = true;
        signal?.removeEventListener('abort', cancel);
        stream.destroy();
        if (error) reject(error); else fulfil(reply);
      };
      const cancel = () => finish(failure('cancelled', 'The local socket request was cancelled'));
      signal?.addEventListener('abort', cancel, { once: true });
      stream.once('connect', () => stream.write(`${json}\n`));
      stream.on('data', chunk => {
        length += chunk.length;
        if (length > RESPONSE_BYTES) return finish(failure('size_limit', 'Response exceeds 2 MiB'));
        chunks.push(chunk);
        const body = Buffer.concat(chunks);
        const end = body.indexOf(10);
        if (end >= 0) finish(null, body.subarray(0, end).toString('utf8'));
      });
      stream.once('error', error => finish(failure('disconnected', error.message)));
      stream.once('end', () => finish(failure('disconnected', 'Socket closed before a complete response')));
    });
  }
}

/** The fixed hook is the only browser evaluation performed by this transport. */
export class BrowserTransport {
  constructor(page) { this.page = page; this.kind = 'browser'; }
  async request(json, { signal } = {}) {
    aborted(signal);
    boundedJson(json, REQUEST_BYTES, 'Request');
    let result;
    try {
      if (JSON.parse(json).operation?.op === 'hello') {
        await this.page.waitForFunction(() => typeof window.__POLYORAMA_AUTOMATION?.request === 'function', null, { timeout: 60000 });
        aborted(signal);
      }
      result = await this.page.evaluate(async request => {
      if (typeof window.__POLYORAMA_AUTOMATION?.request !== 'function') {
        throw new Error('Application automation is unavailable; enable ?automation=1 before startup');
      }
      return await window.__POLYORAMA_AUTOMATION.request(request);
    }, json); }
    catch (error) { throw failure('disconnected', error.message); }
    return boundedJson(result, RESPONSE_BYTES, 'Response');
  }
}

/** Reads completed observations. Polling never requests application repaint. */
export class ApplicationClient {
  constructor(transport, { timeoutMs = 15000, scheduler = clock } = {}) {
    this.transport = transport;
    this.timeoutMs = timeout(timeoutMs);
    this.scheduler = scheduler;
    this.instance = null;
    this.application = null;
  }
  async request(operation, { timeoutMs = this.timeoutMs, signal, requestId = randomUUID(), mutation = false } = {}) {
    aborted(signal);
    const milliseconds = timeout(timeoutMs);
    if (operation.op !== 'hello' && !this.instance) throw failure('unbound', 'Call hello before using the application');
    const request = { version: 1, request_id: requestId, operation };
    if (this.instance) request.instance = this.instance;
    const json = boundedJson(request, REQUEST_BYTES, 'Request');
    const cancellation = new AbortController();
    let dispatchStarted = false;
    let authoritativeError = false;
    let timer;
    let onAbort;
    try {
      const deadline = new Promise((_, reject) => {
        timer = setTimeout(() => { reject(failure('timeout', `Request exceeded ${milliseconds} ms`)); cancellation.abort(); }, milliseconds);
        onAbort = () => { reject(failure('cancelled', 'The local request was cancelled')); cancellation.abort(); };
        signal?.addEventListener('abort', onAbort, { once: true });
      });
      aborted(signal);
      dispatchStarted = true;
      const raw = await Promise.race([this.transport.request(json, { signal: cancellation.signal }), deadline]);
      const response = JSON.parse(boundedJson(raw, RESPONSE_BYTES, 'Response'));
      if (response.version !== 1 || response.request_id !== requestId || typeof response.instance !== 'string'
        || Boolean(response.result) === Boolean(response.error)) {
        throw failure('invalid_response', 'Response version, request identity or result/error envelope is invalid');
      }
      if (this.instance && response.instance !== this.instance) throw failure('wrong_instance', 'The application restarted; create and bind a new client');
      if (response.error) { authoritativeError = true; throw failure(response.error.code, response.error.message, { request_id: requestId }); }
      const expectedKind = ['invoke', 'cancel', 'receipt'].includes(operation.op) ? 'receipt' : operation.op;
      if (response.result.kind !== expectedKind) throw failure('invalid_response', `Expected ${expectedKind} result`);
      if (operation.op === 'hello') {
        this.instance = response.instance;
        this.application = response.result.application;
        this.limits = response.result.limits;
      }
      return response.result;
    } catch (error) {
      if (mutation && dispatchStarted && !authoritativeError && error.code !== 'wrong_instance') {
        throw failure('uncertain_completion', 'The mutation may have reached the application. Look up this request ID; do not repeat it.', {
          request_id: requestId, cause_code: error.code ?? 'transport_error', cause: error,
        });
      }
      if (error instanceof ApplicationError) throw error;
      throw failure('invalid_response', error.message, { cause: error });
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener('abort', onAbort);
    }
  }
  async hello(options) { return this.request({ op: 'hello' }, options); }
  async observe(options) { return (await this.request({ op: 'observe' }, options)).observation; }
  async query(selected = {}, { limit = 64, cursor, ...options } = {}) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 256) throw failure('invalid_request', 'Query limit must be 1..256');
    return this.request({ op: 'query', selector: selector(selected), limit, ...(cursor ? { cursor } : {}) }, options);
  }
  async discover(selected = {}, { limit = 64, cursor, ...options } = {}) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 256) throw failure('invalid_request', 'Discovery limit must be 1..256');
    return this.request({ op: 'discover', selector: selector(selected), limit, ...(cursor ? { cursor } : {}) }, options);
  }
  async receipt(requestId, options) { return (await this.request({ op: 'receipt', request_id: requestId }, options)).receipt; }
  async cancel(requestId, options = {}) {
    return (await this.request({ op: 'cancel', request_id: requestId }, { ...options, mutation: true })).receipt;
  }
  async invoke(selected, { expected, arguments: suppliedArguments, wait = true, timeoutMs = this.timeoutMs, signal, requestId = randomUUID() } = {}) {
    const started = this.scheduler.now();
    const budget = timeout(timeoutMs);
    const remaining = () => budget - (this.scheduler.now() - started);
    const readOptions = () => {
      const milliseconds = remaining();
      if (milliseconds <= 0) throw failure('timeout', 'Invocation deadline expired before dispatch');
      return { timeoutMs: milliseconds, signal };
    };
    if (!expected) {
      const discovered = await this.discover(selected, { limit: 2, ...readOptions() });
      if (discovered.total !== 1 || discovered.capabilities.length !== 1) {
        throw failure(discovered.total ? 'ambiguous' : 'no_match', 'Semantic invocation requires one exact capability target');
      }
      expected = discovered.capabilities[0].target;
    }
    const result = await this.request({ op: 'invoke', selector: selector(selected), expected,
      ...(suppliedArguments !== undefined ? { arguments: suppliedArguments } : {}) }, {
      ...readOptions(), requestId, mutation: true,
    });
    if (!wait || result.receipt.state !== 'queued') return result.receipt;
    if (remaining() <= 0) throw failure('uncertain_completion', 'Receipt completion is unconfirmed at the invocation deadline', { request_id: requestId });
    return this.waitReceipt(requestId, { timeoutMs: remaining(), signal });
  }
  async waitReceipt(requestId, { timeoutMs = this.timeoutMs, signal, intervalMs = 50 } = {}) {
    const deadline = this.scheduler.now() + timeout(timeoutMs);
    interval(intervalMs);
    try {
      while (true) {
        aborted(signal);
        const remaining = deadline - this.scheduler.now();
        if (remaining <= 0) throw failure('timeout', 'Receipt wait reached its deadline');
        const receipt = await this.receipt(requestId, { timeoutMs: remaining, signal });
        if (receipt.state !== 'queued') return receipt;
        await this.scheduler.sleep(Math.min(intervalMs, Math.max(0, deadline - this.scheduler.now())), signal);
      }
    } catch (error) {
      throw failure('uncertain_completion', 'Completion is unconfirmed. Look up the original request ID; do not repeat the mutation.', {
        request_id: requestId, cause_code: error.code, cause: error,
      });
    }
  }
  async wait(condition, { timeoutMs = this.timeoutMs, signal, intervalMs = 50 } = {}) {
    const deadline = this.scheduler.now() + timeout(timeoutMs);
    interval(intervalMs);
    const selected = selector(condition.selector ?? {});
    const kinds = ['present', 'absent', 'enabled', 'capability_enabled', 'focused', 'selected', 'checked', 'selection_changed', 'name', 'status', 'fact', 'audits_clear'];
    if (!kinds.includes(condition.condition)) throw failure('invalid_request', 'Unsupported observation wait condition');
    let baseline = condition.previous;
    let last;
    let lastError;
    let lastCapability;
    let lastCapabilityObservation;
    try {
    while (true) {
      aborted(signal);
      const remaining = deadline - this.scheduler.now();
      if (remaining <= 0) throw failure('timeout', 'Observation wait reached its monotonic deadline', { observation: last, last_error: lastError });
      try {
        last = await this.observe({ timeoutMs: remaining, signal });
        if (this.scheduler.now() >= deadline) throw failure('timeout', 'Observation arrived after the monotonic wait deadline', { observation: last });
        lastError = undefined;
        let capability;
        let capabilityObservation;
        const node = ['selection_changed', 'fact', 'audits_clear', 'capability_enabled'].includes(condition.condition) ? undefined
          : oneNode(last, condition.condition === 'status' ? { ...selected, role: 'status' } : selected);
        let done = false;
        if (condition.condition === 'present') done = Boolean(node);
        if (condition.condition === 'absent') done = !node;
        if (condition.condition === 'enabled') done = node?.enabled === true;
        if (condition.condition === 'capability_enabled') {
          const capabilities = await this.discover(selected, { limit: 2, timeoutMs: Math.max(1, deadline - this.scheduler.now()), signal });
          if (capabilities.total > 1) throw failure('ambiguous', 'Capability wait requires one target; add a pane or identity');
          capability = capabilities.capabilities[0];
          capabilityObservation = capabilities.observation;
          lastCapability = capability;
          lastCapabilityObservation = capabilityObservation;
          done = capability?.availability.state === 'enabled';
          if (this.scheduler.now() >= deadline) throw failure('timeout', 'Capability discovery exceeded the wait deadline');
        }
        if (condition.condition === 'focused') done = node?.focused === true;
        if (condition.condition === 'selected') done = node?.selected === true;
        if (condition.condition === 'checked') {
          if (typeof condition.value !== 'boolean') throw failure('invalid_request', 'A checked wait requires a boolean value');
          done = node?.checked === condition.value;
        }
        if (condition.condition === 'name' || condition.condition === 'status') done = node?.name === condition.value;
        if (condition.condition === 'fact') {
          if (typeof condition.key !== 'string' || !condition.key) throw failure('invalid_request', 'A fact wait requires a named published fact');
          done = Object.hasOwn(last.facts, condition.key) && same(last.facts[condition.key], condition.value);
        }
        if (condition.condition === 'audits_clear') done = last.snapshot.semantic_audit.length === 0
          && last.snapshot.text_audit.length === 0 && last.snapshot.text_audit_coverage != null;
        if (condition.condition === 'selection_changed') {
          const current = nodesFor(last, selected).filter(candidate => candidate.selected).map(candidate => candidate.id).sort();
          if (baseline === undefined) baseline = current;
          else done = !same(current, baseline);
        }
        if (done) return { observation: last, node: node ?? null,
          ...(capability ? { capability, capability_observation: capabilityObservation } : {}) };
      } catch (error) {
        if (error.code !== 'missing_observation') throw error;
        lastError = { code: error.code, message: error.message };
      }
      await this.scheduler.sleep(Math.min(intervalMs, Math.max(0, deadline - this.scheduler.now())), signal);
    }
    } catch (error) {
      error.observation = last ?? null;
      error.last_error = lastError ?? { code: error.code, message: error.message };
      if (lastCapability) {
        error.last_capability = lastCapability;
        error.capability_observation = lastCapabilityObservation;
      }
      throw error;
    }
  }
  async target(selected, { timeoutMs = 15000, signal, adapter } = {}) {
    const started = this.scheduler.now();
    const deadline = started + Math.min(15000, timeout(timeoutMs));
    let previous;
    let confirmations = 0;
    let firstSample;
    let last;
    let lastObservation;
    let lastReason;
    try {
    while (this.scheduler.now() < deadline) {
      aborted(signal);
      try {
        const observation = await this.observe({ timeoutMs: Math.max(1, deadline - this.scheduler.now()), signal });
        lastObservation = observation;
        const node = oneNode(observation, selector(selected));
        if (node?.enabled) {
          lastReason = 'Awaiting consistent enabled target and host geometry';
          const current = physicalTarget(observation, node);
          const geometry = adapter ? await hostCall(localSignal => adapter.geometry({ signal: localSignal }), deadline - this.scheduler.now(), signal) : undefined;
          if (adapter) adapter.point(current, geometry);
          const fingerprint = signature(current, geometry);
          if (fingerprint === previous) confirmations += 1;
          else { confirmations = 1; firstSample = this.scheduler.now(); }
          previous = fingerprint;
          last = { ...current, geometry, fingerprint, observed_ms: this.scheduler.now() - started };
          if (confirmations >= 3 && this.scheduler.now() - firstSample >= 100 && this.scheduler.now() < deadline) return last;
        } else {
          previous = undefined; confirmations = 0;
          lastReason = node ? node.disabled_reason ?? 'Target is disabled' : 'Target is absent';
        }
      } catch (error) {
        if (error.code !== 'missing_observation') throw error;
        previous = undefined; confirmations = 0;
      }
      await this.scheduler.sleep(Math.min(50, Math.max(0, deadline - this.scheduler.now())), signal);
    }
    throw failure('timeout', 'Target did not become enabled and stable within the bounded observation deadline', { target: last, last_reason: lastReason });
    } catch (error) {
      error.observation = lastObservation ?? null;
      error.last_reason = error.last_reason ?? (error.code === 'timeout' ? lastReason ?? error.message : error.message);
      throw error;
    }
  }
  async click(selected, adapter, options = {}) {
    if (!adapter) throw failure('invalid_request', 'Physical input requires an explicit host adapter');
    const requestId = randomUUID();
    const deadline = this.scheduler.now() + Math.min(15000, timeout(options.timeoutMs ?? 15000));
    const remaining = () => deadline - this.scheduler.now();
    const stable = await this.target(selected, { ...options, timeoutMs: remaining(), adapter });
    if (remaining() <= 0) throw failure('timeout', 'Physical input reached its deadline before dispatch');
    const observation = await this.observe({ ...options, timeoutMs: remaining() });
    const node = oneNode(observation, selected);
    if (!node?.enabled) throw failure('stale_target', 'The target disappeared or became disabled before physical input');
    const current = physicalTarget(observation, node);
    const geometry = await hostCall(signal => adapter.geometry({ signal }), remaining(), options.signal);
    if (signature(current, geometry) !== stable.fingerprint) throw failure('stale_target', 'Target or host geometry changed before physical input');
    const point = adapter.point(current, geometry);
    try { await hostCall(signal => adapter.click(point, { signal }), remaining(), options.signal); }
    catch (error) {
      throw failure('uncertain_physical_input', 'Physical input completion is unconfirmed; observe the application before repeating it', {
        request_id: requestId, cause_code: error.code, cause: error, point, node: node.id,
      });
    }
    return { request_id: requestId, route: 'physical', input: 'pointer', instance: this.instance, observation: observation.id,
      node: node.id, point, geometry, scale: current.scale, observed_ms: stable.observed_ms,
      completion: 'input_dispatched', outcome: 'unverified' };
  }
  async keyboardInput(input, value, adapter, options = {}) {
    const requestId = randomUUID();
    if (!adapter) throw failure('invalid_request', 'Physical input requires an explicit host adapter');
    const deadline = this.scheduler.now() + timeout(options.timeoutMs ?? this.timeoutMs);
    const observation = await this.observe(options);
    let node;
    if (options.selector) {
      node = oneNode(observation, selector(options.selector));
      if (!node?.enabled || !node.focused) throw failure('unavailable', 'The keyboard/text target must be uniquely present, enabled and focused');
    }
    aborted(options.signal);
    if (this.scheduler.now() >= deadline) throw failure('timeout', 'Keyboard/text deadline expired before input dispatch');
    try {
      await hostCall(signal => adapter[input](value, { signal }), deadline - this.scheduler.now(), options.signal);
    } catch (error) {
      throw failure('uncertain_physical_input', 'Physical input completion is unconfirmed; observe the application before repeating it', {
        request_id: requestId, cause_code: error.code, cause: error, observation: observation.id, node: node?.id ?? null,
      });
    }
    return { request_id: requestId, route: 'physical', input: input === 'key' ? 'keyboard' : 'text', instance: this.instance,
      observation: observation.id, node: node?.id ?? null,
      focus: observation.snapshot.nodes.filter(candidate => candidate.focused).map(candidate => ({ id: candidate.id,
        pane: candidate.pane, domain: candidate.domain_reference, role: candidate.role })),
      ...(input === 'key' ? { key: value } : {}), completion: 'input_dispatched', outcome: 'unverified' };
  }
  async key(key, adapter, options) {
    if (typeof key !== 'string' || !key || key.length > 256) throw failure('invalid_request', 'A bounded key chord is required');
    return this.keyboardInput('key', key, adapter, options);
  }
  async text(value, adapter, options) {
    if (typeof value !== 'string' || Buffer.byteLength(value) > 65536) throw failure('invalid_request', 'Physical text is limited to 64 KiB');
    return this.keyboardInput('text', value, adapter, options);
  }
  async capture(adapter, { path, metadataPath, inputRoute = 'observation_only', ...options } = {}) {
    if (!path) throw failure('invalid_request', 'Capture requires an output path');
    if (!adapter) throw failure('invalid_request', 'Capture requires an explicit host adapter');
    const evidence = { instance: this.instance, application: this.application, input_route: inputRoute,
      host: this.transport.kind, capture: { path: resolve(path), exact_observation_frame: false }, diagnostics: [] };
    const stamp = () => ({ monotonic_ms: this.scheduler.now(), wall_time: new Date().toISOString() });
    const observe = async label => {
      const started = stamp();
      try { evidence[label] = { started, observation: await this.observe(options), finished: stamp() }; }
      catch (error) { evidence[label] = { started, finished: stamp(), error: { code: error.code, message: error.message } }; evidence.diagnostics.push({ stage: label, ...evidence[label].error }); }
    };
    await observe('before');
    evidence.capture.started = stamp();
    try {
      evidence.capture.geometry = await hostCall(signal => adapter.geometry({ signal }), options.timeoutMs ?? this.timeoutMs, options.signal);
      await mkdir(dirname(evidence.capture.path), { recursive: true });
      await hostCall(signal => adapter.capture(evidence.capture.path, { signal }), options.timeoutMs ?? this.timeoutMs, options.signal);
      evidence.capture.status = 'captured';
    } catch (error) {
      evidence.capture.status = 'failed';
      evidence.diagnostics.push({ stage: 'capture', code: error.code ?? 'capture_failed', message: error.message });
    }
    evidence.capture.finished = stamp();
    await observe('after');
    const observation = evidence.before.observation ?? evidence.after.observation;
    if (observation) {
      const snapshot = observation.snapshot;
      evidence.viewport = { collection: observation.collection, pixels_per_point: snapshot.pixels_per_point,
        root: snapshot.nodes.find(node => node.id === snapshot.root)?.rect ?? null };
    }
    evidence.status = evidence.diagnostics.length ? 'partial' : 'complete';
    evidence.capture.relationship = { semantic_boundary: 'completed_ui_pass',
      before_observation: evidence.before.observation?.id ?? null,
      after_observation: evidence.after.observation?.id ?? null,
      claim: 'host capture bracketed by completed observations; GPU/OS frame identity is unavailable' };
    if (metadataPath) {
      try {
        await mkdir(dirname(resolve(metadataPath)), { recursive: true });
        await writeFile(resolve(metadataPath), `${JSON.stringify(evidence, null, 2)}\n`);
      } catch (error) {
        evidence.diagnostics.push({ stage: 'metadata', code: error.code ?? 'metadata_failed', message: error.message });
        evidence.status = 'partial';
      }
    }
    return evidence;
  }
}

function validateGeometry(target, geometry) {
  for (const key of ['x', 'y', 'width', 'height', 'device_scale']) {
    if (!Number.isFinite(geometry[key])) throw failure('invalid_geometry', `Invalid host ${key}`);
  }
  if (geometry.width <= 0 || geometry.height <= 0 || geometry.device_scale <= 0) throw failure('invalid_geometry', 'Invalid host viewport size or scale');
  const width = target.root.rect.max_x - target.root.rect.min_x;
  const height = target.root.rect.max_y - target.root.rect.min_y;
  if (Math.abs(geometry.width * geometry.device_scale - width * target.scale) > 2
    || Math.abs(geometry.height * geometry.device_scale - height * target.scale) > 2) {
    throw failure('invalid_geometry', 'Host viewport geometry does not match the completed root and scale');
  }
  return { x: geometry.x + ((target.node.rect.min_x + target.node.rect.max_x) / 2 - target.root.rect.min_x) * target.scale / geometry.device_scale,
    y: geometry.y + ((target.node.rect.min_y + target.node.rect.max_y) / 2 - target.root.rect.min_y) * target.scale / geometry.device_scale };
}

export class BrowserPhysicalAdapter {
  constructor(page, { canvas = 'canvas' } = {}) { this.page = page; this.canvas = canvas; }
  async locator() {
    const locator = this.page.locator(this.canvas);
    if (await locator.count() !== 1) throw failure('ambiguous', 'Physical capture/input requires exactly one host canvas');
    return locator;
  }
  async geometry() {
    const box = await (await this.locator()).boundingBox();
    if (!box) throw failure('invalid_geometry', 'Host canvas is not visible');
    return { ...box, device_scale: await this.page.evaluate(() => window.devicePixelRatio) };
  }
  point(target, geometry) { return validateGeometry(target, geometry); }
  async click(point) { await this.page.mouse.click(point.x, point.y); }
  async key(key) { await this.page.keyboard.press(key); }
  async text(value) { await this.page.keyboard.type(value); }
  async capture(path) { await (await this.locator()).screenshot({ path }); }
}

/** X11 only: the configured window must be the host's client viewport. */
export class NativePhysicalAdapter {
  constructor(windowId, { execute = runFile, xdotool = 'xdotool', imageMagick = 'import', timeoutMs = 2000 } = {}) {
    if (!/^[0-9]+$/.test(String(windowId))) throw failure('invalid_request', 'Supply an explicit numeric X11 window ID');
    this.windowId = String(windowId);
    this.execute = execute;
    this.xdotool = xdotool;
    this.imageMagick = imageMagick;
    this.timeoutMs = timeout(timeoutMs);
  }
  async run(program, args, { signal } = {}) { return this.execute(program, args, { timeout: this.timeoutMs, maxBuffer: RESPONSE_BYTES, signal }); }
  async geometry(options) {
    const { stdout } = await this.run(this.xdotool, ['getwindowgeometry', '--shell', this.windowId], options);
    const values = Object.fromEntries(stdout.trim().split('\n').map(line => line.split('=')));
    const geometry = { x: Number(values.X), y: Number(values.Y), width: Number(values.WIDTH), height: Number(values.HEIGHT), device_scale: 1 };
    if (['X', 'Y', 'WIDTH', 'HEIGHT'].some(key => !/^-?[0-9]+$/.test(values[key] ?? ''))) throw failure('invalid_geometry', 'Unrecognised xdotool window geometry');
    return geometry;
  }
  point(target, geometry) { return validateGeometry(target, geometry); }
  async click(point, options) {
    await this.run(this.xdotool, ['windowfocus', '--sync', this.windowId], options);
    const x = Math.round(point.x), y = Math.round(point.y);
    // Older xdotool --sync waits for a movement event even when the pointer is
    // already at its destination. Verify placement directly before one click.
    await this.run(this.xdotool, ['mousemove', String(x), String(y)], options);
    const { stdout } = await this.run(this.xdotool, ['getmouselocation', '--shell'], options);
    const position = Object.fromEntries(stdout.trim().split('\n').map(line => line.split('=')));
    if (Number(position.X) !== x || Number(position.Y) !== y) throw failure('invalid_geometry', 'Native pointer did not reach the resolved target');
    await this.run(this.xdotool, ['click', '1'], options);
  }
  async key(key, options) {
    await this.run(this.xdotool, ['windowfocus', '--sync', this.windowId], options);
    const aliases = { Control: 'ctrl', Meta: 'super', ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down', Backspace: 'BackSpace', Enter: 'Return' };
    const chord = key.split('+').map(part => aliases[part] ?? part).join('+');
    await this.run(this.xdotool, ['key', '--clearmodifiers', chord], options);
  }
  async text(value, options) {
    await this.run(this.xdotool, ['windowfocus', '--sync', this.windowId], options);
    await this.run(this.xdotool, ['type', '--clearmodifiers', '--', value], options);
  }
  async capture(path, options) { await this.run(this.imageMagick, ['-window', this.windowId, `png:${resolve(path)}`], options); }
}
