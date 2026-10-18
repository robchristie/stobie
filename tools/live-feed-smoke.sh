#!/usr/bin/env bash
# Run tools/live-feed-smoke.mjs for one host, native or browser, under a
# private Xvfb display and the repository UI sysroot. The native host needs
# `cargo build --release -p live-feed`; the browser host needs
# `cargo xtask build-web`.
set -euo pipefail
HOST="${1:?usage: tools/live-feed-smoke.sh native|browser}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${STOBIE_EVIDENCE_DIR:-$ROOT/.tools/runtime/live-feed-evidence}/live-feed"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/live-feed-x11-XXXXXX")"
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
export DISPLAY="${STOBIE_LIVE_FEED_DISPLAY:-:98}" WGPU_BACKEND=gl XDG_RUNTIME_DIR=/tmp/runtime STOBIE_BROWSER_HEADFUL=1
owned_start XVFB_PID "$XVFB" "$DISPLAY" -screen 0 1280x800x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/xvfb-$HOST.log" 2>&1
sleep 1
ui_sandbox "$(command -v node)" tools/live-feed-smoke.mjs "$HOST" "$EVIDENCE_DIR"
