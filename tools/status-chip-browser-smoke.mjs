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

const root = resolve('apps/polyorama-gallery/web');
const evidence = resolve(process.env.POLYORAMA_EVIDENCE_DIR ?? '.tools/runtime/status-chip-browser');
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
const profile = await mkdtemp(join(tmpdir(), 'polyorama-status-chip-browser-'));
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
async function target(name, role = 'status_chip', domain = null) {
  const s = await snapshot();
  const node = s.ui_snapshot.nodes.find(n => n.name === name && n.role === role && (domain === null || n.domain_reference?.value?.id === domain));
  assert(node, `visible ${role} ${name}`);
  const root = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const canvas = await page.locator('canvas').boundingBox();
  const r = node.rect;
  const point = { x: canvas.x + ((r.min_x + r.max_x) / 2 - root.min_x) * canvas.width / (root.max_x - root.min_x), y: canvas.y + ((r.min_y + r.max_y) / 2 - root.min_y) * canvas.height / (root.max_y - root.min_y) };
  const x = value => canvas.x + (value - root.min_x) * canvas.width / (root.max_x - root.min_x);
  targets.push({ name, role, id: node.id, frame: s.frame, rect: r, point });
  return { node, point, start: { x: x(r.min_x + 0.5), y: point.y }, end: { x: x(r.max_x - 0.5), y: point.y } };
}
async function record(name) {
  const before = await snapshot();
  const bytes = await page.screenshot({ path: join(evidence, `browser-${name}.png`) });
  const after = await snapshot();
  assert.deepEqual(after.text_audit, [], `${name}: measured text`);
  assert.deepEqual(after.ui_snapshot.semantic_audit, [], `${name}: semantics`);
  assert.equal(after.text_audit_coverage.failed_components, 0);
  const chips = after.ui_snapshot.nodes.filter(n => n.role === 'status_chip');
  assert(chips.length > 0); assert(chips.every(n => n.actions.length === 0 && !n.selected && !n.checked));
  steps.push({ name, state: after, before_capture_frame: before.frame, image: { path: `browser-${name}.png`, sha256: createHash('sha256').update(bytes).digest('hex') } });
  return after;
}
async function dragCopy(label, expected) {
  const t = await target(label);
  await page.mouse.move(t.start.x, t.start.y); await page.mouse.down();
  await page.mouse.move(t.end.x, t.end.y, { steps: 8 }); await page.mouse.up();
  await page.keyboard.press('Control+c');
  await page.waitForFunction(expected => navigator.clipboard.readText().then(text => text === expected), expected);
  return t;
}
try {
  if (process.env.POLYORAMA_STATUS_CHIP_PREVIEW_URL) {
    const preview = new URL(process.env.POLYORAMA_STATUS_CHIP_PREVIEW_URL);
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
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  page.on('pageerror', error => errors.push(String(error)));
  const response = await page.goto(url, { waitUntil: 'domcontentloaded' });
  assert(response?.ok(), `Gallery HTTP status ${response?.status()}`);
  await page.waitForFunction(() => document.body.classList.contains('ready') && window.__POLYORAMA_GALLERY_HANDLE?.snapshot().frame > 0);
  await story('status-chip/tasks');
  await record('tasks-dark');
  const original = await target('Active');
  await dragCopy('Active', 'Active'); await record('selectable');
  const inert = await target('Scheduled', 'status_chip', '2');
  assert.equal(inert.node.text_selectable ?? false, false);
  await page.mouse.move(inert.point.x, inert.point.y); await record('row-hover');
  await page.mouse.click(inert.point.x, inert.point.y);
  await page.waitForFunction(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot().status_chip_fixture.selected_task === 2);
  const selected = await record('row-selected');
  assert.equal(selected.status_chip_fixture.activations, 1);
  assert((await target('Survey northern site', 'result_row')).node.focused);
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot().status_chip_fixture.activations === 2);
  await record('parent-keyboard');
  const firstTask = await target('Active', 'status_chip', '1');
  await page.mouse.click(firstTask.point.x, firstTask.point.y);
  await page.waitForFunction(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot().status_chip_fixture.selected_task === 1);
  const complete = await target('Complete demo task', 'button');
  await page.mouse.click(complete.point.x, complete.point.y);
  await page.waitForFunction(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot().status_chip_fixture.completed);
  const current = await record('updated');
  assert(!current.ui_snapshot.nodes.some(n => n.role === 'status_chip' && n.name === 'Active'));
  assert.equal((await target('Completed')).node.id, original.node.id);
  for (const [name, config] of [
    ['light-compact', configuration({ appearance: 'light', density: 'compact' })],
    ['dark-high-contrast', configuration({ contrast: 'high' })],
    ['light-high-contrast-150', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'wide' })],
  ]) {
    await story('status-chip/treatments', config); const treatments = await record(name);
    assert.equal(treatments.ui_snapshot.nodes.filter(n => n.role === 'status_chip').length, 18, 'all three parent surfaces visible');
  }
  await story('status-chip/long-narrow', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'narrow' }));
  const localised = 'Überprüfung durch das regionale Forschungsteam ausstehend';
  assert((await snapshot()).text.some(t => t.component_id.kind === 'status_chip' && t.truncated));
  await dragCopy(localised, localised); await record('long-copy');
  const scheduled = await target('Scheduled');
  const beforeScroll = await snapshot();
  await page.mouse.move(scheduled.point.x, scheduled.point.y); await page.mouse.wheel(0, 65);
  await page.waitForTimeout(200);
  const scrolled = await record('scrolled');
  const firstIds = beforeScroll.ui_snapshot.nodes.filter(n => n.role === 'status_chip' && n.name === 'Scheduled').map(n => n.id);
  const lastIds = scrolled.ui_snapshot.nodes.filter(n => n.role === 'status_chip' && n.name === 'Scheduled').map(n => n.id);
  assert.notDeepEqual(lastIds, firstIds, 'scroll changes current visible observations');
  assert(scrolled.text_audit_coverage.attempted_components > scrolled.text_audit_coverage.measured_components, 'clipped submissions retain denominator');
  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, []);
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/polyorama_gallery_bg.wasm'))).digest('hex'), url, browser: browser.version(), steps, targets, idle, input_route: 'Physical Playwright mouse/keyboard; hooks only choose stories/appearance and read observations', errors };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Status-chip browser workflow passed: full copy, inert parent activation, updates, appearances, scrolling and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), state: await snapshot().catch(() => null), steps, targets }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await context?.close();
  if (server.listening) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
