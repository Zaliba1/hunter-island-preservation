# SPDX-License-Identifier: MPL-2.0
"""Resource I/O from the Orlen mod, for the same fingerprinted retail IPA."""
from base64 import b64decode, b64encode
from hashlib import sha256
import json
import os
from pathlib import Path
import plistlib
import re
import struct
import subprocess

def unsupported():
    return ValueError("Unsupported Hunter Island 2.02 resource or executable.")


ARMV7_HASH = '5b9572690e5dba80c3f6244c2e45d1c50479b46ed91045f6fd036780178e3b08'

KEY_FRAGMENTS = (0x3c93e4, 0x3c9034, 0x3c91f4, 0x3c8db4, 0x3c7ca4, 0x3c8a14, 0x3c89b4, 0x3c8434)

def read_member(archive, name, max_size=1024 * 1024):
    info = archive.getinfo(name)
    if not 0 < info.file_size <= max_size:
        raise unsupported()
    return archive.read(info)

def resource_key(archive, bundle):
    metadata = plistlib.loads(read_member(archive, f'{bundle}/Info.plist'))
    executable = metadata['CFBundleExecutable']
    if not re.fullmatch(r'[A-Za-z0-9_.-]+', executable):
        raise unsupported()
    binary = read_member(archive, f'{bundle}/{executable}', max_size=32 * 1024 * 1024)
    if binary.startswith(b'\xca\xfe\xba\xbe'):
        count = struct.unpack_from('>I', binary, 4)[0]
        if count > 32:
            raise unsupported()
        for i in range(count):
            cpu, subtype, offset, size, _ = struct.unpack_from('>5I', binary, 8 + i * 20)
            if cpu == 12 and subtype & 255 == 9:
                binary = binary[offset:offset + size]
                break
    if sha256(binary).hexdigest() != ARMV7_HASH:
        raise unsupported()

    # The verified ARMv7 executable stores the resource key as CFString fragments.
    cursor = 28
    segments = []
    command_count = struct.unpack_from('<I', binary, 16)[0]
    for _ in range(command_count):
        command, size = struct.unpack_from('<II', binary, cursor)
        if command == 1:
            address, _, file_offset, file_size = struct.unpack_from('<4I', binary, cursor + 24)
            segments.append((address, file_offset, file_size))
        cursor += size

    def file_offset(address):
        return next(offset + address - base for base, offset, size in segments if base <= address < base + size)

    parts = []
    for address in KEY_FRAGMENTS:
        pointer, size = struct.unpack_from('<II', binary, file_offset(address) + 8)
        start = file_offset(pointer)
        parts.append(binary[start:start + size])
    return bytes.fromhex(b''.join(parts).decode('ascii'))

RESOURCE_CIPHER = r"""
$ErrorActionPreference = 'Stop'
$request = $input | Out-String | ConvertFrom-Json
$aes = [Security.Cryptography.Aes]::Create()
try {
    $aes.Mode = 'CBC'
    $aes.Padding = 'PKCS7'
    $aes.Key = [Convert]::FromBase64String($request.key)
    $aes.IV = [byte[]]::new(16)
    foreach ($member in $request.members) {
        $bytes = [Convert]::FromBase64String($member)
        $cipher = if ($request.encrypt) { $aes.CreateEncryptor() } else { $aes.CreateDecryptor() }
        try {
            [Convert]::ToBase64String($cipher.TransformFinalBlock($bytes, 0, $bytes.Length))
        } finally {
            $cipher.Dispose()
        }
    }
} finally {
    $aes.Dispose()
}
"""

def crypt_resources(key, members, *, encrypt=False):
    powershell = Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
    request = {'key': b64encode(key).decode('ascii'),
               'members': [b64encode(member).decode('ascii') for member in members],
               'encrypt': encrypt}
    result = subprocess.run(
        [str(powershell), '-NoProfile', '-NonInteractive', '-Command', RESOURCE_CIPHER],
        input=json.dumps(request).encode('ascii'), capture_output=True, timeout=30,
        creationflags=subprocess.CREATE_NO_WINDOW, check=True,
    )
    output = [b64decode(line, validate=True) for line in result.stdout.splitlines()]
    if len(output) != len(members):
        raise unsupported()
    return output
