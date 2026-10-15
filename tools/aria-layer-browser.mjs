// Check a hosted application's hidden ARIA layer in Chromium: the browser
// accessibility tree against reviewed expectations, an axe-core rule check,
// and that layer writes track accessibility-tree changes rather than frames.
//
// node tools/aria-layer-browser.mjs --app record-desk|gallery|lab
//
// It then activates controls through the layer (a click event, as a screen
// reader's default action sends; `element.click()`; and Playwright's forced
// coordinate click), checks that a plain Playwright click hits an enabled
// control's one-pixel target, and checks focus reporting. Where an application exposes
// its handle, the outcome is read through the application interface, which
// the check enables on the running page only after the structural, idle and
// hover checks, so those see the default page.
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
import { ApplicationClient, BrowserTransport } from './application-client.mjs';

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
    const canvas = document.querySelector(`[aria-owns~="${layer.id}"]`);
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
  // Every enabled clickable control shown on the canvas has a one-pixel hit
  // target at its centre (where Lantern and Playwright click), and only a
  // shown control has one. The pixel is the topmost element there, lies
  // inside every list and region that encloses its control, and over no
  // other control: egui reports a widget's whole rectangle even where a list
  // clips it, so a pixel outside its list would sit over something else. A
  // control whose centre a later (topmost) control's box also covers has
  // none; an earlier one, such as a dock splitter's grab area beneath its
  // panes, does not contest it.
  async function sweepHitTargets(label) {
    const sweep = await page.evaluate(layer => {
      const clickable = ['button', 'tab', 'option', 'radio', 'checkbox', 'switch', 'menuitem', 'combobox', 'link'];
      const clipping = ['listbox', 'list', 'grid', 'tree', 'tabpanel', 'region'];
      const result = { targets: 0, outside: 0, contested: 0, occluded: [], unmarked: [], hidden: [], overlapping: [] };
      const container = document.querySelector(layer);
      const inside = (rect, x, y) => x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
      const targets = [...container.querySelectorAll('[data-stobie-hit]')];
      // Where a later control (egui's topmost) overlaps a control's centre,
      // that control offers no pixel.
      const interactive = [...container.querySelectorAll('[role]')].filter(element => [...clickable, 'separator', 'slider', 'scrollbar'].includes(element.getAttribute('role')));
      const contested = (element, x, y) => interactive.some(other => other !== element && !other.contains(element) && !element.contains(other)
        && (element.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING) && inside(other.getBoundingClientRect(), x, y) && other.getBoundingClientRect().width > 0);
      for (const element of container.querySelectorAll('[role]')) {
        const name = `${element.getAttribute('role')} ${element.getAttribute('aria-label') ?? ''}`;
        const box = element.getBoundingClientRect();
        // Lantern's point: the centre of the viewport-clipped border box.
        const left = Math.max(box.left, 0), right = Math.min(box.right, innerWidth);
        const top = Math.max(box.top, 0), bottom = Math.min(box.bottom, innerHeight);
        const [x, y] = [(left + right) / 2, (top + bottom) / 2];
        let shown = right > left && bottom > top && inside(container.getBoundingClientRect(), x, y);
        for (let ancestor = element.parentElement; shown && ancestor && ancestor !== container; ancestor = ancestor.parentElement) {
          if (clipping.includes(ancestor.getAttribute('role'))) shown = inside(ancestor.getBoundingClientRect(), x, y);
        }
        const marked = element.hasAttribute('data-stobie-hit');
        if (!shown) {
          result.outside += 1;
          if (marked) result.hidden.push(name);
          continue;
        }
        if (!marked) {
          if (contested(element, x, y)) result.contested += 1;
          else if (clickable.includes(element.getAttribute('role')) && element.getAttribute('aria-disabled') !== 'true') result.unmarked.push(name);
          continue;
        }
        result.targets += 1;
        if (!element.contains(document.elementFromPoint(x, y))) result.occluded.push(name);
        const under = targets.find(other => other !== element && !other.contains(element) && !element.contains(other)
          && (element.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING) && inside(other.getBoundingClientRect(), x, y));
        if (under) result.overlapping.push(`${name} over ${under.getAttribute('role')} ${under.getAttribute('aria-label') ?? ''}`);
      }
      return result;
    }, LAYER);
    assert(sweep.targets > 0, `${label}: the layer offers hit targets`);
    assert.deepEqual(sweep.hidden, [], `${label}: no control outside its list or region has a hit target`);
    assert.deepEqual(sweep.occluded, [], `${label}: every hit target is the topmost element at its control's centre`);
    assert.deepEqual(sweep.overlapping, [], `${label}: no hit target lies over another control`);
    assert.deepEqual(sweep.unmarked, [], `${label}: every shown enabled clickable control has a hit target`);
    check(label, { targets: sweep.targets, outside: sweep.outside, contested: sweep.contested });
  }
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
  // An ordinary pointer click needs a hit target at the element's centre:
  // Playwright's actionability check and Lantern's strict targets both test
  // `elementFromPoint` there. Every enabled clickable element has a
  // one-pixel target, which forwards real input to the canvas.
  await sweepHitTargets('hit targets');

  // Real input at a hit target reaches the canvas unchanged: the canvas
  // receives the pointer's moves and never hears that the pointer left.
  {
    await page.evaluate(() => {
      const canvas = document.querySelector('canvas');
      window.__canvasInput = { moves: [], leaves: 0 };
      canvas.addEventListener('pointermove', event => window.__canvasInput.moves.push([event.clientX, event.clientY]));
      canvas.addEventListener('mouseleave', () => { window.__canvasInput.leaves += 1; });
    });
    const centre = await page.evaluate(layer => {
      const element = document.querySelector(`${layer} [data-stobie-hit]`);
      const box = element.querySelector('[data-stobie-hit-target]').getBoundingClientRect();
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    }, LAYER);
    before = await probe();
    await page.mouse.move(centre.x - 3, centre.y);
    await page.mouse.move(centre.x, centre.y);
    await page.mouse.move(centre.x + 3, centre.y);
    after = await settle('pointer over a hit target');
    const input = await page.evaluate(() => window.__canvasInput);
    assert(input.moves.some(([x, y]) => Math.abs(x - centre.x) < 0.5 && Math.abs(y - centre.y) < 0.5), `the canvas received the move onto the hit target: ${JSON.stringify(input.moves)}`);
    assert.equal(input.leaves, 0, 'the canvas never heard the pointer leave');
    assert.equal(after.mutations, before.mutations, 'pointer movement wrote nothing');
    check('pointer at a hit target', { moves: input.moves.length });
  }

  // Activation through the layer takes the same route as a physical click: a
  // disabled control does nothing, and a hidden capability has no element.
  // A plain Playwright click hits the control's target and activates it as a
  // physical click at its centre does.
  for (const step of expectations.plainClick ?? []) {
    const { role, name } = step.target;
    await page.getByRole(role, { name, exact: true }).click({ timeout: 5000 });
    await settle(`plain click ${name}`);
    const changed = await accessibilityTree(page);
    for (const expected of step.then ?? []) {
      assert(changed.some(node => matches(node, expected)), `after a plain click on ${name}, expected ${JSON.stringify(expected)}`);
    }
    check('plain click', { target: step.target });
  }
  let client;
  if (expectations.handle) {
    // Enable automation as the maintained bootstraps do with ?automation=1.
    await page.evaluate(handle => {
      window[handle].enable_automation(crypto.randomUUID());
      window.__STOBIE_AUTOMATION = Object.freeze({ request: json => window[handle].automation_request(json) });
    }, expectations.handle);
    await settle('automation');
    client = new ApplicationClient(new BrowserTransport(page));
    await client.hello();
  }
  const facts = async () => client ? (await client.observe()).facts : {};
  for (const step of expectations.activate ?? []) {
    await page.evaluate(() => { window.__ariaLog = []; });
    before = await probe();
    const element = page.getByRole(step.target.role, { name: step.target.name, exact: true }).first();
    if (step.route === 'event') await element.dispatchEvent('click');
    else if (step.route === 'element') await element.evaluate(target => target.click());
    else if (step.route === 'pointer') await element.click({ force: true });
    else throw new Error(`unknown route ${step.route}`);
    after = await settle(`activate ${step.target.name}`);
    const changed = await accessibilityTree(page);
    for (const expected of step.then ?? []) {
      assert(changed.some(node => matches(node, expected)), `after ${step.route} activation of ${step.target.name}, expected ${JSON.stringify(expected)}`);
    }
    const observed = await facts();
    for (const [key, value] of Object.entries(step.facts ?? {})) {
      assert.deepEqual(observed[key], value, `after ${step.route} activation of ${step.target.name}, fact ${key}`);
    }
    if (step.writes !== undefined && after.mutations - before.mutations !== step.writes) console.log(await page.evaluate(() => window.__ariaLog.join('\n')));
    if (step.writes !== undefined) assert.equal(after.mutations - before.mutations, step.writes, `${step.target.name} layer writes`);
    check(`activate by ${step.route}`, { target: step.target, writes: after.mutations - before.mutations, facts: step.facts });
  }
  for (const hidden of expectations.hidden ?? []) {
    const region = page.getByRole('region', { name: hidden.region, exact: true });
    assert.equal(await region.getByRole(hidden.role, { name: hidden.name, exact: true }).count(), 0, `${hidden.name} is hidden in ${hidden.region}`);
    assert.equal(await page.getByRole('region', { name: hidden.shownIn, exact: true }).getByRole(hidden.role, { name: hidden.name, exact: true }).count(), 1, `${hidden.name} is shown in ${hidden.shownIn}`);
    check('hidden capability', { target: hidden });
  }

  // Keyboard focus stays on the canvas; the canvas names egui's focus.
  if (expectations.focus) {
    const canvas = page.locator('canvas');
    await canvas.focus();
    const named = [];
    for (let press = 0; press < expectations.focus.tabs; press++) {
      await page.keyboard.press('Tab');
      await settle('Tab');
      const state = await page.evaluate(() => {
        const canvas = document.querySelector('canvas');
        const active = document.getElementById(canvas.getAttribute('aria-activedescendant') ?? '');
        return { canvasFocused: document.activeElement === canvas, role: active?.getAttribute('role'), name: active?.getAttribute('aria-label') };
      });
      assert(state.canvasFocused, 'Tab keeps DOM focus on the canvas');
      assert(state.role, 'aria-activedescendant names a layer element');
      const current = `${state.role} ${state.name}`;
      assert.notEqual(current, named.at(-1), 'each Tab moves to another control');
      named.push(current);
    }
    const tree = await accessibilityTree(page);
    assert.equal(tree.find(node => node.role === 'application').properties.focused, true, 'the application keeps focus');
    check('Tab focus', { named });
    // Focus moved onto a layer element, as assistive technology may move it,
    // reaches egui and returns to the application; typing then edits the field.
    if (expectations.focus.edit) {
      const { target, text, fact } = expectations.focus.edit;
      await page.getByRole(target.role, { name: target.name, exact: true }).focus();
      // A focused text field blinks its caret, so wait rather than settle.
      await page.waitForFunction(name => document.activeElement?.getAttribute('aria-label') === name, target.name, { timeout: 5000 });
      const agent = await page.evaluate(() => ({ tag: document.activeElement?.tagName, name: document.activeElement?.getAttribute('aria-label') }));
      assert.deepEqual(agent, { tag: 'INPUT', name: target.name }, 'egui edits text through its named text agent');
      await page.keyboard.press('End');
      await page.keyboard.type(text);
      await page.waitForTimeout(500);
      const observed = await facts();
      assert(String(observed[fact]).endsWith(text), `typing reached ${fact}: ${observed[fact]}`);
      await page.keyboard.press('Escape');
      check('focus handoff and typing', { target, fact: observed[fact] });
    }
  }
  // `locator.fill()` edits a text field through the layer: the field keeps
  // DOM focus, and egui takes its text as typed input. Escape then returns
  // focus to the canvas.
  for (const step of expectations.fill ?? []) {
    const field = page.getByRole(step.target.role, { name: step.target.name, exact: true });
    await field.fill(step.text);
    await page.waitForFunction(name => document.activeElement?.getAttribute('aria-label') === name, step.target.name, { timeout: 5000 });
    await page.waitForTimeout(500);
    assert.equal(await field.inputValue(), step.text, `${step.target.name} keeps the filled text`);
    if (client) {
      const node = (await client.observe()).snapshot.nodes.find(node => node.name === step.target.name);
      assert.equal(node?.value, step.text, `egui took the filled text for ${step.target.name}`);
    }
    const observed = await facts();
    for (const [key, value] of Object.entries(step.facts ?? {})) {
      assert.deepEqual(observed[key], value, `after filling ${step.target.name}, fact ${key}`);
    }
    // An application shortcut pressed in the field still reaches egui.
    if (step.press) {
      await page.keyboard.press(step.press.key);
      await page.waitForTimeout(500);
      const pressed = await facts();
      for (const [key, value] of Object.entries(step.press.facts)) {
        assert.deepEqual(pressed[key], value, `after ${step.press.key} in ${step.target.name}, fact ${key}`);
      }
    }
    await page.keyboard.press('Escape');
    assert(await page.evaluate(() => document.activeElement === document.querySelector('canvas')), 'Escape returns focus to the canvas');
    check('fill', { target: step.target, length: step.text.length, facts: step.facts });
  }
  // After scrolling, rows drawn nowhere take no pointer either.
  if (expectations.scrolledSweep) {
    const { click, wheel } = expectations.scrolledSweep;
    await page.getByRole(click.role, { name: click.name, exact: true }).click();
    await settle('scrolled sweep tab');
    const list = await box(page, wheel);
    await page.mouse.move(list.x, list.y);
    await page.mouse.wheel(0, wheel.dy);
    await settle('scrolled sweep wheel');
    await sweepHitTargets('hit targets after scrolling');
  }
  // Destroying the application removes the layer and restores the canvas.
  if (expectations.handle) {
    await page.evaluate(handle => window[handle].destroy(), expectations.handle);
    assert.equal(await page.locator(LAYER).count(), 0, 'destroy removes the layer');
    const canvas = await page.evaluate(() => ['role', 'aria-owns', 'aria-activedescendant'].map(name => document.querySelector('canvas').getAttribute(name)));
    assert.deepEqual(canvas, [null, null, null], 'destroy restores the canvas');
    check('destroy');
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
