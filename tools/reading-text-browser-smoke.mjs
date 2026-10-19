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

// The Gallery's reading/conversation reply, exactly as the story presents it.
const REPLY = "Two papers stand out this morning.\n\n"
  + "The first describes an attention layout for long documents that keeps memory "
  + "use flat as the context grows. Its evaluation covers question answering over "
  + "books and retrieval across a hundred thousand tokens, and the gains hold on "
  + "smaller models as well as larger ones, which makes it practical on a laptop.\n\n"
  + "The second surveys how agents that call tools are evaluated. It separates "
  + "task success from the cost of reaching it, argues that most published "
  + "results report only the first, and proposes a short checklist for honest "
  + "comparisons: fixed budgets, repeated runs and published transcripts. Both "
  + "links open in a new tab, so this conversation stays where it is.";
const ATTENTION = { name: 'Flat-memory attention for long documents', url: 'https://example.org/papers/flat-memory-attention' };
const SURVEY = { name: 'Evaluating agents that call tools: a survey', url: 'https://example.org/papers/evaluating-tool-agents' };
const ARCHIVED = 'Archived copy (unavailable offline)';

const root = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/reading-text-browser');
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
const profile = await mkdtemp(join(tmpdir(), 'stobie-reading-text-browser-'));
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
  assert(!page.isClosed());
  const after = await snapshot();
  assert.equal(after.story, before.story, `${label}: the Gallery kept its state`);
  assert(after.frame >= before.frame);
  opened.push({ route: label, url: tab.url(), opener: null, referrer: '', gallery_url: page.url(), story: after.story });
  await tab.close();
  await page.bringToFront();
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
  browser = context.browser(); page = await context.newPage();
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  page.on('pageerror', error => errors.push(String(error)));
  const response = await page.goto(rendererUrl(url), { waitUntil: 'domcontentloaded' });
  assert(response?.ok(), `Gallery HTTP status ${response?.status()}`);
  await page.waitForFunction(() => document.body.classList.contains('ready') && window.__STOBIE_GALLERY_HANDLE?.snapshot().frame > 0);
  await assertBrowserRenderer(page);

  await story('reading/conversation');
  const conversation = await record('conversation-dark');
  const reading = conversation.text.filter(t => t.component_id.kind === 'reading_text');
  assert.equal(reading.length, 3, 'three messages');
  assert(reading.every(t => t.overflow === 'wrap_unbounded' && t.declared_max_lines === 0 && !t.truncated));
  assert(reading.some(t => t.line_count > 8), 'the reply runs past the bounded recipes\' eight lines');
  const links = conversation.ui_snapshot.nodes.filter(n => n.role === 'link');
  assert.deepEqual(links.map(n => [n.name, n.value]), [[ATTENTION.name, ATTENTION.url], [SURVEY.name, SURVEY.url]]);
  // The ARIA layer presents each as a link.
  for (const { name } of [ATTENTION, SURVEY]) assert.equal(await page.getByRole('link', { name, exact: true }).count(), 1, `ARIA link ${name}`);

  // Selecting the whole reply copies it exactly, across paragraphs.
  const replyText = reading.reduce((a, b) => (b.line_count > a.line_count ? b : a));
  const area = centre(conversation, replyText.allocated_rect, await page.locator('canvas').boundingBox());
  await page.mouse.move(area.left + 1, area.top + 4); await page.mouse.down();
  await page.mouse.move(area.right - 1, area.bottom - 4, { steps: 12 }); await page.mouse.up();
  await page.keyboard.press('Control+c');
  await page.waitForFunction(expected => navigator.clipboard.readText().then(text => text === expected), REPLY, { timeout: 5_000 })
    .catch(async error => {
      const copied = await page.evaluate(() => navigator.clipboard.readText());
      throw new Error(`copied ${JSON.stringify(copied)}: ${error}`);
    });
  await record('reply-selected');

  // A physical click opens the link in a new tab; the Gallery stays.
  const attention = await link(ATTENTION.name);
  await page.mouse.move(attention.point.x, attention.point.y);
  await record('link-hover');
  await opensNewTab('pointer', ATTENTION.url, () => page.mouse.click(attention.point.x, attention.point.y));
  // Keyboard: focus through the ARIA layer, then Enter and Space.
  for (const key of ['Enter', 'Space']) {
    await page.getByRole('link', { name: SURVEY.name, exact: true }).focus();
    await page.waitForFunction(name => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes.some(n => n.role === 'link' && n.name === name && n.focused), SURVEY.name);
    if (key === 'Enter') await record('link-focused');
    await opensNewTab(`keyboard-${key}`, SURVEY.url, () => page.keyboard.press(key));
  }

  // Narrow, enlarged high contrast: long labels wrap, a blank label shows its
  // URL, a disabled link opens nothing, and scrolling replaces observations.
  await story('reading/long-narrow', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'narrow' }));
  const narrow = await record('long-narrow-light-high-contrast-150');
  const narrowLinks = narrow.ui_snapshot.nodes.filter(n => n.role === 'link');
  assert.equal(narrowLinks.length, 3);
  assert(narrowLinks.some(n => n.name === 'https://example.org/changes'), 'a blank label shows the URL');
  const linkText = narrow.text.filter(t => t.component_id.kind === 'link');
  assert(linkText.some(t => t.line_count > 1) && linkText.every(t => !t.truncated), 'long link labels wrap, never truncate');
  const disabled = await link(ARCHIVED);
  assert.equal(disabled.node.enabled, false);
  let blocked = false;
  const unexpected = context.waitForEvent('page', { timeout: 1_500 }).then(() => { blocked = true; }, () => {});
  await page.mouse.click(disabled.point.x, disabled.point.y);
  await unexpected;
  assert.equal(blocked, false, 'a disabled link opens nothing');
  const crossing = narrow.text.filter(t => t.component_id.kind === 'reading_text');
  assert(crossing.some(t => t.allocated_rect.max_y > t.clip_rect.max_y), 'reading text crossing the fold keeps its clipped observation');
  assert(narrow.text_audit_coverage.attempted_components > narrow.text_audit_coverage.measured_components, 'text below the fold is attempted but not observed');
  const bare = await link('https://example.org/changes');
  await page.mouse.move(bare.point.x, bare.point.y); await page.mouse.wheel(0, 400);
  await page.waitForFunction(frame => window.__STOBIE_GALLERY_HANDLE.snapshot().frame > frame, narrow.frame);
  await page.waitForTimeout(300);
  const scrolled = await record('long-narrow-scrolled');
  const before = new Set(narrow.text.map(t => `${t.component_id.kind}:${t.component_id.instance}`));
  const after = new Set(scrolled.text.map(t => `${t.component_id.kind}:${t.component_id.instance}`));
  assert.notDeepEqual([...after].sort(), [...before].sort(), 'scrolling changes the current observations');

  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, []);
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/stobie_gallery_bg.wasm'))).digest('hex'), url, browser: browser.version(), steps, targets, opened, idle, input_route: 'Physical Playwright mouse and keyboard, and focus through the ARIA layer; hooks only choose stories and appearance and read observations', errors };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Reading-text browser workflow passed: full copy, new-tab links by pointer and keyboard, disabled link, narrow wrapping, scrolling and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), state: await snapshot().catch(() => null), steps, targets, opened }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await context?.close();
  if (server.listening) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
