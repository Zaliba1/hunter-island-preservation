# Setup and updates

## Your game copy

The verified game version is Hunter Island **2.02**, 32-bit ARM, supplied as
an already decrypted IPA. The importer checks the archive, bundle and executable
and reports incompatible files.

The following steps use the English launcher labels.

## Windows

1. Download the standalone Windows x64 ZIP from Releases and extract it completely.
   A short path such as C:/Games/HunterIsland is suitable.
2. Open **Start Hunter Island.exe** from the extracted
   folder. Keep `_internal`, `launcher` and `runtime` beside it. Python and Tk are included.
3. Select **Import your IPA…** and choose your game file.
4. Choose the display mode, language and optional mods, then select **Play**.
5. Close the game window to return to the launcher.

Playing requires working graphics drivers and the Windows system components.
Rust and compilers are development requirements.
Windows runs the game in an AppContainer without network capabilities.
If the isolation check fails, the launcher displays the error.

Windows data is stored by default in
%LOCALAPPDATA%/HunterIslandPreservation.
This directory contains the imported copy, settings, normal game saves and
local backups. To update, extract the new package into its own directory and
open its launcher. It uses the same local data directory.

## Android

1. Download or transfer the ARM64 APK to your phone.
2. Open the APK. If prompted, allow installation from your file/download app.
3. Open **Hunter Island · touchHLE**.
4. Select **Import your IPA…** and choose your file.
5. Choose the display mode, game language and mod switches, then select
   **Play**.

Updates signed with the same project key can be installed over the existing
app. A personal build uses your own key. Export your save before switching
signatures: Android cannot install an APK with a different signature over the
same existing app.

Android keeps the import and settings in private app storage. Saves are stored
in the app-specific external files directory. Uninstalling the app may remove
this data. **Export save…** writes a ZIP to a location
you select.

## Game language

The Windows **Language** menu defaults to English and also offers Deutsch,
Français, Español, Italiano and 日本語, covering all six languages in the IPA.
The launcher updates immediately. The game uses the same selection on its next
start. **System language** uses the first supported Windows display language,
with English as a fallback. Previously saved choices are kept.
The in-game silver exchange uses the selected game language.

The Android and Mac launchers open in English. Their **Game language** menus
offer the same six languages for the game, with English as the initial choice.
The selected game language also applies to the in-game silver exchange.

## Display modes

Windows offers landscape iPad at 1024 × 768 and iPhone at 480 × 320.
Android also offers experimental iPhone Retina at 960 × 640.
On Android, select **Integer scaling** in the launcher to use it on the next
game launch. The Mac launcher offers **Start in fullscreen** and
**Integer scaling**. The launcher remembers these choices.

While playing on Windows or Mac, press **F11** to switch between a window and
full screen. Press **F10** to switch between fit-to-screen and integer scaling
in full screen. Integer scaling uses whole pixels and may leave wider borders.
The emulator fits the image to the screen if a whole-number scale cannot fit.
The game keeps its original aspect ratio in either mode. In the Windows FSR
release, F9 enters fit-to-screen fullscreen with FSR 1.2 from the window
or integer mode. Further presses cycle to FSR 1.2 with light RCAS sharpening,
bilinear, and back to FSR 1.2. The active filter is displayed briefly.

Desktop players can set startup defaults with a `touchHLE_options.txt` file
in the local `profiles/default/run` directory. Use one line for the game:

```text
pockettrend.hunterisland: --fullscreen --integer-scaling
```

For experimental supersampling, try `--fullscreen --scale-hack=2` on that
line. It renders twice the width and height and uses four times as many pixels
before display scaling. A smaller output can smooth edges when the frame is
downsampled. The tested iPad title screen rendered at 2048 × 1536 with this
option. Full gameplay at this setting has yet to be tested.

### Local HD artwork on Mac and Android

Select **2× internal resolution** in Display mode to double the render dimensions on each axis, for example from 1024 × 768 to 2048 × 1536 on the iPad profile. This switch can be used independently of local HD artwork.

Enable **Local HD artwork (monsters 4×, characters and selected scenes 2×)** in Display mode. A background CPU worker reads the artwork from your imported IPA. A completed texture appears on its next load and remains available after restarting.

Standalone monsters use the compact Real-ESRGAN animevideov3 model at 4×. The tavern, courtroom, stone house and castle battlefield use a 2× blend with 25% model output. Dialog portraits are identified from the local Chats.plist. They, both heroes’ dialog and full-body pictures, and the individual frames of the monster portrait and hero walking sheets use a 2× blend with 12.5% model output. Speaking monsters already covered by the 4× monster profile retain it. The worker preserves source transparency and processes each packed frame in isolation. Text, interface elements and other scenes use their original textures.

Generated files stay in `hd-monsters-animev3-x4-v1` under the emulator data directory. Monster cache keys remain compatible with the original option. New entries include a processing version and a digest of the image and any frame rectangles. Close the game and remove this folder to reclaim space. All processing works offline.

## Saves

The game writes normal checkpoints at its intended save points. Reloading may
therefore resume in a town. Emulator savestates for an ongoing battle are
not implemented.

Android accepts compatible backup ZIPs through **Import save…**. Before
importing, the app backs up the existing save locally. Use
**Export pre-import backup…** to copy that backup out of the app.

The Windows diagnostic export can include saves when you explicitly select
that option; it produces an Android-compatible ZIP. The portable Windows
launcher currently has no button for importing phone saves.

## Bug reports

Include the version, operating system/device, display mode, enabled mods and
exact reproduction steps. Review diagnostic packages before sharing them.
Android exports diagnostics and saves through separate actions.
Windows can include saves only when explicitly selected.
