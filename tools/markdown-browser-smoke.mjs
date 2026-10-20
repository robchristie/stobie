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

// The Gallery's markdown/digest story, as its source presents it.
const FIRST = { name: 'Flat-memory attention for long documents', url: 'https://example.org/papers/flat-memory-attention' };
const SECOND = { name: 'Evaluating agents that call tools', url: 'https://example.org/papers/evaluating-tool-agents' };
const LINKS = [
  [FIRST.name, FIRST.url],
  [SECOND.name, SECOND.url],
  ['Speculative decoding on a phone', 'https://example.org/papers/speculative-decoding-phone'],
  ['Probing attention heads for retrieval', 'https://example.org/papers/retrieval-heads'],
  ['discussion', 'https://example.org/discussions/reward-models'],
];
// The first item's text, link label included, exactly as selection copies it.
const FIRST_ITEM = `${FIRST.name} — keeps memory use flat as the context grows, and the gains hold on small models.`;

const root = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/markdown-browser');
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
const profile = await mkdtemp(join(tmpdir(), 'stobie-markdown-browser-'));
const steps = [];
const targets = [];
const errors = [];
const opened = [];
let context, page, browser, url;
const snapshot = () => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot());
const configuration = (overrides = {}) => ({ appearance: 'dark', contrast: 'standard', density: 'comfortable', font_scale: 1, width: 'regular', ...overrides });
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
}
function centre(s, rect, canvas) {
  const root = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const x = value => canvas.x + (value - root.min_x) * canvas.width / (root.max_x - root.min_x);
  const y = value => canvas.y + (value - root.min_y) * canvas.height / (root.max_y - root.min_y);
  return { x: x((rect.min_x + rect.max_x) / 2), y: y((rect.min_y + rect.max_y) / 2), left: x(rect.min_x), right: x(rect.max_x), top: y(rect.min_y), bottom: y(rect.max_y) };
}
async function link(name) {
  const s = await snapshot();
  const node = s.ui_snapshot.nodes.find(n => n.role === 'link' && n.name === name);
  assert(node, `visible link ${name}`);
  const point = centre(s, node.rect, await page.locator('canvas').boundingBox());
  targets.push({ name, role: 'link', id: node.id, frame: s.frame, rect: node.rect, point });
  return { node, point };
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
/** Activate with `act` and return the tab it opened, checking the Gallery stays put. */
async function opensNewTab(label, expected, act) {
  const before = await snapshot();
  const pageUrl = page.url();
  const popup = context.waitForEvent('page', { timeout: 10_000 });
  await act();
  const tab = await popup;
  await tab.waitForLoadState('domcontentloaded');
  assert.equal(tab.url(), expected, `${label}: the new tab shows the link's URL`);
  assert.equal(await tab.evaluate(() => window.opener), null, `${label}: the new tab has no opener`);
  assert.equal(await tab.evaluate(() => document.referrer), '', `${label}: the new tab has no referrer`);
  assert.equal(page.url(), pageUrl, `${label}: the Gallery page was not replaced`);
  const after = await snapshot();
  assert.equal(after.story, before.story, `${label}: the Gallery kept its state`);
  opened.push({ route: label, url: tab.url(), opener: null, referrer: '', gallery_url: page.url(), story: after.story });
  await tab.close();
  await page.bringToFront();
}
async function opensNothing(act) {
  let blocked = false;
  const unexpected = context.waitForEvent('page', { timeout: 1_500 }).then(() => { blocked = true; }, () => {});
  await act();
  await unexpected;
  return !blocked;
}
try {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  url = `http://127.0.0.1:${server.address().port}/`;
  context = await chromium.launchPersistentContext(profile, {
    ...hostedLinuxWebGpuLaunchOptions(), viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1,
    executablePath: process.env.STOBIE_CHROMIUM,
    env: { ...process.env, LD_LIBRARY_PATH: process.env.STOBIE_USE_SYSTEM_UI_LIBS === '1' ? process.env.LD_LIBRARY_PATH ?? '' : `${resolve('.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}` },
  });
  // Links point at example.org; answer them locally so nothing leaves the host.
  await context.route('https://example.org/**', route => route.fulfill({ status: 200, contentType: 'text/html', body: '<!doctype html><title>Example source</title><p>Example source</p>' }));
  // Nothing else may load either, such as an unsafe link's target.
  await context.route(target => !target.href.startsWith(url) && !target.href.startsWith('https://example.org/'), route => route.abort());
  browser = context.browser(); page = await context.newPage();
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  page.on('pageerror', error => errors.push(String(error)));
  const response = await page.goto(rendererUrl(url), { waitUntil: 'domcontentloaded' });
  assert(response?.ok(), `Gallery HTTP status ${response?.status()}`);
  await page.waitForFunction(() => document.body.classList.contains('ready') && window.__STOBIE_GALLERY_HANDLE?.snapshot().frame > 0);
  await assertBrowserRenderer(page);

  await story('markdown/digest');
  const digest = await record('digest-dark');
  const blocks = digest.text.filter(t => t.component_id.kind === 'reading_text');
  assert(blocks.length >= 10, `each block is measured reading text: ${blocks.length}`);
  assert(blocks.every(t => t.overflow === 'wrap_unbounded' && t.declared_max_lines === 0 && !t.truncated && t.interaction === 'selectable'));
  const markers = digest.text.filter(t => t.component_id.kind === 'list_marker');
  assert.equal(markers.length, 4, 'four numbered items');
  const links = digest.ui_snapshot.nodes.filter(n => n.role === 'link');
  assert.deepEqual(links.map(n => [n.name, n.value]), LINKS);
  // The ARIA layer presents each link, and the headings with their levels.
  for (const [name] of LINKS) assert.equal(await page.getByRole('link', { name, exact: true }).count(), 1, `ARIA link ${name}`);
  assert.equal(await page.getByRole('heading', { name: 'Morning digest', level: 2 }).count(), 1, 'ARIA heading level 2');
  assert.equal(await page.getByRole('heading', { name: 'Also noted', level: 3 }).count(), 1, 'ARIA heading level 3');

  // Selecting a whole item copies its text, the link's label included.
  const item = blocks.find(t => {
    const rect = links[0].rect;
    return t.allocated_rect.min_y <= rect.min_y && rect.max_y <= t.allocated_rect.max_y + 1;
  });
  assert(item, 'the first item holds the first link');
  const area = centre(digest, item.allocated_rect, await page.locator('canvas').boundingBox());
  await page.mouse.move(area.left + 1, area.top + 4); await page.mouse.down();
  await page.mouse.move(area.right - 1, area.bottom - 4, { steps: 12 }); await page.mouse.up();
  await page.keyboard.press('Control+c');
  await page.waitForFunction(expected => navigator.clipboard.readText().then(text => text === expected), FIRST_ITEM, { timeout: 5_000 })
    .catch(async error => {
      const copied = await page.evaluate(() => navigator.clipboard.readText());
      throw new Error(`copied ${JSON.stringify(copied)}: ${error}`);
    });
  assert.equal((await snapshot()).story, 'markdown/digest', 'dragging across a link selects, it does not open');
  await record('item-selected');

  // A physical click on a link inside the text opens a new tab.
  const first = await link(FIRST.name);
  await page.mouse.move(first.point.x, first.point.y);
  await record('link-hover');
  await opensNewTab('pointer', FIRST.url, () => page.mouse.click(first.point.x, first.point.y));
  // Keyboard: focus through the ARIA layer, then Enter and Space.
  for (const key of ['Enter', 'Space']) {
    await page.getByRole('link', { name: SECOND.name, exact: true }).focus();
    await page.waitForFunction(name => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes.some(n => n.role === 'link' && n.name === name && n.focused), SECOND.name);
    if (key === 'Enter') await record('link-focused');
    await opensNewTab(`keyboard-${key}`, SECOND.url, () => page.keyboard.press(key));
  }

  // Narrow, enlarged high contrast: unsafe links stay text, open nothing and
  // say why; long code and identifiers wrap; scrolling reaches the partial tail.
  await story('markdown/long-narrow', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'narrow' }));
  const narrow = await record('long-narrow-light-high-contrast-150');
  assert(narrow.text.filter(t => t.component_id.kind === 'reading_text').every(t => !t.truncated && t.layout_error == null));
  const code = narrow.text.find(t => t.role === 'monospace_technical');
  assert(code && code.line_count > 1, 'the long code line wraps');
  for (const name of ['run script', 'relative']) {
    const unsafe = await link(name);
    assert.equal(unsafe.node.enabled, false, `${name} is unavailable`);
    assert(unsafe.node.disabled_reason, `${name} says why`);
    assert(await opensNothing(() => page.mouse.click(unsafe.point.x, unsafe.point.y)), `${name} opens nothing`);
  }
  assert.equal((await snapshot()).story, 'markdown/long-narrow', 'the Gallery stays');
  const canvas = await page.locator('canvas').boundingBox();
  const scrollAt = centre(narrow, code.allocated_rect, canvas);
  let scrolled = narrow;
  for (let turn = 0; turn < 8 && !scrolled.ui_snapshot.nodes.some(n => n.role === 'link' && n.name === 'the survey'); turn += 1) {
    await page.mouse.move(scrollAt.x, scrollAt.y); await page.mouse.wheel(0, 400);
    await page.waitForFunction(frame => window.__STOBIE_GALLERY_HANDLE.snapshot().frame > frame, scrolled.frame);
    await page.waitForTimeout(300);
    scrolled = await snapshot();
  }
  scrolled = await record('long-narrow-scrolled');
  const partial = scrolled.ui_snapshot.nodes.find(n => n.role === 'link' && n.name === 'the survey');
  assert(partial && partial.enabled, 'the reply still arriving links what is complete');
  assert(!scrolled.ui_snapshot.nodes.some(n => n.role === 'link' && n.name === 'link'), 'an unfinished link stays text');
  const before = new Set(narrow.text.map(t => `${t.component_id.kind}:${t.component_id.instance}`));
  const after = new Set(scrolled.text.map(t => `${t.component_id.kind}:${t.component_id.instance}`));
  assert.notDeepEqual([...after].sort(), [...before].sort(), 'scrolling changes the current observations');
  const archived = scrolled.ui_snapshot.nodes.find(n => n.role === 'link' && n.name === 'the original thread');
  if (archived) {
    assert.equal(archived.enabled, false, 'a link in a disabled region is unavailable');
    const point = centre(scrolled, archived.rect, canvas);
    assert(await opensNothing(() => page.mouse.click(point.x, point.y)), 'a disabled link opens nothing');
  }

  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, []);
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/stobie_gallery_bg.wasm'))).digest('hex'), url, browser: browser.version(), steps, targets, opened, idle, input_route: 'Physical Playwright mouse and keyboard, and focus through the ARIA layer; hooks only choose stories and appearance and read observations', errors };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Markdown browser workflow passed: item copy across a link, new-tab links by pointer and keyboard, unsafe links, narrow wrapping, partial input, scrolling and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), state: await snapshot().catch(() => null), steps, targets, opened }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await context?.close();
  if (server.listening) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
