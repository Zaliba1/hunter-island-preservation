# SPDX-License-Identifier: MPL-2.0
"""Map every shipped executable/library/font; fail on unknown compiled inputs."""
from pathlib import Path
import hashlib,json

GUEST={'libgcc_s.1.dylib':'GCC-5664-with-runtime-exception',
       'libstdc++.6.0.9.dylib':'libstdcxx-39-with-linking-exception',
       'libz.1.2.3.dylib':'zlib-23.0.1','libsqlite3.dylib':'SQLite-3.6.10',
       'libxml2.2.dylib':'libxml2-22.3'}
WINDOWS_LAUNCHER_BINARIES = {
    'python311.dll': 'CPython-3.11.9',
    'tcl86t.dll': 'Tcl-8.6',
    'tk86t.dll': 'Tk-8.6',
    'libffi-8.dll': 'libffi',
    'vcruntime140.dll': 'Microsoft-VC-runtime-from-CPython',
}
def inventory(root):
    root=Path(root);files=[]
    for path in sorted(root.rglob('*')):
        if not path.is_file() or path.suffix.lower() not in {'.exe','.dll','.pyd','.dylib','.ttf','.otf'}:continue
        name=path.name.lower();rel=path.relative_to(root).as_posix();component=None
        if name.startswith('api-ms-win-') or name=='ucrtbase.dll':
            raise ValueError('Windows OS component must not be copied from the build machine: '+rel)
        if rel in {'runtime/touchHLE.exe','runtime/hunter-history.exe'}:component='touchHLE-and-static-dependencies-with-GNU-runtime'
        elif rel=='runtime/isolation-probe.exe':component='launcher'
        elif rel=='HunterIsland-Launcher.exe':component='launcher-and-PyInstaller-bootloader'
        elif path.parent==root/'_internal':
            if path.suffix.lower()=='.pyd':component='CPython-3.11.9-standard-library-extension'
            else:component=WINDOWS_LAUNCHER_BINARIES.get(name)
        elif rel.startswith('runtime/touchHLE_dylibs/') and path.name in GUEST:component=GUEST[path.name]
        elif rel.startswith('runtime/touchHLE_fonts/') and name.startswith('liberation'):component='Liberation-fonts'
        elif rel.startswith('runtime/touchHLE_fonts/') and name.startswith('notosans'):component='Noto-fonts'
        if component is None:raise ValueError('Unclassified compiled component: '+rel)
        record=dict(path=rel,component=component,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),bytes=path.stat().st_size)
        if path.suffix.lower() in {'.exe','.dll','.pyd'}:
            import pefile
            pe=pefile.PE(str(path))
            record['imports']=[entry.dll.decode('ascii') for entry in getattr(pe,'DIRECTORY_ENTRY_IMPORT',[])]
            for group in getattr(pe,'FileInfo',[]):
                for entry in group:
                    for table in getattr(entry,'StringTable',[]):
                        for key,value in table.entries.items():
                            if key in {b'FileVersion',b'ProductVersion',b'CompanyName'}:
                                record[key.decode()]=value.decode('utf-8',errors='replace')
            pe.close()
        files.append(record)
    return dict(schema='hunter-license-inventory-v1',compiled_and_font_files=files,
        source_delivery='Matching Source and SourceDependencies ZIPs accompany the Windows ZIP; see SOURCE.md and LICENSES.md.',
        license_scope='Per-component terms apply. MPL-2.0 covers project files; third-party components retain their own terms.')

if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('root',type=Path);p.add_argument('--out',type=Path,required=True);a=p.parse_args()
    a.out.write_text(json.dumps(inventory(a.root),indent=2)+'\n')
