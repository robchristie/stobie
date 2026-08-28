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
WINDOW_WIDTH=""
WINDOW_HEIGHT=""
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
  WINDOW_WIDTH="$(xdo getwindowgeometry --shell "$WINDOW_ID" | sed -n 's/^WIDTH=//p')"
  WINDOW_HEIGHT="$(xdo getwindowgeometry --shell "$WINDOW_ID" | sed -n 's/^HEIGHT=//p')"
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

target_point() {
  local kind="$1"
  local pane="$2"
  local name="$3"
  local fraction_x="${4:-0.5}"
  local fraction_y="${5:-0.5}"
  local delta_x="${6:-0}"
  local delta_y="${7:-0}"
  jq -er \
    --arg kind "$kind" \
    --arg name "$name" \
    --argjson pane "$pane" \
    --argjson fraction_x "$fraction_x" \
    --argjson fraction_y "$fraction_y" \
    --argjson delta_x "$delta_x" \
    --argjson delta_y "$delta_y" \
    --argjson window_width "$WINDOW_WIDTH" \
    --argjson window_height "$WINDOW_HEIGHT" '
      .ui_geometry as $geometry
      | (if $kind == "control" then
           $geometry.controls[] | select(.name == $name and ($pane == 0 or .pane == $pane))
         elif $kind == "splitter" then
           $geometry.splitters[] | select(.node == $pane)
         elif $kind == "thumbnail_scroll" or $kind == "results_scroll" then
           {rect: $geometry[$kind]}
         elif $kind == "rightmost_pane_body" then
           $geometry.pane_bodies | max_by(.rect.min_x)
         elif $kind == "first_result_row" then
           $geometry.result_rows[0]
         else
           $geometry[$kind][] | select(.pane == $pane)
         end) as $target
      | $geometry.root as $root
      | $target.rect as $rect
      | select($root != null and $rect != null)
      | select($rect.max_x > $rect.min_x and $rect.max_y > $rect.min_y)
      | [
          (((($rect.min_x + ($rect.max_x - $rect.min_x) * $fraction_x + $delta_x) - $root.min_x)
            * $window_width / ($root.max_x - $root.min_x)) | round),
          (((($rect.min_y + ($rect.max_y - $rect.min_y) * $fraction_y + $delta_y) - $root.min_y)
            * $window_height / ($root.max_y - $root.min_y)) | round)
        ]
      | select(.[0] >= 0 and .[0] <= $window_width and .[1] >= 0 and .[1] <= $window_height)
      | @tsv
    ' "$SNAPSHOT"
}

move_target() {
  local point
  point="$(target_point "$@")"
  local x="${point%%$'\t'*}"
  local y="${point#*$'\t'}"
  xdo mousemove --window "$WINDOW_ID" "$x" "$y"
}

: >"$APP_LOG"
launch_app
capture native-default.png
snapshot
UI_GEOMETRY_INITIAL="$(jq -c '.ui_geometry' "$SNAPSHOT")"

# Pointer-centred zoom and a coalesced linked-camera drag.
move_target image_viewports 1 "" 0.35 0.45
xdo click --repeat 3 --delay 80 4
sleep 0.5
snapshot
PAN_BEFORE="$(jq -c '.cameras[] | select(.pane == 1).camera' "$SNAPSHOT")"
LINKED_BEFORE="$(jq -c '.cameras[] | select(.pane == 2).camera' "$SNAPSHOT")"
UNDO_BEFORE="$(jq -r '.undo_depth' "$SNAPSHOT")"
move_target image_viewports 1 "" 0.35 0.45
xdo mousedown 1
for step in 1 2 3 4 5 6 7 8 9 10; do
  move_target image_viewports 1 "" 0.35 0.45 "$((step * 9))" "$((step * 5))"
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
move_target control 0 undo
xdo click 1
sleep 0.5
snapshot
PAN_UNDONE="$(jq -c '.cameras[] | select(.pane == 1).camera' "$SNAPSHOT")"
LINKED_UNDONE="$(jq -c '.cameras[] | select(.pane == 2).camera' "$SNAPSHOT")"
jq -e --argjson primary "$PAN_BEFORE" --argjson linked "$LINKED_BEFORE" --argjson undo "$UNDO_BEFORE" '
  ((.cameras[] | select(.pane == 1).camera) == $primary)
  and ((.cameras[] | select(.pane == 2).camera) == $linked)
  and (.undo_depth == $undo)
' "$SNAPSHOT" >/dev/null
move_target control 0 redo
xdo click 1
move_target control 1 fit
xdo click 1
sleep 1

# Construct, commit, edit, undo and redo a world-coordinate polygon.
move_target control 1 polygon
xdo click 1
sleep 1
move_target image_viewports 1 "" 0.2 0.2
xdo click 1
move_target image_viewports 1 "" 0.65 0.25
xdo click 1
move_target image_viewports 1 "" 0.4 0.65
xdo click 1
sleep 1
xdo click 3
sleep 2
capture native-polygon.png
snapshot
move_target control 1 edit_vertex
xdo click 1
move_target image_viewports 1 "" 0.2 0.2
xdo mousedown 1
move_target image_viewports 1 "" 0.2 0.2 35 25
xdo mouseup 1
move_target control 0 undo
xdo click 1
move_target control 0 redo
xdo click 1
xdo key Delete
move_target control 0 undo
xdo click 1
move_target control 0 redo
xdo click 1
move_target control 0 undo
xdo click 1

# Result selection/recentring and progressive thumbnail virtualisation.
move_target first_result_row 0 ""
xdo click 1
sleep 0.5
snapshot
move_target control 5 recenter_primary
xdo click 1
move_target tabs 6 ""
xdo click 1
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
move_target thumbnail_scroll 0 ""
xdo click --repeat 5 --delay 50 5
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
  --argjson resident_keys "$THUMBNAIL_RESIDENT_KEYS" \
  --argjson ui_geometry_initial "$UI_GEOMETRY_INITIAL" '
  {
    ui_geometry: $ui_geometry_initial,
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
move_target tabs 8 ""
xdo click 1
sleep 0.5
snapshot
move_target pane_bodies 8 "" 0.8 0.75
xdo click --repeat 6 --delay 50 5
sleep 1
capture native-diagnostics.png
SPLITTER_HASH_BEFORE="$(jq -r '.workspace_hash' "$SNAPSHOT")"
SPLITTER_UNDO_BEFORE="$(jq -r '.undo_depth' "$SNAPSHOT")"
SPLITTER_X_BEFORE="$(jq -r '.ui_geometry.splitters[] | select(.node == 1) | ((.rect.min_x + .rect.max_x) * 0.5)' "$SNAPSHOT")"
move_target splitter 1 "" 0.5 0.25
xdo mousedown 1
move_target splitter 1 "" 0.5 0.25 -47 0
xdo mouseup 1
sleep 0.5
snapshot
SPLITTER_HASH_AFTER="$(jq -r '.workspace_hash' "$SNAPSHOT")"
SPLITTER_UNDO_AFTER="$(jq -r '.undo_depth' "$SNAPSHOT")"
SPLITTER_X_AFTER="$(jq -r '.ui_geometry.splitters[] | select(.node == 1) | ((.rect.min_x + .rect.max_x) * 0.5)' "$SNAPSHOT")"
jq -ne \
  --arg before_hash "$SPLITTER_HASH_BEFORE" \
  --arg after_hash "$SPLITTER_HASH_AFTER" \
  --argjson undo_before "$SPLITTER_UNDO_BEFORE" \
  --argjson undo_after "$SPLITTER_UNDO_AFTER" \
  --argjson x_before "$SPLITTER_X_BEFORE" \
  --argjson x_after "$SPLITTER_X_AFTER" '
    ($after_hash != $before_hash)
      and ($undo_after == ($undo_before + 1))
      and (($x_after - ($x_before - 47)) > -1)
      and (($x_after - ($x_before - 47)) < 1)
  ' >/dev/null
move_target control 0 undo
xdo click 1
sleep 0.5
snapshot
test "$(jq -r '.workspace_hash' "$SNAPSHOT")" = "$SPLITTER_HASH_BEFORE"
move_target control 0 redo
xdo click 1
sleep 0.5
snapshot
test "$(jq -r '.workspace_hash' "$SNAPSHOT")" = "$SPLITTER_HASH_AFTER"

SPLITTER_NO_OP_HASH="$(jq -r '.workspace_hash' "$SNAPSHOT")"
SPLITTER_NO_OP_UNDO="$(jq -r '.undo_depth' "$SNAPSHOT")"
move_target splitter 1 "" 0.5 0.25
xdo mousedown 1
move_target splitter 1 "" 0.5 0.25 -30 0
move_target splitter 1 "" 0.5 0.25
xdo mouseup 1
sleep 0.5
snapshot
test "$(jq -r '.workspace_hash' "$SNAPSHOT")" = "$SPLITTER_NO_OP_HASH"
test "$(jq -r '.undo_depth' "$SNAPSHOT")" = "$SPLITTER_NO_OP_UNDO"

SEMANTIC_UPDATE="$ROOT/.tools/runtime/native-semantic-update.json"
jq \
  --arg before_hash "$SPLITTER_HASH_BEFORE" \
  --arg after_hash "$SPLITTER_HASH_AFTER" \
  --argjson undo_before "$SPLITTER_UNDO_BEFORE" \
  --argjson undo_after "$SPLITTER_UNDO_AFTER" \
  --argjson x_before "$SPLITTER_X_BEFORE" \
  --argjson x_after "$SPLITTER_X_AFTER" '
    . + {
      physical_splitter_resize: {
        pointer_delta_x: -47,
        splitter_centre_before: $x_before,
        splitter_centre_after: $x_after,
        workspace_hash_before: $before_hash,
        workspace_hash_after: $after_hash,
        undo_depth_before: $undo_before,
        undo_depth_after: $undo_after,
        undo_restored_original: true,
        redo_restored_resize: true,
        out_and_back_no_op: true
      }
    }
  ' docs/vertical-slice-evidence/native-semantic.json >"$SEMANTIC_UPDATE"
mv "$SEMANTIC_UPDATE" docs/vertical-slice-evidence/native-semantic.json

move_target tabs 4 ""
xdo mousedown 1
sleep 1
move_target rightmost_pane_body 0 "" 0.5 0.25
sleep 1
xdo mouseup 1
sleep 2
capture native-rearranged-dock.png
move_target control 0 save_layout
xdo click 1
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
