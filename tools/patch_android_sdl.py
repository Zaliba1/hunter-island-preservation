# SPDX-License-Identifier: MPL-2.0
"""Replay one documented compatibility change on the pinned SDL source copies."""
from pathlib import Path
import hashlib,json
root=Path(__file__).resolve().parents[1]
relative=Path('src/sensor/android/SDL_androidsensor.c')
old=b'ALooper_pollAll(0, NULL, &events, (void**)&source)'
new=b'ALooper_pollOnce(0, NULL, &events, (void**)&source)'
notice=b'/* Modified 2026-10-26: Hunter Island Android port, NDK polling compatibility. */ '
record_path=root/'android-sdl-patch.json'
previous={r['file']:r for r in json.loads(record_path.read_text())} if record_path.exists() else {}
records=[]
for base in (root/'emulator/vendor/SDL',root/'emulator/vendor/rust/sdl2-sys-0.37.0/SDL'):
 path=base/relative;data=path.read_bytes();name_in_root=path.relative_to(root).as_posix()
 current=hashlib.sha256(data).hexdigest()
 prior=previous.get(name_in_root)
 original=prior['original_sha256'] if prior and current==prior['patched_sha256'] else current
 if old in data:
  assert data.count(old)==1;data=data.replace(old,new)
 elif new not in data:raise ValueError('Unexpected SDL sensor source; review patch against new version')
 if notice not in data:data=data.replace(new,notice+new)
 path.write_bytes(data)
 changed=hashlib.sha256(path.read_bytes()).hexdigest()
 checksum=base.parent/'.cargo-checksum.json'
 if base.parent.name=='sdl2-sys-0.37.0' and checksum.is_file():
  content=json.loads(checksum.read_text());name='SDL/'+relative.as_posix()
  if name in content['files'] and content['files'][name]!=changed:
   content['files'][name]=changed
   checksum.write_text(json.dumps(content,separators=(',',':')))
 records.append(dict(file=path.relative_to(root).as_posix(),original_sha256=original,patched_sha256=changed,
  modified_date='2026-10-26',
  change='Use ALooper_pollOnce; pollAll was removed from current NDK headers. Queue read still drains pending sensor events.'))
(root/'android-sdl-patch.json').write_text(json.dumps(records,indent=2)+'\n')
print('Documented SDL NDK 28 sensor compatibility patch applied to Android-only copies.')
