// Exercise the `stobie` CLI against a native application and compare it with
// the Node client.
//
// node tools/stobie-cli-smoke.mjs STOBIE APP_BINARY APP EVIDENCE_DIR
// node tools/stobie-cli-smoke.mjs STOBIE WEB_ROOT record-desk-browser EVIDENCE_DIR
//
// APP is record-desk, lab or record-desk-browser. For Record Desk, natively
// or in Chromium, the same scenarios run through the Node client and the Rust
// CLI, each against its own fresh instance, and their outcomes must agree.
// Natively, each command's output size is also checked against its budget and
// reported. Run inside the smoke environment (tools/stobie-cli-smoke.sh),
// which supplies a display.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { extname, resolve, sep } from 'node:path';
import { ApplicationClient, BrowserTransport, SocketTransport } from './application-client.mjs';

const [stobie, binary, app, evidence] = process.argv.slice(2);
assert(stobie && binary && ['record-desk', 'lab', 'record-desk-browser'].includes(app) && evidence, 'usage: STOBIE APP_BINARY|WEB_ROOT record-desk|lab|record-desk-browser EVIDENCE_DIR');
const browserHost = app === 'record-desk-browser';
await mkdir(evidence, { recursive: true });

function cli(args, { json = true, input } = {}) {
  const result = spawnSync(stobie, json ? [...args, '--json'] : args, { encoding: 'utf8', input, timeout: 180000 });
  // A completed exchange prints JSON on stdout, even when its outcome is a
  // rejection; an error prints it on stderr.
  let body = null;
  if (json) {
    for (const text of [result.stdout, result.stderr]) {
      try { body = JSON.parse(text); break; } catch { body = null; }
    }
  }
  return { status: result.status, body, stdout: result.stdout, stderr: result.stderr };
}

function launch(name) {
  const launched = cli(['launch', name, '--', binary]);
  assert.equal(launched.status, 0, `launch ${name}: ${launched.stderr}`);
  return launched.body.session.target.socket;
}

// Run the parity scenarios: first through the Node client from `startNode`,
// then through the CLI from `startCli`, each on its own fresh instance.
async function runParity(startNode, startCli, finish) {
  const client = await startNode();
  let viaCli;
  const outcome = (status, body) => status === 0
    ? { ok: true, state: body.receipt?.state ?? null }
    : { ok: false, code: (body?.error?.code ?? body?.receipt?.error?.code) ?? null, state: body?.receipt?.state ?? null };
  const nodeOutcome = async run => {
    try {
      const value = await run();
      if (value?.state && value.state !== 'completed') return { ok: false, code: value.error?.code ?? null, state: value.state };
      return { ok: true, state: value?.state ?? null };
    } catch (error) {
      return { ok: false, code: error.code ?? null, state: null };
    }
  };
  const facts = async () => (await client.observe()).facts;
  const cliFacts = () => viaCli(['observe', '--limit', '0']).body.facts;
  const scenarios = [
    ['select a record', ['invoke', 'record-desk.select-record', '--value', '1013'],
      () => client.invoke({ capability: 'record-desk.select-record' }, { arguments: { value: '1013' } })],
    ['set notes as typing would', ['set-text', 'record-desk.notes.1013', 'Parity notes'],
      () => client.invoke({ id: 'record-desk.notes.1013', capability: 'stobie.set-text' }, { arguments: { value: 'Parity notes' } })],
    ['choose a category', ['invoke', 'record-desk.edit-category', '--value', 'ideas'],
      () => client.invoke({ capability: 'record-desk.edit-category' }, { arguments: { value: 'ideas' } })],
    ['reject an unknown category', ['invoke', 'record-desk.edit-category', '--value', 'archive'],
      () => client.invoke({ capability: 'record-desk.edit-category' }, { arguments: { value: 'archive' } })],
    ['reject a value for a no-argument action', ['invoke', 'record-desk.apply', '--value-json', '1'],
      () => client.invoke({ capability: 'record-desk.apply' }, { arguments: { value: 1 } })],
    ['apply', ['invoke', 'record-desk.apply'], () => client.invoke({ capability: 'record-desk.apply' })],
    ['undo', ['invoke', 'record-desk.undo'], () => client.invoke({ capability: 'record-desk.undo' })],
    ['reject an unavailable action', ['invoke', 'record-desk.undo'], () => client.invoke({ capability: 'record-desk.undo' })],
    ['reject an ambiguous selector', ['invoke', '--role', 'button'], () => client.invoke({ role: 'button' })],
    ['wait for a fact', ['wait', 'fact', '--key', 'redo_entries', '--value', '1'],
      () => client.wait({ condition: 'fact', key: 'redo_entries', value: 1 })],
    ['time out waiting for undone text', ['wait', 'value', '--id', 'record-desk.notes.1013', '--value', 'Parity notes', '--timeout', '1000'],
      () => client.wait({ condition: 'value', selector: { id: 'record-desk.notes.1013' }, value: 'Parity notes' }, { timeoutMs: 1000 })],
    ['cancel an unknown request', ['cancel', 'no-such-request'], () => client.cancel('no-such-request')],
  ];
  const nodeResults = [];
  for (const [, , run] of scenarios) nodeResults.push(await nodeOutcome(run));
  const nodeFacts = await facts();
  viaCli = await startCli();
  scenarios.forEach(([name, args], index) => {
    const rust = viaCli(args);
    const rustOutcome = outcome(rust.status, rust.body);
    report.parity.push({ name, rust: rustOutcome, node: nodeResults[index] });
  });
  const rustFacts = cliFacts();
  await finish();
  for (const { name, rust, node } of report.parity) {
    assert.deepEqual(rust, node, `${name}: the Rust CLI and the Node client disagree`);
  }
  // Both instances end in the same application state.
  assert.deepEqual(rustFacts, nodeFacts, 'final facts agree');
}

const report = { app, budgets: {}, parity: [] };
let browser;
let server;
try {
  if (browserHost) {
    // Serve the web build and open it with ?automation=1 in Chromium with a
    // DevTools endpoint, as a Lantern-managed browser provides one.
    const { chromium } = await import('playwright');
    const { hostedLinuxWebGpuLaunchOptions, uiLibraryEnvironment } = await import('./browser-launch.mjs');
    const root = resolve(binary);
    const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
    server = createServer(async (request, response) => {
      const path = resolve(root, new URL(request.url, 'http://local').pathname.slice(1) || 'index.html');
      try {
        assert(path.startsWith(`${root}${sep}`));
        response.writeHead(200, { 'Content-Type': types[extname(path)] ?? 'application/octet-stream' });
        response.end(await readFile(path));
      } catch { response.writeHead(404); response.end(); }
    });
    await new Promise(done => server.listen(0, '127.0.0.1', done));
    const options = hostedLinuxWebGpuLaunchOptions();
    const port = 9400 + (process.pid % 500);
    browser = await chromium.launch({ ...options, args: [...options.args, `--remote-debugging-port=${port}`], env: uiLibraryEnvironment() });
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    const url = `http://127.0.0.1:${server.address().port}/?automation=1`;
    const fresh = async () => {
      await page.goto(url);
      await page.waitForFunction(() => window.__STOBIE_AUTOMATION, null, { timeout: 30000 });
    };
    const endpoint = `http://127.0.0.1:${port}`;
    await fresh();
    const viaCli = args => cli([...args, '--session', 'parity-browser']);
    assert.equal(cli(['attach', 'parity-browser', '--cdp', endpoint, '--page', url]).status, 0, 'attach');
    report.budgets.observe = { bytes: Buffer.byteLength(cli(['observe', '--session', 'parity-browser'], { json: false }).stdout), budget: 8192 };
    assert(report.budgets.observe.bytes <= 8192, 'browser observe is within its budget');
    const shot = resolve(evidence, 'stobie-cli-browser.png');
    const captured = viaCli(['capture', '--output', shot]);
    assert.equal(captured.status, 0, `capture: ${captured.stderr}`);
    assert.equal(captured.body.route, 'browser_canvas');
    await runParity(
      async () => { await fresh(); const client = new ApplicationClient(new BrowserTransport(page)); await client.hello(); return client; },
      async () => { await fresh(); return viaCli; },
      async () => {},
    );
    assert.equal(cli(['stop', 'parity-browser']).status, 0);
  } else {
    // Output size budgets for each command, in bytes. Launching reports the
    // application's socket.
    assert.ok(launch(`${app}-sizes`));
    const session = ['--session', `${app}-sizes`];
    const budgets = app === 'record-desk'
      ? { 'observe': 8192, 'observe --json': 24576, 'discover': 6144, 'discover --json': 49152, 'query --role result_row': 4096, 'hello': 512 }
      : { 'observe': 16384, 'observe --json': 49152, 'discover': 12288, 'discover --json': 98304, 'query --role tab': 2048, 'hello': 512 };
    for (const [command, budget] of Object.entries(budgets)) {
      const args = command.split(' ');
      const json = args.includes('--json');
      const result = cli([...args.filter(arg => arg !== '--json'), ...session], { json });
      assert.equal(result.status, 0, `${command}: ${result.stderr}`);
      const bytes = Buffer.byteLength(result.stdout);
      report.budgets[command] = { bytes, budget };
      assert(bytes <= budget, `${app} ${command} is ${bytes} bytes, over its ${budget}-byte budget`);
    }
    assert.equal(cli(['stop', `${app}-sizes`]).status, 0);
    if (app === 'record-desk') {
      // The same scenarios through both clients, each on a fresh instance in
      // turn: eframe drains invocations only for a window that is not covered.
      let nodeSocket;
      await runParity(
        async () => { nodeSocket = launch('parity-node'); const client = new ApplicationClient(new SocketTransport(nodeSocket)); await client.hello(); return client; },
        async () => { assert.equal(cli(['stop', 'parity-node']).status, 0); launch('parity-cli'); return args => cli([...args, '--session', 'parity-cli']); },
        async () => { assert.equal(cli(['stop', 'parity-cli']).status, 0); },
      );
    }
  }
  console.log(`stobie CLI smoke passed for ${app}: ${Object.entries(report.budgets).map(([command, { bytes, budget }]) => `${command} ${bytes}/${budget} B`).join(', ')}${report.parity.length ? `; ${report.parity.length} parity scenarios agree` : ''}`);
} finally {
  await writeFile(resolve(evidence, `stobie-cli-${app}.json`), JSON.stringify(report, null, 2));
  await browser?.close();
  server?.close();
  for (const name of [`${app}-sizes`, 'parity-node', 'parity-cli']) cli(['stop', name]);
}
