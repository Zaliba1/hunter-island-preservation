# SPDX-License-Identifier: MPL-2.0
"""Fetch public native dependencies at the exact revisions in upstream.json."""
from pathlib import Path
import json
import subprocess

root=Path(__file__).resolve().parents[1]
for dependency in json.loads((root/'upstream.json').read_text())['dependencies']:
    path=root/'emulator'/dependency['path']
    if not path.resolve().is_relative_to((root/'emulator/vendor').resolve()):
        raise ValueError('Dependency outside vendor directory')
    if path.exists():
        actual=subprocess.check_output(['git','rev-parse','HEAD'],cwd=path,text=True).strip()
        if actual!=dependency['commit']:
            raise ValueError('Existing dependency has a different revision: '+dependency['path'])
        if subprocess.check_output(['git','status','--porcelain'],cwd=path):
            raise ValueError('Existing dependency has local changes: '+dependency['path'])
    else:
        if not dependency['url'].startswith('https://github.com/') or len(dependency['commit'])!=40:
            raise ValueError('Expected a public pinned GitHub dependency')
        subprocess.run(['git','clone','--no-checkout',dependency['url'],str(path)],check=True)
        subprocess.run(['git','checkout','--detach',dependency['commit']],cwd=path,check=True)
print('Pinned dependencies ready. Provide Boost 1.81.0 to build-emulator.ps1.')
