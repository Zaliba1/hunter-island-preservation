# SPDX-License-Identifier: MPL-2.0
from pathlib import Path
import copy,hashlib,json,os,subprocess,sys,tempfile,unittest,zipfile
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'tools'));sys.path.insert(0,str(ROOT/'launcher'))
from prepare_github_release import validate_release
from hunter_launcher import VERSION
from audit_public import audit

class ReleaseGateTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name)
        self.engine=b'engine fixture, never executed';self.sha=hashlib.sha256(self.engine).hexdigest()
        self.names=[f'HunterIsland-{kind}-{VERSION}.zip' for kind in ('Windows-x64','Source','SourceDependencies')]
        self.windows={'runtime/touchHLE.exe':self.engine,'runtime-manifest.json':json.dumps(dict(version=VERSION,files={'touchHLE.exe':dict(sha256=self.sha)})),
            'licenses/GPL-3.0.txt':'license fixture\n'*3000}
        self.source={'launcher/hunter_launcher/__init__.py':f'VERSION = {VERSION!r}\n','LICENSES.md':'Combined binary GPL-3.0-or-later'}
        source=b'original public source fixture';rust=b'crate fixture'
        self.dependencies={'archive-manifest.json':json.dumps(dict(archives=[dict(path='native/library.zip',sha256=hashlib.sha256(source).hexdigest())])),
            'native/library.zip':source,'rust-files.json':json.dumps({'crate/lib.rs':hashlib.sha256(rust).hexdigest()}),'rust/crate/lib.rs':rust}
        self.checks=dict(version=VERSION,identified_license_delivery_checks_passed=True,private_marker_audit_passed=True,
            validation=dict(version=VERSION,engine_sha256=self.sha),distribution=dict(version=VERSION,engine_sha256=self.sha,windows_zip_extracted_and_run=True),
            source_delivery=dict(archives=1,rust_files=1,all_checksums_verified=True))
    def tearDown(self):self.temp.cleanup()
    def write(self):
        for name,files in zip(self.names,(self.windows,self.source,self.dependencies)):
            with zipfile.ZipFile(self.root/name,'w',zipfile.ZIP_DEFLATED) as z:
                for path,data in files.items():z.writestr(path,data)
        (self.root/'license-review.md').write_text('test review fixture')
        (self.root/'release-checks.json').write_text(json.dumps(self.checks))
        names=self.names+['license-review.md','release-checks.json']
        (self.root/'SHA256SUMS.txt').write_text(''.join(hashlib.sha256((self.root/n).read_bytes()).hexdigest()+'  '+n+'\n' for n in names))
    def test_consistent_release_validates_locally_without_upload(self):
        self.write();self.assertEqual(validate_release(self.root)['engine_sha256'],self.sha)
    def test_explicit_publication_block_is_honored(self):
        self.write();(self.root/'DO-NOT-PUBLISH.md').write_text('blocked')
        with self.assertRaisesRegex(ValueError,'explicitly blocked'):validate_release(self.root)
    def test_truthy_text_does_not_count_as_passed_review(self):
        self.checks['private_marker_audit_passed']='false';self.write()
        with self.assertRaisesRegex(ValueError,'gate not satisfied'):validate_release(self.root)
    def test_mismatched_test_evidence_and_source_are_rejected(self):
        self.checks['distribution']['version']='another-version';self.write()
        with self.assertRaisesRegex(ValueError,'version differs'):validate_release(self.root)
        self.checks['distribution']['version']=VERSION
        self.source['launcher/hunter_launcher/__init__.py']="VERSION='wrong'\n";self.write()
        with self.assertRaisesRegex(ValueError,'Source ZIP version'):validate_release(self.root)
    def test_changed_engine_is_rejected_even_after_outer_hash_update(self):
        self.windows['runtime/touchHLE.exe']=b'different engine';self.write()
        with self.assertRaisesRegex(ValueError,'engine hash mismatch'):validate_release(self.root)
    def test_changed_dependency_source_is_rejected(self):
        self.dependencies['rust/crate/lib.rs']=b'changed';self.write()
        with self.assertRaisesRegex(ValueError,'Rust source hash mismatch'):validate_release(self.root)
    def test_audit_rejects_binary_keys_and_other_private_pem_headers(self):
        key=self.root/'signing.p12';key.write_bytes(b'binary private fixture')
        with self.assertRaisesRegex(ValueError,'private-data filename'):audit(self.root)
        key.unlink();pem=self.root/'secret.pem';pem.write_bytes(b'-----BEGIN '+b'RSA PRIVATE KEY-----\nfixture')
        with self.assertRaisesRegex(ValueError,'credential marker'):audit(self.root)
    def test_desktop_cli_error_returns_without_interactive_pause(self):
        command=f'"{Path(os.environ["SystemRoot"])/"System32/cmd.exe"}" /d /s /c ""{ROOT/"Start.cmd"}" --unknown-test-argument"'
        result=subprocess.run(command,capture_output=True,text=True,stdin=subprocess.DEVNULL,timeout=15)
        self.assertEqual(result.returncode,2,result.stdout+result.stderr)
        self.assertIn('unrecognized arguments',result.stderr)

if __name__=='__main__':unittest.main()
