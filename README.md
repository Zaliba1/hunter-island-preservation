# Hunter Island Preservation

An unofficial project for preserving Hunter Island 2.02 offline on Windows,
Android, and Apple Silicon Mac, based on [touchHLE v0.3.0](https://github.com/touchHLE/touchHLE).
Import your own compatible, already decrypted 32-bit IPA locally.

The shared source tree also contains an experimental native **Apple Silicon Mac**
app for macOS 13 or later. See [MACOS.md](MACOS.md) for test-build downloads,
installation, data locations and the remaining gameplay checks.

## Current release

**0.2.0-preview.10** combines the optional **Zalibas Expansion** campaign and
graphics filters on Windows x64, Android arm64 and Apple Silicon Mac. Import
your own original Hunter Island 2.02 IPA. Enable the campaign in the launcher
to enter its separate saves. The expansion uses the selected game language.
On desktop, press F9 to cycle the output filters. On Android choose the output
filter in the launcher. Local HD artwork and 2× internal resolution can be
selected separately. See [release notes](RELEASE_NOTES.md).

## Previous releases

**0.2.0-preview.9-fsr.1** adds optional FSR 1.2 upscaling to the Windows build. In-game,
**F9** enters fit-to-screen fullscreen and selects FSR. Press it again to
compare FSR with light RCAS sharpening and bilinear filtering. The active
filter appears briefly in the game.

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

Use the current downloads below. The [Windows FSR release](https://github.com/Zaliba1/hunter-island-preservation/releases/tag/v0.2.0-preview.9-fsr.1)
includes its source archives, checksums and test results.

| Platform | Download | Requirements |
| --- | --- | --- |
| Windows PC | [Standalone Windows ZIP](https://github.com/Zaliba1/hunter-island-preservation/releases/download/v0.2.0-preview.9-fsr.1/HunterIsland-Windows-x64-0.2.0-preview.9-fsr.1-standalone.zip) | Windows 10/11 x64 with OpenGL drivers. Python and Tk are bundled. |
| Android | [Android ARM64 release APK](https://github.com/Zaliba1/hunter-island-preservation/releases/download/v0.2.0-preview.9-packaging.1/HunterIsland-Android-arm64-0.2.0-preview.9-release.apk) | Android 8+, ARM64, OpenGL ES 3.0 |
| Apple Silicon Mac | [Apple Silicon Mac ZIP](https://github.com/Zaliba1/hunter-island-preservation/releases/download/v0.2.0-preview.9-display.1/HunterIsland-macOS-arm64-test-a6495e5.zip) | macOS 13+, ARM64. Experimental, ad-hoc signed. |

### Windows

Extract the entire standalone ZIP, open **Start Hunter Island.exe**, select
**Import your IPA…**, then select **Play**.
New profiles start in English. Changing **Language** updates the launcher
immediately and applies to the game when you next start it.

### Android

Install the APK, open the app, import your IPA and start the game.
Android may ask you to allow APK installation from your download or file app.

### Mac

Extract the ZIP, move **Hunter Island.app** to Applications, and open it.
Select **Import your IPA…**, then select **Play**. See [MACOS.md](MACOS.md).

The [setup guide](SETUP.md) covers installation, updates, display modes,
save transfer and bug reports. The launchers remember your choices.
The [optional mods and testing collections](MODS.md) are disabled initially.

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
