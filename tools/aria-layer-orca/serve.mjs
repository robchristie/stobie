// Serve one directory on loopback for the containerised browser.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, resolve, sep } from 'node:path';

const [site, port] = process.argv.slice(2);
const root = resolve(site);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
createServer(async (request, response) => {
  const path = resolve(root, new URL(request.url, 'http://local').pathname.slice(1) || 'index.html');
  try {
    if (!path.startsWith(root + sep)) throw new Error('outside the site');
    const body = await readFile(path);
    response.writeHead(200, { 'Content-Type': types[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(body);
  } catch { response.writeHead(404); response.end(); }
}).listen(Number(port), '127.0.0.1');
