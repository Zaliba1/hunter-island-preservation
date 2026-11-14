# SPDX-License-Identifier: MPL-2.0
"""Prepare the matching dependency asset; no network or build execution."""
from pathlib import Path, PurePosixPath
import argparse, hashlib, json, posixpath, re, shutil, stat, tomllib, zipfile

def digest(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for chunk in iter(lambda:f.read(1024**2),b''):h.update(chunk)
    return h.hexdigest()

def extract(archive,destination,prefix=None):
    destination=Path(destination).resolve()
    with zipfile.ZipFile(archive) as z:
        if len(z.infolist())>150000 or sum(x.file_size for x in z.infolist())>6*1024**3:
            raise ValueError('Source archive exceeds extraction limit')
        selected=[]
        for info in z.infolist():
            p=PurePosixPath(info.filename)
            if p.is_absolute() or '..' in p.parts or '\\' in info.filename or ':' in info.filename:
                raise ValueError('Unsafe source archive member: '+info.filename)
            if prefix:
                if not info.filename.startswith(prefix):continue
                relative=info.filename[len(prefix):]
                if not relative:continue
            else:relative=info.filename
            target=destination/relative
            if not target.resolve().is_relative_to(destination):raise ValueError('Outside source directory')
            selected.append((info,target))
        def materialize(info,target,seen=()):
            if info.filename in seen:raise ValueError('Source symlink cycle')
            if stat.S_ISLNK(info.external_attr>>16):
                link=z.read(info).decode('utf-8')
                joined=posixpath.normpath(posixpath.join(posixpath.dirname(info.filename),link))
                if link.startswith('/') or '\\' in link or ':' in link or joined.startswith('../'):
                    raise ValueError('Outside archive symlink target')
                if joined in z.namelist():
                    materialize(z.getinfo(joined),target,(*seen,info.filename))
                else:
                    children=[i for i in z.infolist() if i.filename.startswith(joined.rstrip('/')+'/')]
                    if not children:raise ValueError('Missing source symlink target: '+joined)
                    for child in children:
                        materialize(child,target/child.filename[len(joined.rstrip('/'))+1:],(*seen,info.filename))
                return
            if info.is_dir():target.mkdir(parents=True,exist_ok=True);return
            target.parent.mkdir(parents=True,exist_ok=True)
            with z.open(info) as incoming,target.open('wb') as output:shutil.copyfileobj(incoming,output)
        for info,target in selected:materialize(info,target)

def prepare(asset,root):
    asset=Path(asset).resolve();root=Path(root).resolve()
    if asset.is_dir():deps=asset
    else:
        deps=root/'.dependencies'
        if deps.exists():raise ValueError('Dependency staging exists; use its directory explicitly or a fresh source checkout')
        deps.mkdir();extract(asset,deps)
    manifest=json.loads((deps/'archive-manifest.json').read_text())
    if manifest.get('failures'):raise ValueError('Incomplete dependency source set')
    for record in manifest['archives']:
        path=deps/record['path']
        if not path.resolve().is_relative_to(deps) or digest(path)!=record['sha256']:
            raise ValueError('Dependency checksum mismatch: '+record['path'])
    engine=root/'emulator';vendor=engine/'vendor'
    for name in ('SDL','dynarmic','openal-soft','stb'):
        dest=vendor/name
        if dest.exists():raise ValueError('Refuse to overwrite native source: '+name)
        dest.mkdir();extract(deps/'native'/(name+'.zip'),dest)
    rust=vendor/'rust'
    if rust.exists():raise ValueError('Refuse to overwrite Rust sources')
    expected=json.loads((deps/'rust-files.json').read_text())
    actual={f.relative_to(deps/'rust').as_posix():f for f in (deps/'rust').rglob('*') if f.is_file()}
    if set(expected)!=set(actual) or any(f.is_symlink() or digest(f)!=expected[name] for name,f in actual.items()):
        raise ValueError('Rust dependency file manifest mismatch')
    shutil.copytree(deps/'rust',rust)
    boost=vendor/'boost'
    if boost.exists():raise ValueError('Refuse to overwrite Boost')
    boost.mkdir();extract(deps/'boost/boost_1_81_0.zip',boost,prefix='boost_1_81_0/boost/')
    # Boost's include root must contain the boost/ directory.
    headers=vendor/'boost-headers-staging'
    boost.replace(headers);boost.mkdir();headers.replace(boost/'boost')
    with zipfile.ZipFile(deps/'boost/boost_1_81_0.zip') as z:
        (boost/'LICENSE_1_0.txt').write_bytes(z.read('boost_1_81_0/LICENSE_1_0.txt'))
    config=deps/'cargo-vendor-config.toml'
    text=config.read_text()
    if tomllib.loads(text)['source']['vendored-sources']['directory']!='vendor/rust':
        raise ValueError('Vendor config must use vendor/rust')
    current=engine/'.cargo/config.toml'
    base=current.read_text()
    if '[source.' in base:raise ValueError('Source replacement already configured')
    current.write_text(base+'\n# Offline source asset for this release.\n'+text)
    print('Dependency sources verified and prepared. Build with cargo --frozen.')

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('asset',type=Path);parser.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1]);args=parser.parse_args()
    prepare(args.asset,args.root)
