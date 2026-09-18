// Deterministic startup contracts against the exact packaged bytes, without timing gates.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions } from './browser-launch.mjs';
import { createProductionServer } from './browser-serve.mjs';

async function restoredNonImageLayout(browser, url) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  try {
    const page = await context.newPage();
    await page.clock.install();
    await page.goto(url);
    await page.waitForFunction(() => window.__POLYORAMA_STARTUP?.status === 'ready' && window.__POLYORAMA_HANDLE);
    // Existing test action seeds a valid all-tabs Results layout. Save and reopen
    // through ordinary persistence; no application/storage reset runs on reload.
    await page.evaluate(() => window.__POLYORAMA_HANDLE.test_action({ kind: 'queue_zero_viewport_upload' }));
    await page.waitForFunction(() => {
      const panes = window.__POLYORAMA_HANDLE.test_snapshot().visible_panes;
      return panes.length === 1 && panes[0] === 5;
    });
    const clickRect = async rect => {
      await page.mouse.move((rect.min_x + rect.max_x) / 2, (rect.min_y + rect.max_y) / 2);
      await page.mouse.down(); await page.waitForTimeout(80); await page.mouse.up();
    };
    await clickRect(await page.evaluate(() => window.__POLYORAMA_HANDLE.test_snapshot().ui_snapshot.nodes.find(n => n.actions.includes('save_layout')).rect));
    await page.waitForFunction(() => localStorage.getItem('polyorama.vertical-slice.v2') !== null);
    await page.reload();
    await page.waitForFunction(() => window.__POLYORAMA_HANDLE && window.__POLYORAMA_STARTUP?.milestones.rendering_opportunity_proxy && window.__POLYORAMA_STARTUP?.milestones.worker_ready);
    let state = await page.evaluate(() => ({ startup: window.__POLYORAMA_STARTUP, snapshot: window.__POLYORAMA_HANDLE.test_snapshot() }));
    assert.deepEqual(state.snapshot.visible_panes, [5], 'Results layout must actually restore');
    assert.equal(state.snapshot.render.draw_calls, 0);
    assert.equal(state.startup.milestones.first_useful_content, undefined, 'image timing stays unavailable');
    assert.equal(state.startup.status, 'ready', 'usable non-image workspace must finish startup');
    await page.clock.fastForward(61_000);
    state = await page.evaluate(() => ({ startup: window.__POLYORAMA_STARTUP, snapshot: window.__POLYORAMA_HANDLE.test_snapshot() }));
    assert.equal(state.startup.status, 'ready', 'watchdog must not destroy a valid restored layout');
    assert.deepEqual(state.snapshot.visible_panes, [5]);
    await clickRect(state.snapshot.ui_geometry.tabs.find(tab => tab.pane === 1).rect);
    await page.waitForFunction(() => window.__POLYORAMA_STARTUP.milestones.first_useful_content && window.__POLYORAMA_HANDLE.test_snapshot().render.draw_calls > 0);
    console.log('Restored Results-only layout stays usable past watchdog; physical image-tab selection records later content');
  } finally { await context.close(); }
}

const directory = process.argv[2] || 'target/browser-production';
const browser = await chromium.launch(hostedLinuxWebGpuLaunchOptions());
try {
  if (process.argv[3]) {
    const previous = createProductionServer({ directory: process.argv[3], recordRequests: true });
    await new Promise(resolve => previous.listen(0, '127.0.0.1', resolve));
    const port = previous.address().port;
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await context.newPage();
    await page.goto(`http://127.0.0.1:${port}/lab/`);
    await page.waitForFunction(() => window.__POLYORAMA_STARTUP?.status === 'ready');
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
      await page.waitForFunction(() => window.__POLYORAMA_STARTUP?.status === 'ready');
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
        await page.waitForFunction(() => { const s = window.__POLYORAMA_STARTUP; return s?.failure || (s?.status === 'ready' && s.milestones.first_useful_content); }, null, { timeout: 60000 });
        const report = await page.evaluate(() => window.__POLYORAMA_STARTUP);
        assert.equal(report.status, 'ready', JSON.stringify(report));
        for (const milestone of ['wasm_init_begin', 'wasm_init_end', 'application_construct_begin', 'application_construct_end', 'framework_start_begin', 'framework_start_end', 'workspace_frame_submitted', 'rendering_opportunity_proxy', 'first_useful_content']) assert(report.milestones[milestone], milestone);
        assert.equal(errors.length, 0, errors.join('\n'));
        await context.close();
        console.log(`Startup complete: ${basePath}${app}/`);
      }
      if (basePath === '/') await restoredNonImageLayout(browser, `http://127.0.0.1:${server.address().port}/lab/`);
      for (const app of ['lab', 'gallery', 'viewer']) for (const failure of app === 'gallery' ? ['webgpu'] : ['webgpu', 'worker']) {
        const context = await browser.newContext();
        if (failure === 'worker') await context.route('**/worker.js', route => route.abort());
        else await context.addInitScript(() => Object.defineProperty(navigator, 'gpu', { value: undefined }));
        const page = await context.newPage();
        await page.goto(`http://127.0.0.1:${server.address().port}${basePath}${app}/`);
        await page.waitForFunction(() => window.__POLYORAMA_STARTUP?.status === 'failed', null, { timeout: 60000 });
        const result = await page.evaluate(() => ({ startup: window.__POLYORAMA_STARTUP, text: document.getElementById('loading').textContent, ready: document.body.classList.contains('ready') }));
        assert(result.startup.failure.phase.includes(failure === 'worker' ? 'worker' : 'framework'), JSON.stringify(result));
        assert(result.text.includes('Initialisation failed')); assert.equal(result.ready, false);
        assert.equal(result.startup.cleanup, failure === 'worker' ? 'destroyed' : 'not_constructed', 'failed startup must release its handle');
        await context.close(); console.log(`Startup failure handled: ${basePath}${app}/${failure}`);
      }
    } finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
  }
} finally { await browser.close(); }
