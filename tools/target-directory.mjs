// The directories Cargo builds this repository's workspace and Record Desk
// into. The smoke wrappers resolve them outside their sandbox, which has no
// cargo, and export STOBIE_TARGET_DIR and STOBIE_RECORD_DESK_TARGET_DIR; run
// directly, a smoke asks Cargo, which honours CARGO_TARGET_DIR and its
// configuration.
import { execFileSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export function cargoTargetDirectory(manifestPath) {
  const args = ['metadata', '--format-version', '1', '--no-deps'];
  if (manifestPath) args.push('--manifest-path', resolve(root, manifestPath));
  return JSON.parse(execFileSync('cargo', args, { cwd: root, encoding: 'utf8' })).target_directory;
}

export function stobieTargetDirectory() {
  return process.env.STOBIE_TARGET_DIR || cargoTargetDirectory();
}

export function recordDeskTargetDirectory() {
  return process.env.STOBIE_RECORD_DESK_TARGET_DIR || cargoTargetDirectory('consumers/record-desk/Cargo.toml');
}
