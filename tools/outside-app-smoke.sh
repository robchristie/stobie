#!/usr/bin/env bash
# Prove the outside-the-repository route: `stobie new` creates an application
# in a directory outside this checkout, pointed at this checkout's source with
# `--stobie-path`; it is tested and built natively and with `stobie build-web`,
# then tools/outside-app-smoke.mjs starts it natively and in Chromium on WebGPU
# and WebGL2 and operates it with `stobie`. Further applications check Web
# Push, per-row actions and the composer. The release `stobie` binary must
# already be built.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
STOBIE="$ROOT/target/release/stobie"
[[ -x "$STOBIE" ]] || { echo "build stobie first: cargo build --release -p stobie-cli" >&2; exit 2; }
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${STOBIE_EVIDENCE_DIR:-$ROOT/.tools/runtime/outside-app-evidence}/outside-app"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"

# A temporary directory this script creates is removed when it exits.
OWN_OUTSIDE=0
if [[ -z "${STOBIE_OUTSIDE_DIR:-}" ]]; then OWN_OUTSIDE=1; fi
OUTSIDE="${STOBIE_OUTSIDE_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/stobie-outside-XXXXXX")}"
mkdir -p "$OUTSIDE"
OUTSIDE="$(cd "$OUTSIDE" && pwd -P)"
remove_outside() { if (( OWN_OUTSIDE )); then rm -rf -- "$OUTSIDE"; fi; }
trap remove_outside EXIT
case "$OUTSIDE/" in
  "$(pwd -P)/"*) echo "STOBIE_OUTSIDE_DIR must be outside the checkout: $OUTSIDE" >&2; exit 2 ;;
esac
# Generated applications compile into this checkout's ignored target area,
# so CI caches them and the scratch directory stays small.
export CARGO_TARGET_DIR="$ROOT/target/outside-app"

# The CLI pins the revision it was built from. Cargo fetches the pin even
# when a local patch replaces it, so pin a commit GitHub has: CI's checkout
# commit, or locally the merge base with origin/main when HEAD is unpushed.
NEW_FLAGS=()
if [[ -n "${STOBIE_OUTSIDE_PIN:-}" ]]; then
  NEW_FLAGS=(--stobie-rev "$STOBIE_OUTSIDE_PIN")
elif [[ -z "${GITHUB_ACTIONS:-}" && -z "$(git branch -r --contains HEAD 2>/dev/null)" ]]; then
  NEW_FLAGS=(--stobie-rev "$(git merge-base HEAD origin/main)")
fi

APP="$OUTSIDE/outside-notes"
rm -rf "$APP" "$OUTSIDE/outside-imaging"
"$STOBIE" new outside-notes --path "$APP" --stobie-path "$ROOT" "${NEW_FLAGS[@]}" --json >"$EVIDENCE_DIR/new.json"
(
  cd "$APP"
  cargo fmt --check
  cargo test
  cargo clippy --all-targets -- -D warnings
  cargo build --release
  "$STOBIE" build-web --json >"$EVIDENCE_DIR/build-web.json"
  "$STOBIE" build-web --production --output "$OUTSIDE/production" --json >"$EVIDENCE_DIR/build-web-production.json"
)
# A second application opts into Web Push: its library is the check's
# (tools/web-push-app/lib.rs) and its page declares stobie-web-push.
PUSH_APP="$OUTSIDE/outside-push"
rm -rf "$PUSH_APP" "$OUTSIDE/push-production"
"$STOBIE" new outside-push --path "$PUSH_APP" --stobie-path "$ROOT" "${NEW_FLAGS[@]}" >/dev/null
cp tools/web-push-app/lib.rs "$PUSH_APP/src/lib.rs"
sed -i 's|<link rel="manifest"|<meta name="stobie-web-push" content="enabled" />\n    <link rel="manifest"|' "$PUSH_APP/web/index.html"
grep -q 'name="stobie-web-push"' "$PUSH_APP/web/index.html"
(
  cd "$PUSH_APP"
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings
  "$STOBIE" build-web --production --output "$OUTSIDE/push-production" --json >"$EVIDENCE_DIR/build-web-push.json"
)
# A third presents its actions once per row (tools/row-actions-app/lib.rs).
ROWS_APP="$OUTSIDE/outside-rows"
rm -rf "$ROWS_APP"
"$STOBIE" new outside-rows --path "$ROWS_APP" --stobie-path "$ROOT" "${NEW_FLAGS[@]}" >/dev/null
cp tools/row-actions-app/lib.rs "$ROWS_APP/src/lib.rs"
(
  cd "$ROWS_APP"
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings
  cargo build --release
  "$STOBIE" build-web --json >"$EVIDENCE_DIR/build-web-rows.json"
)
# A fourth edits an application-owned draft in a composer
# (tools/composer-app/lib.rs).
COMPOSER_APP="$OUTSIDE/outside-composer"
rm -rf "$COMPOSER_APP"
"$STOBIE" new outside-composer --path "$COMPOSER_APP" --stobie-path "$ROOT" "${NEW_FLAGS[@]}" >/dev/null
cp tools/composer-app/lib.rs "$COMPOSER_APP/src/lib.rs"
(
  cd "$COMPOSER_APP"
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings
  cargo build --release
  "$STOBIE" build-web --json >"$EVIDENCE_DIR/build-web-composer.json"
)
# The image layer compiles natively and for the web from outside too.
"$STOBIE" new outside-imaging --imaging --path "$OUTSIDE/outside-imaging" --stobie-path "$ROOT" "${NEW_FLAGS[@]}" >/dev/null
(
  cd "$OUTSIDE/outside-imaging"
  cargo check --all-targets
  cargo check --lib --target wasm32-unknown-unknown
)

SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/outside-app-x11-XXXXXX")"
mkdir -p "$SMOKE_TMP/.X11-unix" "$SMOKE_TMP/runtime"
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
chmod 0700 "$SMOKE_TMP/runtime"
if [[ "${STOBIE_USE_SYSTEM_UI_LIBS:-0}" == 1 ]]; then
  XVFB="$(command -v Xvfb)"
else
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XVFB="$SYSROOT/usr/bin/Xvfb"
  # Chromium's hidden text input needs a real font for typed text to arrive.
  export FONTCONFIG_FILE="$SMOKE_TMP/fonts.conf"
  cat >"$FONTCONFIG_FILE" <<EOF
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig><dir>$ROOT/crates/stobie-ui-egui/assets/fonts</dir><cachedir>$SMOKE_TMP/font-cache</cachedir></fontconfig>
EOF
fi
source tools/native-smoke-lifecycle.sh
# The generated application lives outside every directory the sandbox binds.
if (( ${#UI_COMMAND[@]} )); then
  UI_COMMAND+=(--bind "$OUTSIDE" "$OUTSIDE")
fi
trap 'cleanup; rm -rf -- "$SMOKE_TMP"; remove_outside' EXIT
export DISPLAY=:93 WGPU_BACKEND=gl XDG_RUNTIME_DIR=/tmp/runtime STOBIE_BROWSER_HEADFUL=1
owned_start XVFB_PID "$XVFB" "$DISPLAY" -screen 0 1440x1000x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/xvfb.log" 2>&1
sleep 1
ui_sandbox "$(command -v node)" tools/outside-app-smoke.mjs "$STOBIE" "$APP" "$CARGO_TARGET_DIR/release/outside-notes" "$EVIDENCE_DIR"
# The generated page is installable: both builds wrote its manifest and
# service worker, which Chromium accepts, and it starts offline.
ui_sandbox "$(command -v node)" tools/pwa-smoke.mjs outside-notes "$APP/web" "$OUTSIDE/production" "$EVIDENCE_DIR"
# The opted-in application subscribes inside a tap, shows every push and
# receives a tapped notification's route, under a strict CSP.
ui_sandbox "$(command -v node)" tools/web-push-smoke.mjs outside-push "$OUTSIDE/push-production" "$EVIDENCE_DIR"
# Each row's control carries its row's ID through `stobie` natively and in
# the browser, and the ARIA layer names it after its row.
ui_sandbox "$(command -v node)" tools/row-actions-smoke.mjs "$STOBIE" "$ROWS_APP" "$CARGO_TARGET_DIR/release/outside-rows" "$EVIDENCE_DIR"
# A composer's draft is typed and sent by keyboard and by touch in Chromium,
# and set and sent by agents through `stobie` natively and in the browser.
ui_sandbox "$(command -v node)" tools/composer-smoke.mjs "$STOBIE" "$COMPOSER_APP" "$CARGO_TARGET_DIR/release/outside-composer" "$EVIDENCE_DIR"
