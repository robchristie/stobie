# Shared owned background-process lifecycle for the native smoke scripts.
# Source after ROOT, SYSROOT, SMOKE_TMP and EVIDENCE_DIR have been set.
UI_COMMAND=()
if [[ "${POLYORAMA_USE_SYSTEM_UI_LIBS:-0}" != "1" ]]; then
  UI_COMMAND=(bwrap --die-with-parent --ro-bind / / --bind "$SMOKE_TMP" /tmp
    --ro-bind /usr/bin /opt --ro-bind "$SYSROOT/usr/bin" /usr/bin
    --bind "$ROOT/.tools/runtime" "$ROOT/.tools/runtime"
    --bind "$EVIDENCE_DIR" "$EVIDENCE_DIR" --dev-bind /dev /dev --proc /proc)
fi
ui_sandbox() {
  "${UI_COMMAND[@]}" "$@"
}
owned_start() {
  local variable="$1"
  shift
  python3 "$ROOT/tools/owned-process.py" "${UI_COMMAND[@]}" "$@" &
  printf -v "$variable" '%s' "$!"
}
owned_stop() {
  local pid="${!1}"
  if [[ -n "$pid" ]]; then
    kill -TERM "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    printf -v "$1" '%s' ''
  fi
}
owned_wait_window() {
  local window_title="$1" window_owner="$2" window_wait_limit="${3:-30}"
  local window_wait_started=$SECONDS window_ids
  [[ "$window_wait_limit" =~ ^[0-9]+$ ]] || return 2
  while (( SECONDS - window_wait_started < window_wait_limit )); do
    if ! kill -0 "$window_owner" 2>/dev/null; then
      echo "application exited before window appeared: $window_title" >&2
      return 1
    fi
    if window_ids="$(xdo search --onlyvisible --name "$window_title")"; then
      printf '%s\n' "${window_ids%%$'\n'*}"
      return
    fi
    sleep 0.1
  done
  echo "window did not appear within ${window_wait_limit}s: $window_title" >&2
  return 1
}
APP_PID=""
XVFB_PID=""
cleanup() {
  owned_stop APP_PID
  owned_stop XVFB_PID
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
