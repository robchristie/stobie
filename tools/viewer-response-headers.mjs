// Run against the generated viewer package: node tools/viewer-response-headers.mjs WEB_ROOT.
// This exercises the same Fetch adapter and real WASM cache used by worker.js.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const root = pathToFileURL(resolve(process.argv[2] ?? '.tools/runtime/viewer-web') + '/');
const { default: init, WorkerClient } = await import(new URL('pkg/emuella_viewer.js', root));
const { beginResponse } = await import(new URL('response.js', root));
await init({ module_or_path: await readFile(new URL('pkg/emuella_viewer_bg.wasm', root)) });
const identity = {
  source_sha256: 'test-source', bands: [0],
  profile: { width: 256, height: 256, tile_edge: 256, decomposition_levels: 2,
    bits_per_sample: 8, components: 1, bits_per_pixel: 2.5 },
  codec_revision: 'test', encoding_contract: 'test', spatial_policy_sha256: 'test',
  payload_sha256: 'test', descriptor_format: 'test',
};
const manifest = { target: 'response-test', identity, encoded_bytes: 1024,
  main_header_bytes: 128, descriptor_sha256: ['test'] };
manifest.tid = createHash('sha256').update(JSON.stringify([
  identity, manifest.encoded_bytes, manifest.main_header_bytes, manifest.descriptor_sha256,
])).digest('hex');
const client = new WorkerClient(1 << 20);
client.register(manifest);
const headers = () => new Headers([
  ['jPiP-tId', manifest.tid], ['JpIp-FsIz', '128,128'],
  ['jPIP-roFF', '3,4'], ['JPIP-rsiZ', '100,101'],
]);
// Explicit class/codestream, an inherited final fragment, an ignored class and EOR.
const body = new Uint8Array([
  0x61, 0, 0, 0, 2, 7, 8, 0x31, 2, 1, 9, 0x60, 10, 0, 0, 2, 99, 98, 0, 2, 2, 50, 51,
]);
let rejected = 0;
function rejectsWithoutAdmission(fields) {
  const before = client.metrics();
  // A malformed next response must also invalidate an earlier unfinished reader.
  beginResponse(client, manifest.tid, headers());
  assert.throws(() => beginResponse(client, manifest.tid, fields));
  assert.throws(() => client.receive(body), /no response/);
  const after = client.metrics();
  assert.equal(after.compressed_bytes, before.compressed_bytes);
  assert.equal(after.received_jpp_bytes, before.received_jpp_bytes);
  rejected++;
}
for (const [name, value] of [
  ['JPIP-tid', 'another-target'], ['JPIP-fsiz', '256,256'],
  ['JPIP-roff', '0,0'], ['JPIP-rsiz', '1,1'],
]) {
  const fields = headers();
  const first = fields.get(name);
  fields.append(name, value);
  assert.equal(fields.get(name), `${first}, ${value}`, 'exercise Fetch duplicate combination');
  rejectsWithoutAdmission(fields);
}
for (const name of ['JPIP-fsiz', 'JPIP-roff', 'JPIP-rsiz']) {
  for (const value of [
    '128,128,256,256', '128,128, 256,256', '128,128,round-down',
    '128', '128,', '128,x', '-1,1', '1.5,1', '4294967296,1', '1, 1',
  ]) {
    const fields = headers(); fields.set(name, value); rejectsWithoutAdmission(fields);
  }
}
for (const [name, value] of [
  ['JPIP-fsiz', '0,128'], ['JPIP-rsiz', '0,1'], ['JPIP-roff', '4294967295,4'],
  ['JPIP-rsiz', '128,128'], ['JPIP-tid', 'another-target'],
]) {
  const fields = headers(); fields.set(name, value); rejectsWithoutAdmission(fields);
}
for (const name of ['JPIP-tid', 'JPIP-fsiz', 'JPIP-roff', 'JPIP-rsiz']) {
  const fields = headers(); fields.delete(name); rejectsWithoutAdmission(fields);
}
beginResponse(client, manifest.tid, headers());
client.receive(body);
client.finish();
assert.equal(client.metrics().compressed_bytes, 3, 'valid effective window admits the payload');
assert.equal(client.metrics().received_jpp_bytes, body.length);
// Invalid responses must preserve already admitted bins as well as the empty-cache case.
const conflict = headers(); conflict.append('JPIP-tid', 'another-target');
rejectsWithoutAdmission(conflict);
client.free();
console.log(`Viewer response headers passed: ${rejected} rejected cases without cache admission, case-insensitive valid effective window admitted, previous reader invalidated`);
