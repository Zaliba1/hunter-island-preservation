# Source delivery

The latest packaged release is
[0.2.0-preview.1](https://github.com/Zaliba1/hunter-island-preservation/releases/tag/v0.2.0-preview.1).
Its matching source assets are:

- HunterIsland-Source-0.2.0-preview.1.zip
- HunterIsland-SourceDependencies-0.2.0-preview.1.zip

The current Git source is unreleased **0.2.0-preview.2**, including the arena
Rewards fix. There is no preview.2 binary or source-asset release yet.
The dependency revisions are unchanged; use the preview.1 SourceDependencies
archive when building the current checkout, following BUILD.md.

For a future preview.2 binary release, supply the matching preview.2 Source and
SourceDependencies archives alongside the binaries. Retain sources for each
distributed version.

Source includes the modified work, controlling build scripts, tests and notices.
SourceDependencies includes complete pinned native/Rust sources, full Boost and
published guest-library source releases. Developers use their own Android key;
the project's private key is excluded.

The root LICENSE covers project source under MPL-2.0.
The combined emulator binary follows upstream's GPL-3.0-or-later declaration.
Historical Apple guest binaries have not been independently reproduced byte for
byte. Published source releases and original build files accompany the released
binaries. Rebuilding them requires a suitable historical platform/compiler
environment. No Apple SDK or game is included.
