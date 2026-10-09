import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createHash, randomUUID } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { open, mkdir, readFile, writeFile, mkdtemp, rm, unlink } from 'node:fs/promises';
import { extname, resolve } from 'node:path';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions } from './browser-launch.mjs';
import { ApplicationClient, SocketTransport, BrowserTransport, BrowserPhysicalAdapter, NativePhysicalAdapter } from './application-client.mjs';
import { observeWarmedIdle } from './browser-idle.mjs';
import { interfaceSelection, assertInterfaceCoverage } from './application-interface-selection.mjs';

const selection = interfaceSelection(process.argv.slice(2));

const evidence = resolve(process.env.POLYORAMA_EVIDENCE_DIR ?? '.tools/runtime/application-interface-evidence/application-interface');
await mkdir(evidence, { recursive: true });
const revision = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
const dirty = Boolean(execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim());
const reports = [];
const sliceOnly = process.env.POLYORAMA_INTERFACE_SLICE === '1';
const sleep = ms => new Promise(done => setTimeout(done, ms));
const selector = capability => ({ capability });
const fact = (key, value) => ({ condition: 'fact', key, value });
async function hash(path) { return createHash('sha256').update(await readFile(path)).digest('hex'); }
async function until(operation, predicate, milliseconds = 15000) {
  const deadline = performance.now() + milliseconds;
  let last;
  while (performance.now() < deadline) {
    try { last = await operation(); if (predicate(last)) return last; }
    catch (error) { last = { error: error.message }; }
    await sleep(50);
  }
  throw new Error(`Host startup timed out: ${JSON.stringify(last)}`);
}

async function journey(app, host, client, physical, build) {
  const report = { app, host, revision, dirty, build, steps: [], inputs: [], receipts: [], failures: [], instance: null };
  reports.push(report);
  report.hello = await client.hello();
  report.instance = client.instance;
  report.application = client.application;
  for (const [operation, version, code] of [[{ op: 'hello' }, 999, 'unsupported_version'], [{ op: 'evaluate' }, 1, 'unsupported_operation']]) {
    const reply = JSON.parse(await client.transport.request(JSON.stringify({ version, request_id: `negotiation-${code}`, instance: client.instance, operation })));
    assert.equal(reply.error.code, code);
    report.failures.push({ name: code, reply });
  }
  const observe = async name => {
    const observation = await client.observe();
    assert.deepEqual(observation.snapshot.semantic_audit, [], `${name}: semantic audit`);
    assert.deepEqual(observation.snapshot.text_audit, [], `${name}: text audit`);
    assert.ok(observation.snapshot.text_audit_coverage, `${name}: audit coverage`);
    report.steps.push({ name, observation });
    return observation;
  };
  const invoke = async (selected, options) => {
    const receipt = await client.invoke(selected, options);
    report.receipts.push(receipt);
    return receipt;
  };
  const click = async selected => report.inputs.push(await client.click(selected, physical));
  const key = async chord => report.inputs.push(await client.key(chord, physical));
  const edit = async (id, text) => {
    await click({ id });
    await client.wait({ condition: 'focused', selector: { id } });
    await key('Control+a');
    await key('Backspace');
    if (text) report.inputs.push(await client.text(text, physical, { selector: { id } }));
    await client.wait(fact('draft_title', text));
  };
  const failure = async (name, selected, code, options) => {
    const before = await client.observe();
    const receipt = await invoke(selected, options);
    assert.equal(receipt.state, 'rejected', name);
    assert.equal(receipt.error.code, code, name);
    const after = await client.observe();
    assert.deepEqual(after.facts, before.facts, `${name}: no mutation`);
    report.failures.push({ name, receipt, before: before.id, after: after.id });
  };
  await client.wait({ condition: 'present', selector: { role: 'application' } });
  await client.wait({ condition: 'audits_clear' });
  assert.equal((await client.query({ id: 'deliberately-absent' })).total, 0);
  await assert.rejects(client.query({ pane: -1 }), error => error.code === 'invalid_request');
  await observe('initial');
  if (app === 'record-desk') {
    // Representative slice before richer workflows: typed discovery, current
    // query, normal mutation, later observation, and duplicate-safe retry.
    const arrange = selector('record-desk.arrange');
    await client.wait({ condition: 'enabled', selector: arrange });
    assert.equal((await client.query(arrange)).total, 1);
    const discovered = await client.discover(arrange);
    assert.equal(discovered.capabilities[0].semantic_invocable, true);
    const before = await client.observe();
    const nextAxis = before.facts.layout_axis === 'Horizontal' ? 'Vertical' : 'Horizontal';
    const requestId = `slice-${host}`;
    const expected = discovered.capabilities[0].target;
    assert.equal((await invoke(arrange, { requestId, expected })).state, 'completed');
    await client.wait(fact('layout_axis', nextAxis));
    const after = await observe('slice-arranged');
    assert.equal((await invoke(arrange, { requestId, expected })).state, 'completed');
    assert.deepEqual((await client.observe()).facts, after.facts);
    await assert.rejects(client.invoke({ ...arrange, id: expected.id }, { requestId, expected }), error => error.code === 'request_id_conflict');
    await failure('stale arrangement', arrange, 'stale_target', { expected });
    assert.equal((await invoke(arrange)).state, 'completed');
    await client.wait(fact('layout_axis', before.facts.layout_axis));
    if (!sliceOnly) {
    await client.wait({ condition: 'present', selector: { id: 'record-desk.record.1013' } });
    await click({ id: 'record-desk.record.1013' });
    await client.wait(fact('selected_record', '1013'));
    await failure('unavailable Apply', selector('record-desk.apply'), 'unavailable');
    await edit('record-desk.title.1013', '');
    await failure('invalid draft', selector('record-desk.apply'), 'validation_failed');
    await edit('record-desk.title.1013', 'Shared interface edit');
    const reviewedBefore = (await client.query(selector('record-desk.toggle-reviewed'))).nodes[0].node.checked;
    await click(selector('record-desk.toggle-reviewed'));
    await client.wait({ condition: 'checked', selector: selector('record-desk.toggle-reviewed'), value: !reviewedBefore });
    const applyExpected = (await client.discover(selector('record-desk.apply'))).capabilities[0].target;
    await edit('record-desk.title.1013', 'Shared interface edited again');
    await failure('stale draft', selector('record-desk.apply'), 'stale_target', { expected: applyExpected });
    await failure('invalid arguments', selector('record-desk.apply'), 'invalid_arguments', { arguments: { title: 'forbidden' } });
    const applyId = `apply-${host}`;
    const currentExpected = (await client.discover(selector('record-desk.apply'))).capabilities[0].target;
    assert.equal((await invoke(selector('record-desk.search'))).state, 'completed');
    await client.wait({ condition: 'focused', selector: { id: 'record-desk.search' } });
    assert.equal((await invoke(selector('record-desk.apply'), { requestId: applyId, expected: currentExpected })).state, 'completed');
    await client.wait(fact('undo_entries', 1));
    await client.wait(fact('draft_dirty', false));
    await client.wait({ condition: 'checked', selector: selector('record-desk.toggle-reviewed'), value: !reviewedBefore });
    await observe('semantic-apply');
    assert.equal((await invoke(selector('record-desk.apply'), { requestId: applyId, expected: currentExpected })).state, 'completed');
    assert.equal((await client.observe()).facts.undo_entries, 1);
    assert.equal((await client.discover(selector('record-desk.undo'))).capabilities[0].availability.state, 'enabled');
    assert.equal((await invoke(selector('record-desk.undo'))).state, 'completed');
    await client.wait(fact('redo_entries', 1));
    await client.wait({ condition: 'checked', selector: selector('record-desk.toggle-reviewed'), value: reviewedBefore });
    assert.equal((await invoke(selector('record-desk.redo'))).state, 'completed');
    await client.wait(fact('undo_entries', 1));
    await client.wait({ condition: 'checked', selector: selector('record-desk.toggle-reviewed'), value: !reviewedBefore });
    await observe('semantic-redo');
    // Corresponding physical edit/Apply/undo/redo remains real host input.
    await edit('record-desk.title.1013', 'Physical shared interface edit');
    await click(selector('record-desk.apply'));
    await client.wait(fact('undo_entries', 2));
    await key('Control+z');
    await client.wait(fact('redo_entries', 1));
    await key('Control+Shift+z');
    await client.wait(fact('undo_entries', 2));
    await observe('physical-redo');
    }
  } else {
    const fit = { capability: 'fit_view', pane: 1 };
    await client.wait({ condition: 'capability_enabled', selector: { capability: 'toggle_diagnostics' } });
    await client.wait({ condition: 'enabled', selector: { id: 'action.fit_view.pane.1' } });
    assert.equal((await client.discover(fit)).capabilities[0].semantic_invocable, true);
    const paneQuery = await client.query({ pane: 1, domain: { kind: 'pane', value: 1 } }, { limit: 3 });
    assert.ok(paneQuery.total >= 3);
    assert.ok(paneQuery.nodes.every(n => n.node.pane === 1));
    report.query_idle = await observeWarmedIdle(async () => (await client.observe()).snapshot.frame, sleep);
    const bounded = await client.query({ role: 'result_row', pane: 5 }, { limit: 2 });
    assert.equal(bounded.nodes.length, 2);
    assert.ok(bounded.total > 2 && bounded.next_cursor);
    assert.ok((await client.query({ role: 'result_row', pane: 5 }, { limit: 2, cursor: bounded.next_cursor })).nodes.length <= 2);
    const observation = await client.observe();
    const collection = observation.collection.collections.find(c => c.id === 'results');
    assert.equal(collection.total, 1_000_000);
    assert.ok(collection.observed < 100);
    const link = { capability: 'link_views', pane: 1 };
    const linked = observation.facts['camera.1.linked'];
    const expected = (await client.discover(link)).capabilities[0].target;
    const requestId = `lab-slice-${host}`;
    assert.equal((await invoke(link, { expected, requestId })).state, 'completed');
    await client.wait(fact('camera.1.linked', !linked));
    assert.equal((await invoke(link, { expected, requestId })).state, 'completed');
    assert.equal((await client.observe()).facts['camera.1.linked'], !linked);
    await failure('stale camera assumptions', link, 'stale_target', { expected });
    await failure('ambiguous Fit pane', { capability: 'fit_view' }, 'ambiguous', { expected: (await client.discover(fit)).capabilities[0].target });
    if (!sliceOnly) {
    await failure('invalid Fit arguments', fit, 'invalid_arguments', { arguments: { camera: 'forbidden' } });
    // Fit reaches the shared validated camera-intent route and changes scale.
    const viewport = (await client.observe()).snapshot.nodes.find(n => n.id === 'pane.1.viewport').rect;
    const fitScale = String(131072 / Math.max(viewport.max_x - viewport.min_x, viewport.max_y - viewport.min_y));
    const initialScale = (await client.observe()).facts['camera.1.scale'];
    assert.notEqual(initialScale, fitScale);
    assert.equal((await invoke(fit)).state, 'completed');
    await client.wait(fact('camera.1.scale', fitScale));
    await client.wait(fact('camera.1.centre_x', '65536'));
    await observe('semantic-fit');
    await click({ id: 'action.link_views.pane.1' });
    await client.wait(fact('camera.1.linked', linked));
    await observe('physical-link');
    }
  }
  await client.wait({ condition: 'present', selector: { role: 'application' } });
  const idle = await observeWarmedIdle(async () => (await client.observe()).snapshot.frame, sleep);
  report.idle = idle;
  await assert.rejects(client.wait({ condition: 'present', selector: { id: 'deliberately-absent' } }, { timeoutMs: 100 }), error => Boolean(error.code === 'timeout' && error.observation));
  const abort = new AbortController(); abort.abort();
  await assert.rejects(client.wait({ condition: 'present', selector: {} }, { signal: abort.signal }), error => error.code === 'cancelled');
  report.capture = await client.capture(physical, { path: resolve(evidence, `${app}-${host}.png`), metadataPath: resolve(evidence, `${app}-${host}-capture.json`), inputRoute: 'semantic_and_physical' });
  assert.equal(report.capture.status, 'complete');
  await writeFile(resolve(evidence, `${app}-${host}.json`), JSON.stringify(report, null, 2));
}

const webRoots = { 'record-desk': 'consumers/record-desk/web', lab: 'apps/analytical-workspace-lab/web' };
const server = createServer(async (request, response) => {
  const url = new URL(request.url, 'http://local');
  const [app, ...parts] = url.pathname.slice(1).split('/');
  const root = resolve(webRoots[app] ?? '.tools/absent');
  const path = resolve(root, parts.join('/') || 'index.html');
  try {
    if (!path.startsWith(`${root}/`)) throw new Error('outside root');
    const bytes = await readFile(path);
    response.writeHead(200, { 'Content-Type': ({ '.js': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html', '.css': 'text/css' })[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(bytes);
  } catch { response.writeHead(404); response.end(); }
});
let browser;
try {
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  if (selection.hosts.includes('browser')) browser = await chromium.launch(hostedLinuxWebGpuLaunchOptions());
  const previewRecord = resolve('.tools/runtime/application-interface-preview-up.json');
  try {
    const preview = browser && JSON.parse(await readFile(previewRecord));
    if (preview) {
      const page = await browser.newPage();
      try {
        const response = await page.goto(`${preview.url}/?automation=1`);
        await writeFile(resolve(evidence, 'private-preview-browser.json'), JSON.stringify({ preview, status: response.status(), browser_verified: response.ok() && await page.locator('canvas').count() === 1 }, null, 2));
      } catch (error) {
        await writeFile(resolve(evidence, 'private-preview-browser.json'), JSON.stringify({ preview, browser_verified: false, error: error.message }, null, 2));
      } finally { await page.close(); }
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  for (const app of selection.hosts.includes('browser') ? selection.apps : []) {
    const page = await browser.newPage({ viewport: app === 'lab' ? { width: 1440, height: 900 } : { width: 1080, height: 760 } });
    await page.goto(`http://127.0.0.1:${server.address().port}/${app}/?automation=1`);
    await journey(app, 'browser', new ApplicationClient(new BrowserTransport(page)), new BrowserPhysicalAdapter(page), { wasm_sha256: await hash(resolve(webRoots[app], 'pkg', app === 'lab' ? 'analytical_workspace_lab_bg.wasm' : 'record_desk_bg.wasm')) });
    await page.close();
  }
  for (const app of selection.hosts.includes('native') ? selection.apps : []) {
    const binary = resolve(app === 'lab' ? 'target/release/analytical-workspace-lab' : 'consumers/record-desk/target/release/record-desk');
    const socket = resolve('.tools/runtime', `ai-${randomUUID().slice(0, 8)}.sock`);
    const storage = await mkdtemp(resolve('.tools/runtime', 'ai-store-'));
    const runtime = await open(resolve(evidence, `${app}-native.log`), 'w');
    const process = spawn(binary, [], { detached: true, stdio: ['ignore', runtime.fd, runtime.fd], env: { ...globalThis.process.env, POLYORAMA_AUTOMATION_SOCKET: socket, RECORD_DESK_STATE_PATH: resolve(storage, 'synthetic-unsaved.json'), XDG_DATA_HOME: resolve(storage, 'synthetic-lab-data') } });
    try {
      const transport = new SocketTransport(socket);
      const client = new ApplicationClient(transport);
      await until(() => client.hello(), value => value.kind === 'hello');
      const xdo = globalThis.process.env.POLYORAMA_XDOTOOL ?? 'xdotool';
      const windows = await until(() => execFileSync(xdo, ['search', '--onlyvisible', '--pid', String(process.pid)], { encoding: 'utf8' }).trim(), value => Boolean(value));
      const window = windows.split('\n')[0];
      await journey(app, 'native', client, new NativePhysicalAdapter(window, { xdotool: xdo, imageMagick: globalThis.process.env.POLYORAMA_IMPORT ?? 'import' }), { binary_sha256: await hash(binary) });
    } finally {
      process.kill('SIGTERM');
      await new Promise(done => { if (process.exitCode !== null || process.signalCode !== null) done(); else process.once('exit', done); });
      await runtime.close();
      await unlink(socket).catch(error => { if (error.code !== 'ENOENT') throw error; });
      await rm(storage, { recursive: true });
    }
  }
  assertInterfaceCoverage(reports, selection);
  await writeFile(resolve(evidence, 'summary.json'), JSON.stringify({ revision, dirty, selection, slice_only: sliceOnly, reports: reports.map(report => ({ app: report.app, host: report.host, instance: report.instance, steps: report.steps.length, failures: report.failures.length, idle: report.idle, capture: report.capture.status })) }, null, 2));
  console.log(`Shared interface passed: ${reports.map(report => `${report.app}/${report.host}`).join(', ')}`);
} catch (error) {
  await writeFile(resolve(evidence, 'failure.json'), JSON.stringify({ message: error.message, stack: error.stack, code: error.code, last_observation: error.observation, reports }, null, 2));
  throw error;
} finally {
  await browser?.close();
  await new Promise(done => server.close(done));
}
