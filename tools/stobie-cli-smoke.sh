#!/usr/bin/env bash
# Run tools/stobie-cli-smoke.mjs for one application (record-desk, lab or
# record-desk-browser) on a private Xvfb display, as the native smokes do. The
# application (or Record Desk's web build) and the `stobie` binary must
# already be built in release mode.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
APP="${1:?usage: stobie-cli-smoke.sh record-desk|lab|record-desk-browser}"
case "$APP" in
  record-desk) BINARY="$ROOT/consumers/record-desk/target/release/record-desk" ;;
  lab) BINARY="$ROOT/target/release/analytical-workspace-lab" ;;
  record-desk-browser) BINARY="$ROOT/consumers/record-desk/web"; export STOBIE_BROWSER_HEADFUL=1 ;;
  *) echo "unknown application $APP" >&2; exit 2 ;;
esac
STOBIE="$ROOT/target/release/stobie"
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${STOBIE_EVIDENCE_DIR:-$ROOT/.tools/runtime/stobie-cli-evidence}/stobie-cli"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/stobie-cli-x11-XXXXXX")"
mkdir -p "$SMOKE_TMP/.X11-unix" "$SMOKE_TMP/runtime"
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
chmod 0700 "$SMOKE_TMP/runtime"
if [[ "${STOBIE_USE_SYSTEM_UI_LIBS:-0}" == 1 ]]; then
  XVFB="$(command -v Xvfb)"
else
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XVFB="$SYSROOT/usr/bin/Xvfb"
fi
source tools/native-smoke-lifecycle.sh
trap 'cleanup; rm -rf -- "$SMOKE_TMP"' EXIT
# Sessions, sockets and saved state stay in the smoke's private directory.
export DISPLAY=:94 WGPU_BACKEND=gl XDG_RUNTIME_DIR=/tmp/runtime RECORD_DESK_STATE_PATH=/tmp/record-desk-state.json
owned_start XVFB_PID "$XVFB" "$DISPLAY" -screen 0 1440x1000x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/stobie-cli-xvfb-$APP.log" 2>&1
sleep 1
ui_sandbox "$(command -v node)" tools/stobie-cli-smoke.mjs "$STOBIE" "$BINARY" "$APP" "$EVIDENCE_DIR"
