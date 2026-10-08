"""Check the production traversal against delayed and failed observations."""

import unittest

from tools.navigation_native_focus import focus_navigation_target


class Clock:
    def __init__(self):
        self.now = 0.0

    def monotonic(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


def state(frame, focused):
    return {"frame": frame, "navigation_fixture": {"selected": "home", "activations": 0, "targets": [{"id": value} for value in ["home", "tasks", "activity"]]},
            "ui_snapshot": {"nodes": [{"id": focused, "focused": True}]}}


class NavigationNativeFocusTests(unittest.TestCase):
    def test_delayed_presentation_blocks_another_tab(self):
        clock, transitions, presses = Clock(), [], []
        current, pending = state(0, "initial"), None

        def snapshot():
            nonlocal current, pending
            if pending and clock.now >= pending[0]:
                current, pending = pending[1], None
            return current

        def press():
            nonlocal pending
            self.assertIsNone(pending, "another key preceded the prior presentation")
            presses.append(clock.now)
            pending = (clock.now + 0.2, state(len(presses), "tasks" if len(presses) == 2 else "home"))

        result = focus_navigation_target(snapshot, press, "tasks", transitions,
            monotonic=clock.monotonic, sleep=clock.sleep)
        self.assertEqual(result["frame"], 2)
        self.assertGreaterEqual(presses[1] - presses[0], 0.2)
        self.assertEqual([t["after"]["frame"] for t in transitions], [1, 2])

    def test_fixed_delay_discriminator_can_outpace_presentation(self):
        clock, presses = Clock(), []
        due = None

        def press():
            nonlocal due
            presses.append(due is not None and clock.now < due)
            due = clock.now + 0.2

        press()
        clock.sleep(0.06)
        press()
        self.assertEqual(presses, [False, True])

    def test_stale_frame_times_out_without_another_key(self):
        clock, presses, transitions = Clock(), [], []
        with self.assertRaises(TimeoutError):
            focus_navigation_target(lambda: state(1, "home"), lambda: presses.append(1), "tasks", transitions,
                timeout_seconds=0.2, monotonic=clock.monotonic, sleep=clock.sleep)
        self.assertEqual(presses, [1])
        self.assertEqual(transitions[-1]["after"]["frame"], 1)
        self.assertEqual(transitions[-1]["error"], "presentation did not advance")

    def test_unreachable_stops_at_forty_actions(self):
        current, transitions, presses = [state(0, "home")], [], []

        def press():
            presses.append(1)
            current[0] = state(len(presses), "activity" if len(presses) % 2 else "home")

        with self.assertRaisesRegex(AssertionError, "after 40 actions"):
            focus_navigation_target(lambda: current[0], press, "tasks", transitions)
        self.assertEqual(len(presses), 40)
        self.assertEqual(transitions[-1]["after"]["frame"], 40)

    def test_target_on_final_permitted_action_is_accepted(self):
        current, transitions, presses = [state(0, "home")], [], []

        def press():
            presses.append(1)
            current[0] = state(len(presses), "tasks" if len(presses) == 40 else ("activity" if len(presses) % 2 else "home"))

        result = focus_navigation_target(lambda: current[0], press, "tasks", transitions)
        self.assertEqual(result["frame"], 40)
        self.assertEqual(len(presses), 40)

    def test_failed_dispatch_propagates_without_another_key(self):
        transitions, presses = [], []
        error = RuntimeError("xdotool failed")

        def press():
            presses.append(1)
            raise error

        with self.assertRaises(RuntimeError) as result:
            focus_navigation_target(lambda: state(1, "home"), press, "tasks", transitions)
        self.assertIs(result.exception, error)
        self.assertEqual(presses, [1])
        self.assertEqual(transitions[0]["dispatch"], "failed")

    def test_tab_cannot_select_or_activate_a_destination(self):
        current, transitions = [state(0, "home")], []

        def press():
            current[0] = state(1, "tasks")
            current[0]["navigation_fixture"]["selected"] = "tasks"

        with self.assertRaisesRegex(AssertionError, "changed selection or activation"):
            focus_navigation_target(lambda: current[0], press, "tasks", transitions)


    def _focus_progress_probe(self, newer_home_frames):
        clock, presses, transitions = Clock(), [], []
        current, due = state(52, "home"), None

        def snapshot():
            nonlocal current
            if due is not None and clock.now >= due:
                current = state(53 + newer_home_frames, "tasks")
            elif due is not None:
                # Existing presentations can advance while the prior focus is
                # still published. These are real observations, not progress.
                current = state(53 + min(int(clock.now / 0.03), newer_home_frames - 1), "home")
            return current

        def press():
            nonlocal due
            if due is not None and clock.now < due:
                raise RuntimeError("speculative Tab before fixture focus progressed")
            presses.append(1)
            due = clock.now + 0.12

        result = focus_navigation_target(snapshot, press, "tasks", transitions,
            monotonic=clock.monotonic, sleep=clock.sleep)
        self.assertEqual(result["ui_snapshot"]["nodes"][0]["id"], "tasks")
        self.assertEqual(presses, [1])
        self.assertGreaterEqual(len(transitions[0]["observations"]), newer_home_frames + 1)

    def test_focus_progress_waits_past_first_newer_home_frame(self):
        self._focus_progress_probe(1)

    def test_focus_progress_waits_past_two_newer_home_frames(self):
        self._focus_progress_probe(2)

    def test_focus_progress_unchanged_home_times_out_without_another_tab(self):
        clock, presses, transitions, frame = Clock(), [], [], [1]

        def snapshot():
            frame[0] += 1
            return state(frame[0], "home")

        with self.assertRaises(TimeoutError):
            focus_navigation_target(snapshot, lambda: presses.append(1), "tasks", transitions,
                timeout_seconds=0.2, monotonic=clock.monotonic, sleep=clock.sleep)
        self.assertEqual(presses, [1])
        self.assertGreater(transitions[-1]["after"]["frame"], transitions[-1]["before"]["frame"])
        self.assertGreater(len(transitions[-1]["observations"]), 1)

    def test_focus_progress_refresh_accepts_target_before_another_dispatch(self):
        reads, presses, transitions = [], [], []

        def snapshot():
            reads.append(1)
            return state(len(reads), "tasks" if len(reads) > 1 else "home")

        focus_navigation_target(snapshot, lambda: presses.append(1), "tasks", transitions)
        self.assertEqual(presses, [])

    def test_focus_progress_checks_transient_selection_and_activation(self):
        for mutation, value in [("selected", "tasks"), ("activations", 1)]:
            with self.subTest(mutation=mutation):
                clock, transitions, presses = Clock(), [], []

                def snapshot():
                    result = state(52 if not presses else (53 if clock.now < 0.06 else 54), "home")
                    if presses and 0.03 <= clock.now < 0.06:
                        result["navigation_fixture"][mutation] = value
                    if clock.now >= 0.06:
                        result["ui_snapshot"]["nodes"] = [{"id": "tasks", "focused": True}]
                    return result

                with self.assertRaisesRegex(AssertionError, "changed selection or activation"):
                    focus_navigation_target(snapshot, lambda: presses.append(1), "tasks", transitions,
                        monotonic=clock.monotonic, sleep=clock.sleep)
                self.assertEqual(presses, [1])

    def test_focus_progress_empty_focus_is_not_settled(self):
        clock, presses, transitions = Clock(), [], []

        def snapshot():
            if not presses:
                return state(52, "home")
            if clock.now < 0.09:
                return state(53, "chrome")
            return state(54, "tasks")

        def press():
            self.assertEqual(presses, [], "empty focus allowed another speculative Tab")
            presses.append(1)

        focus_navigation_target(snapshot, press, "tasks", transitions,
            monotonic=clock.monotonic, sleep=clock.sleep)
        self.assertEqual(presses, [1])
        self.assertEqual(transitions[0]["observations"][1]["fixture_focus_ids"], [])

    def test_focus_progress_keeps_one_deadline_across_actions(self):
        clock, presses, transitions = Clock(), [], []
        current, due = state(0, "home"), None

        def snapshot():
            nonlocal current, due
            if due and clock.now >= due[0]:
                current, due = due[1], None
            return current

        def press():
            nonlocal due
            presses.append(1)
            due = (clock.now + 0.09, state(len(presses), "activity" if len(presses) == 1 else "tasks"))

        with self.assertRaises(TimeoutError):
            focus_navigation_target(snapshot, press, "tasks", transitions, timeout_seconds=0.15,
                monotonic=clock.monotonic, sleep=clock.sleep)
        self.assertEqual(len(presses), 2)
        self.assertLess(clock.now, 0.18)
