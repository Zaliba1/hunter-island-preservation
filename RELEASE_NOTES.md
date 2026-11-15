# 0.2.0-preview.2

Fix the crash when opening **Rewards / Belohnungen** from the arena overview
in Hunter Island 2.02.

- Add modal view-controller presentation and dismissal with ownership and
  completion callbacks.
- Load directory-based NIB runtime archives and decode their font metadata.
- Add basic legacy table views, row index paths and custom-cell layout.
- Support padded RGB images without alpha and UIKit bitmap image contexts,
  including scale, orientation and nested-context restoration.
- Respect noninteractive decorative NIB views so the reward list can scroll.
- Decode button image/background properties and preserve state ownership.
- Build Android from a tracked, neutral options template, without requiring a
  local runtime settings file.

The three optional mods remain opt-in. Install the APK over the previous
project-signed version, or extract the new Windows package into a separate
program directory. Keep your existing local save/profile directory.

Matching sources, dependency sources, license inventories and SHA-256 checksums
accompany both platforms. See VALIDATION.md for actual checks and remaining
limits, especially the scope of Android device testing.

Known visual limits: the Rewards close control may still display "Button";
table clipping and modal-background composition remain incomplete.
The verified Windows Rewards interaction path completes without a panic.
The current APK is build/package-tested; physical-device execution is unverified.
