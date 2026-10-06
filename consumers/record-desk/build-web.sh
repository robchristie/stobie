#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
[[ "$(wasm-bindgen --version)" == 'wasm-bindgen 0.2.127' ]] || {
  echo 'Install wasm-bindgen-cli 0.2.127 before building Record Desk.' >&2; exit 1;
}
cargo build --locked --release --target wasm32-unknown-unknown --lib
wasm-bindgen --target web --out-dir web/pkg target/wasm32-unknown-unknown/release/record_desk.wasm
