# Runtime-log guards must reject scanner failures as well as matched errors.
assert_clean_native_runtime_log() {
  local log_path="$1"
  local scenario="$2"
  local scan_status
  if rg 'panicked|WGPU error|Exiting because of error' "$log_path"; then
    echo "$scenario observed an application failure" >&2
    return 1
  else
    scan_status=$?
  fi
  if [[ "$scan_status" != 1 ]]; then
    echo "$scenario runtime log scan failed ($scan_status): $log_path" >&2
    return 1
  fi
}
