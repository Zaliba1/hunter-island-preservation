# SPDX-License-Identifier: MPL-2.0
"""Inventory the actual APK bytes, retaining source and notice mappings."""
from pathlib import Path
import argparse,hashlib,json,zipfile

def sha(data):return hashlib.sha256(data).hexdigest()

def inventory(apk,root):
    root=Path(root);records=[]
    runtime=json.loads((root/'android-runtime-provenance.json').read_text())
    native={r['file']:r for r in runtime['libraries']}
    guest={r['sha256']:r for r in json.loads((root/'guest-library-provenance.json').read_text())}
    mapping={
        'libtouchHLE.so':('touchHLE and statically incorporated dependencies','GPL-3.0-or-later',
            ['licenses/GPL-3.0.txt','licenses/MPL-2.0.txt','licenses/OpenAL-Soft-LGPL-2.0.txt','licenses/rust-standard-library/COPYRIGHT-library.html'],
            'emulator/ plus matching native/Rust dependency archives; per-file terms retained'),
        'libSDL2.so':('SDL','Zlib',['licenses/native/SDL/LICENSE.txt'],'native/SDL.zip plus android-sdl-patch.json and tools/patch_android_sdl.py'),
        'libc++_shared.so':('LLVM libc++ runtime','Apache-2.0 WITH LLVM-exception',
            ['licenses/android-ndk/NDK-NOTICE.txt','licenses/android-ndk/LLVM-NOTICE.txt'],'Android NDK 28.2.13676358 runtime; original toolchain notices supplied'),
    }
    with zipfile.ZipFile(apk) as z:
        if len(z.namelist())!=len(set(z.namelist())) or z.testzip():raise ValueError('Duplicate or corrupt APK member')
        for info in z.infolist():
            name=info.filename;data=z.read(info);suffix=Path(name).suffix.lower()
            if suffix not in {'.so','.dylib','.ttf','.otf','.dex','.jar','.class','.a','.o','.exe','.dll'}:continue
            record=dict(path=name,sha256=sha(data),bytes=len(data))
            if suffix=='.so':
                filename=Path(name).name
                if name!='lib/arm64-v8a/'+filename or filename not in mapping:raise ValueError('Unknown native library: '+name)
                if sha(data)!=native[filename]['sha256']:raise ValueError('Native provenance mismatch: '+name)
                component,license,notices,source=mapping[filename]
            elif suffix=='.dylib':
                origin=guest.get(sha(data))
                if not name.startswith('assets/touchHLE_dylibs/') or origin is None:raise ValueError('Unknown guest library: '+name)
                source=origin['source_archive'];license=origin['license'];component='Guest '+Path(origin['path']).name
                notices=['licenses/touchHLE_dylibs/README.md'];record['exception']=origin['exception']
                record['independently_bit_reproduced']=False
            elif suffix in {'.ttf','.otf'}:
                filename=Path(name).name
                if not name.startswith('assets/touchHLE_fonts/') or data!=(root/'emulator/touchHLE_fonts'/filename).read_bytes():raise ValueError('Unknown/changed font: '+name)
                if filename.startswith('Liberation'):family='liberation'
                elif filename.startswith('NotoSansJP'):family='noto'
                else:raise ValueError('Unclassified font: '+name)
                component=family+' fonts';license='OFL-1.1';source='emulator/touchHLE_fonts/'+filename
                notices=['licenses/touchHLE_fonts/LICENSE.'+family]
            elif suffix=='.dex' and name.startswith('classes') and '/' not in name:
                component='Android Java launcher, SDL Java integration and generated Android glue'
                license='MPL-2.0 AND Zlib';source='emulator/android/app/src/ and native/SDL.zip/android-project/app/src/main/java/'
                notices=['licenses/MPL-2.0.txt','licenses/native/SDL/LICENSE.txt']
                if any(marker in data for marker in (b'Lorg/robolectric/',b'Lorg/junit/',b'Landroidx/test/')):raise ValueError('Host test framework present in production DEX')
            else:raise ValueError('Unclassified compiled component: '+name)
            for notice in notices:
                expected=(root/notice).read_bytes()
                if z.read('assets/preservation-notices/'+notice)!=expected:raise ValueError('Missing/changed notice: '+notice)
            record.update(component=component,license=license,notices=notices,source=source);records.append(record)
        if {Path(x['path']).name for x in records if x['path'].endswith('.so')}!=set(mapping):raise ValueError('Incomplete native library set')
        # Verify the Android notice set; other-platform references stay in Source.
        notes=[]
        for path in sorted((root/'licenses').rglob('*')):
            if path.is_file():
                rel=path.relative_to(root).as_posix();data=path.read_bytes()
                if rel.startswith(('licenses/runtime/','licenses/gnu-runtime/')) or rel in {'licenses/tcl8.6.txt','licenses/tk8.6.txt'}:continue
                if z.read('assets/preservation-notices/'+rel)!=data:raise ValueError('Notice bytes differ: '+rel)
                notes.append(dict(path=rel,sha256=sha(data),bytes=len(data)))
        for name,source in [('LICENSES.md','ANDROID-LICENSES.md'),('SOURCE.md','ANDROID-SOURCE.md')]:
            if z.read('assets/preservation-notices/'+name)!=(root/source).read_bytes():raise ValueError('Outdated APK source/license summary')
    return dict(schema='hunter-android-license-inventory-v1',apk_sha256=sha(Path(apk).read_bytes()),
        binary_distribution_license='GPL-3.0-or-later',source_license_scope='MPL-2.0 project files and original per-component terms; no new use restrictions.',
        compiled_and_font_files=records,notice_files=notes,
        rust_dependency_inventory='android-rust-dependencies.json',system_libraries='Android platform libraries are dynamically referenced and are not copied into this APK.',
        build_only_dependencies='AGP, Gradle, JDK, SDK, NDK and Robolectric/JUnit are separately obtained build/test tools; only the explicitly listed NDK runtime is shipped.',
        historical_guest_binary_limit='Pinned upstream hashes and published source releases supplied; historical Apple guest SDK binaries were not independently bit-reproduced.')

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('apk',type=Path);p.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1]);p.add_argument('--out',type=Path,required=True);a=p.parse_args()
    a.out.write_text(json.dumps(inventory(a.apk,a.root),indent=2)+'\n')
