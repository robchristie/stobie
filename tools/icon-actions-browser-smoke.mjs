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
import { focusByTab } from './browser-keyboard-focus.mjs';

const root = resolve('apps/polyorama-gallery/web');
const evidence = resolve(process.env.POLYORAMA_EVIDENCE_DIR ?? '.tools/runtime/icons-browser');
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
const profile = await mkdtemp(join(tmpdir(), 'polyorama-icons-browser-'));
const steps = [];
const targets = [];
const errors = [];
let context;
let page;
let browser;
let url;
let serverStarted = false;
let pressedComparison;
const snapshot = () => page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot());
const configuration = (overrides = {}) => ({ appearance: 'dark', contrast: 'standard', density: 'comfortable', font_scale: 1, width: 'regular', ...overrides });

async function ready() {
  await page.waitForFunction(() => document.body.classList.contains('ready')
    && window.__POLYORAMA_GALLERY_HANDLE?.snapshot().frame > 0, null, { timeout: 30_000 });
}
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
  await page.waitForTimeout(100);
}
async function waitFixture(expected) {
  await page.waitForFunction((expected) => {
    const fixture = window.__POLYORAMA_GALLERY_HANDLE.snapshot().icon_fixture;
    return Object.entries(expected).every(([key, value]) => key.startsWith('count:')
      ? fixture.activations[key.slice(6)] === value : (fixture[key] ?? null) === value);
  }, expected, { timeout: 10_000 });
}
async function control(control) {
  const s = await snapshot();
  const node = s.ui_snapshot.nodes.find(node => node.id === s.icon_fixture.targets[control]);
  assert(node, `current semantic target for ${control}`);
  const rootNode = s.ui_snapshot.nodes.find(node => node.id === s.ui_snapshot.root);
  const canvas = await page.locator('canvas').boundingBox();
  const r = node.rect;
  const rr = rootNode.rect;
  assert(r.max_x > r.min_x && r.max_y > r.min_y, `positive target for ${control}`);
  const point = {
    x: canvas.x + ((r.min_x + r.max_x) / 2 - rr.min_x) * canvas.width / (rr.max_x - rr.min_x),
    y: canvas.y + ((r.min_y + r.max_y) / 2 - rr.min_y) * canvas.height / (rr.max_y - rr.min_y),
  };
  targets.push({ control, node_id: node.id, frame: s.frame, rect: r, point });
  return { node, point, tab_input_epoch: s.tab_input_epoch };
}
async function click(controlId) {
  const { point } = await control(controlId);
  await page.mouse.click(point.x, point.y);
}
async function focus(controlId) {
  return focusByTab(controlId, {
    target: control,
    pressTab: () => page.keyboard.press('Tab'),
    waitForTab: epoch => page.waitForFunction(epoch => window.__POLYORAMA_GALLERY_HANDLE.snapshot().tab_input_epoch > epoch, epoch, { timeout: 15000 }),
    wait: ms => page.waitForTimeout(ms),
    maxTabs: 40,
  });
}

async function record(name, capture = true, expectedFixture = {}, expectedFocusedControl) {
  let s = await snapshot();
  assert.deepEqual(s.text_audit, [], `${name}: text audit`);
  assert.deepEqual(s.ui_snapshot.semantic_audit, [], `${name}: semantic audit`);
  assert(s.text_audit_coverage, `${name}: text coverage`);
  let image;
  let captureBracket;
  if (capture) {
    const beforeSettle = s;
    const assertFixture = (state) => {
      for (const [key, expected] of Object.entries(expectedFixture)) {
        assert.equal(state.icon_fixture[key] ?? null, expected, `${name}: retained ${key} during capture`);
      }
      if (expectedFocusedControl) {
        assert(state.ui_snapshot.nodes.some(node => node.id === state.icon_fixture.targets[expectedFocusedControl] && node.focused),
          `${name}: retained ${expectedFocusedControl} focus during capture`);
      }
    };
    assertFixture(beforeSettle);
    // The hook publishes during egui's UI pass, before the GPU compositor has
    // necessarily presented it. Hold the real input state while rendering settles.
    await page.waitForTimeout(100);
    const beforeCapture = await snapshot();
    assertFixture(beforeCapture);
    assert(beforeCapture.frame >= beforeSettle.frame, `${name}: monotonic capture frame`);
    const png = await page.screenshot({ path: resolve(evidence, `browser-${name}.png`) });
    s = await snapshot();
    assertFixture(s);
    assert(s.frame >= beforeCapture.frame, `${name}: monotonic captured frame`);
    assert.deepEqual(s.text_audit, [], `${name}: captured text audit`);
    assert.deepEqual(s.ui_snapshot.semantic_audit, [], `${name}: captured semantic audit`);
    captureBracket = { settle_ms: 100, before_settle: beforeSettle, before_capture: beforeCapture, after_capture: s, expected_fixture: expectedFixture, expected_focused_control: expectedFocusedControl };
    const pixels = await page.evaluate(async (base64) => {
      const bytes = Uint8Array.from(atob(base64), character => character.charCodeAt(0));
      const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
      const canvas = document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      context.drawImage(bitmap, 0, 0);
      const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
      let minimum = 255;
      let maximum = 0;
      for (let index = 0; index < pixels.length; index += 4) {
        minimum = Math.min(minimum, pixels[index], pixels[index + 1], pixels[index + 2]);
        maximum = Math.max(maximum, pixels[index], pixels[index + 1], pixels[index + 2]);
      }
      return { minimum, maximum };
    }, png.toString('base64'));
    assert(pixels.maximum > 0 && pixels.minimum !== pixels.maximum,
      `${name}: screenshot has no rendered pixels; use the configured POLYORAMA_BROWSER_HEADFUL=1 DISPLAY route`);
    image = { path: `browser-${name}.png`, width: png.readUInt32BE(16), height: png.readUInt32BE(20), sha256: createHash('sha256').update(png).digest('hex'), pixels };
  }
  steps.push({ name, state: s, device_scale_factor: await page.evaluate(() => devicePixelRatio), image, capture_bracket: captureBracket });
  return s;
}
async function comparePressedControl() {
  const hover = steps.find(step => step.name === 'hover');
  const pressed = steps.find(step => step.name === 'pressed');
  const target = (step) => step.state.ui_snapshot.nodes.find(node => node.id === step.state.icon_fixture.targets.icon);
  const hoverTarget = target(hover);
  const pressedTarget = target(pressed);
  assert.equal(hoverTarget.id, pressedTarget.id, 'hover/press retain logical instance');
  assert.deepEqual(hoverTarget.rect, pressedTarget.rect, 'hover/press retain target geometry');
  const rootNode = pressed.state.ui_snapshot.nodes.find(node => node.id === pressed.state.ui_snapshot.root);
  const rr = rootNode.rect;
  const r = pressedTarget.rect;
  const canvas = await page.locator('canvas').boundingBox();
  const scale = pressed.device_scale_factor;
  const pixelRect = {
    x: Math.floor((canvas.x + (r.min_x - rr.min_x) * canvas.width / (rr.max_x - rr.min_x)) * scale),
    y: Math.floor((canvas.y + (r.min_y - rr.min_y) * canvas.height / (rr.max_y - rr.min_y)) * scale),
    width: Math.ceil((r.max_x - r.min_x) * canvas.width / (rr.max_x - rr.min_x) * scale),
    height: Math.ceil((r.max_y - r.min_y) * canvas.height / (rr.max_y - rr.min_y) * scale),
  };
  const [left, right] = await Promise.all([readFile(resolve(evidence, hover.image.path)), readFile(resolve(evidence, pressed.image.path))]);
  const result = await page.evaluate(async ({ left, right, rect }) => {
    const pixels = async (base64) => {
      const bytes = Uint8Array.from(atob(base64), character => character.charCodeAt(0));
      const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
      const canvas = document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      context.drawImage(bitmap, 0, 0);
      return context.getImageData(rect.x, rect.y, rect.width, rect.height).data;
    };
    const [a, b] = await Promise.all([pixels(left), pixels(right)]);
    let changed = 0;
    let maximumDifference = 0;
    for (let index = 0; index < a.length; index += 4) {
      let difference = 0;
      for (let channel = 0; channel < 4; channel++) difference = Math.max(difference, Math.abs(a[index + channel] - b[index + channel]));
      if (difference > 0) changed++;
      maximumDifference = Math.max(maximumDifference, difference);
    }
    return { changed_pixels: changed, maximum_channel_difference: maximumDifference, total_pixels: rect.width * rect.height };
  }, { left: left.toString('base64'), right: right.toString('base64'), rect: pixelRect });
  assert.notEqual(hover.image.sha256, pressed.image.sha256, 'held pointer press visibly differs from hover');
  assert(result.changed_pixels > 0, 'the current Fit view target visibly changes on held press');
  return { control: 'icon', node_id: pressedTarget.id, ui_rect: r, pixel_rect: pixelRect, hover_sha256: hover.image.sha256, pressed_sha256: pressed.image.sha256, ...result };
}
function observeErrors(page) {
  page.on('pageerror', error => errors.push(error.toString()));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  page.on('response', response => { if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
}

try {
  if (process.env.POLYORAMA_ICON_PREVIEW_URL) {
    const preview = new URL(process.env.POLYORAMA_ICON_PREVIEW_URL);
    assert(preview.protocol === 'https:' && preview.pathname === '/gallery/'
      && !preview.username && !preview.password && !preview.search && !preview.hash,
    'POLYORAMA_ICON_PREVIEW_URL must be the private HTTPS /gallery/ URL');
    url = preview.href;
  } else {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    serverStarted = true;
    url = `http://127.0.0.1:${server.address().port}/`;
  }
  context = await chromium.launchPersistentContext(profile, {
    ...hostedLinuxWebGpuLaunchOptions(), viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1,
    executablePath: process.env.POLYORAMA_CHROMIUM,
    env: { ...process.env, LD_LIBRARY_PATH: process.env.POLYORAMA_USE_SYSTEM_UI_LIBS === '1'
      ? process.env.LD_LIBRARY_PATH ?? '' : `${resolve('.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}` },
  });
  browser = context.browser();
  page = await context.newPage();
  observeErrors(page);
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await ready();
  const manifest = await page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.manifest());
  const requiredStories = ['icons/vocabulary', 'icons/action-presentations', 'icons/toolbar', 'icons/long-narrow'];
  assert(requiredStories.every(id => manifest.some(story => story.id === id)), 'typed icon stories registered');
  assert.equal(new Set(manifest.map(story => story.id)).size, manifest.length);
  await story('icons/action-presentations');
  const ordinary = await record('presentations', false);
  const saveNodes = ordinary.ui_snapshot.nodes.filter(node => node.actions.includes('save_layout'));
  assert.equal(saveNodes.length, 2, 'two distinct presentations of Save layout');
  assert.notEqual(saveNodes[0].id, saveNodes[1].id, 'logical instance identity');
  const disabled = (await control('disabled')).node;
  assert.equal(disabled.enabled, false);
  assert.equal(disabled.disabled_reason, 'History is empty');
  await click('text');
  await waitFixture({ 'count:text': 1 });
  const icon = await control('icon');
  await page.mouse.move(icon.point.x, icon.point.y);
  await waitFixture({ hovered: 'icon' });
  await record('hover', true, { hovered: 'icon', pointer_down: null });
  await page.mouse.down();
  await waitFixture({ pointer_down: 'icon' });
  await record('pressed', true, { hovered: 'icon', pointer_down: 'icon' });
  pressedComparison = await comparePressedControl();
  await page.mouse.up();
  await waitFixture({ 'count:icon': 1, pointer_down: null });
  await record('pointer-activation', false);
  await click('toggle');
  await waitFixture({ 'count:toggle': 1, linked: false });
  await click('disabled');
  await waitFixture({ 'count:disabled': 0 });
  await record('toggle-disabled', false);
  const unavailable = await control('disabled');
  await page.mouse.move(1400, 880);
  await page.waitForTimeout(150); // Leave the preceding click before entering a fresh hover.
  await page.mouse.move(unavailable.point.x, unavailable.point.y);
  await page.waitForTimeout(700); // Production disabled-tooltip delay, bounded below one second.
  await record('disabled-tooltip');
  await click('text');
  await waitFixture({ 'count:text': 2 });
  await page.mouse.move(1400, 880);
  await focus('icon');
  await record('keyboard-focus', true, {}, 'icon');
  await click('icon');
  await waitFixture({ 'count:icon': 2 });
  await page.mouse.down();
  await waitFixture({ hovered: 'icon', pointer_down: 'icon' });
  await record('focused-pressed', true, { hovered: 'icon', pointer_down: 'icon' }, 'icon');
  await page.mouse.up();
  await waitFixture({ 'count:icon': 3, pointer_down: null });
  await page.keyboard.press('Enter');
  await waitFixture({ 'count:icon': 4 });
  await focus('labelled');
  await page.keyboard.press('Space');
  await waitFixture({ 'count:labelled': 1 });
  await focus('toggle');
  await page.keyboard.press('Enter');
  await waitFixture({ 'count:toggle': 2, linked: true });
  await record('keyboard-activation', false);
  await story('icons/toolbar');
  await record('toolbar-dark');
  await click('polygon');
  await waitFixture({ selected_tool: 'polygon', 'count:polygon': 1 });
  await click('labelled');
  await waitFixture({ 'count:labelled': 1 });
  await record('toolbar-activation', false);
  await story('icons/toolbar', configuration({ appearance: 'light', density: 'compact' }));
  await record('toolbar-light-compact');
  await story('icons/long-narrow', configuration({ contrast: 'high', font_scale: 1.5, width: 'narrow' }));
  const narrow = await record('long-narrow-high-contrast');
  const longName = 'Export the selected annotations and their complete supporting evidence';
  assert.equal(narrow.ui_snapshot.nodes.filter(node => node.actions.includes('export_selection') && node.name === longName).length, 2, 'complete names for both long-label instances');
  assert.equal(narrow.text.filter(text => text.truncated && text.role === 'button_label').length, 2, 'both reserved label slots elide');
  await click('export');
  await waitFixture({ 'count:export': 1 });
  await record('long-narrow-activation', false);
  await story('icons/vocabulary');
  const vocabulary = await record('vocabulary');
  assert.equal(vocabulary.text.length, 30, 'complete measured vocabulary captions');
  for (const scale of [1.25, 2]) {
    const scaledContext = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: scale });
    try {
      page = await scaledContext.newPage();
      observeErrors(page);
      await page.goto(url, { waitUntil: 'domcontentloaded' });
      await ready();
      await story('icons/vocabulary');
      const scaled = await record(`vocabulary-display-${scale}`);
      assert.equal(scaled.ui_snapshot.pixels_per_point, scale, 'actual presentation scale');
    } finally { await scaledContext.close(); }
  }
  page = context.pages().find(page => !page.isClosed() && page.url() === url);
  assert(page, 'retained ordinary page');
  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, [], 'browser runtime errors');
  const report = {
    source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '',
    wasm_sha256: createHash('sha256').update(await readFile(resolve(root, 'pkg/polyorama_gallery_bg.wasm'))).digest('hex'),
    url, browser: browser.version(), launch: hostedLinuxWebGpuLaunchOptions(), manifest, steps, targets, pressed_comparison: pressedComparison, idle, errors,
    input_route: 'Playwright physical mouse/keyboard; hooks select fixed stories/configurations and read state/geometry',
  };
  await writeFile(resolve(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Typed icon browser smoke passed: ${manifest.length} stories, pointer/keyboard, disabled/toggle, 1/1.25/2 display scale and idle; ${url}`);
} catch (error) {
  if (page && !page.isClosed()) {
    await page.screenshot({ path: resolve(evidence, 'browser-failure.png') }).catch(() => {});
    await writeFile(resolve(evidence, 'browser-failure.json'), `${JSON.stringify(await snapshot().catch(() => null), null, 2)}\n`);
  }
  await writeFile(resolve(evidence, 'browser-failure-report.json'), `${JSON.stringify({ error: String(error), url, errors, steps, targets, pressed_comparison: pressedComparison }, null, 2)}\n`);
  throw error;
} finally {
  await context?.close();
  if (serverStarted) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
