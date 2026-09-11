// Bounded diagnostic proof preparation. Importing this module never launches a browser.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, open, readFile, readlink, realpath, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { dirname, isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { execFileSync } from 'node:child_process';

export const STORE = '/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1';
export const DISPOSITION = 'quality-rejected-diagnostic-only';
export const LIMITS = Object.freeze({ contexts: 9, regionalJobs: 2212, maskFiles: 1883,
  pressureForward: 64, inflightJobs: 1, httpConcurrent: 2, httpRequestsPerJob: 128,
  responseBytes: 8 << 20, jsonBytes: 8 << 20, catalogueBytes: 16 << 20,
  decodedPairBytes: 4 << 20, compressedBytes: 64 << 20, pressureCompressedBytes: 1 << 20,
  descriptorBytes: 16 << 20, codecWorkspaceBytes: 64 << 20,
  jobMs: 60000, contextMs: 660000, outerMs: 6000000, httpMs: 30000 });
export const ASSETS = ['94_104001000B823500-PAN16', '94_104001000B823500-RGB16',
  '106_10400100413CDF00-PAN16', '106_10400100413CDF00-RGB16', '105_104001002F92BB00-RGB8'];
const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const PROTOCOL = ['tools/viewer-acceptance-browser-masks.mjs',
  'tools/tests/viewer-acceptance-browser-masks.test.mjs', 'docs/viewer-acceptance-browser-masks.md',
  'docs/viewer-acceptance-browser-environment.md'];
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
// Failure-only launch diagnostics; never subscribe to browser logs or page events.
export function launchError(error, secrets = []) {
  const original = String(error?.message ?? error);
  const sanitise = text => text.replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, '')
    .replace(/(?:\x1b\[|\u009b)[0-?]*[ -/]*[@-~]/g, '')
    .replace(/\t/g, ' ')
    .replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f\u200b-\u200f\u202a-\u202e\u2060-\u206f]/g, '');
  let clean = sanitise(original);
  for (const secret of secrets.filter(x => typeof x === 'string').map(sanitise).filter(Boolean).sort((a, b) => b.length - a.length))
    clean = clean.split(secret).join('[REDACTED]');
  clean = clean.replace(/([a-z][a-z0-9+.-]*:\/\/)[^\s/@]+:[^\s/@]+@/gi, '$1[REDACTED]@')
    .replace(/\b(Bearer|Basic)\s+[^\s,;]+/gi, '$1 [REDACTED]')
    .replace(/((?:[\w-]*(?:token|secret|password|passwd|credential|api[_-]?key)[\w-]*|authorization|cookie)(?:\s*[=:]\s*|\s+))(?:"[^"]*"|'[^']*'|[^\s&;,]+)/gi, '$1[REDACTED]');
  const cap = (s, bytes, tail = false) => {
    const chars = Array.from(s); if (tail) chars.reverse();
    let used = 0, kept = [];
    for (const c of chars) { const n = Buffer.byteLength(c); if (used + n > bytes) break; kept.push(c); used += n; }
    return (tail ? kept.reverse() : kept).join('');
  };
  const headerLine = clean.split('\n', 1)[0], header = cap(headerLine, 512);
  const remainder = clean.slice(header.length).replace(/^\n/, ''), tail = cap(remainder, 4096, true);
  const exit = [...clean.matchAll(/\bexitCode\s*[=:]\s*(null|-?\d+)\b/g)].at(-1);
  const signal = [...clean.matchAll(/\bsignal\s*[=:]\s*(null|SIG[A-Z0-9]+)\b/g)].at(-1);
  const exitProperty = error?.exitCode ?? error?.code;
  return { header, tail, original_characters: original.length, character_unit: 'UTF-16 code units',
    sanitised_characters: clean.length, sanitised_bytes: Buffer.byteLength(clean),
    header_bytes: Buffer.byteLength(header), tail_bytes: Buffer.byteLength(tail),
    truncated: header.length < headerLine.length || tail.length < remainder.length,
    sanitised: clean !== original,
    exit_code: exit ? (exit[1] === 'null' ? null : Number(exit[1])) : (Number.isSafeInteger(exitProperty) ? exitProperty : null),
    signal: signal ? (signal[1] === 'null' ? null : signal[1]) : (/^SIG[A-Z0-9]+$/.test(error?.signal) ? error.signal : null) };
}
const requireThat = (value, message) => { if (!value) throw new Error(message); };
const same = (a, b) => { try { assert.deepEqual(a, b); return true; } catch { return false; } };
const uint = (value, maximum, name) => requireThat(Number.isSafeInteger(value) && value >= 0 && value <= maximum, name);
export function beneath(root, path) {
  const rel = relative(resolve(root), resolve(path));
  return rel !== '' && !rel.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`) && rel !== '..' && !isAbsolute(rel);
}
async function protectedPath(path) {
  requireThat(isAbsolute(path) && beneath(STORE, await realpath(path)), 'protected input outside approved store');
  return path;
}
// One optional alias invariant, never an approval of the host's ordinary /tmp.
export function validateTemporaryAlias(alias) {
  requireThat(alias?.kind === 'private-linux-tmp/1' && alias.path === '/tmp'
    && typeof alias.backing === 'string'
    && dirname(dirname(alias.backing)) === STORE
    && /^viewer-acceptance-browser-masks-[a-z0-9][a-z0-9-]{0,100}-execution$/.test(dirname(alias.backing).split('/').at(-1))
    && alias.backing === join(dirname(alias.backing), 'tmp')
    && /^[0-9]+$/.test(alias.dev) && /^[0-9]+$/.test(alias.ino)
    && typeof alias.dev === 'string' && typeof alias.ino === 'string'
    && /^mnt:\[[0-9]+\]$/.test(alias.parent_mount_namespace), 'invalid private temporary alias');
}
export function checkTemporaryAliasMapping(alias, observed) {
  const unescape = value => value.replace(/\\(040|011|012|134)/g, (_, octal) => String.fromCharCode(parseInt(octal, 8)));
  const mounts = observed.mountinfo.trim().split('\n').map(line => {
    const fields = line.split(' '), separator = fields.indexOf('-');
    requireThat(separator >= 6, 'invalid mountinfo');
    return { line, id: fields[0], path: unescape(fields[4]), options: fields.slice(6, separator) };
  });
  // A bind over an existing /tmp mount leaves both records in mountinfo. The
  // opened directory's fdinfo identifies the effective mount, without guessing order.
  const tmp = mounts.filter(m => m.id === observed.alias.mount_id && m.path === '/tmp');
  const root = mounts.filter(m => m.path === '/');
  requireThat(observed.platform === 'linux' && observed.mount_namespace !== alias.parent_mount_namespace
    && /^mnt:\[[0-9]+\]$/.test(observed.mount_namespace), 'temporary alias requires an isolated mount namespace');
  requireThat(observed.backing.realpath === alias.backing && observed.alias.realpath === '/tmp'
    && [observed.alias, observed.backing].every(s => s.directory && s.dev === alias.dev && s.ino === alias.ino),
  'temporary alias backing device/inode mismatch');
  requireThat(tmp.length === 1 && root.length === 1
    && observed.alias.mount_id !== observed.backing.mount_id
    && [...tmp, ...root].every(m => !m.options.some(o => /^(shared|master|propagate_from):/.test(o)))
    && !mounts.some(m => m.path.startsWith('/tmp/')), 'temporary alias must be a private mapped /tmp without submounts');
  const backingMount = mounts.find(m => m.id === observed.backing.mount_id);
  requireThat(backingMount && (alias.backing === backingMount.path || beneath(backingMount.path, alias.backing)),
    'temporary backing mount missing');
  return { ...observed, mountinfo: undefined, mountinfo_sha256: sha256(observed.mountinfo),
    alias_mountinfo: tmp[0].line, backing_mountinfo: backingMount?.line,
    parent_mount_namespace: alias.parent_mount_namespace, backing_path: alias.backing, alias_path: '/tmp' };
}
export async function inspectTemporaryAlias(alias) {
  const identity = async path => {
    const handle = await open(path, 'r');
    try {
      const s = await handle.stat({ bigint: true });
      const fdinfo = await readFile(`/proc/self/fdinfo/${handle.fd}`, 'utf8');
      return { realpath: await realpath(path), directory: s.isDirectory(), dev: String(s.dev), ino: String(s.ino),
        mount_id: fdinfo.match(/^mnt_id:\s*(\d+)$/m)?.[1] };
    } finally { await handle.close(); }
  };
  return checkTemporaryAliasMapping(alias, { platform: process.platform,
    mount_namespace: await readlink('/proc/self/ns/mnt'), mountinfo: await readFile('/proc/self/mountinfo', 'utf8'),
    backing: await identity(alias.backing), alias: await identity('/tmp') });
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
    && /^viewer-acceptance-browser-masks-[a-z0-9][a-z0-9-]{0,100}$/.test(out['output-name']), 'run requires committed protocol, grant and fresh output name');
  return out;
}
export async function boundedFile(path, maximum) {
  const handle = await open(path, 'r');
  try {
    const info = await handle.stat();
    requireThat(info.isFile() && info.size <= maximum, 'file exceeds bound or is not regular');
    const bytes = Buffer.alloc(info.size + 1);
    let count = 0;
    while (count < bytes.length) {
      const got = await handle.read(bytes, count, bytes.length - count, null);
      if (!got.bytesRead) break;
      count += got.bytesRead;
    }
    requireThat(count === info.size, 'file size changed during bounded read');
    return bytes.subarray(0, count);
  } finally { await handle.close(); }
}
export async function pinnedFile(identity, maximum = LIMITS.jsonBytes, protectedInput = false) {
  requireThat(identity && isAbsolute(identity.path) && /^[0-9a-f]{64}$/.test(identity.sha256), 'invalid file identity');
  uint(identity.bytes, maximum, 'invalid pinned byte length');
  if (protectedInput) await protectedPath(identity.path);
  const bytes = await boundedFile(identity.path, maximum);
  requireThat(bytes.length === identity.bytes && sha256(bytes) === identity.sha256, 'pinned file identity mismatch');
  return bytes;
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
export function regionGeometry(profile, region) {
  for (const k of ['x', 'y', 'width', 'height']) uint(region[k], 0xffffffff, 'invalid region geometry');
  requireThat(region.width > 0 && region.height > 0 && region.x + region.width <= profile.width
    && region.y + region.height <= profile.height, 'region outside source');
  uint(region.discard, profile.decomposition_levels, 'invalid reduction');
  requireThat(Array.isArray(region.components) && [1, 3].includes(region.components.length)
    && region.components.every((c, i) => Number.isInteger(c) && c >= 0 && c < profile.components
      && (i === 0 || c > region.components[i - 1])), 'invalid component selection');
  const scale = 2 ** region.discard;
  const width = Math.ceil((region.x + region.width) / scale) - Math.ceil(region.x / scale);
  const height = Math.ceil((region.y + region.height) / scale) - Math.ceil(region.y / scale);
  // Engine admission includes the possible unaligned leading cell. Actual output
  // geometry remains ceil(start/scale)..ceil(end/scale), as in the native receipt.
  const reserveWidth = Math.ceil((region.x + region.width) / scale) - Math.floor(region.x / scale);
  const reserveHeight = Math.ceil((region.y + region.height) / scale) - Math.floor(region.y / scale);
  const pairBytes = reserveWidth * reserveHeight * (region.components.length * 4 + 2);
  requireThat(width > 0 && height > 0 && pairBytes <= LIMITS.decodedPairBytes, 'output reservation exceeds bound');
  return { width, height, pairBytes };
}
export function requestFor(manifest, region, sequence) {
  const geometry = regionGeometry(manifest.identity.profile, region);
  return { key: { representation: Array.from(Buffer.from(manifest.tid, 'hex')),
    region: { x: region.x, y: region.y, width: region.width, height: region.height },
    reduction: region.discard, components: region.components, stage: 1 },
  token: { source_generation: 0, demand_epoch: 1, sequence }, max_decoded_bytes: geometry.pairBytes };
}
export function tilesFor(profile, region) {
  const result = [], columns = Math.ceil(profile.width / profile.tile_edge);
  for (let y = Math.floor(region.y / profile.tile_edge); y <= Math.floor((region.y + region.height - 1) / profile.tile_edge); y++)
    for (let x = Math.floor(region.x / profile.tile_edge); x <= Math.floor((region.x + region.width - 1) / profile.tile_edge); x++) result.push(y * columns + x);
  return result;
}
export function maskShape(profile, discard, tile) {
  uint(discard, profile.decomposition_levels, 'mask level');
  const columns = Math.ceil(profile.width / profile.tile_edge), rows = Math.ceil(profile.height / profile.tile_edge);
  uint(tile, columns * rows - 1, 'mask tile');
  const x = tile % columns * profile.tile_edge, y = Math.floor(tile / columns) * profile.tile_edge;
  const width = Math.ceil(Math.min(profile.tile_edge, profile.width - x) / 2 ** discard);
  const height = Math.ceil(Math.min(profile.tile_edge, profile.height - y) / 2 ** discard);
  const pixels = width * height, planeBytes = Math.ceil(pixels / 8);
  return { width, height, pixels, planeBytes, bytes: planeBytes * profile.components * (discard ? 2 : 1) };
}
// Compare the actual HTTP body with retained canonical bytes. Oracle hashes bind
// the retained independent GDAL reductions; raw planes never enter a report.
export function inspectMask(actual, expected, profile, discard, tile, bands, oracle) {
  const shape = maskShape(profile, discard, tile);
  requireThat(expected.length === shape.bytes, 'retained mask length mismatch');
  if (actual.length !== shape.bytes) return { tile, discard, bytes: actual.length, expected_bytes: shape.bytes,
    byte_differences: null, noncanonical_padding: null, all_without_any: null, partial_band_cells: 0,
    actual_sha256: sha256(actual), expected_sha256: sha256(expected), planes: [], agrees: false, length_agrees: false };
  let byteDifferences = 0, noncanonicalPadding = 0, allWithoutAny = 0, partial = 0;
  for (let i = 0; i < actual.length; i++) byteDifferences += Number(actual[i] !== expected[i]);
  const planes = [];
  for (let c = 0; c < profile.components; c++) {
    const base = c * shape.planeBytes * (discard ? 2 : 1);
    const unpacked = [];
    for (let p = 0; p < (discard ? 2 : 1); p++) {
      const offset = base + p * shape.planeBytes;
      if (shape.pixels % 8) noncanonicalPadding += Number((actual[offset + shape.planeBytes - 1] >> (shape.pixels % 8)) !== 0);
      const bits = Buffer.alloc(shape.pixels);
      for (let i = 0; i < bits.length; i++) bits[i] = (actual[offset + (i >> 3)] >> (i & 7)) & 1;
      unpacked.push(bits);
    }
    if (!discard) unpacked.push(unpacked[0]);
    for (let i = 0; i < shape.pixels; i++) {
      allWithoutAny += Number(unpacked[0][i] > unpacked[1][i]);
      partial += Number(unpacked[1][i] > unpacked[0][i]);
    }
    for (const [p, name] of ['all', 'any'].entries()) {
      const record = oracle.find(r => r.tile === tile && r.discard === discard && r.band === bands[c] && r.plane === name);
      requireThat(record && record.samples === shape.pixels, 'missing exact-mask oracle');
      const digest = sha256(unpacked[p]);
      planes.push({ band: bands[c], plane: name, actual_sha256: digest, expected_sha256: record.expected_sha256,
        agrees: digest === record.expected_sha256 });
    }
  }
  return { tile, discard, bytes: actual.length, byte_differences: byteDifferences,
    noncanonical_padding: noncanonicalPadding, all_without_any: allWithoutAny, partial_band_cells: partial,
    actual_sha256: sha256(actual), expected_sha256: sha256(expected), planes,
    agrees: !byteDifferences && !noncanonicalPadding && !allWithoutAny && planes.every(p => p.agrees) };
}

// Self-contained: the same function is serialised into the browser page and
// exercised with authored arrays in Node tests. No emitted checksum is consulted.
export function comparePixels(pixels, samples, validity, expected) {
  const result = { geometry_agrees: pixels.width === expected.width && pixels.height === expected.height
      && pixels.precision === expected.precision && pixels.layout === expected.layout,
    sample_length_agrees: pixels.samples.length * 2 === samples.length,
    validity_length_agrees: pixels.validity.length === validity.length,
    sample_differences: 0, false_valid: 0, false_invalid: 0, nonbinary: 0, nonbinary_expected: 0,
    valid_cells: 0, invalid_cells: 0, samples_checked: 0, validity_checked: 0 };
  for (let i = 0; i < Math.min(pixels.samples.length, Math.floor(samples.length / 2)); i++) {
    result.sample_differences += Number(pixels.samples[i] !== (samples[2 * i] | (samples[2 * i + 1] << 8)));
    result.samples_checked++;
  }
  for (let i = 0; i < Math.min(pixels.validity.length, validity.length); i++) {
    const actual = pixels.validity[i], wanted = validity[i];
    result.nonbinary += Number(actual !== 0 && actual !== 1);
    result.nonbinary_expected += Number(wanted !== 0 && wanted !== 1);
    result.false_valid += Number(actual !== 0 && wanted === 0);
    result.false_invalid += Number(actual === 0 && wanted === 1);
    result.valid_cells += Number(wanted === 1); result.invalid_cells += Number(wanted === 0);
    result.validity_checked++;
  }
  result.samples_agree = result.geometry_agrees && result.sample_length_agrees && !result.sample_differences;
  result.validity_agrees = result.geometry_agrees && result.validity_length_agrees
    && !result.false_valid && !result.false_invalid && !result.nonbinary && !result.nonbinary_expected;
  return result;
}
export function makePlan(scenes) {
  requireThat(scenes.length === 5 && scenes.every((s, i) => s.manifest.target === ASSETS[i]), 'five ordered representations required');
  const contexts = scenes.map((s, scene) => ({ id: `scene-${scene + 1}`, kind: 'agreement', scene,
    jobs: s.requests.map((_, index) => ({ scene, index })), maskFiles: s.masks.tiles * s.masks.levels }));
  const pressure = [], seen = new Set();
  // Use all retained native-resolution, all-component windows, once each.
  // Selection is independent of observed cache behaviour and never expands on failure.
  for (const [scene, s] of scenes.entries()) for (const [index, r] of s.requests.entries()) {
    if (r.discard || r.components.length !== s.manifest.identity.profile.components) continue;
    const key = `${scene}:${JSON.stringify(r)}`;
    if (!seen.has(key)) { seen.add(key); pressure.push({ scene, index }); }
  }
  requireThat(pressure.length > 0 && pressure.length <= LIMITS.pressureForward, 'pressure case budget exceeded');
  for (const kind of ['missing', 'corrupt', 'stale']) contexts.push({ id: `required-mask-${kind}`, kind,
    scene: 3, jobs: [{ scene: 3, index: 0 }], maskFiles: 0 });
  contexts.push({ id: 'mask-pressure-revisit', kind: 'pressure', jobs: [...pressure, ...pressure],
    forwardJobs: pressure.length, maskFiles: 0 });
  return { schema: 'viewer-acceptance-browser-masks-plan/1', disposition: DISPOSITION, limits: LIMITS,
    contexts, browser_launches: 9, workers: 9, warmups: 0, runner_retries: 0,
    planned_jobs: contexts.reduce((n, c) => n + c.jobs.length, 0),
    planned_catalogue_masks: contexts.reduce((n, c) => n + c.maskFiles, 0),
    pressure_forward_jobs: pressure.length, pressure_revisit_jobs: pressure.length,
    scope: 'HTTP exact-mask delivery and actual WASM worker; GPU, normal latency and seven-event recovery unmeasured' };
}
export function coverageFor(scene, rows) {
  const successful = rows.filter(r => r.kind === 'Completed' && r.request_agrees
    && r.comparison?.samples_agree && r.comparison?.validity_agrees);
  const profile = scene.manifest.identity.profile;
  const requests = successful.map(row => scene.requests[row.index]);
  const levels = [...new Set(requests.map(r => r.discard))].sort((a, b) => a - b);
  const boundaries = Object.entries(scene.masks.boundary_points ?? {}).map(([component, point]) => ({
    component: Number(component), checked_native_cases: successful.filter(row => {
      const r = scene.requests[row.index];
      return r.discard === 0 && r.components.length === 1 && r.components[0] === Number(component)
        && r.x <= point[0] && r.y <= point[1] && r.x + r.width > point[0] && r.y + r.height > point[1]
        && row.comparison.valid_cells > 0 && row.comparison.invalid_cells > 0;
    }).length,
  }));
  const native = requests.filter(r => r.discard === 0).length;
  const crossTile = requests.filter(r => tilesFor(profile, r).length > 1).length;
  const clipped = requests.filter(r => r.x + r.width === profile.width || r.y + r.height === profile.height).length;
  const selections = Array.from({ length: profile.components }, (_, c) => [c]);
  if (profile.components === 3) selections.push([0, 1, 2]);
  const missingSelections = [];
  for (let level = 0; level < 7; level++) for (const components of selections)
    if (!requests.some(r => r.discard === level && same(r.components, components))) missingSelections.push({ level, components });
  return { levels_checked: levels, native_cases: native, cross_tile_cases: crossTile,
    clipped_source_edge_cases: clipped, source_boundaries: boundaries, missing_selections: missingSelections,
    source_boundary_applicability: boundaries.length ? 'observed-native-transitions' : 'no-transition-in-retained-source-oracle',
    proved: levels.length === 7 && native > 0 && crossTile > 0 && clipped > 0
      && !missingSelections.length && boundaries.every(b => b.checked_native_cases > 0) };
}
export function checkMetrics(metrics, compressed) {
  for (const key of ['compressed_bytes', 'peak_compressed_bytes', 'descriptor_bytes', 'peak_descriptor_bytes',
    'mask_bytes', 'peak_mask_bytes', 'mask_evictions', 'received_mask_bytes', 'decode_count', 'retries', 'peak_codec_workspace_bytes'])
    uint(metrics?.[key], Number.MAX_SAFE_INTEGER, `missing or invalid worker metric ${key}`);
  return metrics.compressed_bytes <= compressed && metrics.peak_compressed_bytes <= compressed
    && metrics.mask_bytes <= compressed && metrics.peak_mask_bytes <= compressed
    && metrics.descriptor_bytes <= LIMITS.descriptorBytes && metrics.peak_descriptor_bytes <= LIMITS.descriptorBytes
    && metrics.peak_codec_workspace_bytes <= LIMITS.codecWorkspaceBytes;
}
function summariseMetrics(metrics) {
  const fields = ['compressed_bytes', 'peak_compressed_bytes', 'descriptor_bytes', 'peak_descriptor_bytes',
    'mask_bytes', 'peak_mask_bytes', 'mask_evictions', 'received_mask_bytes', 'decode_count', 'retries',
    'received_jpp_bytes', 'received_descriptor_bytes', 'peak_codec_workspace_bytes', 'wasm_linear_bytes'];
  return Object.fromEntries(fields.map(k => [k, metrics[k] ?? null]));
}

export async function loadInputs(capsulePath) {
  const capsuleBytes = await boundedFile(capsulePath, LIMITS.jsonBytes), capsule = JSON.parse(capsuleBytes);
  requireThat(capsule.schema === 'viewer-acceptance-browser-masks-input/1' && capsule.disposition === DISPOSITION
    && /^[0-9a-f]{40}$/.test(capsule.runtime_commit), 'invalid capsule');
  if (capsule.temporary_alias !== undefined) validateTemporaryAlias(capsule.temporary_alias);
  const service = new URL(capsule.service.url);
  requireThat(service.protocol === 'http:' && ['127.0.0.1', '[::1]'].includes(service.hostname)
    && service.pathname === '/' && !service.search && !service.hash && !service.username && !service.password,
  'service must be an explicit loopback HTTP origin');
  requireThat(/^[0-9a-f]{64}$/.test(capsule.service.catalogue_sha256), 'pinned service catalogue required');
  await pinnedJSON(capsule.service.identity);
  await pinnedJSON(capsule.build_record);
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
    requireThat(root === await realpath(join(dirname(result.result.path), 'representation')), 'prepared root differs from retained native owner');
    const native = await pinnedJSON(result.evidence.native, true), requests = await pinnedJSON(result.evidence.requests, true);
    const masks = await pinnedJSON(result.evidence.masks, true), agreement = await pinnedJSON(result.evidence.regional_agreement, true);
    const manifestBytes = await boundedFile(await protectedPath(join(root, 'manifest.json')), 1 << 20);
    requireThat(sha256(manifestBytes) === result.storage.manifest_sha256, 'manifest identity mismatch');
    const manifest = JSON.parse(manifestBytes), profile = manifest.identity.profile;
    uint(profile.width, 0xffffffff, 'profile width'); uint(profile.height, 0xffffffff, 'profile height');
    requireThat(profile.width > 0 && profile.height > 0 && profile.width === result.grid.width
      && profile.height === result.grid.height && profile.components === result.grid.components
      && profile.bits_per_sample === result.grid.precision, 'profile differs from native source grid');
    requireThat(manifest.target === result.asset && manifest.tid === native.tid && /^[0-9a-f]{64}$/.test(manifest.tid)
      && manifest.identity.validity.policy === 'source-validity-v1' && profile.tile_edge === 512
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
      && level.every(digest => /^[0-9a-f]{64}$/.test(digest))), 'invalid mask identity catalogue');
    scenes.push({ root, manifest, requests, native, masks, agreement, result });
  }
  const plan = makePlan(scenes);
  requireThat(plan.planned_catalogue_masks === LIMITS.maskFiles
    && scenes.reduce((n, s) => n + s.requests.length, 0) === LIMITS.regionalJobs, 'frozen cohort count changed');
  return { capsule, capsule_sha256: sha256(capsuleBytes), scenes, assets, plan };
}

export async function boundedResponse(response, maximum) {
  const chunks = []; let size = 0;
  requireThat(response.body, 'missing HTTP body');
  const reader = response.body.getReader();
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.length;
      requireThat(size <= maximum, 'HTTP response exceeds bound');
      chunks.push(value);
    }
  } finally { await reader.cancel(); }
  return Buffer.concat(chunks, size);
}
export function faultResponse(kind, body, status) {
  if (kind === 'missing') return { status: 404, body: Buffer.from('required mask unavailable') };
  if (kind === 'corrupt') {
    requireThat(status === 200 && body.length > 0, 'cannot corrupt unsuccessful or empty mask');
    const altered = Buffer.from(body); altered[0] ^= 1;
    return { status, body: altered };
  }
  return { status, body };
}
export function maskRoute(manifest, discard, tile) {
  return `/mask/${manifest.target}/${discard}/${tile}?tid=${manifest.tid}`;
}
// One monitoring owner; no route transcript, response bodies or unbounded events.
export async function createProofServer(inputs, context, emit, readReference = (identity, maximum) => pinnedFile(identity, maximum, true)) {
  const state = { active: null, fault: null, requests: 0, bytes: 0, inflight: 0,
    errors: 0, errorDetails: [], maskComparisons: 0, maskDisagreements: 0, partialBandCells: 0,
    maskReads: new Map(), jobRequests: 0, jobMaskReads: [], maximumInflight: 0 };
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
          requireThat(bytes.length === maskShape(scene.manifest.identity.profile, discard, tile).bytes
            && sha256(bytes) === scene.manifest.identity.validity.tile_sha256[discard][tile], 'original fault response identity mismatch');
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
        const path = join(scene.root, `masks/${discard}/${tile}.bin`);
        const shape = maskShape(scene.manifest.identity.profile, discard, tile);
        const expected = await readReference({ path, bytes: shape.bytes,
          sha256: scene.manifest.identity.validity.tile_sha256[discard][tile] }, LIMITS.responseBytes);
        const audit = inspectMask(bytes, expected, scene.manifest.identity.profile, discard, tile,
          scene.manifest.identity.validity.bands, scene.masks.records);
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
    const started = performance.now();
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
      const timing = value.metrics?.timing;
      const workerInterval = timing && Number.isFinite(timing.finished_ms) && Number.isFinite(timing.started_ms)
        ? timing.finished_ms - timing.started_ms : null;
      return { kind, request_agrees: requestAgrees, comparison, metrics: value.metrics,
        error: typeof value.error === 'string' ? value.error.slice(0, 1024) : null,
        page_job_and_comparison_ms: performance.now() - started, worker_execution_ms: workerInterval,
        cross_realm_intervals_unavailable: true };
    } finally {
      // Do not retain Completed, its typed buffers, or reference buffers between jobs.
      if (event?.Completed) event.Completed.pixels = null;
      event = null; sampleBytes = null; validityBytes = null;
    }
  }, { request, manifest: scene.manifest, expected: { width: n.width, height: n.height,
    precision: scene.manifest.identity.profile.bits_per_sample, layout: region.components.length === 1 ? 'Scalar' : 'Rgb' } });
}
export function failureProof(kind, result, fault) {
  const expectedError = kind === 'missing' ? /HTTP 404/ : kind === 'stale' ? /HTTP 400/ : /mask.*digest|digest.*mask/i;
  return result.kind === 'Failed' && result.request_agrees && result.metrics.decode_count === 0
    && fault?.kind === kind && expectedError.test(result.error ?? '')
    && (kind !== 'stale' || fault.stale_identity_rejected === true);
}
export function pressureProof(forward, revisits, maskEvictions) {
  const keys = new Set(forward.flatMap(r => r.mask_reads));
  const refetched = new Set(revisits.flatMap(r => r.mask_reads).filter(k => keys.has(k)));
  return { actual_mask_evictions: maskEvictions, refetched_previously_loaded_masks: refetched.size,
    completed_revisits: revisits.filter(r => r.kind === 'Completed' && r.comparison?.samples_agree
      && r.comparison?.validity_agrees && r.request_agrees).length,
    proved: maskEvictions > 0 && refetched.size > 0 && forward.length === revisits.length
      && [...forward, ...revisits].every(r => r.kind === 'Completed' && r.comparison?.samples_agree
        && r.comparison?.validity_agrees && r.request_agrees && r.bounds_agree) };
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
      serviceWorkers: 'block', args: ['--no-sandbox', '--disable-background-networking', '--disable-breakpad', '--disable-crash-reporter'],
      env: { ...process.env, TMPDIR: temporary.path } });
    report.browser_version = browser.browser()?.version() ?? null;
    // This local proxy is the sole network destination, including Worker fetches.
    await browser.route('**/*', route => route.request().url().startsWith(`${proxy.url}/`)
      ? route.continue() : route.abort('blockedbyclient'));
    const page = browser.pages()[0] ?? await browser.newPage();
    page.on('worker', () => report.workers++); page.on('pageerror', () => report.page_errors++);
    await page.goto(`${proxy.url}/proof`, { timeout: LIMITS.jobMs });
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
      const scene = inputs.scenes[job.scene], result = await compareJob(page, scene, job.index, sequence);
      let boundsAgree = false, metricError = null;
      try { boundsAgree = checkMetrics(result.metrics, compressed); }
      catch (error) { metricError = String(error.message).slice(0, 512); }
      const row = { ...job, ...result, bounds_agree: boundsAgree, metric_error: metricError,
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
    if (report.jobs.some(r => !r.bounds_agree || !r.request_agrees)) report.missing_proof.push('worker-budget-or-request-identity');
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
      if (!report.pressure.proved) report.missing_proof.push('actual-mask-eviction-and-revisit');
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
  requireThat(grant.schema === 'viewer-acceptance-browser-masks-grant/1' && grant.disposition === DISPOSITION
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
  const result = { schema: 'viewer-acceptance-browser-masks-result/1', status: 'partial', disposition: DISPOSITION,
    protocol_commit: args['protocol-commit'], runtime_commit: inputs.capsule.runtime_commit,
    capsule_sha256: inputs.capsule_sha256, grant_sha256: sha256(grantBytes),
    operation_owner: grant.operation_owner, planned_jobs: inputs.plan.planned_jobs,
    contexts: [], missing_proof: [], viewer_acceptance: false, selected_configuration: null,
    gpu_viewing: 'unmeasured', normal_latency: 'unmeasured', inherited_seven_event_recovery: 'unmeasured',
    cross_realm_intervals_unavailable: true, speed_claim: null };
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
        missing_proof: context.missing_proof, http: context.http, coverage: context.coverage ?? null, pressure: context.pressure ?? null });
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
    if (!result.missing_proof.length) result.status = 'complete-diagnostic';
  } catch (error) {
    result.missing_proof.push('execution-or-identity-failure'); result.error = String(error.message).slice(0, 1024);
  } finally {
    if (journal) await journal.close();
    await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  }
  console.log(JSON.stringify({ status: result.status, output }));
  if (result.status !== 'complete-diagnostic') process.exitCode = 2;
}
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try { await run(parseArgs(process.argv.slice(2))); }
  catch { console.error('Browser mask proof did not start or complete; input/grant/preflight failure. No automatic retry.'); process.exitCode = 2; }
}
