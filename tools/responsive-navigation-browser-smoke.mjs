import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { chromium } from 'playwright';
import { assertBrowserRenderer, hostedLinuxWebGpuLaunchOptions, rendererUrl } from './browser-launch.mjs';
import { observeWarmedIdle } from './browser-idle.mjs';

// Responsive navigation in the Gallery, in headful Chromium with physical
// input: the list, the open conversation and its details as columns on a
// desktop, a split on a tablet and a stack with Back on a phone, read
// through observation and the ARIA layer, across resizes that keep what is
// open.
const root = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/responsive-navigation-browser');
const assets = new Map([
  ['index.html', 'text/html'], ['styles.css', 'text/css'], ['bootstrap.js', 'text/javascript'],
  ['browser-startup.js', 'text/javascript'], ['pkg/stobie_gallery.js', 'text/javascript'],
  ['pkg/stobie_gallery_bg.wasm', 'application/wasm'],
]);
const server = createServer(async (request, response) => {
  const name = new URL(request.url, 'http://local').pathname.slice(1) || 'index.html';
  try {
    assert(assets.has(name), 'allowlisted Gallery asset');
    const body = await readFile(resolve(root, name));
    response.writeHead(200, { 'Content-Type': assets.get(name), 'Cache-Control': 'no-store' });
    response.end(body);
  } catch { response.writeHead(404); response.end(); }
});
await mkdir(evidence, { recursive: true });
const profile = await mkdtemp(join(tmpdir(), 'stobie-responsive-navigation-browser-'));
const steps = [];
const targets = [];
const errors = [];
let context, page, browser, url;
const snapshot = () => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot());
// Nothing open arrives as undefined from the WASM bridge.
const fixture = async () => ({ open: null, ...(await page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot().responsive_fixture)) });
const configuration = (overrides = {}) => ({ appearance: 'dark', contrast: 'standard', density: 'comfortable', font_scale: 1, width: 'desktop', ...overrides });
async function configure(config) {
  const before = await snapshot();
  await page.evaluate(config => window.__STOBIE_GALLERY_HANDLE.set_configuration(config), config);
  await page.waitForFunction(({ config, frame }) => {
    const s = window.__STOBIE_GALLERY_HANDLE.snapshot();
    return s.frame > frame && Object.entries(config).every(([key, value]) => s.configuration[key] === value);
  }, { config, frame: before.frame });
  await page.waitForTimeout(300);
}
async function story(id, config = configuration()) {
  const before = await snapshot();
  await page.evaluate(({ id, config }) => {
    window.__STOBIE_GALLERY_HANDLE.set_configuration(config);
    window.__STOBIE_GALLERY_HANDLE.select_story(id);
  }, { id, config });
  await page.waitForFunction(({ id, config, frame }) => {
    const s = window.__STOBIE_GALLERY_HANDLE.snapshot();
    return s.frame > frame && s.story === id && Object.entries(config).every(([key, value]) => s.configuration[key] === value);
  }, { id, config, frame: before.frame });
  await page.waitForTimeout(300);
}
async function target(name, role) {
  const s = await snapshot();
  const node = s.ui_snapshot.nodes.find(n => n.name === name && n.role === role);
  assert(node, `visible ${role} ${name}`);
  const root = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const canvas = await page.locator('canvas').boundingBox();
  const r = node.rect;
  const point = { x: canvas.x + ((r.min_x + r.max_x) / 2 - root.min_x) * canvas.width / (root.max_x - root.min_x), y: canvas.y + ((r.min_y + r.max_y) / 2 - root.min_y) * canvas.height / (root.max_y - root.min_y) };
  targets.push({ name, role, id: node.id, frame: s.frame, rect: r, point });
  return { node, point };
}
async function click(name, role) {
  const { point } = await target(name, role);
  await page.mouse.click(point.x, point.y);
}
async function record(name) {
  const before = await snapshot();
  const bytes = await page.screenshot({ path: join(evidence, `browser-${name}.png`) });
  const after = await snapshot();
  assert.deepEqual(after.text_audit, [], `${name}: measured text`);
  assert.deepEqual(after.ui_snapshot.semantic_audit, [], `${name}: semantics`);
  assert.equal(after.text_audit_coverage.failed_components, 0);
  steps.push({ name, state: after, before_capture_frame: before.frame, image: { path: `browser-${name}.png`, sha256: createHash('sha256').update(bytes).digest('hex') } });
  return after;
}
// The recipe's panes: regions inside the Gallery's story pane.
const panes = s => s.ui_snapshot.nodes.filter(n => n.role === 'pane' && n.id !== 'gallery.story' && n.parent === 'gallery.story').map(n => n.name);
const pane = (s, name) => s.ui_snapshot.nodes.find(n => n.role === 'pane' && n.name === name);
const rows = s => s.ui_snapshot.nodes.filter(n => n.role === 'navigation_item');
const button = (s, name) => s.ui_snapshot.nodes.find(n => n.role === 'button' && n.name === name);
const layer = () => page.locator('[id^="stobie-a11y:layer"]');
const control = (role, name) => layer().getByRole(role, { name, exact: true });
const waitFor = predicate => page.waitForFunction(predicate, null, { timeout: 10000 });
async function settle(expected) {
  await page.waitForFunction(expected => {
    const s = window.__STOBIE_GALLERY_HANDLE.snapshot();
    const names = s.ui_snapshot.nodes.filter(n => n.role === 'pane' && n.parent === 'gallery.story').map(n => n.name);
    return JSON.stringify(names) === JSON.stringify(expected);
  }, expected, { timeout: 10000 });
}

try {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  url = `http://127.0.0.1:${server.address().port}/`;
  context = await chromium.launchPersistentContext(profile, {
    ...hostedLinuxWebGpuLaunchOptions(), viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1,
    executablePath: process.env.STOBIE_CHROMIUM,
    env: { ...process.env, LD_LIBRARY_PATH: process.env.STOBIE_USE_SYSTEM_UI_LIBS === '1' ? process.env.LD_LIBRARY_PATH ?? '' : `${resolve('.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}` },
  });
  browser = context.browser(); page = await context.newPage();
  page.on('pageerror', error => errors.push(String(error)));
  const response = await page.goto(rendererUrl(url), { waitUntil: 'domcontentloaded' });
  assert(response?.ok(), `Gallery HTTP status ${response?.status()}`);
  await page.waitForFunction(() => document.body.classList.contains('ready') && window.__STOBIE_GALLERY_HANDLE?.snapshot().frame > 0);
  await assertBrowserRenderer(page);
  const history = await page.evaluate(() => history.length);

  // Desktop, dark: the list, the conversation and its details as columns.
  await story('navigation/responsive');
  let s = await record('desktop-columns');
  assert.equal((await fixture()).layout, 'columns');
  assert.deepEqual(panes(s), ['Conversations', 'Ada Lovelace', 'About Ada Lovelace']);
  assert.deepEqual(pane(s, 'Ada Lovelace').domain_reference, { kind: 'external', value: { namespace: 'gallery.conversation', id: 'ada' } });
  assert.deepEqual(rows(s).map(n => [n.name, n.selected]), [
    ['Ada Lovelace', true], ['Grace Hopper', false], ['Edsger Dijkstra', false], ['Katherine Johnson', false],
  ]);
  assert.equal(button(s, 'Back'), undefined, 'columns have no Back');
  assert.equal(button(s, 'Details').checked, true, 'the details are shown');
  // Each pane's controls belong to it.
  assert(rows(s).every(n => n.parent === pane(s, 'Conversations').id), 'rows belong to the list');
  assert.equal(button(s, 'Details').parent, pane(s, 'Ada Lovelace').id, 'the toggle is in the conversation\'s header');
  // The ARIA layer: each pane is a named region, the open row is current
  // and the toggle states whether the details show.
  await layer().locator('[role]').first().waitFor({ state: 'attached', timeout: 60000 });
  for (const name of ['Conversations', 'Ada Lovelace', 'About Ada Lovelace']) {
    assert.equal(await control('region', name).count(), 1, `region ${name}`);
  }
  assert.equal(await control('region', 'Conversations').getByRole('button', { name: 'Grace Hopper', exact: true }).count(), 1, 'rows are inside the list region');
  assert.equal(await control('button', 'Ada Lovelace').getAttribute('aria-current'), 'true');
  assert.equal(await control('button', 'Grace Hopper').getAttribute('aria-current'), 'false');
  assert.equal(await control('button', 'Details').getAttribute('aria-pressed'), 'true');

  // A pointer opens another conversation; the details follow it.
  await click('Grace Hopper', 'navigation_item');
  await settle(['Conversations', 'Grace Hopper', 'About Grace Hopper']);
  s = await record('desktop-opened');
  assert.deepEqual(rows(s).map(n => n.selected), [false, true, false, false]);
  await waitFor(() => document.querySelector('[id^="stobie-a11y:layer"] [aria-label="Grace Hopper"]')?.getAttribute('aria-current') === 'true');
  // The toggle hides the details.
  await click('Details', 'button');
  await settle(['Conversations', 'Grace Hopper']);
  s = await record('desktop-details-hidden');
  assert.equal(button(s, 'Details').checked, false);
  await waitFor(() => document.querySelector('[id^="stobie-a11y:layer"] [aria-label="Details"]')?.getAttribute('aria-pressed') === 'false');
  assert.deepEqual(await fixture(), { open: 'grace', details: false, layout: 'columns', opened: 1, backs: 0, toggles: 1 });

  // A tablet's width: the same conversation, split beside the list.
  await configure(configuration({ width: 'wide' }));
  await settle(['Conversations', 'Grace Hopper']);
  s = await record('tablet-split');
  assert.equal((await fixture()).layout, 'split');
  assert.equal(button(s, 'Back'), undefined, 'the list is in view, so no Back');
  // The details replace the conversation beside the list, with Back.
  await click('Details', 'button');
  await settle(['Conversations', 'About Grace Hopper']);
  s = await record('tablet-details');
  assert(button(s, 'Back'), 'the details have Back');
  await click('Back', 'button');
  await settle(['Conversations', 'Grace Hopper']);
  assert.deepEqual(await fixture(), { open: 'grace', details: false, layout: 'split', opened: 1, backs: 1, toggles: 2 });

  // A phone's width: one level at a time, with Back.
  await configure(configuration({ width: 'narrow' }));
  await settle(['Grace Hopper']);
  s = await record('phone-conversation');
  assert.equal((await fixture()).layout, 'stack');
  assert(button(s, 'Back') && button(s, 'Details'), 'the header has Back and the toggle');
  assert.equal(await control('button', 'Back').count(), 1);
  await click('Back', 'button');
  await settle(['Conversations']);
  s = await record('phone-list');
  assert.equal(rows(s).length, 4, 'the list is the root');
  // Opening a row pushes its conversation; the keyboard goes back.
  await click('Edsger Dijkstra', 'navigation_item');
  await settle(['Edsger Dijkstra']);
  // Focus follows the reader: the covered row's focus moves to Back.
  await waitFor(() => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes.find(n => n.focused)?.name === 'Back');
  await record('phone-back-focused');
  await page.keyboard.press('Enter');
  await settle(['Conversations']);
  // Back returns focus to the row that opened the conversation.
  await waitFor(() => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes.find(n => n.focused)?.name === 'Edsger Dijkstra');
  s = await record('phone-back-returned-focus');
  assert.deepEqual(rows(s).map(n => n.selected), [false, false, false, false], 'nothing is open');
  // Back to a desktop's width: the list and a prompt, nothing lost.
  await configure(configuration());
  await settle(['Conversations']);
  s = await record('desktop-nothing-open');
  assert(s.text.some(t => t.content?.text === 'Choose a conversation'), 'the empty content asks for a choice');
  assert.deepEqual({ ...(await fixture()), open: (await fixture()).open ?? null }, { open: null, details: false, layout: 'columns', opened: 2, backs: 3, toggles: 2 });

  // Light compact at a tablet's width, from the story's own fixture:
  // selecting another story and back resets it.
  await story('navigation/responsive-narrow', configuration({ width: 'narrow' }));
  await story('navigation/responsive', configuration({ appearance: 'light', density: 'compact', width: 'wide' }));
  s = await record('tablet-light-compact');
  assert.deepEqual(panes(s), ['Conversations', 'About Ada Lovelace']);

  // A phone, light high contrast at 150%: the root is a list of
  // navigation items with counts; a long title elides beside Back.
  await story('navigation/responsive-narrow', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'narrow' }));
  await settle(['Conversations and shared assistants']);
  s = await record('phone-long-list');
  assert(s.text.some(t => t.truncated), 'long destinations elide');
  await click('Weekend planner for the walking group', 'navigation_item');
  await settle(['Weekend planner for the walking group']);
  s = await record('phone-long-title');
  assert(s.text.some(t => t.truncated), 'the long title elides beside Back');
  assert.equal(await control('region', 'Weekend planner for the walking group').count(), 1, 'the region carries the whole title');
  await click('Back', 'button');
  await settle(['Conversations and shared assistants']);
  s = await snapshot();
  const archived = rows(s).find(n => n.name === 'Katherine Coleman Goble Johnson');
  assert(archived && archived.enabled === false && archived.disabled_reason === 'This conversation is archived', 'the archived row is disabled with its reason');
  assert(rows(s).some(n => n.name === 'Augusta Ada King, Countess of Lovelace'), 'whole names in semantics');
  const first = await target(rows(s)[0].name, 'navigation_item');
  await page.mouse.move(first.point.x, first.point.y);
  for (let i = 0; i < 10; i += 1) {
    await page.mouse.wheel(0, 200);
    await page.waitForTimeout(80);
  }
  await page.waitForTimeout(300);
  s = await record('phone-long-scrolled');
  assert(rows(s).some(n => n.name === 'Radia Joy Perlman'), 'the list scrolled to its end');
  assert.notEqual(rows(s)[0].name, 'Augusta Ada King, Countess of Lovelace', 'the first rows scrolled out of view and observation');
  await click('Radia Joy Perlman', 'navigation_item');
  await settle(['Radia Joy Perlman']);
  await click('Back', 'button');
  await settle(['Conversations and shared assistants']);
  s = await record('phone-long-returned');
  assert(rows(s).some(n => n.name === 'Radia Joy Perlman'), 'the list kept its scroll position');
  assert.notEqual(rows(s)[0].name, 'Augusta Ada King, Countess of Lovelace', 'the list did not return to its top');

  // Navigation pushes no browser history: the in-app Back is the route.
  assert.equal(await page.evaluate(() => history.length), history, 'no history entries');

  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, []);
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/stobie_gallery_bg.wasm'))).digest('hex'), url, browser: browser.version(), steps, targets, idle, input_route: 'Physical Playwright mouse/keyboard; hooks only choose stories, widths and appearance and read observations', errors };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Responsive navigation browser workflow passed: columns, split and stack, Back, the details toggle, selection, resizes keeping state, ARIA regions, scrolling, no history entries and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), state: await snapshot().catch(() => null), steps, targets }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await context?.close();
  if (server.listening) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
