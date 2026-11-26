# SPDX-License-Identifier: MPL-2.0
"""Fail closed on local data in source/release trees. No recursive work-folder copy."""
from pathlib import Path
import argparse
import json
import os
import re
import zipfile

BAD_PARTS={'touchHLE_sandbox','profiles','games','backups','logs','Debugpakete','private','profile','temp'}
BAD_SUFFIX={'.ipa','.pdb','.etl','.wav','.mp3','.m4a','.partial','.p12','.jks','.keystore','.key'}
SKIP={'.git','.build','.dependencies','dist','__pycache__','.venv','target'}
def audit(root,release=False):
    root=Path(root).resolve();errors=[];count=0
    embedded_count=0
    markers=[os.environ.get('USERNAME','').lower().encode()]
    markers.extend(x.lower().encode() for x in os.environ.get('HI_PRIVATE_MARKERS','').split('|') if x)
    home=os.environ.get('USERPROFILE','')
    if home:markers.extend([home.lower().encode(),home.replace('\\','/').lower().encode(),home.replace('\\','\\\\').lower().encode()])
    markers=[m for m in markers if len(m)>3]
    def check_data(label,data):
        data=data.lower()
        # The repository URL is the account identity explicitly chosen for
        # publication. Local paths and all other private markers still fail.
        public_url = b"https://github.com/zaliba1/hunter-island-preservation"
        data=data.replace(public_url, b"https://github.com/project-owner/project")
        data=data.replace(public_url.decode().encode("utf-16-le"),
                          "https://github.com/project-owner/project".encode("utf-16-le"))
        # Public mod title and its two fixed program identifiers.
        for title in ("Zalibas Expansion", "--zalibas-expansion", "zalibas_expansion"):
            for encoding in ("utf-8", "utf-16-le"):
                data=data.replace(title.lower().encode(encoding), b"<mod>")
        if Path(label).name.lower() == "touchhle.exe":
            # LLVM packs option-comparison constants together in the PE.
            # Match both neighboring fragments, never a bare account name.
            packed_flags = b"libas-expansion" + b"--zalibas-expansion"[:-3] + b"exchange"
            data=data.replace(packed_flags, b"<packed option comparisons>")
            packed_arena_flags = b"riple9alibas-expansion--zalibas-expanson-copy-campaign--expansion-copy"
            data=data.replace(packed_arena_flags, b"<packed option comparisons>")
        if any(marker in data or marker.decode(errors='ignore').encode('utf-16-le') in data for marker in markers):
            errors.append(label+': local identity/path marker')
        if re.search(rb'-----begin (?:[a-z0-9]+ )?private key-----',data) or re.search(rb'gh[pousr]_[a-z0-9]{30,}',data):
            errors.append(label+': credential marker')
    for p in root.rglob('*'):
        rel=p.relative_to(root)
        if any(part in SKIP for part in rel.parts):continue
        if not release and len(rel.parts)>2 and rel.parts[:2]==('emulator','vendor') and rel.parts[2] in {'boost','SDL','dynarmic','openal-soft','stb','rust'}:continue
        if p.is_symlink():errors.append(str(rel)+': symlink');continue
        if not p.is_file():continue
        count+=1
        if any(part in BAD_PARTS for part in rel.parts) or p.suffix.lower() in BAD_SUFFIX:
            errors.append(str(rel)+': private-data filename')
        if not release and p.suffix.lower() in {'.exe','.dll'}:
            errors.append(str(rel)+': build binary in source')
        check_data(str(rel),p.read_bytes())
        if p.suffix.lower()=='.zip':
            with zipfile.ZipFile(p) as archive:
                infos=archive.infolist()
                if len(infos)>20000 or sum(i.file_size for i in infos)>256*1024**2:
                    errors.append(str(rel)+': oversized embedded ZIP')
                else:
                    for info in infos:
                        check_data(str(rel)+'!'+info.filename,archive.read(info));embedded_count+=1
        if release and p.name=='HunterIsland-Launcher.exe':
            from PyInstaller.archive.readers import CArchiveReader
            archive=CArchiveReader(str(p))
            for name,entry in archive.toc.items():
                if entry[-1]=='z':
                    pyz=archive.open_embedded_archive(name)
                    for module in pyz.toc:
                        data=pyz.extract(module,raw=True)
                        if data:
                            check_data(str(rel)+'!'+module,data);embedded_count+=1
                else:
                    check_data(str(rel)+'!'+name,archive.extract(name));embedded_count+=1
        if rel.name in {'AGENTS.md','CLAUDE.md','pending-save-handoff.json'}:
            errors.append(str(rel)+': private workspace metadata')
    if errors:raise ValueError('\n'.join(errors))
    return dict(files=count,embedded_entries_checked=embedded_count,source=not release,private_markers_found=False)

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('root',type=Path);parser.add_argument('--release',action='store_true')
    args=parser.parse_args();print(json.dumps(audit(args.root,args.release),indent=2))
