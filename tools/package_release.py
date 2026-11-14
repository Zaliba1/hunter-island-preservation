# SPDX-License-Identifier: MPL-2.0
"""Build an explicit clean release. Never reads a user profile or an IPA."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import zipfile
from audit_public import audit, SKIP
from license_inventory import inventory
from verify_source_delivery import verify as verify_source_delivery
from verify_windows_imports import verify as verify_windows_imports
from audit_dependency_sources import audit as audit_dependency_sources

root=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(root/'launcher'))
from hunter_launcher import VERSION
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def archive_tree(source,target,exclude=()):
    with zipfile.ZipFile(target,'x',zipfile.ZIP_DEFLATED,strict_timestamps=False) as archive:
        for p in sorted(source.rglob('*')):
            rel=p.relative_to(source)
            if any(x in SKIP or x in exclude for x in rel.parts):continue
            if p.is_file():archive.write(p,rel.as_posix())

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--emulator-exe',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--dependency-sources',type=Path,required=True)
    args=parser.parse_args()
    if args.out.exists():raise ValueError('Output must be a new directory')
    if b'OpenAL Soft LGPL relink verification' in args.emulator_exe.read_bytes():
        raise ValueError('Do not package the modified-library QA executable')
    audit(root)
    source_delivery=verify_source_delivery(args.dependency_sources)
    source_privacy=audit_dependency_sources(args.dependency_sources)
    subprocess.run([sys.executable,'-m','unittest','discover','-s',str(root/'tests'),'-v'],check=True)
    args.out.mkdir(parents=True)
    package=args.out/'HunterIsland-Launcher'
    package.mkdir()
    shutil.copytree(root/'launcher',package/'launcher',ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
    shutil.copyfile(root/'Start.cmd',package/'Start.cmd')
    runtime=package/'runtime';runtime.mkdir()
    shutil.copyfile(args.emulator_exe,runtime/'touchHLE.exe')
    for name in ('touchHLE_fonts','touchHLE_dylibs'):
        shutil.copytree(root/'emulator'/name,runtime/name)
    shutil.copyfile(root/'emulator/touchHLE_default_options.txt',runtime/'touchHLE_default_options.txt')
    csc=Path(os.environ['SystemRoot'])/'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    subprocess.run([str(csc),'/nologo','/optimize+','/debug-','/target:exe','/out:'+str(runtime/'isolation-probe.exe'),str(root/'launcher/IsolationProbe.cs')],check=True)
    manifest=dict(schema=1,version=VERSION,files={})
    for p in sorted(runtime.rglob('*')):
        if p.is_file():manifest['files'][p.relative_to(runtime).as_posix()]=dict(sha256=sha(p),bytes=p.stat().st_size)
    (package/'runtime-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    for name in ('README.md','LICENSE','LICENSES.md','PRIVACY.md','KNOWN_ISSUES.md','THIRD_PARTY.md','SOURCE.md','RELEASE_NOTES.md','guest-library-provenance.json','toolchain-provenance.json','SETUP.md','MODS.md','BUILD.md','LICENSE_REVIEW.md'):
        shutil.copyfile(root/name,package/name)
    if (root/'VALIDATION.md').exists():shutil.copyfile(root/'VALIDATION.md',package/'VALIDATION.md')
    licenses=package/'licenses';licenses.mkdir()
    shutil.copytree(root/'licenses',licenses,dirs_exist_ok=True)
    component_inventory=inventory(package)
    imports_check=verify_windows_imports(package,component_inventory)
    (package/'license-inventory.json').write_text(json.dumps(component_inventory,indent=2)+'\n')
    report=audit(package,True)
    source_report=audit(root)
    (args.out/'audit.json').write_text(json.dumps(dict(release=report,source=source_report,source_delivery=source_delivery,windows_imports=imports_check,
        source_dependency_privacy=source_privacy,bundled_python=False,bundled_microsoft_vc_runtime=False),indent=2))
    archive_tree(package,args.out/f'HunterIsland-Windows-x64-{VERSION}.zip')
    # Dependencies can be fetched at pinned revisions; no local vendor checkout,
    # build cache, private analysis, or existing .git history enters this archive.
    source_zip=args.out/f'HunterIsland-Source-{VERSION}.zip'
    source_files={}
    with zipfile.ZipFile(source_zip,'x',zipfile.ZIP_DEFLATED,strict_timestamps=False) as archive:
        for p in sorted(root.rglob('*')):
            rel=p.relative_to(root)
            if any(x in SKIP for x in rel.parts):continue
            if len(rel.parts)>2 and rel.parts[:2]==('emulator','vendor') and rel.parts[2] in {'boost','SDL','dynarmic','openal-soft','stb','rust'}:continue
            if p.is_file():
                archive.write(p,rel.as_posix())
                source_files[rel.as_posix()]=sha(p)
        archive.writestr('source-files.json',json.dumps(source_files,indent=2)+'\n')
    dependency_zip=args.out/f'HunterIsland-SourceDependencies-{VERSION}.zip'
    with zipfile.ZipFile(dependency_zip,'x',zipfile.ZIP_DEFLATED,strict_timestamps=False) as archive:
        for path in sorted(args.dependency_sources.rglob('*')):
            if path.is_symlink():raise ValueError('Source dependency symlink outside archived public inputs')
            if not path.is_file():continue
            rel=path.relative_to(args.dependency_sources).as_posix()
            method=zipfile.ZIP_STORED if path.name.endswith(('.zip','.gz','.tgz','.xz')) else zipfile.ZIP_DEFLATED
            archive.write(path,rel,compress_type=method)
    (args.out/'SHA256SUMS.txt').write_text(''.join(sha(p)+'  '+p.name+'\n' for p in sorted(args.out.glob('*.zip'))))
    print(json.dumps(dict(package=str(package),audit=report),indent=2))

if __name__=='__main__':main()
