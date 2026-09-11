// Preparation is import-safe. Only a separately granted `run` imports Playwright.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, readFile, realpath, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { STORE, DISPOSITION, boundedFile, sha256, launchError } from './viewer-acceptance-browser-masks.mjs';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const FILES = ['tools/viewer-acceptance-browser-masks.mjs',
  'tools/viewer-acceptance-browser-launch.mjs', 'tools/tests/viewer-acceptance-browser-launch.test.mjs',
  'docs/viewer-acceptance-browser-launch.md'];
export const FAILED_NAME = 'viewer-acceptance-browser-masks-fullscene-20260911t072423z';
export const CHROME = { path: '/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',
  bytes: 290614600, sha256: '0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71' };
export const LD_PATH = '/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64:/nvme/development/polyorama/.tools/sysroot/usr/lib';
export const BUDGET = { invocations: 1, browser_launches: 1, workers: 0, jobs: 0, services: 0,
  retries: 0, launch_ms: 60000, outer_seconds: 90 };
const CORE_FILES = ['package.json', 'index.js', 'index.mjs', 'lib/bootstrap.js', 'lib/coreBundle.js', 'lib/utilsBundle.js'];
const check = (value, message) => { if (!value) throw new Error(message); };
const json = async path => JSON.parse(await boundedFile(path, 1 << 20));
const save = (path, value) => writeFile(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });

export function paths(name) {
  check(/^viewer-acceptance-browser-masks-fullscene-\d{8}t\d{6}z$/.test(name) && name !== FAILED_NAME,
    'fresh same-length fullscene group required');
  const output = join(STORE, name);
  return { output, execution: `${output}-execution`, profile: join(output, 'scene-1/profile'),
    downloads: join(output, 'scene-1/downloads'), tmp: join(output, 'tmp'),
    cache: join(output, 'cache'), config: join(output, 'config') };
}
export function launchOptions(p, env) {
  return { executablePath: CHROME.path, headless: true, timeout: BUDGET.launch_ms,
    acceptDownloads: false, downloadsPath: p.downloads, serviceWorkers: 'block',
    args: ['--no-sandbox', '--disable-background-networking', '--disable-breakpad', '--disable-crash-reporter'],
    ...(env === undefined ? {} : { env }) };
}
export function environment(p, inherited) {
  check(inherited.LD_LIBRARY_PATH === LD_PATH, 'recorded LD_LIBRARY_PATH required');
  // Debug output bypasses bounded failure retention. Reject it, never silently amend it.
  for (const key of ['DEBUG', 'PWDEBUG', 'NODE_OPTIONS', 'NODE_DEBUG', 'LD_DEBUG', 'LD_PRELOAD', 'LD_AUDIT'])
    check(!inherited[key], `unsupported instrumentation: ${key}`);
  return { ...inherited, TMPDIR: p.tmp, XDG_CACHE_HOME: p.cache, XDG_CONFIG_HOME: p.config };
}
export function environmentIdentity(env) {
  const relevant = /^(?:LD_|XDG_|LC_|DBUS_|PLAYWRIGHT_|PW|CHROME|CHROMIUM|LIBGL_|MESA_|VK_|EGL_|__GL|NVIDIA_|DRI_|FONTCONFIG_|GDK_|GTK_|QT_|NODE_|SSL_|HTTP_PROXY$|HTTPS_PROXY$|ALL_PROXY$|NO_PROXY$|http_proxy$|https_proxy$|all_proxy$|no_proxy$|HOME$|PATH$|TMPDIR$|TMP$|TEMP$|DISPLAY$|WAYLAND_DISPLAY$|XAUTHORITY$|LANG$|LANGUAGE$|TZ$|DEBUG$)/;
  // Hash values, including any credentials. Never persist the inherited environment.
  return Object.fromEntries(Object.keys(env).filter(k => relevant.test(k)).sort().map(k => [k, sha256(env[k])]));
}
export function resolvedLibraries(text) {
  const libraries = [];
  for (const line of text.trim().split('\n')) {
    if (/^\s*linux-vdso\.so/.test(line)) continue;
    const match = line.match(/^\s*(?:\S+\s+=>\s+)?(\/\S+)\s+\(0x[0-9a-f]+\)\s*$/);
    check(match, 'unresolved library, relocation error or unrecognised loader output');
    libraries.push(match[1]);
  }
  check(libraries.length > 0, 'no resolved libraries');
  return [...new Set(libraries)].sort();
}
async function identity(path) {
  const canonical = await realpath(path), info = await stat(canonical);
  check(info.isFile(), 'identity requires regular file');
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(canonical)) hash.update(chunk);
  return { path, realpath: canonical, bytes: info.size, sha256: hash.digest('hex') };
}
async function committedFiles(commit) {
  check(/^[0-9a-f]{40}$/.test(commit), 'full protocol commit required');
  const result = {};
  for (const file of FILES) {
    const bytes = execFileSync('git', ['-C', REPO, 'show', `${commit}:${file}`],
      { maxBuffer: 1 << 20, stdio: ['ignore', 'pipe', 'pipe'] });
    check(sha256(await readFile(join(REPO, file))) === sha256(bytes), 'uncommitted protocol bytes');
    result[file] = sha256(bytes);
  }
  return result;
}
export function validateGrant(grant, capsule, digest, files, commit) {
  check(grant.schema === 'viewer-acceptance-browser-launch-grant/1' && grant.disposition === DISPOSITION,
    'launch diagnostic grant required');
  assert.deepEqual(grant.budget, BUDGET);
  assert.deepEqual(grant.protocol_files, files);
  check(grant.protocol_commit === commit && grant.capsule_sha256 === digest
    && grant.output_name === capsule.output_name, 'grant identity mismatch');
  check(grant.native_owner_stopped === true && grant.no_native_hardware_overlap === true,
    'native owner must stop before grant');
  for (const key of ['operation_owner', 'attribution', 'native_stop_receipt'])
    check(typeof grant[key] === 'string' && grant[key].trim().length > 0, `missing ${key}`);
  check(Array.isArray(grant.lineage) && grant.lineage.length > 0 && grant.lineage.length <= 16
    && grant.lineage.every(s => typeof s === 'string' && s.length > 0), 'lineage required');
}
// Inject only the authored mock or Playwright's Chromium API. No pages, pixels,
// navigation, event listeners, Workers, jobs, service or retry path exist here.
export async function oneProbe(chromium, p, env) {
  let context;
  const result = { status: 'partial', disposition: DISPOSITION, launch_attempts: 1,
    usable_context: false, cleanup_complete: false, viewer_acceptance: false, gpu_quality_masks: 'unmeasured' };
  try {
    context = await chromium.launchPersistentContext(p.profile, launchOptions(p, env));
    result.usable_context = true;
  } catch (error) {
    const secrets = Object.entries(env).filter(([k]) => /token|secret|password|passwd|credential|api.?key|authorization|cookie/i.test(k)).map(([, v]) => v);
    result.launch_error = launchError(error, secrets);
  }
  if (context) {
    try { await context.close(); result.cleanup_complete = true; }
    catch { result.cleanup_incomplete = true; } // Do not retain non-launch errors.
  }
  if (result.usable_context && result.cleanup_complete) result.status = 'blank-context-diagnostic-only';
  return result;
}
async function main(args) {
  check(args.length === 3 && ['validate', 'run'].includes(args[0]), 'usage: validate|run CAPSULE PROTOCOL_COMMIT');
  const [mode, capsulePath, commit] = args;
  const capsuleBytes = await boundedFile(capsulePath, 1 << 20), capsule = JSON.parse(capsuleBytes);
  check(capsule.schema === 'viewer-acceptance-browser-launch-input/1', 'invalid capsule schema');
  const p = paths(capsule.output_name), env = environment(p, process.env);
  check(await realpath(STORE) === STORE && await realpath(p.execution) === p.execution
    && capsulePath === join(p.execution, 'capsule.json') && await realpath(capsulePath) === capsulePath,
    'capsule must remain in fresh approved execution group');
  const files = await committedFiles(commit);
  const chrome = await identity(CHROME.path);
  assert.deepEqual({ path: chrome.path, bytes: chrome.bytes, sha256: chrome.sha256 }, CHROME);
  assert.deepEqual(capsule.chromium, chrome);
  assert.deepEqual(capsule.environment_sha256, environmentIdentity(env));
  assert.deepEqual(capsule.launch_options, launchOptions(p, undefined));
  assert.deepEqual(capsule.paths, p);
  // Loader trace only: no Chrome initialisation, no source inspection, no output transcript.
  const loader = spawnSync('/usr/bin/ldd', ['-r', CHROME.path],
    { env, encoding: 'utf8', timeout: 15000, maxBuffer: 1 << 20, stdio: ['ignore', 'pipe', 'pipe'] });
  check(!loader.error && loader.status === 0 && !loader.signal && !loader.stderr, 'loader check failed');
  const libraries = [];
  for (const path of resolvedLibraries(loader.stdout)) libraries.push(await identity(path));
  assert.deepEqual(capsule.resolved_libraries, libraries);
  const api = [];
  for (const path of CORE_FILES) api.push(await identity(join(REPO, 'node_modules/playwright-core', path)));
  assert.deepEqual(capsule.playwright_core_files, api);
  check((await json(join(REPO, 'node_modules/playwright-core/package.json'))).version === '1.62.1', 'Playwright version mismatch');
  assert.deepEqual(capsule.node, await identity(process.execPath));
  const digest = sha256(capsuleBytes);
  if (mode === 'validate') {
    console.log(JSON.stringify({ status: 'validated-without-launch', capsule_sha256: digest, protocol_files: files, budget: BUDGET }));
    return;
  }
  const grantBytes = await boundedFile(join(p.execution, 'grant.json'), 1 << 20), grant = JSON.parse(grantBytes);
  validateGrant(grant, capsule, digest, files, commit);
  const attribution = { operation_owner: grant.operation_owner, attribution: grant.attribution, lineage: grant.lineage };
  assert.deepEqual(await json(join(p.execution, 'lineage.json')), attribution);
  await mkdir(p.output); // Exclusive consumption of this grant, including failed prelaunch setup.
  await save(join(p.output, 'lineage.json'), attribution);
  await save(join(p.output, 'identity.json'), { capsule, capsule_sha256: digest, grant, grant_sha256: sha256(grantBytes) });
  for (const path of [p.tmp, p.cache, p.config, dirname(p.profile), p.profile, p.downloads]) await mkdir(path);
  // Set before importing the API so its own temporary files have the same path shape.
  Object.assign(process.env, { TMPDIR: p.tmp, XDG_CACHE_HOME: p.cache, XDG_CONFIG_HOME: p.config });
  await save(join(p.output, 'attempt.json'), { protocol_commit: commit, started_utc: new Date().toISOString(), budget: BUDGET });
  const { chromium } = await import('playwright-core');
  const result = await oneProbe(chromium, p, env);
  await save(join(p.output, 'result.json'), { schema: 'viewer-acceptance-browser-launch-result/1',
    protocol_commit: commit, capsule_sha256: digest, grant_sha256: sha256(grantBytes), ...result });
  console.log(JSON.stringify({ status: result.status, output: p.output }));
  if (result.status === 'partial') process.exitCode = 2;
}
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try { await main(process.argv.slice(2)); }
  catch { console.error('Launch diagnostic incomplete: input, grant, identity or setup failure. Preserve group; zero retries.'); process.exitCode = 2; }
}
