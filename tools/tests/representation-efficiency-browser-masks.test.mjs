// Authored bytes only: no protected reads, browser launch or persistent outputs.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { sha256, STORE, ASSETS, makePlan as legacyPlan, pressureProof } from '../viewer-acceptance-browser-masks.mjs';
import { LIMITS, compactIdentity, inspectCompactMask, released, oversizedRequirement,
  makePlan, validateTemporaryAlias, parseArgs, createProofServer, hardwareAdapter, gpuIdentity, checkCompactMetrics } from '../representation-efficiency-browser-masks.mjs';

const profile = { width: 3, height: 1, tile_edge: 512, decomposition_levels: 6, components: 1, bits_per_sample: 16 };
function fixture(state, discard = 0) {
  const legacy = Buffer.from(discard ? [1, 3] : [state === 0 ? 0 : state === 1 ? 7 : 5]);
  const actual = state === 2 ? Buffer.concat([Buffer.from([state]), legacy]) : Buffer.from([state]);
  const manifest = { target: ASSETS[0], tid: 'ab'.repeat(32), identity: { profile, validity: { bands: [1],
    policy: 'source-validity-v2', tile_sha256: Array.from({ length: 7 }, () => [`${state}:${sha256(actual)}`]) } } };
  const pixels = discard ? 2 : 3;
  const oracle = ['all', 'any'].map((plane, i) => ({ discard, tile: 0, band: 1, plane, samples: pixels,
    expected_sha256: sha256(Buffer.from(Array.from({ length: pixels }, (_, bit) => ((legacy[discard ? i : 0] >> bit) & 1)))) }));
  return { legacy, actual, manifest, oracle };
}
test('constant states preserve exact clipped padding and independent oracle bits', () => {
  for (const state of [0, 1]) {
    const f = fixture(state);
    const row = inspectCompactMask(f.actual, f.legacy, f.manifest, 0, 0, f.oracle);
    assert.equal(row.agrees, true); assert.equal(row.compact_bytes, 1);
    assert.equal(row.noncanonical_padding, 0);
  }
});
test('mixed states preserve distinct ALL and ANY planes at reduced levels', () => {
  const f = fixture(2, 1);
  const row = inspectCompactMask(f.actual, f.legacy, f.manifest, 1, 0, f.oracle);
  assert.equal(row.agrees, true); assert.equal(row.partial_band_cells, 1);
  assert.equal(row.compact_bytes, 3); assert.equal(row.legacy_bytes, 2);
});
test('digest, tag, state length and false constant declarations fail closed', () => {
  const f = fixture(1);
  assert.throws(() => inspectCompactMask(Buffer.from([0]), f.legacy, f.manifest, 0, 0, f.oracle), /identity/);
  assert.throws(() => inspectCompactMask(Buffer.from([1, 0]), f.legacy, f.manifest, 0, 0, f.oracle), /identity/);
  const invalid = structuredClone(f.manifest); invalid.identity.validity.tile_sha256[0][0] = `3:${sha256(f.actual)}`;
  assert.throws(() => compactIdentity(invalid, 0, 0), /identity/);
  assert.equal(inspectCompactMask(f.actual, Buffer.from([5]), f.manifest, 0, 0, f.oracle).agrees, false);
  assert.equal(inspectCompactMask(f.actual, Buffer.from([255]), f.manifest, 0, 0, f.oracle).agrees, false);
});
function scenes() {
  return ASSETS.map((target, scene) => ({ manifest: { target, identity: { profile } }, masks: { tiles: 1, levels: 7 },
    requests: Array.from({ length: scene === 0 ? 11 : 9 }, (_, x) => ({ x, y: 0, width: 1, height: 1, discard: 0, components: [0] })) }));
}
test('47×2 pressure order and budgets remain inherited; cancellation is separate', () => {
  const inputs = scenes(), legacy = legacyPlan(inputs), plan = makePlan(inputs);
  assert.deepEqual(plan.contexts.slice(0, 9), legacy.contexts);
  assert.equal(plan.pressure_forward_jobs, 47); assert.equal(plan.pressure_revisit_jobs, 47);
  assert.equal(plan.limits.pressureCompressedBytes, 1048576);
  assert.equal(plan.contexts[9].kind, 'cancellation');
  assert.equal(plan.planned_jobs, legacy.planned_jobs + 2);
  inputs[0].requests.pop(); assert.throws(() => makePlan(inputs), /47-window/);
});
test('oversized pressure records separate image bins from masks and never count as completion', () => {
  const row = oversizedRequirement('working-set admission: required compressed bins and masks need 1321074 bytes, limit 1048576 (image bins 1124466, masks 196608)');
  assert.deepEqual(row, { required_total_bytes: 1321074, limit_bytes: 1048576, image_bin_bytes: 1124466,
    mask_bytes: 196608, excluded: true, completed: false });
  assert.equal(oversizedRequirement('mask digest mismatch'), null);
  assert.throws(() => oversizedRequirement('required compressed bins and masks need 1321074 bytes, limit 1048576 (image bins 1124465, masks 196608)'), /accounting/);
  const failed = { kind: 'Failed', mask_reads: ['key'], oversized: row };
  assert.equal(pressureProof([failed], [failed], 1).proved, false);
});
test('reservation release requires observed zeros, not absent receipt fields', () => {
  assert.equal(released({}), false);
  assert.equal(released({ request_working_set_bytes: 0, request_pin_metadata_bytes: 0 }), true);
  assert.equal(released({ request_working_set_bytes: 0, request_pin_metadata_bytes: 64 }), false);
});
test('fresh output and private alias names stay beneath the approved campaign group', () => {
  const alias = { kind: 'private-linux-tmp/1', path: '/tmp',
    backing: `${STORE}/representation-efficiency-browser-masks-authored-execution/tmp`,
    dev: '42', ino: '123', parent_mount_namespace: 'mnt:[100]' };
  assert.doesNotThrow(() => validateTemporaryAlias(alias));
  assert.throws(() => validateTemporaryAlias({ ...alias, backing: '/tmp' }), /invalid/);
  assert.equal(parseArgs(['run', '--capsule', '/input', '--grant', '/grant', '--protocol-commit', 'a'.repeat(40),
    '--output-name', 'representation-efficiency-browser-masks-authored']).mode, 'run');
});
test('actual HTTP compact delivery compares retained bitmap/oracle; corrupt and stale faults remain distinct', async () => {
  const f = fixture(2, 1), rows = [];
  const legacyManifest = structuredClone(f.manifest); legacyManifest.identity.validity.tile_sha256[1][0] = sha256(f.legacy);
  const upstream = createServer((request, response) => {
    if (request.url.includes('tid=' + '0'.repeat(64))) { response.writeHead(400); response.end('stale mask identity'); }
    else { response.writeHead(200); response.end(f.actual); }
  });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  try {
    for (const kind of ['agreement', 'corrupt', 'stale', 'missing', 'cancellation']) {
      const context = { id: kind, kind, jobs: [{}], maskFiles: 1 };
      const inputs = { scenes: [{ manifest: f.manifest, legacyManifest, legacyRoot: '/authored', masks: { records: f.oracle } }],
        assets: new Map(), capsule: { service: { url: `http://127.0.0.1:${upstream.address().port}` } } };
      const proxy = await createProofServer(inputs, context, async row => rows.push(row), async () => f.legacy);
      try {
        proxy.state.active = { scene: 0, index: 0, cancel: kind === 'cancellation' };
        let cancelled = false; proxy.state.onCancel = async () => { cancelled = true; };
        const result = await fetch(`${proxy.url}/mask/${f.manifest.target}/1/0?tid=${f.manifest.tid}`);
        const bytes = Buffer.from(await result.arrayBuffer());
        if (kind === 'agreement') { assert.equal(result.status, 200); assert.deepEqual(bytes, f.actual); assert.equal(rows.at(-1).agrees, true); }
        if (kind === 'corrupt') { assert.equal(result.status, 200); assert.notDeepEqual(bytes, f.actual); assert.equal(proxy.state.fault.kind, kind); }
        if (kind === 'stale') { assert.equal(result.status, 400); assert.equal(proxy.state.fault.stale_identity_rejected, true); }
        if (kind === 'missing') assert.equal(result.status, 404);
        if (kind === 'cancellation') { assert.equal(cancelled, true); assert.equal(proxy.state.cancellationRequested, true); }
        assert.equal(proxy.state.errors, 0);
      } finally { await proxy.close(); }
    }
  } finally { await new Promise(resolve => upstream.close(resolve)); }
});

test('hardware adapter evidence rejects missing and software identities', () => {
  assert.equal(hardwareAdapter({ available: true, vendor: 'nvidia', architecture: 'ampere', fallback: false }), true);
  assert.equal(hardwareAdapter({ available: true, vendor: 'intel', description: 'SwiftShader', fallback: false }), false);
  assert.equal(hardwareAdapter({ available: false }), false);
  assert.equal(hardwareAdapter({ available: true, vendor: 'nvidia', fallback: true }), false);
});

test('GPU handshake retains bounded identity without raw extension or command-line dumps', () => {
  const info = { commandLine: 'omit', gpu: { devices: [{ vendorId: 4318, deviceId: 8708, deviceString: 'RTX 3090', extra: 'omit' }],
    auxAttributes: { glRenderer: 'NVIDIA Vulkan', glExtensions: 'omit' }, featureStatus: { vulkan: 'enabled_on', webgpu: 'enabled' } } };
  const result = gpuIdentity(info);
  assert.equal(result.handshake, 'SystemInfo.getInfo/1'); assert.equal(result.devices[0].vendorId, 4318);
  assert.equal(JSON.stringify(result).includes('omit'), false);
  assert.throws(() => gpuIdentity({ gpu: { devices: Array(9).fill({}) } }), /bounded/);
});

test('cache slot and container metadata remain charged when no payload entries remain', () => {
  const metrics = Object.fromEntries(['compressed_bytes', 'peak_compressed_bytes', 'mask_bytes', 'peak_mask_bytes',
    'mask_evictions', 'received_mask_bytes', 'decode_count', 'retries', 'peak_codec_workspace_bytes',
    'request_working_set_bytes', 'request_pin_metadata_bytes', 'peak_request_working_set_bytes',
    'peak_request_pin_metadata_bytes'].map(k => [k, 0]));
  Object.assign(metrics, { descriptor_bytes: 80, peak_descriptor_bytes: 80, compact_catalogue_metadata_bytes: 4,
    mask_cache_metadata_bytes: 64, peak_mask_cache_metadata_bytes: 64, mask_cache_entries: 0,
    mask_cache_slots: 3, mask_cache_slot_bytes: 16, mask_cache_container_bytes: 16 });
  assert.equal(checkCompactMetrics(metrics, LIMITS.pressureCompressedBytes), true);
  assert.equal(checkCompactMetrics({ ...metrics, mask_cache_metadata_bytes: 0 }, LIMITS.pressureCompressedBytes), false);
  assert.equal(checkCompactMetrics({ ...metrics, descriptor_bytes: 64 }, LIMITS.pressureCompressedBytes), false);
  assert.equal(checkCompactMetrics({ ...metrics, mask_cache_entries: 4 }, LIMITS.pressureCompressedBytes), false);
  assert.throws(() => checkCompactMetrics({ ...metrics, mask_cache_container_bytes: undefined }, LIMITS.pressureCompressedBytes), /missing compact metric/);
  const wasm = { ...metrics, mask_cache_slot_bytes: 8, mask_cache_container_bytes: 8, mask_cache_metadata_bytes: 32 };
  assert.equal(checkCompactMetrics(wasm, LIMITS.pressureCompressedBytes), true);
});
