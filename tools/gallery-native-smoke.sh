#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
"$ROOT/tools/bootstrap-linux-ui.sh"
EVIDENCE_DIR="${STOBIE_EVIDENCE_DIR:-$ROOT/.tools/runtime/gallery-smoke-evidence}"
mkdir -p "$EVIDENCE_DIR" .tools/runtime

command -v import >/dev/null
IMPORT="$(command -v import)"
SYSROOT="$ROOT/.tools/sysroot"
if [[ "${STOBIE_USE_SYSTEM_UI_LIBS:-0}" == "1" ]]; then
  command -v Xvfb >/dev/null
  command -v xdotool >/dev/null
  XDO="$(command -v xdotool)"
  XVFB="$(command -v Xvfb)"
else
  command -v bwrap >/dev/null
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XDO="$SYSROOT/usr/bin/xdotool"
  XVFB="$SYSROOT/usr/bin/Xvfb"
fi
XVFB_LOG="$EVIDENCE_DIR/gallery-native-xvfb.log"
APP_LOG="$EVIDENCE_DIR/gallery-native-runtime.log"
SNAPSHOT="$ROOT/.tools/runtime/gallery-native-snapshot.json"
STORY_COUNT="$(sed -n 's/.*pub const ALL: \[Self; \([0-9]*\)\].*/\1/p' apps/stobie-gallery/src/catalogue.rs)"
[[ "$STORY_COUNT" =~ ^[0-9]+$ ]]
SMOKE_TMP="$ROOT/.tools/runtime/gallery-native-x11-tmp"

mkdir -p "$SMOKE_TMP/.X11-unix"
find "$SMOKE_TMP" -mindepth 1 -maxdepth 1 ! -name '.X11-unix' -delete
find "$SMOKE_TMP/.X11-unix" -mindepth 1 -delete
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"

source "$ROOT/tools/native-smoke-lifecycle.sh"
xdo() {
  DISPLAY="$DISPLAY_NUMBER" ui_sandbox "$XDO" "$@"
}

owned_start_xvfb "$XVFB_LOG" -screen 0 1440x900x24 -nolisten tcp +extension GLX
DISPLAY_NUMBER="$DISPLAY"

: >"$APP_LOG"
rm -f "$SNAPSHOT"
DISPLAY="$DISPLAY_NUMBER" WGPU_BACKEND=gl \
  STOBIE_GALLERY_STORY=reference/application-shell \
  STOBIE_GALLERY_SNAPSHOT_PATH="$SNAPSHOT" \
  owned_start APP_PID "$STOBIE_TARGET_DIR"/release/stobie-gallery >>"$APP_LOG" 2>&1
WINDOW_ID="$(owned_wait_window 'Stobie Component Gallery' "$APP_PID")"
xdo windowfocus --sync "$WINDOW_ID"
DISPLAY="$DISPLAY_NUMBER" ui_sandbox "$IMPORT" -window root "$EVIDENCE_DIR/gallery-native-overview.png"
xdo key --window "$WINDOW_ID" F12
for _ in 1 2 3 4 5; do
  [[ -s "$SNAPSHOT" ]] && break
  sleep 0.2
done
test -s "$SNAPSHOT"
jq -e --argjson story_count "$STORY_COUNT" '
  .story == "reference/application-shell"
  and .story_count == $story_count
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
echo "native gallery smoke passed: GL/llvmpipe, $STORY_COUNT stories, empty text and semantic audits"
