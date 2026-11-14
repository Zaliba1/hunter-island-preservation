# SPDX-License-Identifier: MPL-2.0
from hunter_launcher.app import main

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        import sys
        if sys.stderr is None:
            from tkinter import messagebox
            messagebox.showerror('Hunter Island',str(error))
        else:
            raise
