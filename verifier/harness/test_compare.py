"""Checks of compare.py's ignore rules: python3 -m unittest harness.test_compare (from verifier/)."""
import unittest

from harness.compare import compare

IGNORE = {"controls", "unnamed_controls"}


def trace(initial_controls, mpris_status="Paused", step_controls=(), net=()):
    state = {"mpris": {"status": mpris_status}, "queue": [], "controls": list(initial_controls)}
    return {
        "errors": [],
        "initial": {"state": state, "tray": ["Show"], "controls": list(initial_controls)},
        "steps": [{"i": 0, "label": "", "state": {"mpris": {"status": mpris_status}, "controls": list(step_controls)}, "net": list(net)}],
        "final": {"tray": ["Show"], "controls": [], "unnamed_controls": 3},
    }


class IgnoreMeansTheSameEverywhere(unittest.TestCase):
    def test_start_up_inventory_is_ignored_like_a_steps(self):
        golden = trace(["button: Home"])
        got = trace(["button: Home", "button: Fixture 1001"], step_controls=["button: Extra"])
        self.assertEqual(compare(golden, got, IGNORE), [])

    def test_without_ignore_the_inventory_still_counts(self):
        golden = trace(["button: Home"])
        got = trace(["button: Home", "button: Fixture 1001"])
        diffs = compare(golden, got)
        self.assertIn("initial.state.controls: extra button: Fixture 1001", diffs)
        self.assertIn("initial.controls: extra button: Fixture 1001", diffs)

    def test_behaviour_in_the_initial_state_still_counts(self):
        golden = trace([], mpris_status="Paused")
        got = trace([], mpris_status="Playing")
        diffs = compare(golden, got, IGNORE)
        self.assertTrue(any(d.startswith("initial.state.mpris.status") for d in diffs), diffs)

    def test_network_and_steps_still_count(self):
        golden = trace([], net=["GET api.tidal.com/v1/pages/explore?countryCode=US"])
        got = trace([], net=[])
        self.assertEqual(compare(golden, got, IGNORE), ["step 0.net: missing GET api.tidal.com/v1/pages/explore?countryCode=US"])

    def test_final_tray_still_counts(self):
        golden = trace([])
        got = trace([])
        got["final"]["tray"] = ["Quit"]
        self.assertTrue(any(d.startswith("final.tray") for d in compare(golden, got, IGNORE)))


if __name__ == "__main__":
    unittest.main()
