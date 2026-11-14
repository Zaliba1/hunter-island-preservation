# SPDX-License-Identifier: MPL-2.0
"""Use the user's installed Python; no interpreter/runtime is redistributed."""
from pathlib import Path
import os
import struct
import subprocess
import sys

def main():
    if os.name != 'nt' or sys.version_info < (3, 11) or struct.calcsize('P') != 8:
        raise RuntimeError('Windows und Python 3.11+ in 64 Bit werden benoetigt.')
    import tkinter  # Fail visibly before starting a windowless child.
    entry = Path(__file__).with_name('launch.py')
    # Keep CLI calls synchronous for diagnostics and reproducible tests.
    if len(sys.argv) > 1:
        return subprocess.call([sys.executable, '-E', '-s', str(entry), *sys.argv[1:]])
    windowed = Path(sys.executable).with_name('pythonw.exe')
    if not windowed.is_file():
        raise RuntimeError('pythonw.exe fehlt. Bitte die offizielle Python-Installation mit Tk verwenden.')
    subprocess.Popen([str(windowed), '-E', '-s', str(entry)], cwd=entry.parent)
    return 0

if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except Exception as error:
        print('Hunter Island konnte nicht gestartet werden: ' + str(error), file=sys.stderr)
        print('Voraussetzung: offizielles Python 3.11+ (64 Bit) mit Tcl/Tk. Siehe README.md.', file=sys.stderr)
        raise SystemExit(1)
