// Start an application that `stobie new` generated outside this checkout,
// natively and in Chromium on WebGPU and on WebGL2, and operate it only
// through the `stobie` command: observe it, set its draft and invoke one
// action. Run inside the smoke environment (tools/outside-app-smoke.sh).
//
// node tools/outside-app-smoke.mjs STOBIE APP_DIR NATIVE_BINARY EVIDENCE_DIR
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { resolve } from 'node:path';

const [stobie, app, binary, evidence] = process.argv.slice(2);
assert(stobie && app && binary && evidence, 'usage: STOBIE APP_DIR NATIVE_BINARY EVIDENCE_DIR');
await mkdir(evidence, { recursive: true });
const name = 'outside-notes';

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

// The same journey on every host: read, type a draft, invoke Add, observe.
function journey(session) {
  const at = args => ok([...args, '--session', session]);
  const before = at(['observe']);
  assert.equal(before.application.name, 'Outside Notes');
  assert.equal(before.facts.notes, 1, 'the template starts with one note');
  const [add] = at(['discover', `${name}.add`]).capabilities;
  assert.equal(add.availability.state, 'disabled', 'Add needs a draft');
  at(['set-text', `${name}.draft`, 'Typed from outside']);
  at(['wait', 'fact', '--key', 'draft', '--value', 'Typed from outside']);
  const invoked = at(['invoke', `${name}.add`]);
  assert.equal(invoked.receipt.state, 'completed', JSON.stringify(invoked.receipt));
  at(['wait', 'fact', '--key', 'notes', '--value', '2']);
  const after = at(['observe']).facts;
  assert.deepEqual([after.notes, after.draft], [2, ''], 'Add consumed the draft');
  return { before: before.facts, after };
}

const report = { app: name, native: null, browser: {} };
try {
  // Native: a release build launched under a private automation socket.
  ok(['launch', 'outside-native', '--', binary]);
  try {
    report.native = journey('outside-native');
  } finally {
    ok(['stop', 'outside-native']);
  }

  // Browser: `stobie serve` serves the build; Chromium exposes DevTools.
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
    for (const renderer of ['webgpu', 'webgl2']) {
      process.env.STOBIE_BROWSER_RENDERER = renderer;
      const options = launch.hostedLinuxWebGpuLaunchOptions();
      const port = 9300 + (process.pid % 400) + (renderer === 'webgl2' ? 1 : 0);
      const browser = await chromium.launch({ ...options, args: [...options.args, `--remote-debugging-port=${port}`], env: launch.uiLibraryEnvironment() });
      try {
        const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
        const errors = [];
        page.on('pageerror', error => errors.push(String(error)));
        const pageUrl = launch.rendererUrl(`${url}?automation=1`, renderer);
        await page.goto(pageUrl);
        await page.waitForFunction(() => window.__STOBIE_AUTOMATION && window.__STOBIE_STARTUP?.status === 'ready', null, { timeout: 60000 });
        await launch.assertBrowserRenderer(page, '#app-canvas', renderer);
        const session = `outside-${renderer}`;
        const attached = ok(['attach', session, '--cdp', `http://127.0.0.1:${port}`, '--page', pageUrl]);
        const hello = ok(['hello', '--session', session]);
        assert.equal(hello.renderer?.backend, renderer, 'hello reports the running backend');
        report.browser[renderer] = { renderer: hello.renderer, application: attached.application.name, ...journey(session) };
        ok(['stop', session]);
        assert.deepEqual(errors, [], 'no page errors');
      } finally {
        await browser.close();
      }
    }
  } finally {
    server.kill();
  }
} finally {
  await writeFile(resolve(evidence, 'outside-app.json'), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`outside-app smoke passed: native and browser (webgpu, webgl2); evidence ${evidence}/outside-app.json`);
