#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
"$ROOT/tools/bootstrap-linux-ui.sh"
EVIDENCE_DIR="${POLYORAMA_EVIDENCE_DIR:-$ROOT/docs/design-agent-loop-evidence}"
mkdir -p "$EVIDENCE_DIR" .tools/runtime

command -v bwrap >/dev/null
command -v import >/dev/null
IMPORT="$(command -v import)"
SYSROOT="$ROOT/.tools/sysroot"
LIBS="$SYSROOT/usr/lib"
XDO="$SYSROOT/usr/bin/xdotool"
DISPLAY_NUMBER=:96
XVFB_LOG="$EVIDENCE_DIR/gallery-native-xvfb.log"
APP_LOG="$EVIDENCE_DIR/gallery-native-runtime.log"
SNAPSHOT="$ROOT/.tools/runtime/gallery-native-snapshot.json"
SMOKE_TMP="$ROOT/.tools/runtime/gallery-native-x11-tmp"

mkdir -p "$SMOKE_TMP/.X11-unix"
find "$SMOKE_TMP" -mindepth 1 -maxdepth 1 ! -name '.X11-unix' -delete
find "$SMOKE_TMP/.X11-unix" -mindepth 1 -delete
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"

ui_sandbox() {
  bwrap --ro-bind / / --bind "$SMOKE_TMP" /tmp --ro-bind /usr/bin /opt \
    --ro-bind "$SYSROOT/usr/bin" /usr/bin \
    --bind "$ROOT/.tools/runtime" "$ROOT/.tools/runtime" \
    --bind "$EVIDENCE_DIR" "$EVIDENCE_DIR" \
    --dev-bind /dev /dev --proc /proc "$@"
}
xdo() {
  DISPLAY="$DISPLAY_NUMBER" LD_LIBRARY_PATH="$LIBS" ui_sandbox "$XDO" "$@"
}

LD_LIBRARY_PATH="$LIBS" ui_sandbox "$SYSROOT/usr/bin/Xvfb" "$DISPLAY_NUMBER" \
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

: >"$APP_LOG"
rm -f "$SNAPSHOT"
DISPLAY="$DISPLAY_NUMBER" LD_LIBRARY_PATH="$LIBS" WGPU_BACKEND=gl \
  POLYORAMA_GALLERY_STORY=reference/application-shell \
  POLYORAMA_GALLERY_SNAPSHOT_PATH="$SNAPSHOT" \
  ui_sandbox target/release/polyorama-gallery >>"$APP_LOG" 2>&1 &
APP_PID=$!
sleep 5
kill -0 "$APP_PID"
WINDOW_ID="$(xdo search --onlyvisible --name 'Polyorama Component Gallery' | head -n 1)"
xdo windowfocus --sync "$WINDOW_ID"
DISPLAY="$DISPLAY_NUMBER" ui_sandbox "$IMPORT" -window root "$EVIDENCE_DIR/gallery-native-overview.png"
xdo key --window "$WINDOW_ID" F12
for _ in 1 2 3 4 5; do
  [[ -s "$SNAPSHOT" ]] && break
  sleep 0.2
done
test -s "$SNAPSHOT"
jq -e '
  .story == "reference/application-shell"
  and .story_count == 18
  and (.text | length) > 0
  and (.text_audit | length) == 0
  and (.ui_snapshot.nodes | length) > 0
  and (.ui_snapshot.nodes | length) < 1000
  and (.ui_snapshot.semantic_audit | length) == 0
  and any(.ui_snapshot.nodes[]; .role == "tab")
  and any(.ui_snapshot.nodes[]; .role == "splitter")
  and (.story_rect.max_x > .story_rect.min_x)
  and (.story_rect.max_y > .story_rect.min_y)
' "$SNAPSHOT" >/dev/null
cp "$SNAPSHOT" "$EVIDENCE_DIR/gallery-native-snapshot.json"
if grep -E "panicked|WGPU error|Exiting because of error" "$APP_LOG"; then
  echo "native gallery smoke observed an application failure" >&2
  exit 1
fi
echo "native gallery smoke passed: GL/llvmpipe, 18 stories, empty text and semantic audits"
