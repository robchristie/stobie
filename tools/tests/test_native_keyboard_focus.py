"""Keep physical native traversal behind input receipt and current focus."""

import unittest

from tools.native_keyboard_focus import focus_native_target


def state(frame, epoch, focused, *, present=True, enabled=True):
    nodes = [{"id": "start", "enabled": True, "focused": focused == "start"}]
    if present:
        nodes.append({"id": "target", "enabled": enabled, "focused": focused == "target"})
    return {"tab_input_epoch": epoch, "ui": {"frame": frame, "nodes": nodes}}


class NativeKeyboardFocusTests(unittest.TestCase):
    def test_newer_frames_and_even_target_focus_do_not_acknowledge_tab(self):
        current, presses, transitions = [state(1, 0, "start")], [], []

        def wait(predicate):
            for observation in [state(2, 0, "start"), state(3, 0, "target")]:
                self.assertFalse(predicate(observation))
            current[0] = state(4, 1, "target")
            self.assertTrue(predicate(current[0]))
            return current[0]

        focus_native_target(lambda: current[0], lambda: presses.append(1), wait,
            "target", transitions, settle=lambda _seconds: None)
        self.assertEqual(presses, [1])
        self.assertEqual([s["tab_input_epoch"] for s in transitions[0]["observations"]], [0, 0, 1])

    def test_receipt_precedes_pacing_and_fresh_focus_observation(self):
        current, events, transitions = [state(1, 0, "start")], [], []

        def snapshot():
            events.append("snapshot")
            return current[0]

        def wait(predicate):
            events.append("receipt")
            current[0] = state(2, 1, "start")
            self.assertTrue(predicate(current[0]))
            return current[0]

        def settle(seconds):
            self.assertEqual(seconds, 0.05)
            events.append("settle")
            current[0] = state(3, 1, "target")

        focus_native_target(snapshot, lambda: events.append("Tab"), wait, "target", transitions,
            settle=settle)
        self.assertEqual(events, ["snapshot", "Tab", "receipt", "settle", "snapshot"])
        self.assertEqual(transitions[0]["after"]["focused_ids"], ["target"])

    def test_already_focused_needs_no_input(self):
        def unexpected(*_args):
            self.fail("already focused target dispatched or waited")

        result = focus_native_target(lambda: state(1, 0, "target"), unexpected,
            unexpected, "target", [], settle=unexpected)
        self.assertEqual(result["tab_input_epoch"], 0)

    def traverse(self, *, target_on_action):
        current, presses, transitions = [state(1, 0, "start")], [], []

        def press():
            presses.append(1)
            current[0] = state(len(presses) + 1, len(presses),
                "target" if len(presses) == target_on_action else None)

        def wait(predicate):
            self.assertTrue(predicate(current[0]))
            return current[0]

        result = focus_native_target(lambda: current[0], press, wait, "target", transitions,
            settle=lambda _seconds: None)
        return result, presses, transitions

    def test_empty_focus_is_a_valid_intermediate_tab_cycle(self):
        result, presses, transitions = self.traverse(target_on_action=2)
        self.assertEqual(presses, [1, 1])
        self.assertEqual(transitions[0]["after"]["focused_ids"], [])
        self.assertEqual(result["tab_input_epoch"], 2)

    def test_final_permitted_action_is_observed(self):
        result, presses, transitions = self.traverse(target_on_action=24)
        self.assertEqual(len(presses), 24)
        self.assertEqual(result["tab_input_epoch"], 24)
        self.assertEqual(transitions[-1]["after"]["focused_ids"], ["target"])

    def test_unreachable_stops_at_existing_action_limit(self):
        with self.assertRaisesRegex(AssertionError, "after 24 actions"):
            self.traverse(target_on_action=25)

    def test_missing_or_disabled_popup_fails_without_further_input(self):
        for observation in [state(2, 1, None, present=False), state(2, 1, None, enabled=False)]:
            with self.subTest(observation=observation):
                presses, transitions = [], []

                def wait(predicate):
                    predicate(observation)
                    self.fail("unavailable popup was accepted")

                with self.assertRaisesRegex(AssertionError, "disappeared or became disabled"):
                    focus_native_target(lambda: state(1, 0, "start"),
                        lambda: presses.append(1), wait, "target", transitions,
                        settle=lambda _seconds: None)
                self.assertEqual(presses, [1])
                self.assertFalse(transitions[-1]["after"]["target_enabled"])

    def test_timeout_does_not_dispatch_another_tab_or_settle(self):
        presses, transitions = [], []

        def wait(predicate):
            self.assertFalse(predicate(state(2, 0, "start")))
            raise TimeoutError("receipt missing")

        def settle(_seconds):
            self.fail("missing receipt started settling")

        with self.assertRaisesRegex(TimeoutError, "receipt missing"):
            focus_native_target(lambda: state(1, 0, "start"),
                lambda: presses.append(1), wait, "target", transitions, settle=settle)
        self.assertEqual(presses, [1])
        self.assertEqual(transitions[-1]["error"], "receipt missing")

    def test_popup_disappearing_after_receipt_is_not_success(self):
        current, presses, transitions = [state(1, 0, "start")], [], []

        def wait(predicate):
            current[0] = state(2, 1, "target")
            self.assertTrue(predicate(current[0]))
            return current[0]

        def settle(_seconds):
            current[0] = state(3, 1, None, present=False)

        with self.assertRaisesRegex(AssertionError, "disappeared or became disabled"):
            focus_native_target(lambda: current[0], lambda: presses.append(1), wait,
                "target", transitions, settle=settle)
        self.assertEqual(presses, [1])
        self.assertFalse(transitions[-1]["after"]["target_present"])

    def test_failed_dispatch_propagates_without_waiting(self):
        presses, transitions = [], []
        error = RuntimeError("xdotool failed")

        def press():
            presses.append(1)
            raise error

        def wait(_predicate):
            self.fail("failed dispatch started observation")

        with self.assertRaises(RuntimeError) as result:
            focus_native_target(lambda: state(1, 0, "start"), press, wait, "target", transitions)
        self.assertIs(result.exception, error)
        self.assertEqual(presses, [1])
        self.assertEqual(transitions[-1]["dispatch"], "failed")


if __name__ == "__main__":
    unittest.main()
