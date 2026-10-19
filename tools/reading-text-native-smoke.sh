#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${STOBIE_EVIDENCE_DIR:-$ROOT/.tools/runtime/reading-text-evidence}/reading-text-native"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/reading-text-native-x11-XXXXXX")"
mkdir -p "$SMOKE_TMP/.X11-unix"
chmod 1777 "$SMOKE_TMP" "$SMOKE_TMP/.X11-unix"
if [[ "${STOBIE_USE_SYSTEM_UI_LIBS:-0}" == 1 ]]; then
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
export DISPLAY="${STOBIE_READING_TEXT_DISPLAY:-:97}" WGPU_BACKEND=gl
SNAPSHOT="$ROOT/.tools/runtime/reading-text-native-snapshot.json"
OPENED="$SMOKE_TMP/opened.txt"
rm -f "$SNAPSHOT"
# The platform's default browser, as eframe asks for it: the BROWSER variable
# names this recorder, so a link opens nothing but leaves its URL behind.
RECORDER="$SMOKE_TMP/record-browser"
printf '#!/bin/sh\nprintf "%%s\\n" "$1" >> "%s"\n' "$OPENED" > "$RECORDER"
chmod +x "$RECORDER"
owned_start XVFB_PID "$XVFB" "$DISPLAY" -screen 0 1440x900x24 -nolisten tcp +extension GLX >"$EVIDENCE_DIR/native-xvfb.log" 2>&1
sleep 1
BROWSER="$RECORDER" STOBIE_GALLERY_STORY="reading/conversation" STOBIE_GALLERY_SNAPSHOT_PATH="$SNAPSHOT" \
  STOBIE_GALLERY_SNAPSHOT_CONTINUOUS=1 \
  owned_start APP_PID target/release/stobie-gallery >"$EVIDENCE_DIR/native-runtime.log" 2>&1
ui_sandbox "$(command -v python3)" - "$SNAPSHOT" "$XDO" "$IMPORT" "$EVIDENCE_DIR" "$OPENED" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

snapshot_path, xdotool, imagemagick, evidence, opened = sys.argv[1:]
evidence, opened = Path(evidence), Path(opened)
steps, targets = [], []
LINK = ("Flat-memory attention for long documents", "https://example.org/papers/flat-memory-attention")

def xdo(*arguments):
    return subprocess.check_output([xdotool, *map(str, arguments)], text=True).strip()

def snapshot():
    return json.loads(Path(snapshot_path).read_text())

def wait(predicate, description, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            state = snapshot()
            if predicate(state):
                return state
        except (OSError, json.JSONDecodeError):
            pass
        time.sleep(0.03)
    raise AssertionError(f"native reading-text timeout: {description}")

state = wait(lambda s: s["frame"] > 0 and s["story"] == "reading/conversation", "first reading frame")
deadline = time.monotonic() + 10
while time.monotonic() < deadline:
    try:
        window = xdo("search", "--onlyvisible", "--name", "Stobie Component Gallery").splitlines()[0]
        break
    except (subprocess.CalledProcessError, IndexError):
        time.sleep(0.05)
else:
    raise AssertionError("native Gallery did not expose a visible window")
xdo("windowfocus", "--sync", window)

def to_window(state, x, y):
    root = next(n for n in state["ui_snapshot"]["nodes"] if n["id"] == state["ui_snapshot"]["root"])["rect"]
    scale = state["ui_snapshot"]["pixels_per_point"]
    return [round((x - root["min_x"]) * scale), round((y - root["min_y"]) * scale)]

def record(name):
    state = snapshot()
    assert state["text_audit"] == [], name
    assert state["ui_snapshot"]["semantic_audit"] == [], name
    assert state["text_audit_coverage"]["failed_components"] == 0
    steps.append({"name": name, "state": state})
    return state

try:
    state = record("ordinary")
    reading = [t for t in state["text"] if t["component_id"]["kind"] == "reading_text"]
    assert len(reading) == 3 and max(t["line_count"] for t in reading) > 8, reading
    reply = max(reading, key=lambda t: t["line_count"])["allocated_rect"]
    start = to_window(state, reply["min_x"] + 1, reply["min_y"] + 4)
    end = to_window(state, reply["max_x"] - 1, reply["max_y"] - 4)
    xdo("mousemove", "--window", window, *start)
    xdo("mousedown", "1")
    xdo("mousemove", "--window", window, *end)
    xdo("mouseup", "1")
    xdo("key", "ctrl+c")
    targets.append({"name": "reply", "rect": reply, "start": start, "end": end})
    record("selection-copy-input")
    link = next(n for n in snapshot()["ui_snapshot"]["nodes"] if n["role"] == "link" and n["name"] == LINK[0])
    assert link["value"] == LINK[1]
    r = link["rect"]
    point = to_window(snapshot(), (r["min_x"] + r["max_x"]) / 2, (r["min_y"] + r["max_y"]) / 2)
    targets.append({"name": LINK[0], "node_id": link["id"], "rect": r, "point": point})
    xdo("mousemove", "--window", window, *point)
    record("link-hover")
    assert not opened.exists(), "hovering opens nothing"
    xdo("click", "1")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not (opened.exists() and opened.read_text().strip()):
        time.sleep(0.05)
    urls = opened.read_text().split() if opened.exists() else []
    assert urls == [LINK[1]], f"the platform's default browser received {urls}"
    record("link-opened")
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
    capture = evidence / "native-reading-text.png"
    subprocess.run([imagemagick, "-window", window, str(capture)], check=True)
    image = capture.read_bytes()
    width, height = struct.unpack(">II", image[16:24])
    report = {"source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
              "binary_sha256": hashlib.sha256(Path("target/release/stobie-gallery").read_bytes()).hexdigest(),
              "story": state["story"], "configuration": state["configuration"], "display": os.environ["DISPLAY"],
              "backend": "GL/llvmpipe on Xvfb", "steps": steps, "targets": targets, "opened": urls, "idle": idle,
              "image": {"path": capture.name, "width": width, "height": height, "sha256": hashlib.sha256(image).hexdigest()},
              "input_route": "xdotool pointer and keyboard; continuous snapshot publication reads state only; BROWSER records the default-browser request"}
    (evidence / "native-workflow.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Reading-text native smoke passed: {state['story']}, selection input, link opened in the default browser, empty audits and idle")
except BaseException as error:
    (evidence / "native-failure-report.json").write_text(json.dumps({"error": str(error), "steps": steps, "targets": targets}, indent=2) + "\n")
    subprocess.run([imagemagick, "-window", window, str(evidence / "native-failure.png")], check=False)
    raise
PY
source "$ROOT/tools/native-runtime-errors.sh"
assert_clean_native_runtime_log "$EVIDENCE_DIR/native-runtime.log" "native reading-text smoke"
