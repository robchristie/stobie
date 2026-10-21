// Feed events into the Live Feed example from outside its passes and check
// that it is idle between events, that each event repaints it, and, in the
// browser, that its WebSocket client reconnects and catches up from the last
// sequence number it received. Run inside the smoke environment
// (tools/live-feed-smoke.sh).
//
// node tools/live-feed-smoke.mjs native|browser EVIDENCE_DIR
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { existsSync } from 'node:fs';
import { mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { extname, join, normalize, resolve } from 'node:path';
import { ApplicationClient, BrowserTransport, SocketTransport } from './application-client.mjs';
import { stobieTargetDirectory } from './target-directory.mjs';
import { observeWarmedIdle } from './browser-idle.mjs';

const [host, evidenceArgument] = process.argv.slice(2);
assert(['native', 'browser'].includes(host) && evidenceArgument, 'usage: native|browser EVIDENCE_DIR');
const evidence = resolve(evidenceArgument);
await mkdir(evidence, { recursive: true });
const wait = ms => new Promise(fulfil => setTimeout(fulfil, ms));
const report = { host, steps: [] };

// Reading observations never repaints, so polling them leaves idle intact.
async function observe(client) { return client.observe(); }
async function frame(client) { return (await observe(client)).snapshot.frame; }
async function waitFacts(client, expected, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  let facts;
  while (Date.now() < deadline) {
    facts = (await observe(client)).facts;
    if (Object.entries(expected).every(([key, value]) => facts[key] === value)) return facts;
    await wait(50);
  }
  throw new Error(`facts ${JSON.stringify(expected)} not observed; last ${JSON.stringify(facts)}`);
}
async function idle(client, name) {
  const result = await observeWarmedIdle(() => frame(client), wait);
  report.steps.push({ name, idle: result });
  return result;
}
// One event, sent while idle, presents in a bounded number of passes: the
// pass that applies it and egui's settling pass, then idle again.
async function eventRepaints(client, name, send, expected) {
  const before = await idle(client, `${name}: idle before`);
  await send();
  const facts = await waitFacts(client, expected);
  const after = await idle(client, `${name}: idle after`);
  const passes = after.frame_after - before.frame_after;
  assert(passes >= 1 && passes <= 3, `${name}: ${passes} passes for one event`);
  report.steps.push({ name, passes, facts });
}
// A message the transport filters out wakes nothing.
async function noRepaint(client, name, send) {
  const before = await idle(client, `${name}: idle before`);
  await send();
  await wait(700);
  const after = await frame(client);
  assert.equal(after, before.frame_after, `${name}: no pass`);
  report.steps.push({ name, passes: 0 });
}

async function native() {
  const binary = resolve(stobieTargetDirectory(), 'release/live-feed');
  assert(existsSync(binary), 'build first: cargo build --release -p live-feed');
  const runtime = process.env.XDG_RUNTIME_DIR ?? '/tmp';
  await mkdir(runtime, { recursive: true });
  const socket = join(runtime, `live-feed-${process.pid}.sock`);
  await rm(socket, { force: true });
  const log = [];
  const app = spawn(binary, [], {
    env: { ...process.env, STOBIE_AUTOMATION_SOCKET: socket },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  app.stdout.on('data', chunk => log.push(String(chunk)));
  app.stderr.on('data', chunk => log.push(String(chunk)));
  try {
    for (let attempt = 0; !existsSync(socket); attempt++) {
      assert(attempt < 300 && app.exitCode === null, `the application did not start: ${log.join('')}`);
      await wait(100);
    }
    const client = new ApplicationClient(new SocketTransport(socket));
    report.hello = await client.hello();
    await waitFacts(client, { connection: 'open', last_sequence: 0 });
    // Each line on standard input is one event, sent from the reader thread.
    const line = text => new Promise(fulfil => app.stdin.write(`${text}\n`, fulfil));
    await eventRepaints(client, 'first event', () => line(JSON.stringify({ seq: 1, text: 'One' })),
      { last_sequence: 1, latest: 'One' });
    await eventRepaints(client, 'second event', () => line('Two'), { last_sequence: 2, latest: 'Two', received: 2 });
    await eventRepaints(client, 'source closed', () => new Promise(fulfil => app.stdin.end(fulfil)),
      { connection: 'closed' });
    await idle(client, 'closed and idle');
  } finally {
    app.kill('SIGTERM');
    if (app.exitCode === null) await once(app, 'exit');
    await writeFile(join(evidence, 'live-feed-native.log'), log.join(''));
  }
  assert(!/panicked|WGPU error/.test(log.join('')), 'no application failure');
}

async function browser() {
  const { chromium } = await import('playwright');
  const launch = await import('./browser-launch.mjs');
  const root = resolve('apps/live-feed/web');
  assert(existsSync(join(root, 'pkg/live_feed_bg.wasm')), 'build first: cargo xtask build-web');
  const mime = new Map([['.html', 'text/html'], ['.js', 'text/javascript'], ['.css', 'text/css'], ['.wasm', 'application/wasm']]);
  const server = createServer(async (request, response) => {
    try {
      const path = normalize(join(root, new URL(request.url, 'http://local').pathname.slice(1) || 'index.html'));
      if (!path.startsWith(root) || !(await stat(path)).isFile()) throw new Error('not found');
      response.writeHead(200, { 'Content-Type': mime.get(extname(path)) ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
      response.end(await readFile(path));
    } catch { response.writeHead(404); response.end(); }
  });
  await new Promise(fulfil => server.listen(0, '127.0.0.1', fulfil));
  const url = `http://127.0.0.1:${server.address().port}/?automation=1`;
  const browser = await chromium.launch({ ...launch.hostedLinuxWebGpuLaunchOptions(), env: launch.uiLibraryEnvironment() });
  const errors = [];
  try {
    const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
    page.on('pageerror', error => errors.push(String(error)));
    // Headless pages are never hidden, so the page's visibility is emulated:
    // eframe reads it through the prototype getters, before it starts.
    await page.addInitScript(() => {
      let hidden = false;
      Object.defineProperty(Document.prototype, 'hidden', { configurable: true, get: () => hidden });
      Object.defineProperty(Document.prototype, 'visibilityState', { configurable: true, get: () => (hidden ? 'hidden' : 'visible') });
      window.__setPageHidden = value => { hidden = value; document.dispatchEvent(new Event('visibilitychange')); };
    });
    const setHidden = hidden => page.evaluate(value => window.__setPageHidden(value), hidden);
    // A mock feed server: it keeps every event, and on each connection sends
    // those after the client's `after`, overlapping by one to test that the
    // client drops a replay.
    const gateway = { events: [], connections: [], socket: null };
    gateway.publish = event => { gateway.events.push(event); gateway.socket?.send(JSON.stringify(event)); };
    await page.routeWebSocket(/\/feed(\?|$)/, socket => {
      const after = Number(new URL(socket.url()).searchParams.get('after'));
      gateway.connections.push({ url: socket.url(), after });
      gateway.socket = socket;
      socket.onClose(() => { if (gateway.socket === socket) gateway.socket = null; });
      for (const event of gateway.events.filter(event => event.seq >= after)) socket.send(JSON.stringify(event));
    });
    await page.goto(launch.rendererUrl(url), { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => document.body.classList.contains('ready'), null, { timeout: 60000 });
    await launch.assertBrowserRenderer(page, '#live-feed-canvas');
    const client = new ApplicationClient(new BrowserTransport(page));
    report.hello = await client.hello();
    await waitFacts(client, { connection: 'open', last_sequence: 0 });
    assert.equal(gateway.connections[0].after, 0, 'the first connection starts from 0');
    assert.match(gateway.connections[0].url, /\/feed\?after=0$/, 'the feed is beside the page');

    for (const seq of [1, 2, 3]) gateway.publish({ seq, text: `Event ${seq}` });
    await waitFacts(client, { last_sequence: 3, received: 3 });
    await eventRepaints(client, 'event while connected', () => gateway.publish({ seq: 4, text: 'Event 4' }),
      { last_sequence: 4, latest: 'Event 4' });
    await noRepaint(client, 'replayed event and heartbeat', () => {
      gateway.socket.send(JSON.stringify({ seq: 4, text: 'Event 4' }));
      gateway.socket.send(JSON.stringify({ type: 'ping' }));
    });

    // The server drops the connection; events published meanwhile arrive
    // when the client reconnects from its last sequence number.
    gateway.socket.close({ code: 1012, reason: 'restart' });
    gateway.publish({ seq: 5, text: 'Event 5' });
    gateway.publish({ seq: 6, text: 'Event 6' });
    const caughtUp = await waitFacts(client, { connection: 'open', last_sequence: 6 });
    assert.equal(gateway.connections.length, 2, 'one reconnection');
    assert.equal(gateway.connections[1].after, 4, 'the client catches up from its last sequence number');
    assert.deepEqual([caughtUp.received, caughtUp.missed, caughtUp.shown], [6, 0, 6], 'every event once, none missed');
    report.steps.push({ name: 'reconnected and caught up', facts: caughtUp, connections: gateway.connections });
    await eventRepaints(client, 'event after reconnecting', () => gateway.publish({ seq: 7, text: 'Event 7' }),
      { last_sequence: 7, received: 7 });

    // An event that arrives while the page is hidden is presented when the
    // page is shown again, with no input. Nothing has focus, as on a page
    // the user only reads, so eframe sees no focus change on showing it.
    report.focusedBeforeHiding = await page.evaluate(() => document.activeElement?.id || document.activeElement?.tagName);
    await page.evaluate(() => document.activeElement?.blur());
    await idle(client, 'idle before hiding');
    await setHidden(true);
    await wait(300);
    gateway.publish({ seq: 8, text: 'Event 8' });
    await wait(1500);
    const hidden = await observe(client);
    assert.equal(hidden.facts.last_sequence, 7, 'a hidden page presents nothing');
    await setHidden(false);
    const shown = await waitFacts(client, { last_sequence: 8, latest: 'Event 8' }, 5000);
    await idle(client, 'idle after showing');
    report.steps.push({ name: 'event while hidden, presented on show', facts: shown });
    assert.deepEqual(errors, [], 'no page errors');
  } finally {
    await browser.close();
    server.close();
  }
}

try {
  await (host === 'native' ? native() : browser());
} finally {
  await writeFile(join(evidence, `live-feed-${host}.json`), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`live-feed ${host} smoke passed; evidence ${evidence}/live-feed-${host}.json`);
