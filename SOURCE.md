# Source delivery

Matching sources accompany the Windows and Android binaries in
[Releases](https://github.com/Zaliba1/hunter-island-preservation/releases).

- HunterIsland-Source-0.2.0-preview.2.zip
- HunterIsland-SourceDependencies-0.2.0-preview.2.zip

Retain both archives beside this version's binaries. Sources for preview.1
remain attached to its own release; use the archives matching your binary.

Source includes this modified work, controlling build scripts, tests and notices.
SourceDependencies includes complete pinned native/Rust sources, full Boost and
published guest-library source releases. BUILD.md documents rebuilding and
OpenAL replacement/relinking. Developers generate their own Android signing
key; the project's private key is excluded.

The root LICENSE covers project source under MPL-2.0.
The combined emulator binary follows upstream's GPL-3.0-or-later declaration.
Historical Apple guest binaries have not been independently reproduced byte for
byte. Published source releases and original build files accompany them.
Rebuilding requires a suitable historical platform/compiler environment.
No Apple SDK or game is included.
