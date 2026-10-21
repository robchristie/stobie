// Modal dialogs in the Gallery, in headful Chromium with physical input: the
// routine editor is a centred dialog on a desktop and a bottom sheet on a
// phone. Focus moves into it and stays there under Tab and Shift+Tab, text
// typed reaches its fields, the scrim and the controls behind it do nothing,
// an agent is refused what is behind it, Escape and the close control
// dismiss it and focus returns to its opener. The confirm focuses its cancel
// first, works by keyboard and pointer, and Escape cancels it. The ARIA
// layer shows the dialog as modal and nothing behind it.
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

const SHEET = 'dialog/sheet';
const CONFIRM = 'dialog/confirm';
const EDITOR = 'Edit routine';
const DELETE_TITLE = 'Delete “Morning briefing”?';
const EDITOR_CONTROLS = ['Name', 'Prompt', 'Send a notification', 'Save', 'Close'];
const REFUSAL = 'The dialog "Edit routine" is open';
const LAYER = '[id^="stobie-a11y:layer"]';

const root = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/dialog-browser');
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

const steps = [];
const errors = [];
let browser, url, page;

const snapshot = page => page.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot());
const fixture = async page => (await snapshot(page)).dialog_fixture;
const dialogNode = s => s.ui_snapshot.nodes.find(n => n.role === 'dialog' || n.role === 'alert_dialog');
const focused = s => s.ui_snapshot.nodes.find(n => n.focused)?.name;
const node = (s, name) => s.ui_snapshot.nodes.find(n => n.name === name && n.role !== 'text');
const layer = page => page.locator(LAYER);

async function open(name, { width = 1440, height = 900, touch = false } = {}) {
  const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1, hasTouch: touch });
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(`${name}: ${error}`));
  const response = await page.goto(rendererUrl(url), { waitUntil: 'domcontentloaded' });
  assert(response?.ok(), `Gallery HTTP status ${response?.status()}`);
  await page.waitForFunction(() => document.body.classList.contains('ready') && window.__STOBIE_GALLERY_HANDLE?.snapshot().frame > 0);
  await assertBrowserRenderer(page);
  await page.waitForSelector(`${LAYER} [role]`, { state: 'attached', timeout: 60000 });
  return { context, page };
}
/**
 * Select `id` afresh, which resets its state; each dialog story opens its
 * dialog when first shown. Selecting the shown story changes nothing, so the
 * icon vocabulary is selected first.
 */
async function story(page, id) {
  if ((await snapshot(page)).story === id) await story(page, 'icons/vocabulary');
  const before = await snapshot(page);
  await page.evaluate(id => window.__STOBIE_GALLERY_HANDLE.select_story(id), id);
  await page.waitForFunction(({ id, frame }) => {
    const s = window.__STOBIE_GALLERY_HANDLE.snapshot();
    return s.frame > frame && s.story === id;
  }, { id, frame: before.frame });
  await page.waitForTimeout(300);
}
/** Wait until the snapshot holds the dialog titled `title`, or none. */
async function dialog(page, title) {
  await page.waitForFunction(title => {
    const found = window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes
      .find(n => n.role === 'dialog' || n.role === 'alert_dialog');
    return title === null ? !found : found?.name === title;
  }, title, { timeout: 10000 });
}
async function focusOn(page, name) {
  await page.waitForFunction(name => window.__STOBIE_GALLERY_HANDLE.snapshot().ui_snapshot.nodes
    .find(n => n.focused)?.name === name, name, { timeout: 10000 })
    .catch(async error => { throw new Error(`focus is on ${focused(await snapshot(page))}, not ${name}: ${error}`); });
}
/** The centre of the node named `name`, in page coordinates. */
async function point(page, name) {
  const s = await snapshot(page);
  const found = node(s, name);
  assert(found, `visible ${name}`);
  const root = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const canvas = await page.locator('canvas').boundingBox();
  const r = found.rect;
  return {
    x: canvas.x + ((r.min_x + r.max_x) / 2 - root.min_x) * canvas.width / (root.max_x - root.min_x),
    y: canvas.y + ((r.min_y + r.max_y) / 2 - root.min_y) * canvas.height / (root.max_y - root.min_y),
  };
}
async function click(page, name) {
  const { x, y } = await point(page, name);
  await page.mouse.click(x, y);
}
async function record(page, name) {
  const before = await snapshot(page);
  const bytes = await page.screenshot({ path: join(evidence, `browser-${name}.png`) });
  const after = await snapshot(page);
  assert.deepEqual(after.text_audit, [], `${name}: measured text`);
  assert.deepEqual(after.ui_snapshot.semantic_audit, [], `${name}: semantics`);
  const shown = dialogNode(after);
  steps.push({ name, frame: after.frame, before_capture_frame: before.frame, dialog: shown ? { role: shown.role, name: shown.name, rect: shown.rect } : null, focused: focused(after), fixture: after.dialog_fixture, image: { path: `browser-${name}.png`, sha256: createHash('sha256').update(bytes).digest('hex') } });
  return after;
}
/** While a dialog is open, the ARIA layer holds it, modal, and nothing behind it. */
async function layerShowsOnly(page, role, title, inside) {
  const modal = layer(page).locator(`[role="${role}"]`);
  await modal.first().waitFor({ state: 'attached', timeout: 10000 });
  assert.equal(await modal.count(), 1, `one ${role}`);
  assert.equal(await modal.getAttribute('aria-modal'), 'true', `${title} is modal`);
  assert.equal(await modal.getAttribute('aria-label'), title);
  for (const name of inside) {
    assert.equal(await modal.getByRole('button', { name, exact: true }).or(modal.getByRole('textbox', { name, exact: true })).or(modal.getByRole('switch', { name, exact: true })).count(), 1, `${name} is inside the ${role}`);
  }
  for (const behind of ['Appearance workbench', 'button/default', EDITOR, 'Delete routine']) {
    if (inside.includes(behind)) continue;
    assert.equal(await layer(page).getByRole('button', { name: behind, exact: true }).count(), 0, `${behind} is hidden behind the ${role}`);
  }
}
/**
 * Press Tab (or Shift+Tab) `count` times, returning each focused control.
 * Each press waits until the Gallery has counted it and focus rests on a named
 * control other than the one it left: egui moves focus a pass after it reads
 * the key, so the pass that counts it can still show the old control, or none.
 */
async function tabs(page, key, count) {
  const visited = [];
  for (let i = 0; i < count; i += 1) {
    const before = await snapshot(page);
    const from = focused(before) ?? null;
    await page.keyboard.press(key);
    await page.waitForFunction(({ epoch, from }) => {
      const s = window.__STOBIE_GALLERY_HANDLE.snapshot();
      return s.tab_input_epoch > epoch && s.ui_snapshot.nodes.some(n => n.focused && n.name && n.name !== from);
    }, { epoch: before.tab_input_epoch, from }, { timeout: 10000 })
      .catch(async error => { throw new Error(`${key} from ${from} left focus on ${focused(await snapshot(page)) ?? 'no control'} after ${visited.join(',')}: ${error}`); });
    visited.push(focused(await snapshot(page)));
  }
  return visited;
}

try {
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  url = `http://127.0.0.1:${server.address().port}/`;
  browser = await chromium.launch({ ...hostedLinuxWebGpuLaunchOptions(), executablePath: process.env.STOBIE_CHROMIUM, env: uiLibraryEnvironment() });

  // A desktop: the editor is a dialog centred over the whole window, and
  // focus starts in its first field.
  const desktop = await open('desktop');
  page = desktop.page;
  await story(page, SHEET);
  await dialog(page, EDITOR);
  await focusOn(page, 'Name');
  let s = await record(page, 'desktop-editor');
  const screenRect = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const surface = dialogNode(s).rect;
  assert(Math.abs((surface.min_x + surface.max_x) / 2 - (screenRect.min_x + screenRect.max_x) / 2) <= 1, 'centred across');
  assert(Math.abs((surface.min_y + surface.max_y) / 2 - (screenRect.min_y + screenRect.max_y) / 2) <= 1, 'centred down');
  assert(surface.max_x - surface.min_x <= 561, 'no wider than a dialog');
  await layerShowsOnly(page, 'dialog', EDITOR, EDITOR_CONTROLS);

  // The scrim does nothing, and neither does the story list behind it.
  const canvas = await page.locator('canvas').boundingBox();
  await page.mouse.click(canvas.x + 60, canvas.y + 150);
  await page.waitForTimeout(300);
  s = await snapshot(page);
  assert.equal(s.story, SHEET, 'the story list behind the scrim is unreachable');
  assert.equal(dialogNode(s)?.name, EDITOR, 'the scrim does not dismiss');

  // Typing reaches the single-line field and the multiline one.
  await click(page, 'Name');
  await focusOn(page, 'Name');
  await page.keyboard.press('End');
  await page.keyboard.type(' at dawn');
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().dialog_fixture.name === 'Morning briefing at dawn', null, { timeout: 5000 });
  // Tab and Shift+Tab reach every control in the dialog and nothing else.
  let visited = await tabs(page, 'Tab', 6);
  assert.deepEqual(new Set(visited), new Set(EDITOR_CONTROLS), `Tab stays in the dialog: ${visited}`);
  assert.equal(visited[0], 'Prompt', 'Tab goes from Name to Prompt');
  assert.equal(visited.at(-1), 'Prompt', 'Tab wraps');
  await page.keyboard.press('End');
  await page.keyboard.type(' Keep it short.');
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().dialog_fixture.prompt.endsWith(' Keep it short.'), null, { timeout: 5000 });
  visited = await tabs(page, 'Shift+Tab', 6);
  assert.deepEqual(new Set(visited), new Set(EDITOR_CONTROLS), `Shift+Tab stays in the dialog: ${visited}`);
  s = await record(page, 'desktop-typed');

  // An agent is refused what is behind the dialog.
  await page.evaluate(() => {
    window.__STOBIE_GALLERY_HANDLE.enable_automation(crypto.randomUUID());
    window.__STOBIE_AUTOMATION = Object.freeze({ request: json => window.__STOBIE_GALLERY_HANDLE.automation_request(json) });
  });
  await page.mouse.move(1400, 870);
  const client = new ApplicationClient(new BrowserTransport(page));
  await client.hello();
  let discovered;
  for (let attempt = 0; ; attempt += 1) {
    try {
      discovered = await client.discover({ capability: 'gallery.open-workbench' });
      break;
    } catch (error) {
      if (error.code !== 'missing_observation' || attempt === 20) throw error;
      await page.waitForTimeout(100);
    }
  }
  assert.deepEqual(discovered.capabilities.map(c => c.availability), [{ state: 'disabled', reason: REFUSAL }]);
  const refused = await client.invoke({ capability: 'gallery.open-workbench' });
  assert.equal(refused.state, 'rejected', JSON.stringify(refused));
  assert.equal(refused.error.code, 'unavailable', JSON.stringify(refused));
  steps.push({ name: 'agent-refused', receipt: refused });
  assert.equal(dialogNode(await snapshot(page))?.name, EDITOR, 'the refusal changed nothing');

  // Escape dismisses. The story opened this dialog itself, so it has no
  // opener to return focus to.
  await page.keyboard.press('Escape');
  await dialog(page, null);
  s = await record(page, 'desktop-dismissed');
  assert.deepEqual([s.dialog_fixture.open, s.dialog_fixture.saved], [false, 0]);
  assert.equal(await layer(page).locator('[role="dialog"]').count(), 0, 'the layer drops the dialog');
  await layer(page).getByRole('button', { name: 'Appearance workbench', exact: true }).waitFor({ state: 'attached', timeout: 5000 });

  // Focus the opener through the ARIA layer, as assistive technology may;
  // Enter opens the dialog, and Escape returns focus to the opener.
  await layer(page).getByRole('button', { name: EDITOR, exact: true }).focus();
  await focusOn(page, EDITOR);
  await page.keyboard.press('Enter');
  await dialog(page, EDITOR);
  await focusOn(page, 'Name');
  await page.keyboard.press('Escape');
  await dialog(page, null);
  await focusOn(page, EDITOR);
  await record(page, 'desktop-focus-returned');

  // Enter on the opener opens it again; the pointer saves.
  await page.keyboard.press('Enter');
  await dialog(page, EDITOR);
  await focusOn(page, 'Name');
  await click(page, 'Save');
  await dialog(page, null);
  assert.deepEqual([(await fixture(page)).open, (await fixture(page)).saved], [false, 1]);
  await focusOn(page, EDITOR);

  // The pointer opens it, and its close control dismisses it.
  await click(page, EDITOR);
  await dialog(page, EDITOR);
  await click(page, 'Close');
  await dialog(page, null);
  await focusOn(page, EDITOR);
  assert.equal((await fixture(page)).saved, 1, 'closing saves nothing');

  // The confirm: cancel is focused first. Escape cancels the one the
  // story opened; Delete routine opens it again, and Enter on its focused
  // cancel keeps the routine and returns focus to the opener.
  await story(page, CONFIRM);
  await dialog(page, DELETE_TITLE);
  await focusOn(page, 'Keep routine');
  s = await record(page, 'desktop-confirm');
  await layerShowsOnly(page, 'alertdialog', DELETE_TITLE, ['Keep routine', 'Delete']);
  await page.keyboard.press('Escape');
  await dialog(page, null);
  await layer(page).getByRole('button', { name: 'Delete routine', exact: true }).focus();
  await focusOn(page, 'Delete routine');
  await page.keyboard.press('Enter');
  await dialog(page, DELETE_TITLE);
  await focusOn(page, 'Keep routine');
  await page.keyboard.press('Enter');
  await dialog(page, null);
  await focusOn(page, 'Delete routine');
  assert.deepEqual([(await fixture(page)).open, (await fixture(page)).deleted], [false, false], 'Enter on cancel keeps the routine');
  // Escape cancels.
  await page.keyboard.press('Enter');
  await dialog(page, DELETE_TITLE);
  await focusOn(page, 'Keep routine');
  await page.keyboard.press('Escape');
  await dialog(page, null);
  await focusOn(page, 'Delete routine');
  assert.equal((await fixture(page)).deleted, false, 'Escape keeps the routine');
  // Tab reaches Delete, and wraps to cancel.
  await page.keyboard.press('Enter');
  await dialog(page, DELETE_TITLE);
  await focusOn(page, 'Keep routine');
  visited = await tabs(page, 'Tab', 2);
  assert.deepEqual(visited, ['Delete', 'Keep routine'], 'Tab stays in the confirm');
  await tabs(page, 'Tab', 1);
  await page.keyboard.press('Enter');
  await dialog(page, null);
  s = await record(page, 'desktop-deleted');
  assert.equal(s.dialog_fixture.deleted, true, 'Enter on Delete deletes');
  assert.equal(node(s, 'Delete routine').enabled, false, 'the opener is now disabled');
  // The pointer confirms, from a fresh story.
  await story(page, CONFIRM);
  await dialog(page, DELETE_TITLE);
  await click(page, 'Delete');
  await dialog(page, null);
  assert.equal((await fixture(page)).deleted, true, 'a click on Delete deletes');

  await page.mouse.move(1400, 880);
  const idle = await observeWarmedIdle(async () => (await snapshot(page)).frame, ms => page.waitForTimeout(ms));
  await desktop.context.close();

  // A phone: the editor is a sheet along the bottom, the window's width.
  const phone = await open('phone', { width: 390, height: 844, touch: true });
  page = phone.page;
  await story(page, SHEET);
  await dialog(page, EDITOR);
  s = await record(page, 'phone-sheet');
  const screen = s.ui_snapshot.nodes.find(n => n.id === s.ui_snapshot.root).rect;
  const sheet = dialogNode(s).rect;
  assert(Math.abs(sheet.max_y - screen.max_y) <= 1, `the sheet meets the bottom: ${JSON.stringify([sheet, screen])}`);
  assert(Math.abs(sheet.min_x - screen.min_x) <= 1 && Math.abs(sheet.max_x - screen.max_x) <= 1, 'the sheet spans the width');
  await layerShowsOnly(page, 'dialog', EDITOR, EDITOR_CONTROLS);
  // A tap in the prompt types there; a tap on Close dismisses.
  const prompt = await point(page, 'Prompt');
  await page.touchscreen.tap(prompt.x, prompt.y);
  await focusOn(page, 'Prompt');
  // The tap makes the prompt the native field, the topmost element over
  // the sheet, as on iOS; typing there reaches egui.
  const native = name => page.waitForFunction(name => {
    const element = document.activeElement;
    if (element?.getAttribute('aria-label') !== name || !element.hasAttribute('data-stobie-native-active')) return null;
    const box = element.getBoundingClientRect();
    return document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2) === element;
  }, name, { timeout: 5000 });
  await native('Prompt');
  await page.keyboard.press('End');
  await page.keyboard.type(' Tapped.');
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().dialog_fixture.prompt.endsWith(' Tapped.'), null, { timeout: 5000 });
  const name = await point(page, 'Name');
  await page.touchscreen.tap(name.x, name.y);
  await focusOn(page, 'Name');
  await native('Name');
  await page.keyboard.press('End');
  await page.keyboard.type(' too');
  await page.waitForFunction(() => window.__STOBIE_GALLERY_HANDLE.snapshot().dialog_fixture.name.endsWith(' too'), null, { timeout: 5000 });
  const close = await point(page, 'Close');
  await page.touchscreen.tap(close.x, close.y);
  await dialog(page, null);
  await record(page, 'phone-dismissed');
  await phone.context.close();

  assert.deepEqual(errors, []);
  const report = {
    source_revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '',
    wasm_sha256: createHash('sha256').update(await readFile(join(root, 'pkg/stobie_gallery_bg.wasm'))).digest('hex'),
    url, browser: browser.version(), steps, idle,
    input_route: 'Physical Playwright mouse, touch screen and keyboard, and the application interface; hooks only choose the story and read observations',
    errors,
  };
  await writeFile(join(evidence, 'browser-workflow.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log('Dialog browser workflow passed: centred dialog and bottom sheet, focus in, Tab containment, typing, scrim, agent refusal, Escape and close with focus returned, confirm by keyboard and pointer, native fields in the phone sheet, ARIA modal and idle');
} catch (error) {
  const state = await page?.evaluate(() => window.__STOBIE_GALLERY_HANDLE.snapshot()).catch(() => null);
  const dom = await page?.evaluate(layer => ({
    active: document.activeElement && { tag: document.activeElement.tagName, id: document.activeElement.id, label: document.activeElement.getAttribute('aria-label') },
    fields: [...document.querySelectorAll(`${layer} input, ${layer} textarea`)].map(e => ({ tag: e.tagName, id: e.id, label: e.getAttribute('aria-label') })),
  }), LAYER).catch(() => null);
  await writeFile(join(evidence, 'browser-failure.json'), JSON.stringify({ error: String(error), dom, state: state && { story: state.story, fixture: state.dialog_fixture, focused: state.ui_snapshot.nodes.find(n => n.focused)?.name, nodes: state.ui_snapshot.nodes.map(n => [n.role, n.name]) }, steps, errors }, null, 2));
  await page?.screenshot({ path: join(evidence, 'browser-failure.png') }).catch(() => {});
  throw error;
} finally {
  await browser?.close();
  if (server.listening) await new Promise(done => server.close(done));
}
