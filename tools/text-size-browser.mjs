// Check that the browser's text size reaches `UiPreferences::system`, the
// preferences an application on the framework host follows by default,
// through the Gallery's WebAssembly build: Chromium's own font size setting,
// a root font size the page sets, the bounds of the font scale, and a change
// while the page runs, which applies after the page regains focus, is shown
// again or is resized.
//
// Build the Gallery's web package first (`cargo xtask verify-stage
// gallery-browser` does), then: bash tools/text-size-browser.sh, which runs
// Chromium headful on a private Xvfb. Headless Chromium ignores the font size
// setting.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions, uiLibraryEnvironment } from './browser-launch.mjs';

const web = resolve('apps/stobie-gallery/web');
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/verification-evidence/text-size-browser');
await mkdir(evidence, { recursive: true });

// A page that loads only the Gallery's module: no canvas, renderer or
// application. `?root=20px` sets the root's font size before the module starts.
const page = root => `<!doctype html><html lang="en"${root ? ` style="font-size: ${root}"` : ''}>
<meta charset="utf-8"><title>Text size</title>
<script type="module">
import init, { system_preferences_report } from './pkg/stobie_gallery.js';
await init({ module_or_path: './pkg/stobie_gallery_bg.wasm' });
window.__TEXT_SIZE = () => ({
  root: getComputedStyle(document.documentElement).fontSize,
  ...JSON.parse(system_preferences_report()),
});
</script>`;
const assets = new Map([
  ['pkg/stobie_gallery.js', 'text/javascript'],
  ['pkg/stobie_gallery_bg.wasm', 'application/wasm'],
]);
const server = createServer(async (request, response) => {
  const url = new URL(request.url, 'http://local');
  const name = url.pathname.slice(1);
  try {
    if (name === 'text-size.html') {
      const root = url.searchParams.get('root');
      assert(root === null || /^\d+px$/.test(root), 'a root size in pixels');
      response.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' });
      response.end(page(root));
      return;
    }
    assert(assets.has(name), 'allowlisted Gallery asset');
    const body = await readFile(join(web, name));
    response.writeHead(200, { 'Content-Type': assets.get(name), 'Cache-Control': 'no-store' });
    response.end(body);
  } catch {
    response.writeHead(404);
    response.end();
  }
});
await new Promise(resolveListen => server.listen(0, '127.0.0.1', resolveListen));
const origin = `http://127.0.0.1:${server.address().port}`;

// Everything but the font scale stays at `UiPreferences::default`, with the
// system's appearance.
const preferences = font_scale => ({
  schema_version: 1, appearance: 'system', contrast: 'standard', density: 'comfortable', font_scale, motion: 'full',
});
const launch = { ...hostedLinuxWebGpuLaunchOptions(), env: uiLibraryEnvironment() };
const report = { browser: null, cases: [] };

/** Open the page in `context` and wait for the module. */
async function open(context, root) {
  const tab = await context.newPage();
  const errors = [];
  tab.on('pageerror', error => errors.push(String(error)));
  await tab.goto(`${origin}/text-size.html${root ? `?root=${root}` : ''}`);
  await tab.waitForFunction(() => window.__TEXT_SIZE, null, { timeout: 30_000 });
  return { tab, errors };
}

async function check(label, tab, root, scale) {
  const result = await tab.evaluate(() => window.__TEXT_SIZE());
  assert.equal(result.root, root, `${label}: the root's computed font size`);
  assert.equal(result.font_scale, scale, `${label}: the font scale`);
  assert.deepEqual(result.preferences, preferences(scale), `${label}: the system preferences`);
  report.cases.push({ label, ...result });
}

// Chromium's own font size setting, as a person sets it in Settings >
// Appearance > Font size, seeded into a fresh profile before launch.
for (const [setting, scale] of [[16, 1], [20, 1.25], [24, 1.5], [32, 1.5]]) {
  const profile = await mkdtemp(join(evidence, 'profile-'));
  try {
    await mkdir(join(profile, 'Default'));
    await writeFile(join(profile, 'Default', 'Preferences'), JSON.stringify({ webkit: { webprefs: { default_font_size: setting } } }));
    const context = await chromium.launchPersistentContext(profile, launch);
    try {
      report.browser ??= context.browser()?.version() ?? null;
      const { tab, errors } = await open(context);
      await check(`browser font size ${setting}`, tab, `${setting}px`, scale);
      assert.deepEqual(errors, [], 'no page errors');
    } finally {
      await context.close();
    }
  } finally {
    await rm(profile, { recursive: true, force: true });
  }
}

const browser = await chromium.launch(launch);
report.browser ??= browser.version();
try {
  // A root size the page sets, larger and smaller than the browser's.
  for (const [root, scale] of [['18px', 1.125], ['12px', 1]]) {
    const context = await browser.newContext();
    try {
      const { tab, errors } = await open(context, root);
      await check(`page root ${root}`, tab, root, scale);
      assert.deepEqual(errors, [], 'no page errors');
    } finally {
      await context.close();
    }
  }

  // A change while the page runs applies after focus, being shown again or a
  // resize, and asking in between costs nothing and returns the last value.
  const context = await browser.newContext({ viewport: { width: 1000, height: 700 } });
  try {
    const { tab, errors } = await open(context);
    await check('running at the default', tab, '16px', 1);
    const setRoot = size => tab.evaluate(size => { document.documentElement.style.fontSize = size; }, size);
    await setRoot('22px');
    await check('changed, before an event', tab, '22px', 1);
    await tab.evaluate(() => window.dispatchEvent(new Event('focus')));
    await check('changed, after focus', tab, '22px', 1.375);
    await setRoot('16px');
    await tab.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
    await check('changed, after being shown again', tab, '16px', 1);
    await setRoot('20px');
    await tab.setViewportSize({ width: 800, height: 700 });
    await tab.waitForFunction(() => window.__TEXT_SIZE().font_scale === 1.25, null, { timeout: 10_000 });
    await check('changed, after a resize', tab, '20px', 1.25);
    assert.deepEqual(errors, [], 'no page errors');
  } finally {
    await context.close();
  }
} finally {
  await browser.close();
  server.close();
  await writeFile(join(evidence, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`Text size reaches the system preferences in ${report.browser}: ${report.cases.length} cases, Chromium's font size setting included`);
