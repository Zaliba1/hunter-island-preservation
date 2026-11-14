# SPDX-License-Identifier: MPL-2.0
"""Stage only explicitly built native libraries and original toolchain notices."""
from pathlib import Path
import argparse,hashlib,json,shutil,subprocess
p=argparse.ArgumentParser();p.add_argument('--target-dir',type=Path,required=True);p.add_argument('--sdk-root',type=Path,required=True);a=p.parse_args()
root=Path(__file__).resolve().parents[1];target=a.target_dir.resolve()/'aarch64-linux-android/release'
ndk=a.sdk_root.resolve()/'ndk/28.2.13676358';toolchain=ndk/'toolchains/llvm/prebuilt/windows-x86_64'
dest=root/'emulator/android/app/jniLibs/arm64-v8a';dest.mkdir(parents=True,exist_ok=True)
sdl=list(target.glob('build/sdl2-sys-*/out/lib/libSDL2.so'))
if len(sdl)!=1:raise ValueError('Expected exactly one SDL output; use a dedicated Android target directory')
records=[]
for name,path in [('libtouchHLE.so',target/'libtouchHLE.so'),('libSDL2.so',sdl[0]),('libc++_shared.so',toolchain/'sysroot/usr/lib/aarch64-linux-android/libc++_shared.so')]:
 output=dest/name;shutil.copyfile(path,output)
 subprocess.run([str(toolchain/'bin/llvm-strip.exe'),'--strip-unneeded',str(output)],check=True)
 records.append(dict(file=name,sha256=hashlib.sha256(output.read_bytes()).hexdigest(),bytes=output.stat().st_size))
notice=root/'licenses/android-ndk';notice.mkdir(exist_ok=True)
for name,path in [('NDK-NOTICE.txt',ndk/'NOTICE'),('NDK-TOOLCHAIN-NOTICE.txt',ndk/'NOTICE.toolchain'),('LLVM-NOTICE.txt',toolchain/'NOTICE')]:
 if path.is_file():shutil.copyfile(path,notice/name)
(root/'android-runtime-provenance.json').write_text(json.dumps(dict(ndk='28.2.13676358',libraries=records,symbols_stripped=True),indent=2)+'\n')
print('Three explicit ARM64 libraries staged and stripped; no user data or signing material copied.')
