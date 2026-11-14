# SPDX-License-Identifier: MPL-2.0
"""Offline Android publication gate: verify actual package and matching sources."""
from pathlib import Path,PurePosixPath
import argparse,hashlib,json,re,stat,zipfile

def digest(stream):
    h=hashlib.sha256()
    for chunk in iter(lambda:stream.read(1048576),b''):h.update(chunk)
    return h.hexdigest()

def checked_zip(path):
    z=zipfile.ZipFile(path);seen=set()
    try:
        if len(z.infolist())>150000 or sum(i.file_size for i in z.infolist())>6*1024**3:raise ValueError('Archive inspection limit')
        for i in z.infolist():
            p=PurePosixPath(i.filename)
            if i.filename in seen or p.is_absolute() or '..' in p.parts or '\\' in i.filename or ':' in i.filename or stat.S_ISLNK(i.external_attr>>16):raise ValueError('Unsafe archive member')
            seen.add(i.filename)
        if z.testzip():raise ValueError('Archive CRC mismatch')
        return z
    except BaseException:z.close();raise

def validate(root):
    root=Path(root).resolve();checks=json.loads((root/'release-checks.json').read_text());version=checks['version']
    if (root/'DO-NOT-PUBLISH.md').exists():raise ValueError('Publication blocked')
    if not re.fullmatch(r'(?:0\.1\.0-android[1-9][0-9]*|0\.2\.0-preview\.1)',version):raise ValueError('Invalid version')
    apk=f'HunterIsland-Android-arm64-{version}.apk';source=f'HunterIsland-Source-{version}.zip';deps=f'HunterIsland-SourceDependencies-{version}.zip'
    names={apk,source,deps,'android-license-inventory.json','license-review.md','release-checks.json','START-HIER.md'}
    hashes={}
    for line in (root/'SHA256SUMS.txt').read_text().splitlines():
        match=re.fullmatch(r'([0-9a-f]{64})  ([^/\\]+)',line)
        if not match:raise ValueError('Malformed checksum')
        wanted,name=match.groups();p=root/name
        if name in hashes or name in {'.','..'} or p.is_symlink() or p.resolve().parent!=root:raise ValueError('Unsafe checksum entry')
        with p.open('rb') as f:actual=digest(f)
        if wanted!=actual:raise ValueError('Checksum mismatch: '+name)
        hashes[name]=actual
    if not names.issubset(hashes):raise ValueError('Required binary/source/review asset missing')
    for key in ('identified_license_delivery_checks_passed','private_marker_audit_passed','fresh_export_dependency_preparation_passed','fresh_export_rebuild_passed'):
        if checks.get(key) is not True:raise ValueError('Release gate incomplete: '+key)
    inventory=json.loads((root/'android-license-inventory.json').read_text())
    if inventory['apk_sha256']!=hashes[apk] or checks['apk_sha256']!=hashes[apk]:raise ValueError('APK evidence hash mismatch')
    if checks.get('physical_device_execution_tested') is True:
        device=checks.get('device_validation',{})
        if device.get('runtime_payload_identical_to_device_tested_apk') is not True:
            raise ValueError('Device test does not match the distributed runtime payload')
    with checked_zip(root/source) as src,checked_zip(root/apk) as app:
        files=json.loads(src.read('source-files.json'))
        if set(files)|{'source-files.json'}!=set(src.namelist()):raise ValueError('Source file set mismatch')
        for name,wanted in files.items():
            with src.open(name) as f:actual=digest(f)
            if wanted!=actual:raise ValueError('Source hash mismatch: '+name)
        if json.loads(src.read('android-version.json'))['version']!=version:raise ValueError('Source version mismatch')
        if b'[source.' in src.read('emulator/.cargo/config.toml'):raise ValueError('Source export has local Cargo replacement')
        for entry in inventory['compiled_and_font_files']:
            with app.open(entry['path']) as f:actual=digest(f)
            if actual!=entry['sha256']:raise ValueError('Compiled/font inventory differs')
        for entry in inventory['notice_files']:
            if app.read('assets/preservation-notices/'+entry['path'])!=src.read(entry['path']):raise ValueError('APK/source notices differ')
        for name,original in [('LICENSES.md','ANDROID-LICENSES.md'),('SOURCE.md','ANDROID-SOURCE.md')]:
            if app.read('assets/preservation-notices/'+name)!=src.read(original):raise ValueError('APK/source disclosure differs')
        rust=json.loads(src.read('android-rust-dependencies.json'))
        for package in rust['packages']:
            if not package['license'] or not package['notices']:raise ValueError('Unmapped Rust license')
            for name in package['notices']:
                if name not in files:raise ValueError('Missing Rust attribution text')
    with checked_zip(root/deps) as z:
        manifest=json.loads(z.read('archive-manifest.json'))
        if manifest.get('failures'):raise ValueError('Incomplete dependency collection')
        for entry in manifest['archives']:
            with z.open(entry['path']) as f:actual=digest(f)
            if actual!=entry['sha256']:raise ValueError('Dependency source hash mismatch')
        rust_files=json.loads(z.read('rust-files.json'))
        for name,wanted in rust_files.items():
            with z.open('rust/'+name) as f:actual=digest(f)
            if actual!=wanted:raise ValueError('Vendored source hash mismatch')
    return dict(assets_verified=True,version=version,source_files=len(files),rust_packages=len(rust['packages']),native_archives_and_supplementary_sources=len(manifest['archives']),vendored_rust_files=len(rust_files),upload_performed=False,physical_device_execution_tested=checks.get('physical_device_execution_tested') is True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('assets',type=Path);a=p.parse_args();print(json.dumps(validate(a.assets),indent=2))
