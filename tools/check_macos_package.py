# SPDX-License-Identifier: MPL-2.0
"""Inspect the exact Mac app, its signatures, payloads and corresponding sources."""
from pathlib import Path
import argparse,hashlib,json,os,plistlib,subprocess,sys
root=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(root/'tools'))
from audit_public import audit
p=argparse.ArgumentParser();p.add_argument('app',type=Path);p.add_argument('--out',type=Path,required=True);a=p.parse_args()
app=a.app.resolve();info=plistlib.loads((app/'Contents/Info.plist').read_bytes())
assert info['CFBundleExecutable']=='HunterIslandLauncher' and info['LSMinimumSystemVersion']=='13.0'
subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
compiled=[]
for name in ['HunterIslandLauncher','touchHLE','isolation-probe']:
    path=app/'Contents/MacOS'/name
    arch=subprocess.check_output(['lipo','-archs',str(path)],text=True).strip()
    assert arch=='arm64',(name,arch)
    links=subprocess.check_output(['otool','-L',str(path)],text=True)
    for line in links.splitlines()[1:]:
        dependency=line.strip().split(' (')[0]
        assert dependency.startswith(('/System/Library/','/usr/lib/')),(name,dependency)
    compiled.append(dict(path='Contents/MacOS/'+name,architecture=arch,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),system_libraries_only=True))
notices=app/'Contents/Resources/notices'
assert (notices/'licenses/GPL-3.0.txt').stat().st_size>30000
assert (notices/'licenses/MPL-2.0.txt').stat().st_size>10000
for alias in ['LICENSE','LICENSES.md','LICENSE_REVIEW.md','THIRD_PARTY.md','SOURCE.md','MACOS.md']:
    assert (notices/alias).read_bytes()==(root/alias).read_bytes()
guest=json.loads((root/'guest-library-provenance.json').read_text())
for record in guest:
    # The original inventory stores path and digest. Match its filename.
    expected=record['sha256']
    matches=[f for f in (app/'Contents/Resources/touchHLE_dylibs').iterdir() if hashlib.sha256(f.read_bytes()).hexdigest()==expected]
    assert matches,'Missing guest library checksum'
for path in app.rglob('*'):
    if not path.is_file():continue
    data=path.read_bytes().lower()
    for private in [str(Path.home()),str(root)]:
        for encoding in ('utf-8','utf-16-le'):
            assert private.lower().encode(encoding) not in data,('Private build path',path.relative_to(app))
report=dict(schema='hunter-macos-preview-v1',source_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
    architecture='arm64',minimum_macos='13.0',compiled_files=compiled,signature='ad-hoc',notarized=False,
    source_dependency_asset='HunterIsland-SourceDependencies-0.2.0-preview.2.zip',source_dependency_sha256='b232e533bea7ce266cbd8751ac83f19a496c4c8f87d02b411e887f1244cc4788',
    native_launcher_ui_smoke_tested=True,ipa_fixture_import_tested=True,graphics_shader_render_readback_tested=True,
    actual_game_start_tested=False,actual_game_audio_touch_save_tested=False,original_game_included=False,
    privacy=audit(app,True),isolation_scope='Network denied, reads under the real home directory denied except app/profile, writes limited to the run directory. Seatbelt probe required before play.',
    notes='Experimental test branch. Engine test/graphics probes are synthetic and do not establish Hunter Island gameplay compatibility.')
a.out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
