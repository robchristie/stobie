#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
bash tools/bootstrap-linux-ui.sh
EVIDENCE_DIR="${POLYORAMA_EVIDENCE_DIR:-$ROOT/.tools/runtime/icons-evidence}/icons-native"
mkdir -p "$EVIDENCE_DIR" .tools/runtime
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"
SYSROOT="$ROOT/.tools/sysroot"
SMOKE_TMP="$(mktemp -d "$ROOT/.tools/runtime/icons-native-x11-XXXXXX")"
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
export DISPLAY="${POLYORAMA_ICON_DISPLAY:-:94}" WGPU_BACKEND=gl
STORY="${POLYORAMA_GALLERY_STORY:-icons/action-presentations}"
SNAPSHOT="$ROOT/.tools/runtime/icons-native-snapshot.json"
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
    raise AssertionError(f"native icon timeout: {description}")

state = wait(lambda s: s["frame"] > 0 and s["icon_fixture"]["targets"], "first icon frame")
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

def node(control):
    state = snapshot()
    target = state["icon_fixture"]["targets"][control]
    current = next(n for n in state["ui_snapshot"]["nodes"] if n["id"] == target)
    root = next(n for n in state["ui_snapshot"]["nodes"] if n["id"] == state["ui_snapshot"]["root"])
    r, rr = current["rect"], root["rect"]
    scale = state["ui_snapshot"]["pixels_per_point"]
    point = [round(((r["min_x"] + r["max_x"]) / 2 - rr["min_x"]) * scale),
             round(((r["min_y"] + r["max_y"]) / 2 - rr["min_y"]) * scale)]
    assert r["max_x"] > r["min_x"] and r["max_y"] > r["min_y"], control
    targets.append({"control": control, "node_id": target, "frame": state["frame"], "rect": r, "point": point})
    return current, point

def move(control):
    _, point = node(control)
    xdo("mousemove", "--window", window, *point)

def click(control):
    before = snapshot()["frame"]
    move(control)
    xdo("click", "1")
    wait(lambda s: s["frame"] > before, f"click frame {control}")

def fixture(**expected):
    return wait(lambda s: all(s["icon_fixture"][key] == value for key, value in expected.items()), str(expected))

def count(control, expected):
    return wait(lambda s: s["icon_fixture"]["activations"][control] == expected, f"{control} count {expected}")

def focus(control):
    for _ in range(40):
        state = snapshot()
        if any(n["id"] == state["icon_fixture"]["targets"][control] and n["focused"] for n in state["ui_snapshot"]["nodes"]):
            return
        xdo("key", "Tab")
        time.sleep(0.06)
    raise AssertionError(f"keyboard did not reach {control}")

def record(name):
    state = snapshot()
    assert state["text_audit"] == [], name
    assert state["ui_snapshot"]["semantic_audit"] == [], name
    steps.append({"name": name, "state": state})
    return state

try:
    record("ordinary")
    if state["story"] == "icons/action-presentations":
        disabled, _ = node("disabled")
        assert not disabled["enabled"] and disabled["disabled_reason"] == "History is empty"
        click("text")
        count("text", 1)
        move("icon")
        fixture(hovered="icon")
        record("hover")
        xdo("mousedown", "1")
        fixture(pointer_down="icon")
        record("pressed")
        xdo("mouseup", "1")
        count("icon", 1)
        fixture(pointer_down=None)
        click("toggle")
        count("toggle", 1)
        fixture(linked=False)
        click("disabled")
        count("disabled", 0)
        record("toggle-disabled")
        click("text")
        count("text", 2)
        xdo("mousemove", "--window", window, 1400, 880)
        focus("icon")
        record("keyboard-focus")
        xdo("key", "Return")
        count("icon", 2)
        focus("labelled")
        xdo("key", "space")
        count("labelled", 1)
        focus("toggle")
        xdo("key", "Return")
        count("toggle", 2)
        fixture(linked=True)
        record("keyboard-activation")
    elif state["story"] == "icons/toolbar":
        click("polygon")
        count("polygon", 1)
        fixture(selected_tool="polygon")
        click("labelled")
        count("labelled", 1)
        focus("icon")
        xdo("key", "Return")
        count("icon", 1)
        record("toolbar-activation")
    elif state["story"] == "icons/long-narrow":
        labels = [n for n in state["ui_snapshot"]["nodes"] if "export_selection" in n["actions"] and n["name"] == "Export the selected annotations and their complete supporting evidence"]
        assert len(labels) == 2
        click("export")
        count("export", 1)
        focus("labelled")
        xdo("key", "space")
        count("labelled", 1)
        record("long-narrow-activation")
    else:
        raise AssertionError(f"unsupported physical icon story: {state['story']}")
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
    capture = evidence / "native-actions.png"
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
    print(f"Typed icon native smoke passed: {state['story']}, pointer/keyboard, empty audits and idle")
except BaseException as error:
    (evidence / "native-failure-report.json").write_text(json.dumps({"error": str(error), "steps": steps, "targets": targets}, indent=2) + "\n")
    subprocess.run([imagemagick, "-window", window, str(evidence / "native-failure.png")], check=False)
    raise
PY
if rg 'panicked|WGPU error|Exiting because of error' "$EVIDENCE_DIR/native-runtime.log"; then
  echo 'native icon smoke observed an application failure' >&2
  exit 1
fi
