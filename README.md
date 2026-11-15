# Hunter Island Preservation

An unofficial project for preserving Hunter Island 2.02 offline on Windows
and Android, based on [touchHLE v0.3.0](https://github.com/touchHLE/touchHLE).
Import your own compatible, already decrypted 32-bit IPA locally.

## Current release

**0.2.0-preview.3** adds a saved game-language selector on Windows and Android,
with English as the default. It includes the arena Rewards fix from preview.2.
The release includes matching sources, dependency sources and license
inventories. See VALIDATION.md for the checks performed and remaining limits.

## Play

Download the package for your device from
[Releases](https://github.com/Zaliba1/hunter-island-preservation/releases).

| Platform | Download | Requirements |
| --- | --- | --- |
| Windows PC | HunterIsland-Windows-x64-0.2.0-preview.3.zip | Windows 10/11 x64, 64-bit Python 3.11+ with Tcl/Tk, OpenGL drivers |
| Android | HunterIsland-Android-arm64-0.2.0-preview.3.apk | Android 8+, ARM64, OpenGL ES 3.0 |

**Windows:** extract the entire ZIP, open Start.cmd, import your IPA and start
the game. Install Python with Tcl/Tk from
[python.org](https://www.python.org/downloads/windows/).

**Android:** install the APK, open the app, import your IPA and start the game.
Android may ask you to allow APK installation from your download or file app.

The [setup guide](SETUP.md) covers installation, updates, display modes,
save transfer and bug reports. It identifies the current launchers' German
button labels. All three [optional mods](MODS.md) are disabled initially.
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
