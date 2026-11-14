# Source delivery

Matching sources for the Windows/Android preview accompany the binaries in
[GitHub releases](https://github.com/Zaliba1/hunter-island-preservation/releases).

- HunterIsland-Source-0.2.0-preview.1.zip
- HunterIsland-SourceDependencies-0.2.0-preview.1.zip

Retain both assets beside each binary release. Recipients need access to the
matching release and sources. Consider private repository access controls when
sharing direct APK or ZIP copies.

Source includes this modified work, controlling build scripts, Android changes,
tests and notices. SourceDependencies includes complete pinned native/Rust
sources, full Boost and published guest-library source releases.

[BUILD.md](BUILD.md) documents rebuilding and OpenAL replacement/relinking with
the supplied work. Developers generate their own Android key to build, modify
and install the software. The project's private key is excluded.

The root LICENSE covers project source under MPL-2.0.
The combined emulator binary follows upstream's GPL-3.0-or-later declaration.
Historical Apple guest library binaries have not been independently reproduced
byte for byte. Complete published source releases and original build files
accompany them. Rebuilding requires a suitable historical platform/compiler
environment. No Apple SDK or game is included.
