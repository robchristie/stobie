// Authored strings and API mocks only: no filesystem writes, HTTP or Playwright.
import test from 'node:test';
import assert from 'node:assert/strict';
import { launchError, DISPOSITION } from '../viewer-acceptance-browser-masks.mjs';
import { BUDGET, CHROME, LD_PATH, FAILED_NAME, paths, launchOptions, environment,
  environmentIdentity, resolvedLibraries, validateGrant, oneProbe } from '../viewer-acceptance-browser-launch.mjs';

const p = paths('viewer-acceptance-browser-masks-fullscene-20260911t123456z');
const env = environment(p, { LD_LIBRARY_PATH: LD_PATH, HOME: '/authored/home' });
test('launch tail preserves final useful stderr after a very long launch command', () => {
  const message = 'browserType.launchPersistentContext: closed\n<launching> ' + '--flag=x '.repeat(3000)
    + '\n[pid=12][err] Failed to bind socket: path too long\n[pid=12] <process did exit: exitCode=21, signal=null>';
  const result = launchError(new Error(message));
  assert.equal(result.header, 'browserType.launchPersistentContext: closed');
  assert.match(result.tail, /Failed to bind socket: path too long/);
  assert.equal(result.original_characters, message.length);
  assert.equal(result.exit_code, 21); assert.equal(result.signal, null);
  assert.equal(result.truncated, true); assert.ok(result.tail_bytes <= 4096);
});
test('byte caps preserve valid Unicode and retain a bounded single-line header', () => {
  const result = launchError('😀'.repeat(1000) + '\n' + 'é😀'.repeat(3000) + 'final stderr');
  assert.ok(result.header_bytes <= 512); assert.ok(result.tail_bytes <= 4096);
  assert.equal(Buffer.from(result.header).toString(), result.header);
  assert.equal(Buffer.from(result.tail).toString(), result.tail);
  assert.ok(result.tail.endsWith('final stderr')); assert.equal(result.truncated, true);
  assert.ok(launchError('x'.repeat(10000) + 'final stderr').tail.endsWith('final stderr'));
  const short = launchError('header\nshort error');
  assert.equal(short.truncated, false); assert.equal(short.sanitised, false);
  assert.equal(short.tail, 'short error');
});
test('escape sequences, OSC, C1, bidi and controls cannot reach retained output', () => {
  const result = launchError('\x1b[31mheader\x1b[0m\n\x1b]0;hidden\x07stderr\x00\r\b\u202e\u009b32m final');
  assert.equal(result.header, 'header'); assert.equal(result.tail, 'stderr final');
  assert.equal(result.sanitised, true);
});
test('secrets are redacted before truncation and credential values do not survive', () => {
  const secret = 'authored-secret-' + 'x'.repeat(5000);
  const result = launchError(`header\n${secret}\nhttps://alice:private@host/ token=abc password="two words" Authorization: Bearer abcdef\nuseful stderr`, [secret]);
  const text = JSON.stringify(result);
  for (const value of ['authored-secret', 'alice', 'private', 'two words', 'abcdef', 'token=abc']) assert.ok(!text.includes(value));
  assert.match(result.tail, /useful stderr/); assert.match(result.tail, /REDACTED/);
  assert.ok(!launchError('header\n--password "private words"').tail.includes('private words'));
  assert.ok(!launchError('header\npri\x00vate', ['pri\x00vate']).tail.includes('private'));
  assert.equal(launchError('header\nsocks5://alice:private@host\tend').tail, 'socks5://[REDACTED]@host end');
});
test('exit and signal are observations, with null for absent or null values', () => {
  assert.equal(launchError('closed\nexitCode=null, signal=SIGSEGV').signal, 'SIGSEGV');
  assert.equal(launchError('closed\nexitCode=null, signal=SIGSEGV').exit_code, null);
  assert.equal(launchError('closed').exit_code, null);
  assert.equal(launchError('closed\nexitCode=1\nexitCode=9').exit_code, 9);
  const fields = launchError({ message: 'closed', exitCode: 23, signal: 'SIGTERM' });
  assert.equal(fields.exit_code, 23); assert.equal(fields.signal, 'SIGTERM');
});
test('path shape stays long, scene-1 and equal in length to failed group', () => {
  assert.equal(p.output.length, p.output.replace(/\d{8}t\d{6}z$/, '20260911t072423z').length);
  assert.ok(p.profile.endsWith('/scene-1/profile')); assert.ok(p.tmp.endsWith('/tmp'));
  assert.throws(() => paths(FAILED_NAME));
  for (const name of ['short', '../escape', FAILED_NAME + '-retry']) assert.throws(() => paths(name));
});
test('same full Chrome options and relevant environment; no silent TMPDIR repair', () => {
  const options = launchOptions(p, env);
  assert.equal(options.executablePath, CHROME.path); assert.equal(options.timeout, 60000);
  assert.equal(options.headless, true); assert.equal(options.serviceWorkers, 'block');
  assert.equal(options.acceptDownloads, false); assert.equal(options.env.TMPDIR, p.tmp);
  assert.deepEqual(options.args, ['--no-sandbox', '--disable-background-networking', '--disable-breakpad', '--disable-crash-reporter']);
  assert.throws(() => environment(p, {}));
  for (const key of ['DEBUG', 'PWDEBUG', 'LD_DEBUG', 'LD_PRELOAD', 'NODE_OPTIONS'])
    assert.throws(() => environment(p, { ...env, [key]: 'enabled' }));
  const ids = environmentIdentity({ ...env, HTTPS_PROXY: 'https://user:secret@proxy', DISPLAY: ':0' });
  assert.ok(!JSON.stringify(ids).includes('secret')); assert.ok(ids.DISPLAY);
  assert.notDeepEqual(environmentIdentity(env), environmentIdentity({ ...env, DISPLAY: ':0' }));
});
test('all loader-resolved paths including interpreter are pinned; missing or unknown output rejects', () => {
  assert.deepEqual(resolvedLibraries('linux-vdso.so.1 (0x123)\n liba.so => /lib/liba.so (0xabc)\n /lib64/ld-linux.so (0x123)'),
    ['/lib/liba.so', '/lib64/ld-linux.so']);
  for (const text of ['', 'libbad.so => not found', 'undefined symbol: x', 'arbitrary warning']) assert.throws(() => resolvedLibraries(text));
});
test('grant requires exact one-probe budget, commit, identities, attribution and native stop', () => {
  const files = { authored: 'a'.repeat(64) }, commit = 'b'.repeat(40), digest = 'c'.repeat(64), capsule = { output_name: 'authored' };
  const grant = { schema: 'viewer-acceptance-browser-launch-grant/1', disposition: DISPOSITION,
    protocol_commit: commit, protocol_files: files, capsule_sha256: digest, output_name: capsule.output_name,
    budget: BUDGET, operation_owner: 'authored', attribution: 'synthetic', lineage: ['failed-parent'],
    native_owner_stopped: true, no_native_hardware_overlap: true, native_stop_receipt: 'authored receipt' };
  assert.doesNotThrow(() => validateGrant(grant, capsule, digest, files, commit));
  for (const change of [{ budget: { ...BUDGET, browser_launches: 2 } }, { budget: { ...BUDGET, jobs: 1 } },
    { native_owner_stopped: false }, { no_native_hardware_overlap: false }, { lineage: [] },
    { protocol_commit: 'x' }, { capsule_sha256: 'x' }, { protocol_files: {} }, { native_stop_receipt: '' }])
    assert.throws(() => validateGrant({ ...grant, ...change }, capsule, digest, files, commit));
});
test('successful mock launches exactly once and only closes the blank context', async () => {
  const calls = [];
  const chromium = { async launchPersistentContext(profile, options) {
    calls.push('launch'); assert.equal(profile, p.profile); assert.deepEqual(options, launchOptions(p, env));
    return { async close() { calls.push('close'); } };
  } };
  const result = await oneProbe(chromium, p, env);
  assert.deepEqual(calls, ['launch', 'close']); assert.equal(result.usable_context, true);
  assert.equal(result.status, 'blank-context-diagnostic-only'); assert.equal(result.viewer_acceptance, false);
  assert.equal(result.launch_error, undefined);
});
test('failed mock retains only sanitised launch error, with no second launch', async () => {
  let calls = 0;
  const result = await oneProbe({ async launchPersistentContext() { calls++; throw new Error('closed\nuseful stderr\nexitCode=1, signal=null'); } }, p, env);
  assert.equal(calls, 1); assert.equal(result.status, 'partial');
  assert.match(result.launch_error.tail, /useful stderr/); assert.equal(result.launch_error.exit_code, 1);
});
test('cleanup failure stays partial without retaining non-launch errors', async () => {
  const result = await oneProbe({ async launchPersistentContext() {
    return { async close() { throw new Error('non-launch sensitive transcript'); } };
  } }, p, env);
  assert.equal(result.status, 'partial'); assert.equal(result.cleanup_incomplete, true);
  assert.equal(result.launch_error, undefined); assert.ok(!JSON.stringify(result).includes('transcript'));
});
