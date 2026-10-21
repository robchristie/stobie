// The composer from outside the repository: the `outside-composer`
// application (tools/composer-app/lib.rs) edits a draft it owns in a
// composer whose Send carries the draft and is hidden while the draft is
// empty. Run natively and in Chromium, an agent sets the draft with
// `stobie set-text` and sends it by Send's ID or by its text; a refused send
// keeps the draft. In headful Chromium, typing with the keyboard, Shift+Enter
// for a newline and Enter to send, and a mouse press on Send, send the draft
// and keep the field focused. On a touch screen, a tap makes the field the
// platform's own, Return there types a newline, and a tap on Send sends and
// leaves that field focused, so a phone keeps its keyboard up. Run inside the
// smoke environment (tools/outside-app-smoke.sh).
//
// node tools/composer-smoke.mjs STOBIE APP_DIR NATIVE_BINARY EVIDENCE_DIR
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { resolve } from 'node:path';

const [stobie, app, binary, evidence] = process.argv.slice(2);
assert(stobie && app && binary && evidence, 'usage: STOBIE APP_DIR NATIVE_BINARY EVIDENCE_DIR');
await mkdir(evidence, { recursive: true });
const name = 'outside-composer';
const LAYER = '[id^="stobie-a11y:layer"]';

function cli(args) {
  const result = spawnSync(stobie, [...args, '--json'], { encoding: 'utf8', timeout: 180000 });
  let body;
  try { body = JSON.parse(result.stdout || result.stderr); } catch { body = null; }
  return { status: result.status, body, stderr: result.stderr };
}

function ok(args) {
  const result = cli(args);
  assert.equal(result.status, 0, `stobie ${args.join(' ')}: ${result.stderr}`);
  return result.body;
}

/// The composer's field in an observation.
function field(observation) {
  const fields = observation.nodes.filter(node => node.role === 'multiline_text_input' && node.name === 'Message');
  assert.equal(fields.length, 1, 'one message field');
  return fields[0];
}

/// The controls of one action in an observation, with the argument each carries.
function controls(observation, action) {
  return observation.nodes
    .filter(node => node.capabilities.includes(`${name}.${action}`))
    .map(node => ({ id: node.id, name: node.name, argument: node.argument }));
}

// The same journey on every host, through `stobie` alone.
function journey(session) {
  const at = args => ok([...args, '--session', session]);
  const draft = text => {
    at(['set-text', message.id, text]);
    at(['wait', 'fact', '--key', 'draft', '--value', text]);
    const [send] = controls(at(['observe']), 'send');
    assert.equal(send?.argument, text, 'Send carries the draft');
    return send;
  };
  const before = at(['observe']);
  assert.equal(before.application.name, 'Outside Composer');
  assert.deepEqual([before.facts.draft, before.facts.sent], ['', 0]);
  const message = field(before);
  // An empty draft shows the microphone alone.
  assert.deepEqual(controls(before, 'send'), [], 'Send is hidden while the draft is empty');
  assert.deepEqual(controls(before, 'dictate').map(control => control.name), ['Dictate']);
  const empty = cli(['invoke', `${name}.send`, '--value', '', '--session', session]);
  assert.equal(empty.status, 1, empty.stderr);
  assert.equal(empty.body.receipt.error.code, 'unavailable', JSON.stringify(empty.body.receipt));

  // Send by its control's ID: the control carries the draft.
  const first = draft('Book the 9:30 train');
  const sent = at(['invoke', `${name}.send`, '--id', first.id]);
  assert.equal(sent.receipt.state, 'completed', JSON.stringify(sent.receipt));
  at(['wait', 'fact', '--key', 'sent', '--value', '1']);
  let facts = at(['observe']).facts;
  assert.deepEqual([facts.last, facts.draft], ['Book the 9:30 train', ''], 'the send cleared the draft');
  // Send by the draft's text: the control that carries it.
  draft('Two lines,\nas typed');
  const byValue = at(['invoke', `${name}.send`, '--value', 'Two lines,\nas typed']);
  assert.equal(byValue.receipt.state, 'completed', JSON.stringify(byValue.receipt));
  at(['wait', 'fact', '--key', 'sent', '--value', '2']);

  // A refused send keeps the draft.
  at(['invoke', `${name}.limit`]);
  at(['wait', 'fact', '--key', 'limited', '--value', 'true']);
  const held = draft('Over the limit');
  const refused = cli(['invoke', `${name}.send`, '--id', held.id, '--session', session]);
  assert.notEqual(refused.body?.receipt?.state, 'completed', JSON.stringify(refused.body));
  assert.equal(refused.body.receipt.error.message, 'Daily message limit reached');
  facts = at(['observe']).facts;
  assert.deepEqual([facts.draft, facts.sent], ['Over the limit', 2], 'the refused draft stays');
  at(['invoke', `${name}.limit`]);
  at(['wait', 'fact', '--key', 'limited', '--value', 'false']);
  const [again] = controls(at(['observe']), 'send');
  assert.equal(at(['invoke', `${name}.send`, '--id', again.id]).receipt.state, 'completed');
  at(['wait', 'fact', '--key', 'sent', '--value', '3']);
  const after = at(['observe']).facts;
  assert.deepEqual([after.last, after.draft], ['Over the limit', '']);
  return { before: before.facts, after, field: message.id };
}

// A centre point of an element's box, for a press that lands on the canvas
// beneath the transparent layer.
async function centre(locator) {
  const box = await locator.boundingBox();
  assert(box && box.width > 8 && box.height > 8, 'the control has a box');
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

// With a keyboard and mouse: type, Shift+Enter, Enter, and a press on Send.
async function keyboard(page, session) {
  const at = args => ok([...args, '--session', session]);
  const layer = page.locator(LAYER);
  await layer.locator('[role]').first().waitFor({ state: 'attached', timeout: 60000 });
  const message = field(at(['observe']));
  const area = page.locator(`textarea[aria-label="Message"][id^="stobie-a11y:"]`);
  const button = label => page.getByRole('button', { name: label, exact: true });
  assert.equal(await button('Send').count(), 0, 'no Send while the draft is empty');
  assert.equal(await button('Dictate').count(), 1);
  const box = await area.boundingBox();
  await page.mouse.click(box.x + 12, box.y + box.height / 2);
  at(['wait', 'focused', '--id', message.id]);
  await page.keyboard.type('Hello Ada', { delay: 20 });
  await page.keyboard.press('Shift+Enter');
  await page.keyboard.type('two lines', { delay: 20 });
  at(['wait', 'fact', '--key', 'draft', '--value', 'Hello Ada\ntwo lines']);
  assert.equal(await button('Send').count(), 1, 'Send shows for a draft');
  await page.locator('canvas').screenshot({ path: resolve(evidence, 'composer.png') });
  // Enter sends; the field keeps focus, so typing continues.
  await page.keyboard.press('Enter');
  at(['wait', 'fact', '--key', 'sent', '--value', '1']);
  let facts = at(['observe']).facts;
  assert.deepEqual([facts.last, facts.draft], ['Hello Ada\ntwo lines', ''], 'Enter sent the draft and typed no newline');
  at(['wait', 'focused', '--id', message.id]);
  await page.keyboard.type('Pressed', { delay: 20 });
  at(['wait', 'fact', '--key', 'draft', '--value', 'Pressed']);
  // A press on Send sends, and the field keeps focus.
  const send = await centre(button('Send'));
  await page.mouse.click(send.x, send.y);
  at(['wait', 'fact', '--key', 'sent', '--value', '2']);
  at(['wait', 'focused', '--id', message.id]);
  await page.keyboard.type('Still here', { delay: 20 });
  at(['wait', 'fact', '--key', 'draft', '--value', 'Still here']);
  // A refused send keeps the draft, and Enter does nothing more.
  at(['invoke', `${name}.limit`]);
  at(['wait', 'fact', '--key', 'limited', '--value', 'true']);
  await page.keyboard.press('Enter');
  at(['wait', 'fact', '--key', 'error', '--value', 'Daily message limit reached']);
  facts = at(['observe']).facts;
  assert.deepEqual([facts.draft, facts.sent], ['Still here', 2], 'the refused draft stays');
  return { sent: facts.sent, field: message.id };
}

// On a touch screen the field becomes the platform's own: Return types a
// newline, and a tap on Send sends and keeps that field focused.
async function touch(page, session) {
  const at = args => ok([...args, '--session', session]);
  await page.locator(LAYER).locator('[role]').first().waitFor({ state: 'attached', timeout: 60000 });
  const area = page.locator(`textarea[aria-label="Message"][id^="stobie-a11y:"]`);
  const box = await area.boundingBox();
  await page.touchscreen.tap(box.x + 12, box.y + box.height / 2);
  const active = () => page.waitForFunction(() => {
    const element = document.activeElement;
    return element?.tagName === 'TEXTAREA' && element.getAttribute('aria-label') === 'Message'
      && element.hasAttribute('data-stobie-native-active');
  }, null, { timeout: 5000 });
  await active();
  await page.keyboard.insertText('From the phone');
  at(['wait', 'fact', '--key', 'draft', '--value', 'From the phone']);
  // Return is the platform's: it types a newline and sends nothing.
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('second line');
  at(['wait', 'fact', '--key', 'draft', '--value', 'From the phone\nsecond line']);
  assert.equal(at(['observe']).facts.sent, 0, 'Return on a touch screen sends nothing');
  await page.waitForTimeout(300);
  // The platform's field wraps as egui does, so the grown field holds it.
  const fit = await page.evaluate(() => {
    const element = document.activeElement;
    return { overflow: element.scrollHeight - element.clientHeight, scrollbar: element.offsetWidth - element.clientWidth };
  });
  assert.deepEqual(fit, { overflow: 0, scrollbar: 0 }, 'the native field fits its text without scrolling');
  await page.locator('canvas').screenshot({ path: resolve(evidence, 'composer-touch.png') });
  const send = await centre(page.getByRole('button', { name: 'Send', exact: true }));
  await page.touchscreen.tap(send.x, send.y);
  at(['wait', 'fact', '--key', 'sent', '--value', '1']);
  const facts = at(['observe']).facts;
  assert.deepEqual([facts.last, facts.draft], ['From the phone\nsecond line', '']);
  // The platform's field stays focused and empty, so the keyboard stays up.
  await page.waitForTimeout(500);
  await active();
  assert.equal(await page.evaluate(() => document.activeElement.value), '', 'the native field cleared');
  await page.keyboard.insertText('And again');
  at(['wait', 'fact', '--key', 'draft', '--value', 'And again']);
  // A hardware keyboard's Ctrl+Return still sends from the platform's field.
  await page.keyboard.press('Control+Enter');
  at(['wait', 'fact', '--key', 'sent', '--value', '2']);
  assert.equal(at(['observe']).facts.last, 'And again');
  await active();
  return { sent: 2 };
}

const report = { app: name, native: null, browser: null, keyboard: null, touch: null };
try {
  ok(['launch', 'composer-native', '--', binary]);
  try {
    report.native = journey('composer-native');
  } finally {
    ok(['stop', 'composer-native']);
  }

  const server = spawn(stobie, ['serve', resolve(app, 'web'), '--port', '0', '--json'], { stdio: ['ignore', 'pipe', 'inherit'] });
  const [line] = await Promise.race([
    once(createInterface({ input: server.stdout }), 'line'),
    once(server, 'exit').then(([code]) => { throw new Error(`stobie serve exited with ${code} before listening`); }),
    new Promise((_, reject) => setTimeout(() => reject(new Error('stobie serve did not start within 30 s')), 30000).unref()),
  ]);
  const url = JSON.parse(line).url;
  const { chromium } = await import('playwright');
  const launch = await import('./browser-launch.mjs');
  try {
    const options = launch.hostedLinuxWebGpuLaunchOptions();
    const port = await launch.freeLoopbackPort();
    const browser = await chromium.launch({ ...options, args: [...options.args, `--remote-debugging-port=${port}`], env: launch.uiLibraryEnvironment() });
    try {
      const pageUrl = launch.rendererUrl(`${url}?automation=1`);
      const errors = [];
      const open = async context => {
        const page = await context.newPage();
        page.on('pageerror', error => errors.push(String(error)));
        await page.goto(pageUrl, { waitUntil: 'commit' });
        return page;
      };
      const desktop = await browser.newContext({ viewport: { width: 1000, height: 800 } });
      let page = await open(desktop);
      ok(['attach', 'composer-keyboard', '--cdp', `http://127.0.0.1:${port}`, '--page', pageUrl]);
      report.keyboard = await keyboard(page, 'composer-keyboard');
      ok(['stop', 'composer-keyboard']);
      // A fresh start for the agent's journey.
      await page.reload({ waitUntil: 'commit' });
      ok(['attach', 'composer-browser', '--cdp', `http://127.0.0.1:${port}`, '--page', pageUrl]);
      report.browser = journey('composer-browser');
      ok(['stop', 'composer-browser']);
      await desktop.close();
      // A phone's width with a touch screen.
      const phone = await browser.newContext({ viewport: { width: 390, height: 760 }, hasTouch: true });
      page = await open(phone);
      ok(['attach', 'composer-touch', '--cdp', `http://127.0.0.1:${port}`, '--page', pageUrl]);
      report.touch = await touch(page, 'composer-touch');
      ok(['stop', 'composer-touch']);
      await phone.close();
      assert.deepEqual(errors, [], 'no page errors');
    } finally {
      await browser.close();
    }
  } finally {
    server.kill();
  }
} finally {
  await writeFile(resolve(evidence, 'composer.json'), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`composer smoke passed: native, browser, keyboard and touch; evidence ${evidence}/composer.json`);
