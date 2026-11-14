# SPDX-License-Identifier: MPL-2.0
"""Scan expanded public dependency archives for known private identity markers.

Upstream fixture credentials/authors are public source data. This check targets
the builder's identity, including paths inside compressed source members.
"""
from pathlib import Path
import json
import os
import tarfile
import zipfile

def audit(root):
    root=Path(root)
    values=[os.environ.get('USERNAME','')]
    home=os.environ.get('USERPROFILE','')
    values.extend([home,home.replace('\\','/'),home.replace('\\','\\\\')])
    values.extend(os.environ.get('HI_PRIVATE_MARKERS','').split('|'))
    values=[v.lower() for v in values if len(v)>3]
    markers=[v.encode(encoding) for v in values for encoding in ('utf-8','utf-16-le')]
    overlap=max(map(len,markers),default=1)-1
    count=0;total=0
    def scan(label,stream):
        nonlocal count,total
        if any(v in label.lower() for v in values):
            raise ValueError('Private identity in source filename: '+label)
        tail=b''
        while chunk:=stream.read(1024**2):
            total+=len(chunk)
            data=tail+chunk.lower()
            if any(m in data for m in markers):
                raise ValueError('Private identity in dependency source: '+label)
            tail=data[-overlap:] if overlap else b''
        count+=1
    archives=json.loads((root/'archive-manifest.json').read_text())['archives']
    for record in archives:
        path=root/record['path']
        if path.suffix=='.zip':
            with zipfile.ZipFile(path) as archive:
                for info in archive.infolist():
                    if not info.is_dir():
                        with archive.open(info) as stream:scan(record['path']+'!'+info.filename,stream)
        else:
            with tarfile.open(path,mode='r|*') as archive:
                for member in archive:
                    if member.isfile():
                        with archive.extractfile(member) as stream:scan(record['path']+'!'+member.name,stream)
    for path in root.rglob('*'):
        if path.is_file() and path.relative_to(root).as_posix() not in {r['path'] for r in archives}:
            with path.open('rb') as stream:scan(path.relative_to(root).as_posix(),stream)
    return dict(archives=len(archives),expanded_files_scanned=count,expanded_bytes_scanned=total,
        known_private_markers_found=False,scope='Known builder identity/path markers, including UTF-16; public upstream fixtures are retained.')

if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser();parser.add_argument('root',type=Path);args=parser.parse_args()
    print(json.dumps(audit(args.root),indent=2))
