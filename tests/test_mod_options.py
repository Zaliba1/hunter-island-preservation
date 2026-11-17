# SPDX-License-Identifier: MPL-2.0
import sys
import unittest
from itertools import product
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "launcher"))
from hunter_launcher.app import MOD_OPTIONS, mod_arguments
from hunter_launcher.storage import Store, atomic_json
import tempfile
from unittest.mock import patch

class ModOptionsTest(unittest.TestCase):
    def test_windows_test_checkbox_reaches_play_and_persists_without_changing_normal_save(self):
        import tkinter as tk
        from tkinter import ttk
        from hunter_launcher.app import gui
        def controls(widget):
            for child in widget.winfo_children():
                yield child
                yield from controls(child)
        with tempfile.TemporaryDirectory() as folder:
            store=Store(folder)
            atomic_json(store.config, {'game': {'version': '2.02'}, 'mods': {'--hunter-rare-dragons': True}})
            root=tk.Tk();root.withdraw();failures=[]
            def click():
                try:
                    box=next(w for w in controls(root) if isinstance(w,ttk.Checkbutton) and str(w.cget('text')).startswith('Testspielstand'))
                    level1=next(w for w in controls(root) if isinstance(w,ttk.Checkbutton) and str(w.cget('text')).startswith('Neues Spiel:'))
                    box.invoke(); level1.invoke()
                    self.assertFalse(bool(root.getvar(str(box.cget('variable')))))
                    self.assertTrue(bool(root.getvar(str(level1.cget('variable')))))
                    box.invoke()
                    self.assertFalse(bool(root.getvar(str(level1.cget('variable')))))
                    next(w for w in controls(root) if isinstance(w,ttk.Button) and w.cget('text')=='Spiel starten').invoke()
                except BaseException as error:failures.append(error)
            def check():
                try:
                    play.assert_called_once()
                    self.assertTrue(store.settings()['mods']['--hunter-test-save'])
                    self.assertTrue(store.settings()['mods']['--hunter-rare-dragons'])
                    self.assertEqual(store.bundle.name,'pockettrend.hunterisland-test-v1')
                except BaseException as error:failures.append(error)
                finally:
                    for callback in root.tk.splitlist(root.tk.call('after','info')):root.after_cancel(callback)
                    root.destroy()
            root.after(50,click);root.after(400,check)
            with patch('tkinter.Tk',return_value=root),patch('hunter_launcher.app.play',return_value={'exit_code':0}) as play:
                gui(store,Path(folder))
            if failures:raise failures[0]

    def test_testing_profile_and_normal_profile_are_separate(self):
        with tempfile.TemporaryDirectory() as directory:
            store = Store(directory)
            normal = store.bundle
            normal.mkdir(parents=True)
            (normal / 'original').write_bytes(b'normal')
            atomic_json(store.config, {'mods': {'--hunter-test-save': True}})
            self.assertNotEqual(store.bundle, normal)
            self.assertEqual(store.bundle.name, 'pockettrend.hunterisland-test-v1')
            with self.assertRaisesRegex(ValueError, 'normales Spiel'):
                store.require_testing_seed()
            seed = normal / 'Documents/saves0/sv_1'
            seed.parent.mkdir(parents=True)
            seed.write_bytes(b'normal checkpoint')
            store.require_testing_seed()
            store.bundle.mkdir()
            (store.bundle / 'testing').write_bytes(b'test')
            atomic_json(store.config, {'mods': {'--hunter-test-save': False}})
            self.assertEqual(store.bundle, normal)
            self.assertEqual((normal / 'original').read_bytes(), b'normal')
    def test_older_profiles_keep_original_rules(self):
        self.assertEqual(mod_arguments({"device": "ipad"}), [])

    def test_all_choices_are_independent_and_unknown_flags_cannot_be_injected(self):
        for states in product((False, True), repeat=len(MOD_OPTIONS)):
            config = dict(zip((flag for flag, _ in MOD_OPTIONS), states))
            config["--unapproved-flag"] = True
            if config['--hunter-test-save'] and config['--hunter-level1-save']:
                with self.assertRaises(ValueError): mod_arguments({'mods': config})
                continue
            self.assertEqual(mod_arguments({"mods": config}),
                             [flag for (flag, _), enabled in zip(MOD_OPTIONS, states) if enabled])

    def test_level_one_starts_fresh_without_a_normal_seed(self):
        with tempfile.TemporaryDirectory() as directory:
            store=Store(directory)
            normal=store.bundle
            atomic_json(store.config,{'mods':{'--hunter-level1-save':True,'--hunter-rare-dragons':True}})
            store.require_testing_seed()
            self.assertEqual(store.bundle.name,'pockettrend.hunterisland-level1-v1')
            self.assertFalse(normal.exists())
            self.assertEqual(mod_arguments(store.settings()),['--hunter-rare-dragons','--hunter-level1-save'])

    def test_invalid_settings_are_rejected(self):
        for mods in (None, [], {"--hunter-rare-dragons": "false"}):
            with self.assertRaises(ValueError):
                mod_arguments({"mods": mods})
