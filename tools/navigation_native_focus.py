"""Observe native navigation focus progress before another physical Tab."""

import time


def _observation(state):
    fixture = state["navigation_fixture"]
    focused = [node["id"] for node in state["ui_snapshot"]["nodes"] if node["focused"]]
    fixture_ids = {target["id"] for target in fixture["targets"]}
    return {
        "frame": state["frame"],
        "tab_input_epoch": state["tab_input_epoch"],
        "focused_ids": focused,
        "fixture_focus_ids": sorted(set(focused) & fixture_ids),
        "selected": fixture["selected"],
        "activations": fixture["activations"],
    }


def focus_navigation_target(
    snapshot, press_tab, target_id, transitions, *, max_actions=40,
    timeout_seconds=10, monotonic=time.monotonic, sleep=time.sleep,
):
    """Keep one traversal deadline and stop after failed dispatch/observation.

    The Gallery publishes during presentation. A newer frame alone does not
    acknowledge a Tab: the previous key's release can present first, and two
    presses received in one input frame move focus only once. Wait for the
    application's `tab_input_epoch` receipt before issuing another Tab. Once
    fixture focus is observable, also wait for different observable focus.
    """
    deadline = monotonic() + timeout_seconds
    state = snapshot()
    initial = _observation(state)
    expected = (initial["selected"], initial["activations"])

    def observe(state, transition=None):
        observed = _observation(state)
        if transition is not None:
            transition["after"] = observed
            if not transition["observations"] or transition["observations"][-1] != observed:
                transition["observations"].append(observed)
        if (observed["selected"], observed["activations"]) != expected:
            raise AssertionError("native navigation Tab changed selection or activation")
        return observed

    if target_id in initial["focused_ids"]:
        return state
    for action in range(1, max_actions + 1):
        # Do not dispatch against an earlier snapshot if focus has since moved.
        state = snapshot()
        before = observe(state)
        if monotonic() >= deadline:
            raise TimeoutError("native navigation focus traversal deadline exceeded")
        if target_id in before["focused_ids"]:
            return state
        transition = {"action": action, "target_id": target_id, "before": before,
                      "after": None, "observations": [before], "dispatch": "pending",
                      "barrier": "tab_receipt_and_fixture_focus_progress" if before["fixture_focus_ids"] else "tab_receipt"}
        transitions.append(transition)
        try:
            press_tab()
        except BaseException as error:
            transition.update(dispatch="failed", error=str(error))
            raise
        transition["dispatch"] = "acknowledged by xdotool"
        while True:
            state = snapshot()
            after = observe(state, transition)
            received = after["tab_input_epoch"] > before["tab_input_epoch"]
            if monotonic() >= deadline:
                if after["frame"] <= before["frame"]:
                    transition["error"] = "presentation did not advance"
                elif not received:
                    transition["error"] = "Tab was not received"
                else:
                    transition["error"] = "observable fixture focus did not progress"
                raise TimeoutError(f"native navigation {transition['error']} after Tab {action}")
            # Examine every observation, including the final permitted action.
            if target_id in after["focused_ids"]:
                return state
            if received and after["frame"] > before["frame"]:
                if not before["fixture_focus_ids"]:
                    break
                if after["fixture_focus_ids"] and after["fixture_focus_ids"] != before["fixture_focus_ids"]:
                    break
            sleep(0.03)
    raise AssertionError(f"keyboard did not reach {target_id} after {max_actions} actions")
