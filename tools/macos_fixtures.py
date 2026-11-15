# SPDX-License-Identifier: MPL-2.0
"""Synthetic import fixtures only. Never reads a game, user profile or save."""
from pathlib import Path
import json,plistlib,struct,sys,zipfile
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def macho(cpu=12,encrypted=0):
    return struct.pack('<7I',0xfeedface,cpu,6,2,1,20,0)+struct.pack('<5I',0x21,20,0,0,encrypted)
def fixture(name,ok,**kw):
    path=out/(name+'.ipa')
    with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED) as z:
        meta=dict(CFBundleIdentifier=kw.get('bundle','pockettrend.hunterisland'),CFBundleExecutable='Game',CFBundleShortVersionString='2.02')
        z.writestr('Payload/Game.app/Info.plist',plistlib.dumps(meta,fmt=kw.get('fmt',plistlib.FMT_XML)))
        z.writestr('Payload/Game.app/Game',kw.get('binary',macho()))
        for n,d in kw.get('extra',[]):
            if n=='SYMLINK':
                i=zipfile.ZipInfo('Payload/Game.app/link');i.create_system=3;i.external_attr=0o120777<<16;z.writestr(i,b'outside')
            else:z.writestr(n,d)
    cases.append(dict(file=path.name,accepted=ok))
cases=[]
fixture('xml',True);fixture('binary-plist',True,fmt=plistlib.FMT_BINARY)
fixture('encrypted',False,binary=macho(encrypted=1));fixture('wrong-cpu',False,binary=macho(cpu=7))
fixture('wrong-game',False,bundle='other.game');fixture('truncated',False,binary=b'invalid')
fixture('traversal',False,extra=[('../outside',b'x')]);fixture('symlink',False,extra=[('SYMLINK',b'')])
fixture('wildcard',False,extra=[('Payload/Game.app/a*b',b'x')])
fixture('case-duplicate',False,extra=[('Payload/Game.app/info.plist',b'x')])
fixture('fat-arm',True,binary=struct.pack('>7I',0xcafebabe,1,12,6,28,len(macho()),0)+macho())
fixture('fat-bounds',False,binary=struct.pack('>7I',0xcafebabe,1,12,6,9999,48,0)+macho())
(out/'cases.json').write_text(json.dumps(cases,indent=2))
print('Synthetic Mac import fixtures:',len(cases))
