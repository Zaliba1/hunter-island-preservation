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
from .language import LANGUAGES, language_arguments, selected_language

MOD_OPTIONS = (
    ("--hunter-rare-dragons", "Seltene Startdrachen häufiger finden (5 % pro Wildkampf)"),
    ("--hunter-silver-exchange", "Silber gegen Gold tauschen (50:1)"),
    ("--hunter-no-arena-cooldown", "Kolosseum ohne 12-Stunden-Wartezeit"),
    ("--hunter-test-save", "Testspielstand erstellen / verwenden (separate Kopie)"),
)

def mod_arguments(settings):
    mods = settings.get("mods", {})
    if not isinstance(mods, dict):
        raise ValueError("Ungültige Mod-Einstellungen")
    if any(not isinstance(value, bool) for value in mods.values()):
        raise ValueError("Mod-Einstellungen müssen Wahrheitswerte sein")
    return [flag for flag, _ in MOD_OPTIONS if mods.get(flag, False)]

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
            if capture:
                args.append('--capture-frames')
            for tap in test_taps:
                if not __import__('re').fullmatch(r'\d+,\d+,\d+',tap):
                    raise ValueError('Ungültige QA-Eingabe')
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
    root = tk.Tk()
    root.title('Hunter Island · Testlauncher')
    root.geometry('760x830')
    root.minsize(710,800)
    root.configure(bg='#f3f5f7')
    style = ttk.Style(root)
    style.theme_use('clam')
    style.configure('TFrame',background='#f3f5f7')
    style.configure('TLabel',background='#f3f5f7',foreground='#182c3d',font=('Segoe UI',10))
    style.configure('Title.TLabel',font=('Segoe UI',22,'bold'))
    style.configure('TButton',font=('Segoe UI',10),padding=(14,9))
    frame = ttk.Frame(root,padding=28)
    frame.pack(fill='both',expand=True)
    ttk.Label(frame,text='Hunter Island',style='Title.TLabel').pack(anchor='w')
    ttk.Label(frame,text=f'Windows-Testversion {VERSION} · touchHLE').pack(anchor='w',pady=(2,18))
    status = tk.StringVar()
    ttk.Label(frame,textvariable=status,wraplength=580,justify='left').pack(anchor='w',fill='x',pady=(0,16))
    row = ttk.Frame(frame);row.pack(anchor='w',fill='x')
    import_button = ttk.Button(row,text='Eigene IPA importieren …')
    import_button.pack(side='left')
    start_button = ttk.Button(row,text='Spiel starten')
    start_button.pack(side='left',padx=10)
    device = tk.StringVar(value=store.settings().get('device','ipad'))
    devices = ttk.Combobox(frame,textvariable=device,values=['ipad','iphone'],state='readonly',width=12)
    devices.pack(anchor='w',pady=12)
    ttk.Label(frame,text='iPad: 1024 × 768   ·   iPhone: 480 × 320',font=('Segoe UI',9)).pack(anchor='w')
    mod_values = {}
    mod_controls = []
    settings = store.settings()
    mod_arguments(settings)
    ttk.Label(frame,text='Spiel-Sprache / Game language').pack(anchor='w',pady=(14,4))
    languages = ttk.Combobox(frame,name='game_language',values=[label for _, label in LANGUAGES],state='readonly',width=34)
    languages.current([code for code, _ in LANGUAGES].index(selected_language(settings)))
    languages.pack(anchor='w')
    ttk.Label(frame,text='Wird gespeichert und beim nächsten Spielstart verwendet.',font=('Segoe UI',9)).pack(anchor='w',pady=(3,0))
    def save_language(_event):
        try:
            with store.lock():
                config = store.settings()
                config['language'] = LANGUAGES[languages.current()][0]
                atomic_json(store.config,config)
        except Exception as error:
            languages.current([code for code, _ in LANGUAGES].index(selected_language(store.settings())))
            messagebox.showerror('Sprache konnte nicht gespeichert werden',str(error),parent=root)
    languages.bind('<<ComboboxSelected>>',save_language)
    ttk.Label(frame,text='Optionale Mods · gelten ab dem nächsten Spielstart').pack(anchor='w',pady=(14,4))
    for flag, label in MOD_OPTIONS:
        value = tk.BooleanVar(value=settings.get('mods', {}).get(flag, False))
        control = ttk.Checkbutton(frame, text=label, variable=value)
        control.pack(anchor='w', pady=2)
        mod_values[flag] = value
        mod_controls.append(control)
    ttk.Label(frame,text='Testspielstand: Beim ersten Start wird dein normaler Stand kopiert und erweitert.\nHaken entfernen = normal weiterspielen. Die Testkopie bleibt erhalten.',
        wraplength=650,font=('Segoe UI',9)).pack(anchor='w',pady=(6,0))
    ttk.Separator(frame).pack(fill='x',pady=18)
    export_button = ttk.Button(frame,text='Diagnosepaket speichern …')
    export_button.pack(anchor='w')
    saves = tk.BooleanVar(value=False)
    save_check = ttk.Checkbutton(frame,text='Meinen Spielstand ausdrücklich beilegen',variable=saves)
    save_check.pack(anchor='w',pady=8)
    ttk.Label(frame,text='Alles bleibt lokal. Es gibt keine automatischen Uploads.\nNeue Profile beginnen ohne Spielstand.',wraplength=580,font=('Segoe UI',9)).pack(anchor='w',pady=(8,0))
    busy = False
    events = queue.Queue()
    def refresh():
        game = store.settings().get('game')
        if not busy:
            status.set(('Hunter Island '+game['version']+' ist importiert. Bereit zum Spielen.' if game else
                'Importiere deine eigene, bereits entschlüsselte Hunter-Island-IPA. Bestätigt getestet ist Version 2.02.'))
            start_button.configure(state='normal' if game else 'disabled')
    def background(action,done):
        nonlocal busy
        busy = True
        for widget in (import_button,start_button,export_button,save_check,devices,languages,*mod_controls):
            widget.configure(state='disabled')
        def worker():
            try:
                events.put((done,action(),None))
            except Exception as error:
                events.put((done,None,error))
        threading.Thread(target=worker,daemon=False).start()
    def poll():
        nonlocal busy
        try:
            done,result,error = events.get_nowait()
        except queue.Empty:
            pass
        else:
            busy = False
            for widget in (import_button,start_button,export_button,save_check,*mod_controls):
                widget.configure(state='normal')
            devices.configure(state='readonly')
            languages.configure(state='readonly')
            refresh()
            if error:
                messagebox.showerror('Vorgang fehlgeschlagen',str(error),parent=root)
            else:
                done(result)
        root.after(150,poll)
    def choose_ipa():
        source = filedialog.askopenfilename(title='Eigene Hunter-Island-IPA wählen',filetypes=[('iOS-App','*.ipa')],parent=root)
        if source:
            status.set('IPA wird kopiert und statisch geprüft …')
            def imported(info):
                if info['compatibility'] != 'tested-version':
                    messagebox.showwarning('Ungetestete Spielversion','Importiert: '+info['version']+'. Bestätigt getestet ist bisher nur 2.02.',parent=root)
            background(lambda:store.import_ipa(source),imported)
    def start():
        config = store.settings();config['device']=device.get()
        config['mods'] = {flag: value.get() for flag, value in mod_values.items()}
        try:
            with store.lock():
                atomic_json(store.config,config)
        except Exception as error:
            messagebox.showerror('Profil wird verwendet',str(error),parent=root)
            return
        status.set('Spiel läuft. Schließe das Spielfenster, um zum Launcher zurückzukehren.')
        def stopped(result):
            if result['exit_code']:
                messagebox.showwarning('Spiel beendet','Das Spiel wurde mit einem Fehler beendet. Du kannst jetzt ein Diagnosepaket speichern.',parent=root)
        background(lambda:play(store,release),stopped)
    def export():
        include = saves.get()
        if include and not messagebox.askyesno('Spielstand beilegen','Der Export enthält dann deine vollständigen Gast-/Spielstanddaten. Möchtest du sie ausdrücklich beilegen?',parent=root):
            return
        path = filedialog.asksaveasfilename(title='Lokales Diagnosepaket speichern',defaultextension='.zip',initialfile='HunterIsland-Diagnose.zip',filetypes=[('ZIP','*.zip')],parent=root)
        if path:
            background(lambda:diagnostics.export(store,path,include),lambda _:messagebox.showinfo('Gespeichert','Das Paket wurde lokal gespeichert. Es wird nicht automatisch versendet.',parent=root))
    def close():
        if busy:
            messagebox.showinfo('Vorgang läuft','Bitte zuerst das Spielfenster schließen bzw. den Vorgang abwarten. Du kannst dieses Fenster minimieren.',parent=root)
        else:
            root.destroy()
    import_button.configure(command=choose_ipa)
    start_button.configure(command=start)
    export_button.configure(command=export)
    root.protocol('WM_DELETE_WINDOW',close)
    refresh();poll();root.mainloop()

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
            raise ValueError('Benutzerdaten müssen außerhalb des Programm-/Quellverzeichnisses liegen')
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
