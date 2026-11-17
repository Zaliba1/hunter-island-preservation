# SPDX-License-Identifier: MPL-2.0
"""Assemble app-local Windows binaries and corresponding sources from pinned MSYS2 packages."""
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
import zipfile


PYTHON_LIB = PurePosixPath('ucrt64/lib/python3.14')
EXTENSIONS = {
    '_bisect', '_blake2', '_bz2', '_ctypes', '_decimal', '_elementtree', '_heapq',
    '_json', '_lzma', '_md5', '_multibytecodec', '_multiprocessing', '_overlapped',
    '_pickle', '_queue', '_random', '_sha1', '_sha2', '_sha3', '_socket',
    '_statistics', '_struct', '_tkinter', '_uuid', '_wmi', '_zstd', 'array',
    'binascii', 'math', 'mmap', 'pyexpat', 'select', 'unicodedata', 'zlib',
}
OMIT_PYTHON = {'__pycache__', 'ensurepip', 'idlelib', 'site-packages', 'test', 'tests', 'turtledemo'}
ENTRY = 'Start Hunter Island.exe'


def verified_file(cache, name, record):
    if Path(name).name != name:
        raise ValueError(f'Invalid package filename: {name}')
    file = cache / name
    with file.open('rb') as stream:
        if hashlib.file_digest(stream, 'sha256').hexdigest() != record['sha256']:
            raise ValueError(f'Runtime package checksum mismatch: {name}')
    return file


def fetch_runtime(lock_path, cache):
    lock = json.loads(lock_path.read_text(encoding='utf-8'))
    records = {r['filename']: r for r in lock['packages'].values()} | lock['sources']
    cache.mkdir(parents=True, exist_ok=True)
    for name, record in sorted(records.items()):
        if Path(name).name != name or not record['url'].startswith('https://repo.msys2.org/mingw/'):
            raise ValueError(f'Unexpected runtime package location: {name}')
        target = cache / name
        if target.exists():
            verified_file(cache, name, record)
            continue
        with tempfile.TemporaryDirectory(dir=cache) as pending:
            staging = Path(pending)
            with urllib.request.urlopen(record['url'], timeout=60) as response, (staging / name).open('xb') as output:
                shutil.copyfileobj(response, output)
            verified_file(staging, name, record).replace(target)
    print(f'Verified {len(records)} MSYS2 packages and corresponding source archives.')


def selected_member(name, provider, bins):
    path = PurePosixPath(name)
    if path.is_absolute() or '..' in path.parts or path.parts[0] != 'ucrt64' or '\\' in name or ':' in name:
        return False
    if path.parent == PurePosixPath('ucrt64/bin') and path.name.lower() in bins:
        return True
    if path.is_relative_to('ucrt64/share/licenses'):
        return True
    if provider == 'python' and path.is_relative_to(PYTHON_LIB):
        relative = path.relative_to(PYTHON_LIB)
        if path.parent == PYTHON_LIB / 'lib-dynload' and path.suffix == '.pyd':
            stem = path.name.split('.', 1)[0]
            return stem in EXTENSIONS or stem.startswith('_codecs_')
        return (path.suffix == '.py' and OMIT_PYTHON.isdisjoint(relative.parts)
                and not any(p.startswith('config-') for p in relative.parts))
    if provider in {'tcl', 'tk'}:
        return path.is_relative_to(f'ucrt64/lib/{provider}8.6') and 'demos' not in path.parts
    return False


def toolkit_notice(tar_exe, source_archive, name, version):
    member = f'mingw-w64-{name}/{name}{version}-src.tar.gz'
    source = subprocess.check_output([tar_exe, '-xOf', str(source_archive), member])
    with tarfile.open(fileobj=io.BytesIO(source), mode='r:gz') as archive:
        with archive.extractfile(f'{name}{version}/license.terms') as notice:
            return notice.read()


def bundle_launcher(root, package, work, cache, source_zip):
    windows = Path(os.environ['SystemRoot'])
    tar_exe = windows / 'System32/tar.exe'
    compiler = windows / 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    lock_path = root / 'windows-portable-runtime.json'
    lock = json.loads(lock_path.read_text(encoding='utf-8'))
    sources = {name: verified_file(cache, name, record) for name, record in lock['sources'].items()}
    runtime = package / '_internal'
    notices = package / 'licenses/windows-portable'
    notices.mkdir()
    compiled = {}

    for name, record in lock['packages'].items():
        archive = verified_file(cache, record['filename'], record)
        entries = subprocess.check_output([tar_exe, '-tf', str(archive)], encoding='utf-8').splitlines()
        bins = set(record['bin'])
        selected = [entry for entry in entries if entry and not entry.endswith('/') and selected_member(entry, name, bins)]
        selected_bins = {PurePosixPath(entry).name.lower() for entry in selected if entry.startswith('ucrt64/bin/')}
        if bins != selected_bins:
            raise ValueError(f'Missing runtime files in {record["filename"]}: {bins - selected_bins}')
        stage = work / name
        stage.mkdir(parents=True)
        manifest = stage / 'members.txt'
        manifest.write_bytes(('\n'.join(selected) + '\n').encode('utf-8'))
        subprocess.run([tar_exe, '-xf', str(archive), '-C', str(stage), '--no-recursion', '-T', str(manifest)], check=True)
        for entry in selected:
            source = stage / entry
            if not source.is_file() or source.is_symlink() or not source.resolve().is_relative_to(stage.resolve()):
                raise ValueError(f'Unsupported runtime archive member: {entry}')
            relative = PurePosixPath(entry).relative_to('ucrt64')
            if relative.is_relative_to('share/licenses'):
                target = notices / name / relative.relative_to('share/licenses')
            else:
                target = runtime / relative
                if target.suffix.lower() in {'.exe', '.dll', '.pyd'}:
                    compiled[target.relative_to(package).as_posix()] = record['name'] + ' ' + record['version']
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists():
                raise ValueError(f'Duplicate runtime file: {relative}')
            shutil.copyfile(source, target)
        if name in {'tcl', 'tk'}:
            target = notices / name / 'license.terms'
            target.parent.mkdir(exist_ok=True)
            target.write_bytes(toolkit_notice(tar_exe, sources[record['source']], name, lock['tcl_tk_version']))

    subprocess.run([
        compiler, '/nologo', '/target:winexe', '/platform:x64', '/debug-', '/optimize+',
        '/reference:System.Windows.Forms.dll', '/out:' + str(package / ENTRY), str(root / 'launcher/PortableStart.cs'),
    ], check=True)
    (package / 'windows-launcher-provenance.json').write_text(json.dumps({
        'runtime': 'MSYS2 UCRT64 CPython and Tk', 'python': lock['python_version'],
        'tcl_tk': lock['tcl_tk_version'], 'compiled_files': compiled,
        'runtime_sources': source_zip.name, 'notices': 'licenses/windows-portable/',
    }, indent=2) + '\n', encoding='utf-8')
    shutil.copyfile(lock_path, package / lock_path.name)
    (package / 'START-HERE.txt').write_text(
        'Extract the entire ZIP and double-click Start Hunter Island.exe.\n'
        'Keep the _internal, launcher and runtime folders beside the starter.\n'
        'Select Import your IPA to choose your game file, then select Play.\n'
        'Use Language to change the launcher and game language. See SETUP.md.\n', encoding='utf-8',
    )
    with zipfile.ZipFile(source_zip, 'x', zipfile.ZIP_STORED) as archive:
        archive.write(lock_path, lock_path.name)
        for name, source in sorted(sources.items()):
            archive.write(source, name)


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description='Download and verify portable Windows runtime inputs.')
    parser.add_argument('cache', type=Path)
    args = parser.parse_args()
    fetch_runtime(Path(__file__).resolve().parents[1] / 'windows-portable-runtime.json', args.cache)
