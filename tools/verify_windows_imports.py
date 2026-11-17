# SPDX-License-Identifier: MPL-2.0
"""Check each executable's shipped DLL context; Windows/NET are prerequisites."""
from pathlib import Path
import json
OS={name+'.dll' for name in '''kernel32 kernelbase ntdll ole32 oleaut32 advapi32 shell32 user32 gdi32 gdi32full winmm imm32 version setupapi bcrypt bcryptprimitives ws2_32 mscoree msvcrt ucrtbase rpcrt4 comdlg32 comctl32 shlwapi crypt32 cryptbase ncrypt secur32 iphlpapi dnsapi netapi32 normaliz userenv uxtheme dwmapi dwrite opengl32 glu32 winspool msimg32 usp10 powrprof dbghelp wintrust psapi wtsapi32 cfgmgr32 shcore propsys'''.split()}
def verify(root,inventory):
    root=Path(root);missing=[];resolved=0
    for record in inventory['compiled_and_font_files']:
        rel=Path(record['path'])
        for name in record.get('imports',[]):
            lower=name.lower()
            if lower in OS or lower.startswith(('api-ms-win-','ext-ms-win-')):continue
            context=root/'runtime' if rel.parts[0]=='runtime' else root/'_internal/bin'
            if not context.is_dir() or not any(p.is_file() and p.name.lower()==lower for p in context.iterdir()):
                missing.append(dict(file=record['path'],missing=name))
            else:resolved+=1
    if missing:raise ValueError('Missing non-OS DLL dependencies: '+json.dumps(missing))
    return dict(non_os_imports_resolved=resolved,missing_non_os_imports=[],windows_and_net_prerequisites=True)
