import { createServer } from 'node:http';
import { readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { sha256 } from './browser-package.mjs';

const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.mjs': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.wasm': 'application/wasm', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.ico': 'image/x-icon', '.woff2': 'font/woff2' };

/** Prefer br, then gzip, then identity at equal quality; explicit q=0 is honoured. */
export function negotiateEncoding(header = '', available = ['br', 'gzip', 'identity']) {
  if (!header.trim()) return available.includes('identity') ? 'identity' : null;
  const qualities = new Map();
  for (const token of header.split(',')) {
    const [name, ...parameters] = token.trim().toLowerCase().split(';');
    const qParameter = parameters.map(value => value.trim()).find(value => value.startsWith('q='));
    const qText = qParameter?.slice(2) ?? '1';
    const quality = /^(?:0(?:\.\d{0,3})?|1(?:\.0{0,3})?)$/.test(qText) ? Number(qText) : 0;
    qualities.set(name, quality);
  }
  const quality = name => qualities.get(name) ?? (name === 'identity' ? (qualities.get('*') === 0 ? 0 : 1) : (qualities.get('*') ?? 0));
  return available.map((name, order) => ({ name, quality: quality(name), order }))
    .filter(item => item.quality > 0).sort((a, b) => b.quality - a.quality || a.order - b.order)[0]?.name ?? null;
}

/** Return an unbound HTTP server. Only manifest-declared package resources are served. */
export function createProductionServer({ directory = 'target/browser-production', retainedDirectories = [], basePath = '/', encoding = 'negotiated', network = null, fallback = null, recordRequests = false, maxRequestRecords = 4096 } = {}) {
  directory = realpathSync(directory);
  if (!/^\/(?:[a-zA-Z0-9_-]+\/)*$/.test(basePath)) throw new Error('basePath must be / or a slash-terminated path such as /preview/');
  if (!['negotiated', 'identity'].includes(encoding)) throw new Error(`Unknown encoding mode ${encoding}`);
  if (network && (!(network.bytesPerSecond > 0) || !(network.latencyMs >= 0))) throw new Error('Network requires positive bytesPerSecond and nonnegative latencyMs');
  function readManifest(packageDirectory) {
    const result = JSON.parse(readFileSync(path.join(packageDirectory, 'manifest.json'), 'utf8'));
    if (result.schemaVersion !== 1) throw new Error('Unsupported browser package manifest');
    return result;
  }
  const manifest = readManifest(directory);
  const resources = new Map();
  function addAssets(packageManifest, packageDirectory, retained) {
    for (const app of packageManifest.apps) {
      if (!retained) resources.set(app.entry.path, { ...app.entry, directory: packageDirectory, immutable: false });
      for (const asset of app.assets) {
        if (!asset.path.startsWith(`${app.name}/assets/${app.contentId}/`)) throw new Error('Asset is outside its versioned package');
        const isHtml = path.extname(asset.path) === '.html';
        if (retained && isHtml) continue;
        const existing = resources.get(asset.path);
        if (existing) {
          for (const representation of ['raw', 'br', 'gzip']) {
            if (existing[representation].sha256 !== asset[representation].sha256 || existing[representation].bytes !== asset[representation].bytes) {
              throw new Error(`Conflicting immutable asset: ${asset.path}`);
            }
          }
          continue;
        }
        resources.set(asset.path, { ...asset, directory: packageDirectory, immutable: !isHtml });
      }
    }
  }
  addAssets(manifest, directory, false);
  // Previous builds must be supplied explicitly. Their HTML and arbitrary files
  // never join the current publication; only declared immutable assets survive.
  for (const retainedDirectory of retainedDirectories) {
    const packageDirectory = realpathSync(retainedDirectory);
    addAssets(readManifest(packageDirectory), packageDirectory, true);
  }
  if (!Number.isSafeInteger(maxRequestRecords) || maxRequestRecords < 1) throw new Error('maxRequestRecords must be a positive integer');
  const requests = [];
  let nextSend = 0;
  // One shared egress budget covers static assets and streamed proxy responses.
  function reserveChunk(length) {
    const now = performance.now();
    const due = Math.max(now, nextSend) + length / network.bytesPerSecond * 1000;
    nextSend = due;
    return Math.max(0, due - now);
  }
  function writeChunk(response, chunk) {
    return new Promise((resolve, reject) => {
      let timer;
      const cleanup = () => {
        clearTimeout(timer);
        response.removeListener('close', closed);
        response.removeListener('error', finished);
      };
      const finished = error => { cleanup(); if (error) reject(error); else resolve(); };
      const closed = () => finished(new Error('Response closed during network transfer'));
      if (response.destroyed) { closed(); return; }
      response.once('close', closed);
      response.once('error', finished);
      const write = () => {
        if (response.destroyed) { closed(); return; }
        // Waiting for the write callback bounds queued output even when the
        // socket applies backpressure. No complete proxy body is accumulated.
        try { response.write(chunk, finished); } catch (error) { finished(error); }
      };
      if (!network) { write(); return; }
      timer = setTimeout(write, reserveChunk(chunk.length));
    });
  }
  async function pipeNetwork(source, response) {
    const abortSource = () => source.destroy?.();
    response.once('close', abortSource);
    try {
      if (response.destroyed) throw new Error('Response already closed');
      for await (const value of source) {
        const bytes = typeof value === 'string' ? Buffer.from(value) : value;
        for (let offset = 0; offset < bytes.length; offset += 16_384) {
          await writeChunk(response, bytes.subarray(offset, offset + 16_384));
        }
      }
      if (response.destroyed) throw new Error('Response closed during network transfer');
      response.end();
    } catch (error) {
      source.destroy?.();
      response.destroy();
      throw error;
    } finally {
      response.removeListener('close', abortSource);
    }
  }
  function sendBody(response, bytes) {
    if (!network) { response.end(bytes); return; }
    let offset = 0;
    const send = () => {
      if (response.destroyed) return;
      if (offset === bytes.length) { response.end(); return; }
      const end = Math.min(offset + 16_384, bytes.length);
      const chunk = bytes.subarray(offset, end);
      offset = end;
      setTimeout(() => {
        if (!response.write(chunk)) response.once('drain', send);
        else send();
      }, reserveChunk(chunk.length));
    };
    send();
  }
  const server = createServer((request, response) => {
    let sentBytes = 0;
    if (recordRequests) for (const method of ['write', 'end']) {
      const original = response[method];
      response[method] = function (chunk, encoding, ...rest) {
        if (typeof chunk === 'string') sentBytes += Buffer.byteLength(chunk, typeof encoding === 'string' ? encoding : undefined);
        else if (chunk instanceof Uint8Array) sentBytes += chunk.byteLength;
        return original.call(this, chunk, encoding, ...rest);
      };
    }
    response.setHeader('Cache-Control', 'no-store');
    if (recordRequests) response.on('finish', () => {
      if (requests.length >= maxRequestRecords) { server.requestsDropped++; return; }
      requests.push({ url: request.url, method: request.method, acceptEncoding: request.headers['accept-encoding'] ?? '', ifNoneMatch: request.headers['if-none-match'] ?? null, status: response.statusCode, headers: response.getHeaders(), sentBytes });
    });
    const fail = status => { response.statusCode = status; response.end(); };
    const handle = () => {
      let pathname;
      try { pathname = decodeURIComponent(request.url.split('?')[0]); } catch { fail(400); return; }
      if (!pathname.startsWith('/') || pathname.includes('\\') || pathname.includes('\0') || pathname.split('/').some(part => part === '..' || part === '.')) { fail(400); return; }
      if (!pathname.startsWith(basePath)) {
        if (fallback) fallback(request, response);
        else fail(404);
        return;
      }
      const relative = pathname.slice(basePath.length);
      const app = manifest.apps.find(item => relative === item.name || relative === `${item.name}/`);
      if (app && relative === app.name) {
        response.statusCode = 308;
        response.setHeader('Location', `${basePath}${app.name}/${request.url.includes('?') ? `?${request.url.split('?').slice(1).join('?')}` : ''}`);
        response.end(); return;
      }
      const record = resources.get(app ? `${app.name}/index.html` : relative);
      if (!record) {
        if (fallback) { fallback(request, response); return; }
        fail(404); return;
      }
      if (!['GET', 'HEAD'].includes(request.method)) { response.setHeader('Allow', 'GET, HEAD'); fail(405); return; }
      response.setHeader('Vary', 'Accept-Encoding');
      const selected = negotiateEncoding(request.headers['accept-encoding'], encoding === 'identity' ? ['identity'] : ['br', 'gzip', 'identity']);
      if (!selected) { fail(406); return; }
      const representation = selected === 'identity' ? record.raw : record[selected];
      const suffix = selected === 'br' ? '.br' : selected === 'gzip' ? '.gz' : '';
      let bytes;
      try {
        const filename = realpathSync(path.join(record.directory, `${record.path}${suffix}`));
        if (!filename.startsWith(`${record.directory}${path.sep}`)) throw new Error('Resource escaped package');
        bytes = readFileSync(filename);
        if (bytes.length !== representation.bytes || sha256(bytes) !== representation.sha256) throw new Error('Resource does not match package manifest');
      } catch { fail(500); return; }
      const etag = `"${representation.sha256}"`;
      response.setHeader('Content-Type', MIME[path.extname(record.path)] ?? 'application/octet-stream');
      response.setHeader('Cache-Control', record.immutable ? 'public, max-age=31536000, immutable' : 'no-cache');
      response.setHeader('ETag', etag);
      response.setHeader('X-Content-Type-Options', 'nosniff');
      if (selected !== 'identity') response.setHeader('Content-Encoding', selected);
      const matches = request.headers['if-none-match']?.split(',').some(value => value.trim() === '*' || value.trim().replace(/^W\//, '') === etag);
      if (matches) { response.statusCode = 304; response.end(); return; }
      response.setHeader('Content-Length', bytes.length);
      if (request.method === 'HEAD') response.end();
      else sendBody(response, bytes);
    };
    if (network?.latencyMs) setTimeout(handle, network.latencyMs);
    else handle();
  });
  server.pipeNetwork = pipeNetwork;
  server.requests = requests;
  server.requestsDropped = 0;
  server.manifest = manifest;
  return server;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const options = {};
    let port = 4173, host = '127.0.0.1';
    for (let index = 2; index < process.argv.length; index += 2) {
      const flag = process.argv[index], value = process.argv[index + 1];
      if (!value) throw new Error(`Missing value for ${flag}`);
      if (flag === '--directory') options.directory = value;
      else if (flag === '--retain-directory') (options.retainedDirectories ??= []).push(value);
      else if (flag === '--base-path') options.basePath = value;
      else if (flag === '--encoding') options.encoding = value;
      else if (flag === '--port') port = Number(value);
      else if (flag === '--host') host = value;
      else throw new Error(`Unknown option ${flag}`);
    }
    const server = createProductionServer(options);
    server.listen(port, host, () => console.log(`Browser package: http://${host}:${server.address().port}${options.basePath || '/'}`));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
