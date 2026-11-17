"""User data is outside the program. Nothing discovers or imports existing saves."""
from contextlib import contextmanager
from pathlib import Path
import datetime
import json
import os
import shutil
import stat
import uuid
import zipfile
from . import ipa

def atomic_json(path, value):
    temp = path.with_name(path.name + '.' + uuid.uuid4().hex + '.tmp')
    try:
        temp.write_text(json.dumps(value, indent=2, ensure_ascii=False), encoding='utf-8')
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)

def clean_tree(root):
    root = Path(root)
    if not root.exists():
        return
    for p in [root, *root.rglob('*')]:
        if p.is_symlink() or getattr(p.lstat(), 'st_file_attributes', 0) & 0x400:
            raise ValueError('Verknüpfungen/Junctions im Profil werden nicht unterstützt')

class Store:
    def __init__(self, data_root=None):
        if data_root is None:
            if os.name != 'nt' or not os.environ.get('LOCALAPPDATA'):
                raise RuntimeError('Windows mit LOCALAPPDATA wird benötigt')
            data_root = Path(os.environ['LOCALAPPDATA']) / 'HunterIslandPreservation'
        self.root = Path(data_root).resolve()
        self.run = self.root / 'profiles/default/run'
        self.logs = self.root / 'logs'
        self.backups = self.root / 'backups'
        self.config = self.root / 'settings.json'
        self.root.mkdir(parents=True, exist_ok=True)

    @contextmanager
    def lock(self):
        import msvcrt
        with (self.root / 'session.lock').open('a+b') as stream:
            if stream.seek(0, 2) == 0:
                stream.write(b'0'); stream.flush()
            stream.seek(0)
            try:
                msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
            except OSError as error:
                raise RuntimeError('Dieses Profil wird bereits verwendet. Bitte den anderen Launcher/das Spiel schließen.') from error
            try:
                yield
            finally:
                stream.seek(0)
                msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)

    @property
    def bundle(self):
        testing = self.settings().get('mods', {}).get('--hunter-test-save', False)
        name = 'pockettrend.hunterisland-test-v1' if testing else ipa.BUNDLE
        return self.run / 'touchHLE_sandbox' / name

    def require_testing_seed(self):
        if not self.settings().get('mods', {}).get('--hunter-test-save', False):
            return
        if (self.bundle / 'test-profile.json').is_file():
            return
        normal = self.run / 'touchHLE_sandbox' / ipa.BUNDLE / 'Documents'
        if not any(p.is_file() and p.name[3:].isdigit() for p in normal.glob('saves*/sv_*')):
            raise ValueError('Bitte zuerst ohne Testspielstand-Haken ein normales Spiel starten und speichern. Danach kann daraus die separate Testkopie erstellt werden.')

    def settings(self):
        if not self.config.exists():
            return dict(device='ipad')
        value = json.loads(self.config.read_text(encoding='utf-8'))
        if value.get('device', 'ipad') not in ('ipad', 'iphone'):
            raise ValueError('Ungültiges Geräteprofil')
        return value

    def import_ipa(self, source):
        with self.lock():
            games = self.root / 'games'
            games.mkdir(exist_ok=True)
            clean_tree(games)
            temp = games / (uuid.uuid4().hex + '.importing')
            try:
                if Path(source).stat().st_size > ipa.MAX_ARCHIVE:
                    raise ValueError('IPA zu groß')
                total=0
                with Path(source).open('rb') as incoming, temp.open('xb') as outgoing:
                    for chunk in iter(lambda:incoming.read(1024*1024),b''):
                        total+=len(chunk)
                        if total>ipa.MAX_ARCHIVE:
                            raise ValueError('IPA überschreitet das Importlimit während des Kopierens')
                        outgoing.write(chunk)
                info = ipa.inspect(temp)
                dest = games / (info['sha256'] + '.ipa')
                if dest.exists():
                    if ipa.digest(dest) != info['sha256']:
                        raise ValueError('Vorhandene IPA-Prüfsumme stimmt nicht')
                else:
                    os.replace(temp, dest)
                config = self.settings()
                config['game'] = info
                atomic_json(self.config, config)
                return info
            finally:
                temp.unlink(missing_ok=True)

    def game_path(self):
        info = self.settings().get('game')
        if not info or len(info.get('sha256', '')) != 64 or any(c not in '0123456789abcdef' for c in info['sha256']):
            raise ValueError('Bitte zuerst deine eigene Hunter-Island-IPA importieren')
        path = self.root / 'games' / (info['sha256'] + '.ipa')
        if ipa.digest(path) != info['sha256']:
            raise ValueError('Importierte IPA fehlt oder wurde verändert; bitte erneut importieren')
        return path

    def stage_runtime(self, release):
        release = Path(release)
        manifest = json.loads((release / 'runtime-manifest.json').read_text())
        self.run.mkdir(parents=True, exist_ok=True)
        clean_tree(self.run)
        required = {'touchHLE.exe', 'isolation-probe.exe', 'touchHLE_default_options.txt'}
        if not required.issubset(manifest['files']):
            raise ValueError('Unvollständiges Programmpaket')
        # Only this explicit release manifest is copied. Never a profile/tree sync.
        for name, record in manifest['files'].items():
            p = ipa.safe_name(name)
            if p.parts[0] not in {'touchHLE.exe', 'isolation-probe.exe', 'touchHLE_default_options.txt', 'touchHLE_fonts', 'touchHLE_dylibs'}:
                raise ValueError('Nicht erlaubte Laufzeitdatei')
            source = release / 'runtime' / name
            if source.is_symlink() or not source.resolve().is_relative_to((release / 'runtime').resolve()):
                raise ValueError('Verlinkte Laufzeitdatei')
            if ipa.digest(source) != record['sha256']:
                raise ValueError('Programmpaket beschädigt: ' + name)
            target = self.run.joinpath(*p.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            if not target.exists() or ipa.digest(target) != record['sha256']:
                temp = target.with_suffix(target.suffix + '.updating')
                shutil.copyfile(source, temp)
                os.replace(temp, target)
        source = self.game_path()
        target = self.run / 'HunterIsland.ipa'
        expected = self.settings()['game']['sha256']
        if not target.exists() or ipa.digest(target) != expected:
            temp = self.run / 'HunterIsland.importing'
            shutil.copyfile(source, temp)
            os.replace(temp, target)
        for name in ('temp', 'profile'):
            (self.run / name).mkdir(exist_ok=True)
        return self.run / 'touchHLE.exe'

    def backup(self):
        clean_tree(self.bundle)
        files = sorted(p for p in self.bundle.rglob('*') if p.is_file()) if self.bundle.exists() else []
        if not files:
            return None
        if len(files) > 10000 or sum(p.stat().st_size for p in files) > 512 * 1024**2:
            raise ValueError('Spielstand-Sicherung überschreitet das Limit')
        self.backups.mkdir(exist_ok=True)
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S')
        path = self.backups / (stamp + '-' + uuid.uuid4().hex[:8] + '.zip')
        temp = path.with_suffix('.partial')
        try:
            with zipfile.ZipFile(temp, 'x', zipfile.ZIP_DEFLATED) as archive:
                for p in files:
                    archive.write(p, 'guest-data/' + p.relative_to(self.bundle).as_posix())
            temp.replace(path)
        finally:
            temp.unlink(missing_ok=True)
        # Bounded local backup retention. These are private, never release inputs.
        for old in sorted(self.backups.glob('*.zip'))[:-20]:
            old.unlink()
        return path
