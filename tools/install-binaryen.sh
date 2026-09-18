#!/usr/bin/env bash
# Pinned tool only; installs into ignored repository-local storage.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)-$(uname -m)" != Linux-x86_64 ]]; then
  echo 'Install Binaryen 131 for your platform and set WASM_OPT to its wasm-opt executable.' >&2
  exit 1
fi
mkdir -p .tools/browser
archive=.tools/browser/binaryen-version_131-x86_64-linux.tar.gz
if [[ ! -f "$archive" ]]; then
  curl --fail --location --output "$archive.partial" \
    https://github.com/WebAssembly/binaryen/releases/download/version_131/binaryen-version_131-x86_64-linux.tar.gz
  mv "$archive.partial" "$archive"
fi
printf '%s  %s\n' b5bf1f0eaf17c63ee588ff7a5954dc8f6ce2c26989051c66f24dfe9ece3e46db "$archive" | sha256sum --check
if [[ ! -x .tools/browser/binaryen-version_131/bin/wasm-opt ]]; then
  tar -xzf "$archive" -C .tools/browser
fi
.tools/browser/binaryen-version_131/bin/wasm-opt --version
