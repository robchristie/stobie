// Bounded diagnostic proof preparation. Importing this module never launches a browser.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, open, readFile, realpath, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { execFileSync } from 'node:child_process';

import { STORE, DISPOSITION, LIMITS as LEGACY_LIMITS, ASSETS, sha256, launchError,
  beneath, inspectTemporaryAlias, boundedFile, pinnedFile, regionGeometry, requestFor,
  tilesFor, maskShape, inspectMask, comparePixels, makePlan as legacyPlan, coverageFor,
  checkMetrics, boundedResponse, faultResponse, maskRoute, failureProof, pressureProof
} from './viewer-acceptance-browser-masks.mjs';
export const LIMITS = Object.freeze({ ...LEGACY_LIMITS, contexts: 10 });
// Same Vulkan hardware setup as the repository's bounded viewer browser probe.
export const BROWSER_ARGS = Object.freeze(['--no-sandbox', '--enable-unsafe-webgpu',
  '--use-angle=vulkan', '--enable-features=Vulkan,CDPScreenshotNewSurface',
  '--disable-vulkan-surface', '--disable-dev-shm-usage', '--disable-background-networking',
  '--disable-breakpad', '--disable-crash-reporter']);
export function hardwareAdapter(adapter) {
  return adapter && adapter.available === true && adapter.fallback !== true
    && /nvidia|amd|intel|qualcomm|apple/i.test(adapter.vendor ?? '')
    && !/swiftshader|llvmpipe|lavapipe|software/i.test([adapter.architecture, adapter.description].join(' '));
}

export function gpuIdentity(info) {
  const gpu = info?.gpu;
  requireThat(Array.isArray(gpu?.devices) && gpu.devices.length > 0 && gpu.devices.length <= 8,
    'bounded GPU system identity required');
  const fields = ['vendorId', 'deviceId', 'vendorString', 'deviceString', 'driverVendor', 'driverVersion'];
  const bounded = value => typeof value === 'string' ? value.slice(0, 512) : Number.isSafeInteger(value) ? value : null;
  return { handshake: 'SystemInfo.getInfo/1', devices: gpu.devices.map(device =>
    Object.fromEntries(fields.map(key => [key, bounded(device[key])]))),
  renderer: bounded(gpu.auxAttributes?.glRenderer),
  vulkan: bounded(gpu.featureStatus?.vulkan), webgpu: bounded(gpu.featureStatus?.webgpu) };
}
const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const PROTOCOL = ['tools/representation-efficiency-browser-masks.mjs',
  'tools/tests/representation-efficiency-browser-masks.test.mjs',
  'docs/representation-efficiency-browser-masks.md', 'tools/viewer-acceptance-browser-masks.mjs'];

const requireThat = (value, message) => { if (!value) throw new Error(message); };
const same = (a, b) => { try { assert.deepEqual(a, b); return true; } catch { return false; } };
const uint = (value, maximum, name) => requireThat(Number.isSafeInteger(value) && value >= 0 && value <= maximum, name);

async function protectedPath(path) {
  requireThat(isAbsolute(path) && beneath(STORE, await realpath(path)), 'protected input outside approved store');
  return path;
}
// One optional alias invariant, never an approval of the host's ordinary /tmp.
export function validateTemporaryAlias(alias) {
  requireThat(alias?.kind === 'private-linux-tmp/1' && alias.path === '/tmp'
    && typeof alias.backing === 'string'
    && dirname(dirname(alias.backing)) === STORE
    && /^representation-efficiency-browser-masks-[a-z0-9][a-z0-9-]{0,100}-execution$/.test(dirname(alias.backing).split('/').at(-1))
    && alias.backing === join(dirname(alias.backing), 'tmp')
    && /^[0-9]+$/.test(alias.dev) && /^[0-9]+$/.test(alias.ino)
    && typeof alias.dev === 'string' && typeof alias.ino === 'string'
    && /^mnt:\[[0-9]+\]$/.test(alias.parent_mount_namespace), 'invalid private temporary alias');
}
export async function temporaryDirectory(alias, output) {
  if (alias === undefined) return { path: join(output, 'tmp'), provenance: null };
  validateTemporaryAlias(alias);
  requireThat(await realpath(STORE) === STORE, 'approved store root alias changed');
  return { path: '/tmp', provenance: await inspectTemporaryAlias(alias) };
}
export function parseArgs(args) {
  const mode = args.shift();
  requireThat(['validate', 'run'].includes(mode), 'expected validate or run');
  const out = { mode };
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i];
    requireThat(['--capsule', '--grant', '--protocol-commit', '--output-name'].includes(key)
      && args[i + 1] && !args[i + 1].startsWith('--') && !(key.slice(2) in out), 'unknown, duplicate or missing argument');
    out[key.slice(2)] = args[i + 1];
  }
  requireThat(out.capsule, '--capsule required');
  if (mode === 'run') requireThat(out.grant && /^[0-9a-f]{40}$/.test(out['protocol-commit'])
    && /^representation-efficiency-browser-masks-[a-z0-9][a-z0-9-]{0,100}$/.test(out['output-name']), 'run requires committed protocol, grant and fresh output name');
  return out;
}

async function pinnedJSON(identity, protectedInput = false) {
  return JSON.parse(await pinnedFile(identity, LIMITS.jsonBytes, protectedInput));
}
async function hashLargeFile(identity) {
  requireThat(identity && isAbsolute(identity.path) && /^[0-9a-f]{64}$/.test(identity.sha256), 'invalid executable identity');
  requireThat((await stat(identity.path)).size === identity.bytes, 'executable size mismatch');
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(identity.path)) hash.update(chunk);
  requireThat(hash.digest('hex') === identity.sha256, 'executable hash mismatch');
}

// Selection and pressure order are inherited verbatim. Cancellation is a separate
// context and cannot warm, split or shrink a pressure request.
export function makePlan(scenes) {
  const plan = legacyPlan(scenes);
  requireThat(plan.pressure_forward_jobs === 47 && plan.pressure_revisit_jobs === 47,
    'inherited 47-window two-pass pressure cohort changed');
  plan.contexts.push({ id: 'cancel-and-recover', kind: 'cancellation', scene: 3,
    jobs: [{ scene: 3, index: 0, cancel: true }, { scene: 3, index: 0 }], maskFiles: 0 });
  return { ...plan, schema: 'representation-efficiency-browser-masks-plan/1', limits: LIMITS,
    browser_launches: 10, workers: 10, planned_jobs: plan.planned_jobs + 2 };
}
export function compactIdentity(manifest, discard, tile) {
  const tag = manifest.identity.validity.tile_sha256[discard]?.[tile];
  requireThat(typeof tag === 'string' && /^[012]:[0-9a-f]{64}$/.test(tag), 'invalid compact mask identity');
  const state = Number(tag[0]);
  return { state, sha256: tag.slice(2), bytes: state === 2 ? maskShape(manifest.identity.profile, discard, tile).bytes + 1 : 1 };
}
export function inspectCompactMask(actual, legacy, manifest, discard, tile, oracle) {
  const id = compactIdentity(manifest, discard, tile);
  requireThat(actual.length === id.bytes && actual[0] === id.state && sha256(actual) === id.sha256,
    'compact mask payload identity mismatch');
  const shape = maskShape(manifest.identity.profile, discard, tile);
  let bitmap;
  if (id.state === 2) bitmap = actual.subarray(1);
  else {
    // Oracle-only expansion of one bounded tile. The production client retains
    // constants; this comparator allocation is outside Worker memory claims.
    bitmap = Buffer.alloc(shape.bytes, id.state === 1 ? 255 : 0);
    if (id.state === 1 && shape.pixels % 8)
      for (let p = 1; p <= shape.bytes / shape.planeBytes; p++) bitmap[p * shape.planeBytes - 1] = (1 << (shape.pixels % 8)) - 1;
  }
  const compared = inspectMask(bitmap, legacy, manifest.identity.profile, discard, tile,
    manifest.identity.validity.bands, oracle);
  return { ...compared, state: id.state, compact_bytes: actual.length, legacy_bytes: legacy.length,
    compact_sha256: sha256(actual) };
}
export function released(metrics) {
  return metrics?.request_working_set_bytes === 0 && metrics?.request_pin_metadata_bytes === 0;
}
export function oversizedRequirement(error) {
  const match = /required compressed bins and masks need (\d+) bytes, limit (\d+) \(image bins (\d+), masks (\d+)\)/.exec(error ?? '');
  if (!match) return null;
  const [total, limit, image, masks] = match.slice(1).map(Number);
  requireThat([total, limit, image, masks].every(Number.isSafeInteger) && total === image + masks
    && limit === LIMITS.pressureCompressedBytes && total > limit, 'inconsistent oversized request accounting');
  return { required_total_bytes: total, limit_bytes: limit, image_bin_bytes: image, mask_bytes: masks,
    excluded: true, completed: false };
}
export function checkCompactMetrics(metrics, compressed) {
  for (const key of ['request_working_set_bytes', 'request_pin_metadata_bytes',
    'peak_request_working_set_bytes', 'peak_request_pin_metadata_bytes', 'compact_catalogue_metadata_bytes',
    'mask_cache_metadata_bytes', 'peak_mask_cache_metadata_bytes', 'mask_cache_entries',
    'mask_cache_slots', 'mask_cache_slot_bytes', 'mask_cache_container_bytes'])
    uint(metrics?.[key], Number.MAX_SAFE_INTEGER, `missing compact metric ${key}`);
  const cacheMetadata = metrics.mask_cache_slots * metrics.mask_cache_slot_bytes + metrics.mask_cache_container_bytes;
  return checkMetrics(metrics, compressed) && metrics.peak_request_working_set_bytes <= compressed
    && metrics.mask_cache_slot_bytes > 0 && Number.isSafeInteger(cacheMetadata)
    && metrics.mask_cache_metadata_bytes === cacheMetadata
    && metrics.mask_cache_entries <= metrics.mask_cache_slots
    && metrics.mask_cache_metadata_bytes <= metrics.peak_mask_cache_metadata_bytes
    && metrics.peak_mask_cache_metadata_bytes <= metrics.peak_descriptor_bytes
    && metrics.mask_cache_metadata_bytes + metrics.compact_catalogue_metadata_bytes <= metrics.descriptor_bytes;
}
function summariseMetrics(metrics) {
  const fields = ['compressed_bytes', 'peak_compressed_bytes', 'descriptor_bytes', 'peak_descriptor_bytes',
    'mask_bytes', 'peak_mask_bytes', 'mask_evictions', 'received_mask_bytes', 'decode_count', 'retries',
    'received_jpp_bytes', 'received_descriptor_bytes', 'peak_codec_workspace_bytes', 'wasm_linear_bytes',
    'request_working_set_bytes', 'request_pin_metadata_bytes', 'peak_request_working_set_bytes',
    'peak_request_pin_metadata_bytes', 'compact_catalogue_metadata_bytes',
    'mask_cache_metadata_bytes', 'peak_mask_cache_metadata_bytes', 'mask_cache_entries',
    'mask_cache_slots', 'mask_cache_slot_bytes', 'mask_cache_container_bytes'];
  return Object.fromEntries(fields.map(k => [k, metrics[k] ?? null]));
}

export async function loadInputs(capsulePath) {
  const capsuleBytes = await boundedFile(capsulePath, LIMITS.jsonBytes), capsule = JSON.parse(capsuleBytes);
  requireThat(capsule.schema === 'representation-efficiency-browser-masks-input/1' && capsule.disposition === DISPOSITION
    && /^[0-9a-f]{40}$/.test(capsule.runtime_commit), 'invalid capsule');
  if (capsule.temporary_alias !== undefined) validateTemporaryAlias(capsule.temporary_alias);
  const service = new URL(capsule.service.url);
  requireThat(service.protocol === 'http:' && ['127.0.0.1', '[::1]'].includes(service.hostname)
    && service.pathname === '/' && !service.search && !service.hash && !service.username && !service.password,
  'service must be an explicit loopback HTTP origin');
  requireThat(/^[0-9a-f]{64}$/.test(capsule.service.catalogue_sha256), 'pinned service catalogue required');
  await pinnedJSON(capsule.service.identity);
  const build = await pinnedJSON(capsule.build_record);
  requireThat(build.runtime_commit === capsule.runtime_commit && /^[0-9a-f]{40}$/.test(build.tree), 'build must identify candidate and tree');
  await hashLargeFile(capsule.chromium);
  requireThat(capsule.playwright_version === '1.62.1', 'Playwright version must remain pinned');
  const staticRoot = await realpath(capsule.static_root), assets = new Map();
  requireThat(Array.isArray(capsule.static_assets) && capsule.static_assets.length <= 32, 'bounded static asset map required');
  for (const entry of capsule.static_assets) {
    requireThat(/^[a-zA-Z0-9_./-]+$/.test(entry.path) && !entry.path.split('/').includes('..')
      && !isAbsolute(entry.path) && !assets.has(`/${entry.path}`), 'invalid or duplicate static asset path');
    const path = await realpath(join(staticRoot, entry.path));
    requireThat(beneath(staticRoot, path), 'static asset escapes pinned root');
    const identity = { ...entry, path };
    // WASM modules are streamed for identity verification, never raster-sized allocations.
    await hashLargeFile(identity); assets.set(`/${entry.path}`, identity);
  }
  for (const path of ['/worker.js', '/response.js', '/pkg/emuella_viewer.js', '/pkg/emuella_viewer_bg.wasm'])
    requireThat(assets.has(path), `missing pinned static asset ${path}`);
  const results = await pinnedJSON(capsule.native_evidence);
  requireThat(results.schema === 'viewer-acceptance-full-scenes-results/1' && results.disposition === DISPOSITION
    && results.results.length === 5 && Array.isArray(capsule.representations) && capsule.representations.length === 5,
  'frozen five-scene native evidence required');
  const scenes = [];
  for (const [i, result] of results.results.entries()) {
    const supplied = capsule.representations[i];
    requireThat(result.asset === ASSETS[i] && supplied.asset === result.asset, 'representation order or identity mismatch');
    const root = await protectedPath(await realpath(supplied.root));
    const conversion = await pinnedJSON(supplied.conversion, true);
    const legacyRoot = await protectedPath(await realpath(join(dirname(result.result.path), 'representation')));
    requireThat(conversion.schema === 'compact-validity-conversion/1' && conversion.legacy_root === legacyRoot
      && conversion.root === root, 'conversion root differs from retained native owner');
    const legacyBytes = await boundedFile(await protectedPath(join(legacyRoot, 'manifest.json')), 1 << 20);
    requireThat(sha256(legacyBytes) === result.storage.manifest_sha256, 'legacy manifest identity mismatch');
    const legacyManifest = JSON.parse(legacyBytes);
    const native = await pinnedJSON(result.evidence.native, true), requests = await pinnedJSON(result.evidence.requests, true);
    const masks = await pinnedJSON(result.evidence.masks, true), agreement = await pinnedJSON(result.evidence.regional_agreement, true);
    const manifestBytes = await boundedFile(await protectedPath(join(root, 'manifest.json')), 1 << 20);
    requireThat(manifestBytes.length === conversion.manifest_bytes, 'compact manifest length mismatch');
    const manifest = JSON.parse(manifestBytes), profile = manifest.identity.profile;
    uint(profile.width, 0xffffffff, 'profile width'); uint(profile.height, 0xffffffff, 'profile height');
    requireThat(profile.width > 0 && profile.height > 0 && profile.width === result.grid.width
      && profile.height === result.grid.height && profile.components === result.grid.components
      && profile.bits_per_sample === result.grid.precision, 'profile differs from native source grid');
    requireThat(manifest.target === result.asset && manifest.tid === conversion.tid && legacyManifest.tid === native.tid && conversion.legacy_tid === native.tid && /^[0-9a-f]{64}$/.test(manifest.tid)
      && manifest.identity.validity.policy === 'source-validity-v2' && profile.tile_edge === 512
      && profile.decomposition_levels === 6 && [1, 3].includes(profile.components), 'native representation contract mismatch');
    requireThat(manifest.identity.validity.source_sha256 === result.source.sha256
      && same(manifest.identity.validity.bands, result.bands), 'original source validity identity mismatch');
    requireThat(requests.length === result.regional_agreement.requests && requests.length <= 1024
      && native.records.length === requests.length && agreement.records.length === requests.length, 'incomplete native records');
    for (const [index, region] of requests.entries()) {
      const geometry = regionGeometry(profile, region), record = agreement.records[index], n = native.records[index];
      requireThat(same(region, record.request) && same(region, n.region)
        && geometry.width === n.width && geometry.height === n.height, 'native request/geometry mismatch');
      requireThat(record.samples.bytes === geometry.width * geometry.height * region.components.length * 2
        && record.validity.bytes === geometry.width * geometry.height, 'native reference length mismatch');
      for (const key of ['samples', 'validity']) {
        await protectedPath(record[key].path);
        requireThat(dirname(await realpath(record[key].path)) === dirname(await realpath(result.evidence.native.path)), 'reference escapes native owner');
        // Hash now and again on use; never retain more than one regional payload.
        await pinnedFile(record[key], LIMITS.decodedPairBytes, true);
      }
    }
    requireThat(masks.complete_source_geometry && masks.tiles === Math.ceil(profile.width / 512) * Math.ceil(profile.height / 512)
      && masks.levels === 7 && manifest.identity.validity.tile_sha256.length === 7, 'mask catalogue coverage incomplete');
    requireThat(manifest.identity.validity.tile_sha256.every(level => Array.isArray(level) && level.length === masks.tiles
      && level.every(digest => /^[012]:[0-9a-f]{64}$/.test(digest))), 'invalid mask identity catalogue');
    requireThat(same(manifest.identity.profile, legacyManifest.identity.profile), 'image profile changed');
    requireThat(conversion.catalogue.length === masks.tiles * masks.levels, 'conversion catalogue incomplete');
    for (let d = 0; d < masks.levels; d++) for (let t = 0; t < masks.tiles; t++) {
      const id = compactIdentity(manifest, d, t);
      const row = conversion.catalogue.find(r => r.discard === d && r.tile === t);
      requireThat(row && row.compact_sha256 === id.sha256 && row.compact_bytes === id.bytes
        && row.legacy_sha256 === legacyManifest.identity.validity.tile_sha256[d][t], 'conversion catalogue identity mismatch');
    }
    scenes.push({ root, legacyRoot, legacyManifest, conversion, manifest, requests, native, masks, agreement, result });
  }
  const plan = makePlan(scenes);
  requireThat(plan.planned_catalogue_masks === LIMITS.maskFiles
    && scenes.reduce((n, s) => n + s.requests.length, 0) === LIMITS.regionalJobs, 'frozen cohort count changed');
  return { capsule, capsule_sha256: sha256(capsuleBytes), scenes, assets, plan };
}

// One monitoring owner; no route transcript, response bodies or unbounded events.
export async function createProofServer(inputs, context, emit, readReference = (identity, maximum) => pinnedFile(identity, maximum, true)) {
  const state = { active: null, fault: null, requests: 0, bytes: 0, inflight: 0,
    errors: 0, errorDetails: [], maskComparisons: 0, maskDisagreements: 0, partialBandCells: 0,
    cancellationRequested: false, onCancel: null, maskReads: new Map(), jobRequests: 0, jobMaskReads: [], maximumInflight: 0 };
  const scenesByTarget = new Map(inputs.scenes.map(s => [s.manifest.target, s]));
  const controllers = new Set(); let drained;
  const drain = new Promise(resolve => { drained = resolve; });
  let closing = false;
  const respond = (out, status, bytes, type = 'application/octet-stream', extra = {}) => {
    state.bytes += bytes.length;
    requireThat(state.bytes <= 2 ** 31, 'context HTTP byte budget exceeded');
    out.writeHead(status, { ...extra, 'content-type': type, 'content-length': bytes.length,
      'cache-control': 'no-store', 'x-content-type-options': 'nosniff' }); out.end(bytes);
  };
  const server = createServer(async (incoming, outgoing) => {
    state.inflight++; state.maximumInflight = Math.max(state.maximumInflight, state.inflight);
    const controller = new AbortController(); controllers.add(controller);
    const deadline = setTimeout(() => controller.abort(), LIMITS.httpMs);
    outgoing.on('close', () => { if (!outgoing.writableFinished) controller.abort(); });
    try {
      requireThat(!closing, 'proxy is closing');
      requireThat(state.inflight <= LIMITS.httpConcurrent && incoming.method === 'GET', 'HTTP concurrency or method bound');
      state.requests++; state.jobRequests++;
      requireThat(state.requests <= context.maskFiles + context.jobs.length * LIMITS.httpRequestsPerJob + 64,
        'context HTTP request budget exceeded');
      requireThat(state.jobRequests <= LIMITS.httpRequestsPerJob, 'job HTTP request budget exceeded');
      const url = new URL(incoming.url, 'http://127.0.0.1');
      requireThat(incoming.url.startsWith('/') && !incoming.url.startsWith('//') && incoming.url.length <= 65536, 'invalid local route');
      if (url.pathname === '/proof') {
        respond(outgoing, 200, Buffer.from('<!doctype html><meta charset="utf-8"><title>Bounded worker proof</title><link rel="icon" href="data:,">'), 'text/html'); return;
      }
      const asset = inputs.assets.get(url.pathname);
      if (asset) {
        requireThat(!url.search, 'static asset query denied');
        await hashLargeFile(asset);
        const type = url.pathname.endsWith('.wasm') ? 'application/wasm' : 'text/javascript';
        // Static code is streamed; no unpinned paths are served.
        outgoing.writeHead(200, { 'content-type': type, 'content-length': asset.bytes, 'cache-control': 'no-store' });
        await new Promise((resolve, reject) => {
          const stream = createReadStream(asset.path); stream.on('error', reject);
          outgoing.on('error', reject); outgoing.on('finish', resolve);
          outgoing.on('close', () => { stream.destroy(); if (!outgoing.writableFinished) reject(new Error('static stream closed')); });
          stream.pipe(outgoing);
        }); return;
      }
      if (url.pathname === '/expected/samples' || url.pathname === '/expected/validity') {
        requireThat(state.active && !url.search, 'no active native reference');
        const { scene, index } = state.active, kind = url.pathname.split('/').at(-1);
        const identity = inputs.scenes[scene].agreement.records[index][kind];
        const bytes = await readReference(identity, LIMITS.decodedPairBytes);
        respond(outgoing, 200, bytes); return;
      }
      const mask = /^\/mask\/([^/]+)\/(\d+)\/(\d+)$/.exec(url.pathname);
      const descriptor = /^\/descriptor\/([^/]+)\/(\d+)$/.exec(url.pathname);
      requireThat(url.pathname === '/catalogue' || mask || descriptor || url.pathname === '/jpip', 'route not authorised');
      if (mask || descriptor) {
        const scene = scenesByTarget.get((mask || descriptor)[1]);
        requireThat(scene && url.search === `?tid=${scene.manifest.tid}`, 'HTTP representation identity mismatch');
        if (mask) maskShape(scene.manifest.identity.profile, Number(mask[2]), Number(mask[3]));
      }
      if (url.pathname === '/jpip') {
        requireThat(state.active, 'JPP outside active worker job');
        const scene = inputs.scenes[state.active.scene];
        requireThat(url.searchParams.get('target') === scene.manifest.target, 'JPP target differs from active native request');
      }
      if (mask && state.active?.cancel && !state.cancellationRequested) {
        requireThat(typeof state.onCancel === 'function', 'missing cancellation observer');
        state.cancellationRequested = true;
        await state.onCancel();
        respond(outgoing, 200, Buffer.from([0])); return;
      }
      const fault = mask && state.active && ['missing', 'corrupt', 'stale'].includes(context.kind) && !state.fault;
      let upstreamPath = incoming.url;
      if (fault && context.kind === 'stale') {
        url.searchParams.set('tid', '0'.repeat(64)); upstreamPath = url.pathname + url.search;
      }
      let status, bytes, headers = {};
      if (fault && context.kind === 'missing') ({ status, body: bytes } = faultResponse('missing'));
      else {
        const upstream = await fetch(new URL(upstreamPath, inputs.capsule.service.url),
          { redirect: 'error', signal: controller.signal, headers: { 'accept-encoding': 'identity' } });
        status = upstream.status;
        bytes = await boundedResponse(upstream, url.pathname === '/catalogue' ? LIMITS.catalogueBytes : LIMITS.responseBytes);
        headers = Object.fromEntries([...upstream.headers].filter(([key]) => !['content-length', 'content-encoding',
          'transfer-encoding', 'connection', 'set-cookie', 'content-type'].includes(key)));
        if (fault && context.kind === 'corrupt') {
          const scene = scenesByTarget.get(mask[1]), discard = Number(mask[2]), tile = Number(mask[3]);
          const identity = compactIdentity(scene.manifest, discard, tile);
          requireThat(bytes.length === identity.bytes && sha256(bytes) === identity.sha256, 'original fault response identity mismatch');
          ({ status, body: bytes } = faultResponse('corrupt', bytes, status));
        }
      }
      if (fault) state.fault = { kind: context.kind, target: mask[1], discard: Number(mask[2]), tile: Number(mask[3]),
        status, bytes: bytes.length, forwarded_sha256: sha256(bytes), stale_identity_rejected: context.kind === 'stale'
          ? status === 400 && bytes.toString('utf8').includes('stale mask identity') : null };
      if (url.pathname === '/catalogue') {
        requireThat(status === 200 && sha256(bytes) === inputs.capsule.service.catalogue_sha256, 'service catalogue identity mismatch');
        const catalogue = JSON.parse(bytes);
        requireThat(catalogue.length === inputs.scenes.length && inputs.scenes.every(s =>
          catalogue.some(m => same(m, s.manifest))), 'service/native manifests differ');
      }
      if (mask && !fault && status === 200) {
        const scene = scenesByTarget.get(mask[1]), discard = Number(mask[2]), tile = Number(mask[3]);
        const path = join(scene.legacyRoot, `masks/${discard}/${tile}.bin`);
        const shape = maskShape(scene.manifest.identity.profile, discard, tile);
        const expected = await readReference({ path, bytes: shape.bytes,
          sha256: scene.legacyManifest.identity.validity.tile_sha256[discard][tile] }, LIMITS.responseBytes);
        const audit = inspectCompactMask(bytes, expected, scene.manifest, discard, tile, scene.masks.records);
        state.maskComparisons++; state.maskDisagreements += Number(!audit.agrees);
        state.partialBandCells += audit.partial_band_cells;
        const key = `${scene.manifest.tid}/${discard}/${tile}`;
        state.maskReads.set(key, (state.maskReads.get(key) ?? 0) + 1);
        if (state.active) state.jobMaskReads.push(key);
        await emit({ kind: 'mask', target: mask[1], context: context.id,
          source: state.active ? 'worker-fetch' : 'catalogue-fetch', ...audit });
      }
      respond(outgoing, status, bytes, url.pathname === '/catalogue' ? 'application/json' : 'application/octet-stream', headers);
    } catch (error) {
      state.errors++;
      if (state.errorDetails.length < 16) state.errorDetails.push(String(error.message).slice(0, 512));
      // Keep protected service error bodies out of generic process output.
      if (!outgoing.headersSent) respond(outgoing, 502, Buffer.from('bounded proof proxy failure'));
      else outgoing.destroy();
    } finally {
      clearTimeout(deadline); controllers.delete(controller); state.inflight--;
      if (closing && !state.inflight) drained();
    }
  });
  server.requestTimeout = LIMITS.httpMs;
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return { state, url: `http://127.0.0.1:${server.address().port}`, server,
    close: async () => {
      closing = true; for (const controller of controllers) controller.abort();
      server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
      if (state.inflight) await drain;
    } };
}

// Runs only after committed-input and grant checks. No application UI is loaded.
async function installPageWorker(page, compressed) {
  await page.evaluate(({ compressed, comparator }) => {
    window.__comparePixels = (0, eval)(`(${comparator})`);
    const worker = new Worker('/worker.js', { type: 'module', name: 'emuella-mask-proof' });
    window.__proof = { worker, pending: null, unexpected: 0, terminalEvents: 0, workerErrors: 0 };
    worker.onmessage = ({ data }) => {
      const state = window.__proof;
      if (data.Completed || data.Failed || data.Cancelled) state.terminalEvents++;
      if (!state.pending) { state.unexpected++; return; }
      const pending = state.pending; state.pending = null;
      clearTimeout(pending.timer); pending.resolve(data);
    };
    worker.onerror = () => {
      const state = window.__proof; state.workerErrors++;
      if (state.pending) {
        clearTimeout(state.pending.timer); state.pending.reject(new Error('worker error')); state.pending = null;
      }
    };
    window.__send = data => new Promise((resolve, reject) => {
      if (window.__proof.pending) { reject(new Error('one-job queue exceeded')); return; }
      const timer = setTimeout(() => { window.__proof.pending = null; worker.terminate(); reject(new Error('worker deadline')); }, 60000);
      window.__proof.pending = { resolve, reject, timer }; worker.postMessage(data);
    });
    window.__initialised = window.__send({ kind: 'init', server: location.origin, compressed });
  }, { compressed, comparator: comparePixels.toString() });
  const initialised = await page.evaluate(async () => Boolean((await window.__initialised).Catalogue));
  requireThat(initialised, 'worker catalogue initialisation failed');
  await page.evaluate(() => { window.__initialised = null; });
}
async function compareJob(page, scene, index, sequence) {
  const region = scene.requests[index], n = scene.native.records[index];
  const request = requestFor(scene.manifest, region, sequence);
  return page.evaluate(async ({ request, manifest, expected }) => {
    let event, sampleBytes, validityBytes;
    try {
      event = await window.__send({ kind: 'job', job: { request, manifest } });
      const kind = ['Completed', 'Failed', 'Cancelled'].find(k => event[k]);
      if (!kind) throw new Error('unexpected worker envelope');
      const value = event[kind];
      // Structural equality of the token and full request prevents stale attribution.
      const stable = value => {
        if (Array.isArray(value)) return value.map(stable);
        if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(k => [k, stable(value[k])]));
        return value;
      };
      const requestAgrees = JSON.stringify(stable(value.request)) === JSON.stringify(stable(request));
      let comparison = null;
      if (kind === 'Completed') {
        const pixels = value.pixels;
        if (!(pixels.samples instanceof Uint16Array) || !(pixels.validity instanceof Uint8Array)
          || pixels.samples.byteLength * 2 + pixels.validity.byteLength * 2 > request.max_decoded_bytes)
          throw new Error('actual transferred arrays exceed reservation or have wrong type');
        const read = async path => {
          const response = await fetch(path, { cache: 'no-store' });
          if (!response.ok) throw new Error('native reference HTTP failure');
          // The local server independently enforces pinned lengths and the same bound.
          const bytes = new Uint8Array(await response.arrayBuffer());
          if (bytes.length > request.max_decoded_bytes) throw new Error('native reference byte bound');
          return bytes;
        };
        sampleBytes = await read('/expected/samples'); validityBytes = await read('/expected/validity');
        comparison = window.__comparePixels(pixels, sampleBytes, validityBytes, expected);
      }
      const { timing: _timing, elapsed_ms: _elapsed, ...metrics } = value.metrics ?? {};
      return { kind, request_agrees: requestAgrees, comparison, metrics,
        error: typeof value.error === 'string' ? value.error.slice(0, 1024) : null,

        cross_realm_intervals_unavailable: true };
    } finally {
      // Do not retain Completed, its typed buffers, or reference buffers between jobs.
      if (event?.Completed) event.Completed.pixels = null;
      event = null; sampleBytes = null; validityBytes = null;
    }
  }, { request, manifest: scene.manifest, expected: { width: n.width, height: n.height,
    precision: scene.manifest.identity.profile.bits_per_sample, layout: region.components.length === 1 ? 'Scalar' : 'Rgb' } });
}

async function executeContext(inputs, planned, directory, chromium, emit) {
  await mkdir(directory); await mkdir(join(directory, 'profile')); await mkdir(join(directory, 'downloads'));
  const proxy = await createProofServer(inputs, planned, emit);
  const report = { id: planned.id, kind: planned.kind, status: 'partial', jobs: [],
    browser_version: null, workers: 0, page_errors: 0, missing_proof: [] };
  let browser, timer, timedOut = false;
  try {
    timer = setTimeout(() => { timedOut = true; if (browser) void browser.close(); }, LIMITS.contextMs);
    const temporary = await temporaryDirectory(inputs.capsule.temporary_alias, dirname(directory));
    if (temporary.provenance) {
      report.temporary_alias = temporary.provenance;
      await writeFile(join(directory, 'temporary-alias.json'), JSON.stringify(temporary.provenance, null, 2) + '\n', { flag: 'wx' });
    }
    browser = await chromium.launchPersistentContext(join(directory, 'profile'), {
      executablePath: inputs.capsule.chromium.path, headless: true, timeout: LIMITS.jobMs,
      acceptDownloads: false, downloadsPath: join(directory, 'downloads'),
      serviceWorkers: 'block', args: [...BROWSER_ARGS],
      env: { ...process.env, TMPDIR: temporary.path } });
    report.browser_version = browser.browser()?.version() ?? null;
    // A single CDP handshake completes GPU discovery before the sole adapter
    // observation. It performs no Worker job, image transfer or application pump.
    const system = await browser.browser().newBrowserCDPSession();
    try { report.gpu_system = gpuIdentity(await system.send('SystemInfo.getInfo')); }
    finally { await system.detach(); }
    // This local proxy is the sole network destination, including Worker fetches.
    await browser.route('**/*', route => route.request().url().startsWith(`${proxy.url}/`)
      ? route.continue() : route.abort('blockedbyclient'));
    const page = browser.pages()[0] ?? await browser.newPage();
    page.on('worker', () => report.workers++); page.on('pageerror', () => report.page_errors++);
    await page.goto(`${proxy.url}/proof`, { timeout: LIMITS.jobMs });
    report.webgpu_adapter = await page.evaluate(async () => {
      const adapter = await navigator.gpu?.requestAdapter({ powerPreference: 'high-performance', forceFallbackAdapter: false });
      if (!adapter) return { available: false };
      const info = adapter.info;
      return { available: true, vendor: info.vendor, architecture: info.architecture,
        device: info.device, description: info.description,
        fallback: adapter.isFallbackAdapter ?? info.isFallbackAdapter ?? null };
    });
    requireThat(hardwareAdapter(report.webgpu_adapter), 'hardware WebGPU adapter not observed');
    const compressed = planned.kind === 'pressure' ? LIMITS.pressureCompressedBytes : LIMITS.compressedBytes;
    await installPageWorker(page, compressed);
    if (planned.kind === 'agreement') {
      const scene = inputs.scenes[planned.scene];
      for (let discard = 0; discard < 7; discard++) for (let tile = 0; tile < scene.masks.tiles; tile++) {
        requireThat(!timedOut, 'context deadline'); proxy.state.jobRequests = 0;
        const status = await page.evaluate(async path => {
          const response = await fetch(path, { cache: 'no-store' });
          // Drain one bounded HTTP mask at a time; proxy compares its actual body.
          const bytes = await response.arrayBuffer();
          return { status: response.status, bytes: bytes.byteLength };
        }, maskRoute(scene.manifest, discard, tile));
        requireThat(status.status === 200, 'catalogue mask delivery incomplete');
      }
    }
    for (const [sequence, job] of planned.jobs.entries()) {
      requireThat(!timedOut, 'context deadline');
      proxy.state.active = job; proxy.state.jobRequests = 0; proxy.state.jobMaskReads = [];
      proxy.state.onCancel = job.cancel ? () => page.evaluate(request => {
        window.__proof.worker.postMessage({ kind: 'cancel', request });
      }, requestFor(inputs.scenes[job.scene].manifest, inputs.scenes[job.scene].requests[job.index], sequence)) : null;
      const scene = inputs.scenes[job.scene], result = await compareJob(page, scene, job.index, sequence);
      let boundsAgree = false, metricError = null;
      try { boundsAgree = checkCompactMetrics(result.metrics, compressed); }
      catch (error) { metricError = String(error.message).slice(0, 512); }
      const row = { ...job, ...result, bounds_agree: boundsAgree, metric_error: metricError,
        reservation_released: released(result.metrics),
        oversized: planned.kind === 'pressure' && result.kind === 'Failed' ? oversizedRequirement(result.error) : null,
        mask_reads: [...proxy.state.jobMaskReads], native_samples: scene.agreement.records[job.index].samples.sha256,
        native_validity: scene.agreement.records[job.index].validity.sha256 };
      row.metrics = summariseMetrics(result.metrics ?? {});
      report.jobs.push(row); await emit({ kind: 'job', context: planned.id, ...row });
      proxy.state.active = null;
    }
    // Bounded quiet observation catches duplicate/unexpected terminal envelopes.
    await page.waitForTimeout(50);
    const worker = await page.evaluate(() => {
      const p = window.__proof; p.worker.terminate();
      return { unexpected: p.unexpected, terminal_events: p.terminalEvents, worker_errors: p.workerErrors };
    });
    report.worker = worker;
    if (report.workers !== 1 || worker.unexpected || worker.worker_errors || report.page_errors
      || worker.terminal_events !== planned.jobs.length) report.missing_proof.push('worker-envelope-or-context-count');
    if (report.jobs.some(r => !r.bounds_agree || !r.request_agrees || !r.reservation_released)) report.missing_proof.push('worker-budget-or-request-identity');
    if (planned.kind === 'agreement') {
      report.coverage = coverageFor(inputs.scenes[planned.scene], report.jobs);
      if (!report.coverage.proved) report.missing_proof.push('regional-boundary-or-selection-coverage');
      if (report.coverage.source_boundaries.length && !proxy.state.partialBandCells) report.missing_proof.push('partial-reduced-mask-cells');
      if (report.jobs.some(r => r.kind !== 'Completed' || !r.comparison?.samples_agree || !r.comparison?.validity_agrees))
        report.missing_proof.push('regional-samples-or-validity');
      if (proxy.state.maskComparisons < planned.maskFiles || proxy.state.maskDisagreements) report.missing_proof.push('exact-mask-delivery');
    } else if (planned.kind === 'pressure') {
      report.pressure = pressureProof(report.jobs.slice(0, planned.forwardJobs), report.jobs.slice(planned.forwardJobs),
        Math.max(0, ...report.jobs.map(r => Number.isSafeInteger(r.metrics.mask_evictions) ? r.metrics.mask_evictions : 0)));
      report.pressure.strict_47_window_completion = report.pressure.proved;
      report.pressure.exclusions = report.jobs.filter(r => r.oversized).map(r => ({ scene: r.scene, index: r.index, ...r.oversized }));
      report.pressure.completed_jobs = report.jobs.filter(r => r.kind === 'Completed').length;
      report.pressure.lifecycle_proved = report.pressure.actual_mask_evictions > 0
        && report.pressure.refetched_previously_loaded_masks > 0
        && report.jobs.length === 94 && report.jobs.every(r => r.reservation_released && r.bounds_agree && r.request_agrees
          && (r.kind === 'Completed' && r.comparison?.samples_agree && r.comparison?.validity_agrees
            || r.kind === 'Failed' && r.oversized));
      if (!report.pressure.lifecycle_proved) report.missing_proof.push('mask-lifecycle-or-unclassified-pressure-failure');
    } else if (planned.kind === 'cancellation') {
      const [cancelled, recovered] = report.jobs;
      report.cancellation_proved = proxy.state.cancellationRequested && cancelled?.kind === 'Cancelled'
        && cancelled.request_agrees && cancelled.reservation_released && cancelled.metrics.decode_count === 0
        && recovered?.kind === 'Completed' && recovered.reservation_released
        && recovered.comparison?.samples_agree && recovered.comparison?.validity_agrees;
      if (!report.cancellation_proved) report.missing_proof.push('cancellation-release-and-recovery');
    } else {
      report.failure_proved = failureProof(planned.kind, report.jobs[0], proxy.state.fault);
      if (!report.failure_proved) report.missing_proof.push('required-mask-failure-before-publication');
    }
    if (proxy.state.errors) report.missing_proof.push('proxy-input-or-bound-failure');
    if (!report.missing_proof.length) report.status = 'complete-diagnostic';
  } catch (error) {
    if (!browser) {
      const secrets = Object.entries(process.env).filter(([key]) => /token|secret|password|passwd|credential|api.?key|authorization|cookie/i.test(key)).map(([, value]) => value);
      report.launch_error = launchError(error, secrets);
      report.error = report.launch_error.header;
    } else report.error = String(error.message).slice(0, 1024);
    report.missing_proof.push(timedOut ? 'context-deadline' : 'context-operation-failed');
  } finally {
    clearTimeout(timer); if (browser) await browser.close().catch(() => {});
    await proxy.close();
    report.http = { requests: proxy.state.requests, body_bytes: proxy.state.bytes, maximum_inflight: proxy.state.maximumInflight,
      errors: proxy.state.errors, error_details: proxy.state.errorDetails, mask_comparisons: proxy.state.maskComparisons,
      mask_disagreements: proxy.state.maskDisagreements, partial_band_cells: proxy.state.partialBandCells, fault: proxy.state.fault };
    await writeFile(join(directory, 'result.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
  }
  return report;
}

export function committedProtocol(commit) {
  requireThat(/^[0-9a-f]{40}$/.test(commit), 'full committed revision required');
  const files = {};
  for (const path of PROTOCOL) {
    const committed = execFileSync('git', ['-C', REPO, 'show', `${commit}:${path}`], { maxBuffer: LIMITS.jsonBytes, stdio: ['ignore', 'pipe', 'pipe'] });
    const local = execFileSync('git', ['-C', REPO, 'hash-object', path], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
    const frozen = execFileSync('git', ['-C', REPO, 'rev-parse', `${commit}:${path}`], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
    requireThat(local === frozen, 'protocol file differs from committed bytes'); files[path] = sha256(committed);
  }
  return files;
}
export function validateGrant(grant, args, inputs, files) {
  requireThat(grant.schema === 'representation-efficiency-browser-masks-grant/1' && grant.disposition === DISPOSITION
    && grant.protocol_commit === args['protocol-commit'] && same(grant.protocol_files, files)
    && grant.capsule_sha256 === inputs.capsule_sha256 && grant.plan_sha256 === sha256(JSON.stringify(inputs.plan))
    && grant.output_name === args['output-name'] && grant.max_invocations === 1
    && grant.max_browser_launches === LIMITS.contexts && grant.runner_retries === 0,
  'grant does not authorise this exact invocation');
  requireThat(typeof grant.operation_owner === 'string' && grant.operation_owner.length > 0
    && typeof grant.attribution === 'string' && grant.attribution.length > 0
    && Array.isArray(grant.lineage) && grant.lineage.length > 0 && grant.lineage.length <= 16
    && grant.lineage.every(x => typeof x === 'string' && x.length > 0), 'fresh attribution, lineage and operation owner required');
}
async function run(args) {
  const inputs = await loadInputs(args.capsule);
  if (args.mode === 'validate') {
    console.log(JSON.stringify({ status: 'validated-without-browser', capsule_sha256: inputs.capsule_sha256,
      plan_sha256: sha256(JSON.stringify(inputs.plan)), browser_launches: inputs.plan.browser_launches,
      planned_jobs: inputs.plan.planned_jobs, planned_catalogue_masks: inputs.plan.planned_catalogue_masks,
      pressure_forward_jobs: inputs.plan.pressure_forward_jobs, pressure_revisit_jobs: inputs.plan.pressure_revisit_jobs })); return;
  }
  const grantBytes = await boundedFile(args.grant, LIMITS.jsonBytes), grant = JSON.parse(grantBytes);
  const files = committedProtocol(args['protocol-commit']); validateGrant(grant, args, inputs, files);
  requireThat(inputs.capsule.runtime_commit === args['protocol-commit'], 'runtime must match committed candidate');
  requireThat(execFileSync('git', ['-C', REPO, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() === inputs.capsule.runtime_commit,
    'checkout must be frozen at candidate');
  execFileSync('git', ['-C', REPO, 'diff', '--quiet', 'HEAD', '--'], { stdio: 'pipe' });
  requireThat(await realpath(STORE) === STORE, 'approved store root alias changed');
  const output = join(STORE, args['output-name']);
  if (inputs.capsule.temporary_alias !== undefined) {
    const execution = `${output}-execution`;
    requireThat(inputs.capsule.temporary_alias.backing === join(execution, 'tmp')
      && dirname(await realpath(args.capsule)) === execution && dirname(await realpath(args.grant)) === execution,
    'temporary alias must belong to this approved execution group');
    const lineage = JSON.parse(await boundedFile(join(execution, 'lineage.json'), LIMITS.jsonBytes));
    requireThat(same(lineage, { operation_owner: grant.operation_owner, attribution: grant.attribution, lineage: grant.lineage }),
      'temporary alias execution lineage differs from grant');
  }
  const temporary = await temporaryDirectory(inputs.capsule.temporary_alias, output);
  await mkdir(output); // Exclusive; never remove or reuse.
  const result = { schema: 'representation-efficiency-browser-masks-result/1', status: 'partial', disposition: DISPOSITION,
    protocol_commit: args['protocol-commit'], runtime_commit: inputs.capsule.runtime_commit,
    capsule_sha256: inputs.capsule_sha256, grant_sha256: sha256(grantBytes),
    operation_owner: grant.operation_owner, planned_jobs: inputs.plan.planned_jobs,
    contexts: [], missing_proof: [], viewer_acceptance: false, selected_configuration: null,
    gpu_viewing: 'unmeasured', normal_latency: 'unmeasured', inherited_seven_event_recovery: 'unmeasured',
    cross_realm_intervals_unavailable: true, speed_claim: null, memory_claim: 'Worker accounting; no process RSS or GPU measurement',
    storage: inputs.scenes.map(s => ({ target: s.manifest.target, conversion: s.conversion })) };
  let journal;
  try {
    await writeFile(join(output, 'lineage.json'), JSON.stringify({ attribution: grant.attribution, lineage: grant.lineage,
      operation_owner: grant.operation_owner, capsule_sha256: inputs.capsule_sha256, grant_sha256: sha256(grantBytes) }, null, 2) + '\n', { flag: 'wx' });
    await writeFile(join(output, 'plan.json'), JSON.stringify(inputs.plan, null, 2) + '\n', { flag: 'wx' });
    await writeFile(join(output, 'inputs.json'), JSON.stringify({ capsule: inputs.capsule, grant, protocol_files: files }, null, 2) + '\n', { flag: 'wx' });
    if (temporary.provenance) await writeFile(join(output, 'temporary-alias.json'), JSON.stringify(temporary.provenance, null, 2) + '\n', { flag: 'wx' });
    for (const child of ['tmp', 'cache', 'config']) await mkdir(join(output, child));
    process.env.TMPDIR = temporary.path;
    process.env.XDG_CACHE_HOME = join(output, 'cache'); process.env.XDG_CONFIG_HOME = join(output, 'config');
    const playwrightPackage = JSON.parse(await readFile(join(REPO, 'node_modules/playwright/package.json'), 'utf8'));
    requireThat(playwrightPackage.version === inputs.capsule.playwright_version, 'installed Playwright version differs');
    const { chromium } = await import('playwright');
    journal = await open(join(output, 'comparisons.jsonl'), 'wx');
    let emitted = 0;
    const emit = async row => {
      emitted++;
      requireThat(emitted <= LIMITS.maskFiles + inputs.plan.planned_jobs * LIMITS.httpRequestsPerJob, 'comparison record bound exceeded');
      await journal.write(JSON.stringify(row) + '\n');
    };
    const started = performance.now();
    for (const planned of inputs.plan.contexts) {
      if (performance.now() - started >= LIMITS.outerMs) { result.missing_proof.push('outer-deadline'); break; }
      const context = await executeContext(inputs, planned, join(output, planned.id), chromium, emit);
      result.contexts.push({ id: context.id, status: context.status, jobs: context.jobs.length,
        missing_proof: context.missing_proof, http: context.http, coverage: context.coverage ?? null, pressure: context.pressure ?? null,
        cancellation_proved: context.cancellation_proved ?? null, failure_proved: context.failure_proved ?? null,
        webgpu_adapter: context.webgpu_adapter ?? null, gpu_system: context.gpu_system ?? null });
      // Checkpoint only bounded aggregates. Every completed or failed context is retained.
      await emit({ kind: 'context-summary', ...result.contexts.at(-1) });
    }
    for (const asset of inputs.assets.values()) await hashLargeFile(asset);
    await hashLargeFile(inputs.capsule.chromium);
    // Recheck immutable native metadata and reference bindings without re-execution.
    const after = await loadInputs(args.capsule);
    requireThat(after.capsule_sha256 === inputs.capsule_sha256, 'capsule changed during execution');
    result.immutable_inputs_rechecked = true;
    if (result.contexts.length !== LIMITS.contexts) result.missing_proof.push('unattempted-contexts');
    if (result.contexts.some(c => c.status !== 'complete-diagnostic')) result.missing_proof.push('incomplete-context-proof');
    if (!result.missing_proof.length) {
      const pressure = result.contexts.find(c => c.pressure)?.pressure;
      result.strict_pressure_completed = pressure?.strict_47_window_completion === true;
      result.status = result.strict_pressure_completed ? 'complete-scoped-diagnostic' : 'complete-scoped-with-pressure-exclusions';
    }
  } catch (error) {
    result.missing_proof.push('execution-or-identity-failure'); result.error = String(error.message).slice(0, 1024);
  } finally {
    if (journal) await journal.close();
    await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  }
  console.log(JSON.stringify({ status: result.status, output }));
  if (result.missing_proof.length) process.exitCode = 2;
}
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try { await run(parseArgs(process.argv.slice(2))); }
  catch { console.error('Browser mask proof did not start or complete; input/grant/preflight failure. No automatic retry.'); process.exitCode = 2; }
}
