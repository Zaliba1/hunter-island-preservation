# SPDX-License-Identifier: MPL-2.0
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'launcher'))
from hunter_launcher.app import gui, play
from hunter_launcher.language import LANGUAGES, language_arguments
from hunter_launcher.storage import Store, atomic_json


class LanguageOptionsTest(unittest.TestCase):
    def test_older_profiles_default_to_english_and_untrusted_values_cannot_add_options(self):
        for config in ({}, {'device': 'ipad'}, {'language': '--unapproved-option'},
                       {'language': None}, {'language': []}):
            self.assertEqual(language_arguments(config), ['--preferred-languages=en'])

    def test_launch_uses_saved_choice_and_system_mode_omits_override(self):
        with tempfile.TemporaryDirectory() as folder:
            store = Store(Path(folder) / 'profile')
            with patch.object(store, 'stage_runtime', return_value=Path(folder) / 'fixture.exe'), \
                    patch('hunter_launcher.isolation.Container') as container:
                container.return_value.launch.return_value = {'exit_code': 0}
                for code, _ in LANGUAGES:
                    atomic_json(store.config, {'language': code})
                    play(store, Path(folder))
                    args = container.return_value.launch.call_args.args[1]
                    overrides = [a for a in args if a.startswith('--preferred-languages=')]
                    self.assertEqual(overrides, [] if code == 'system' else ['--preferred-languages=' + code])

    def test_windows_dropdown_saves_without_starting_game_and_restores_on_reopen(self):
        import tkinter as tk
        from tkinter import ttk

        def controls(widget):
            for child in widget.winfo_children():
                yield child
                yield from controls(child)

        with tempfile.TemporaryDirectory() as folder:
            store = Store(Path(folder) / 'profile')
            atomic_json(store.config, {'device': 'iphone', 'mods': {'--hunter-rare-dragons': True}})
            for expected, select in [('English', 'Deutsch'), ('Deutsch', None)]:
                root = tk.Tk()
                root.withdraw()
                failures = []

                def check():
                    try:
                        combo = next(w for w in controls(root)
                                     if isinstance(w, ttk.Combobox) and w.winfo_name() == 'game_language')
                        self.assertEqual(combo.get(), expected)
                        self.assertEqual(str(combo['state']), 'readonly')
                        if select:
                            combo.current(list(combo['values']).index(select))
                            combo.event_generate('<<ComboboxSelected>>')
                            config = Store(store.root).settings()
                            self.assertEqual(config['language'], 'de')
                            self.assertEqual(config['device'], 'iphone')
                            self.assertTrue(config['mods']['--hunter-rare-dragons'])
                    except BaseException as error:
                        failures.append(error)
                    finally:
                        for callback in root.tk.splitlist(root.tk.call('after', 'info')):
                            root.after_cancel(callback)
                        root.destroy()

                root.after(50, check)
                with patch('tkinter.Tk', return_value=root):
                    gui(store, Path(folder))
                if failures:
                    raise failures[0]
