// The copy recipe in headful Chromium, under a test profile that grants
// clipboard access: a mouse click, a touch tap, Enter on the control
// focused through the ARIA layer and a click on its layer element each
// write the text inside that event's own dispatch, the clipboard then holds
// it, and the control
// confirms with a polite status; an agent copies through the application
// interface; a refused write shows the failure; and the page goes idle once
// the confirmation ends.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { chromium } from 'playwright';
import { assertBrowserRenderer, hostedLinuxWebGpuLaunchOptions, rendererUrl, uiLibraryEnvironment } from './browser-launch.mjs';
import { observeWarmedIdle } from './browser-idle.mjs';
import { ApplicationClient, BrowserTransport } from './application-client.mjs';

// The Gallery's copy/actions story, exactly as it presents them.
const CODE = 'QX7-4410-KM2P';
const MESSAGE = 'The meeting moved to Thursday at 10:30. Bring the printed agenda '
  + 'and the two reports; the room is on the third floor.';
const LINK = 'https://example.org/einladung/QX7-4410-KM2P';
const STORY = 'copy/actions';
const COPY_TEXT = 'stobie.copy-text';
const LAYER = '[id^="stobie-a11y:layer"]';

const root = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/copy-browser');
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

/**
 * Record every clipboard write with the event being dispatched when it was
 * made, and optionally refuse them all, as a browser without permission
 * would.
 */
function clipboardProbe(refuse) {
  window.__copyProbe = { writes: [], trusted: null };
  // The trusted input event being dispatched, if any: its phase returns to
  // none once its dispatch ends, including any synchronous copy the ARIA
  // layer forwards to the canvas within it.
  for (const type of ['pointerdown', 'pointerup', 'touchstart', 'touchend', 'keydown', 'keyup', 'click']) {
    window.addEventListener(type, event => { if (event.isTrusted) window.__copyProbe.trusted = event; }, true);
  }
  const original = Clipboard.prototype.writeText;
  Clipboard.prototype.writeText = function (text) {
    window.__copyProbe.writes.push({
      text,
      event: window.event?.type ?? null,
      trusted: window.__copyProbe.trusted?.eventPhase ? window.__copyProbe.trusted.type : null,
      active: navigator.userActivation?.isActive ?? null,
    });
    if (refuse) return Promise.reject(new DOMException('Write permission denied.', 'NotAllowedError'));
    return original.call(this, text);
  };
}

const steps = [];
const writes = [];
const errors = [];
let browser, url;

async function open(name, { touch = false, refuse = false } = {}) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, hasTouch: touch });
  if (!refuse) await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: url });
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(`${name}: ${error}`));
  await page.addInitScript(clipboardProbe, refuse);
  const response = await page.goto(rendererUrl(url), { waitUntil: 'domcontentloaded' });
  assert(response?.ok(), `Gallery HTTP status ${response?.status()}`);
  await page.waitForFunction(() => document.body.classList.contains('ready') && window.__STOBIE_GALLERY_HANDLE?.snapshot().frame > 0);
  await assertBrowserRenderer(page);
  await page.waitForSelector(`${LAYER} [role]`, { state: 'attached', timeout: 60000 });
  const before = await snapshot(page);
  await page.evaluate(id => window.__STOBIE_GALLERY_HANDLE.select_story(id), STORY);
  await page.waitForFunction(({ id, frame }) => {
    const s = window.__STOBIE_GALLERY_HANDLE.snapshot();
    return s.frame > frame && s.story === id;
  }, { id: STORY, frame: before.frame });
  return { context, page, name };
}
const snapshot = page => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot());
function centre(s, rect, canvas) {
  const root = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const x = value => canvas.x + (value - root.min_x) * canvas.width / (root.max_x - root.min_x);
  const y = value => canvas.y + (value - root.min_y) * canvas.height / (root.max_y - root.min_y);
  return { x: x((rect.min_x + rect.max_x) / 2), y: y((rect.min_y + rect.max_y) / 2) };
}
/** The copy control named `name`, with the point at its centre. */
async function control(page, name, index = 0) {
  const s = await snapshot(page);
  const nodes = s.ui_snapshot.nodes.filter(n => n.role === 'button' && n.name === name);
  assert(nodes[index], `copy control ${name}`);
  return { node: nodes[index], point: centre(s, nodes[index].rect, await page.locator('canvas').boundingBox()) };
}
const statuses = s => s.ui_snapshot.nodes.filter(n => n.role === 'status').map(n => n.name);
async function record(page, name) {
  const before = await snapshot(page);
  const bytes = await page.screenshot({ path: join(evidence, `browser-${name}.png`) });
  const after = await snapshot(page);
  assert.deepEqual(after.text_audit, [], `${name}: measured text`);
  assert.deepEqual(after.ui_snapshot.semantic_audit, [], `${name}: semantics`);
  steps.push({ name, frame: after.frame, before_capture_frame: before.frame, statuses: statuses(after), image: { path: `browser-${name}.png`, sha256: createHash('sha256').update(bytes).digest('hex') } });
  return after;
}
/** Take the writes made since the last call. */
async function taken(page, route) {
  const made = await page.evaluate(() => window.__copyProbe.writes.splice(0));
  writes.push(...made.map(write => ({ route, ...write, text_sha256: createHash('sha256').update(write.text).digest('hex'), text: undefined })));
  return made;
}
/** Wait for the control to show `message` as its status, in the snapshot and in the ARIA layer. */
async function confirms(page, message) {
  await page.waitForFunction(message => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes
    .some(n => n.role === 'status' && n.name === message), message, { timeout: 5000 });
  await page.locator(`${LAYER} [role="status"]`, { hasText: message }).first().waitFor({ state: 'attached', timeout: 5000 });
  // A status is polite unless it says otherwise.
  const status = page.locator(`${LAYER} [role="status"]`, { hasText: message });
  assert.equal(await status.count(), 1, `${message} is one status`);
  assert.equal(await status.getAttribute('aria-live'), null, `${message} is polite`);
}
async function clipboardHolds(page, expected) {
  await page.waitForFunction(expected => navigator.clipboard.readText().then(text => text === expected), expected, { timeout: 5000 })
    .catch(async error => {
      const held = await page.evaluate(() => navigator.clipboard.readText());
      throw new Error(`the clipboard holds ${JSON.stringify(held)}: ${error}`);
    });
}
/** The status ends after its time, and the control returns to its label. */
async function ends(page, message, seconds) {
  const started = Date.now();
  await page.waitForFunction(message => !window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes
    .some(n => n.role === 'status' && n.name === message), message, { timeout: (seconds + 3) * 1000 });
  const elapsed = (Date.now() - started) / 1000;
  assert(elapsed <= seconds + 1, `${message} ended after ${elapsed} s`);
  assert.equal(await page.locator(`${LAYER} [role="status"]`, { hasText: message }).count(), 0, `${message} left the ARIA layer`);
  return elapsed;
}

try {
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  url = `http://127.0.0.1:${server.address().port}/`;
  browser = await chromium.launch({ ...hostedLinuxWebGpuLaunchOptions(), executablePath: process.env.STOBIE_CHROMIUM, env: uiLibraryEnvironment() });

  // A mouse click writes inside its pointerup handler.
  const desktop = await open('desktop');
  let page = desktop.page;
  const story = await record(page, 'copy-actions-dark');
  assert.deepEqual(story.ui_snapshot.nodes.filter(n => n.role === 'button').map(n => [n.name, n.enabled]),
    [['Copy code', true], ['Copy message', true], ['Link kopieren', true], ['Copy code', false]]);
  assert.deepEqual(statuses(story), [], 'no status before an outcome');
  const code = await control(page, 'Copy code');
  await page.mouse.click(code.point.x, code.point.y);
  await confirms(page, 'Copied');
  let made = await taken(page, 'mouse');
  assert.equal(made.length, 1, 'one write per click');
  assert.deepEqual([made[0].text, made[0].event, made[0].trusted], [CODE, 'pointerup', 'pointerup'], 'the click writes inside its trusted pointerup');
  await clipboardHolds(page, CODE);
  const copied = await record(page, 'copied');
  // The button keeps its name; the confirmation is the status's.
  assert.equal(copied.ui_snapshot.nodes.find(n => n.id === code.node.id).name, 'Copy code');
  const confirmation = await ends(page, 'Copied', 2);
  steps.push({ name: 'confirmation-ended', seconds: confirmation });

  // The keyboard: focus through the ARIA layer, then Enter writes inside keydown.
  await page.locator(`${LAYER} [role="button"][aria-label="Link kopieren"]`).focus();
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes.some(n => n.name === 'Link kopieren' && n.focused));
  await page.keyboard.press('Enter');
  await confirms(page, 'In die Zwischenablage kopiert');
  made = await taken(page, 'keyboard');
  assert.equal(made.length, 1);
  assert.deepEqual([made[0].text, made[0].event, made[0].trusted], [LINK, 'keydown', 'keydown'], 'Enter writes inside its trusted keydown');
  await clipboardHolds(page, LINK);
  await record(page, 'keyboard-copied-localised');
  await ends(page, 'In die Zwischenablage kopiert', 2);

  // Assistive technology: a click on the control's layer element, as
  // VoiceOver's double tap sends, writes inside that click, once.
  await page.locator(`${LAYER} [role="button"][aria-label="Copy message"]`).evaluate(target => target.click());
  await confirms(page, 'Copied');
  made = await taken(page, 'layer-click');
  assert.equal(made.length, 1, 'one write per activation');
  assert.deepEqual([made[0].text, made[0].event], [MESSAGE, 'click'], 'the layer click writes inside its own dispatch');
  await clipboardHolds(page, MESSAGE);
  await ends(page, 'Copied', 2);

  // A disabled control writes nothing.
  const expired = await control(page, 'Copy code', 1);
  assert.equal(expired.node.enabled, false);
  assert.equal(expired.node.disabled_reason, 'The invitation has expired');
  await page.mouse.click(expired.point.x, expired.point.y);
  await page.waitForTimeout(500);
  assert.deepEqual(await taken(page, 'disabled'), [], 'a disabled control writes nothing');

  // An agent copies through the application interface, outside any gesture,
  // which this profile's permission allows.
  await page.evaluate(() => {
    window.__STOBIE_GALLERY_HANDLE.enable_automation(crypto.randomUUID());
    window.__STOBIE_AUTOMATION = Object.freeze({ request: json => window.__STOBIE_GALLERY_HANDLE.automation_request(json) });
  });
  // Automation publishes from the next pass on, so a move asks for one.
  await page.mouse.move(1400, 870);
  const client = new ApplicationClient(new BrowserTransport(page));
  await client.hello();
  let discovered;
  for (let attempt = 0; ; attempt += 1) {
    try {
      discovered = await client.discover({ capability: COPY_TEXT });
      break;
    } catch (error) {
      if (error.code !== 'missing_observation' || attempt === 20) throw error;
      await page.waitForTimeout(100);
    }
  }
  assert.equal(discovered.capabilities.length, 4, 'four copy controls');
  const message = await control(page, 'Copy message');
  const capability = discovered.capabilities.find(c => c.target.id === message.node.id);
  assert(capability, 'the message control is discoverable');
  assert.match(capability.target.meaning, /^text-revision:\d+$/, 'the meaning names a revision, never the text');
  assert(!JSON.stringify(discovered).includes(MESSAGE.slice(0, 24)), 'discovery never carries the text');
  const receipt = await client.invoke({ id: message.node.id, capability: COPY_TEXT });
  assert.equal(receipt.state, 'completed', JSON.stringify(receipt));
  await confirms(page, 'Copied');
  made = await taken(page, 'agent');
  assert.equal(made.length, 1);
  assert.equal(made[0].text, MESSAGE);
  await clipboardHolds(page, MESSAGE);
  await ends(page, 'Copied', 2);

  // Idle once every outcome has ended.
  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot(page)).frame, ms => page.waitForTimeout(ms));
  await desktop.context.close();

  // A touch tap writes inside its pointerup, before touchend.
  const phone = await open('touch', { touch: true });
  page = phone.page;
  const tapped = await control(page, 'Copy code');
  await page.touchscreen.tap(tapped.point.x, tapped.point.y);
  await confirms(page, 'Copied');
  made = await taken(page, 'touch');
  assert.equal(made.length, 1, 'one write per tap');
  assert.deepEqual([made[0].text, made[0].event, made[0].trusted], [CODE, 'pointerup', 'pointerup'], 'the tap writes inside its trusted pointerup, before touchend');
  await clipboardHolds(page, CODE);
  await record(page, 'touch-copied');
  await phone.context.close();

  // A refused write shows the failure, for its own time.
  const refused = await open('refused', { refuse: true });
  page = refused.page;
  const failing = await control(page, 'Copy message');
  await page.mouse.click(failing.point.x, failing.point.y);
  await confirms(page, "Couldn't copy");
  made = await taken(page, 'refused');
  assert.equal(made.length, 1);
  await record(page, 'refused');
  const failure = await ends(page, "Couldn't copy", 5);
  steps.push({ name: 'failure-ended', seconds: failure });
  await refused.context.close();

  assert.deepEqual(errors, []);
  const report = {
    source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '',
    wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/stobie_gallery_bg.wasm'))).digest('hex'),
    url, browser: browser.version(), steps, writes, idle,
    input_route: 'Physical Playwright mouse, touch screen and keyboard, focus and a click through the ARIA layer, and the application interface; hooks only choose the story and read observations',
    profile: 'clipboard-read and clipboard-write granted, except the refused context, whose writeText rejects',
    errors,
  };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Copy browser workflow passed: mouse, touch, keyboard and layer activation write inside their events, agent copy, disabled, refused and idle');
} catch (error) {
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), steps, writes, errors }, null, 2));
  throw error;
} finally {
  await browser?.close();
  if (server.listening) await new Promise(done => server.close(done));
}
