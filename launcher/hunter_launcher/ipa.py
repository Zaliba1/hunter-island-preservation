"""Bounded static inspection. Never executes, decrypts, or extracts an IPA."""
from pathlib import Path, PurePosixPath
import hashlib
import plistlib
import stat
import struct
import zipfile
from .language import LauncherInputError

BUNDLE = 'pockettrend.hunterisland'
MAX_ARCHIVE = 2 * 1024**3
MAX_MEMBER = 512 * 1024**2
MAX_TOTAL = 3 * 1024**3

def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()

def safe_name(name):
    p = PurePosixPath(name)
    if not name or '\\' in name or ':' in name or '\x00' in name or p.is_absolute() or '..' in p.parts:
        raise LauncherInputError('unsafe_archive')
    if p.as_posix() != name.rstrip('/'):
        raise LauncherInputError('unsafe_archive')
    return p

def macho_slices(data):
    if len(data) < 28:
        raise LauncherInputError('invalid_ipa')
    if data[:4] == b'\xca\xfe\xba\xbe':
        count = struct.unpack_from('>I', data, 4)[0]
        if not 1 <= count <= 16 or len(data) < 8 + count * 20:
            raise LauncherInputError('invalid_ipa')
        slices = []
        for i in range(count):
            _, _, offset, size, _ = struct.unpack_from('>5I', data, 8 + i * 20)
            if offset < 8 + count * 20 or size < 28 or offset + size > len(data):
                raise LauncherInputError('invalid_ipa')
            slices += macho_slices(data[offset:offset + size])
        return slices
    magic = data[:4]
    if magic not in (b'\xce\xfa\xed\xfe', b'\xcf\xfa\xed\xfe'):
        raise LauncherInputError('invalid_ipa')
    is64 = magic == b'\xcf\xfa\xed\xfe'
    cpu, subtype, _, ncmds, commands_size = struct.unpack_from('<5I', data, 4)
    start = 32 if is64 else 28
    end = start + commands_size
    if end > len(data) or ncmds > 10000:
        raise LauncherInputError('invalid_ipa')
    pos = start
    encrypted = False
    for _ in range(ncmds):
        if pos + 8 > end:
            raise LauncherInputError('invalid_ipa')
        cmd, size = struct.unpack_from('<2I', data, pos)
        if size < 8 or pos + size > end:
            raise LauncherInputError('invalid_ipa')
        if cmd in (0x21, 0x2C):
            if size < 20:
                raise LauncherInputError('invalid_ipa')
            encrypted |= struct.unpack_from('<I', data, pos + 16)[0] != 0
        pos += size
    if pos != end:
        raise LauncherInputError('invalid_ipa')
    return [dict(cpu=cpu, subtype=subtype, bits=64 if is64 else 32, encrypted=encrypted)]

def inspect(path, check_crc=True):
    path = Path(path)
    if path.stat().st_size > MAX_ARCHIVE:
        raise LauncherInputError('ipa_size')
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        if not 1 <= len(infos) <= 50000:
            raise LauncherInputError('invalid_ipa')
        total = 0
        names = set()
        plists = []
        for info in infos:
            safe_name(info.filename)
            folded = info.filename.rstrip('/').casefold()
            if folded in names or stat.S_ISLNK(info.external_attr >> 16) or info.flag_bits & 1:
                raise LauncherInputError('invalid_ipa')
            names.add(folded)
            total += info.file_size
            if info.file_size > MAX_MEMBER or total > MAX_TOTAL:
                raise LauncherInputError('ipa_size')
            p = PurePosixPath(info.filename)
            if len(p.parts) == 3 and p.parts[0] == 'Payload' and p.parts[1].endswith('.app') and p.parts[2] == 'Info.plist':
                if info.file_size > 1024**2:
                    raise LauncherInputError('invalid_ipa')
                plists.append(info.filename)
        if len(plists) != 1:
            raise LauncherInputError('invalid_ipa')
        metadata = plistlib.loads(archive.read(plists[0]))
        if not isinstance(metadata, dict) or metadata.get('CFBundleIdentifier') != BUNDLE:
            raise LauncherInputError('wrong_game')
        executable = metadata.get('CFBundleExecutable')
        if not isinstance(executable, str) or not executable or PurePosixPath(executable).name != executable:
            raise LauncherInputError('invalid_ipa')
        safe_name(executable)
        binary = str(PurePosixPath(plists[0]).parent / executable)
        slices = macho_slices(archive.read(binary))
        arm = [s for s in slices if s['cpu'] == 12 and s['bits'] == 32]
        if not arm:
            raise LauncherInputError('wrong_arch')
        if any(s['encrypted'] for s in arm):
            raise LauncherInputError('encrypted_ipa')
        if check_crc and archive.testzip() is not None:
            raise LauncherInputError('invalid_ipa')
    def label(key):
        value = metadata.get(key, '')
        if not isinstance(value, str) or len(value) > 128 or any(ord(c) < 32 for c in value):
            raise LauncherInputError('invalid_ipa')
        return value
    version = label('CFBundleShortVersionString') or label('CFBundleVersion')
    return dict(bundle=BUNDLE, version=version, minimum_ios=label('MinimumOSVersion'),
                architectures=slices, sha256=digest(path),
                compatibility='tested-version' if version == '2.02' else 'untested-version')
