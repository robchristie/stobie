# Shared owned background-process lifecycle for the native smoke scripts.
# Source after ROOT, SYSROOT, SMOKE_TMP and EVIDENCE_DIR have been set (and
# XVFB before owned_start_xvfb).
UI_COMMAND=()
if [[ "${STOBIE_USE_SYSTEM_UI_LIBS:-0}" != "1" ]]; then
  # The sandbox sees the host read-only and has a private /tmp, so a host
  # TMPDIR, inside the checkout or under the host's /tmp, is read-only or
  # missing there; temporary files go to the private /tmp instead.
  UI_COMMAND=(bwrap --die-with-parent --ro-bind / / --bind "$SMOKE_TMP" /tmp --setenv TMPDIR /tmp
    --ro-bind /usr/bin /opt --ro-bind "$SYSROOT/usr/bin" /usr/bin
    --bind "$ROOT/.tools/runtime" "$ROOT/.tools/runtime"
    --bind "$EVIDENCE_DIR" "$EVIDENCE_DIR" --dev-bind /dev /dev --proc /proc)
fi
# The directories Cargo builds this workspace and Record Desk into, from
# CARGO_TARGET_DIR, Cargo configuration or each workspace's own target/. They
# are resolved here because the sandbox has no cargo, and exported for the
# smokes it runs.
if [[ -z "${STOBIE_TARGET_DIR:-}" ]]; then
  STOBIE_TARGET_DIR="$(python3 "$ROOT/tools/target_directory.py")"
fi
if [[ -z "${STOBIE_RECORD_DESK_TARGET_DIR:-}" ]]; then
  STOBIE_RECORD_DESK_TARGET_DIR="$(python3 "$ROOT/tools/target_directory.py" \
    --manifest-path "$ROOT/consumers/record-desk/Cargo.toml")"
fi
export STOBIE_TARGET_DIR STOBIE_RECORD_DESK_TARGET_DIR
ui_sandbox() {
  "${UI_COMMAND[@]}" "$@"
}
owned_start() {
  local variable="$1"
  shift
  python3 "$ROOT/tools/owned-process.py" "${UI_COMMAND[@]}" "$@" &
  printf -v "$variable" '%s' "$!"
}
# Start Xvfb (from $XVFB) with the given options on a display it finds free,
# set XVFB_PID and export DISPLAY once the server accepts connections. Xvfb's
# -displayfd tries successive display numbers, so concurrent runs on one host
# never share one; STOBIE_XVFB_DISPLAY (such as :99) names a fixed display
# instead. The server's output goes to the log named first.
owned_start_xvfb() {
  local log="$1" ready="$SMOKE_TMP/xvfb-display" display=() started=$SECONDS number
  shift
  if [[ -n "${STOBIE_XVFB_DISPLAY:-}" ]]; then display=("$STOBIE_XVFB_DISPLAY"); fi
  : >"$ready"
  python3 "$ROOT/tools/owned-process.py" --pass-fd 3 "${UI_COMMAND[@]}" \
    "$XVFB" "${display[@]}" -displayfd 3 "$@" 3>"$ready" >"$log" 2>&1 &
  XVFB_PID=$!
  while (( SECONDS - started < 30 )); do
    # Xvfb writes the number and a newline once it accepts connections.
    if [[ "$(wc -l <"$ready")" -ge 1 ]]; then
      number="$(head -n 1 "$ready")"
      [[ "$number" =~ ^[0-9]+$ ]] || { echo "Xvfb reported display \"$number\"" >&2; return 1; }
      export DISPLAY=":$number"
      return
    fi
    if ! kill -0 "$XVFB_PID" 2>/dev/null; then
      echo "Xvfb exited before it accepted connections; its log ($log):" >&2
      tail -n 20 "$log" >&2 || true
      return 1
    fi
    sleep 0.05
  done
  echo "Xvfb did not report a display within 30s; its log is $log" >&2
  return 1
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
