#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${POLYORAMA_EVIDENCE_DIR:-$ROOT/.tools/runtime/status-chip-evidence}/status-chip-native"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/status-chip-native-x11-XXXXXX")"
mkdir -p "$SMOKE_TMP/.X11-unix"
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
if [[ "${POLYORAMA_USE_SYSTEM_UI_LIBS:-0}" == 1 ]]; then
  XVFB="$(command -v Xvfb)"
  XDO="$(command -v xdotool)"
else
  export LD_LIBRARY_PATH="$SYSROOT/usr/lib"
  XVFB="$SYSROOT/usr/bin/Xvfb"
  XDO="$SYSROOT/usr/bin/xdotool"
fi
IMPORT="$(command -v import)"
source tools/native-smoke-lifecycle.sh
trap 'cleanup; rm -rf -- "$SMOKE_TMP"' EXIT
export DISPLAY="${POLYORAMA_STATUS_CHIP_DISPLAY:-:94}" WGPU_BACKEND=gl
STORY="${POLYORAMA_GALLERY_STORY:-status-chip/tasks}"
SNAPSHOT="$ROOT/.tools/runtime/status-chip-native-snapshot.json"
rm -f "$SNAPSHOT"
owned_start XVFB_PID "$XVFB" "$DISPLAY" -screen 0 1440x900x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/native-xvfb.log" 2>&1
sleep 1
POLYORAMA_GALLERY_STORY="$STORY" POLYORAMA_GALLERY_SNAPSHOT_PATH="$SNAPSHOT" \
  POLYORAMA_GALLERY_SNAPSHOT_CONTINUOUS=1 \
  owned_start APP_PID target/release/polyorama-gallery >"$EVIDENCE_DIR/native-runtime.log" 2>&1
ui_sandbox "$(command -v python3)" - "$SNAPSHOT" "$XDO" "$IMPORT" "$EVIDENCE_DIR" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

snapshot_path, xdotool, imagemagick, evidence = sys.argv[1:]
evidence = Path(evidence)
steps, targets = [], []

def xdo(*arguments):
    return subprocess.check_output([xdotool, *map(str, arguments)], text=True).strip()

def snapshot():
    return json.loads(Path(snapshot_path).read_text())

def wait(predicate, description):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        try:
            state = snapshot()
            if predicate(state):
                return state
        except (OSError, json.JSONDecodeError):
            pass
        time.sleep(0.03)
    raise AssertionError(f"native status-chip timeout: {description}")

state = wait(lambda s: s["frame"] > 0 and s["status_chip_fixture"] is not None, "first status-chip frame")
deadline = time.monotonic() + 10
while time.monotonic() < deadline:
    try:
        window = xdo("search", "--onlyvisible", "--name", "Polyorama Component Gallery").splitlines()[0]
        break
    except (subprocess.CalledProcessError, IndexError):
        time.sleep(0.05)
else:
    raise AssertionError("native Gallery did not expose a visible window")
xdo("windowfocus", "--sync", window)

def node(name, role="status_chip", domain=None):
    state = snapshot()
    current = next(n for n in state["ui_snapshot"]["nodes"] if n["name"] == name and n["role"] == role and (domain is None or (n["domain_reference"] or {}).get("value", {}).get("id") == domain))
    root = next(n for n in state["ui_snapshot"]["nodes"] if n["id"] == state["ui_snapshot"]["root"])
    r, rr = current["rect"], root["rect"]
    scale = state["ui_snapshot"]["pixels_per_point"]
    point = [round(((r["min_x"] + r["max_x"]) / 2 - rr["min_x"]) * scale),
             round(((r["min_y"] + r["max_y"]) / 2 - rr["min_y"]) * scale)]
    targets.append({"name": name, "node_id": current["id"], "frame": state["frame"], "rect": r, "point": point})
    return current, point

def record(name):
    state = snapshot()
    assert state["text_audit"] == [], name
    assert state["ui_snapshot"]["semantic_audit"] == [], name
    assert state["text_audit_coverage"]["failed_components"] == 0
    steps.append({"name": name, "state": state})
    return state

try:
    record("ordinary")
    standalone, point = node("Active")
    assert standalone["text_selectable"]
    rect = standalone["rect"]
    scale = state["ui_snapshot"]["pixels_per_point"]
    xdo("mousemove", "--window", window, round((rect["min_x"] + 0.5) * scale), point[1])
    xdo("mousedown", "1")
    xdo("mousemove", "--window", window, round((rect["max_x"] - 0.5) * scale), point[1])
    xdo("mouseup", "1")
    xdo("key", "ctrl+c")
    record("selection-copy-input")
    inert, point = node("Scheduled", domain="2")
    assert not inert.get("text_selectable", False)
    xdo("mousemove", "--window", window, *point)
    wait(lambda s: s["status_chip_fixture"]["hovered_task"] == 2, "inert row hover")
    record("row-hover")
    xdo("click", "1")
    wait(lambda s: s["status_chip_fixture"]["selected_task"] == 2 and s["status_chip_fixture"]["activations"] == 1, "inert chip activates parent")
    record("row-selected")
    parent, _ = node("Survey northern site", "result_row")
    assert parent["focused"]
    xdo("key", "Return")
    wait(lambda s: s["status_chip_fixture"]["activations"] == 2, "parent Enter activation")
    record("parent-keyboard")
    _, point = node("Active", domain="1")
    xdo("mousemove", "--window", window, *point)
    xdo("click", "1")
    wait(lambda s: s["status_chip_fixture"]["selected_task"] == 1, "return to first task")
    _, point = node("Complete demo task", "button")
    xdo("mousemove", "--window", window, *point)
    xdo("click", "1")
    wait(lambda s: s["status_chip_fixture"]["completed"], "consumer status update")
    current = record("updated")
    completed, _ = node("Completed")
    assert completed["id"] == standalone["id"]
    assert not any(n["role"] == "status_chip" and n["name"] == "Active" for n in current["ui_snapshot"]["nodes"])
    xdo("mousemove", "--window", window, 1400, 880)
    initial = snapshot()["frame"]
    previous, quiet = initial, 0
    for sample in range(1, 36):
        time.sleep(0.1)
        current = snapshot()["frame"]
        quiet = quiet + 1 if current == previous else 0
        previous = current
        if quiet == 7:
            time.sleep(0.7)
            final = snapshot()["frame"]
            assert final == current, f"idle repainted after settling: {current} -> {final}"
            idle = {"settle_initial_frame": initial, "settle_final_frame": current,
                    "settle_observed_ms": sample * 100, "frame_before": current,
                    "frame_after": final, "idle_observed_ms": 700}
            break
    else:
        raise AssertionError(f"native Gallery did not settle: {initial} -> {previous}")
    capture = evidence / "native-status-chips.png"
    subprocess.run([imagemagick, "-window", window, str(capture)], check=True)
    image = capture.read_bytes()
    width, height = struct.unpack(">II", image[16:24])
    report = {"source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
              "binary_sha256": hashlib.sha256(Path("target/release/polyorama-gallery").read_bytes()).hexdigest(),
              "story": state["story"], "configuration": state["configuration"], "display": os.environ["DISPLAY"],
              "backend": "GL/llvmpipe on Xvfb", "steps": steps, "targets": targets, "idle": idle,
              "image": {"path": capture.name, "width": width, "height": height, "sha256": hashlib.sha256(image).hexdigest()},
              "input_route": "xdotool pointer and keyboard; continuous snapshot publication reads state only"}
    (evidence / "native-workflow.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Status-chip native smoke passed: {state['story']}, pointer/keyboard, empty audits and idle")
except BaseException as error:
    (evidence / "native-failure-report.json").write_text(json.dumps({"error": str(error), "steps": steps, "targets": targets}, indent=2) + "\n")
    subprocess.run([imagemagick, "-window", window, str(evidence / "native-failure.png")], check=False)
    raise
PY
if rg 'panicked|WGPU error|Exiting because of error' "$EVIDENCE_DIR/native-runtime.log"; then
  echo 'native status-chip smoke observed an application failure' >&2
  exit 1
fi
