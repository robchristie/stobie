import assert from 'node:assert/strict';
import { once } from 'node:events';
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import test from 'node:test';
import { Readable } from 'node:stream';
import { fileURLToPath } from 'node:url';
import { brotliDecompressSync, gunzipSync } from 'node:zlib';
import { packageBrowser, sha256, WASM_OPT_VERSION } from '../browser-package.mjs';
import { createProductionServer, negotiateEncoding } from '../browser-serve.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const temporaryRoot = path.join(root, 'target/browser-package-tests');
mkdirSync(temporaryRoot, { recursive: true });
function fixture(t) {
  const directory = mkdtempSync(path.join(temporaryRoot, 'test-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const input = path.join(directory, 'input'), output = path.join(directory, 'output');
  mkdirSync(path.join(input, 'pkg/snippets/helper'), { recursive: true });
  writeFileSync(path.join(input, 'index.html'), '<html><head></head><body><script type="module" src="bootstrap.js"></script></body></html>');
  writeFileSync(path.join(input, 'bootstrap.js'), "import './pkg/app.js'; new Worker('worker.js', {type:'module'});\n");
  writeFileSync(path.join(input, 'worker.js'), "import './pkg/app.js';\n");
  writeFileSync(path.join(input, 'pkg/app.js'), "import './snippets/helper/module.js'; export const wasm = new URL('app_bg.wasm', import.meta.url);\n");
  writeFileSync(path.join(input, 'pkg/snippets/helper/module.js'), 'export const value = 42;\n');
  writeFileSync(path.join(input, 'pkg/app_bg.wasm'), Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]));
  const options = { apps: [{ name: 'lab', directory: input }], output, sourceIdentity: 'fixture' };
  return { directory, input, output, options };
}
function get(server, resource, headers = {}, method = 'GET') {
  return new Promise((resolve, reject) => {
    const request = http.request({ host: '127.0.0.1', port: server.address().port, path: resource, headers, method, agent: false }, response => {
      const chunks = [];
      response.on('data', chunk => chunks.push(chunk));
      response.on('end', () => resolve({ status: response.statusCode, headers: response.headers, bytes: Buffer.concat(chunks) }));
    });
    request.on('error', reject);
    request.end();
  });
}
async function serve(t, options, port = 0) {
  const server = createProductionServer({recordRequests: true, ...options});
  server.listen(port, '127.0.0.1');
  await once(server, 'listening');
  t.after(() => new Promise(resolve => server.close(resolve)));
  return server;
}

test('packaging is reproducible, preserves relative imports and round-trips every representation', t => {
  const { options, output, input } = fixture(t);
  const first = packageBrowser(options);
  assert.deepEqual(packageBrowser(options), first);
  const app = first.apps[0];
  assert.equal(app.contentId.length, 64);
  assert.match(readFileSync(path.join(output, app.entry.path), 'utf8'), new RegExp(`<base href="./assets/${app.contentId}/">`));
  for (const record of [...app.assets, app.entry]) {
    const raw = readFileSync(path.join(output, record.path));
    const br = readFileSync(path.join(output, `${record.path}.br`));
    const gzip = readFileSync(path.join(output, `${record.path}.gz`));
    assert.deepEqual(brotliDecompressSync(br), raw);
    assert.deepEqual(gunzipSync(gzip), raw);
    for (const [kind, bytes] of [['raw', raw], ['br', br], ['gzip', gzip]]) assert.deepEqual(record[kind], { bytes: bytes.length, sha256: sha256(bytes) });
  }
  for (const relative of ['worker.js', 'pkg/app.js', 'pkg/snippets/helper/module.js']) assert.deepEqual(readFileSync(path.join(output, app.assetDirectory, relative)), readFileSync(path.join(input, relative)));
  assert.notEqual(packageBrowser({ ...options, sourceIdentity: 'different-configuration' }).apps[0].contentId, app.contentId);
  writeFileSync(path.join(input, 'bootstrap.js'), 'changed input\n');
  const second = packageBrowser(options);
  assert.notEqual(second.apps[0].contentId, app.contentId);
  assert.ok(!existsSync(path.join(output, app.assetDirectory)));
});

test('missing or incompatible optimiser fails before replacing existing output', t => {
  const { options, directory, output } = fixture(t);
  const previous = packageBrowser(options);
  assert.throws(() => packageBrowser({ ...options, variant: 'Oz', wasmOpt: path.join(directory, 'missing') }), /wasm-opt 131 is required/);
  const fake = path.join(directory, 'wasm-opt');
  writeFileSync(fake, '#!/usr/bin/env node\nconsole.log("wasm-opt version 130");\n');
  chmodSync(fake, 0o755);
  assert.throws(() => packageBrowser({ ...options, variant: 'O3', wasmOpt: fake }), /Incompatible wasm-opt/);
  writeFileSync(fake, `#!/usr/bin/env node\nif(process.argv.includes('--version')) console.log('wasm-opt version ${WASM_OPT_VERSION}'); else process.exit(1);\n`);
  assert.throws(() => packageBrowser({ ...options, variant: 'Oz', wasmOpt: fake }), /wasm-opt failed/);
  assert.deepEqual(JSON.parse(readFileSync(path.join(output, 'manifest.json'))), previous);
  assert.throws(() => packageBrowser({ ...options, variant: 'O4' }), /Unknown variant/);
});

test('output replacement and inputs reject unsafe locations and symlinks', t => {
  const { options, directory, input } = fixture(t);
  assert.throws(() => packageBrowser({ ...options, output: directory }), /isolated/);
  assert.throws(() => packageBrowser({ ...options, output: path.join(input, 'output') }), /isolated/);
  const unrelated = path.join(directory, 'unrelated');
  mkdirSync(unrelated); writeFileSync(path.join(unrelated, 'keep'), 'keep');
  assert.throws(() => packageBrowser({ ...options, output: unrelated }), /unrelated output/);
  symlinkSync(path.join(unrelated, 'keep'), path.join(input, 'linked'));
  assert.throws(() => packageBrowser(options), /Symlink/);
});

test('quality negotiation respects exclusions, fallback, wildcards and malformed qualities', () => {
  for (const [header, expected] of [['', 'identity'], ['br, gzip', 'br'], ['br;q=0, gzip', 'gzip'], ['gzip;q=0.8, br;q=0.5, identity;q=0', 'gzip'], ['deflate', 'identity'], ['*;q=0', null], ['br;q=0,gzip;q=0,identity;q=0', null], ['*;q=0.5,identity;q=0', 'br'], ['br;q=2,gzip;q=1', 'gzip'], ['br;q=0.4', 'identity']]) assert.equal(negotiateEncoding(header), expected, header);
});

for (const basePath of ['/', '/preview/nested/']) {
  test(`HTTP serves a coherent package under ${basePath} with representation-aware validators`, async t => {
    const { options, output, input } = fixture(t);
    const manifest = packageBrowser(options);
    const app = manifest.apps[0];
    const server = await serve(t, { directory: output, basePath });
    const html = await get(server, `${basePath}lab/`);
    assert.equal(html.status, 200);
    assert.equal(html.headers['cache-control'], 'no-cache');
    assert.equal(html.headers.vary, 'Accept-Encoding');
    assert.match(html.headers['content-type'], /^text\/html/);
    const assetPath = `${basePath}${app.assetDirectory}/pkg/app_bg.wasm`;
    for (const [encoding, decompress] of [['br', brotliDecompressSync], ['gzip', gunzipSync], ['identity', bytes => bytes]]) {
      const response = await get(server, assetPath, { 'Accept-Encoding': encoding });
      assert.equal(response.status, 200);
      assert.equal(response.headers['content-type'], 'application/wasm');
      assert.equal(response.headers['content-encoding'], encoding === 'identity' ? undefined : encoding);
      assert.equal(response.headers['cache-control'], 'public, max-age=31536000, immutable');
      assert.deepEqual(decompress(response.bytes), readFileSync(path.join(input, 'pkg/app_bg.wasm')));
      const cached = await get(server, assetPath, { 'Accept-Encoding': encoding, 'If-None-Match': `W/${response.headers.etag}` });
      assert.equal(cached.status, 304);
      assert.equal(cached.bytes.length, 0);
      const head = await get(server, assetPath, { 'Accept-Encoding': encoding }, 'HEAD');
      assert.equal(Number(head.headers['content-length']), response.bytes.length);
      assert.equal(head.bytes.length, 0);
    }
    const redirect = await get(server, `${basePath}lab?query=1`);
    assert.equal(redirect.status, 308);
    assert.equal(redirect.headers.location, `${basePath}lab/?query=1`);
    for (const relative of ['worker.js', 'pkg/app.js', 'pkg/snippets/helper/module.js']) {
      const response = await get(server, `${basePath}${app.assetDirectory}/${relative}`);
      assert.equal(response.status, 200);
      assert.match(response.headers['content-type'], /^text\/javascript/);
    }
    assert.equal((await get(server, assetPath, { 'Accept-Encoding': '*;q=0' })).status, 406);
    assert.equal((await get(server, assetPath, {}, 'POST')).status, 405);
    assert.equal(server.requests.at(-1).status, 405);
    for (const resource of [`${basePath}manifest.json`, `${basePath}catalogue`, `${basePath}lab/assets/fake/pkg/app.js`]) {
      const response = await get(server, resource);
      assert.equal(response.status, 404);
      assert.equal(response.headers['cache-control'], 'no-store');
    }
    for (const resource of [`${basePath}%2e%2e/package.json`, `${basePath}lab/%5c..%5csecret`, `${basePath}%ZZ`]) {
      const response = await get(server, resource);
      assert.equal(response.status, 400);
      assert.equal(response.headers['cache-control'], 'no-store');
    }
  });
}

test('identity baseline, request evidence and opt-in network profile apply to worker assets', async t => {
  const { options, output } = fixture(t);
  const app = packageBrowser(options).apps[0];
  const server = await serve(t, { directory: output, encoding: 'identity', network: { bytesPerSecond: 1_250_000, latencyMs: 25 }, fallback: (_request, response) => response.end('data') });
  const start = performance.now();
  const response = await get(server, `/${app.assetDirectory}/worker.js`, { 'Accept-Encoding': 'br,gzip' });
  assert.ok(performance.now() - start >= 20);
  assert.equal(response.headers['content-encoding'], undefined);
  assert.equal(server.requests[0].sentBytes, response.bytes.length);
  const data = await get(server, '/catalogue');
  assert.equal(data.bytes.toString(), 'data');
  assert.equal(data.headers['cache-control'], 'no-store');
  assert.equal(server.requests.at(-1).sentBytes, 4);
  const subpathServer = await serve(t, { directory: output, basePath: '/preview/', fallback: (_request, response) => response.end('origin data') });
  const originData = await get(subpathServer, '/catalogue');
  assert.equal(originData.bytes.toString(), 'origin data');
  assert.equal(originData.headers['cache-control'], 'no-store');
});

test('server fails closed when a listed representation is changed or escapes the output', async t => {
  const { options, output, directory } = fixture(t);
  const app = packageBrowser(options).apps[0];
  const asset = app.assets[0];
  const server = await serve(t, { directory: output });
  writeFileSync(path.join(output, asset.path), 'tampered');
  assert.equal((await get(server, `/${asset.path}`)).status, 500);
  const secret = path.join(directory, 'secret');
  writeFileSync(secret, 'secret');
  rmSync(path.join(output, asset.path));
  symlinkSync(secret, path.join(output, asset.path));
  assert.equal((await get(server, `/${asset.path}`)).status, 500);
});

const realWasmOpt = process.env.WASM_OPT || path.join(root, '.tools/browser/binaryen-version_131/bin/wasm-opt');
test('pinned real wasm-opt builds Oz and O3 variants with distinct configuration identities', t => {
  const { options } = fixture(t);
  const none = packageBrowser(options);
  const oz = packageBrowser({ ...options, variant: 'Oz', wasmOpt: realWasmOpt });
  const o3 = packageBrowser({ ...options, variant: 'O3', wasmOpt: realWasmOpt });
  assert.match(oz.toolVersions.wasmOpt, /^wasm-opt version 131/);
  assert.equal(new Set([none, oz, o3].map(value => value.apps[0].contentId)).size, 3);
});


test('CLI reads build configuration from a JSON identity file and includes it in versioning', t => {
  const { options, directory, input, output } = fixture(t);
  const identityPath = path.join(directory, 'source-identity.json');
  const identity = { profile: 'release', browserTestApi: false, rust: 'fixture-rust', bindgen: 'fixture-bindgen', commit: 'fixture-commit' };
  writeFileSync(identityPath, JSON.stringify(identity));
  const result = spawnSync(process.execPath, [path.join(root, 'tools/browser-package.mjs'), '--app', `lab=${input}`, '--output', output, '--source-identity-file', identityPath], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(result.stdout);
  assert.deepEqual(manifest.sourceIdentity, identity);
  assert.equal(packageBrowser({ ...options, sourceIdentity: identity }).apps[0].contentId, manifest.apps[0].contentId);
  assert.notEqual(packageBrowser({ ...options, sourceIdentity: { ...identity, browserTestApi: true } }).apps[0].contentId, manifest.apps[0].contentId);
  writeFileSync(identityPath, '[]');
  const rejected = spawnSync(process.execPath, [path.join(root, 'tools/browser-package.mjs'), '--source-identity-file', identityPath], { encoding: 'utf8' });
  assert.equal(rejected.status, 1);
  assert.match(rejected.stderr, /requires a JSON object/);
});

test('new publication revalidates HTML at the same origin and explicitly retains previous immutable assets', async t => {
  const { options, directory, input, output } = fixture(t);
  writeFileSync(path.join(input, 'extra.html'), '<html>old secondary page</html>');
  const oldApp = packageBrowser(options).apps[0];
  const oldServer = await serve(t, { directory: output, basePath: '/preview/' });
  const port = oldServer.address().port;
  const oldHtml = await get(oldServer, '/preview/lab/');
  const oldAsset = await get(oldServer, `/preview/${oldApp.assetDirectory}/bootstrap.js`, { 'Accept-Encoding': 'br' });
  await new Promise(resolve => oldServer.close(resolve));
  writeFileSync(path.join(input, 'bootstrap.js'), 'export const build = 2;\n');
  const currentOutput = path.join(directory, 'current');
  const currentApp = packageBrowser({ ...options, output: currentOutput }).apps[0];
  assert.notEqual(currentApp.contentId, oldApp.contentId);
  const server = await serve(t, { directory: currentOutput, retainedDirectories: [output], basePath: '/preview/' }, port);
  const revisitedHtml = await get(server, '/preview/lab/', { 'If-None-Match': oldHtml.headers.etag });
  assert.equal(revisitedHtml.status, 200);
  assert.equal(revisitedHtml.headers['cache-control'], 'no-cache');
  assert.notEqual(revisitedHtml.headers.etag, oldHtml.headers.etag);
  assert.ok(revisitedHtml.bytes.toString().includes(currentApp.contentId));
  assert.ok(!revisitedHtml.bytes.toString().includes(oldApp.contentId));
  const retainedAsset = await get(server, `/preview/${oldApp.assetDirectory}/bootstrap.js`, { 'Accept-Encoding': 'br' });
  assert.deepEqual(retainedAsset.bytes, oldAsset.bytes);
  assert.equal(retainedAsset.headers.etag, oldAsset.headers.etag);
  assert.match(retainedAsset.headers['cache-control'], /immutable/);
  const revalidatedAsset = await get(server, `/preview/${oldApp.assetDirectory}/bootstrap.js`, { 'Accept-Encoding': 'br', 'If-None-Match': oldAsset.headers.etag });
  assert.equal(revalidatedAsset.status, 304);
  assert.equal((await get(server, `/preview/${currentApp.assetDirectory}/bootstrap.js`)).bytes.toString(), 'export const build = 2;\n');
  assert.equal((await get(server, `/preview/${oldApp.assetDirectory}/extra.html`)).status, 404);
  writeFileSync(path.join(output, oldApp.assetDirectory, 'unlisted.js'), 'unlisted');
  assert.equal((await get(server, `/preview/${oldApp.assetDirectory}/unlisted.js`)).status, 404);
  const isolated = await serve(t, { directory: currentOutput });
  assert.equal((await get(isolated, `/${oldApp.assetDirectory}/bootstrap.js`)).status, 404);
});


test('network streaming forwards a bounded body before the source completes alongside static requests', async t => {
  const { options, output } = fixture(t);
  const app = packageBrowser(options).apps[0];
  let releaseTail;
  const clientSawPrefix = new Promise(resolve => { releaseTail = resolve; });
  const prefix = Buffer.alloc(48_000, 65), tail = Buffer.alloc(33_000, 66);
  const writes = [];
  let transfer;
  const server = await serve(t, {
    directory: output,
    network: { bytesPerSecond: 1_250_000, latencyMs: 0 },
    fallback: (_request, response) => {
      const originalWrite = response.write;
      response.write = function (chunk, ...args) { writes.push(chunk.length); return originalWrite.call(this, chunk, ...args); };
      const source = Readable.from((async function* () {
        yield prefix;
        // Full-body buffering would deadlock: the producer waits until its
        // first bytes have reached the client before producing the tail.
        await clientSawPrefix;
        yield tail;
      })(), { objectMode: false, highWaterMark: 16_384 });
      transfer = server.pipeNetwork(source, response);
    },
  });
  const streamed = new Promise((resolve, reject) => {
    const request = http.get({ host: '127.0.0.1', port: server.address().port, path: '/data', agent: false }, response => {
      const chunks = [];
      response.on('data', chunk => { chunks.push(chunk); releaseTail(); });
      response.on('end', () => resolve(Buffer.concat(chunks)));
      response.on('error', reject);
    });
    request.on('error', reject);
  });
  const [body, staticResponse] = await Promise.all([streamed, get(server, `/${app.assetDirectory}/worker.js`)]);
  await transfer;
  assert.deepEqual(body, Buffer.concat([prefix, tail]));
  assert.equal(staticResponse.status, 200);
  assert.ok(writes.length > 2);
  assert.ok(writes.every(length => length <= 16_384));
  assert.equal(writes.reduce((sum, length) => sum + length, 0), body.length);
  assert.equal(server.requests.find(request => request.url === '/data').sentBytes, body.length);
});

test('network streaming abort destroys the upstream and rejects its pending transfer', async t => {
  const { options, output } = fixture(t);
  packageBrowser(options);
  let source, settleTransfer;
  const transferred = new Promise(resolve => { settleTransfer = resolve; });
  const server = await serve(t, {
    directory: output,
    network: { bytesPerSecond: 1_250_000, latencyMs: 0 },
    fallback: (_request, response) => {
      source = new Readable({ highWaterMark: 16_384, read() { this.push(Buffer.alloc(16_384)); } });
      server.pipeNetwork(source, response).then(() => settleTransfer(null), settleTransfer);
    },
  });
  await new Promise((resolve, reject) => {
    const request = http.get({ host: '127.0.0.1', port: server.address().port, path: '/stream', agent: false }, response => {
      response.once('data', () => { response.destroy(); resolve(); });
    });
    request.on('error', reject);
  });
  const error = await transferred;
  assert.ok(error instanceof Error);
  assert.equal(source.destroyed, true);
});

test('build configuration changes asset identity while provenance-only changes do not', t => {
  const { options } = fixture(t);
  const first = packageBrowser({ ...options, sourceIdentity: { revision: 'first', dirty: true, profile: 'release' } });
  const recorded = packageBrowser({ ...options, sourceIdentity: { revision: 'second', dirty: false, profile: 'release' } });
  assert.equal(first.apps[0].contentId, recorded.apps[0].contentId);
  assert.notEqual(first.sourceIdentity.revision, recorded.sourceIdentity.revision);
  const configured = packageBrowser({ ...options, sourceIdentity: { revision: 'second', dirty: false, profile: 'different' } });
  assert.notEqual(recorded.apps[0].contentId, configured.apps[0].contentId);
});


test('request evidence is disabled by default and enabled recording caps retained records and counts drops', async t => {
  const { options, output } = fixture(t);
  packageBrowser(options);
  const defaultServer = createProductionServer({ directory: output });
  defaultServer.listen(0, '127.0.0.1');
  await once(defaultServer, 'listening');
  t.after(() => new Promise(resolve => defaultServer.close(resolve)));
  assert.equal((await get(defaultServer, '/lab/')).status, 200);
  assert.equal((await get(defaultServer, '/missing')).status, 404);
  assert.deepEqual(defaultServer.requests, []);
  assert.equal(defaultServer.requestsDropped, 0);

  const recordedServer = await serve(t, { directory: output, maxRequestRecords: 2 });
  const html = await get(recordedServer, '/lab/');
  assert.equal((await get(recordedServer, '/missing')).status, 404);
  assert.equal((await get(recordedServer, '/lab/', { 'If-None-Match': html.headers.etag })).status, 304);
  assert.equal((await get(recordedServer, '/lab/index.html')).status, 200);
  assert.deepEqual(recordedServer.requests.map(record => [record.url, record.status]), [['/lab/', 200], ['/missing', 404]]);
  assert.equal(recordedServer.requests[0].sentBytes, html.bytes.length);
  assert.equal(recordedServer.requestsDropped, 2);

  for (const invalid of [NaN, Infinity, 0, -1, 1.5, '2']) {
    assert.throws(() => createProductionServer({ directory: output, recordRequests: true, maxRequestRecords: invalid }), /positive integer/);
  }
});
