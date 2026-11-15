# Matching Android sources

Windows and Android share one source tree. This checkout is unreleased
0.2.0-preview.2. No APK containing the final arena Rewards changes is published.

The existing preview.1 APK is accompanied by
HunterIsland-Source-0.2.0-preview.1.zip and
HunterIsland-SourceDependencies-0.2.0-preview.1.zip in
[Releases](https://github.com/Zaliba1/hunter-island-preservation/releases).

When distributing a future preview.2 APK, supply its corresponding
HunterIsland-Source-0.2.0-preview.2.zip and
HunterIsland-SourceDependencies-0.2.0-preview.2.zip.
These names describe the required future assets; they are not currently
available downloads.

Source includes the modified emulator, Android launcher, controlling build
scripts, tests, notices, provenance and SDL compatibility patch.
The dependency archive supplies native sources and Cargo-vendored dependencies.
Follow BUILD.md to build the current checkout with your own signing key.
The project key is not required to modify, debug, rebuild or install your copy.
OpenAL can be changed and relinked using
tools/build-android.ps1 -RebuildOpenAL.

Historical guest libraries retain their original notices and published source
releases. Their historical SDK binaries have not been independently reproduced
byte for byte. LICENSE_REVIEW.md documents this provenance boundary.
