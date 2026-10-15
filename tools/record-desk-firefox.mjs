// Run Record Desk in Playwright's pinned Firefox on whichever backend Firefox
// offers, and operate it through the ARIA layer: select a record, set its
// notes, Apply and Undo. Not part of `cargo xtask verify`, which installs only
// Chromium; see docs/browser-startup.md for how to run it.
//
//   node tools/record-desk-firefox.mjs [EVIDENCE_DIR]
//
// Set STOBIE_FIREFOX_HEADFUL=1 with a DISPLAY; headless Firefox offers no WebGL2.
//
// Controls are activated with `element.click()`, the layer's assistive
// technology route. Playwright's Firefox clicks at whole CSS pixels, which can
// miss a layer hit target, one CSS pixel at a fractional position, so a plain
// `locator.click()` is not reliable here.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { extname, resolve, sep } from 'node:path';
import { firefox } from 'playwright';

const root = resolve('consumers/record-desk/web');
const evidence = resolve(process.argv[2] ?? '.tools/runtime/record-desk-firefox');
await mkdir(evidence, { recursive: true });
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
const server = createServer(async (request, response) => {
  const path = resolve(root, new URL(request.url, 'http://local').pathname.slice(1) || 'index.html');
  try {
    assert(path.startsWith(`${root}${sep}`));
    response.writeHead(200, { 'Content-Type': types[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(await readFile(path));
  } catch { response.writeHead(404); response.end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await firefox.launch({ headless: process.env.STOBIE_FIREFOX_HEADFUL !== '1' });
const report = { browser: `Firefox ${browser.version()}`, steps: [] };
try {
  const page = await browser.newPage({ viewport: { width: 1080, height: 760 } });
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  const started = Date.now();
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.locator('canvas[data-ready="true"]').waitFor({ timeout: 60000 });
  report.ready_ms = Date.now() - started;
  report.renderer = await page.locator('canvas').evaluate(canvas => ({ ...canvas.dataset }));
  report.webgpu = await page.evaluate(() => Boolean(navigator.gpu));
  const role = (kind, name) => page.getByRole(kind, { name, exact: true });
  const activate = (kind, name) => role(kind, name).evaluate(element => element.click());
  const notes = role('textbox', 'Record notes');
  const until = async (label, check) => {
    const deadline = Date.now() + 10000;
    while (!(await check())) {
      assert(Date.now() < deadline, `timed out waiting for ${label}`);
      await page.waitForTimeout(100);
    }
    report.steps.push(label);
  };
  await activate('option', 'Prepare review agenda · Operations · Reviewed');
  await until('record selected', async () => (await role('textbox', 'Record title').inputValue()) === 'Prepare review agenda');
  const original = await notes.inputValue();
  await notes.fill('Checked in Firefox');
  await until('notes drafted', async () => !(await role('button', 'Apply').isDisabled()));
  await activate('button', 'Apply');
  await until('draft applied', async () => !(await role('button', 'Undo').isDisabled()) && (await notes.inputValue()) === 'Checked in Firefox');
  await activate('button', 'Undo');
  await until('edit undone', async () => (await notes.inputValue()) === original);
  await page.screenshot({ path: resolve(evidence, 'record-desk-firefox.png') });
  assert.deepEqual(errors, [], 'browser runtime errors');
  report.passed = true;
  console.log(`Record Desk passed in ${report.browser} on ${report.renderer.stobieRenderer} (${report.renderer.stobieRendererSelection}: ${report.renderer.stobieRendererReason ?? 'preferred'}): ${report.steps.join(', ')}`);
} finally {
  await writeFile(resolve(evidence, 'record-desk-firefox.json'), `${JSON.stringify(report, null, 2)}\n`);
  await browser.close();
  server.close();
}
