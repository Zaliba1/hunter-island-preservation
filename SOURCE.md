# Source delivery

Matching sources accompany locally supplied Windows and Android binaries.
Published builds provide them in
[Releases](https://github.com/Zaliba1/hunter-island-preservation/releases).

For the Windows FSR release:

- HunterIsland-Source-0.2.0-preview.9-fsr.1.zip
- HunterIsland-SourceDependencies-0.2.0-preview.9-fsr.1.zip
- HunterIsland-WindowsRuntimeSources-0.2.0-preview.9-fsr.1.zip

The standalone Windows variant also has a `HunterIsland-WindowsRuntimeSources-*.zip`
with the sources for its bundled MSYS2 Python/Tcl/Tk runtime and libraries.

Mac test artifacts use `HunterIsland-Source-test-<commit>.zip` and
`HunterIsland-SourceDependencies-test-<commit>.zip` beside the matching app ZIP.

Retain both archives beside this version's binaries. Sources for older builds
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

## Additional Arena Fights preview

The release's source ZIP includes the native engine, all six localizations,
the launcher integration and the authored inputs to the bundled AI policy.
Use the pinned [native and Rust dependency sources](https://github.com/Zaliba1/hunter-island-preservation/releases/download/v0.2.0-preview.15/HunterIsland-SourceDependencies-0.2.0-preview.15.zip)
and the matching [Windows launcher runtime sources](https://github.com/Zaliba1/hunter-island-preservation/releases/download/v0.2.0-preview.13/HunterIsland-WindowsRuntimeSources-0.2.0-preview.13.zip).
The QuickJS files used in the build are in `emulator/vendor/hunter-quickjs`.
