"""Explicit local export. Default report contains no saves, IPA, or raw game log."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import platform
import re
import zipfile
from . import VERSION, ipa
from .storage import clean_tree

def redact(text, roots=()):
    for root in sorted((str(p) for p in roots if str(p)), key=len, reverse=True):
        text = re.sub(re.escape(root), '<LOCAL-PATH>', text, flags=re.I)
        text = re.sub(re.escape(root.replace('\\', '\\\\')), '<LOCAL-PATH>', text, flags=re.I)
    text = re.sub(r'(?i)[a-z]:[\\/][^\r\n\"<>]*', '<LOCAL-PATH>', text)
    text = re.sub(r'(?i)(?:/home/|/Users/)[^/\s]+', '<USER-HOME>', text)
    text = re.sub(r'[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}', '<EMAIL>', text)
    text = re.sub(r'\bS-1-(?:\d+-)+\d+\b', '<SID>', text)
    return text

def export(store, target, with_saves=False):
    target = Path(target)
    if target.exists():
        raise ValueError('Bitte einen neuen Dateinamen wählen; vorhandene Dateien werden nicht überschrieben')
    roots = [store.root, Path.home(), os.environ.get('USERNAME', '')]
    with store.lock():
        config = store.settings()
        from .app import mod_arguments
        report = dict(schema='hunter-tester-report-v1', launcher=VERSION,
            created_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
            windows=platform.release(), os_version=platform.version(), architecture=platform.machine(),
            game=config.get('game'), device=config.get('device','ipad'), active_mods=mod_arguments(config), includes_saves=with_saves,
            raw_logs_included=False, emulator_savestate=False)
        executable=store.run/'touchHLE.exe'
        report['emulator_sha256']=ipa.digest(executable) if executable.is_file() else None
        isolation = store.logs / 'game-isolation.json'
        if isolation.exists():
            data = json.loads(isolation.read_text())
            report['last_run'] = {key:data.get(key) for key in ('is_appcontainer','capabilities','sid_verified','resumed','exit_code','timed_out')}
        game_log = store.logs / 'game.log'
        panic_lines = []
        if game_log.exists():
            # Extract only panic location/assertion and symbolic frames. Arbitrary
            # guest messages can contain player-entered text or game resources.
            with game_log.open(encoding='utf-8',errors='replace') as stream:
                for line in stream:
                    if len(line) <= 4096 and (re.match(r"^thread '(?:main|<unnamed>)' panicked at ",line) or re.match(r'^\s*\d+:\s+(?:touchHLE|std::|core::)',line)):
                        panic_lines.append(redact(line, roots))
                        panic_lines = panic_lines[-100:]
        files = []
        if with_saves:
            clean_tree(store.bundle)
            files = sorted(p for p in store.bundle.rglob('*') if p.is_file()) if store.bundle.exists() else []
            if len(files) > 10000 or sum(p.stat().st_size for p in files) > 512 * 1024**2:
                raise ValueError('Spielstandexport zu groß')
        temp = target.with_name(target.name + '.partial')
        if temp.exists():
            raise ValueError('Eine unvollständige Exportdatei liegt bereits am Ziel')
        try:
            with zipfile.ZipFile(temp,'x',zipfile.ZIP_DEFLATED) as archive:
                records = {}
                prefix = 'diagnostics/' if with_saves else ''
                diagnostics = {
                    prefix+'report.json': json.dumps(report,indent=2,ensure_ascii=False).encode('utf-8'),
                    prefix+'panic.txt': (''.join(panic_lines) or 'No captured panic location or symbolic frames.\n').encode('utf-8'),
                }
                for name, data in diagnostics.items():
                    archive.writestr(name,data)
                    records[name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
                for p in files:
                    before = p.stat()
                    if before.st_size > 128 * 1024**2:
                        raise ValueError('Einzelne Spielstanddatei ist zu groß')
                    data = p.read_bytes()
                    after = p.stat()
                    if (before.st_size,before.st_mtime_ns) != (after.st_size,after.st_mtime_ns):
                        raise RuntimeError('Spielstand wurde während des Exports verändert')
                    name = 'guest-data/'+p.relative_to(store.bundle).as_posix()
                    archive.writestr(name,data)
                    records[name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),
                                         mtime_ns=after.st_mtime_ns)
                if with_saves:
                    manifest = dict(schema='hunter-game-save-debug-v1',bundle=ipa.BUNDLE,
                                    kind='game-save-and-diagnostics',emulator_savestate=False,
                                    created_utc=report['created_utc'],producer='Hunter Island Windows '+VERSION,
                                    files=records,save_files=[name for name in records if name.startswith('guest-data/')])
                    archive.writestr('manifest.json',json.dumps(manifest,indent=2))
            temp.rename(target)
        finally:
            temp.unlink(missing_ok=True)
    return target
