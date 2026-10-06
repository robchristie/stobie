#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ "$(uname)" != Linux ]]; then
  exec node tools/record-desk-browser-smoke.mjs
fi
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${POLYORAMA_EVIDENCE_DIR:-$ROOT/.tools/runtime/record-desk-evidence}/record-desk"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/record-desk-browser-x11-XXXXXX")"
mkdir -p "$SMOKE_TMP/.X11-unix"
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
if [[ "${POLYORAMA_USE_SYSTEM_UI_LIBS:-0}" == 1 ]]; then
  XVFB="$(command -v Xvfb)"
else
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XVFB="$SYSROOT/usr/bin/Xvfb"
  # Minimal hosts may have fontconfig but no installed system fonts. Chromium's
  # hidden editable input needs a real font even though egui draws the canvas.
  export FONTCONFIG_FILE="$SMOKE_TMP/fonts.conf"
  cat >"$FONTCONFIG_FILE" <<EOF
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig><dir>$ROOT/crates/polyorama-ui-egui/assets/fonts</dir><cachedir>$SMOKE_TMP/font-cache</cachedir></fontconfig>
EOF
fi
source tools/native-smoke-lifecycle.sh
trap 'cleanup; rm -rf -- "$SMOKE_TMP"' EXIT
export DISPLAY=:91 POLYORAMA_BROWSER_HEADFUL=1
owned_start XVFB_PID "$XVFB" "$DISPLAY" -screen 0 1440x1000x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/browser-xvfb.log" 2>&1
sleep 1
POLYORAMA_EVIDENCE_DIR="$EVIDENCE_DIR" ui_sandbox "$(command -v node)" tools/record-desk-browser-smoke.mjs
