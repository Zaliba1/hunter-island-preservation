# SPDX-License-Identifier: MPL-2.0
import sys
from string import Formatter
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'launcher'))
from hunter_launcher.app import gui, play
from hunter_launcher.language import (GAME_LANGUAGES, LANGUAGES, LANGUAGE_CODES,
                                      LauncherInputError, issue_message,
                                      language_arguments, message, translations)
from hunter_launcher.storage import Store, atomic_json


class LanguageOptionsTest(unittest.TestCase):
    def test_older_profiles_default_to_english_and_untrusted_values_cannot_add_options(self):
        for config in ({}, {'device': 'ipad'}, {'language': '--unapproved-option'},
                       {'language': None}, {'language': []}):
            self.assertEqual(language_arguments(config), ['--preferred-languages=en'])

    def test_launch_uses_saved_choice_and_system_mode_passes_the_supported_windows_language(self):
        with tempfile.TemporaryDirectory() as folder:
            store = Store(Path(folder) / 'profile')
            with patch.object(store, 'stage_runtime', return_value=Path(folder) / 'fixture.exe'), \
                    patch('hunter_launcher.language.windows_ui_languages', return_value=['pl-PL', 'fr-CA']), \
                    patch('hunter_launcher.isolation.Container') as container:
                container.return_value.launch.return_value = {'exit_code': 0}
                for code, _ in LANGUAGES:
                    atomic_json(store.config, {'language': code})
                    play(store, Path(folder))
                    args = container.return_value.launch.call_args.args[1]
                    overrides = [a for a in args if a.startswith('--preferred-languages=')]
                    self.assertEqual(overrides, ['--preferred-languages=' + ('fr' if code == 'system' else code)])
            with patch('hunter_launcher.language.windows_ui_languages', return_value=['pl-PL']):
                self.assertEqual(language_arguments({'language': 'system'}), ['--preferred-languages=en'])

    def test_complete_catalogs_keep_the_same_message_parameters(self):
        english = translations('en')
        def fields(value):
            return {key for _, key, _, _ in Formatter().parse(value) if key is not None}
        for code in GAME_LANGUAGES:
            with self.subTest(code=code):
                catalog = translations(code)
                self.assertEqual(catalog.keys(), english.keys())
                self.assertEqual({k: fields(v) for k, v in catalog.items()},
                                 {k: fields(v) for k, v in english.items()})
        self.assertIn('FairPlay で暗号化', issue_message('ja', LauncherInputError('encrypted_ipa')))

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
            play_labels = {'en': 'Play', 'de': 'Spiel starten', 'fr': 'Jouer',
                           'es': 'Jugar', 'it': 'Gioca', 'ja': 'プレイ', 'system': 'Jouer'}
            for expected, change in [('English', True), ('Langue du système', False)]:
                root = tk.Tk()
                root.withdraw()
                failures = []

                def check():
                    try:
                        combo = next(w for w in controls(root)
                                     if isinstance(w, ttk.Combobox) and w.winfo_name() == 'game_language')
                        self.assertEqual(combo.get(), expected)
                        self.assertEqual(str(combo['state']), 'readonly')
                        play_button = next(w for w in controls(root) if w.winfo_name() == 'play')
                        full = next(w for w in controls(root) if w.winfo_name() == 'full')
                        if change:
                            full.invoke()
                            for code in LANGUAGE_CODES:
                                combo.current(LANGUAGE_CODES.index(code))
                                combo.event_generate('<<ComboboxSelected>>')
                                self.assertEqual(play_button.cget('text'), play_labels[code])
                                self.assertTrue(bool(root.getvar(str(full.cget('variable')))))
                            config = Store(store.root).settings()
                            self.assertEqual(config['language'], 'system')
                            self.assertEqual(config['device'], 'iphone')
                            self.assertTrue(config['mods']['--hunter-rare-dragons'])
                            self.assertNotIn('--hunter-test-save', config['mods'])
                        else:
                            self.assertEqual(play_button.cget('text'), play_labels['system'])
                    except BaseException as error:
                        failures.append(error)
                    finally:
                        for callback in root.tk.splitlist(root.tk.call('after', 'info')):
                            root.after_cancel(callback)
                        root.destroy()

                root.after(50, check)
                with patch('tkinter.Tk', return_value=root), \
                        patch('hunter_launcher.language.windows_ui_languages', return_value=['fr-CA']):
                    gui(store, Path(folder))
                if failures:
                    raise failures[0]
