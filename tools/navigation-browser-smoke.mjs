import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions } from './browser-launch.mjs';
import { observeWarmedIdle } from './browser-idle.mjs';
import { focusNavigationDestination } from './navigation-browser-focus.mjs';

const root = resolve('apps/polyorama-gallery/web');
const evidence = resolve(process.env.POLYORAMA_EVIDENCE_DIR ?? '.tools/runtime/navigation-browser');
const assets = new Map([
  ['index.html', 'text/html'], ['styles.css', 'text/css'], ['bootstrap.js', 'text/javascript'],
  ['browser-startup.js', 'text/javascript'], ['pkg/polyorama_gallery.js', 'text/javascript'],
  ['pkg/polyorama_gallery_bg.wasm', 'application/wasm'],
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
const profile = await mkdtemp(join(tmpdir(), 'polyorama-navigation-browser-'));
const steps = [];
const targets = [];
const errors = [];
let context, page, browser, url;
const snapshot = () => page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot());
const configuration = (overrides = {}) => ({ appearance: 'dark', contrast: 'standard', density: 'comfortable', font_scale: 1, width: 'regular', ...overrides });
async function story(id, config = configuration()) {
  const before = await snapshot();
  await page.evaluate(({ id, config }) => {
    window.__POLYORAMA_GALLERY_HANDLE.set_configuration(config);
    window.__POLYORAMA_GALLERY_HANDLE.select_story(id);
  }, { id, config });
  await page.waitForFunction(({ id, config, frame }) => {
    const s = window.__POLYORAMA_GALLERY_HANDLE.snapshot();
    return s.frame > frame && s.story === id && Object.entries(config).every(([key, value]) => s.configuration[key] === value);
  }, { id, config, frame: before.frame });
}
async function fixture(expected) {
  await page.waitForFunction(expected => {
    const fixture = window.__POLYORAMA_GALLERY_HANDLE.snapshot().navigation_fixture;
    return Object.entries(expected).every(([key, value]) => (fixture[key] ?? null) === value);
  }, expected, { timeout: 10000 });
}
async function target(destination) {
  const s = await snapshot();
  const id = destination === 'complete_task' ? s.navigation_fixture.count_target : s.navigation_fixture.targets.find(t => t.destination === destination).id;
  const node = s.ui_snapshot.nodes.find(node => node.id === id);
  assert(node, `visible destination ${destination}`);
  const root = s.ui_snapshot.nodes.find(node => node.id === s.ui_snapshot.root).rect;
  const canvas = await page.locator('canvas').boundingBox();
  const r = node.rect;
  const point = { x: canvas.x + ((r.min_x + r.max_x) / 2 - root.min_x) * canvas.width / (root.max_x - root.min_x), y: canvas.y + ((r.min_y + r.max_y) / 2 - root.min_y) * canvas.height / (root.max_y - root.min_y) };
  targets.push({ destination, id, frame: s.frame, rect: r, point });
  return { node, point };
}
async function click(destination) { const { point } = await target(destination); await page.mouse.click(point.x, point.y); }
async function focus(destination) {
  return focusNavigationDestination(destination, {
    target,
    pressTab: () => page.keyboard.press('Tab'),
    wait: ms => page.waitForTimeout(ms),
  });
}
async function record(name, expected = {}) {
  await fixture(expected);
  await page.waitForTimeout(100);
  const before = await snapshot();
  const bytes = await page.screenshot({ path: join(evidence, `browser-${name}.png`) });
  const after = await snapshot();
  assert.deepEqual(after.text_audit, [], `${name}: measured text`);
  assert.deepEqual(after.ui_snapshot.semantic_audit, [], `${name}: semantics`);
  assert(after.text_audit_coverage?.failed_components === 0, `${name}: measured failures`);
  for (const s of [before, after]) for (const [key, value] of Object.entries(expected)) assert.equal(s.navigation_fixture[key] ?? null, value, `${name}: held ${key}`);
  steps.push({ name, state: after, before_capture_frame: before.frame, image: { path: `browser-${name}.png`, sha256: createHash('sha256').update(bytes).digest('hex') } });
  return after;
}
try {
  if (process.env.POLYORAMA_NAVIGATION_PREVIEW_URL) {
    const preview = new URL(process.env.POLYORAMA_NAVIGATION_PREVIEW_URL);
    assert(preview.protocol === 'https:' && preview.pathname === '/gallery/' && !preview.username && !preview.password && !preview.search && !preview.hash);
    url = preview.href;
  } else {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    url = `http://127.0.0.1:${server.address().port}/`;
  }
  context = await chromium.launchPersistentContext(profile, {
    ...hostedLinuxWebGpuLaunchOptions(), viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1,
    executablePath: process.env.POLYORAMA_CHROMIUM,
    env: { ...process.env, LD_LIBRARY_PATH: process.env.POLYORAMA_USE_SYSTEM_UI_LIBS === '1' ? process.env.LD_LIBRARY_PATH ?? '' : `${resolve('.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}` },
  });
  browser = context.browser(); page = await context.newPage();
  page.on('pageerror', error => errors.push(String(error)));
  const response = await page.goto(url, { waitUntil: 'domcontentloaded' });
  assert(response?.ok(), `Gallery HTTP status ${response?.status()}`);
  await page.waitForFunction(() => document.body.classList.contains('ready') && window.__POLYORAMA_GALLERY_HANDLE?.snapshot().frame > 0);
  await story('navigation/sidebar');
  await record('sidebar', { selected: 'home', activations: 0 });
  await focus('tasks');
  const focusState = await record('focus', { selected: 'home', activations: 0 });
  assert((await target('tasks')).node.focused);
  assert(focusState.ui_snapshot.nodes.find(n => n.name === 'Home' && n.role === 'navigation_item').selected);
  await page.keyboard.press('Enter');
  await record('enter', { selected: 'tasks', activations: 1, task_count: 12 });
  await page.keyboard.press('Space');
  await record('space', { selected: 'tasks', activations: 2, task_count: 12 });
  const tasks = await target('tasks');
  await page.mouse.move(tasks.point.x, tasks.point.y);
  await record('selected-hover', { selected: 'tasks', hovered: 'tasks', pointer_down: null });
  await page.mouse.down();
  await record('selected-pressed', { selected: 'tasks', hovered: 'tasks', pointer_down: 'tasks' });
  await page.mouse.up(); await fixture({ activations: 3, task_count: 12 });
  assert.equal((await target('tasks')).node.id, tasks.node.id, 'count changes preserve row identity');
  assert((await target('tasks')).node.focused, 'count changes preserve focus');
  await click('complete_task');
  await record('count-update', { selected: 'tasks', activations: 3, task_count: 11 });
  assert.equal((await target('tasks')).node.id, tasks.node.id, 'independent count update preserves row identity');
  await click('needs_attention');
  await record('destination-change', { selected: 'needs_attention', activations: 4 });
  assert((await snapshot()).text.some(t => t.component_id.kind === 'section_heading'), 'destination content heading');
  await story('navigation/states');
  assert.equal((await target('settings')).node.enabled, false);
  await click('settings');
  await record('disabled-zero', { selected: 'home', activations: 0 });
  assert((await target('activity')).node.description.includes('0 new events'));
  const unselected = await target('tasks');
  await page.mouse.move(unselected.point.x, unselected.point.y);
  await record('unselected-hover', { selected: 'home', hovered: 'tasks', activations: 0 });
  await page.mouse.down();
  await record('unselected-pressed', { selected: 'home', hovered: 'tasks', pointer_down: 'tasks', activations: 0 });
  await page.mouse.up(); await fixture({ selected: 'tasks', activations: 1, task_count: 12 });
  for (const [name, config] of [
    ['light-compact', configuration({ appearance: 'light', density: 'compact' })],
    ['dark-high-contrast', configuration({ contrast: 'high' })],
    ['light-high-contrast-150', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'narrow' })],
  ]) {
    await story('navigation/long-narrow', config);
    const current = await snapshot();
    const visibleRow = current.ui_snapshot.nodes.find(n => n.role === 'navigation_item');
    const visibleDestination = current.navigation_fixture.targets.find(t => t.id === visibleRow.id).destination;
    const visibleTarget = await target(visibleDestination);
    await page.mouse.move(visibleTarget.point.x, visibleTarget.point.y);
    await page.mouse.wheel(0, -600); await page.waitForTimeout(150);
    await record(name);
    const s = await snapshot();
    const task = s.ui_snapshot.nodes.find(n => n.actions.includes('gallery.navigation.tasks'));
    assert(task?.name.includes('entire regional operations team'));
    assert(task.description.includes('12500 outstanding tasks'));
    assert(s.text.some(t => t.component_id.kind === 'navigation_label' && t.truncated));
    const visible = s.ui_snapshot.nodes.filter(n => n.role === 'navigation_item');
    assert(visible.length < 5, 'fully clipped rows omitted');
    const { point } = await target('tasks');
    await page.mouse.move(point.x, point.y); await page.mouse.wheel(0, 200);
    await page.waitForTimeout(150);
    await record(`${name}-scrolled`);
    const settings = await target('settings');
    assert.equal(settings.node.enabled, false);
    assert((await snapshot()).text.some(t => t.component_id.kind === 'navigation_badge' && t.truncated), 'long badge elides');
  }
  await story('navigation/sidebar');
  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, []);
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/polyorama_gallery_bg.wasm'))).digest('hex'), url, browser: browser.version(), steps, targets, idle, input_route: 'Physical Playwright mouse/keyboard; hooks only choose stories/appearance and read observations', errors };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Navigation browser workflow passed: pointer, keyboard, disabled, caller-owned selection/count, appearances, clipping and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), state: await snapshot().catch(() => null), steps, targets }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await context?.close();
  if (server.listening) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
