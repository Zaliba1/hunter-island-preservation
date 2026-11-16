#!/bin/bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail
cd "$(dirname "$0")/.."
test "$(uname -s)" = Darwin && test "$(uname -m)" = arm64 || { echo 'Build on an Apple Silicon Mac.' >&2; exit 1; }
root="$PWD"
mkdir -p .build/macos dist/macos
export MACOSX_DEPLOYMENT_TARGET=13.0
export CMAKE_BUILD_PARALLEL_LEVEL=3 CARGO_BUILD_JOBS=3
export CARGO_TARGET_DIR="$root/.build/macos/target"
export RUSTFLAGS="--remap-path-prefix=$HOME=/build --remap-path-prefix=$root=/source"
export CFLAGS="-ffile-prefix-map=$HOME=/build -ffile-prefix-map=$root=/source -g0"
export CXXFLAGS="$CFLAGS"
if command -v cmake >/dev/null; then export CMAKE="$(command -v cmake)"; fi
cd emulator
cargo +1.90.0 test --locked --release --lib -- --test-threads=1
cargo +1.90.0 build --locked --release --bin touchHLE
cd "$root"
app="$root/dist/macos/Hunter Island.app"
test ! -e "$app" || { echo 'Use a fresh output directory; refusing to overwrite an app.' >&2; exit 1; }
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/notices"
cp "$CARGO_TARGET_DIR/release/touchHLE" "$app/Contents/MacOS/touchHLE"
swiftc -parse-as-library -swift-version 5 -O -target arm64-apple-macos13.0 macos/Core.swift macos/Launcher.swift -o "$app/Contents/MacOS/HunterIslandLauncher" -framework AppKit
swiftc -parse-as-library -swift-version 5 -O -target arm64-apple-macos13.0 macos/Probe.swift -o "$app/Contents/MacOS/isolation-probe"
swiftc -parse-as-library -swift-version 5 -O -target arm64-apple-macos13.0 macos/Core.swift macos/CoreTests.swift -o .build/macos/core-tests
python3 tools/macos_fixtures.py .build/macos/fixtures
.build/macos/core-tests .build/macos/fixtures "$app"
cp -R emulator/touchHLE_fonts emulator/touchHLE_dylibs "$app/Contents/Resources/"
cp emulator/touchHLE_default_options.txt macos/game.sb "$app/Contents/Resources/"
cp -R licenses "$app/Contents/Resources/notices/"
cp LICENSE LICENSES.md LICENSE_REVIEW.md THIRD_PARTY.md SOURCE.md MACOS.md guest-library-provenance.json "$app/Contents/Resources/notices/"
python3 - "$app" <<'PY'
import pathlib,plistlib,sys
p=pathlib.Path(sys.argv[1])/'Contents/Info.plist'
p.write_bytes(plistlib.dumps(dict(CFBundleIdentifier='org.hunterislandpreservation.macos.preview',CFBundleName='Hunter Island',CFBundleDisplayName='Hunter Island',CFBundleExecutable='HunterIslandLauncher',CFBundlePackageType='APPL',CFBundleShortVersionString='0.2.0',CFBundleVersion='2',LSMinimumSystemVersion='13.0',NSHighResolutionCapable=True,NSPrincipalClass='NSApplication')))
PY
codesign --force --sign - --options runtime --entitlements macos/engine-entitlements.plist "$app/Contents/MacOS/touchHLE"
codesign --force --sign - "$app/Contents/MacOS/isolation-probe"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
HI_MAC_TEST_DATA="$root/.build/macos/ui-profile" HI_MAC_UI_SNAPSHOT="$root/.build/macos/launcher.png" "$app/Contents/MacOS/HunterIslandLauncher" --ui-smoke
# Repeat isolation against the packaged probe and actual SB profile.
.build/macos/core-tests --isolation "$app"
# No original game is needed or supplied for the real GL render/readback probe.
HI_MAC_GRAPHICS_DATA="$root/.build/macos/graphics" .build/macos/core-tests --graphics "$app" > .build/macos/graphics-probe.log 2>&1
short=$(git rev-parse --short HEAD)
ditto -c -k --keepParent "$app" "dist/macos/HunterIsland-macOS-arm64-test-$short.zip"
git archive --format=zip --output="dist/macos/HunterIsland-Source-test-$short.zip" HEAD
python3 tools/verify_source_delivery.py .dependencies > dist/macos/dependency-checks.json
python3 - "$short" <<'PY'
from pathlib import Path
import sys,zipfile
root=Path('.dependencies')
with zipfile.ZipFile(f'dist/macos/HunterIsland-SourceDependencies-test-{sys.argv[1]}.zip','x',zipfile.ZIP_DEFLATED) as z:
    for p in sorted(root.rglob('*')):
        if p.is_symlink():raise ValueError('Unexpected dependency source symlink')
        if p.is_file():z.write(p,p.relative_to(root).as_posix())
PY
python3 tools/check_macos_package.py "$app" --out dist/macos/checks.json
cp MACOS.md dist/macos/README.md
python3 - <<'PY'
from pathlib import Path
import hashlib
p=Path('dist/macos')
(p/'SHA256SUMS.txt').write_text(''.join(hashlib.sha256(f.read_bytes()).hexdigest()+'  '+f.name+'\n' for f in sorted(p.iterdir()) if f.is_file() and f.name!='SHA256SUMS.txt'))
PY
