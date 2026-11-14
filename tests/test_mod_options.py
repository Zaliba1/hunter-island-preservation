# SPDX-License-Identifier: MPL-2.0
import sys
import unittest
from itertools import product
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "launcher"))
from hunter_launcher.app import MOD_OPTIONS, mod_arguments

class ModOptionsTest(unittest.TestCase):
    def test_older_profiles_keep_original_rules(self):
        self.assertEqual(mod_arguments({"device": "ipad"}), [])

    def test_all_choices_are_independent_and_unknown_flags_cannot_be_injected(self):
        for states in product((False, True), repeat=3):
            config = dict(zip((flag for flag, _ in MOD_OPTIONS), states))
            config["--unapproved-flag"] = True
            self.assertEqual(mod_arguments({"mods": config}),
                             [flag for (flag, _), enabled in zip(MOD_OPTIONS, states) if enabled])

    def test_invalid_settings_are_rejected(self):
        for mods in (None, [], {"--hunter-rare-dragons": "false"}):
            with self.assertRaises(ValueError):
                mod_arguments({"mods": mods})
