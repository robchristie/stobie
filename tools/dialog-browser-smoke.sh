#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ "$(uname)" != Linux ]]; then
  exec node tools/dialog-browser-smoke.mjs
fi
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${STOBIE_EVIDENCE_DIR:-$ROOT/.tools/runtime/dialog-evidence}/dialog-browser"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/dialog-browser-x11-XXXXXX")"
mkdir -p "$SMOKE_TMP/.X11-unix"
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
if [[ "${STOBIE_USE_SYSTEM_UI_LIBS:-0}" == 1 ]]; then
  XVFB="$(command -v Xvfb)"
else
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XVFB="$SYSROOT/usr/bin/Xvfb"
  # Minimal hosts may have fontconfig but no installed system fonts. Chromium's
  # hidden editable input needs a real font even though egui draws the canvas.
  export FONTCONFIG_FILE="$SMOKE_TMP/fonts.conf"
  cat >"$FONTCONFIG_FILE" <<FONTS
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig><dir>$ROOT/crates/stobie-ui-egui/assets/fonts</dir><cachedir>$SMOKE_TMP/font-cache</cachedir></fontconfig>
FONTS
fi
source tools/native-smoke-lifecycle.sh
trap 'cleanup; rm -rf -- "$SMOKE_TMP"' EXIT
export STOBIE_BROWSER_HEADFUL=1
owned_start_xvfb "$EVIDENCE_DIR/browser-xvfb.log" -screen 0 1440x900x24 -nolisten tcp +extension GLX
STOBIE_EVIDENCE_DIR="$EVIDENCE_DIR" ui_sandbox "$(command -v node)" tools/dialog-browser-smoke.mjs
