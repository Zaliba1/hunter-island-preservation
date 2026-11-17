# Hunter Island Preservation

An unofficial project for preserving Hunter Island 2.02 offline on Windows
and Android, based on [touchHLE v0.3.0](https://github.com/touchHLE/touchHLE).
Import your own compatible, already decrypted 32-bit IPA locally.

The shared source tree also contains an experimental native **Apple Silicon Mac**
app for macOS 13 or later. See [MACOS.md](MACOS.md) for test-build downloads,
installation, data locations and the remaining gameplay checks.

## Current release

**0.2.0-preview.9** adds an optional fresh campaign with three copies of each
regular monster form, all at level 99 and S rarity. It uses its own save folder
and grants no extra resources. Newly created full and level-one collections
also receive S rarity; existing campaigns keep their monsters and levels.
See [TEST_PROFILE.md](TEST_PROFILE.md).
It also includes the Orlen gravestone fix,
the optional mods, local save history, fusion lineage and saved game-language
selector, with English as the default.
The release includes matching sources, dependency sources and license
inventories. See VALIDATION.md for the checks performed and remaining limits.

## Play

Download the package for your device from
[Releases](https://github.com/Zaliba1/hunter-island-preservation/releases).

| Platform | Download | Requirements |
| --- | --- | --- |
| Windows PC | HunterIsland-Windows-x64-0.2.0-preview.9.zip | Windows 10/11 x64, 64-bit Python 3.11+ with Tcl/Tk, OpenGL drivers |
| Android | HunterIsland-Android-arm64-0.2.0-preview.9.apk | Android 8+, ARM64, OpenGL ES 3.0 |
| Apple Silicon Mac | HunterIsland-macOS-arm64-test-<commit>.zip | macOS 13+, ARM64; experimental, ad-hoc signed |

**Windows:** extract the entire ZIP, open Start.cmd, import your IPA and start
the game. Install Python with Tcl/Tk from
[python.org](https://www.python.org/downloads/windows/).
For instructions to build a local Windows ZIP with Python and Tcl/Tk included,
see [BUILD.md](BUILD.md#build-and-check-windows).

**Android:** install the APK, open the app, import your IPA and start the game.
Android may ask you to allow APK installation from your download or file app.

The [setup guide](SETUP.md) covers installation, updates, display modes,
save transfer and bug reports. It identifies the current launchers' German
button labels. The [optional mods and testing collection](MODS.md) are disabled initially.
Both launchers remember your choices.

## Develop

The shared emulator source is in emulator/, the Windows launcher in launcher/,
and the Android launcher in emulator/android/app/.
[BUILD.md](BUILD.md) explains rebuilding from a fresh source copy, the required
tools and the matching dependency source archive.

Matching source archives accompany the binaries.
[LICENSES.md](LICENSES.md) maps the licenses;
[LICENSE_REVIEW.md](LICENSE_REVIEW.md) records the review and its limits.
[VALIDATION.md](VALIDATION.md) lists the checks actually performed.

This project is independent of touchHLE, Apple and the game's developer.
Project documentation, release notes and commit messages are maintained in
English. Game and launcher interface languages are separate from the
documentation language.
