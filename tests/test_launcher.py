# SPDX-License-Identifier: MPL-2.0
from pathlib import Path
import json
import os
import plistlib
import struct
import sys
import tempfile
import unittest
import zipfile
import zlib
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'launcher'))
from hunter_launcher import ipa, diagnostics
from hunter_launcher.storage import Store, atomic_json
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from audit_public import audit
from prepare_offline_sources import extract
from license_inventory import inventory

def binary(cpu=12, encrypted=0):
    commands = struct.pack('<5I',0x21,20,0,0,encrypted)
    return struct.pack('<7I',0xfeedface,cpu,6,2,1,len(commands),0)+commands

def make_ipa(path, bundle=ipa.BUNDLE, executable=None, version='2.02', extra=()):
    with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('Payload/Game.app/Info.plist',plistlib.dumps(dict(CFBundleIdentifier=bundle,
            CFBundleExecutable='Game',CFBundleShortVersionString=version,MinimumOSVersion='5.1')))
        archive.writestr('Payload/Game.app/Game',executable or binary())
        for name,data in extra:
            archive.writestr(name,data)
    return path

class ImportTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name)
    def tearDown(self):
        self.temp.cleanup()
    def test_valid_arm_metadata_without_extraction(self):
        path=make_ipa(self.root/'game.ipa')
        info=ipa.inspect(path)
        self.assertEqual(info['version'],'2.02')
        self.assertEqual(info['architectures'][0]['bits'],32)
        self.assertEqual(sorted(p.name for p in self.root.iterdir()),['game.ipa'])
    def test_reject_other_game_encryption_and_non_arm(self):
        for kwargs in (dict(bundle='other.game'),dict(executable=binary(encrypted=1)),dict(executable=binary(cpu=7))):
            with self.subTest(kwargs=kwargs):
                with self.assertRaises(ValueError):
                    ipa.inspect(make_ipa(self.root/'bad.ipa',**kwargs))
    def test_reject_traversal_ambiguity_and_zip_symlink(self):
        for name in ('../escape','/absolute','Payload/Game.app/../../escape','Payload\\Game.app\\x','C:/escape','Payload//duplicate'):
            with self.subTest(name=name),self.assertRaises(ValueError):
                ipa.safe_name(name)
        path=make_ipa(self.root/'link.ipa')
        with zipfile.ZipFile(path,'a') as archive:
            info=zipfile.ZipInfo('Payload/Game.app/link');info.create_system=3;info.external_attr=0o120777<<16
            archive.writestr(info,'outside')
        with self.assertRaises(ValueError):ipa.inspect(path)
    def test_corrupt_zip_crc_is_rejected(self):
        path=make_ipa(self.root/'crc.ipa',extra=[('Payload/Game.app/data',b'checksum-target')])
        data=bytearray(path.read_bytes())
        with zipfile.ZipFile(path) as archive:
            info=archive.getinfo('Payload/Game.app/data')
            offset=info.header_offset
            name_len,extra_len=struct.unpack_from('<HH',data,offset+26)
            data[offset+30+name_len+extra_len]^=0xff
        path.write_bytes(data)
        with self.assertRaises((ValueError,zipfile.BadZipFile,zlib.error)):
            ipa.inspect(path)
    def test_malformed_load_commands_and_fat_bounds(self):
        for data in (b'x'*40,struct.pack('>2I',0xcafebabe,999),struct.pack('<7I',0xfeedface,12,6,2,1,8,0)+struct.pack('<2I',1,1000)):
            with self.subTest(data=data),self.assertRaises(ValueError):ipa.macho_slices(data)
    def test_untested_version_is_labelled(self):
        self.assertEqual(ipa.inspect(make_ipa(self.root/'new.ipa',version='9.0'))['compatibility'],'untested-version')
    def test_import_is_transactional_and_stores_no_source_path(self):
        store=Store(self.root/'profile')
        source=make_ipa(self.root/'personal-source-name.ipa')
        first=store.import_ipa(source)
        before=store.config.read_bytes()
        with self.assertRaises(ValueError):store.import_ipa(make_ipa(self.root/'bad.ipa',bundle='wrong'))
        self.assertEqual(store.config.read_bytes(),before)
        self.assertNotIn(str(source),before.decode())
        self.assertEqual(ipa.digest(store.game_path()),first['sha256'])
        self.assertFalse(store.bundle.exists())
        self.assertEqual(list((store.root/'games').glob('*.importing')),[])
    def test_profile_lock_rejects_concurrent_changes(self):
        store=Store(self.root/'profile')
        with store.lock():
            with self.assertRaises(RuntimeError):
                with Store(store.root).lock():pass
    def test_diagnostics_excludes_saves_and_arbitrary_guest_messages(self):
        store=Store(self.root/'profile');store.bundle.mkdir(parents=True)
        (store.bundle/'Save').write_bytes(b'private player save')
        store.logs.mkdir()
        (store.logs/'game.log').write_text("HunterIsland[0] PlayerName: personal-player panicked at private-message\nthread 'main' panicked at C:\\Users\\test-person\\private\\source.rs:4\n")
        path=diagnostics.export(store,self.root/'report.zip')
        with zipfile.ZipFile(path) as archive:
            self.assertEqual(set(archive.namelist()),{'report.json','panic.txt'})
            data=archive.read('panic.txt').decode()
            self.assertNotIn('test-person',data);self.assertNotIn('personal-player',data)
            self.assertIn('<LOCAL-PATH>',data)
            self.assertFalse(json.loads(archive.read('report.json'))['includes_saves'])
        explicit=diagnostics.export(store,self.root/'with-save.zip',True)
        with zipfile.ZipFile(explicit) as archive:
            self.assertEqual(archive.read('guest-data/Save'),b'private player save')
            manifest=json.loads(archive.read('manifest.json'))
            self.assertEqual(manifest['schema'],'hunter-game-save-debug-v1')
            self.assertEqual(manifest['bundle'],ipa.BUNDLE)
            self.assertIs(manifest['emulator_savestate'],False)
            self.assertEqual(set(manifest['files']),set(archive.namelist())-{'manifest.json'})
            for name, record in manifest['files'].items():
                data=archive.read(name)
                self.assertEqual(record['bytes'],len(data))
                self.assertEqual(record['sha256'],__import__('hashlib').sha256(data).hexdigest())
            self.assertIn('diagnostics/report.json',manifest['files'])
    def test_backup_contains_guest_data_only_and_preserves_game(self):
        store=Store(self.root/'profile');store.bundle.mkdir(parents=True)
        (store.bundle/'Save').write_bytes(b'player')
        (store.run/'HunterIsland.ipa').write_bytes(b'never export this')
        backup=store.backup()
        with zipfile.ZipFile(backup) as archive:
            self.assertEqual(archive.namelist(),['guest-data/Save'])
        self.assertEqual((store.bundle/'Save').read_bytes(),b'player')
    def test_runtime_manifest_cannot_copy_user_data_and_checks_hashes(self):
        store=Store(self.root/'profile');release=self.root/'release';release.mkdir()
        files={name:dict(sha256='0'*64) for name in ('touchHLE.exe','isolation-probe.exe','touchHLE_default_options.txt',
            '../Save')}
        files={'../Save':files['../Save'],**files}
        atomic_json(release/'runtime-manifest.json',dict(files=files))
        with self.assertRaisesRegex(ValueError,'Unsafe archive path'):store.stage_runtime(release)

    def test_public_audit_finds_compressed_identity_and_forbidden_data(self):
        root=self.root/'public';root.mkdir()
        (root/'private.ipa').write_bytes(b'not shipped')
        with self.assertRaisesRegex(ValueError,'private-data filename'):audit(root)
        (root/'private.ipa').unlink()
        with zipfile.ZipFile(root/'modules.zip','w',zipfile.ZIP_DEFLATED) as archive:
            archive.writestr('module.pyc',os.environ['USERPROFILE'].encode())
        with self.assertRaisesRegex(ValueError,'local identity/path marker'):audit(root)

    def test_redaction_covers_escaped_paths_and_sid(self):
        text=r'C:\\Users\\sample\\secret.txt S-1-15-2-123456 person@example.org'
        result=diagnostics.redact(text)
        self.assertNotIn('sample',result)
        self.assertNotIn('example.org',result)
        self.assertNotIn('S-1-15',result)

    def test_dependency_extraction_materializes_internal_file_link(self):
        path=self.root/'source.zip'
        with zipfile.ZipFile(path,'w') as z:
            z.writestr('real/header.h',b'public source')
            info=zipfile.ZipInfo('include/header.h');info.create_system=3;info.external_attr=0o120777<<16
            z.writestr(info,'../real/header.h')
        extract(path,self.root/'extracted')
        result=self.root/'extracted/include/header.h'
        self.assertEqual(result.read_bytes(),b'public source')
        self.assertFalse(result.is_symlink())

    def test_dependency_extraction_rejects_external_link(self):
        path=self.root/'source.zip'
        with zipfile.ZipFile(path,'w') as z:
            info=zipfile.ZipInfo('escape.h');info.create_system=3;info.external_attr=0o120777<<16
            z.writestr(info,'../../private.h')
        with self.assertRaisesRegex(ValueError,'Outside archive'):extract(path,self.root/'extracted')

    def test_dependency_extraction_rejects_traversal(self):
        path=self.root/'source.zip'
        with zipfile.ZipFile(path,'w') as z:z.writestr('../private.h',b'never write outside')
        with self.assertRaisesRegex(ValueError,'Unsafe source'):extract(path,self.root/'extracted')
        self.assertFalse((self.root/'private.h').exists())

    def test_license_inventory_rejects_unknown_or_host_os_binaries(self):
        root=self.root/'release';root.mkdir()
        path=root/'unknown.dll';path.write_bytes(b'MZ fixture')
        with self.assertRaisesRegex(ValueError,'Unclassified'):inventory(root)
        path.unlink();(root/'ucrtbase.dll').write_bytes(b'MZ OS fixture')
        with self.assertRaisesRegex(ValueError,'Windows OS'):inventory(root)

if __name__=='__main__':unittest.main()
