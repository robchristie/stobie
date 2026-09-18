// Finite startup observation only. A queue submission and a later RAF callback
// do not establish physical display presentation or GPU execution completion.
const stamp = scope => { const atMs = scope.performance.now(); return { atMs, epochMs: scope.performance.timeOrigin + atMs }; };
const describe = error => error?.message || String(error);

function observeStreaming(scope) {
  const api = scope.WebAssembly;
  const original = api?.instantiateStreaming;
  const report = { available: typeof original === 'function', observed: false, attempts: 0, completed: 0, failures: 0 };
  const observed = function (...args) {
    report.attempts += 1;
    try {
      return Promise.resolve(original.apply(this, args)).then(value => {
        report.completed += 1; return value;
      }, error => { report.failures += 1; throw error; });
    } catch (error) { report.failures += 1; throw error; }
  };
  if (original) {
    try { api.instantiateStreaming = observed; report.observed = api.instantiateStreaming === observed; } catch { /* Unavailable observation remains explicit. */ }
  }
  return { report, restore() { if (api?.instantiateStreaming === observed) api.instantiateStreaming = original; } };
}

export function installStartup({ app, workerUrl, scope = globalThis, timeoutMs = 60000, onFailure = () => {} }) {
  const streaming = observeStreaming(scope);
  const report = scope.__POLYORAMA_STARTUP = {
    instantiateStreaming: streaming.report,
    schema: 1, app, timeOrigin: scope.performance.timeOrigin, phase: 'bootstrap', status: 'starting',
    milestones: {}, failure: null, submissionObservation: 'unavailable',
    memory: { main: {}, worker: {} }, worker: null,
  };
  let memory, timer, raf, foregroundStarted, foregroundRemaining = timeoutMs, restoreSubmit = () => {};
  const loading = scope.document?.getElementById('loading');
  const shell = message => { if (loading) loading.textContent = message; };
  const sample = name => { if (memory) report.memory.main[name] = { ...stamp(scope), wasmLinearBytes: memory.buffer.byteLength }; };
  function cleanup() {
    scope.clearTimeout(timer);
    scope.document?.removeEventListener?.('visibilitychange', onVisibility);
    if (raf !== undefined) scope.cancelAnimationFrame(raf);
    restoreSubmit();
    streaming.restore();
    scope.removeEventListener?.('error', onError);
    scope.removeEventListener?.('unhandledrejection', onRejection);
  }
  function complete() {
    if (report.status === 'starting' && report.milestones.rendering_opportunity_proxy && (!workerUrl || report.milestones.worker_ready)) {
      report.status = 'ready'; report.phase = 'ready'; cleanup();
    }
  }
  function fail(phase, error) {
    if (report.status !== 'starting') return false;
    report.failure = { phase, message: describe(error), ...stamp(scope) };
    report.status = 'failed'; report.phase = phase;
    cleanup();
    scope.document?.body.classList.remove('ready');
    shell(`Initialisation failed (${phase}): ${report.failure.message}`);
    onFailure();
    return true;
  }
  function opportunity() {
    if (raf !== undefined || report.status !== 'starting') return;
    // One callback, requested only after UI handoff/submission. RAF is a proxy,
    // not proof that the compositor displayed this workspace.
    raf = scope.requestAnimationFrame(() => { mark('rendering_opportunity_proxy'); });
  }
  function mark(name) {
    // Usable workspace readiness does not require imagery: a saved Results-only
    // layout may never draw any. First content remains an optional, one-shot observation.
    if (report.status === 'failed' || report.milestones[name] || (report.status === 'ready' && name !== 'first_useful_content')) return false;
    report.milestones[name] = stamp(scope);
    scope.performance.mark?.(`polyorama:${name}`);
    if (name === 'application_construct_begin') { report.phase = 'application_construct'; shell('Constructing workspace…'); }
    if (name === 'application_construct_end') report.phase = 'framework_start';
    if (name === 'workspace_ui_complete') {
      // This is CPU UI construction only; the submit hook records the GPU API call.
      scope.document?.body.classList.add('ready');
      if (report.submissionObservation === 'unavailable') opportunity();
    }
    if (name === 'workspace_frame_submitted') { restoreSubmit(); opportunity(); }
    if (name === 'first_useful_content') sample('first_useful_content');
    complete();
    return true;
  }
  const onError = event => fail(report.phase, event.error || event.message);
  const onRejection = event => fail(report.phase, event.reason);
  scope.addEventListener?.('error', onError);
  scope.addEventListener?.('unhandledrejection', onRejection);
  const prototype = scope.GPUQueue?.prototype;
  if (prototype?.submit) {
    const original = prototype.submit;
    const observed = function (...args) {
      const result = original.apply(this, args);
      if (report.milestones.workspace_ui_complete) mark('workspace_frame_submitted');
      return result;
    };
    try {
      prototype.submit = observed;
      if (prototype.submit === observed) {
        report.submissionObservation = 'GPUQueue.submit after workspace CPU UI construction';
        restoreSubmit = () => { if (prototype.submit === observed) prototype.submit = original; };
      }
    } catch { /* Explicit unavailable fallback; never mislabel CPU work as submission. */ }
  }
  scope.__polyoramaStartupMark = mark;
  scope.__polyoramaStartupFail = fail;
  scope.__polyoramaWorkerUrl = () => workerUrl;
  scope.__polyoramaStartupWorkerMessage = data => {
    if (!data?.__polyoramaStartup) return false;
    if (report.status === 'failed') return true;
    const incoming = data.__polyoramaStartup;
    // Keep the finite first-decode sample even when it follows usable readiness.
    // Subsequent failures belong to the existing worker/runtime error path.
    if (report.status === 'ready' && (incoming.failure || report.worker?.milestones?.first_decode_complete)) return true;
    report.worker = incoming;
    report.memory.worker = incoming.memory;
    if (incoming.failure) fail(`worker:${incoming.failure.phase}`, incoming.failure.message);
    else if (incoming.milestones.worker_ready) mark('worker_ready');
    return true;
  };
  function onVisibility() {
    if (report.status !== 'starting') return;
    scope.clearTimeout(timer);
    if (foregroundStarted !== undefined) {
      foregroundRemaining = Math.max(0, foregroundRemaining - (scope.performance.now() - foregroundStarted));
      foregroundStarted = undefined;
    }
    // RAF may be suspended in background tabs. Only foreground time consumes
    // the startup budget; this listener is removed as soon as the workspace is usable.
    if (scope.document?.hidden) return;
    foregroundStarted = scope.performance.now();
    timer = scope.setTimeout(() => {
      if (scope.document?.hidden) {
        // A visibility event can be delayed behind a throttled timer. Without
        // its transition timestamp, do not charge unknown background time.
        foregroundStarted = undefined; onVisibility(); return;
      }
      fail(report.phase, `Startup exceeded ${timeoutMs / 1000} seconds of foreground time`);
    }, foregroundRemaining);
  }
  scope.document?.addEventListener?.('visibilitychange', onVisibility);
  onVisibility();
  mark('bootstrap');
  return {
    report, mark, fail,
    setMemory(value) { memory = value; streaming.restore(); sample('wasm_init_end'); },
    phase(name, message) { if (report.status === 'starting') { report.phase = name; shell(message); } },
  };
}

export async function startApplication({ app, workerUrl, load, canvasId, handleName, start, afterStart, scope = globalThis }) {
  let handle;
  const startup = installStartup({ app, workerUrl, scope, onFailure: () => {
    // Leave the Rust callback/constructor before releasing partially started resources.
    scope.queueMicrotask?.(() => {
      try { handle?.destroy(); startup.report.cleanup = handle ? 'destroyed' : 'not_constructed'; }
      catch (error) { startup.report.cleanup = { failure: describe(error) }; }
    });
  } });
  try {
    startup.phase('wasm_init', 'Loading application…');
    startup.mark('wasm_init_begin');
    const module = await load();
    if (startup.report.status === 'failed') return startup;
    const wasm = await module.default();
    if (startup.report.status === 'failed') return startup;
    startup.setMemory(wasm.memory);
    startup.mark('wasm_init_end');
    startup.phase('framework_start', 'Initialising WebGPU…');
    if (!scope.navigator?.gpu) throw new Error('WebGPU is unavailable in this browser or context');
    startup.mark('framework_start_begin');
    handle = new module.WebHandle();
    await start(handle, scope.document.getElementById(canvasId));
    if (startup.report.status === 'failed') { handle.destroy(); return startup; }
    startup.mark('framework_start_end');
    scope[handleName] = handle;
    afterStart?.(handle);
    startup.phase('workspace', 'Preparing workspace…');
  } catch (error) { startup.fail(startup.report.phase, error); }
  return startup;
}

export function installWorkerStartup({ scope = globalThis } = {}) {
  const streaming = observeStreaming(scope);
  const report = { instantiateStreaming: streaming.report, timeOrigin: scope.performance.timeOrigin, milestones: {}, memory: {}, resources: [], failure: null };
  let memory;
  const publish = () => scope.postMessage({ __polyoramaStartup: structuredClone(report) });
  const mark = name => {
    if (report.failure || report.milestones[name]) return;
    report.milestones[name] = stamp(scope);
    scope.performance.mark?.(`polyorama:${name}`);
    if (memory) report.memory[name] = { ...stamp(scope), wasmLinearBytes: memory.buffer.byteLength };
    if (name === 'worker_ready' || name === 'first_decode_complete') {
      report.resources = scope.performance.getEntriesByType('resource').map(entry => ({
        name: entry.name, startTime: entry.startTime, duration: entry.duration,
        transferSize: entry.transferSize, encodedBodySize: entry.encodedBodySize, decodedBodySize: entry.decodedBodySize,
      }));
      publish();
    }
  };
  mark('worker_bootstrap');
  return {
    mark,
    setMemory(value) { memory = value; streaming.restore(); },
    fail(phase, error) {
      if (report.failure) return;
      streaming.restore();
      report.failure = { phase, message: describe(error), ...stamp(scope) }; publish();
    },
  };
}
