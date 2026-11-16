# SPDX-License-Identifier: MPL-2.0
from pathlib import Path
import hashlib
import io
import json
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from add_locked_crate import install


class LockedCrateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.crate = self.root / 'fixture-1.0.0.crate'
        self.lock = self.root / 'Cargo.lock'

    def fixture(self, member='fixture-1.0.0/src/lib.rs', symlink=False):
        with tarfile.open(self.crate, 'w:gz') as archive:
            info = tarfile.TarInfo(member)
            if symlink:
                info.type = tarfile.SYMTYPE
                info.linkname = '../../outside'
                archive.addfile(info)
            else:
                info.size = 4
                archive.addfile(info, io.BytesIO(b'test'))
        digest = hashlib.sha256(self.crate.read_bytes()).hexdigest()
        self.lock.write_text(f'[[package]]\nname="fixture"\nversion="1.0.0"\nsource="registry+https://example.invalid"\nchecksum="{digest}"\n')

    def test_verified_source_is_idempotent_and_matches_cargo_checksum_format(self):
        self.fixture()
        for _ in range(2):
            install(self.crate, self.root / 'vendor', self.lock)
        checksum = json.loads((self.root / 'vendor/fixture-1.0.0/.cargo-checksum.json').read_text())
        self.assertEqual(checksum['files']['src/lib.rs'], hashlib.sha256(b'test').hexdigest())
        (self.root / 'vendor/fixture-1.0.0/src/lib.rs').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'Existing crate'):
            install(self.crate, self.root / 'vendor', self.lock)

    def test_lock_mismatch_is_rejected_before_writing_sources(self):
        self.fixture()
        self.crate.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'Cargo.lock'):
            install(self.crate, self.root / 'vendor', self.lock)
        self.assertFalse((self.root / 'vendor').exists())

    def test_traversal_and_links_are_rejected_without_partial_installation(self):
        for member, link in [('fixture-1.0.0/../../outside', False), ('fixture-1.0.0/link', True)]:
            with self.subTest(member=member):
                self.fixture(member, link)
                with self.assertRaisesRegex(ValueError, 'Unsafe'):
                    install(self.crate, self.root / 'vendor', self.lock)
                self.assertFalse((self.root / 'vendor/fixture-1.0.0').exists())


if __name__ == '__main__':
    unittest.main()
