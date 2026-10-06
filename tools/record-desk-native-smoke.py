#!/usr/bin/env python3
"""OS pointer/keyboard journey against read-only current native observations."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = Path(sys.argv[1]).resolve()
XDO, IMPORT = sys.argv[2:4]
BINARY = ROOT / "consumers/record-desk/target/release/record-desk"
SNAPSHOT = EVIDENCE / "native-current.json"
STATE = EVIDENCE / "native-state.json"
STEPS = []
app = None
window = None
runtime = None


def xdo(*args):
    return subprocess.check_output([XDO, *map(str, args)], text=True).strip()


def read():
    try:
        return json.loads(SNAPSHOT.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def wait(predicate):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if app.poll() is not None:
            raise RuntimeError(f"Record Desk exited {app.returncode}; inspect {runtime.name}")
        state = read()
        if state and predicate(state):
            return state
        time.sleep(0.05)
    latest = read() or {}
    raise AssertionError(f"Native observation timed out: { {k: latest.get(k) for k in ('filters', 'selected', 'undo_entries', 'draft_dirty', 'message')} }")


def record(name):
    state = wait(lambda s: bool(s["ui"]["nodes"]))
    assert not state["ui"]["text_audit"], (name, state["ui"]["text_audit"])
    assert not state["ui"]["semantic_audit"], (name, state["ui"]["semantic_audit"])
    (EVIDENCE / f"native-{name}.json").write_text(json.dumps(state, indent=2))
    STEPS.append({"name": name, "state": state})


def key(sequence):
    xdo("key", "--clearmodifiers", sequence)


def click(identity):
    state = wait(lambda s: any(n["enabled"] and (n["id"] == identity or identity in n["actions"]) for n in s["ui"]["nodes"]))
    node = next(n for n in state["ui"]["nodes"] if n["id"] == identity or identity in n["actions"])
    rect = node["rect"]
    xdo("mousemove", "--window", window, int((rect["min_x"] + rect["max_x"]) / 2), int((rect["min_y"] + rect["max_y"]) / 2))
    xdo("click", 1)


def focus(identity):
    for _ in range(24):
        state = wait(lambda s: True)
        if any(n["id"] == identity and n["focused"] for n in state["ui"]["nodes"]):
            return
        frame = state["ui"]["frame"]
        key("Tab")
        wait(lambda s: s["ui"]["frame"] > frame)
    raise AssertionError(f"Keyboard could not reach {identity}")


def title(value):
    click("record-desk.title.1013")
    wait(lambda s: any(n["id"] == "record-desk.title.1013" and n["focused"] for n in s["ui"]["nodes"]))
    key("ctrl+a")
    key("BackSpace")
    if value:
        xdo("type", "--clearmodifiers", "--delay", 1, value)
    wait(lambda s: s["draft"]["title"] == value)


def launch():
    global app, runtime, window
    SNAPSHOT.unlink(missing_ok=True)
    runtime = (EVIDENCE / f"native-runtime-{len(STEPS)}.log").open("w")
    app = subprocess.Popen([str(BINARY)], cwd=BINARY.parent, stdout=runtime, stderr=subprocess.STDOUT,
        env={**os.environ, "RECORD_DESK_STATE_PATH": str(STATE), "RECORD_DESK_SNAPSHOT": str(SNAPSHOT)})
    wait(lambda s: len(s["records"]) == 12 and len(s["ui"]["nodes"]) > 10)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        try:
            window = xdo("search", "--onlyvisible", "--name", "^Record Desk$").splitlines()[0]
            break
        except subprocess.CalledProcessError:
            time.sleep(0.05)
    else:
        raise RuntimeError("Record Desk window did not become visible")
    xdo("windowfocus", "--sync", window)
    # Readiness requires the principal chrome, not just an initial sizing pass.
    wait(lambda s: all(any(action in n["actions"] and n["rect"]["max_y"] > n["rect"]["min_y"]
        for n in s["ui"]["nodes"]) for action in (
            "record-desk.undo", "record-desk.redo", "record-desk.save",
            "record-desk.restore", "record-desk.arrange")))


def stop():
    global app
    if app and app.poll() is None:
        app.terminate()
        app.wait(timeout=10)
    app = None
    if runtime:
        runtime.close()


def terminate(_signal, _frame):
    raise SystemExit(143)


signal.signal(signal.SIGTERM, terminate)
try:
    STATE.unlink(missing_ok=True)  # This file is owned by the synthetic test.
    launch()
    record("ordinary")
    subprocess.run([IMPORT, "-window", window, str(EVIDENCE / "native-ordinary.png")], check=True)
    key("ctrl+f")
    wait(lambda s: any(n["id"] == "record-desk.search" and n["focused"] for n in s["ui"]["nodes"]))
    xdo("type", "--clearmodifiers", "agenda")
    wait(lambda s: s["visible_ids"] == [1013])
    focus("record-desk.record.1013")
    key("Return")
    wait(lambda s: s["selected"] == 1013)
    record("search-select")
    click("record-desk.filter-category")
    click("record-desk.filter-category.option.Some(Ideas)")
    wait(lambda s: s["filters"]["category"] == "Ideas" and s["visible_ids"] == [] and s["selected"] == 1013)
    click("record-desk.filter-review")
    wait(lambda s: any(n["id"] == "record-desk.filter-review.option.Unreviewed" for n in s["ui"]["nodes"]))
    focus("record-desk.filter-review.option.Unreviewed")
    key("Return")
    wait(lambda s: s["filters"]["review"] == "Unreviewed")
    record("no-results")
    key("ctrl+shift+f")
    wait(lambda s: s["visible_ids"] and s["filters"]["query"] == "" and s["filters"]["category"] is None)
    title("")
    key("ctrl+Return")
    wait(lambda s: s["error"] and s["undo_entries"] == 0 and s["records"][1]["title"] == "Prepare review agenda")
    record("invalid")
    title("Review agenda updated")
    click("record-desk.toggle-reviewed")
    wait(lambda s: s["draft"]["reviewed"] != s["records"][1]["reviewed"])
    key("ctrl+Return")
    wait(lambda s: s["undo_entries"] == 1 and not s["draft_dirty"])
    record("apply")
    key("ctrl+z")
    wait(lambda s: s["undo_entries"] == 0 and s["redo_entries"] == 1)
    record("undo")
    key("ctrl+shift+z")
    wait(lambda s: s["undo_entries"] == 1 and s["redo_entries"] == 0)
    record("redo")
    click("polyorama.dock.splitter.1")
    wait(lambda s: any(n["id"] == "polyorama.dock.splitter.1" and n["focused"] for n in s["ui"]["nodes"]))
    key("Right")
    wait(lambda s: s["workspace"]["root"]["Split"]["fraction"] > 0.36)
    saved_layout = wait(lambda s: True)["workspace"]
    key("ctrl+s")
    wait(lambda s: not s["unsaved"] and not s["error"])
    title("Uncommitted draft excluded")
    key("ctrl+s")
    wait(lambda s: "excluded" in s["message"])
    saved = json.loads(STATE.read_text())
    assert saved["records"][1]["title"] == "Review agenda updated"
    assert saved["workspace"] == saved_layout
    stop()
    launch()
    state = wait(lambda s: s["records"][1]["title"] == "Review agenda updated" and not s["unsaved"])
    assert state["workspace"] == saved_layout and state["undo_entries"] == 0 and not state["draft_dirty"]
    record("restart-restored")
    time.sleep(1.0)  # Allow startup and egui hover/animation deadlines to settle.
    frame = wait(lambda s: True)["ui"]["frame"]
    time.sleep(0.35)
    assert wait(lambda s: True)["ui"]["frame"] == frame, "native app should settle when idle"
    xdo("windowsize", "--sync", window, 390, 844)
    wait(lambda s: s["workspace"]["root"]["Split"]["axis"] == "Vertical")
    record("narrow")
    subprocess.run([IMPORT, "-window", window, str(EVIDENCE / "native-narrow.png")], check=True)
    stop()
    STATE.write_text('{broken')  # Direct fault setup, not physical input evidence.
    launch()
    wait(lambda s: s["save_blocked"] and s["error"])
    key("ctrl+s")
    assert STATE.read_text() == '{broken'
    record("malformed-preserved")
    subprocess.run([IMPORT, "-window", window, str(EVIDENCE / "native-error.png")], check=True)
    STATE.unlink()
    click("record-desk.restore")
    wait(lambda s: not s["save_blocked"] and not s["error"])
    key("ctrl+s")
    wait(lambda s: not s["unsaved"] and not s["error"])
    report = {"source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)), "binary_sha256": hashlib.sha256(BINARY.read_bytes()).hexdigest(), "input_route": "xdotool OS pointer/keyboard; snapshot read only", "backend": "wgpu GL / Mesa llvmpipe under Xvfb", "steps": STEPS}
    (EVIDENCE / "native-workflow.json").write_text(json.dumps(report, indent=2))
    print("Record Desk native passed: search/filter/select, invalid Apply, transaction, undo/redo, save/restart, draft exclusion, layout restore, narrow and malformed state")
finally:
    stop()
