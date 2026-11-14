# Matching Android sources

Android and Windows share one source tree.

Supply HunterIsland-Source-0.2.0-preview.1.zip and
HunterIsland-SourceDependencies-0.2.0-preview.1.zip beside the APK.
Both accompany the project release at\nhttps://github.com/Zaliba1/hunter-island-preservation/releases.

Source includes modified emulator, Android launcher, build scripts, tests,
licenses, provenance and SDL compatibility patch.
SourceDependencies supplies native sources and Cargo-vendored dependencies.
Full terms are accessible offline through the app's license browser.

Follow BUILD.md to rebuild with your own signing key.
The project key is not required for modifying or debugging the library,
rebuilding or installing your own copy. OpenAL can be changed and relinked
using tools/build-android.ps1 -RebuildOpenAL.

Historical guest libraries retain their original notices and published source
releases. Their historical SDK binaries have not been independently reproduced
byte for byte. LICENSE_REVIEW.md documents this provenance boundary.
