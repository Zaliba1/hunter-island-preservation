# SPDX-License-Identifier: MPL-2.0
from pathlib import Path
import argparse,json,plistlib,struct,subprocess,tempfile,zipfile,hashlib
p=argparse.ArgumentParser();p.add_argument('--java-home',type=Path,required=True);p.add_argument('--real-ipa',type=Path);a=p.parse_args()
root=Path(__file__).resolve().parents[1];build=root/'.build/importer-tests';build.mkdir(parents=True,exist_ok=True)
java=a.java_home/'bin/java.exe';javac=a.java_home/'bin/javac.exe'
subprocess.run([str(javac),'--release','11','-d',str(build),str(root/'emulator/android/app/src/main/java/org/touchhle/android/IpaInspector.java'),str(root/'tools/InspectorCLI.java')],check=True)
def executable(cpu=12,encrypted=0):
 commands=struct.pack('<5I',0x21,20,0,0,encrypted)
 return struct.pack('<7I',0xfeedface,cpu,6,2,1,len(commands),0)+commands
def fixture(path,fmt=plistlib.FMT_XML,bundle='pockettrend.hunterisland',cpu=12,encrypted=0,extra=(),metadata=None):
 with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED) as z:
  z.writestr('Payload/Game.app/Info.plist',metadata if metadata is not None else plistlib.dumps(dict(CFBundleIdentifier=bundle,CFBundleExecutable='Game',CFBundleShortVersionString='2.02',MinimumOSVersion='6.0',UIDeviceFamily=[1,2]),fmt=fmt))
  z.writestr('Payload/Game.app/Game',executable(cpu,encrypted))
  for name,data in extra:z.writestr(name,data)
 return path
def wide_references(invalid=False,cycle=False):
 # Valid bplist with 64-bit object references. A malformed reference used to
 # be truncated to 32 bits and silently alias an existing object.
 strings=['CFBundleIdentifier','pockettrend.hunterisland','CFBundleExecutable','Game',
          'CFBundleShortVersionString','2.02','MinimumOSVersion','6.0']
 refs=[1,3,5,7,2,4,6,8]
 if invalid:refs[0]+=1<<32
 if cycle:refs[4]=0
 objects=[b'\xd4'+b''.join(n.to_bytes(8,'big') for n in refs)]
 for string in strings:
  data=string.encode('ascii');n=len(data)
  objects.append((bytes([0x50|n]) if n<15 else b'\x5f\x10'+bytes([n]))+data)
 body=b'bplist00';offsets=[]
 for data in objects:offsets.append(len(body));body+=data
 table=len(body);width=max(1,(table.bit_length()+7)//8)
 body+=b''.join(n.to_bytes(width,'big') for n in offsets)
 return body+b'\0'*6+bytes([width,8])+len(objects).to_bytes(8,'big')+b'\0'*8+table.to_bytes(8,'big')
results=[]
def check(name,path,ok):
 before=hashlib.sha256(path.read_bytes()).hexdigest()
 result=subprocess.run([str(java),'-cp',str(build),'InspectorCLI',str(path)],capture_output=True,text=True)
 assert (result.returncode==0)==ok,(name,result.stdout,result.stderr)
 assert hashlib.sha256(path.read_bytes()).hexdigest()==before
 results.append(dict(name=name,accepted=ok,original_unchanged=True))
with tempfile.TemporaryDirectory() as folder:
 folder=Path(folder)
 check('XML plist',fixture(folder/'xml.ipa'),True)
 check('binary plist',fixture(folder/'binary.ipa',fmt=plistlib.FMT_BINARY),True)
 check('valid 64-bit plist references',fixture(folder/'wide.ipa',metadata=wide_references()),True)
 check('out-of-range 64-bit plist reference',fixture(folder/'wide-bad.ipa',metadata=wide_references(invalid=True)),False)
 check('cyclic plist reference',fixture(folder/'cyclic.ipa',metadata=wide_references(cycle=True)),False)
 xml=plistlib.dumps(dict(CFBundleIdentifier='pockettrend.hunterisland',CFBundleExecutable='Game',CFBundleShortVersionString='2.02',MinimumOSVersion='6.0'))
 duplicate=xml.replace(b'</dict>',b'<key>CFBundleIdentifier</key><string>pockettrend.hunterisland</string></dict>')
 check('duplicate XML metadata key',fixture(folder/'xml-duplicate.ipa',metadata=duplicate),False)
 wrongroot=xml.replace(b'<dict>',b'<array><dict>',1).replace(b'</dict>',b'</dict></array>',1)
 check('wrong XML root type',fixture(folder/'xml-wrongroot.ipa',metadata=wrongroot),False)
 check('other game',fixture(folder/'other.ipa',bundle='other.game'),False)
 check('encrypted ARM32',fixture(folder/'encrypted.ipa',encrypted=1),False)
 check('ARM64-only',fixture(folder/'arm64.ipa',cpu=0x100000c),False)
 check('path traversal',fixture(folder/'traversal.ipa',extra=[('../escape',b'x')]),False)
 check('duplicate entry',fixture(folder/'duplicate.ipa',extra=[('Payload/Game.app/Game',executable())]),False)
 corrupt=fixture(folder/'corrupt.ipa',extra=[('Payload/Game.app/data',b'unique-crc-check-target')]);data=bytearray(corrupt.read_bytes())
 with zipfile.ZipFile(corrupt) as z:
  info=z.getinfo('Payload/Game.app/data');offset=info.header_offset+30+len(info.filename.encode())+len(info.extra)
 data[offset]^=1;corrupt.write_bytes(data);check('corrupt ZIP CRC',corrupt,False)
 if a.real_ipa:check('real IPA static inspection',a.real_ipa,True)
report=dict(passed=len(results),cases=results,scope='Host JVM importer tests only; no Android/device execution and no game execution.')
(build/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
