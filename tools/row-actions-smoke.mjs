// Per-row actions from outside the repository: the `outside-rows`
// application (tools/row-actions-app/lib.rs) presents Open, Pinned, Remove
// and the row itself once per person, each carrying the person's ID. Run
// natively and in Chromium, an agent discovers each row's control with the
// ID it carries and invokes it by that ID through `stobie`; a row removed
// after it was discovered is refused. In Chromium, the hidden ARIA layer
// names each row's control after its person, and a plain Playwright click on
// each name acts on that person. Run inside the smoke environment
// (tools/outside-app-smoke.sh).
//
// node tools/row-actions-smoke.mjs STOBIE APP_DIR NATIVE_BINARY EVIDENCE_DIR
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { resolve } from 'node:path';

const [stobie, app, binary, evidence] = process.argv.slice(2);
assert(stobie && app && binary && evidence, 'usage: STOBIE APP_DIR NATIVE_BINARY EVIDENCE_DIR');
await mkdir(evidence, { recursive: true });
const name = 'outside-rows';
const PEOPLE = [[1, 'ada', 'Ada Lovelace'], [2, 'grace', 'Grace Hopper'], [3, 'edsger', 'Edsger Dijkstra']];

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

/// The controls of one action in an observation, with the argument each carries.
function controls(observation, action) {
  return observation.nodes
    .filter(node => node.capabilities.includes(`${name}.${action}`))
    .map(node => ({ id: node.id, name: node.name, argument: node.argument }));
}

// The same journey on every host, through `stobie` alone.
function journey(session) {
  const at = args => ok([...args, '--session', session]);
  const refused = args => {
    const result = cli([...args, '--session', session]);
    assert.equal(result.status, 1, `stobie ${args.join(' ')} is refused: ${result.stderr}`);
    return result.body;
  };
  const before = at(['observe']);
  assert.equal(before.application.name, 'Outside Rows');
  assert.equal(before.facts.people, 3);
  // Each row's control carries its person's ID and is named after them.
  assert.deepEqual(controls(before, 'open').map(({ name, argument }) => [name, argument]),
    PEOPLE.map(([id, , person]) => [`Open ${person}`, id]));
  assert.deepEqual(controls(before, 'select').map(({ name, argument }) => [name, argument]),
    PEOPLE.map(([, key, person]) => [person, key]));
  const open = at(['discover', `${name}.open`]).capabilities;
  assert.deepEqual(open.map(capability => [capability.argument ?? null, capability.rendered]),
    [[1, true], [2, true], [3, true], [null, false]], 'one binding per row and the action\'s own');

  // A value invokes the row's control that carries it.
  const grace = open.find(capability => capability.argument === 2);
  const opened = at(['invoke', `${name}.open`, '--value', '2']);
  assert.equal(opened.receipt.state, 'completed', JSON.stringify(opened.receipt));
  assert.equal(opened.receipt.target.id, grace.id, 'Grace\'s row control ran');
  assert.equal(opened.receipt.target.argument, 2);
  at(['wait', 'fact', '--key', 'opened', '--value', 'grace']);
  // A selector's argument names the row; its control needs no value.
  at(['invoke', `${name}.select`, '--argument', 'edsger']);
  at(['wait', 'fact', '--key', 'selected', '--value', 'edsger']);

  // Availability for one row's argument: a pinned person cannot be removed.
  at(['invoke', `${name}.pin`, '--value', '1']);
  at(['wait', 'fact', '--key', 'pinned', '--value', 'ada']);
  const [removeAda] = at(['discover', `${name}.remove`, '--argument', '1']).capabilities;
  assert.deepEqual(removeAda.availability, { state: 'disabled', reason: 'Unpin Ada Lovelace first' });
  assert.equal(refused(['invoke', `${name}.remove`, '--value', '1']).receipt.error.code, 'unavailable');

  // A removed row's control is stale: its discovered target matches nothing,
  // and the action's own binding refuses the removed person's ID.
  const [edsger] = at(['discover', `${name}.open`, '--argument', '3']).capabilities;
  at(['invoke', `${name}.remove`, '--argument', '3']);
  at(['wait', 'fact', '--key', 'people', '--value', '2']);
  const stale = cli(['invoke', '--id', edsger.id, '--expected', JSON.stringify(edsger.target), '--session', session]);
  assert.equal(stale.status, 1, stale.stderr);
  assert.equal(stale.body.receipt.error.code, 'no_match', JSON.stringify(stale.body));
  const removed = refused(['invoke', `${name}.open`, '--value', '3']).receipt;
  assert.deepEqual([removed.target.argument ?? null, removed.error.message], [null, 'Open is unavailable']);
  at(['invoke', `${name}.restore`]);
  at(['wait', 'fact', '--key', 'people', '--value', '3']);
  const after = at(['observe']).facts;
  // Removing Edsger cleared his selection. A refused invocation is reported in
  // its receipt, not the application.
  assert.deepEqual([after.opened, after.selected, after.pinned, after.error], ['grace', '', 'ada', '']);
  return { before: before.facts, after, open: open.map(({ id, argument }) => ({ id, argument })) };
}

// The browser's accessibility tree names each row's control after its person,
// and a plain click on that name acts on that person.
async function accessibility(page, session) {
  const at = args => ok([...args, '--session', session]);
  const layer = page.locator('[id^="stobie-a11y:layer"]');
  await layer.locator('[role]').first().waitFor({ state: 'attached', timeout: 60000 });
  const button = name => page.getByRole('button', { name, exact: true });
  const names = [];
  for (const [, , person] of PEOPLE) {
    for (const name of [person, `Open ${person}`, `Pinned ${person}`, `Remove ${person}`]) {
      assert.equal(await button(name).count(), 1, `one button named ${name}`);
      names.push(name);
    }
    assert.equal(await button(`Pinned ${person}`).getAttribute('aria-pressed'), 'false');
  }
  const all = await layer.locator('[role="button"]').evaluateAll(elements => elements.map(element => element.getAttribute('aria-label')));
  assert.equal(new Set(all).size, all.length, `button names are distinct: ${all.join(', ')}`);
  // The layer's element is the control the application interface reports.
  const observed = controls(at(['observe']), 'open');
  for (const [id, , person] of PEOPLE) {
    const control = observed.find(control => control.argument === id);
    assert.equal(await button(`Open ${person}`).getAttribute('data-stobie-id'), control.id);
  }
  await page.locator('canvas').screenshot({ path: resolve(evidence, 'row-actions.png') });
  await button('Open Edsger Dijkstra').click();
  at(['wait', 'fact', '--key', 'opened', '--value', 'edsger']);
  await button('Grace Hopper').click();
  at(['wait', 'fact', '--key', 'selected', '--value', 'grace']);
  await button('Pinned Ada Lovelace').click();
  at(['wait', 'fact', '--key', 'pinned', '--value', 'ada']);
  await page.waitForFunction(() => document.querySelector('[aria-label="Pinned Ada Lovelace"]')?.getAttribute('aria-pressed') === 'true', null, { timeout: 5000 });
  // A pinned person's Remove is disabled, with the reason.
  const remove = button('Remove Ada Lovelace');
  assert.equal(await remove.getAttribute('aria-disabled'), 'true');
  assert.match(await remove.getAttribute('aria-description'), /unavailable: Unpin Ada Lovelace first/);
  await button('Remove Grace Hopper').click();
  at(['wait', 'fact', '--key', 'people', '--value', '2']);
  assert.equal(await button('Open Grace Hopper').count(), 0, 'the removed row has no controls');
  // A phone's width, with Ada pinned, Grace removed and Edsger focused.
  await button('Open Edsger Dijkstra').focus();
  await page.setViewportSize({ width: 360, height: 700 });
  await page.waitForTimeout(500);
  await page.locator('canvas').screenshot({ path: resolve(evidence, 'row-actions-narrow.png') });
  await page.setViewportSize({ width: 1000, height: 900 });
  return { names };
}

const report = { app: name, native: null, browser: null };
try {
  ok(['launch', 'rows-native', '--', binary]);
  try {
    report.native = journey('rows-native');
  } finally {
    ok(['stop', 'rows-native']);
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
      const page = await browser.newPage({ viewport: { width: 1000, height: 900 } });
      const errors = [];
      page.on('pageerror', error => errors.push(String(error)));
      const pageUrl = launch.rendererUrl(`${url}?automation=1`);
      await page.goto(pageUrl, { waitUntil: 'commit' });
      ok(['attach', 'rows-aria', '--cdp', `http://127.0.0.1:${port}`, '--page', pageUrl]);
      report.accessibility = await accessibility(page, 'rows-aria');
      ok(['stop', 'rows-aria']);
      // A fresh start for the agent's journey.
      await page.reload({ waitUntil: 'commit' });
      ok(['attach', 'rows-browser', '--cdp', `http://127.0.0.1:${port}`, '--page', pageUrl]);
      report.browser = journey('rows-browser');
      ok(['stop', 'rows-browser']);
      assert.deepEqual(errors, [], 'no page errors');
    } finally {
      await browser.close();
    }
  } finally {
    server.kill();
  }
} finally {
  await writeFile(resolve(evidence, 'row-actions.json'), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`row actions smoke passed: native and browser; evidence ${evidence}/row-actions.json`);
