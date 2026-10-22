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

// Scrolling recipes in the Gallery, in headful Chromium with physical input:
// a conversation that loads older messages keeps the reader's place, both
// scrolled up through its history and at its newest message, and a list
// pane's footer sits at the pane's foot, at desktop and phone widths in
// both themes, read through observation.
const root = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/scroll-browser');
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
const profile = await mkdtemp(join(tmpdir(), 'stobie-scroll-browser-'));
const steps = [];
const targets = [];
const errors = [];
const places = [];
let context, page, browser, url;
const snapshot = async () => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot());
const fixture = () => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot().scroll_fixture);
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
  // Bubbles fit their content over the next passes.
  await page.waitForTimeout(300);
}
async function point(rect) {
  const s = await snapshot();
  const root = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const canvas = await page.locator('canvas').boundingBox();
  return { x: canvas.x + ((rect.min_x + rect.max_x) / 2 - root.min_x) * canvas.width / (root.max_x - root.min_x), y: canvas.y + ((rect.min_y + rect.max_y) / 2 - root.min_y) * canvas.height / (root.max_y - root.min_y) };
}
async function target(name, role) {
  const s = await snapshot();
  const node = s.ui_snapshot.nodes.find(n => n.name === name && n.role === role);
  assert(node, `visible ${role} ${name}`);
  const at = await point(node.rect);
  targets.push({ name, role, id: node.id, frame: s.frame, rect: node.rect, point: at });
  return { node, point: at };
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
// The story publishes only text wholly in view.
const texts = s => s.text.filter(t => t.content?.text);
const text = (s, words) => texts(s).find(t => t.content.text === words);
const messages = s => texts(s).filter(t => t.component_id.kind === 'reading_text').sort((a, b) => a.allocated_rect.min_y - b.allocated_rect.min_y);
const newest = s => messages(s).find(t => t.content.text.startsWith('52. '));
const bubbles = s => s.ui_snapshot.nodes.filter(n => n.role === 'section' && ['You', 'Ada Lovelace'].includes(n.name));
const loadOlder = s => s.ui_snapshot.nodes.find(n => n.role === 'button' && n.name === 'Load older');

// The account sits at the list pane's foot, far below the short list's
// last row, level with the foot of the conversation beside it (desktop)
// or just above the conversation below it (phone).
function checkFooter(s, label, wide) {
  const status = text(s, 'Signed in');
  const last = text(s, 'Raffle prizes');
  assert(status && last, `${label}: the list and its footer in view`);
  const account = texts(s).find(t => t.content.text === 'Ada Lovelace' && Math.abs(t.allocated_rect.min_x - status.allocated_rect.min_x) < 1);
  assert(account, `${label}: the account in view`);
  assert(account.allocated_rect.max_y <= status.allocated_rect.min_y + 0.5, `${label}: the account above its status`);
  const foot = status.allocated_rect.max_y;
  if (wide) {
    const conversation = Math.max(...bubbles(s).map(n => n.rect.max_y));
    assert(foot - last.allocated_rect.max_y > 200, `${label}: the footer is at the pane's foot, not after the list (${foot}, ${last.allocated_rect.max_y})`);
    assert(Math.abs(foot - conversation) < 24, `${label}: the footer is level with the conversation's foot (${foot}, ${conversation})`);
  } else {
    const below = loadOlder(s).rect.min_y;
    assert(foot < below && below - foot < 32, `${label}: the footer is at the foot of the list pane, just above the conversation (${foot}, ${below})`);
    assert(foot - last.allocated_rect.max_y > 8, `${label}: spare room sits between the list and its footer`);
  }
}

async function wheelOver(s, delta, times) {
  const visible = bubbles(s);
  assert(visible.length > 0, 'a message in view');
  const at = await point(visible[Math.floor(visible.length / 2)].rect);
  await page.mouse.move(at.x, at.y);
  for (let i = 0; i < times; i += 1) {
    await page.mouse.wheel(0, delta);
    await page.waitForTimeout(80);
  }
  await page.waitForTimeout(400);
}

// Load older with a pointer and check the place the reader was reading
// stays where it was on screen.
async function loadAndKeep(name, place) {
  const before = await snapshot();
  const kept = place(before);
  assert(kept, `${name}: a message to keep in view`);
  const loads = (await fixture()).loads;
  const button = await target('Load older', 'button');
  assert.equal(button.node.enabled, true, `${name}: Load older is enabled`);
  await page.mouse.click(button.point.x, button.point.y);
  await page.waitForFunction(loads => window.__STOBIE_GALLERY_HANDLE.snapshot().scroll_fixture.loads === loads + 1, loads);
  await page.waitForTimeout(300);
  const after = await record(name);
  const again = text(after, kept.content.text);
  assert(again, `${name}: ${kept.content.text} still in view`);
  const moved = again.allocated_rect.min_y - kept.allocated_rect.min_y;
  places.push({ step: name, text: kept.content.text, before: kept.allocated_rect, after: again.allocated_rect, moved });
  assert(Math.abs(moved) <= 1, `${name}: ${kept.content.text} moved ${moved}`);
  return after;
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

  // Desktop, dark: the list beside the conversation, which opens at its
  // newest message.
  await story('scroll/load-older', configuration({ width: 'wide' }));
  const desktop = await record('desktop-dark');
  assert(newest(desktop), 'the conversation opens at its newest message');
  assert(!texts(desktop).some(t => t.content.text.startsWith('1. ')), 'older messages are not loaded yet');
  checkFooter(desktop, 'desktop', true);

  // At the newest message: older messages arrive above, and the newest
  // message stays where it was.
  await loadAndKeep('desktop-at-end-load', newest);
  // Scrolled up through the history: the message the reader was reading
  // stays where it was.
  await wheelOver(await snapshot(), -240, 6);
  const scrolled = await record('desktop-scrolled-up');
  assert(!newest(scrolled), 'the reader scrolled away from the newest message');
  await loadAndKeep('desktop-scrolled-up-load', s => messages(s)[0]);
  // The last page: the control explains why it is unavailable.
  await loadAndKeep('desktop-last-load', s => messages(s)[0]);
  const last = await snapshot();
  const button = loadOlder(last);
  assert.deepEqual([button.enabled, button.disabled_reason], [false, 'There are no older messages']);
  assert.equal((await fixture()).older_pages, 3);
  // The oldest message reads first once the reader scrolls to the top.
  await wheelOver(last, -400, 20);
  const top = await record('desktop-oldest');
  assert(messages(top)[0].content.text.startsWith('1. '), `the oldest message: ${messages(top)[0].content.text}`);

  // Phone, light: the short list above the conversation; a fresh fixture.
  await story('chat/conversation', configuration({ appearance: 'light', width: 'narrow' }));
  await story('scroll/load-older', configuration({ appearance: 'light', width: 'narrow' }));
  assert.equal((await fixture()).loads, 0, 'a fresh conversation');
  await wheelOver(await snapshot(), 400, 30);
  const phone = await record('phone-light');
  assert(newest(phone), 'the phone conversation at its newest message');
  checkFooter(phone, 'phone', false);
  await loadAndKeep('phone-at-end-load', newest);
  await wheelOver(await snapshot(), -160, 4);
  await loadAndKeep('phone-scrolled-up-load', s => messages(s)[0]);

  // Light high contrast at 150%: the list above the conversation is longer
  // than its pane, so its footer follows the last row; the reader's place.
  await story('chat/conversation', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'wide' }));
  await story('scroll/load-older', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'wide' }));
  await wheelOver(await snapshot(), 400, 30);
  const large = await record('large-light-high-contrast-150');
  const raffle = text(large, 'Raffle prizes');
  const account = texts(large).find(t => t.content.text === 'Ada Lovelace' && Math.abs(t.allocated_rect.min_x - raffle.allocated_rect.min_x) < 1);
  assert(account, 'large: the account in view');
  const gap = account.allocated_rect.min_y - raffle.allocated_rect.max_y;
  assert(gap > 0 && gap < 48, `large: the footer follows the last row (${gap})`);
  assert(!text(large, 'Signed in'), 'large: the footer\'s last line is below the short pane\'s view');
  await loadAndKeep('large-at-end-load', newest);
  await wheelOver(await snapshot(), -160, 3);
  await loadAndKeep('large-scrolled-up-load', s => messages(s)[0]);

  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, []);
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/stobie_gallery_bg.wasm'))).digest('hex'), url, browser: browser.version(), steps, targets, places, idle, input_route: 'Physical Playwright mouse; hooks only choose stories/appearance and read observations', errors };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Scroll browser workflow passed: older messages keep the reader\'s place scrolled up and at the newest message, the pane footer, both themes, desktop and phone widths and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), state: await snapshot().catch(() => null), steps, targets, places }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await context?.close();
  if (server.listening) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
