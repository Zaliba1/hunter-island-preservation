# SPDX-License-Identifier: MPL-2.0
"""Validate local release assets; optional explicit upload to a GitHub draft."""
from pathlib import Path,PurePosixPath
import argparse,ast,hashlib,json,re,stat,subprocess,sys,zipfile
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'launcher'))
from hunter_launcher import VERSION

def digest(stream):
    h=hashlib.sha256()
    for chunk in iter(lambda:stream.read(1024**2),b''):h.update(chunk)
    return h.hexdigest()

def checked_archive(path):
    archive=zipfile.ZipFile(path)
    try:
        infos=archive.infolist();seen=set()
        if len(infos)>40000 or sum(x.file_size for x in infos)>2*1024**3:raise ValueError('Release ZIP exceeds inspection limits')
        for info in infos:
            parts=PurePosixPath(info.filename).parts
            if not parts or PurePosixPath(info.filename).is_absolute() or '..' in parts or '\\' in info.filename or ':' in info.filename:
                raise ValueError('Unsafe release ZIP member')
            if info.filename in seen or stat.S_ISLNK(info.external_attr>>16):raise ValueError('Duplicate or linked release ZIP member')
            seen.add(info.filename)
        if archive.testzip() is not None:raise ValueError('Release ZIP CRC mismatch')
        return archive
    except BaseException:
        archive.close();raise

def validate_release(directory,version=VERSION):
    root=Path(directory).resolve();records={}
    if (root/'DO-NOT-PUBLISH.md').exists():raise ValueError('Release is explicitly blocked: DO-NOT-PUBLISH.md')
    for line in (root/'SHA256SUMS.txt').read_text().splitlines():
        match=re.fullmatch(r'([0-9a-f]{64})  ([^/\\]+)',line)
        if not match:raise ValueError('Invalid checksum line')
        wanted,name=match.groups()
        if name in records or name in {'.','..'}:raise ValueError('Duplicate or unsafe checksum entry')
        path=root/name
        if path.is_symlink() or not path.is_file() or path.resolve().parent!=root:raise ValueError('Release asset is not a regular local file: '+name)
        with path.open('rb') as f:
            actual=digest(f)
        if actual!=wanted:raise ValueError('Checksum mismatch: '+name)
        records[name]=wanted
    names=[f'HunterIsland-Windows-x64-{version}.zip',f'HunterIsland-Source-{version}.zip',
        f'HunterIsland-SourceDependencies-{version}.zip','license-review.md','release-checks.json']
    if not set(names).issubset(records):raise ValueError('Required matching source/review asset missing')
    checks=json.loads((root/'release-checks.json').read_text())
    if checks.get('identified_license_delivery_checks_passed') is not True or checks.get('private_marker_audit_passed') is not True:
        raise ValueError('Release gate not satisfied')
    validation=checks.get('validation',{});distribution=checks.get('distribution',{})
    if any(record.get('version')!=version for record in (checks,validation,distribution)):
        raise ValueError('Release version differs from its test evidence')
    engine=validation.get('engine_sha256','')
    if not re.fullmatch('[0-9a-f]{64}',engine) or distribution.get('engine_sha256')!=engine:
        raise ValueError('Engine hash differs from its test evidence')
    if distribution.get('windows_zip_extracted_and_run') is not True:
        raise ValueError('Exact ZIP distribution test is missing')
    with checked_archive(root/names[0]) as archive:
        manifest=json.loads(archive.read('runtime-manifest.json'))
        if manifest.get('version')!=version:raise ValueError('Windows ZIP manifest version mismatch')
        expected=manifest['files']['touchHLE.exe']['sha256']
        with archive.open('runtime/touchHLE.exe') as stream:actual=digest(stream)
        if expected!=engine or actual!=engine:raise ValueError('Windows ZIP engine hash mismatch')
        if len(archive.read('licenses/GPL-3.0.txt'))<30000:raise ValueError('Combined binary GPL text is missing')
    with checked_archive(root/names[1]) as archive:
        tree=ast.parse(archive.read('launcher/hunter_launcher/__init__.py').decode())
        versions=[n.value.value for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='VERSION' for t in n.targets) and isinstance(n.value,ast.Constant)]
        if versions!=[version]:raise ValueError('Source ZIP version mismatch')
        if b'GPL-3.0-or-later' not in archive.read('LICENSES.md'):raise ValueError('Source ZIP binary-license disclosure is missing')
    with checked_archive(root/names[2]) as archive:
        sources=json.loads(archive.read('archive-manifest.json'))
        if sources.get('failures'):raise ValueError('Dependency-source collection failed')
        expected=checks.get('source_delivery',{})
        rust=json.loads(archive.read('rust-files.json'))
        if expected.get('all_checksums_verified') is not True or len(sources['archives'])!=expected.get('archives') or len(rust)!=expected.get('rust_files'):
            raise ValueError('Source-dependency manifest differs from review')
        for record in sources['archives']:
            with archive.open(record['path']) as stream:actual=digest(stream)
            if actual!=record['sha256']:raise ValueError('Nested source archive hash mismatch')
        for name,wanted in rust.items():
            with archive.open('rust/'+name) as stream:actual=digest(stream)
            if actual!=wanted:raise ValueError('Vendored Rust source hash mismatch')
    return dict(files=names,engine_sha256=engine,checksums=records)

def main():
    p=argparse.ArgumentParser();p.add_argument('assets',type=Path);p.add_argument('--repository')
    p.add_argument('--upload-draft',action='store_true');a=p.parse_args()
    if a.repository and not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9-]{0,38}/[A-Za-z0-9_.-]+',a.repository):raise ValueError('Expected owner/repository')
    if a.upload_draft and not a.repository:raise ValueError('--repository owner/repository is required for an upload')
    result=validate_release(a.assets);root=a.assets.resolve();names=result['files']
    notes=Path(__file__).resolve().parents[1]/'RELEASE_NOTES.md'
    if a.upload_draft:
        args=['gh','release','create','v'+VERSION,'--repo',a.repository,'--draft','--title','Hunter Island on touchHLE '+VERSION,'--notes-file',str(notes),
            *[str(root/name) for name in names],str(root/'SHA256SUMS.txt')]
        subprocess.run(['gh','auth','status'],check=True)
        subprocess.run(args,check=True)
    else:
        print(json.dumps(dict(assets_verified=True,files=names+['SHA256SUMS.txt'],repository=a.repository,
            upload_performed=False,next_step='After creating the intended GitHub repository and authenticating gh, repeat with --repository owner/repository --upload-draft. It creates a draft, not a public release.'),indent=2))

if __name__=='__main__':main()
