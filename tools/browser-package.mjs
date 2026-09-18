import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { brotliCompressSync, brotliDecompressSync, constants, gzipSync, gunzipSync } from 'node:zlib';

export const WASM_OPT_VERSION = '131';
export const VARIANTS = ['none', 'Oz', 'O3'];
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const defaultOptimiser = () => process.env.WASM_OPT || (existsSync(path.join(root, '.tools/browser/binaryen-version_131/bin/wasm-opt')) ? path.join(root, '.tools/browser/binaryen-version_131/bin/wasm-opt') : 'wasm-opt');
export const DEFAULT_APPS = [
  { name: 'lab', directory: path.join(root, 'apps/analytical-workspace-lab/web') },
  { name: 'gallery', directory: path.join(root, 'apps/polyorama-gallery/web') },
  { name: 'viewer', directory: path.join(root, 'apps/emuella-viewer/web') },
];
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const describe = bytes => ({ bytes: bytes.length, sha256: sha256(bytes) });
const serialise = value => `${JSON.stringify(value, null, 2)}\n`;

function filesIn(directory, prefix = '') {
  return readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en')).flatMap(entry => {
    if (entry.name.startsWith('.') || entry.name === 'tests') return [];
    const relative = path.posix.join(prefix, entry.name);
    const full = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Symlink is not a package input: ${full}`);
    if (entry.isDirectory()) return filesIn(full, relative);
    if (!entry.isFile()) throw new Error(`Not a regular file: ${full}`);
    if (/\.(?:br|gz)$/.test(entry.name)) throw new Error(`Precompressed input is not supported: ${full}`);
    return [relative];
  });
}

function optimiserVersion(executable) {
  const result = spawnSync(executable, ['--version'], { encoding: 'utf8' });
  if (result.error || result.status !== 0) throw new Error(`wasm-opt ${WASM_OPT_VERSION} is required: ${result.error?.message || result.stderr}`);
  const version = result.stdout.trim();
  if (!new RegExp(`^wasm-opt version ${WASM_OPT_VERSION}(?:\\s|$)`).test(version)) {
    throw new Error(`Incompatible wasm-opt: expected ${WASM_OPT_VERSION}, observed ${JSON.stringify(version)}`);
  }
  return version;
}

function writeRepresentations(directory, relative, bytes) {
  const destination = path.join(directory, relative);
  mkdirSync(path.dirname(destination), { recursive: true });
  const br = brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } });
  const gz = gzipSync(bytes, { level: 9 });
  if (!brotliDecompressSync(br).equals(bytes) || !gunzipSync(gz).equals(bytes)) throw new Error(`Compression round-trip failed: ${relative}`);
  writeFileSync(destination, bytes);
  writeFileSync(`${destination}.br`, br);
  writeFileSync(`${destination}.gz`, gz);
  return { path: relative, raw: describe(bytes), br: describe(br), gzip: describe(gz) };
}

/** Package already built wasm-bindgen web directories. No build, install or publication occurs. */
export function packageBrowser({ apps = DEFAULT_APPS, output = path.join(root, 'target/browser-production'), variant = 'none', wasmOpt = defaultOptimiser(), sourceIdentity = null } = {}) {
  if (!VARIANTS.includes(variant)) throw new Error(`Unknown variant ${variant}; expected ${VARIANTS.join(', ')}`);
  if (!apps.length) throw new Error('At least one app is required');
  output = path.resolve(output);
  const names = new Set();
  for (const app of apps) {
    if (!/^[a-z][a-z0-9-]*$/.test(app.name) || names.has(app.name)) throw new Error(`Invalid or duplicate app name: ${app.name}`);
    names.add(app.name);
    const input = path.resolve(app.directory);
    if (input === output || input.startsWith(`${output}${path.sep}`) || output.startsWith(`${input}${path.sep}`)) throw new Error('Output must be isolated from inputs');
    if (lstatSync(input).isSymbolicLink()) throw new Error(`Symlink is not a package input: ${input}`);
  }
  if (existsSync(output)) {
    if (lstatSync(output).isSymbolicLink()) throw new Error('Output must not be a symlink');
    if (readdirSync(output).length && !existsSync(path.join(output, '.browser-package'))) throw new Error(`Refusing to replace unrelated output: ${output}`);
  }
  const settings = { variant, wasmOptArgs: variant === 'none' ? [] : [`-${variant}`], compression: { brotliQuality: 11, gzipLevel: 9 } };
  const toolVersions = { node: process.versions.node, zlib: process.versions.zlib, brotli: process.versions.brotli, wasmOpt: variant === 'none' ? null : optimiserVersion(wasmOpt) };
  mkdirSync(path.dirname(output), { recursive: true });
  const staging = mkdtempSync(path.join(path.dirname(output), '.browser-package-'));
  const manifest = { schemaVersion: 1, settings, toolVersions, sourceIdentity, apps: [] };
  try {
    for (const app of [...apps].sort((a, b) => a.name.localeCompare(b.name, 'en'))) {
      const inputs = filesIn(app.directory);
      if (!inputs.includes('index.html') || !inputs.some(name => name.endsWith('.wasm'))) throw new Error(`${app.name} requires index.html and built .wasm input`);
      const inputRecords = [];
      const assets = [];
      const work = path.join(staging, `${app.name}-work`);
      mkdirSync(work);
      for (const relative of inputs) {
        const input = readFileSync(path.join(app.directory, relative));
        inputRecords.push({ path: relative, ...describe(input) });
        if (relative === 'index.html') continue;
        let bytes = input;
        if (relative.endsWith('.wasm') && variant !== 'none') {
          const temporaryInput = path.join(work, 'input.wasm');
          const temporaryOutput = path.join(work, 'output.wasm');
          writeFileSync(temporaryInput, input);
          const result = spawnSync(wasmOpt, [temporaryInput, ...settings.wasmOptArgs, '-o', temporaryOutput], { encoding: 'utf8' });
          if (result.error || result.status !== 0) throw new Error(`wasm-opt failed for ${app.name}/${relative}: ${result.error?.message || result.stderr}`);
          bytes = readFileSync(temporaryOutput);
          rmSync(temporaryOutput);
        }
        assets.push({ path: relative, bytes });
      }
      const inputIdentity = sha256(serialise(inputRecords));
      // Provenance changes alone must not invalidate otherwise identical public assets.
      const contentConfiguration = sourceIdentity && typeof sourceIdentity === 'object'
        ? Object.fromEntries(Object.entries(sourceIdentity).filter(([key]) => !['revision', 'dirty'].includes(key))) : sourceIdentity;
      const contentId = sha256(serialise({ schemaVersion: 1, inputIdentity, contentConfiguration, settings, toolVersions, assets: assets.map(asset => ({ path: asset.path, ...describe(asset.bytes) })) }));
      const assetDirectory = `${app.name}/assets/${contentId}`;
      const records = assets.map(asset => writeRepresentations(staging, `${assetDirectory}/${asset.path}`, asset.bytes));
      let html = readFileSync(path.join(app.directory, 'index.html'), 'utf8');
      if (/<base\b/i.test(html) || !/<head(?:\s[^>]*)?>/i.test(html)) throw new Error(`${app.name}: expected HTML head without an existing base`);
      html = html.replace(/<head(?:\s[^>]*)?>/i, match => `${match}\n<base href="./assets/${contentId}/">`);
      const entry = writeRepresentations(staging, `${app.name}/index.html`, Buffer.from(html));
      manifest.apps.push({ name: app.name, inputIdentity, inputs: inputRecords, contentId, assetDirectory, entry, assets: records });
      rmSync(work, { recursive: true });
    }
    writeFileSync(path.join(staging, 'manifest.json'), serialise(manifest));
    writeFileSync(path.join(staging, '.browser-package'), '1\n');
    // Keep the previous coherent package until all optimisation/compression has succeeded.
    const previous = `${staging}-previous`;
    if (existsSync(output)) renameSync(output, previous);
    try { renameSync(staging, output); } catch (error) {
      if (existsSync(previous)) renameSync(previous, output);
      throw error;
    }
    rmSync(previous, { recursive: true, force: true });
    return manifest;
  } finally {
    rmSync(staging, { recursive: true, force: true });
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv[2] === '--check-optimiser') {
      console.log(optimiserVersion(defaultOptimiser()));
    } else {
    const options = {};
    for (let index = 2; index < process.argv.length; index += 2) {
      const flag = process.argv[index], value = process.argv[index + 1];
      if (!value) throw new Error(`Missing value for ${flag}`);
      if (flag === '--app') {
        const separator = value.indexOf('=');
        if (separator < 1) throw new Error('--app requires name=directory');
        (options.apps ??= []).push({ name: value.slice(0, separator), directory: value.slice(separator + 1) });
      } else if (flag === '--output') options.output = value;
      else if (flag === '--variant') options.variant = value;
      else if (flag === '--wasm-opt') options.wasmOpt = value;
      else if (flag === '--source-identity') options.sourceIdentity = value;
      else if (flag === '--source-identity-file') {
        const identity = JSON.parse(readFileSync(value, 'utf8'));
        if (!identity || typeof identity !== 'object' || Array.isArray(identity)) throw new Error('--source-identity-file requires a JSON object');
        options.sourceIdentity = identity;
      }
      else throw new Error(`Unknown option ${flag}`);
    }
    console.log(JSON.stringify(packageBrowser(options), null, 2));
    }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
