# SPDX-License-Identifier: MPL-2.0
"""Read-only local save inspector UI. No uploads, account or stored paths."""
from pathlib import Path
import json
import subprocess
import threading
import tkinter as tk
from tkinter import filedialog, messagebox, ttk


def main():
    window = tk.Tk()
    window.title('Hunter Island - Save History Inspector')
    window.geometry('780x640')
    frame = ttk.Frame(window, padding=18)
    frame.pack(fill='both', expand=True)
    ttk.Label(frame, text='Inspect a game save', font=('Segoe UI', 17, 'bold')).pack(anchor='w')
    ttk.Label(frame, text='Reads your own IPA and save locally. Neither file is changed or uploaded.').pack(anchor='w', pady=(6, 16))
    selections = {}
    labels = {}

    def choose(kind):
        options = dict(title='Choose your Hunter Island 2.02 IPA' if kind == 'ipa' else 'Choose a game save (sv_...)')
        if kind == 'ipa':
            options['filetypes'] = [('iOS game archive', '*.ipa'), ('All files', '*.*')]
        path = filedialog.askopenfilename(parent=window, **options)
        if path:
            selections[kind] = path
            labels[kind].configure(text=Path(path).name)

    for kind, title in [('ipa', 'Choose your IPA...'), ('save', 'Choose a save...')]:
        row = ttk.Frame(frame)
        row.pack(fill='x', pady=4)
        ttk.Button(row, text=title, command=lambda k=kind: choose(k)).pack(side='left')
        labels[kind] = ttk.Label(row, text='No file selected')
        labels[kind].pack(side='left', padx=12)

    result = tk.Text(frame, wrap='word', font=('Consolas', 10))
    result.pack(fill='both', expand=True, pady=12)
    report = {}

    def show(data):
        report.clear()
        report.update(data)
        lines = ['Status: ' + str(data.get('status', 'unknown')), '',
                 'Local history is not authenticated anti-cheat proof.',
                 'Missing history or unknown earlier play must not be treated as a clean run.', '']
        for name, used in data.get('ever_enabled', {}).items():
            lines.append(name.replace('_', ' ') + ': ' + ('ever enabled' if used else 'no activation recorded'))
        lines.append('')
        for name, value in data.get('counts', {}).items():
            lines.append(f'{name.replace("_", " ")}: {value}')
        lines += ['', 'Full report:', json.dumps(data, indent=2)]
        result.delete('1.0', 'end')
        result.insert('end', '\n'.join(lines))

    def inspect():
        if set(selections) != {'ipa', 'save'}:
            messagebox.showinfo('Select both files', 'Choose your IPA and the game save first.', parent=window)
            return
        here = Path(__file__).resolve().parent
        candidates = [here / 'hunter-history.exe', here.parent / 'runtime/hunter-history.exe']
        executable = next((p for p in candidates if p.is_file()), None)
        if executable is None:
            messagebox.showerror('Inspector not found', 'Place hunter-history.exe beside this tool.', parent=window)
            return
        button.configure(state='disabled')
        paths = dict(selections)

        def completed(data=None, error=None):
            button.configure(state='normal')
            if error:
                messagebox.showerror('Could not inspect the save', error, parent=window)
            else:
                show(data)

        def work():
            try:
                process = subprocess.run([str(executable), '--ipa', paths['ipa'], '--save', paths['save']],
                                         capture_output=True, text=True, encoding='utf-8', timeout=60,
                                         creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
                if process.returncode:
                    raise ValueError(process.stderr.strip() or 'The inspector could not read these files.')
                data = json.loads(process.stdout)
                window.after(0, lambda: completed(data=data))
            except Exception as error:
                message = str(error)
                window.after(0, lambda: completed(error=message))
        threading.Thread(target=work, daemon=True).start()

    def export():
        if not report:
            return
        selected = filedialog.asksaveasfilename(parent=window, title='Export report to a new file',
                                               defaultextension='.json', initialfile='HunterIsland-history.json')
        if selected:
            try:
                with Path(selected).open('x', encoding='utf-8') as stream:
                    json.dump(report, stream, indent=2)
            except Exception as error:
                messagebox.showerror('Report was not exported', str(error) + '\nPlease choose a new filename.', parent=window)

    row = ttk.Frame(frame)
    row.pack(fill='x')
    button = ttk.Button(row, text='Inspect save', command=inspect)
    button.pack(side='left')
    ttk.Button(row, text='Export report...', command=export).pack(side='left', padx=8)
    ttk.Label(frame, text='The imported save may predate tracking. No prior mod use can be inferred from missing metadata.',
              wraplength=730).pack(anchor='w', pady=(12, 0))
    window.mainloop()


if __name__ == '__main__':
    main()
