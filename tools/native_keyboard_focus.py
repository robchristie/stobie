"""Observe native Tab receipt and current focus before further physical input."""

import time


def focus_native_target(snapshot, press_tab, wait, target_id, transitions, *,
                        max_actions=24, settle=time.sleep, settle_seconds=0.05):
    """Retain the caller's wait deadline and inspect every permitted action."""
    def observe(state, transition=None):
        nodes = state["ui"]["nodes"]
        target = next((node for node in nodes if node["id"] == target_id), None)
        observation = {
            "frame": state["ui"]["frame"],
            "tab_input_epoch": state["tab_input_epoch"],
            "focused_ids": sorted(node["id"] for node in nodes if node["focused"]),
            "target_present": target is not None,
            "target_enabled": target is not None and target["enabled"],
            "filters": state.get("filters"),
            "selected": state.get("selected"),
        }
        if transition is not None:
            transition["after"] = observation
            if not transition["observations"] or transition["observations"][-1] != observation:
                transition["observations"].append(observation)
        if not observation["target_enabled"]:
            raise AssertionError(f"Native keyboard target disappeared or became disabled: {target_id}")
        return observation

    for action in range(1, max_actions + 1):
        state = snapshot()
        before = observe(state)
        if target_id in before["focused_ids"]:
            return state
        transition = {"action": action, "target_id": target_id, "before": before,
                      "after": None, "observations": [], "dispatch": "pending"}
        transitions.append(transition)
        try:
            press_tab()
            transition["dispatch"] = "acknowledged by xdotool"

            def received(state):
                after = observe(state, transition)
                return after["tab_input_epoch"] > before["tab_input_epoch"]

            wait(received)
            # Match the browser journey's presentation pacing after receipt.
            # Empty focus can be a valid wrap between egui Tab cycles; only the
            # requested target's observed focus establishes successful traversal.
            settle(settle_seconds)
            state = snapshot()
            after = observe(state, transition)
        except BaseException as error:
            if transition["dispatch"] == "pending":
                transition["dispatch"] = "failed"
            transition["error"] = str(error)
            raise
        if target_id in after["focused_ids"]:
            return state
    raise AssertionError(f"Keyboard could not reach {target_id} after {max_actions} actions")
