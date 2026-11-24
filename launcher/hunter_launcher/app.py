"""Small local launcher, and CLI entry points for inspection and bounded QA."""
from pathlib import Path
import argparse
import json
import os
import queue
import sys
import threading
from . import VERSION, ipa
from .storage import Store, atomic_json
from .language import (LANGUAGES, LANGUAGE_CODES, active_language, issue_message,
                       language_arguments, message, selected_language)
from .profiles import TEST_PROFILES, selected_test_directory

MOD_OPTIONS = (
    ('--zalibas-expansion', 'expansion'),
    ('--hunter-hd-textures', 'hd_textures'),
    ('--scale-hack=2', 'render_2x'),
    ('--hunter-rare-dragons', 'rare'),
    ('--hunter-silver-exchange', 'exchange'),
    ('--hunter-no-arena-cooldown', 'arena'),
    ('--hunter-test-save', 'full'),
    ('--hunter-level1-save', 'level1'),
    ('--hunter-triple99-save', 'triple'),
)

def mod_arguments(settings):
    mods = settings.get("mods", {})
    if not isinstance(mods, dict):
        raise ValueError('Invalid mod settings')
    if any(not isinstance(value, bool) for value in mods.values()):
        raise ValueError('Mod settings must be true or false')
    selected_test_directory(settings)
    return [flag for flag, _ in MOD_OPTIONS if mods.get(flag, False)]

FILTERS = {'Bilinear': 'bilinear', 'FSR 1.2': 'fsr', 'FSR 1.2 + RCAS': 'fsr-sharpened',
           'Anime4K CNN x2 S': 'anime4k', 'FSRCNNX x2': 'fsrcnnx'}

def graphic_settings(settings):
    mods = settings.get('mods', {})
    fresh = '--hunter-hd-textures' not in mods and 'output_filter' not in settings
    return (settings.get('output_filter', 'anime4k' if fresh else 'bilinear'),
            mods.get('--hunter-hd-textures', fresh))

def release_root():
    if getattr(sys,'frozen',False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parents[2]

def play(store, release, timeout=None, test_taps=(), capture=False):
    from .isolation import Container
    with store.lock():
        store.require_testing_seed()
        executable = store.stage_runtime(release)
        store.backup()
        container = Container(store)
        try:
            container.prepare()
            container.probe()
            args = ['HunterIsland.ipa','--no-error-popup','--experimental-gles2',
                '--legacy-string-unsigned-int','--legacy-deferred-texture-deletes','--landscape-left','--runtime-metrics']
            settings = store.settings()
            if settings.get('device','ipad') == 'ipad':
                args.append('--device-family=ipad')
            args.extend(language_arguments(settings))
            args.extend(mod_arguments(settings))
            output, hd = graphic_settings(settings)
            if output in FILTERS.values() and output != 'bilinear':
                args.append('--upscale=' + output)
            if hd and '--hunter-hd-textures' not in args:
                args.append('--hunter-hd-textures')
            if settings.get('integer_scaling'): args.append('--integer-scaling')
            if settings.get('fullscreen'): args.append('--fullscreen')
            origin = settings.get('expansion_origin')
            if '--zalibas-expansion' in args and origin in ('copy', 'new'):
                args.append('--expansion-' + origin + '-campaign')
            if capture:
                args.append('--capture-frames')
            for tap in test_taps:
                if not __import__('re').fullmatch(r'\d+,\d+,\d+',tap):
                    raise ValueError('Invalid QA input')
                args.append('--test-tap='+tap)
            result = container.launch(executable,args,timeout=timeout)
            store.backup()
            return result
        finally:
            container.close()

def gui(store, release):
    import tkinter as tk
    from tkinter import filedialog, messagebox, ttk
    from . import diagnostics

    settings = store.settings()
    mod_arguments(settings)
    language = active_language(settings)
    localized = []
    root = tk.Tk()

    def text(key, **values):
        return message(language, key, **values)

    def translate(widget, key, **values):
        localized.append((widget, key, values))
        return widget

    def error_dialog(key, error):
        messagebox.showerror(text(key), issue_message(language, error), parent=root)

    root.geometry(f"680x{min(640, root.winfo_screenheight()-90)}")
    root.minsize(440, 360)
    root.configure(bg='#f3f5f7')
    style = ttk.Style(root)
    style.theme_use('clam')
    style.configure('TFrame',background='#f3f5f7')
    style.configure('TLabel',background='#f3f5f7',foreground='#182c3d',font=('Segoe UI',10))
    style.configure('Title.TLabel',font=('Segoe UI',22,'bold'))
    style.configure('TButton',font=('Segoe UI',10),padding=(14,9))
    canvas = tk.Canvas(root, highlightthickness=0, background='#f3f5f7')
    scrollbar = ttk.Scrollbar(root, orient='vertical', command=canvas.yview)
    canvas.configure(yscrollcommand=scrollbar.set)
    scrollbar.pack(side='right', fill='y'); canvas.pack(side='left', fill='both', expand=True)
    frame = ttk.Frame(canvas, padding=18)
    item = canvas.create_window(0, 0, anchor='nw', window=frame)
    frame.bind('<Configure>', lambda e: canvas.configure(scrollregion=canvas.bbox('all')))
    canvas.bind('<Configure>', lambda e: canvas.itemconfigure(item, width=e.width))
    root.bind('<MouseWheel>', lambda e: canvas.yview_scroll(-int(e.delta/120), 'units'))
    ttk.Label(frame,text='Hunter Island',style='Title.TLabel').pack(anchor='w')
    translate(ttk.Label(frame), 'version', version=VERSION).pack(anchor='w',pady=(2,18))
    device = tk.StringVar(value=settings.get('device','ipad'))
    translate(ttk.Label(frame), 'resolution').pack(anchor='w')
    devices = ttk.Combobox(frame,textvariable=device,values=['ipad','iphone'],state='readonly',width=12)
    devices.pack(anchor='w',pady=12)
    ttk.Label(frame,text='iPad: 1024 × 768   ·   iPhone: 480 × 320',font=('Segoe UI',9)).pack(anchor='w')
    output, initial_hd = graphic_settings(settings)
    filter_choice = tk.StringVar(value=next((k for k,v in FILTERS.items() if v == output), 'Bilinear'))
    integer = tk.BooleanVar(value=settings.get('integer_scaling', False))
    fullscreen = tk.BooleanVar(value=settings.get('fullscreen', False))
    preset = tk.BooleanVar(value=output == 'anime4k' and initial_hd and not integer.get())
    preset_button = translate(ttk.Checkbutton(frame, variable=preset, name='recommended'), 'preset')
    preset_button.pack(anchor='w', pady=8)
    disclosure = translate(ttk.Button(frame, name='advanced'), 'advanced')
    disclosure.pack(anchor='w')
    advanced = ttk.Frame(frame)
    def toggle_advanced():
        if advanced.winfo_manager(): advanced.pack_forget()
        else: advanced.pack(before=status_label, anchor='w', fill='x', pady=8)
    disclosure.configure(command=toggle_advanced)
    status = tk.StringVar()
    status_label = ttk.Label(frame,textvariable=status,wraplength=580,justify='left')
    status_label.pack(anchor='w',fill='x',pady=(0,16))
    row = ttk.Frame(frame);row.pack(anchor='w',fill='x')
    import_button = translate(ttk.Button(row, name='import_ipa'), 'import')
    import_button.pack(side='left')
    start_button = translate(ttk.Button(row, name='play'), 'play')
    start_button.pack(side='left',padx=10)
    translate(ttk.Label(advanced), 'output_filter').pack(anchor='w')
    filters = ttk.Combobox(advanced, textvariable=filter_choice, values=list(FILTERS), state='readonly')
    filters.pack(anchor='w')
    mod_values = {}
    mod_controls = [preset_button, disclosure]
    for variable, key in [(integer,'integer_scaling'), (fullscreen,'fullscreen')]:
        box = translate(ttk.Checkbutton(advanced, variable=variable, command=lambda: save_controls()), key)
        box.pack(anchor='w'); mod_controls.append(box)
    translate(ttk.Label(advanced), 'language').pack(anchor='w',pady=(14,4))
    languages = ttk.Combobox(advanced, name='game_language', state='readonly', width=34)
    languages.pack(anchor='w')
    translate(ttk.Label(advanced, font=('Segoe UI',9), wraplength=650), 'language_hint').pack(anchor='w',pady=(3,0))

    def update_language(config):
        nonlocal language
        language = active_language(config)
        root.title('Hunter Island · ' + text('title'))
        languages.configure(values=[text('system') if code == 'system' else label for code, label in LANGUAGES])
        languages.current(LANGUAGE_CODES.index(selected_language(config)))
        for widget, key, values in localized:
            widget.configure(text=text(key, **values))
        update_recommendations()

    def save_language(_event):
        index = languages.current()
        if index < 0:
            return
        try:
            with store.lock():
                config = store.settings()
                config['language'] = LANGUAGE_CODES[index]
                atomic_json(store.config,config)
        except Exception as error:
            languages.current(LANGUAGE_CODES.index(selected_language(store.settings())))
            error_dialog('language_error', error)
            return
        update_language(config)
        refresh()

    languages.bind('<<ComboboxSelected>>',save_language)
    translate(ttk.Label(advanced), 'mods').pack(anchor='w',pady=(14,4))
    for flag, key in MOD_OPTIONS:
        value = tk.BooleanVar(value=initial_hd if flag == '--hunter-hd-textures' else settings.get('mods', {}).get(flag, False))
        control = translate(ttk.Checkbutton(advanced, variable=value, name=key, command=lambda f=flag: change_mod(f)), key)
        control.pack(anchor='w', pady=0)
        mod_values[flag] = value
        mod_controls.append(control)
    def choose_test(flag):
        if mod_values[flag].get():
            for other in TEST_PROFILES:
                if other != flag:
                    mod_values[other].set(False)
    for flag in TEST_PROFILES:
        mod_values[flag].trace_add('write',lambda *_args, flag=flag:choose_test(flag))
    translate(ttk.Label(advanced, wraplength=650, font=('Segoe UI',9)), 'collection_hint').pack(anchor='w',pady=(6,0))
    ttk.Separator(advanced).pack(fill='x',pady=18)
    export_button = translate(ttk.Button(advanced), 'export')
    export_button.pack(anchor='w')
    saves = tk.BooleanVar(value=False)
    save_check = translate(ttk.Checkbutton(advanced, variable=saves), 'include_save')
    save_check.pack(anchor='w',pady=8)
    translate(ttk.Label(advanced, wraplength=580, font=('Segoe UI',9)), 'privacy').pack(anchor='w',pady=(8,0))
    busy = False
    events = queue.Queue()
    def update_recommendations():
        for flag, key in MOD_OPTIONS:
            recommended = key in ('arena', 'expansion')
            if key == 'render_2x':
                width, height = (1024,768) if device.get() == 'ipad' else (480,320)
                recommended = root.winfo_screenwidth() >= width*2 and root.winfo_screenheight() >= height*2
            widget = advanced.nametowidget(key)
            widget.configure(text=text(key) + (' · ' + text('developer_recommended') if recommended else ''))

    def choose_campaign(config):
        if not mod_values['--zalibas-expansion'].get() or not store.needs_expansion_choice(config):
            return True
        answer = messagebox.askyesnocancel(text('expansion'), text('campaign_choice'), parent=root)
        if answer is None:
            return False
        config['expansion_origin'] = 'copy' if answer else 'new'
        return True
    def save_controls(_event=None, *, starting=False, expansion_changed=False):
        try:
            with store.lock():
                config = store.settings()
                if not choose_campaign(config):
                    mod_values['--zalibas-expansion'].set(False)
                    return False
                saved_mods = dict(config.get('mods', {}))
                for flag in mod_values:
                    if starting or flag in ('--hunter-hd-textures', '--scale-hack=2') or (expansion_changed and flag == '--zalibas-expansion'):
                        saved_mods[flag] = mod_values[flag].get()
                config.update(device=device.get(), mods=saved_mods,
                              output_filter=FILTERS[filter_choice.get()], integer_scaling=integer.get(),
                              fullscreen=fullscreen.get())
                atomic_json(store.config, config)
            preset.set(config['output_filter']=='anime4k' and mod_values['--hunter-hd-textures'].get() and not integer.get())
            update_recommendations()
            return True
        except Exception as error:
            error_dialog('start_error',error)
            return False
    def change_mod(flag):
        if flag in ('--zalibas-expansion', '--hunter-hd-textures', '--scale-hack=2'):
            save_controls(expansion_changed=flag == '--zalibas-expansion')
    def apply_preset():
        filter_choice.set('Anime4K CNN x2 S' if preset.get() else 'Bilinear')
        mod_values['--hunter-hd-textures'].set(preset.get())
        if preset.get(): integer.set(False)
        save_controls()
    preset_button.configure(command=apply_preset)
    devices.bind('<<ComboboxSelected>>', save_controls)
    filters.bind('<<ComboboxSelected>>', save_controls)
    def refresh():
        game = store.settings().get('game')
        if not busy:
            status.set(text('ready', version=game['version']) if game else text('need_ipa'))
            start_button.configure(state='normal' if game else 'disabled')
    def background(action, done, error_key):
        nonlocal busy
        busy = True
        for widget in (import_button,start_button,export_button,save_check,devices,languages,filters,*mod_controls):
            widget.configure(state='disabled')
        def worker():
            try:
                events.put((done, action(), None, error_key))
            except Exception as error:
                events.put((done, None, error, error_key))
        threading.Thread(target=worker,daemon=False).start()
    def poll():
        nonlocal busy
        try:
            done, result, error, error_key = events.get_nowait()
        except queue.Empty:
            pass
        else:
            busy = False
            for widget in (import_button,start_button,export_button,save_check,*mod_controls):
                widget.configure(state='normal')
            filters.configure(state='readonly')
            devices.configure(state='readonly')
            languages.configure(state='readonly')
            refresh()
            if error:
                error_dialog(error_key, error)
            else:
                done(result)
        root.after(150,poll)
    def choose_ipa():
        source = filedialog.askopenfilename(title=text('choose_ipa'), filetypes=[(text('ios_app'), '*.ipa')], parent=root)
        if source:
            status.set(text('importing'))
            def imported(info):
                if info['compatibility'] != 'tested-version':
                    messagebox.showwarning(text('untested'), text('untested_body', version=info['version']), parent=root)
            background(lambda: store.import_ipa(source), imported, 'import_error')
    def start():
        if not save_controls(starting=True): return
        status.set(text('running'))
        def stopped(result):
            if result['exit_code']:
                messagebox.showwarning(text('game_closed'), text('game_error'), parent=root)
        background(lambda: play(store, release), stopped, 'start_error')
    def export():
        include = saves.get()
        if include and not messagebox.askyesno(text('save_confirm'), text('save_confirm_body'), parent=root):
            return
        path = filedialog.asksaveasfilename(title=text('export'), defaultextension='.zip', initialfile='HunterIsland-Diagnostics.zip', filetypes=[('ZIP','*.zip')], parent=root)
        if path:
            background(lambda: diagnostics.export(store, path, include),
                       lambda _: messagebox.showinfo(text('saved'), text('saved_body'), parent=root),
                       'export_error')
    def close():
        if busy:
            messagebox.showinfo(text('busy'), text('busy_body'), parent=root)
        else:
            root.destroy()
    import_button.configure(command=choose_ipa)
    start_button.configure(command=start)
    export_button.configure(command=export)
    root.protocol('WM_DELETE_WINDOW',close)
    update_language(settings)
    refresh()
    poll()
    root.mainloop()

def main(argv=None):
    parser = argparse.ArgumentParser(description='Local Hunter Island tester launcher')
    parser.add_argument('--data-root',type=Path,help='Separate local QA profile; defaults to LOCALAPPDATA')
    parser.add_argument('--release-root',type=Path,help='Developer runtime package location')
    group = parser.add_mutually_exclusive_group()
    group.add_argument('--inspect-ipa',type=Path)
    group.add_argument('--import-ipa',type=Path)
    group.add_argument('--run',action='store_true')
    group.add_argument('--diagnostics',type=Path)
    parser.add_argument('--include-saves',action='store_true')
    parser.add_argument('--max-seconds',type=int,choices=range(5,601),metavar='5..600')
    parser.add_argument('--qa-tap',action='append',default=[])
    parser.add_argument('--qa-capture',action='store_true',help=argparse.SUPPRESS)
    parser.add_argument('--result-json',type=Path,help='Local CLI QA result file')
    args = parser.parse_args(argv)
    if args.inspect_ipa:
        result = ipa.inspect(args.inspect_ipa)
    else:
        store = Store(args.data_root)
        release = (args.release_root or release_root()).resolve()
        if store.root.is_relative_to(release):
            raise ValueError('User data must be outside the program directory')
        if args.import_ipa:
            result = store.import_ipa(args.import_ipa)
        elif args.run:
            result = play(store,release,args.max_seconds,args.qa_tap,args.qa_capture)
        elif args.diagnostics:
            from .diagnostics import export
            result = dict(export=str(export(store,args.diagnostics,args.include_saves)))
        else:
            gui(store,release)
            return
    if args.result_json:
        atomic_json(args.result_json,result)
    if sys.stdout:
        print(json.dumps(result,indent=2,ensure_ascii=False))

if __name__ == '__main__':
    main()
