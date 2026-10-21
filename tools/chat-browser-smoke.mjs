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

// Chat recipes in the Gallery, in headful Chromium with physical input:
// conversation rows, switches and message bubbles at desktop and phone
// widths in both themes, read through observation and the ARIA layer.
const root = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/chat-browser');
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
const profile = await mkdtemp(join(tmpdir(), 'stobie-chat-browser-'));
const steps = [];
const targets = [];
const errors = [];
let context, page, browser, url;
const snapshot = async () => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot());
const fixture = () => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot().chat_fixture);
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
const rows = s => s.ui_snapshot.nodes.filter(n => n.role === 'navigation_item');
const switches = s => s.ui_snapshot.nodes.filter(n => n.role === 'switch');
const bubbles = s => s.ui_snapshot.nodes.filter(n => n.role === 'section' && ['You', 'Ada Lovelace'].includes(n.name));
const width = r => r.max_x - r.min_x;
const layer = () => page.locator('[id^="stobie-a11y:layer"]');
const control = (role, name) => layer().getByRole(role, { name, exact: true });

// Each bubble sits on its side and within its column; the person's own
// bubbles end where the others start lines.
function checkBubbles(s, label) {
  const all = bubbles(s);
  const own = all.filter(n => n.name === 'You');
  const other = all.filter(n => n.name !== 'You');
  assert(own.length > 0 && other.length > 0, `${label}: bubbles on both sides`);
  const right = Math.max(...own.map(n => n.rect.max_x));
  const left = Math.min(...other.map(n => n.rect.min_x));
  for (const n of own) assert(Math.abs(n.rect.max_x - right) < 1, `${label}: own bubble at the end ${JSON.stringify(n.rect)}`);
  for (const n of other) assert(Math.abs(n.rect.min_x - left) < 1, `${label}: other bubble at the start ${JSON.stringify(n.rect)}`);
  // A bubble takes at most 85% of its column.
  const column = right - left;
  for (const n of all) assert(width(n.rect) <= column * 0.85 + 1, `${label}: bubble within 85% of ${column}: ${JSON.stringify(n.rect)}`);
  // A short message hugs its text on one line.
  for (const t of s.text.filter(t => ['Thank you!', 'OK'].includes(t.content?.text))) assert.equal(t.line_count, 1, `${label}: ${t.content.text} on one line`);
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

  // Desktop, dark: the list beside the conversation.
  await story('chat/conversation', configuration({ width: 'wide' }));
  const desktop = await record('conversation-dark');
  assert.deepEqual(rows(desktop).map(n => [n.name, n.selected]), [
    ['Ada Lovelace', true], ['Kitchen helper', false], ['Grace Hopper', false], ['Edsger Dijkstra', false],
  ]);
  // Each row opens its own conversation by its argument.
  for (const [n, id] of rows(desktop).map((n, i) => [n, ['ada', 'helper', 'grace', 'edsger'][i]])) {
    assert(JSON.stringify(n.argument).includes(`"${id}"`), `${n.name} argument ${JSON.stringify(n.argument)}`);
  }
  const helper = rows(desktop).find(n => n.name === 'Kitchen helper');
  assert.equal(helper.description, '2 unread messages; Three dinners that use the leftover rice; 08:15; Open the conversation with this ID');
  assert.equal(rows(desktop).find(n => n.name === 'Ada Lovelace').description, 'The notes on the engine are ready; 09:41; Open the conversation with this ID');
  assert.deepEqual(switches(desktop).map(n => [n.name, n.checked, n.enabled]), [['Notifications', true, true], ['Keep at the top', false, true]]);
  const kinds = desktop.text.filter(t => t.parent_id?.kind === 'conversation_row').map(t => t.component_id.kind);
  assert.deepEqual(kinds.slice(0, 3), ['conversation_row_time', 'conversation_row_title', 'conversation_row_preview']);
  // Desktop rows show whole names and times; a long preview elides.
  assert(desktop.text.every(t => !['conversation_row_title', 'conversation_row_time'].includes(t.component_id.kind) || !t.truncated), 'desktop rows show whole names and times');
  checkBubbles(desktop, 'desktop');
  const listRight = Math.max(...rows(desktop).map(n => n.rect.max_x));
  assert(bubbles(desktop).every(n => n.rect.min_x > listRight), 'the conversation is beside the list');
  // Agents read the messages' words as content.
  const content = desktop.text.map(t => t.content?.text).filter(Boolean);
  assert(content.includes('Thank you!'), `content: ${content}`);
  assert(content.some(text => text.startsWith('The notes on the engine are ready.')), `content: ${content}`);
  // The ARIA layer: rows are buttons with aria-current, switches state
  // aria-checked, and each bubble groups its message under its sender.
  await layer().locator('[role]').first().waitFor({ state: 'attached', timeout: 60000 });
  assert.equal(await control('button', 'Ada Lovelace').getAttribute('aria-current'), 'true');
  assert.equal(await control('button', 'Kitchen helper').getAttribute('aria-current'), 'false');
  assert.match(await control('button', 'Kitchen helper').getAttribute('aria-description'), /^2 unread messages; Three dinners/);
  assert.equal(await control('switch', 'Notifications').getAttribute('aria-checked'), 'true');
  assert.equal(await control('switch', 'Keep at the top').getAttribute('aria-checked'), 'false');
  assert.equal(await control('group', 'You').count(), 2, 'the person\'s two messages');
  assert.equal(await control('group', 'Ada Lovelace').count(), 2, 'the other party\'s two messages');

  // Hover, then a pointer opens a conversation, which is then read.
  const helperRow = await target('Kitchen helper', 'navigation_item');
  await page.mouse.move(helperRow.point.x, helperRow.point.y);
  await record('row-hover');
  await page.mouse.click(helperRow.point.x, helperRow.point.y);
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().chat_fixture.open === 'helper');
  const opened = await record('row-opened');
  assert.deepEqual(rows(opened).map(n => n.selected), [false, true, false, false]);
  const read = rows(opened).find(n => n.name === 'Kitchen helper');
  assert(read.focused, 'the opened row has focus');
  assert.equal(read.description, 'Three dinners that use the leftover rice; 08:15; Open the conversation with this ID');
  await page.waitForFunction(() => document.querySelector('[id^="stobie-a11y:layer"] [aria-label="Kitchen helper"]')?.getAttribute('aria-current') === 'true', null, { timeout: 5000 });
  // The keyboard reaches the next row and opens it.
  await page.keyboard.press('Tab');
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes.some(n => n.name === 'Grace Hopper' && n.focused));
  await record('row-keyboard-focus');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().chat_fixture.open === 'grace');

  // A switch turns over by pointer, Space and Enter.
  const notifications = await target('Notifications', 'switch');
  await page.mouse.click(notifications.point.x, notifications.point.y);
  await page.waitForFunction(() => !window.__STOBIE_GALLERY_HANDLE.snapshot().chat_fixture.notifications);
  let state = await record('switch-off');
  assert.equal(switches(state).find(n => n.name === 'Notifications').checked, false);
  await page.waitForFunction(() => document.querySelector('[id^="stobie-a11y:layer"] [aria-label="Notifications"]')?.getAttribute('aria-checked') === 'false', null, { timeout: 5000 });
  await page.keyboard.press('Space');
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().chat_fixture.notifications);
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => !window.__STOBIE_GALLERY_HANDLE.snapshot().chat_fixture.notifications);
  state = await record('switch-keyboard');
  assert.equal(switches(state).find(n => n.name === 'Notifications').checked, false);
  assert.equal((await fixture()).toggled, 3);

  // Desktop, light compact.
  await story('chat/conversation', configuration({ appearance: 'light', density: 'compact', width: 'wide' }));
  const light = await record('conversation-light');
  assert.equal(rows(light).length, 4);
  checkBubbles(light, 'light');
  // A regular column, light: the list above the conversation.
  await story('chat/conversation', configuration({ appearance: 'light' }));
  const regular = await record('conversation-regular-light');
  const listEnd = Math.max(...switches(regular).map(n => n.rect.max_y));
  assert(bubbles(regular).every(n => n.rect.min_y > listEnd), 'the conversation is below the list');
  checkBubbles(regular, 'regular');

  // Phone, dark: the list above the conversation.
  await story('chat/conversation', configuration({ width: 'narrow' }));
  const phone = await record('conversation-phone-dark');
  const listBottom = Math.max(...switches(phone).map(n => n.rect.max_y));
  assert(bubbles(phone).filter(n => n.rect.max_y > 0).every(n => n.rect.min_y > listBottom), 'the conversation is below the list');
  checkBubbles(phone, 'phone');

  // Phone, long and localised: elided rows with whole semantics, an
  // archived row, a long switch label, a disabled switch, and bubbles
  // holding a long link and code.
  for (const [name, config] of [
    ['long-dark', configuration({ width: 'narrow' })],
    ['long-light-high-contrast-150', configuration({ appearance: 'light', contrast: 'high', font_scale: 1.5, width: 'narrow' })],
  ]) {
    await story('chat/long-narrow', config);
    const long = await record(name);
    const titles = long.text.filter(t => t.component_id.kind === 'conversation_row_title');
    assert(titles.some(t => t.truncated), `${name}: a long title elides`);
    assert(rows(long).some(n => n.name === 'Augusta Ada King, Countess of Lovelace'), `${name}: whole name in semantics`);
    const archived = rows(long).find(n => n.name === 'Katherine Coleman Goble Johnson');
    if (archived) {
      assert.equal(archived.enabled, false);
      assert.equal(archived.disabled_reason, 'This conversation is archived');
    }
    for (const t of long.text.filter(t => t.component_id.kind === 'conversation_row_time')) assert(!t.truncated, `${name}: times stay whole`);
  }
  // Scroll to the switches and messages.
  const top = await snapshot();
  const first = rows(top)[0];
  const firstRow = await target(first.name, 'navigation_item');
  await page.mouse.move(firstRow.point.x, firstRow.point.y);
  for (let i = 0; i < 12; i += 1) {
    await page.mouse.wheel(0, 200);
    await page.waitForTimeout(80);
  }
  await page.waitForTimeout(300);
  const scrolled = await record('long-scrolled');
  const archive = switches(scrolled).find(n => n.name === 'Archive old conversations');
  assert(archive, 'the disabled switch scrolled into view');
  assert.deepEqual([archive.checked, archive.enabled, archive.disabled_reason], [true, false, 'The household\'s administrator sets this']);
  assert.equal(await control('switch', 'Archive old conversations').getAttribute('aria-disabled'), 'true');
  assert(bubbles(scrolled).length > 0, 'messages scrolled into view');
  const wrapped = scrolled.text.filter(t => t.component_id.kind === 'reading_text' && t.line_count > 1);
  assert(wrapped.length > 0, 'a long message wraps');

  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot()).frame, ms => page.waitForTimeout(ms));
  assert.deepEqual(errors, []);
  const report = { source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '', wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/stobie_gallery_bg.wasm'))).digest('hex'), url, browser: browser.version(), steps, targets, idle, input_route: 'Physical Playwright mouse/keyboard; hooks only choose stories/appearance and read observations', errors };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Chat browser workflow passed: rows, unread state, switches, bubbles, ARIA layer, both themes, desktop and phone widths, scrolling and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), state: await snapshot().catch(() => null), steps, targets }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await context?.close();
  if (server.listening) await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
