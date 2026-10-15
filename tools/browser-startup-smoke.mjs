// Deterministic startup contracts against the exact packaged bytes, without timing gates.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { assertGpuCompositing, hostedLinuxWebGpuLaunchOptions, uiLibraryEnvironment } from './browser-launch.mjs';
import { createProductionServer } from './browser-serve.mjs';

async function restoredNonImageLayout(browser, url) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  try {
    const page = await context.newPage();
    await page.clock.install();
    await page.goto(url);
    await page.waitForFunction(() => window.__STOBIE_STARTUP?.status === 'ready' && window.__STOBIE_HANDLE);
    // Existing test action seeds a valid all-tabs Results layout. Save and reopen
    // through ordinary persistence; no application/storage reset runs on reload.
    await page.evaluate(() => window.__STOBIE_HANDLE.test_action({ kind: 'queue_zero_viewport_upload' }));
    await page.waitForFunction(() => {
      const panes = window.__STOBIE_HANDLE.test_snapshot().visible_panes;
      return panes.length === 1 && panes[0] === 5;
    });
    const clickRect = async rect => {
      await page.mouse.move((rect.min_x + rect.max_x) / 2, (rect.min_y + rect.max_y) / 2);
      await page.mouse.down(); await page.waitForTimeout(80); await page.mouse.up();
    };
    await clickRect(await page.evaluate(() => window.__STOBIE_HANDLE.test_snapshot().ui_snapshot.nodes.find(n => n.actions.includes('save_layout')).rect));
    await page.waitForFunction(() => localStorage.getItem('stobie.vertical-slice.v2') !== null);
    await page.reload();
    await page.waitForFunction(() => window.__STOBIE_HANDLE && window.__STOBIE_STARTUP?.milestones.rendering_opportunity_proxy && window.__STOBIE_STARTUP?.milestones.worker_ready);
    let state = await page.evaluate(() => ({ startup: window.__STOBIE_STARTUP, snapshot: window.__STOBIE_HANDLE.test_snapshot() }));
    assert.deepEqual(state.snapshot.visible_panes, [5], 'Results layout must actually restore');
    assert.equal(state.snapshot.render.draw_calls, 0);
    assert.equal(state.startup.milestones.first_useful_content, undefined, 'image timing stays unavailable');
    assert.equal(state.startup.status, 'ready', 'usable non-image workspace must finish startup');
    await page.clock.fastForward(61_000);
    state = await page.evaluate(() => ({ startup: window.__STOBIE_STARTUP, snapshot: window.__STOBIE_HANDLE.test_snapshot() }));
    assert.equal(state.startup.status, 'ready', 'watchdog must not destroy a valid restored layout');
    assert.deepEqual(state.snapshot.visible_panes, [5]);
    await clickRect(state.snapshot.ui_geometry.tabs.find(tab => tab.pane === 1).rect);
    await page.waitForFunction(() => window.__STOBIE_STARTUP.milestones.first_useful_content && window.__STOBIE_HANDLE.test_snapshot().render.draw_calls > 0);
    console.log('Restored Results-only layout stays usable past watchdog; physical image-tab selection records later content');
  } finally { await context.close(); }
}

// WebGPU unavailable in three ways, each starting on WebGL2.
const webgl2Cases = {
  fallback: {
    init: () => Object.defineProperty(navigator, 'gpu', { value: undefined }),
    selection: 'fallback', reason: /^WebGPU is unavailable \(no navigator\.gpu\)$/,
  },
  device: {
    init: () => { GPUAdapter.prototype.requestDevice = () => Promise.reject(new Error('smoke: device refused')); },
    selection: 'fallback', reason: /^WebGPU failed to start: .*smoke: device refused/,
  },
  forced: { query: '?renderer=webgl2', selection: 'forced', reason: /^forced by \?renderer=webgl2$/ },
};
const failureSpecs = {
  // Neither backend: no navigator.gpu and no WebGL2 context.
  neither: {
    init: () => {
      Object.defineProperty(navigator, 'gpu', { value: undefined });
      const getContext = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (type, ...rest) { return type === 'webgl2' ? null : getContext.call(this, type, ...rest); };
    },
    phase: 'framework_start', cleanup: 'destroyed',
    message: /^No supported renderer: WebGPU is unavailable \(no navigator\.gpu\); WebGL2 failed: .*getContext/,
  },
  override: { query: '?renderer=webgl', phase: 'framework_start', cleanup: 'destroyed', message: /unknown renderer override "webgl"/ },
  // A forced backend has no fallback.
  'forced-webgpu': {
    init: () => Object.defineProperty(navigator, 'gpu', { value: undefined }),
    query: '?renderer=webgpu', phase: 'framework_start', cleanup: 'destroyed',
    message: /^No supported renderer: WebGPU is unavailable \(no navigator\.gpu\); WebGL2 was not tried \(forced by \?renderer=webgpu\)$/,
  },
  // The viewer is not on AppHost and still requires WebGPU.
  webgpu: {
    init: () => Object.defineProperty(navigator, 'gpu', { value: undefined }),
    phase: 'framework_start', cleanup: 'not_constructed', message: /WebGPU is unavailable/,
  },
  worker: { phase: 'worker', cleanup: 'destroyed', message: /./ },
};
const failureCases = { lab: ['neither', 'worker'], gallery: ['neither', 'override', 'forced-webgpu'], viewer: ['webgpu', 'worker'] };

// The Gallery renders the same pixels on both backends: both use an Rgba8Unorm
// surface with egui's own gamma. Allow a little rasteriser variation.
const VISUAL_TOLERANCE = { channel: 8, fraction: 0.001 };
async function backendScreenshot(browser, url) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  try {
    const page = await context.newPage();
    await page.goto(url);
    await page.waitForFunction(() => window.__STOBIE_STARTUP?.failure || window.__STOBIE_STARTUP?.status === 'ready', null, { timeout: 60000 });
    await page.mouse.move(1439, 899);
    await page.waitForTimeout(1000);
    return { backend: await page.locator('canvas').getAttribute('data-stobie-renderer'), png: await page.screenshot() };
  } finally { await context.close(); }
}
async function compareBackends(browser, url) {
  const webgpu = await backendScreenshot(browser, url);
  const webgl2 = await backendScreenshot(browser, `${url}?renderer=webgl2`);
  assert.deepEqual([webgpu.backend, webgl2.backend], ['webgpu', 'webgl2']);
  const context = await browser.newContext();
  try {
    const page = await context.newPage();
    const result = await page.evaluate(async ([a, b, channel]) => {
      const decode = async base64 => {
        const bitmap = await createImageBitmap(new Blob([Uint8Array.from(atob(base64), c => c.charCodeAt(0))], { type: 'image/png' }));
        const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
        const context = canvas.getContext('2d'); context.drawImage(bitmap, 0, 0);
        return context.getImageData(0, 0, bitmap.width, bitmap.height);
      };
      const [x, y] = [await decode(a), await decode(b)];
      let differing = 0, maximum = 0;
      for (let i = 0; i < x.data.length; i += 4) {
        const delta = Math.max(...[0, 1, 2].map(c => Math.abs(x.data[i + c] - y.data[i + c])));
        maximum = Math.max(maximum, delta);
        if (delta > channel) differing += 1;
      }
      return { pixels: x.width * x.height, differing, maximum, same_size: x.width === y.width && x.height === y.height };
    }, [webgpu.png.toString('base64'), webgl2.png.toString('base64'), VISUAL_TOLERANCE.channel]);
    assert(result.same_size, 'backend screenshots differ in size');
    assert(result.differing <= result.pixels * VISUAL_TOLERANCE.fraction, `WebGL2 differs from WebGPU beyond tolerance: ${JSON.stringify(result)}`);
    return result;
  } finally { await context.close(); }
}

const directory = process.argv[2] || 'target/browser-production';
const browser = await chromium.launch({ ...hostedLinuxWebGpuLaunchOptions(), env: uiLibraryEnvironment() });
try {
  const session = await browser.newBrowserCDPSession();
  assertGpuCompositing((await session.send('SystemInfo.getInfo')).gpu);
  await session.detach();
  if (process.argv[3]) {
    const previous = createProductionServer({ directory: process.argv[3], recordRequests: true });
    await new Promise(resolve => previous.listen(0, '127.0.0.1', resolve));
    const port = previous.address().port;
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await context.newPage();
    await page.goto(`http://127.0.0.1:${port}/lab/`);
    await page.waitForFunction(() => window.__STOBIE_STARTUP?.status === 'ready');
    const oldId = previous.manifest.apps.find(app => app.name === 'lab').contentId;
    const oldEtag = previous.requests.find(r => r.url === '/lab/').headers.etag;
    await page.goto('about:blank');
    previous.closeAllConnections(); await new Promise(resolve => previous.close(resolve));
    const current = createProductionServer({ directory, recordRequests: true, retainedDirectories: [process.argv[3]] });
    await new Promise(resolve => current.listen(port, '127.0.0.1', resolve));
    try {
      const newId = current.manifest.apps.find(app => app.name === 'lab').contentId;
      assert.notEqual(oldId, newId, 'publication check needs two distinct builds');
      await page.goto(`http://127.0.0.1:${port}/lab/`);
      await page.waitForFunction(() => window.__STOBIE_STARTUP?.status === 'ready');
      const assets = await page.evaluate(() => performance.getEntriesByType('resource').map(r => r.name).filter(name => name.includes('/assets/')));
      assert(assets.length > 0 && assets.every(name => name.includes(`/assets/${newId}/`)), 'revisit must use only new package assets');
      assert(current.requests.some(r => r.url === '/lab/' && r.status === 200 && r.ifNoneMatch === oldEtag), 'normal revisit must revalidate mutable HTML');
      console.log(`Normal cached revisit upgraded ${oldId} -> ${newId} without mixing packages`);
    } finally { await context.close(); current.closeAllConnections(); await new Promise(resolve => current.close(resolve)); }
  }
  for (const basePath of ['/', '/preview/nested/']) {
    const server = createProductionServer({ directory, basePath });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    try {
      for (const app of ['lab', 'gallery']) {
        const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
        const page = await context.newPage();
        const errors = []; page.on('pageerror', e => errors.push(String(e)));
        await page.goto(`http://127.0.0.1:${server.address().port}${basePath}${app}/`);
        await page.waitForFunction(() => { const s = window.__STOBIE_STARTUP; return s?.failure || (s?.status === 'ready' && s.milestones.first_useful_content); }, null, { timeout: 60000 });
        const report = await page.evaluate(() => window.__STOBIE_STARTUP);
        assert.equal(report.status, 'ready', JSON.stringify(report));
        for (const milestone of ['wasm_init_begin', 'wasm_init_end', 'application_construct_begin', 'application_construct_end', 'framework_start_begin', 'framework_start_end', 'workspace_frame_submitted', 'rendering_opportunity_proxy', 'first_useful_content']) assert(report.milestones[milestone], milestone);
        assert.equal(report.renderer?.backend, 'webgpu', 'WebGPU stays preferred where it is available');
        assert.equal(report.renderer.selection, 'preferred');
        assert.equal(errors.length, 0, errors.join('\n'));
        await context.close();
        console.log(`Startup complete: ${basePath}${app}/`);
      }
      if (basePath === '/') await restoredNonImageLayout(browser, `http://127.0.0.1:${server.address().port}/lab/`);
      if (basePath === '/') {
        const visual = await compareBackends(browser, `http://127.0.0.1:${server.address().port}/gallery/`);
        console.log(`Gallery WebGPU and WebGL2 agree: ${visual.differing} of ${visual.pixels} pixels differ by more than ${VISUAL_TOLERANCE.channel}/255 (maximum ${visual.maximum})`);
      }
      {
        // Forcing the preferred backend keeps WebGPU and says it was forced.
        const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
        const page = await context.newPage();
        await page.goto(`http://127.0.0.1:${server.address().port}${basePath}gallery/?renderer=webgpu`);
        await page.waitForFunction(() => window.__STOBIE_STARTUP?.failure || window.__STOBIE_STARTUP?.status === 'ready', null, { timeout: 60000 });
        const report = await page.evaluate(() => window.__STOBIE_STARTUP);
        assert.equal(report.status, 'ready', JSON.stringify(report.failure));
        assert.deepEqual([report.renderer.backend, report.renderer.selection, report.renderer.reason], ['webgpu', 'forced', 'forced by ?renderer=webgpu']);
        assert(report.milestones.workspace_frame_submitted);
        await context.close();
        console.log(`WebGPU startup (forced): ${basePath}gallery/`);
      }
      for (const app of ['lab', 'gallery']) for (const [name, spec] of Object.entries(webgl2Cases)) {
        const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
        if (spec.init) await context.addInitScript(spec.init);
        const page = await context.newPage();
        const errors = []; page.on('pageerror', e => errors.push(String(e)));
        await page.goto(`http://127.0.0.1:${server.address().port}${basePath}${app}/${spec.query ?? ''}`);
        await page.waitForFunction(() => { const s = window.__STOBIE_STARTUP; return s?.failure || (s?.status === 'ready' && s.milestones.first_useful_content); }, null, { timeout: 60000 });
        const report = await page.evaluate(() => ({ ...window.__STOBIE_STARTUP, canvases: document.querySelectorAll('canvas').length }));
        assert.equal(report.status, 'ready', JSON.stringify(report));
        assert.equal(report.renderer?.backend, 'webgl2', JSON.stringify(report.renderer));
        assert.equal(report.renderer.selection, spec.selection);
        assert.match(report.renderer.reason, spec.reason);
        assert.equal(report.milestones.workspace_frame_submitted, undefined, 'WebGL2 has no queue submission to observe');
        assert.equal(report.submissionObservation, 'unavailable: webgl2 has no GPUQueue.submit');
        assert(report.milestones.rendering_opportunity_proxy && report.milestones.first_useful_content);
        assert.equal(report.canvases, 1, 'a WebGL2 retry replaces the canvas rather than adding one');
        assert.equal(errors.length, 0, errors.join('\n'));
        await context.close();
        console.log(`WebGL2 startup (${name}): ${basePath}${app}/ (${report.renderer.reason})`);
      }
      for (const [app, failures] of Object.entries(failureCases)) for (const failure of failures) {
        const spec = failureSpecs[failure];
        const context = await browser.newContext();
        if (failure === 'worker') await context.route('**/worker.js', route => route.abort());
        if (spec.init) await context.addInitScript(spec.init);
        const page = await context.newPage();
        await page.goto(`http://127.0.0.1:${server.address().port}${basePath}${app}/${spec.query ?? ''}`);
        await page.waitForFunction(() => window.__STOBIE_STARTUP?.status === 'failed', null, { timeout: 60000 });
        await page.waitForFunction(() => window.__STOBIE_STARTUP.cleanup, null, { timeout: 5000 });
        const result = await page.evaluate(() => ({ startup: window.__STOBIE_STARTUP, text: document.getElementById('loading').textContent, ready: document.body.classList.contains('ready') }));
        assert(result.startup.failure.phase.includes(spec.phase), JSON.stringify(result));
        assert.match(result.startup.failure.message, spec.message);
        assert(result.text.includes('Initialisation failed')); assert.equal(result.ready, false);
        assert.equal(result.startup.cleanup, spec.cleanup, 'failed startup must release its handle');
        await context.close(); console.log(`Startup failure handled: ${basePath}${app}/${failure}`);
      }
    } finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
  }
} finally { await browser.close(); }
