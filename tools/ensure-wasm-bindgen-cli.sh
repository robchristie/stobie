#!/usr/bin/env bash
set -euo pipefail

expected_version="0.2.127"
observed_version=""
if ! observed_version="$(wasm-bindgen --version 2>/dev/null)" || \
   [[ "$observed_version" != "wasm-bindgen $expected_version" ]]; then
  printf 'Installing wasm-bindgen CLI %s (observed %s)\n' "$expected_version" "${observed_version:-missing}"
  cargo install --locked --force wasm-bindgen-cli --version "$expected_version"
fi

observed_version=""
if ! observed_version="$(wasm-bindgen --version 2>/dev/null)" || \
   [[ "$observed_version" != "wasm-bindgen $expected_version" ]]; then
  printf 'Expected wasm-bindgen CLI %s after installation; observed %s\n' \
    "$expected_version" "${observed_version:-missing}" >&2
  exit 1
fi
printf 'Using %s\n' "$observed_version"
