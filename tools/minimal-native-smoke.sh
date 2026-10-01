#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
"$ROOT/tools/bootstrap-linux-ui.sh"
EVIDENCE_DIR="${POLYORAMA_EVIDENCE_DIR:-$ROOT/.tools/runtime/minimal-native-evidence}"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
SYSROOT="$ROOT/.tools/sysroot"
IMPORT="$(command -v import)"
if [[ "${POLYORAMA_USE_SYSTEM_UI_LIBS:-0}" == "1" ]]; then
  XDO="$(command -v xdotool)"
  XVFB="$(command -v Xvfb)"
else
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XDO="$SYSROOT/usr/bin/xdotool"
  XVFB="$SYSROOT/usr/bin/Xvfb"
fi
DISPLAY_NUMBER=:94
SNAPSHOT="$ROOT/.tools/runtime/minimal-native-snapshot.json"
SMOKE_TMP="$ROOT/.tools/runtime/minimal-native-x11-tmp"
APP_LOG="$EVIDENCE_DIR/minimal-native-runtime.log"
mkdir -p "$SMOKE_TMP/.X11-unix"
find "$SMOKE_TMP" -mindepth 1 -maxdepth 1 ! -name '.X11-unix' -delete
find "$SMOKE_TMP/.X11-unix" -mindepth 1 -delete
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
source "$ROOT/tools/native-smoke-lifecycle.sh"
xdo() { DISPLAY="$DISPLAY_NUMBER" ui_sandbox "$XDO" "$@"; }
owned_start XVFB_PID "$XVFB" "$DISPLAY_NUMBER" \
  -screen 0 900x700x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/minimal-native-xvfb.log" 2>&1
rm -f "$SNAPSHOT"
sleep 1
DISPLAY="$DISPLAY_NUMBER" WGPU_BACKEND=gl POLYORAMA_MINIMAL_SNAPSHOT="$SNAPSHOT" \
  owned_start APP_PID target/release/examples/minimal-workspace >"$APP_LOG" 2>&1

wait_snapshot() {
  local count="$1"
  for _ in {1..100}; do
    kill -0 "$APP_PID"
    if [[ -s "$SNAPSHOT" ]] && jq -e --argjson count "$count" \
      '.annotations == $count and .undo_entries == $count and .displayed_count == $count
       and (.text_audit | length) == 0 and (.text | length) >= 4' "$SNAPSHOT" >/dev/null 2>&1; then
      return
    fi
    sleep 0.1
  done
  echo "minimal native smoke timed out waiting for $count committed triangles" >&2
  exit 1
}
wait_snapshot 0
WINDOW_ID=""
for _ in {1..100}; do
  WINDOW_ID="$(xdo search --onlyvisible --name 'Polyorama Minimal Workspace' | head -n 1 || true)"
  [[ -n "$WINDOW_ID" ]] && break
  kill -0 "$APP_PID"
  sleep 0.1
done
[[ -n "$WINDOW_ID" ]] || { echo "minimal native window did not become visible" >&2; exit 1; }
xdo windowfocus --sync "$WINDOW_ID"
read -r CLICK_X CLICK_Y < <(jq -r '
  .nodes[] | select(.actions | index("minimal.add-triangle")) |
  [((.rect.min_x + .rect.max_x) / 2 | floor), ((.rect.min_y + .rect.max_y) / 2 | floor)] | @tsv
' "$SNAPSHOT")
cp "$SNAPSHOT" "$EVIDENCE_DIR/minimal-native-before.json"
for count in 1 2; do
  xdo mousemove --window "$WINDOW_ID" "$CLICK_X" "$CLICK_Y"
  xdo click 1
  wait_snapshot "$count"
  cp "$SNAPSHOT" "$EVIDENCE_DIR/minimal-native-after-$count.json"
done
DISPLAY="$DISPLAY_NUMBER" ui_sandbox "$IMPORT" -window "$WINDOW_ID" \
  "$EVIDENCE_DIR/minimal-native-after.png"
if rg 'panicked|WGPU error|Exiting because of error' "$APP_LOG"; then
  echo "minimal native smoke observed an application failure" >&2
  exit 1
fi
echo "minimal native smoke passed: physical Add actions, 0→1→2 triangles/history entries, measured count and empty text audits"
