#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
"$ROOT/tools/bootstrap-linux-ui.sh"

command -v bwrap >/dev/null
command -v import >/dev/null
mkdir -p docs/vertical-slice-evidence .tools/runtime
rm -f .tools/runtime/native-storage.ron

SYSROOT="$ROOT/.tools/sysroot"
LIBS="$SYSROOT/usr/lib"
XDO="$SYSROOT/usr/bin/xdotool"
DISPLAY_NUMBER=:97
XVFB_LOG=docs/vertical-slice-evidence/native-xvfb.log
APP_LOG=docs/vertical-slice-evidence/native-runtime.log

bwrap --ro-bind / / --bind /tmp /tmp --ro-bind /usr/bin /opt \
  --ro-bind "$SYSROOT/usr/bin" /usr/bin --dev-bind /dev /dev --proc /proc \
  --setenv LD_LIBRARY_PATH "$LIBS" "$SYSROOT/usr/bin/Xvfb" "$DISPLAY_NUMBER" \
  -screen 0 1440x900x24 -nolisten tcp +extension GLX >"$XVFB_LOG" 2>&1 &
XVFB_PID=$!
APP_PID=""
cleanup() {
  if [[ -n "$APP_PID" ]]; then kill "$APP_PID" 2>/dev/null || true; fi
  kill "$XVFB_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  wait "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT
sleep 1

launch_app() {
  DISPLAY="$DISPLAY_NUMBER" LD_LIBRARY_PATH="$LIBS" WGPU_BACKEND=gl RUST_LOG=info \
    POLYORAMA_PERSISTENCE_PATH="$ROOT/.tools/runtime/native-storage.ron" \
    target/release/analytical-workspace-lab >>"$APP_LOG" 2>&1 &
  APP_PID=$!
  sleep 5
  kill -0 "$APP_PID"
}

xdo() { DISPLAY="$DISPLAY_NUMBER" LD_LIBRARY_PATH="$LIBS" "$XDO" "$@"; }
capture() { DISPLAY="$DISPLAY_NUMBER" import -window root "docs/vertical-slice-evidence/$1"; }

: >"$APP_LOG"
launch_app
capture native-default.png

# Pointer-centred zoom and a coalesced linked-camera drag.
xdo mousemove 300 300 click --repeat 3 --delay 80 4
xdo mousemove 300 300 mousedown 1 mousemove --sync 360 335 mouseup 1
sleep 1
capture native-linked-camera.png
xdo mousemove 211 77 click 1
sleep 1

# Construct, commit, edit, undo and redo a world-coordinate polygon.
xdo mousemove 110 77 click 1
sleep 1
xdo mousemove 180 180 click 1
xdo mousemove 360 200 click 1
xdo mousemove 270 340 click 1
sleep 1
xdo mousemove 270 340 click 3
sleep 2
capture native-polygon.png
xdo mousemove 165 77 click 1
xdo mousemove 180 180 mousedown 1 mousemove --sync 215 205 mouseup 1
xdo mousemove 195 18 click 1
xdo mousemove 250 18 click 1
xdo key Delete
xdo mousemove 195 18 click 1
xdo mousemove 250 18 click 1
xdo mousemove 195 18 click 1

# Result selection/recentring and progressive thumbnail virtualisation.
xdo mousemove 1120 105 click 1
xdo mousemove 1260 77 click 1
xdo mousemove 1170 53 click 1
xdo mousemove 1180 280 click --repeat 5 --delay 50 5
sleep 2
capture native-thumbnails.png

# Diagnostics, then a dock split resize and pane drag/drop.
xdo mousemove 1180 505 click 1
xdo mousemove 1220 770 click --repeat 6 --delay 50 5
sleep 1
capture native-diagnostics.png
xdo mousemove 1037 400 mousedown 1 mousemove --sync 990 400 mouseup 1
xdo mousemove 550 642 mousedown 1
sleep 1
xdo mousemove 800 500
sleep 1
xdo mousemove 1150 200
sleep 1
xdo mouseup 1
sleep 2
capture native-rearranged-dock.png
xdo mousemove 325 18 click 1
sleep 1

kill "$APP_PID"
wait "$APP_PID" 2>/dev/null || true
APP_PID=""
launch_app
capture native-restored-layout.png

if grep -E "panicked|WGPU error|Exiting because of error" "$APP_LOG"; then
  echo "native smoke test observed an application failure" >&2
  exit 1
fi
echo "native smoke passed: GL/llvmpipe, 1440x900, screenshots and interaction evidence captured"
