// Check a hosted application's hidden ARIA layer in Chromium: the browser
// accessibility tree against reviewed expectations, an axe-core rule check,
// and that layer writes track accessibility-tree changes rather than frames.
//
// node tools/aria-layer-browser.mjs --app record-desk|gallery|lab
//
// The applications must already be built (`cargo xtask build-web`,
// `consumers/record-desk/build-web.sh`). Raw evidence goes to
// STOBIE_EVIDENCE_DIR (default .tools/runtime/aria-layer).
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { extname, resolve, sep } from 'node:path';
import { createRequire } from 'node:module';
import { parseArgs } from 'node:util';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions, uiLibraryEnvironment } from './browser-launch.mjs';

const APPS = {
  'record-desk': 'consumers/record-desk/web',
  gallery: 'apps/stobie-gallery/web',
  lab: 'apps/analytical-workspace-lab/web',
};
const { values: options } = parseArgs({ options: { app: { type: 'string' } } });
const app = options.app;
assert(app in APPS, `--app must be one of ${Object.keys(APPS).join(', ')}`);
const root = resolve(APPS[app]);
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/aria-layer', app);
const expectations = JSON.parse(await readFile(resolve('tools/aria-layer-expectations.json'), 'utf8'))[app];
const axeSource = createRequire(import.meta.url).resolve('axe-core/axe.min.js');
const LAYER = '[id^="stobie-a11y:layer"]';
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

const server = createServer(async (request, response) => {
  const name = new URL(request.url, 'http://local').pathname.slice(1) || 'index.html';
  const path = resolve(root, name);
  try {
    assert(path.startsWith(`${root}${sep}`), 'asset inside the application root');
    const body = await readFile(path);
    response.writeHead(200, { 'Content-Type': types[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(body);
  } catch { response.writeHead(404); response.end(); }
});
await mkdir(evidence, { recursive: true });
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await chromium.launch({ ...hostedLinuxWebGpuLaunchOptions(), env: uiLibraryEnvironment() });
const errors = [];
const report = { app, checks: [] };
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  // Count presented frames by their GPU submissions, and layer writes by
  // mutation records, so idle and hover can be told apart from tree changes.
  await page.addInitScript(() => {
    window.__ariaProbe = { submits: 0, mutations: 0 };
    const submit = GPUQueue.prototype.submit;
    GPUQueue.prototype.submit = function (...args) {
      window.__ariaProbe.submits += 1;
      return submit.apply(this, args);
    };
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForSelector(`${LAYER} [role]`, { state: 'attached', timeout: 60000 });
  await page.evaluate(selector => {
    const layer = document.querySelector(selector);
    const canvas = document.querySelector(`[aria-owns="${layer.id}"]`);
    const observer = new MutationObserver(records => {
      window.__ariaProbe.mutations += records.length;
      window.__ariaLog = (window.__ariaLog ?? []).concat(records.map(record => `${record.type} ${record.target.id || record.target.parentElement?.id} ${record.target.getAttribute?.('role')} ${record.target.getAttribute?.('aria-label') ?? record.target.textContent?.slice(0, 40)} ${record.attributeName ?? ''} ${record.type === 'characterData' ? record.target.data : (record.attributeName ? record.target.getAttribute(record.attributeName) : '')}`)).slice(-80);
    });
    observer.observe(layer, { subtree: true, childList: true, attributes: true, characterData: true });
    observer.observe(canvas, { attributeFilter: ['role', 'aria-label', 'aria-owns', 'aria-activedescendant'] });
  }, LAYER);
  const probe = () => page.evaluate(() => ({ ...window.__ariaProbe }));
  // Wait until neither frames nor layer writes have happened for 700 ms.
  async function settle(label) {
    let previous = await probe();
    let quiet = 0;
    for (let sample = 0; sample < 150 && quiet < 7; sample++) {
      await page.waitForTimeout(100);
      const current = await probe();
      quiet = current.submits === previous.submits && current.mutations === previous.mutations ? quiet + 1 : 0;
      previous = current;
    }
    assert.equal(quiet, 7, `${label}: the application settled`);
    return previous;
  }
  const check = (name, details = {}) => report.checks.push({ name, ...details });

  await settle('startup');
  const tree = await accessibilityTree(page);
  await writeFile(resolve(evidence, 'ax-tree.json'), JSON.stringify(tree, null, 2));

  // Structure every hosted application must have.
  const application = tree.find(node => node.role === 'application');
  assert(application, 'the canvas is an application');
  assert.equal(application.name, expectations.application);
  for (const node of tree) {
    if (node.role === 'tab') assert(node.ancestors.includes('tablist'), `tab ${node.name} is inside a tab list`);
    if (node.role === 'option') assert(node.ancestors.includes('listbox'), `option ${node.name} is inside a list box`);
  }
  // Status messages are polite live regions; a readout that changes with the
  // pointer opts out with aria-live="off", which Chromium reports as no live.
  const silent = await page.locator(`${LAYER} [role="status"][aria-live="off"]`).count();
  const statuses = tree.filter(node => node.role === 'status');
  assert.equal(statuses.filter(node => node.properties.live === 'polite').length, statuses.length - silent, 'statuses are polite live regions unless they opt out');
  for (const expected of expectations.required) {
    const found = tree.filter(node => matches(node, expected));
    assert(found.length > 0, `expected ${JSON.stringify(expected)}; similar: ${JSON.stringify(tree.filter(node => node.role === expected.role).slice(0, 8))}`);
    // CDP omits some ARIA attributes, such as set position; read them from
    // the element a Playwright role locator finds.
    if (expected.dom) {
      const element = page.getByRole(expected.role, { name: expected.name, exact: true }).first();
      for (const [name, value] of Object.entries(expected.dom)) {
        assert.equal(await element.getAttribute(name), value, `${expected.role} ${expected.name} ${name}`);
      }
    }
  }
  check('accessibility tree', { nodes: tree.length, required: expectations.required.length });

  await page.addScriptTag({ path: axeSource });
  const axe = await page.evaluate(async () => (await window.axe.run(document, { resultTypes: ['violations'] })).violations);
  await writeFile(resolve(evidence, 'axe-violations.json'), JSON.stringify(axe, null, 2));
  const serious = axe.filter(violation => ['serious', 'critical'].includes(violation.impact));
  assert.deepEqual(serious.map(violation => `${violation.id}: ${violation.nodes.map(node => node.target.join(' ')).join(', ')}`), [], 'no serious axe-core violations');
  check('axe-core', { version: await page.evaluate(() => window.axe.version), violations: axe.map(violation => ({ id: violation.id, impact: violation.impact })) });

  // An idle application presents no frame and writes nothing.
  let before = await probe();
  await page.waitForTimeout(1500);
  let after = await probe();
  assert.deepEqual(after, before, 'idle: no frame and no layer write');
  check('idle', { observed_ms: 1500, ...after });

  // Hovering repaints without changing the tree, so the layer writes nothing.
  const hover = await box(page, expectations.hover);
  await page.mouse.move(hover.x + 2, hover.y);
  await page.mouse.move(hover.x - 2, hover.y);
  const hovered = await settle('hover');
  assert(hovered.submits > after.submits, 'hover presented frames');
  if (hovered.mutations !== after.mutations) console.log(await page.evaluate(() => window.__ariaLog.join('\n')));
  assert.equal(hovered.mutations, after.mutations, 'hover changed no layer element');
  check('hover', { frames: hovered.submits - after.submits, writes: 0 });

  // A click at an element's centre reaches the canvas and the same control;
  // the resulting change writes only what changed.
  if (expectations.click) {
    const elements = await page.locator(`${LAYER} *`).count();
    before = await probe();
    const target = await box(page, expectations.click.target);
    await page.mouse.click(target.x, target.y);
    after = await settle('click');
    const writes = after.mutations - before.mutations;
    const changed = await accessibilityTree(page);
    for (const expected of expectations.click.then) {
      assert(changed.some(node => matches(node, expected)), `after the click, expected ${JSON.stringify(expected)}`);
    }
    assert(writes > 0 && writes < elements, `the click wrote ${writes} changes to a layer of ${elements} elements`);
    check('coordinate click', { target: expectations.click.target, writes, elements });
  }

  // A status change reaches its polite live region.
  if (expectations.status) {
    const { press, then } = expectations.status;
    const status = page.locator(`${LAYER} [role="status"]`).first();
    const text = await status.textContent();
    await page.locator('canvas').focus();
    await page.keyboard.press(press);
    await settle('status');
    const changed = await status.textContent();
    assert.notEqual(changed, text, 'the status changed');
    assert.match(changed, new RegExp(then), 'the status reports the change');
    check('live status', { before: text, after: changed });
  }
  assert.deepEqual(errors, [], 'no page errors');
  report.passed = true;
  console.log(`ARIA layer browser check passed for ${app}: ${report.checks.map(check => check.name).join(', ')}`);
} finally {
  report.errors = errors;
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
  server.close();
}

/** The browser accessibility tree as role, name, value and properties, with ancestor roles. */
async function accessibilityTree(page) {
  const cdp = await page.context().newCDPSession(page);
  const { nodes } = await cdp.send('Accessibility.getFullAXTree');
  await cdp.detach();
  const byId = new Map(nodes.map(node => [node.nodeId, node]));
  const ignoredRoles = new Set(['StaticText', 'InlineTextBox', 'generic', 'none', 'RootWebArea', 'LineBreak']);
  return nodes
    .filter(node => !node.ignored && !ignoredRoles.has(node.role?.value))
    .map(node => {
      const ancestors = [];
      for (let parent = byId.get(node.parentId); parent; parent = byId.get(parent.parentId)) {
        if (!parent.ignored) ancestors.push(parent.role?.value);
      }
      const properties = Object.fromEntries((node.properties ?? [])
        .filter(property => property.value.value !== undefined)
        .map(property => [property.name, property.value.value]));
      return { role: node.role?.value, name: node.name?.value ?? '', value: node.value?.value, properties, ancestors };
    });
}

/** Whether an accessibility node has every expected role, name, value and property. */
function matches(node, expected) {
  return node.role === expected.role
    && (expected.name === undefined || node.name === expected.name)
    && (expected.value === undefined || node.value === expected.value)
    && (expected.parent === undefined || node.ancestors.includes(expected.parent))
    && Object.entries(expected.properties ?? {}).every(([name, value]) => node.properties[name] === (value ?? undefined));
}

/** The centre of the first layer element with `role` and accessible name `name`. */
async function box(page, { role, name }) {
  const bounds = await page.getByRole(role, { name, exact: true }).first().boundingBox();
  assert(bounds && bounds.width > 0 && bounds.height > 0, `${role} ${name} has a box`);
  return { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
}
