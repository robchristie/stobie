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

export function installStartup({ app, workerUrl, canvasId, scope = globalThis, timeoutMs = 60000, onFailure = () => {} }) {
  const streaming = observeStreaming(scope);
  const report = scope.__STOBIE_STARTUP = {
    instantiateStreaming: streaming.report,
    schema: 1, app, timeOrigin: scope.performance.timeOrigin, phase: 'bootstrap', status: 'starting',
    milestones: {}, failure: null, submissionObservation: 'unavailable', renderer: null,
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
  // The host names its backend on the canvas before constructing the
  // application. WebGL2 has no GPUQueue, so it takes the CPU handoff path.
  function observeRenderer() {
    const data = canvasId === undefined ? undefined : scope.document?.getElementById(canvasId)?.dataset;
    if (report.renderer || !data?.stobieRenderer) return;
    report.renderer = {
      backend: data.stobieRenderer, adapter: data.stobieRendererAdapter ?? '',
      selection: data.stobieRendererSelection ?? '', reason: data.stobieRendererReason ?? null,
    };
    if (report.renderer.backend !== 'webgpu') {
      restoreSubmit(); restoreSubmit = () => {};
      report.submissionObservation = `unavailable: ${report.renderer.backend} has no GPUQueue.submit`;
    }
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
    scope.performance.mark?.(`stobie:${name}`);
    if (['application_construct_begin', 'framework_start_end', 'workspace_ui_complete'].includes(name)) observeRenderer();
    if (name === 'application_construct_begin') { report.phase = 'application_construct'; shell('Constructing workspace…'); }
    if (name === 'application_construct_end') report.phase = 'framework_start';
    if (name === 'workspace_ui_complete') {
      // This is CPU UI construction only; the submit hook records the GPU API call.
      scope.document?.body.classList.add('ready');
      if (report.submissionObservation.startsWith('unavailable')) opportunity();
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
  scope.__stobieStartupMark = mark;
  scope.__stobieStartupFail = fail;
  scope.__stobieWorkerUrl = () => workerUrl;
  scope.__stobieStartupWorkerMessage = data => {
    if (!data?.__stobieStartup) return false;
    if (report.status === 'failed') return true;
    const incoming = data.__stobieStartup;
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

// Keep `--stobie-viewport-height` and `--stobie-viewport-top` on the root
// element at the visual viewport's size and offset, and `--stobie-bottom-inset`
// at the bottom safe area, or 0 while an on-screen keyboard covers it. iOS
// shrinks only the visual viewport for its keyboard, so a page shell sized from
// these stays above the keyboard. Returns a function that stops it.
export function fitVisualViewport({ scope = globalThis } = {}) {
  const viewport = scope.visualViewport;
  const style = scope.document?.documentElement?.style;
  if (!viewport || !style) return () => {};
  const update = () => {
    // A keyboard takes far more height than browser chrome does.
    const keyboard = scope.innerHeight - viewport.height > 120;
    style.setProperty('--stobie-viewport-height', `${viewport.height}px`);
    style.setProperty('--stobie-viewport-top', `${viewport.offsetTop}px`);
    style.setProperty('--stobie-bottom-inset', keyboard ? '0px' : 'env(safe-area-inset-bottom, 0px)');
  };
  update();
  viewport.addEventListener('resize', update);
  viewport.addEventListener('scroll', update);
  return () => { viewport.removeEventListener('resize', update); viewport.removeEventListener('scroll', update); };
}

// Register the service worker `stobie build-web` writes, `sw.js` beside the
// page, for the page's directory. `mode` 'auto' registers it when the page
// links a manifest, true always and false never. The outcome is recorded in
// `report.serviceWorker`; registration never delays or fails startup.
export async function registerServiceWorker({ report, mode = 'auto', scope = globalThis }) {
  const requested = mode === true || (mode === 'auto' && Boolean(scope.document?.querySelector?.('link[rel~="manifest"]')));
  const container = scope.navigator?.serviceWorker;
  if (!requested) report.serviceWorker = { state: 'not_requested' };
  else if (!container || scope.isSecureContext === false) report.serviceWorker = { state: 'unsupported' };
  else {
    report.serviceWorker = { state: 'registering' };
    try {
      const page = new URL('./', scope.location.href);
      const registration = await container.register(new URL('sw.js', page).href, { scope: page.href });
      report.serviceWorker = { state: 'registered', scope: registration.scope };
    } catch (error) { report.serviceWorker = { state: 'failed', message: describe(error) }; }
  }
  return report.serviceWorker;
}

// The framework host chooses WebGPU or WebGL2 itself. Only an application
// whose renderer needs WebGPU, such as the viewer, sets `requireWebGpu`.
// `fitViewport` sizes the page from the visual viewport (`fitVisualViewport`),
// and `serviceWorker` registers the build's service worker once the
// application has started (`registerServiceWorker`).
export async function startApplication({ app, workerUrl, load, canvasId, handleName, start, afterStart, requireWebGpu = false, fitViewport = false, serviceWorker = 'auto', scope = globalThis }) {
  let handle;
  if (fitViewport) fitVisualViewport({ scope });
  const startup = installStartup({ app, workerUrl, canvasId, scope, onFailure: () => {
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
    startup.phase('framework_start', 'Initialising renderer…');
    if (requireWebGpu && !scope.navigator?.gpu) throw new Error('WebGPU is unavailable in this browser or context');
    startup.mark('framework_start_begin');
    handle = new module.WebHandle();
    await start(handle, scope.document.getElementById(canvasId));
    if (startup.report.status === 'failed') { handle.destroy(); return startup; }
    startup.mark('framework_start_end');
    scope[handleName] = handle;
    afterStart?.(handle);
    // Installation caches the package, so it waits until the application runs.
    registerServiceWorker({ report: startup.report, mode: serviceWorker, scope });
    startup.phase('workspace', 'Preparing workspace…');
  } catch (error) { startup.fail(startup.report.phase, error); }
  return startup;
}

export function installWorkerStartup({ scope = globalThis } = {}) {
  const streaming = observeStreaming(scope);
  const report = { instantiateStreaming: streaming.report, timeOrigin: scope.performance.timeOrigin, milestones: {}, memory: {}, resources: [], failure: null };
  let memory;
  const publish = () => scope.postMessage({ __stobieStartup: structuredClone(report) });
  const mark = name => {
    if (report.failure || report.milestones[name]) return;
    report.milestones[name] = stamp(scope);
    scope.performance.mark?.(`stobie:${name}`);
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
