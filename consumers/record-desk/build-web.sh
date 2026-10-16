#!/usr/bin/env bash
# Build Record Desk for the browser with `stobie build-web`, the packaging path
# every Stobie application uses: a locked release wasm build, wasm-bindgen into
# web/pkg (checked against the locked wasm-bindgen crate) and the startup
# loader. In this repository it runs the checkout's own CLI; set STOBIE to use
# another, such as an installed `stobie`.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
if [[ -n "${STOBIE:-}" ]]; then
  exec "$STOBIE" build-web --locked "$@"
fi
exec cargo run --quiet --locked --release --manifest-path ../../Cargo.toml -p stobie-cli -- build-web --locked "$@"
