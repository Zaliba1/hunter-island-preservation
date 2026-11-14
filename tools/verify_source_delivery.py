# SPDX-License-Identifier: MPL-2.0
"""Validate complete dependency inputs without fetching or executing them."""
from pathlib import Path
import json
from prepare_offline_sources import digest

def verify(root):
    root=Path(root).resolve()
    manifest=json.loads((root/'archive-manifest.json').read_text())
    if manifest.get('failures'):raise ValueError('Source archive download failed')
    required={'native/SDL.zip','native/dynarmic.zip','native/openal-soft.zip','native/stb.zip',
        'boost/boost_1_81_0.zip','guest/gcc-5664.tar.gz','guest/libstdcxx-39.tar.gz',
        'guest/zlib-23.0.1.tar.gz','guest/sqlite-3.6.10.tar.gz','guest/libxml2-22.3.tar.gz',
        'runtime/Python-3.11.9.tgz','runtime/openssl-3.0.13.tar.gz'}
    paths={record['path'] for record in manifest['archives']}
    if not required.issubset(paths):raise ValueError('Required corresponding source missing')
    for record in manifest['archives']:
        path=root/record['path']
        if not path.resolve().is_relative_to(root) or not path.is_file() or path.is_symlink() or digest(path)!=record['sha256']:
            raise ValueError('Source checksum mismatch: '+record['path'])
    expected=json.loads((root/'rust-files.json').read_text())
    actual={f.relative_to(root/'rust').as_posix():f for f in (root/'rust').rglob('*') if f.is_file()}
    if set(actual)!=set(expected):raise ValueError('Rust source file set mismatch')
    for name,path in actual.items():
        if path.is_symlink() or digest(path)!=expected[name]:raise ValueError('Rust source checksum mismatch: '+name)
    permitted=paths|{'archive-manifest.json','rust-files.json','cargo-vendor-config.toml'}|{'rust/'+n for n in expected}
    if {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}!=permitted:
        raise ValueError('Unexpected file in dependency source delivery')
    return dict(archives=len(paths),rust_packages=len([p for p in (root/'rust').iterdir() if p.is_dir()]),rust_files=len(expected),all_checksums_verified=True)

if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('root',type=Path);a=p.parse_args();print(json.dumps(verify(a.root),indent=2))
