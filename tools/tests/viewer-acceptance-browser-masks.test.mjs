// Synthetic code/fixtures only. No Playwright import, protected reads or browser launch.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile, readFile, readlink, stat } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { join, resolve } from 'node:path';
import { STORE, ASSETS, DISPOSITION, LIMITS, beneath, parseArgs, sha256, boundedFile,
  pinnedFile, regionGeometry, requestFor, tilesFor, maskShape, inspectMask, comparePixels,
  makePlan, coverageFor, checkMetrics, boundedResponse, faultResponse, createProofServer, maskRoute,
  failureProof, pressureProof, validateGrant, validateTemporaryAlias, checkTemporaryAliasMapping,
  inspectTemporaryAlias, temporaryDirectory } from '../viewer-acceptance-browser-masks.mjs';

const AUTHORED = resolve(process.env.POLYORAMA_AUTHORED_DIR ?? process.env.POLYORAMA_EVIDENCE_DIR
  ?? join(import.meta.dirname, '../../.tools/runtime/authored-browser-tests'));
await mkdir(AUTHORED, { recursive: true });
const fixtureRoot = await mkdtemp(join(AUTHORED, 'authored-tests-'));
const profile = { width: 1025, height: 513, tile_edge: 512, decomposition_levels: 6, components: 3, bits_per_sample: 16 };
const region = { x: 511, y: 511, width: 4, height: 2, discard: 1, components: [0, 1, 2] };
const manifest = { target: ASSETS[0], tid: 'ab'.repeat(32), identity: { profile } };
const identity = (path, bytes) => ({ path, bytes: bytes.length, sha256: sha256(bytes) });
const alias = { kind: 'private-linux-tmp/1', path: '/tmp',
  backing: join(STORE, 'viewer-acceptance-browser-masks-authored-execution/tmp'),
  dev: '42', ino: '123', parent_mount_namespace: 'mnt:[100]' };
function aliasObservation() {
  return { platform: 'linux', mount_namespace: 'mnt:[101]',
    mountinfo: '1 0 8:1 / / rw - ext4 /dev/test rw\n2 1 8:1 /approved/tmp /tmp rw - ext4 /dev/test rw\n',
    backing: { directory: true, realpath: alias.backing, dev: '42', ino: '123', mount_id: '1' },
    alias: { directory: true, realpath: '/tmp', dev: '42', ino: '123', mount_id: '2' } };
}
test('temporary alias is opt-in and never changes default output temporary paths', async () => {
  assert.deepEqual(await temporaryDirectory(undefined, '/original/output'), { path: '/original/output/tmp', provenance: null });
  assert.doesNotThrow(() => validateTemporaryAlias(alias));
  for (const change of [null, { ...alias, path: '/short' }, { ...alias, backing: '/tmp' },
    { ...alias, backing: join(STORE, '../outside/tmp') }, { ...alias, backing: join(STORE, 'tmp') },
    { ...alias, dev: 42 }, { ...alias, parent_mount_namespace: '' }])
    assert.throws(() => validateTemporaryAlias(change), /invalid private temporary alias/);
});
test('temporary alias requires mapped private namespace and pinned device/inode equality', () => {
  const good = aliasObservation(), proof = checkTemporaryAliasMapping(alias, good);
  assert.equal(proof.alias.dev, proof.backing.dev); assert.equal(proof.alias.ino, proof.backing.ino);
  assert.equal(proof.mountinfo_sha256, sha256(good.mountinfo));
  assert.match(proof.alias_mountinfo, / \/tmp /); assert.match(proof.backing_mountinfo, / \/ /);
  const stacked = structuredClone(good);
  stacked.mountinfo += '3 1 0:55 / /tmp rw - tmpfs tmpfs rw\n';
  assert.equal(checkTemporaryAliasMapping(alias, stacked).alias_mountinfo, proof.alias_mountinfo);
  for (const mutate of [
    o => { o.platform = 'darwin'; }, o => { o.mount_namespace = alias.parent_mount_namespace; },
    o => { o.alias.dev = '43'; }, o => { o.alias.ino = '124'; }, o => { o.backing.ino = '124'; },
    o => { o.backing.realpath = '/outside'; }, o => { o.alias.realpath = '/other'; },
    o => { o.alias.directory = false; }, o => { o.mountinfo = o.mountinfo.split('\n')[0]; },
    o => { o.alias.mount_id = '1'; }, o => { o.alias.mount_id = '999'; },
    o => { o.mountinfo = o.mountinfo.replace('/tmp rw -', '/tmp rw shared:3 -'); },
    o => { o.mountinfo = o.mountinfo.replace('/ rw -', '/ rw master:3 -'); },
    o => { o.mountinfo += '3 2 8:2 / /tmp/escape rw - ext4 /dev/other rw\n'; },
  ]) { const bad = structuredClone(good); mutate(bad); assert.throws(() => checkTemporaryAliasMapping(alias, bad)); }
});
test('temporary alias rejects an ordinary host /tmp using actual filesystem identities', async () => {
  const backing = await mkdtemp(join(fixtureRoot, 'unmapped-')), s = await stat(backing, { bigint: true });
  await assert.rejects(inspectTemporaryAlias({ ...alias, backing, dev: String(s.dev), ino: String(s.ino),
    parent_mount_namespace: 'mnt:[0]' }), /mismatch/);
});
test('temporary alias private bind preflight retains authored files and leaves host /tmp unchanged',
  { skip: process.platform !== 'linux' || process.env.POLYORAMA_TEST_PRIVATE_MOUNT !== '1' }, async () => {
  const backing = await mkdtemp(join(fixtureRoot, 'mount-')), s = await stat(backing, { bigint: true });
  const hostTmp = await stat('/tmp', { bigint: true }), parent = await readlink('/proc/self/ns/mnt');
  const spec = { ...alias, backing, dev: String(s.dev), ino: String(s.ino), parent_mount_namespace: parent };
  const moduleUrl = new URL('../viewer-acceptance-browser-masks.mjs', import.meta.url).href;
  const probe = `import { inspectTemporaryAlias } from ${JSON.stringify(moduleUrl)};
    import { writeFile, readFile } from 'node:fs/promises';
    const spec = JSON.parse(process.argv[1]);
    const proof = await inspectTemporaryAlias(spec);
    await writeFile('/tmp/authored-marker', 'synthetic only', { flag: 'wx' });
    if (await readFile(spec.backing + '/authored-marker', 'utf8') !== 'synthetic only') throw Error('alias write differs');
    console.log(JSON.stringify(proof));`;
  // The low-level mapping probe uses only authored scratch; production additionally enforces STORE and lineage.
  const stdout = execFileSync('unshare', ['-Urm', '--propagation', 'private', 'sh', '-eu', '-c',
    'mount --bind "$1" /tmp; shift; exec "$@"', 'alias-probe', backing,
    process.execPath, '--input-type=module', '-e', probe, JSON.stringify(spec)], { encoding: 'utf8', timeout: 10000 });
  const proof = JSON.parse(stdout);
  assert.notEqual(proof.mount_namespace, parent); assert.equal(proof.alias.ino, String(s.ino));
  assert.equal(await readFile(join(backing, 'authored-marker'), 'utf8'), 'synthetic only');
  assert.equal(await readlink('/proc/self/ns/mnt'), parent);
  const after = await stat('/tmp', { bigint: true });
  assert.equal(after.dev, hostTmp.dev); assert.equal(after.ino, hostTmp.ino);
  await writeFile(join(backing, 'mount-preflight.json'), JSON.stringify(proof, null, 2) + '\n', { flag: 'wx' });
});
function metrics() {
  return Object.fromEntries(['compressed_bytes', 'peak_compressed_bytes', 'descriptor_bytes', 'peak_descriptor_bytes',
    'mask_bytes', 'peak_mask_bytes', 'mask_evictions', 'received_mask_bytes', 'decode_count', 'retries', 'peak_codec_workspace_bytes'].map(k => [k, 0]));
}
function scenes() {
  return ASSETS.map(target => ({ manifest: { ...manifest, target }, masks: { tiles: 6, levels: 7 },
    requests: [{ ...region, discard: 0 }, { ...region, discard: 0 }, region] }));
}
function oracleFor(bytes, p, d, tile, bands) {
  const s = maskShape(p, d, tile), records = [];
  for (let c = 0; c < p.components; c++) for (const [plane, name] of ['all', 'any'].entries()) {
    const offset = (c * (d ? 2 : 1) + (d ? plane : 0)) * s.planeBytes;
    const bits = Buffer.from(Array.from({ length: s.pixels }, (_, i) => bytes[offset + (i >> 3)] >> (i & 7) & 1));
    records.push({ tile, discard: d, band: bands[c], plane: name, samples: s.pixels, expected_sha256: sha256(bits) });
  }
  return records;
}
test('strict CLI rejects unknown, duplicated, absent and partial authorisation arguments', () => {
  assert.equal(parseArgs(['validate', '--capsule', '/input.json']).mode, 'validate');
  for (const args of [[], ['run', '--capsule', '/x'], ['validate', '--capsule'],
    ['validate', '--capsule', '/x', '--capsule', '/y'], ['validate', '--browser', 'yes']])
    assert.throws(() => parseArgs(args));
  assert.throws(() => parseArgs(['run', '--capsule', '/x', '--grant', '/g', '--protocol-commit', 'a'.repeat(40), '--output-name', '../escape']));
});
test('approved-store boundary rejects siblings, root itself and traversal', () => {
  assert.equal(beneath('/approved', '/approved/run/profile'), true);
  for (const p of ['/approved', '/approved-other/file', '/approved/../outside/file']) assert.equal(beneath('/approved', p), false);
});
test('bounded pinned reads reject truncation, drift and excess before consumption', async () => {
  const path = join(fixtureRoot, 'bounded.bin'), bytes = Buffer.from([0, 1, 2, 255]);
  await writeFile(path, bytes, { flag: 'wx' });
  assert.deepEqual(await pinnedFile(identity(path, bytes), 4), bytes);
  await assert.rejects(boundedFile(path, 3), /bound/);
  await assert.rejects(pinnedFile({ ...identity(path, bytes), bytes: 3 }, 4), /identity mismatch/);
  await assert.rejects(pinnedFile({ ...identity(path, bytes), sha256: '0'.repeat(64) }, 4), /identity mismatch/);
  await assert.rejects(pinnedFile(identity(path, bytes), 4, true), /approved store/);
});
test('global ceil grid, clipped edges, component order and conservative output-pair reservation', () => {
  assert.deepEqual(regionGeometry(profile, region), { width: 2, height: 1, pairBytes: 84 });
  assert.deepEqual(tilesFor(profile, region), [0, 1, 3, 4]);
  assert.deepEqual(maskShape(profile, 6, 5), { width: 1, height: 1, pixels: 1, planeBytes: 1, bytes: 6 });
  const request = requestFor(manifest, region, 7);
  assert.deepEqual(request.key.representation, Array(32).fill(171));
  assert.equal(request.max_decoded_bytes, 84); assert.equal(request.token.sequence, 7);
  for (const change of [{ components: [0, 0, 2] }, { components: [2, 1, 0] }, { discard: 7 },
    { width: 0 }, { x: 1025 }, { height: 999999 }]) assert.throws(() => regionGeometry(profile, { ...region, ...change }));
  assert.throws(() => regionGeometry({ ...profile, width: 4000, height: 4000 }, { ...region, x: 0, y: 0, width: 4000, height: 4000, discard: 0 }), /reservation/);
});
test('native mask unpacking includes clipped padding, original band order and byte equality', () => {
  const p = { ...profile, width: 9, height: 1, components: 1 }, bytes = Buffer.from([0xaa, 1]);
  const oracle = oracleFor(bytes, p, 0, 0, [5]);
  const good = inspectMask(bytes, bytes, p, 0, 0, [5], oracle);
  assert.equal(good.agrees, true); assert.equal(good.planes.length, 2);
  const bad = Buffer.from([0xaa, 129]), result = inspectMask(bad, bytes, p, 0, 0, [5], oracle);
  assert.equal(result.agrees, false); assert.equal(result.noncanonical_padding, 1); assert.equal(result.byte_differences, 1);
  const short = inspectMask(bytes.subarray(0, 1), bytes, p, 0, 0, [5], oracle);
  assert.equal(short.length_agrees, false); assert.equal(short.bytes, 1); assert.equal(short.expected_bytes, 2);
});
test('reduced all/any proof preserves partial cells, implication errors and oracle disagreement', () => {
  const p = { ...profile, width: 6, height: 2, components: 1 }, bytes = Buffer.from([0b001, 0b101]);
  const oracle = oracleFor(bytes, p, 1, 0, [3]);
  assert.equal(inspectMask(bytes, bytes, p, 1, 0, [3], oracle).partial_band_cells, 1);
  const broken = inspectMask(Buffer.from([0b111, 0b101]), bytes, p, 1, 0, [3], oracle);
  assert.equal(broken.all_without_any, 1); assert.equal(broken.agrees, false);
  const alteredOracle = structuredClone(oracle); alteredOracle[0].expected_sha256 = '0'.repeat(64);
  assert.equal(inspectMask(bytes, bytes, p, 1, 0, [3], alteredOracle).agrees, false);
  assert.throws(() => inspectMask(bytes, bytes, p, 1, 0, [3], []), /oracle/);
});
test('direct sample comparison checks valid zero, U16 high bytes and invalid-side samples', () => {
  const expected = { width: 3, height: 1, precision: 16, layout: 'Scalar' };
  const pixels = { ...expected, samples: new Uint16Array([0, 256, 65535]), validity: new Uint8Array([1, 0, 1]), fnv1a64_u16le: 'untrusted' };
  const samples = new Uint8Array([0, 0, 0, 1, 255, 255]), validity = new Uint8Array([1, 0, 1]);
  assert.equal(comparePixels(pixels, samples, validity, expected).samples_agree, true);
  pixels.samples[1] = 257;
  const mismatch = comparePixels(pixels, samples, validity, expected);
  assert.equal(mismatch.sample_differences, 1); assert.equal(mismatch.samples_agree, false); assert.equal(mismatch.validity_agrees, true);
});
test('direct validity reports both disagreement directions, nonbinary values and missing arrays', () => {
  const expected = { width: 3, height: 1, precision: 8, layout: 'Scalar' };
  const pixels = { ...expected, samples: new Uint16Array([0, 1, 2]), validity: new Uint8Array([1, 0, 2]) };
  const got = comparePixels(pixels, new Uint8Array([0, 0, 1, 0, 2, 0]), new Uint8Array([0, 1, 1]), expected);
  assert.equal(got.false_valid, 1); assert.equal(got.false_invalid, 1); assert.equal(got.nonbinary, 1);
  assert.equal(got.samples_agree, true); assert.equal(got.validity_agrees, false);
  pixels.validity = new Uint8Array(); pixels.samples = new Uint16Array();
  const missing = comparePixels(pixels, new Uint8Array(6), new Uint8Array(3), expected);
  assert.equal(missing.samples_agree, false); assert.equal(missing.validity_agrees, false);
});
test('plan fixes nine contexts, deduplicates forward windows and declares every revisit before execution', () => {
  const plan = makePlan(scenes());
  assert.equal(plan.contexts.length, 9); assert.equal(plan.pressure_forward_jobs, 5);
  assert.equal(plan.planned_jobs, 15 + 3 + 10); assert.equal(plan.runner_retries, 0);
  const pressure = plan.contexts.at(-1);
  assert.deepEqual(pressure.jobs.slice(0, 5), pressure.jobs.slice(5));
  assert.throws(() => makePlan(scenes().slice(1)), /five/);
  const excessive = scenes(); excessive[0].requests = Array.from({ length: 65 }, (_, x) => ({ ...region, discard: 0, x }));
  assert.throws(() => makePlan(excessive), /pressure case budget/);
});
test('unchanged pressure and global limits reject missing and excessive observed counters', () => {
  const m = metrics(); m.mask_bytes = 1 << 20; m.peak_compressed_bytes = 1 << 20;
  assert.equal(checkMetrics(m, LIMITS.pressureCompressedBytes), true);
  m.peak_compressed_bytes++; assert.equal(checkMetrics(m, LIMITS.pressureCompressedBytes), false);
  delete m.mask_evictions; assert.throws(() => checkMetrics(m, LIMITS.compressedBytes), /metric/);
});
test('pressure proof requires actual mask eviction AND observed prior-mask refetch AND exact arrays', () => {
  const row = { kind: 'Completed', request_agrees: true, bounds_agree: true,
    comparison: { samples_agree: true, validity_agrees: true }, mask_reads: ['tid/0/1'] };
  assert.equal(pressureProof([row], [row], 1).proved, true);
  assert.equal(pressureProof([row], [row], 0).proved, false);
  assert.equal(pressureProof([row], [{ ...row, mask_reads: ['tid/0/2'] }], 2).proved, false);
  assert.equal(pressureProof([row], [{ ...row, kind: 'Failed' }], 2).proved, false);
});
test('required-mask failure cannot be inferred from arbitrary failure, timeout or decoded publication', () => {
  const row = { kind: 'Failed', request_agrees: true, error: 'Error: HTTP 404', metrics: { decode_count: 0 } };
  assert.equal(failureProof('missing', row, { kind: 'missing' }), true);
  assert.equal(failureProof('missing', { ...row, kind: 'Completed' }, { kind: 'missing' }), false);
  assert.equal(failureProof('missing', { ...row, error: 'timeout' }, { kind: 'missing' }), false);
  assert.equal(failureProof('missing', { ...row, metrics: { decode_count: 1 } }, { kind: 'missing' }), false);
  assert.equal(failureProof('stale', { ...row, error: 'HTTP 400' }, { kind: 'stale', stale_identity_rejected: false }), false);
});
test('bounded HTTP reader rejects oversized streaming responses and cancellation is observed', async () => {
  let cancelled = false;
  const response = new Response(new ReadableStream({ start(controller) { controller.enqueue(new Uint8Array(5)); }, cancel() { cancelled = true; } }));
  await assert.rejects(boundedResponse(response, 4), /bound/); assert.equal(cancelled, true);
  assert.deepEqual(await boundedResponse(new Response(new Uint8Array([1, 2])), 2), Buffer.from([1, 2]));
});
test('fault corruption copies a bounded response and leaves original bytes untouched', () => {
  const original = Buffer.from([255, 0]); const altered = faultResponse('corrupt', original, 200);
  assert.deepEqual(original, Buffer.from([255, 0])); assert.deepEqual(altered.body, Buffer.from([254, 0]));
  assert.throws(() => faultResponse('corrupt', original, 400));
});
test('grant pins plan, capsule, committed files, single invocation and attributed lineage', () => {
  const plan = makePlan(scenes()), args = { 'protocol-commit': 'a'.repeat(40), 'output-name': 'viewer-acceptance-browser-masks-authored' };
  const inputs = { plan, capsule_sha256: 'b'.repeat(64) }, files = { 'authored.mjs': 'c'.repeat(64) };
  const grant = { schema: 'viewer-acceptance-browser-masks-grant/1', disposition: DISPOSITION,
    protocol_commit: args['protocol-commit'], protocol_files: files, capsule_sha256: inputs.capsule_sha256,
    plan_sha256: sha256(JSON.stringify(plan)), output_name: args['output-name'], max_invocations: 1,
    max_browser_launches: 9, runner_retries: 0, operation_owner: 'authored', attribution: 'synthetic', lineage: ['authored fixture'] };
  assert.doesNotThrow(() => validateGrant(grant, args, inputs, files));
  for (const change of [{ max_invocations: 2 }, { max_browser_launches: 10 }, { runner_retries: 1 },
    { lineage: [] }, { plan_sha256: '0'.repeat(64) }, { capsule_sha256: '0'.repeat(64) }, { protocol_files: {} }])
    assert.throws(() => validateGrant({ ...grant, ...change }, args, inputs, files), /grant|lineage/);
});
test('authored real HTTP proxy audits masks and injects missing/corrupt/stale without touching originals', async () => {
  const root = join(fixtureRoot, 'representation'); await mkdir(join(root, 'masks/0'), { recursive: true });
  const original = Buffer.from([0b101]), path = join(root, 'masks/0/0.bin'); await writeFile(path, original, { flag: 'wx' });
  const p = { ...profile, width: 3, height: 1, components: 1 };
  const m = { ...manifest, identity: { profile: p, validity: { bands: [1], tile_sha256: [[sha256(original)]] } } };
  const catalogue = Buffer.from(JSON.stringify([m])); let upstreamRequests = 0;
  const upstream = createServer((req, res) => {
    upstreamRequests++;
    if (req.url.includes('tid=' + '0'.repeat(64))) { res.writeHead(400); res.end('stale mask identity'); }
    else if (req.url === '/catalogue') res.end(catalogue);
    else res.end(original);
  });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const inputs = { capsule: { service: { url: `http://127.0.0.1:${upstream.address().port}`, catalogue_sha256: sha256(catalogue) } },
    assets: new Map(), scenes: [{ root, manifest: m, masks: { records: oracleFor(original, p, 0, 0, [1]) } }] };
  try {
    for (const kind of ['agreement', 'missing', 'corrupt', 'stale']) {
      const rows = [], proxy = await createProofServer(inputs, { id: kind, kind, jobs: [{}], maskFiles: 1 },
        async row => rows.push(row), (record, maximum) => pinnedFile(record, maximum));
      try {
        assert.equal((await fetch(proxy.url + '/catalogue')).status, 200);
        proxy.state.active = { scene: 0, index: 0 };
        const before = upstreamRequests, response = await fetch(proxy.url + maskRoute(m, 0, 0)), body = Buffer.from(await response.arrayBuffer());
        if (kind === 'agreement') { assert.equal(response.status, 200); assert.equal(rows[0].agrees, true); }
        if (kind === 'missing') { assert.equal(response.status, 404); assert.equal(upstreamRequests, before); }
        if (kind === 'corrupt') { assert.equal(response.status, 200); assert.notDeepEqual(body, original); }
        if (kind === 'stale') { assert.equal(response.status, 400); assert.equal(proxy.state.fault.stale_identity_rejected, true); }
        assert.deepEqual(await readFile(path), original);
      } finally { await proxy.close(); }
    }
  } finally { upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve)); }
});

test('coverage requires actual native transition populations, clipped/cross-tile regions and every level/selection', () => {
  const p = { ...profile, components: 1 };
  const requests = Array.from({ length: 7 }, (_, discard) => ({ x: 511, y: 511, width: 514, height: 2, discard, components: [0] }));
  const scene = { manifest: { identity: { profile: p } }, requests, masks: { boundary_points: { 0: [512, 512] } } };
  const rows = requests.map((_, index) => ({ index, kind: 'Completed', request_agrees: true,
    comparison: { samples_agree: true, validity_agrees: true, valid_cells: 1, invalid_cells: 1 } }));
  assert.equal(coverageFor(scene, rows).proved, true);
  assert.equal(coverageFor(scene, rows.slice(1)).proved, false);
  rows[0].comparison.invalid_cells = 0;
  assert.equal(coverageFor(scene, rows).proved, false);
  scene.masks.boundary_points = {};
  assert.equal(coverageFor(scene, rows).proved, true);
  assert.equal(coverageFor(scene, rows).source_boundary_applicability, 'no-transition-in-retained-source-oracle');
});
test('proxy rejects a job request overflow before contacting any service', async () => {
  const inputs = { capsule: { service: { url: 'http://127.0.0.1:1' } }, assets: new Map(), scenes: [] };
  const proxy = await createProofServer(inputs, { id: 'bound', kind: 'agreement', jobs: [{}], maskFiles: 0 }, async () => {});
  try {
    proxy.state.jobRequests = LIMITS.httpRequestsPerJob;
    const response = await fetch(proxy.url + '/proof'); await response.arrayBuffer();
    assert.equal(response.status, 502); assert.equal(proxy.state.errors, 1);
    assert.match(proxy.state.errorDetails[0], /job HTTP request budget/);
  } finally { await proxy.close(); }
});
