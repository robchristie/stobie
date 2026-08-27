#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
"$ROOT/tools/bootstrap-linux-ui.sh"

command -v bwrap >/dev/null
command -v import >/dev/null
IMPORT="$(command -v import)"
mkdir -p docs/vertical-slice-evidence .tools/runtime
rm -f .tools/runtime/native-storage.ron

SYSROOT="$ROOT/.tools/sysroot"
LIBS="$SYSROOT/usr/lib"
XDO="$SYSROOT/usr/bin/xdotool"
DISPLAY_NUMBER=:97
XVFB_LOG=docs/vertical-slice-evidence/native-xvfb.log
APP_LOG=docs/vertical-slice-evidence/native-runtime.log
SNAPSHOT="$ROOT/.tools/runtime/native-physical-snapshot.json"
SMOKE_TMP="$ROOT/.tools/runtime/native-x11-tmp"

mkdir -p "$SMOKE_TMP/.X11-unix"
find "$SMOKE_TMP" -mindepth 1 -maxdepth 1 ! -name '.X11-unix' -delete
find "$SMOKE_TMP/.X11-unix" -mindepth 1 -delete
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"

ui_sandbox() {
  bwrap --ro-bind / / --bind "$SMOKE_TMP" /tmp --ro-bind /usr/bin /opt \
    --ro-bind "$SYSROOT/usr/bin" /usr/bin \
    --bind "$ROOT/.tools/runtime" "$ROOT/.tools/runtime" \
    --bind "$ROOT/docs/vertical-slice-evidence" "$ROOT/docs/vertical-slice-evidence" \
    --dev-bind /dev /dev --proc /proc "$@"
}

LD_LIBRARY_PATH="$LIBS" ui_sandbox "$SYSROOT/usr/bin/Xvfb" "$DISPLAY_NUMBER" \
  -screen 0 1440x900x24 -nolisten tcp +extension GLX >"$XVFB_LOG" 2>&1 &
XVFB_PID=$!
APP_PID=""
WINDOW_ID=""
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
    POLYORAMA_TEST_SNAPSHOT_PATH="$SNAPSHOT" \
    ui_sandbox target/release/analytical-workspace-lab >>"$APP_LOG" 2>&1 &
  APP_PID=$!
  sleep 5
  kill -0 "$APP_PID"
  WINDOW_ID="$(xdo search --onlyvisible --name 'Analytical Workspace Lab' | head -n 1)"
  xdo windowfocus --sync "$WINDOW_ID"
}

xdo() {
  DISPLAY="$DISPLAY_NUMBER" LD_LIBRARY_PATH="$LIBS" ui_sandbox "$XDO" "$@"
}
capture() {
  DISPLAY="$DISPLAY_NUMBER" \
    ui_sandbox "$IMPORT" -window root "$ROOT/docs/vertical-slice-evidence/$1"
}
snapshot() {
  : >"$SNAPSHOT"
  xdo key --window "$WINDOW_ID" F12
  for _ in 1 2 3 4 5; do
    if [[ -s "$SNAPSHOT" ]]; then return; fi
    sleep 0.2
  done
  echo "native physical snapshot was not written" >&2
  exit 1
}

: >"$APP_LOG"
launch_app
capture native-default.png

# Pointer-centred zoom and a coalesced linked-camera drag.
xdo mousemove 300 300 click --repeat 3 --delay 80 4
sleep 0.5
snapshot
PAN_BEFORE="$(jq -c '.cameras[] | select(.pane == 1).camera' "$SNAPSHOT")"
LINKED_BEFORE="$(jq -c '.cameras[] | select(.pane == 2).camera' "$SNAPSHOT")"
UNDO_BEFORE="$(jq -r '.undo_depth' "$SNAPSHOT")"
xdo mousemove 300 300 mousedown 1
for step in 1 2 3 4 5 6 7 8 9 10; do
  xdo mousemove --sync "$((300 + step * 9))" "$((300 + step * 5))"
  sleep 0.03
done
xdo mouseup 1
sleep 1
capture native-linked-camera.png
snapshot
PAN_AFTER="$(jq -c '.cameras[] | select(.pane == 1).camera' "$SNAPSHOT")"
LINKED_AFTER="$(jq -c '.cameras[] | select(.pane == 2).camera' "$SNAPSHOT")"
UNDO_AFTER="$(jq -r '.undo_depth' "$SNAPSHOT")"
jq -e --argjson before "$PAN_BEFORE" --argjson undo "$UNDO_BEFORE" '
  (.cameras[] | select(.pane == 1).camera) as $primary
  | (.cameras[] | select(.pane == 2).camera) as $linked
  | $primary == $linked
    and ($primary.centre.x == ($before.centre.x - 90 * $before.pixels_per_screen_point))
    and ($primary.centre.y == ($before.centre.y - 50 * $before.pixels_per_screen_point))
    and (.undo_depth == ($undo + 1))
' "$SNAPSHOT" >/dev/null
xdo mousemove 195 18 click 1
sleep 0.5
snapshot
PAN_UNDONE="$(jq -c '.cameras[] | select(.pane == 1).camera' "$SNAPSHOT")"
LINKED_UNDONE="$(jq -c '.cameras[] | select(.pane == 2).camera' "$SNAPSHOT")"
jq -e --argjson primary "$PAN_BEFORE" --argjson linked "$LINKED_BEFORE" --argjson undo "$UNDO_BEFORE" '
  ((.cameras[] | select(.pane == 1).camera) == $primary)
  and ((.cameras[] | select(.pane == 2).camera) == $linked)
  and (.undo_depth == $undo)
' "$SNAPSHOT" >/dev/null
xdo mousemove 250 18 click 1
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
sleep 0.5
snapshot
THUMBNAILS_BEFORE="$(jq -c '.virtualisation' "$SNAPSHOT")"
THUMBNAIL_RESIDENT_BEFORE="$(jq -c '.thumbnail_resident_keys' "$SNAPSHOT")"
THUMBNAIL_FRONTIER_BEFORE="$(jq '
  [
    (.virtualisation.materialised_thumbnail_range[1] - 1),
    ([.thumbnail_resident_keys[] | select(.source == 2) | .x] | max // -1)
  ] | max
' "$SNAPSHOT")"
THUMBNAIL_OFFSET_BEFORE="$(jq -r '.virtualisation.thumbnail_scroll_offset_y' "$SNAPSHOT")"
THUMBNAIL_START_BEFORE="$(jq -r '.virtualisation.visible_thumbnails[0]' "$SNAPSHOT")"
WHEEL_EVENTS_BEFORE="$(jq -r '.physical_wheel_events' "$SNAPSHOT")"
xdo mousemove 1100 150 click --repeat 5 --delay 50 5
sleep 2
capture native-thumbnails.png
snapshot
THUMBNAILS_AFTER="$(jq -c '.virtualisation' "$SNAPSHOT")"
WHEEL_EVENTS_AFTER="$(jq -r '.physical_wheel_events' "$SNAPSHOT")"
THUMBNAIL_RESIDENT_KEYS="$(jq -c '.thumbnail_resident_keys' "$SNAPSHOT")"
VISIBLE_KEYS_AFTER="$(jq -c '.visible_tile_keys' "$SNAPSHOT")"
jq -e --argjson offset "$THUMBNAIL_OFFSET_BEFORE" --argjson start "$THUMBNAIL_START_BEFORE" --argjson wheel "$WHEEL_EVENTS_BEFORE" --argjson frontier "$THUMBNAIL_FRONTIER_BEFORE" '
  .virtualisation as $v
  | ($v.thumbnail_scroll_offset_y > $offset)
    and ($v.visible_thumbnails[0] > $start)
    and (.physical_wheel_events > $wheel)
    and ($v.materialised_thumbnails <= (($v.visible_thumbnails[1] - $v.visible_thumbnails[0]) + 4 * $v.thumbnail_columns))
    and ($v.thumbnail_cache_bytes <= (4 * 1024 * 1024))
    and (any(.visible_tile_keys[]; .source == 2 and .x >= $v.visible_thumbnails[0] and .x < $v.visible_thumbnails[1]))
    and (any(.thumbnail_resident_keys[]; .source == 2 and .x > $frontier))
' "$SNAPSHOT" >/dev/null
jq -n \
  --argjson pan_before "$PAN_BEFORE" \
  --argjson linked_before "$LINKED_BEFORE" \
  --argjson pan_after "$PAN_AFTER" \
  --argjson linked_after "$LINKED_AFTER" \
  --argjson pan_undone "$PAN_UNDONE" \
  --argjson linked_undone "$LINKED_UNDONE" \
  --argjson undo_before "$UNDO_BEFORE" \
  --argjson undo_after "$UNDO_AFTER" \
  --argjson thumbnails_before "$THUMBNAILS_BEFORE" \
  --argjson thumbnails_after "$THUMBNAILS_AFTER" \
  --argjson resident_keys_before "$THUMBNAIL_RESIDENT_BEFORE" \
  --argjson prior_frontier "$THUMBNAIL_FRONTIER_BEFORE" \
  --argjson wheel_before "$WHEEL_EVENTS_BEFORE" \
  --argjson wheel_after "$WHEEL_EVENTS_AFTER" \
  --argjson visible_keys_after "$VISIBLE_KEYS_AFTER" \
  --argjson resident_keys "$THUMBNAIL_RESIDENT_KEYS" '
  {
    physical_pan: {
      pointer_delta: {x: 90, y: 50},
      before: [$pan_before, $linked_before],
      after: [$pan_after, $linked_after],
      undo_restored: [$pan_undone, $linked_undone],
      undo_depth_before: $undo_before,
      undo_depth_after: $undo_after
    },
    thumbnail_scroll: {
      before: $thumbnails_before,
      after: $thumbnails_after,
      resident_keys_before: $resident_keys_before,
      prior_resident_or_materialised_frontier: $prior_frontier,
      physical_wheel_events_before: $wheel_before,
      physical_wheel_events_after: $wheel_after,
      visible_keys_after: $visible_keys_after,
      resident_keys: $resident_keys
    }
  }
' >docs/vertical-slice-evidence/native-semantic.json

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
