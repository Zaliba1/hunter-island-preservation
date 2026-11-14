# SPDX-License-Identifier: MPL-2.0
from pathlib import Path,PurePosixPath
import hashlib,json,os,re,shutil,struct,subprocess,zipfile
import argparse
parser=argparse.ArgumentParser()
parser.add_argument('--apk',type=Path,required=True)
parser.add_argument('--sdk-root',type=Path,required=True)
parser.add_argument('--java-home',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
args=parser.parse_args()
source=Path(__file__).resolve().parents[1]
sdk=args.sdk_root.resolve()
buildtools=sdk/'build-tools/35.0.0'
apk=args.apk.resolve()
env=dict(os.environ,JAVA_HOME=str(args.java_home.resolve()))
version=json.loads((source/'android-version.json').read_text())['version']
verify=subprocess.run([str(buildtools/'apksigner.bat'),'verify','--verbose','--print-certs',str(apk)],env=env,capture_output=True,text=True,check=True)
assert 'CN=Hunter Island Preview' in verify.stdout
align=subprocess.run([str(buildtools/'zipalign.exe'),'-c','-P','16','4',str(apk)],capture_output=True,text=True,check=True)
badging=subprocess.check_output([str(buildtools/'aapt2.exe'),'dump','badging',str(apk)],text=True,errors='replace')
assert "package: name='org.hunterislandpreservation.android'" in badging
assert ("minSdkVersion:'26'" in badging or "sdkVersion:'26'" in badging) and "targetSdkVersion:'35'" in badging
assert 'android.permission.INTERNET' not in badging and 'android.permission.READ_EXTERNAL_STORAGE' not in badging and 'android.permission.WRITE_EXTERNAL_STORAGE' not in badging
markers=[os.environ.get('USERNAME',''),os.environ.get('USERPROFILE','')]
home=os.environ.get('USERPROFILE','');markers += [home.replace('\\','/'),home.replace('\\','\\\\')]
patterns=[m.lower().encode(enc) for m in markers if len(m)>3 for enc in ('utf-8','utf-16-le')]
runtime={};readelf=sdk/'ndk/28.2.13676358/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-readelf.exe'
oslibs={'libc.so','libm.so','libdl.so','liblog.so','libandroid.so','libOpenSLES.so','libEGL.so','libGLESv1_CM.so','libGLESv2.so','libz.so','libaaudio.so'}
qa=args.out.resolve()/'static';qa.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(apk) as archive:
 assert archive.testzip() is None
 for info in archive.infolist():
  name=PurePosixPath(info.filename)
  assert not name.is_absolute() and '..' not in name.parts and '\\' not in info.filename
  assert name.suffix.lower() not in {'.ipa','.p12','.jks','.keystore','.partial','.pdb'}
  assert not any(p in name.parts for p in ('touchHLE_sandbox','games','profiles','backups','logs'))
  data=archive.read(info);lower=data.lower()
  url=b'https://github.com/zaliba1/hunter-island-preservation'
  lower=lower.replace(url,b'https://github.com/project-owner/project')
  lower=lower.replace(url.decode().encode('utf-16-le'),'https://github.com/project-owner/project'.encode('utf-16-le'))
  assert not any(pattern in lower for pattern in patterns),('Known private marker',info.filename)
  if name.suffix=='.so':
   assert name.parts[:2]==('lib','arm64-v8a') and len(name.parts)==3
   assert data[:6]==b'\x7fELF\x02\x01' and struct.unpack_from('<H',data,18)[0]==183
   phoff=struct.unpack_from('<Q',data,32)[0];entsize,count=struct.unpack_from('<HH',data,54)
   loads=[]
   for i in range(count):
    kind,flags,offset,vaddr,paddr,filesz,memsz,alignment=struct.unpack_from('<IIQQQQQQ',data,phoff+i*entsize)
    if kind==1:
     assert alignment>=16384 and (vaddr-offset)%16384==0,(info.filename,alignment)
     loads.append(alignment)
   path=qa/name.name;path.write_bytes(data)
   dynamic=subprocess.check_output([str(readelf),'-d',str(path)],text=True)
   needed=re.findall(r'\(NEEDED\).*?\[(.*?)\]',dynamic)
   runtime[name.name]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),load_segment_alignments=loads,needed=needed)
 expected={'libtouchHLE.so','libSDL2.so','libc++_shared.so'};assert set(runtime)==expected
 for name,record in runtime.items():assert set(record['needed'])<=expected|oslibs,(name,record['needed'])
 fonts=[i for i in archive.infolist() if i.filename.startswith('assets/touchHLE_fonts/') and i.filename.endswith('.ttf')]
 assert fonts and all(i.file_size>10000 for i in fonts)
 for name in ('libgcc_s.1.dylib','libstdc++.6.0.9.dylib','libz.1.2.3.dylib','libsqlite3.dylib','libxml2.2.dylib'):
  assert archive.read('assets/touchHLE_dylibs/'+name)==(source/'emulator/touchHLE_dylibs'/name).read_bytes()
 assert 'GPL-3.0-or-later' in archive.read('assets/preservation-notices/LICENSES.md').decode()
 for alias,name in [('LICENSES.md','ANDROID-LICENSES.md'),('SOURCE.md','ANDROID-SOURCE.md')]:
  assert archive.read('assets/preservation-notices/'+alias)==(source/name).read_bytes()
 assert ("versionName='" + version + "'") in badging
 expected_code=json.loads((source/'android-version.json').read_text())['version_code']
 assert ("versionCode='" + str(expected_code) + "'") in badging
 assert archive.getinfo('assets/preservation-notices/licenses/GPL-3.0.txt').file_size>30000
entries=len(zipfile.ZipFile(apk).infolist())
report=dict(version=version,application_id='org.hunterislandpreservation.android',min_android_api=26,target_android_api=35,
 native_libraries=runtime,signature_verified=True,signature_generic_project_identity=True,apk_16k_alignment_verified=True,
 elf_16k_load_alignment_verified=True,no_internet_or_broad_storage_permission=True,known_private_markers_found=False,
 no_ipa_saves_or_private_signing_key_bundled=True,assets_materialized_and_guest_hashes_verified=True,
 zip_entries_checked=entries,
 device_install_tested=False,device_game_start_tested=False,device_render_audio_touch_tested=False,
 scope='Static signature, alignment, permission, asset and known-private-marker checks. Physical-device runtime remains unverified.')
out=args.out.resolve();out.mkdir(parents=True,exist_ok=True)
dest=out/f'HunterIsland-Android-arm64-{version}.apk'
if dest!=apk:shutil.copyfile(apk,dest)
report['apk_sha256']=hashlib.sha256(dest.read_bytes()).hexdigest()
(out/'checks.json').write_text(json.dumps(report,indent=2)+'\n')
(out/'SHA256SUMS.txt').write_text(report['apk_sha256']+'  '+dest.name+'\n')
(qa/'signature.txt').write_text(verify.stdout);(qa/'badging.txt').write_text(badging)
print(json.dumps(report,indent=2))
