# Setup and updates

## Your game copy

The verified game version is Hunter Island **2.02**, 32-bit ARM, supplied as
an already decrypted IPA. The importer checks the archive, bundle and executable
and reports incompatible files.

The following steps use the English launcher labels.

## Windows

1. Download the Windows x64 ZIP from Releases and extract it completely.
   A short path such as C:/Games/HunterIsland is suitable.
2. If your ZIP contains **Start Hunter Island.exe**, open it from the extracted
   folder. Keep `_internal`, `launcher` and `runtime` beside it. Python and Tk are included.
   For a ZIP with only **Start.cmd**, install 64-bit Python 3.11 or later with
   Tcl/Tk using the [official installer](https://www.python.org/downloads/windows/),
   then open **Start.cmd**.
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
The game uses a fixed aspect ratio, so wider phones have unused space at
the sides.

## Saves

The game writes normal checkpoints at its intended save points. Reloading may
therefore resume in a town. Emulator savestates for an ongoing battle are
not implemented.

Android accepts compatible backup ZIPs through **Spielstand importieren**
(import save). Before importing, the app backs up the existing save locally.
Use **Sicherung vor Import exportieren** (export pre-import backup) to copy
that backup out of the app.

The Windows diagnostic export can include saves when you explicitly select
that option; it produces an Android-compatible ZIP. The portable Windows
launcher currently has no button for importing phone saves.

## Bug reports

Include the version, operating system/device, display mode, enabled mods and
exact reproduction steps. Review diagnostic packages before sharing them.
Android exports diagnostics and saves through separate actions.
Windows can include saves only when explicitly selected.
