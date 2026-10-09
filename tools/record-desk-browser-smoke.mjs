import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { resolve, extname } from 'node:path';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions } from './browser-launch.mjs';
import { waitForClickTarget } from './record-desk-target.mjs';
import { focusByTab } from './browser-keyboard-focus.mjs';

const root = resolve('consumers/record-desk/web');
const evidence = resolve(process.env.POLYORAMA_EVIDENCE_DIR ?? '.tools/runtime/record-desk-evidence/record-desk');
await mkdir(evidence, { recursive: true });
const server = createServer(async (req, res) => {
  const path = resolve(root, req.url === '/' ? 'index.html' : `.${new URL(req.url, 'http://local').pathname}`);
  try {
    if (!path.startsWith(`${root}/`)) throw new Error('outside root');
    const body = await readFile(path);
    res.writeHead(200, { 'Content-Type': ({ '.wasm': 'application/wasm', '.js': 'text/javascript', '.html': 'text/html' })[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(body);
  } catch { res.writeHead(404); res.end(); }
});
let browser;
const steps = [];
const pointerTargets = [];
const errors = [];
let page;
const storageKey = 'polyorama.record-desk.v1';
const snapshot = () => page.evaluate(() => JSON.parse(window.__RECORD_DESK.snapshot()));
async function wait(predicate) {
  await page.waitForFunction(`window.__RECORD_DESK && (${predicate})(JSON.parse(window.__RECORD_DESK.snapshot()))`, null, { timeout: 15000 });
}
async function ready() {
  await page.locator('canvas[data-ready="true"]').waitFor();
  await wait('s => ["record-desk.undo", "record-desk.redo", "record-desk.save", "record-desk.restore", "record-desk.arrange"].every(action => s.ui.nodes.some(n => n.actions.includes(action) && n.rect.max_y > n.rect.min_y))');
}
async function record(name) {
  const state = await snapshot();
  assert.deepEqual(state.ui.text_audit, [], `${name}: text audit`);
  assert.deepEqual(state.ui.semantic_audit, [], `${name}: semantic audit`);
  steps.push({ name, state });
  await writeFile(resolve(evidence, `browser-${name}.json`), JSON.stringify(state, null, 2));
}
async function clickNode(id) {
  const { state, node, root, observed_ms } = await waitForClickTarget(snapshot, ms => page.waitForTimeout(ms), id);
  pointerTargets.push({ id, node_id: node.id, frame: state.ui.frame, rect: node.rect, observed_ms });
  const r = node.rect;
  const canvas = await page.locator('canvas').boundingBox();
  const rootRect = root.rect;
  await page.mouse.click(canvas.x + ((r.min_x + r.max_x) / 2 - rootRect.min_x) * canvas.width / (rootRect.max_x - rootRect.min_x), canvas.y + ((r.min_y + r.max_y) / 2 - rootRect.min_y) * canvas.height / (rootRect.max_y - rootRect.min_y));
}
async function focusNode(id) {
  return focusByTab(id, {
    target: async () => {
      const s = await snapshot();
      const node = s.ui.nodes.find(node => node.id === id);
      assert(node, `current keyboard target ${id}`);
      return { node, tab_input_epoch: s.tab_input_epoch };
    },
    pressTab: () => page.keyboard.press('Tab'),
    waitForTab: epoch => wait(`s => s.tab_input_epoch > ${epoch}`),
    wait: ms => page.waitForTimeout(ms),
    maxTabs: 24,
    settleMs: 50,
  });
}

async function field(id, value) {
  await clickNode(id);
  await wait(`s => s.ui.nodes.some(n => n.id === ${JSON.stringify(id)} && n.focused)`);
  await page.waitForFunction(() => document.activeElement?.tagName === 'INPUT');
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Backspace');
  if (value) await page.keyboard.type(value);
  await wait(`s => s.draft?.title === ${JSON.stringify(value)}`);
}
try {
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  const url = process.env.RECORD_DESK_URL ?? `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch(hostedLinuxWebGpuLaunchOptions());
  page = await browser.newPage({ viewport: { width: 1080, height: 760 } });
  page.on('pageerror', e => errors.push(e.toString()));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('response', r => { if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
  await page.goto(url);
  await page.bringToFront();
  await page.evaluate(() => {
    window.recordDeskInputEvidence = [];
    for (const type of ['keydown', 'input', 'focusin', 'focusout']) {
      document.addEventListener(type, event => {
        const observation = { type, key: event.key, keyCode: event.keyCode, ctrl: event.ctrlKey, data: event.data, active: document.activeElement?.tagName, target: event.target.tagName, defaultPrevented: event.defaultPrevented };
        window.recordDeskInputEvidence.push(observation);
        setTimeout(() => { observation.finalPrevented = event.defaultPrevented; }, 0);
      }, { capture: true });
    }
  });
  await ready();
  await wait('s => s.records.length === 12 && s.ui.nodes.length > 10');
  await record('ordinary');
  await page.screenshot({ path: resolve(evidence, 'browser-ordinary.png') });
  // Every mutation below is physical input. The hook reads state/geometry only.
  await page.keyboard.press('Control+f');
  await wait('s => s.ui.nodes.some(n => n.id === "record-desk.search" && n.focused)');
  await page.waitForFunction(() => document.activeElement?.tagName === 'INPUT');
  await page.keyboard.type('agenda', { delay: 40 });
  await wait('s => s.visible_ids.length === 1');
  await focusNode('record-desk.record.1013');
  await page.keyboard.press('Enter');
  await wait('s => s.selected === 1013');
  await record('search-select');
  // Pointer choice: category Ideas hides the selected Operations record.
  await clickNode('record-desk.filter-category');
  await wait('s => s.ui.nodes.some(n => n.id === "record-desk.filter-category.option.Some(Ideas)")');
  await clickNode('record-desk.filter-category.option.Some(Ideas)');
  await wait('s => s.filters.category === "Ideas" && s.visible_ids.length === 0 && s.selected === 1013');
  await clickNode('record-desk.filter-review');
  await wait('s => s.ui.nodes.some(n => n.id === "record-desk.filter-review.option.Unreviewed")');
  await focusNode('record-desk.filter-review.option.Unreviewed');
  await page.keyboard.press('Enter');
  await wait('s => s.filters.review === "Unreviewed"');
  await page.screenshot({ path: resolve(evidence, 'browser-choice-focus.png') });
  await page.keyboard.press('Escape');
  await wait('s => !s.ui.nodes.some(n => n.id.includes("filter-review.option."))');
  await record('no-results');
  await page.screenshot({ path: resolve(evidence, 'browser-no-results.png') });
  await page.keyboard.press('Control+Shift+f');
  await wait('s => s.filters.query === "" && s.filters.category === null && s.visible_ids.length === 12');
  await field('record-desk.title.1013', '');
  await page.keyboard.press('Control+Enter');
  await wait('s => s.error && s.undo_entries === 0 && s.records[1].title === "Prepare review agenda"');
  await record('invalid');
  await field('record-desk.title.1013', 'Review agenda updated');
  await clickNode('record-desk.toggle-reviewed');
  await wait('s => s.draft.reviewed !== s.records[1].reviewed');
  await page.keyboard.press('Control+Enter');
  await wait('s => s.undo_entries === 1 && !s.draft_dirty && s.records[1].title === "Review agenda updated"');
  await record('apply');
  await page.keyboard.press('Control+z');
  await wait('s => s.undo_entries === 0 && s.redo_entries === 1 && s.records[1].title === "Prepare review agenda"');
  await record('undo');
  await page.keyboard.press('Control+Shift+z');
  await wait('s => s.undo_entries === 1 && s.redo_entries === 0 && s.records[1].title === "Review agenda updated"');
  await record('redo');
  await clickNode('polyorama.dock.splitter.1');
  await wait('s => s.ui.nodes.some(n => n.id === "polyorama.dock.splitter.1" && n.focused)');
  await page.keyboard.press('ArrowRight');
  await wait('s => s.workspace.root.Split.fraction > 0.36');
  const savedLayout = (await snapshot()).workspace;
  await page.keyboard.press('Control+s');
  await wait('s => !s.unsaved && !s.error');
  await field('record-desk.title.1013', 'Uncommitted draft excluded');
  await page.keyboard.press('Control+s');
  await wait('s => s.message.includes("excluded")');
  const saved = await page.evaluate(key => JSON.parse(localStorage.getItem(key)), storageKey);
  assert.equal(saved.records[1].title, 'Review agenda updated');
  assert.deepEqual(saved.workspace, savedLayout);
  await page.reload();
  await ready();
  await wait('s => s.records[1].title === "Review agenda updated" && s.undo_entries === 0 && !s.draft_dirty && !s.unsaved');
  assert.deepEqual((await snapshot()).workspace, savedLayout);
  await record('reload-restored');
  await page.waitForTimeout(1000); // Settle startup and egui hover/animation deadlines.
  const idleFrame = (await snapshot()).ui.frame;
  await page.waitForTimeout(350);
  assert.equal((await snapshot()).ui.frame, idleFrame, 'idle app must stop repainting');
  await page.setViewportSize({ width: 390, height: 844 });
  await wait('s => s.workspace.root.Split.axis === "Vertical"');
  await record('narrow');
  await page.screenshot({ path: resolve(evidence, 'browser-narrow.png') });
  // Fault setup is direct local storage injection, never counted as physical input.
  await page.evaluate(key => localStorage.setItem(key, '{broken'), storageKey);
  await page.reload();
  await ready();
  await wait('s => s.save_blocked && s.error');
  await page.keyboard.press('Control+s');
  assert.equal(await page.evaluate(key => localStorage.getItem(key), storageKey), '{broken');
  await record('malformed-preserved');
  await page.screenshot({ path: resolve(evidence, 'browser-error.png') });
  await page.evaluate(key => localStorage.removeItem(key), storageKey);
  await clickNode('record-desk.restore');
  await wait('s => !s.save_blocked && !s.error');
  // Browser storage denial is a deterministic fault fixture, restored afterwards.
  await page.evaluate(() => { window.originalSetItem = Storage.prototype.setItem; Storage.prototype.setItem = () => { throw new DOMException('test quota', 'QuotaExceededError'); }; });
  await clickNode('record-desk.save');
  await wait('s => s.error && s.unsaved');
  await record('write-failure');
  await page.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
  await clickNode('record-desk.save');
  await wait('s => !s.error && !s.unsaved');
  assert.deepEqual(errors, [], 'browser runtime errors');
  const adapter = await page.evaluate(async () => {
    const info = (await navigator.gpu.requestAdapter())?.info;
    return info && { vendor: info.vendor, architecture: info.architecture, device: info.device, description: info.description };
  });
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(resolve(root, 'pkg/record_desk_bg.wasm'))).digest('hex'), url, browser: browser.version(), launch: hostedLinuxWebGpuLaunchOptions(), adapter, errors, steps, pointer_targets: pointerTargets, input_route: 'Playwright mouse/keyboard; snapshot read only', fault_setup: 'localStorage malformed bytes and Storage.setItem quota fixture' };
  await writeFile(resolve(evidence, 'browser-workflow.json'), JSON.stringify(report, null, 2));
  console.log('Record Desk browser passed: search/filter/select, invalid Apply, transaction, undo/redo, save/reload, draft exclusion, layout restore, narrow, errors and idle');
} catch (error) {
  if (page) {
    await page.screenshot({ path: resolve(evidence, 'browser-failure.png') }).catch(() => {});
    await writeFile(resolve(evidence, 'browser-failure.json'), JSON.stringify(await snapshot().catch(() => null), null, 2));
    await writeFile(resolve(evidence, 'browser-input-failure.json'), JSON.stringify(await page.evaluate(() => ({ hasFocus: document.hasFocus(), active: document.activeElement?.outerHTML, events: window.recordDeskInputEvidence })).catch(() => null), null, 2));
    await writeFile(resolve(evidence, 'browser-pointer-targets.json'), JSON.stringify(pointerTargets, null, 2));
  }
  throw error;
} finally {
  await browser?.close();
  await new Promise(r => server.close(r));
}
