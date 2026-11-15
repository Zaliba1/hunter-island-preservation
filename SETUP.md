# Setup and updates

## Your game copy

The verified game version is Hunter Island **2.02**, 32-bit ARM, supplied as
an already decrypted IPA. The importer checks the archive, bundle and executable
and reports incompatible files.

Project documentation is in English. The current launchers use German button
labels; the labels below are quoted so you can find the corresponding actions.

## Windows

1. Download the Windows x64 ZIP from Releases and extract it completely.
   A short path such as C:/Games/HunterIsland is suitable.
2. Install 64-bit Python 3.11 or later with Tcl/Tk using the official
   [Windows installer](https://www.python.org/downloads/windows/).
3. Open Start.cmd and select **Eigene IPA importieren** (import your IPA).
4. Choose the display mode, game language and optional mods, then select
   **Spiel starten** (start the game).
5. Close the game window to return to the launcher.

Playing requires Python, graphics drivers and Windows components.
Rust and compilers are development requirements.
Windows runs the game in an AppContainer without network capabilities.
If the isolation check fails, the launcher displays the error.

Windows data is stored by default in
%LOCALAPPDATA%/HunterIslandPreservation.
This directory contains the imported copy, settings, normal game saves and
local backups. To update, extract the new package into its own directory and
use its Start.cmd. It uses the same local data directory.

## Android

1. Download or transfer the ARM64 APK to your phone.
2. Open the APK. If prompted, allow installation from your file/download app.
3. Open **Hunter Island · touchHLE**.
4. Select **Eigene IPA importieren** (import your IPA) and choose your file.
5. Choose the display mode, game language and mod switches, then select
   **Spiel starten** (start the game).

Updates signed with the same project key can be installed over the existing
app. A personal build uses your own key. Export your save before switching
signatures: Android cannot install an APK with a different signature over the
same existing app.

Android keeps the import and settings in private app storage. Saves are stored
in the app-specific external files directory. Uninstalling the app may remove
this data. **Spielstand exportieren** (export save) writes a ZIP to a location
you select.

## Game language

Both launchers offer **Spiel-Sprache / Game language**. English is the default,
including for existing profiles that have never selected a game language.
Choose English, Deutsch, Français, Español, Italiano, 日本語, or
**Systemsprache / System language** to follow the device's preferred languages.
The selection is saved immediately and takes effect the next time you start
the game. No command-line arguments are needed.

This setting controls the original game's translations. The launcher interface
and the added mod dialogs currently retain their German labels.

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
