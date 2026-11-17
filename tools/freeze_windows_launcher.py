# SPDX-License-Identifier: MPL-2.0
"""Bundle the optional Windows launcher and its app-local dependencies."""
import json
import os
import shutil
import struct
import subprocess
import sys
from importlib.metadata import distribution
from pathlib import Path


def freeze_launcher(root: Path, package: Path, work: Path) -> bool:
    if sys.platform != 'win32' or struct.calcsize('P') != 8 or sys.version_info[:3] != (3, 11, 9):
        raise RuntimeError('Use official 64-bit CPython 3.11.9 to match the supplied Windows runtime sources.')

    pyinstaller = distribution('pyinstaller')
    work.mkdir(parents=True, exist_ok=True)
    build_environment = dict(os.environ, PYINSTALLER_CONFIG_DIR=str(work.resolve() / 'cache'))
    subprocess.run([
        sys.executable, '-m', 'PyInstaller', '--noconfirm', '--clean', '--onedir',
        '--windowed', '--noupx', '--name', 'HunterIsland-Launcher',
        # The offline launcher only needs hashlib's built-in SHA-256.
        '--exclude-module', '_hashlib', '--exclude-module', 'ssl',
        '--specpath', str(work), '--workpath', str(work / 'build'),
        '--distpath', str(work / 'dist'), str(root / 'launcher/launch.py'),
    ], check=True, env=build_environment)

    frozen = work / 'dist/HunterIsland-Launcher'

    def windows_system_libraries(_directory, names):
        # Supported Windows versions supply their own UCRT and API sets.
        return [name for name in names if name.lower() == 'ucrtbase.dll'
                or name.lower().startswith(('api-ms-win-', 'ext-ms-win-'))]

    redistributable = frozen / '_internal/VCRUNTIME140.dll'
    if redistributable.exists():
        supplied = Path(sys.base_prefix) / 'vcruntime140.dll'
        if not supplied.is_file() or supplied.read_bytes() != redistributable.read_bytes():
            raise ValueError('VC runtime must come from the official CPython installation.')
    for extension in (frozen / '_internal').glob('*.pyd'):
        supplied = Path(sys.base_prefix) / 'DLLs' / extension.name
        if not supplied.is_file() or supplied.read_bytes() != extension.read_bytes():
            raise ValueError(f'Unexpected Python extension: {extension.name}')

    shutil.copy2(frozen / 'HunterIsland-Launcher.exe', package)
    shutil.copytree(frozen / '_internal', package / '_internal', ignore=windows_system_libraries)

    notices = package / 'licenses/runtime'
    copying = pyinstaller.read_text('licenses/COPYING.txt')
    if not copying:
        raise ValueError('Missing PyInstaller distribution notice.')
    (notices / 'PyInstaller-COPYING.txt').write_text(copying, encoding='utf-8')
    provenance = {
        'python': sys.version.split()[0],
        'pyinstaller': pyinstaller.version,
        'pyinstaller_source': f'https://github.com/pyinstaller/pyinstaller/tree/v{pyinstaller.version}',
        'windows_runtime_notices': 'licenses/runtime/',
        'windows_runtime_sources': 'runtime/ in the matching SourceDependencies ZIP',
        'app_local_vc_runtime': redistributable.is_file(),
    }
    (package / 'windows-launcher-provenance.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
    (package / 'START-HERE.txt').write_text(
        'Extract the entire ZIP and double-click HunterIsland-Launcher.exe.\n'
        'Keep the _internal and runtime folders beside the launcher.\n'
        'Python and Tk are included. Select Eigene IPA importieren to choose your game file,\n'
        'then select Spiel starten to play. See SETUP.md for more details.\n',
        encoding='utf-8',
    )
    return redistributable.is_file()
