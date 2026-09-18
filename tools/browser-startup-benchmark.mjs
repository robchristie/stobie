// Opt-in timings; deterministic completion/real-input assertions are also usable in CI.
import assert from 'node:assert/strict';
import { request as httpRequest } from 'node:http';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { cpus, totalmem, platform, release, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions } from './browser-launch.mjs';
import { createProductionServer } from './browser-serve.mjs';

export function spread(values) {
  const ordered = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!ordered.length) return { count: 0, median: null, min: null, max: null };
  const middle = Math.floor(ordered.length / 2);
  return { count: ordered.length, median: ordered.length % 2 ? ordered[middle] : (ordered[middle - 1] + ordered[middle]) / 2, min: ordered[0], max: ordered.at(-1) };
}
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const command = (program, args) => { try { return execFileSync(program, args, { encoding: 'utf8' }).trim(); } catch { return null; } };
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const directory = resolve(option('--directory', 'target/browser-production'));
const output = resolve(option('--output', '.tools/runtime/browser-startup/benchmark'));
const pairs = Number(option('--pairs', '5'));
assert(Number.isInteger(pairs) && pairs > 0 && pairs <= 20, '--pairs must be 1..20');
const apps = option('--apps', 'lab,gallery').split(',');
assert(apps.every(app => ['lab', 'gallery', 'viewer'].includes(app)), 'unknown app');
const profiles = option('--profiles', 'local,network').split(',');
assert(profiles.every(profile => ['local', 'network'].includes(profile)), 'unknown profile');
const encoding = option('--encoding', 'negotiated');
const basePath = option('--base-path', '/');
const launch = hostedLinuxWebGpuLaunchOptions();
const viewport = { width: 1440, height: 900 };
await mkdir(output, { recursive: true });
const manifestBytes = await readFile(join(directory, 'manifest.json'));
const manifest = JSON.parse(manifestBytes);
const report = {
  schema: 1, runnerSha256: hash(await readFile(fileURLToPath(import.meta.url))), serverSha256: hash(await readFile(new URL('./browser-serve.mjs', import.meta.url))), recordedAt: new Date().toISOString(), sourceRevision: command('git', ['rev-parse', 'HEAD']),
  sourceDiffSha256: hash(command('git', ['diff', 'HEAD']) ?? ''), manifestSha256: hash(manifestBytes), manifest,
  host: { platform: platform(), release: release(), cpu: cpus()[0]?.model, logicalCpus: cpus().length, memoryBytes: totalmem() },
  tools: { rustc: command('rustc', ['--version']), wasmBindgen: command('wasm-bindgen', ['--version']), node: process.version, playwright: JSON.parse(await readFile('node_modules/playwright/package.json')).version },
  browser: { launch, viewport, version: null },
  cacheProcedure: 'Each pair starts a fresh Chromium process and temporary profile. Repeat is an ordinary same-URL navigation in that process/profile/origin; localStorage/sessionStorage reset before app code on each navigation. HTTP cache is never cleared/disabled; no interception or cache-busting. OS/GPU and compiled-code cache state uncontrolled.',
  timings: 'Navigation-relative milestones, CPU queue submission and subsequent RAF opportunity; no physical presentation or isolated compilation claim. Input/screenshots follow startup timing capture.',
  encoding, basePath, viewerUpstream: option('--viewer-upstream', null), samples: [], summary: {},
};
if (report.viewerUpstream) {
  const upstream = new URL(report.viewerUpstream);
  assert(upstream.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(upstream.hostname) && !upstream.username && !upstream.password, 'viewer upstream must be a loopback HTTP fixture service');
}
if (report.viewerUpstream) report.fixtureCatalogue = await (await fetch(new URL('/catalogue', report.viewerUpstream))).json();
await writeFile(join(output, 'manifest.json'), manifestBytes);

async function visibleInput(page, app, prefix) {
  // Let startup work settle before readback; this wait is outside all startup timestamps.
  if (app === 'lab') await page.waitForFunction(() => window.__POLYORAMA_DIAGNOSTICS?.runtime.in_flight === 0);
  if (app === 'viewer') await page.waitForFunction(() => window.emuellaViewer?.snapshot().in_flight === 0);
  await page.mouse.move(1400, 880);
  const before = await page.screenshot({ path: `${prefix}-before.png` });
  let semantic = null;
  if (app === 'lab') {
    await page.keyboard.press('2');
    await page.waitForTimeout(120);
  } else if (app === 'gallery') {
    // First catalogue story is button/default. Physical click, no action hook.
    await page.mouse.move(95, 150);
    await page.mouse.down(); await page.waitForTimeout(80); await page.mouse.up();
    await page.waitForFunction(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot().story === 'button/default');
    semantic = await page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot().story);
  } else {
    // Physical wheel zoom in the main image viewport, with generation acknowledgement.
    const generation = await page.evaluate(() => window.emuellaViewer.snapshot().generation);
    await page.mouse.move(550, 300);
    await page.mouse.wheel(0, -160);
    await page.waitForFunction(g => window.emuellaViewer.snapshot().generation > g, generation);
    await page.waitForFunction(() => window.emuellaViewer.snapshot().in_flight === 0);
    semantic = await page.evaluate(() => ({ generation: window.emuellaViewer.snapshot().generation, completed: window.emuellaViewer.snapshot().completed }));
  }
  await page.mouse.move(1400, 880);
  await page.waitForTimeout(120);
  const after = await page.screenshot({ path: `${prefix}-after.png` });
  assert.notEqual(hash(before), hash(after), `${app}: real input must change visible workspace`);
  const pixels = await page.evaluate(async ([before, after]) => {
    const decode = async data => {
      const image = await createImageBitmap(new Blob([Uint8Array.from(atob(data), c => c.charCodeAt(0))], { type: 'image/png' }));
      const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height;
      const ctx = canvas.getContext('2d'); ctx.drawImage(image, 0, 0);
      return ctx.getImageData(0, 0, image.width, image.height).data;
    };
    const a = await decode(before), b = await decode(after);
    let changed = 0, min = 255, max = 0;
    for (let i = 0; i < a.length; i += 4) {
      if (Math.abs(a[i] - b[i]) + Math.abs(a[i + 1] - b[i + 1]) + Math.abs(a[i + 2] - b[i + 2]) > 12) changed++;
      min = Math.min(min, a[i], a[i+1], a[i+2]); max = Math.max(max, a[i], a[i+1], a[i+2]);
    }
    const sample = (data, x, y) => Array.from(data.slice((y * 1440 + x) * 4, (y * 1440 + x) * 4 + 3));
    return { changedPixels: changed, min, max, polygonBefore: sample(a, 78, 80), polygonAfter: sample(b, 78, 80) };
  }, [before.toString('base64'), after.toString('base64')]);
  assert(pixels.changedPixels > 20 && pixels.max - pixels.min > 30, `${app}: blank or unchanged visible output`);
  if (app === 'lab') {
    const [r, g, b] = pixels.polygonAfter;
    assert(g > r + 15 && b > r + 15, 'Polygon toolbar control must visibly become selected');
    assert.notDeepEqual(pixels.polygonBefore, pixels.polygonAfter);
  }
  return { input: app === 'lab' ? 'keyboard 2: Polygon tool' : app === 'gallery' ? 'catalogue physical click: button/default' : 'physical wheel: image zoom', semantic, beforeSha256: hash(before), afterSha256: hash(after), ...pixels };
}

try {
  for (const profile of profiles) {
    const network = profile === 'network' ? { bytesPerSecond: 1_250_000, latencyMs: 40 } : null;
    const server = createProductionServer({ directory, basePath, encoding, recordRequests: true, network: network ?? undefined, fallback: report.viewerUpstream ? (req, res) => {
      if (!/^\/(catalogue|manifest\/|descriptor\/|mask\/|jpip(?:\?|$)|metrics)/.test(req.url)) { res.writeHead(404); res.end(); return; }
      const upstream = httpRequest(new URL(req.url, report.viewerUpstream), { headers: { ...req.headers, host: new URL(report.viewerUpstream).host } }, response => { res.removeHeader('Cache-Control'); res.writeHead(response.statusCode, response.headers); server.pipeNetwork(response, res).catch(() => {}); });
      upstream.on('error', error => { res.writeHead(502); res.end(String(error)); });
      req.pipe(upstream);
    } : undefined });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${server.address().port}`;
    try {
      for (const app of apps) for (let pair = 0; pair < pairs; pair++) {
        const userDataDir = await mkdtemp(join(tmpdir(), 'polyorama-startup-'));
        const context = await chromium.launchPersistentContext(userDataDir, { ...launch, viewport });
        const browserSession = await context.browser().newBrowserCDPSession();
        report.browser.version = (await browserSession.send('Browser.getVersion')).product;
        const gpu = await browserSession.send('SystemInfo.getInfo');
        report.browser.graphics = gpu.gpu;
        await browserSession.detach();
        const page = context.pages()[0] ?? await context.newPage();
        await page.addInitScript(() => {
          if (location.protocol === 'http:' || location.protocol === 'https:') { localStorage.clear(); sessionStorage.clear(); }
          window.__startupAdapters = [];
          if (globalThis.GPUAdapter) {
            const original = GPUAdapter.prototype.requestDevice;
            GPUAdapter.prototype.requestDevice = async function (...args) {
              const device = await original.apply(this, args);
              const info = this.info;
              window.__startupAdapters.push({ vendor: info.vendor, architecture: info.architecture, device: info.device, description: info.description });
              return device;
            };
          }
        });
        try {
          for (const visit of ['cold', 'repeat']) {
            const errors = [], responses = [];
            const onError = e => errors.push(String(e));
            const onResponse = async response => {
              responses.push({ url: response.url(), status: response.status(), headers: await response.allHeaders() });
            };
            page.on('pageerror', onError); context.on('response', onResponse);
            const requestStart = server.requests?.length ?? 0;
            const sample = { app, profile, network, pair, visit, url: `${origin}${basePath.endsWith('/') ? basePath : `${basePath}/`}${app}/` };
            report.samples.push(sample);
            await page.goto(sample.url, { waitUntil: 'domcontentloaded' });
            await page.waitForFunction(() => {
              const s = window.__POLYORAMA_STARTUP;
              return s?.failure || (s?.milestones.rendering_opportunity_proxy && s?.milestones.first_useful_content);
            }, null, { timeout: 120_000 });
            sample.startup = await page.evaluate(() => window.__POLYORAMA_STARTUP);
            assert(!sample.startup.failure, JSON.stringify(sample.startup.failure));
            assert(sample.startup.milestones.workspace_frame_submitted, 'actual queue submission observation required');
            assert.equal(sample.startup.instantiateStreaming.completed, 1, 'generated page loader must complete streaming WASM init');
            assert.equal(sample.startup.instantiateStreaming.failures, 0, 'page loader must not fall back after a streaming error');
            if (app !== 'gallery') {
              assert.equal(sample.startup.worker?.instantiateStreaming.completed, 1, 'worker loader must complete streaming WASM init');
              assert.equal(sample.startup.worker?.instantiateStreaming.failures, 0);
            }
            sample.resources = await page.evaluate(() => performance.getEntriesByType('resource').map(entry => entry.toJSON()));
            sample.navigation = await page.evaluate(() => performance.getEntriesByType('navigation').map(entry => entry.toJSON()));
            sample.adapters = await page.evaluate(() => window.__startupAdapters);
            sample.input = await visibleInput(page, app, join(output, `${app}-${profile}-${pair}-${visit}`));
            sample.diagnostics = await page.evaluate(app => app === 'lab' ? window.__POLYORAMA_DIAGNOSTICS : app === 'viewer' ? (({ completed, cancelled, stale, decoded_peak_bytes, gpu_peak_bytes, worker, errors, gpu_adapter }) => ({ completed, cancelled, stale, decoded_peak_bytes, gpu_peak_bytes, worker, errors, gpu_adapter }))(window.emuellaViewer.snapshot()) : { story: window.__POLYORAMA_GALLERY_HANDLE.snapshot().story }, app);
            if (app === 'lab') {
              const runtime = sample.diagnostics.runtime;
              assert.equal(runtime.failed, 0); assert.equal(runtime.worker_failures, 0);
              assert(runtime.browser_credits_in_use <= runtime.browser_credit_capacity && runtime.worker_queue_depth <= runtime.external_queue_capacity);
            } else if (app === 'viewer') {
              const d = sample.diagnostics;
              assert.equal(d.errors.length, 0);
              assert(d.decoded_peak_bytes <= 16 << 20 && d.gpu_peak_bytes <= 64 << 20 && d.worker.peak_compressed_bytes <= 64 << 20 && d.worker.peak_descriptor_bytes <= 16 << 20);
            }
            assert.equal(server.requestsDropped, 0, 'HTTP evidence exceeded its bounded recording capacity');
            sample.responses = responses; sample.http = server.requests?.slice(requestStart) ?? null; sample.errors = errors;
            assert.equal(errors.length, 0, errors.join('\n'));
            page.off('pageerror', onError); context.off('response', onResponse);
            await page.goto('about:blank');
            await writeFile(join(output, 'results.json'), `${JSON.stringify(report, null, 2)}\n`);
            console.log(`${app} ${profile} ${pair + 1}/${pairs} ${visit}: workspace=${sample.startup.milestones.rendering_opportunity_proxy.atMs.toFixed(1)} content=${sample.startup.milestones.first_useful_content.atMs.toFixed(1)} ms`);
          }
        } finally { await context.close(); await rm(userDataDir, { recursive: true, force: true }); }
      }
    } finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
  }
  for (const app of apps) for (const profile of profiles) for (const visit of ['cold', 'repeat']) {
    const samples = report.samples.filter(s => s.app === app && s.profile === profile && s.visit === visit);
    report.summary[`${app}/${profile}/${visit}`] = Object.fromEntries(['wasm_init_end', 'framework_start_end', 'workspace_frame_submitted', 'rendering_opportunity_proxy', 'first_useful_content'].map(name => [name, spread(samples.map(s => s.startup?.milestones[name]?.atMs))]));
  }
} catch (error) { report.failure = String(error.stack ?? error); throw error; }
finally { await writeFile(join(output, 'results.json'), `${JSON.stringify(report, null, 2)}\n`); }
