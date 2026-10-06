#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${POLYORAMA_EVIDENCE_DIR:-$ROOT/.tools/runtime/record-desk-evidence}/record-desk"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/record-desk-native-x11-XXXXXX")"
mkdir -p "$SMOKE_TMP/.X11-unix"
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
if [[ "${POLYORAMA_USE_SYSTEM_UI_LIBS:-0}" == 1 ]]; then
  XVFB="$(command -v Xvfb)"
  XDO="$(command -v xdotool)"
else
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XVFB="$SYSROOT/usr/bin/Xvfb"
  XDO="$SYSROOT/usr/bin/xdotool"
fi
source tools/native-smoke-lifecycle.sh
trap 'cleanup; rm -rf -- "$SMOKE_TMP"' EXIT
export DISPLAY=:92 WGPU_BACKEND=gl
owned_start XVFB_PID "$XVFB" "$DISPLAY" -screen 0 1440x1000x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/native-xvfb.log" 2>&1
sleep 1
ui_sandbox "$(command -v python3)" tools/record-desk-native-smoke.py "$EVIDENCE_DIR" "$XDO" "$(command -v import)"
