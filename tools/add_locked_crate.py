# SPDX-License-Identifier: MPL-2.0
"""Add a verified Cargo.lock crate to an already prepared offline source set."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import tarfile
import tempfile
import tomllib


def sha(data):
    return hashlib.sha256(data).hexdigest()


def install(crate, vendor, lock):
    crate, vendor = Path(crate), Path(vendor)
    raw = crate.read_bytes()
    if len(raw) > 32 * 1024 * 1024:
        raise ValueError('Crate archive exceeds the limit')
    packages = tomllib.loads(Path(lock).read_text(encoding='utf-8'))['package']
    matched = [p for p in packages if crate.name == f"{p['name']}-{p['version']}.crate"
               and p.get('source', '').startswith('registry+')]
    if len(matched) != 1 or matched[0].get('checksum') != sha(raw):
        raise ValueError('Crate does not match Cargo.lock')
    package = matched[0]
    prefix = f"{package['name']}-{package['version']}"
    vendor.mkdir(parents=True, exist_ok=True)
    destination = vendor / prefix
    with tempfile.TemporaryDirectory(dir=vendor) as temporary:
        staged = Path(temporary) / prefix
        staged.mkdir()
        files = {}
        with tarfile.open(crate, mode='r:gz') as archive:
            members = archive.getmembers()
            if len(members) > 10000 or sum(m.size for m in members) > 64 * 1024 * 1024:
                raise ValueError('Expanded crate exceeds the limit')
            for member in members:
                path = PurePosixPath(member.name)
                if (path.is_absolute() or '..' in path.parts or '\\' in member.name
                        or ':' in member.name or not path.parts or path.parts[0] != prefix
                        or not (member.isdir() or member.isfile())):
                    raise ValueError('Unsafe crate member')
                relative = path.relative_to(prefix)
                if member.isdir():
                    continue
                name = relative.as_posix()
                if name in files or name in {'.', '.cargo-checksum.json'}:
                    raise ValueError('Duplicate or reserved crate member')
                data = archive.extractfile(member).read()
                target = staged / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
                files[name] = sha(data)
        checksum = dict(package=package['checksum'], files=files)
        (staged / '.cargo-checksum.json').write_text(json.dumps(checksum), encoding='utf-8')
        if destination.exists():
            actual = {p.relative_to(destination).as_posix(): sha(p.read_bytes())
                      for p in destination.rglob('*') if p.is_file() and p.name != '.cargo-checksum.json'}
            if any(p.is_symlink() for p in destination.rglob('*')) or actual != files:
                raise ValueError('Existing crate sources differ')
            if json.loads((destination / '.cargo-checksum.json').read_text()) != checksum:
                raise ValueError('Existing crate checksum differs')
        else:
            staged.replace(destination)
    return prefix


def extend(root, crate):
    root = Path(root).resolve()
    lock = root / 'emulator/Cargo.lock'
    name = install(crate, root / 'emulator/vendor/rust', lock)
    dependencies = root / '.dependencies'
    if dependencies.is_dir():
        install(crate, dependencies / 'rust', lock)
        files = {p.relative_to(dependencies / 'rust').as_posix(): sha(p.read_bytes())
                 for p in sorted((dependencies / 'rust').rglob('*')) if p.is_file()}
        (dependencies / 'rust-files.json').write_text(json.dumps(files, indent=2) + '\n', encoding='utf-8')
    return dict(crate=name, checksum_verified_against_lock=True, corresponding_sources_updated=dependencies.is_dir())


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('crate', type=Path)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    print(json.dumps(extend(args.root, args.crate), indent=2))
