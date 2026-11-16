# Matching Android sources

Windows and Android share one source tree. The preview.4 APK is accompanied by:

- HunterIsland-Source-0.2.0-preview.4.zip
- HunterIsland-SourceDependencies-0.2.0-preview.4.zip

Keep both archives alongside locally supplied APKs. Published builds provide
them in [Releases](https://github.com/Zaliba1/hunter-island-preservation/releases).
Retain the matching sources when sharing the APK. Older APKs retain their
corresponding sources in their respective releases.

Source includes the modified emulator, Android launcher, controlling build
scripts, tests, notices, provenance and SDL compatibility patch.
The dependency archive supplies native sources and Cargo-vendored dependencies.
Follow BUILD.md to rebuild with your own signing key. The project key is not
required to modify, debug, rebuild or install your own copy.
OpenAL can be changed and relinked with tools/build-android.ps1 -RebuildOpenAL.

Historical guest libraries retain their original notices and published source
releases. Their historical SDK binaries have not been independently reproduced
byte for byte. LICENSE_REVIEW.md documents this provenance boundary.
