# Rebuilding and source archives

## Inputs

For the current unreleased preview.2, use this Git checkout and the
HunterIsland-SourceDependencies-0.2.0-preview.1.zip archive from the existing
release. Dependency revisions are unchanged. No preview.2 download assets
have been published yet.

For a packaged release, use its matching Source and SourceDependencies archives,
or check out the source commit recorded in its release-checks.json.
No game copy or saves are required to build.

Source includes engine and launcher changes, build scripts, tests, license
texts and provenance. SourceDependencies includes pinned native archives,
complete Cargo packages, Boost and published source releases for the historical
guest libraries.

| Tool | Verified version |
| --- | --- |
| Python | 3.11+ |
| Rust GNU Windows | 1.90.0-x86_64-pc-windows-gnu |
| Winlibs GNU | GCC 15.2.0 / MinGW-w64 13.0.0 UCRT r3 |
| Boost | 1.81.0, included in SourceDependencies |
| Android JDK | Eclipse Temurin 17 |
| Gradle | 8.11.1 |
| Android Gradle Plugin | 8.10.1 |
| Android SDK | Platform 35, Build Tools 35.0.0 |
| Android NDK | 28.2.13676358 |
| Android CMake | 3.22.1 |

Install tools from their official projects.
toolchain-provenance.json records the Winlibs download and checksum.
Visual Studio is not required for this GNU build route.
The Windows isolation probe uses the .NET Framework C# compiler.

## Prepare sources

Extract a fresh source copy to C:/hi-src. Use a separate empty directory for
build outputs. Keep source and toolchain paths short and free of spaces.
The Windows output directory must be at most 45 characters long to avoid
excessively long native CPU-core dependency paths in GCC.

~~~powershell
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-linux-android --toolchain 1.90.0-x86_64-pc-windows-gnu
python tools/prepare_offline_sources.py C:/Downloads/HunterIsland-SourceDependencies-0.2.0-preview.1.zip
python -m venv .venv
./.venv/Scripts/python.exe -m pip install -r requirements-build.txt
~~~

Preparation verifies the archives and generates the local Cargo vendor
configuration. Running it again does not overwrite already prepared vendor
sources.

## Build and check Windows

~~~powershell
./tools/build-emulator.ps1 -BoostRoot ./emulator/vendor/boost -GnuRoot C:/Winlibs/mingw64 -TargetDir C:/hi-build/windows -Offline
./.venv/Scripts/python.exe -m unittest discover -s tests -v
./.venv/Scripts/python.exe tools/package_release.py --emulator-exe C:/hi-build/windows/x86_64-pc-windows-gnu/release/touchHLE.exe --dependency-sources .dependencies --out dist/windows
~~~

The package contains the portable launcher, runtime, license mapping and
manifest. The build runs engine tests. Packaging checks sources, license
entries and imported system libraries.

## Build and check Android

Adjust the JDK, SDK and Gradle paths to your installations.
Create the signing key outside the source tree.

~~~powershell
python tools/create_android_signing.py --java-home C:/Java17 --directory C:/hi-signing
./tools/build-android.ps1 -SdkRoot C:/Android/sdk -JavaHome C:/Java17 -GradleRoot C:/Gradle/gradle-8.11.1 -GnuRoot C:/Winlibs/mingw64 -SigningConfig C:/hi-signing/local-preview-signing.json -TargetDir C:/hi-build/android -Test
~~~

The APK is written to emulator/android/app/build/outputs/apk/debug/app-debug.apk.
-OfflineGradle uses existing Gradle/Maven caches. On the first build, Gradle
downloads the pinned build and test dependencies from the official repositories
configured in the project. Native Cargo builds use --frozen after source
preparation.

Your own signing key is sufficient for personal builds. The project's private
key is excluded from the repository. A build signed with your own key cannot
be installed as an update over the existing project APK.

## Modify and relink OpenAL

Edit emulator/vendor/openal-soft and pass -RebuildOpenAL to the relevant build.
This cleans the wrapper and rebuilds the statically linked library with the
project. tools/patch_android_sdl.py applies SDL's Android compatibility changes.

A successful rebuild verifies the documented build route. Different compiler
paths, signing keys and build times may produce different binary bytes.
VALIDATION.md records the checks actually performed.

## Check a release

Use the supplied tools to inspect the Android package.

~~~powershell
python tools/verify_android_apk.py --apk PATH/TO/app-debug.apk --sdk-root C:/Android/sdk --java-home C:/Java17 --out .build/android-review
python tools/android_license_inventory.py .build/android-review/HunterIsland-Android-arm64-0.2.0-preview.2.apk --out .build/android-license-inventory.json
python tools/audit_public.py .
~~~

Checks cover the signature, version fields, allowed APK payloads, ARM64 and
16-KiB alignment, runtime dependencies, embedded notices, and known local
identity/path markers. The inventory compares APK notices with source files
byte for byte.
