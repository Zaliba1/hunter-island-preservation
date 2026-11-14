# SPDX-License-Identifier: MPL-2.0
"""Export inspectable Android sources without game data, caches or local signing."""
from pathlib import Path
import argparse,hashlib,json,os,shutil,zipfile

SKIP={'.git','.build','.dependencies','__pycache__','.venv','target','dist','.gradle','build','jniLibs','private','profiles','games','backups','logs','touchHLE_sandbox'}
NATIVE={'boost','SDL','dynarmic','openal-soft','stb','rust'}
def export(root,destination,archive):
    root=Path(root).resolve();destination=Path(destination).resolve();archive=Path(archive).resolve()
    if destination.is_relative_to(root) or archive.is_relative_to(root):raise ValueError('Export outside the source checkout')
    if destination.exists() or archive.exists():raise ValueError('Refuse to overwrite an existing source export')
    destination.mkdir(parents=True)
    for current,dirs,names in os.walk(root,followlinks=False):
        rel=Path(current).relative_to(root)
        dirs[:]=[d for d in dirs if d not in SKIP and not (rel==Path('emulator/vendor') and d in NATIVE)]
        for name in sorted(names):
            source=Path(current)/name;relative=source.relative_to(root)
            if name in {'source-files.json','local.properties'}:continue
            if source.is_symlink():raise ValueError('Refuse source symlink: '+str(relative))
            if source.suffix.lower() in {'.ipa','.p12','.jks','.keystore','.key','.apk','.class','.log','.pdb','.etl','.pyc'}:raise ValueError('Private/generated input in source tree: '+str(relative))
            target=destination/relative;target.parent.mkdir(parents=True,exist_ok=True)
            data=source.read_bytes()
            if relative.as_posix()=='emulator/.cargo/config.toml':
                text=data.decode('utf-8');marker='# Offline source asset for this release.'
                data=text.split(marker,1)[0].rstrip().encode('utf-8')+b'\n'
                if b'[source.' in data:raise ValueError('Unexpected local Cargo source configuration')
            target.write_bytes(data)
    records={p.relative_to(destination).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(destination.rglob('*')) if p.is_file()}
    (destination/'source-files.json').write_text(json.dumps(records,indent=2)+'\n')
    archive.parent.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED) as z:
        for p in sorted(destination.rglob('*')):
            if p.is_file():z.write(p,p.relative_to(destination).as_posix())
    print(json.dumps(dict(source_files=len(records),offline_config_reset=True),indent=2))

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1]);p.add_argument('--destination',type=Path,required=True);p.add_argument('--zip',type=Path,required=True);a=p.parse_args();export(a.root,a.destination,a.zip)
