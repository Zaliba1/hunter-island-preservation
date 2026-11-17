# 0.2.0-preview.9

Add an optional fresh campaign with exactly three independent copies of each
of the 353 regular monster forms: 1,059 monsters at level 99 and S rarity. The
original tutorial starter counts toward its form's three copies. No extra
money, tickets, summons, items, recipes or story progress is granted.

New full testing copies and fresh level-one campaigns also give their initial
monsters S rarity. Existing normal and testing campaigns keep their current
rarities, levels and progress. These are initial grants only: loading does not
refill collections, reset levels or force later captures and eggs to S.

The three collection controls are mutually exclusive in the Windows, Android
and Apple Silicon Mac launchers. Each collection has its own save directory
and persistent local history mode, and can be combined with the three ordinary
gameplay mods. All collection boxes off selects normal saves. See TEST_PROFILE.md.

Android version code is 19 and keeps the project's existing update signature.
Matching sources, dependency sources, licenses and checksums accompany the
packages. No IPA, private saves, game catalogs or signing keys are included.
Windows game checks cover creation, save/load and unchanged other profiles;
Android host/package and native Mac checks are reported separately. The new
collection has not been tested on a physical Android device or in Mac gameplay.

# Previous changes: 0.2.0-preview.8

Fix the crash after confirming **Erase Game?** for an occupied save slot.
The shared Foundation adapter now removes nonempty save directories, including
old checkpoints, and returns an NSError on failures instead of panicking.
Removal is scoped to the selected writable guest subtree; protected entries,
linked directories and unrelated host mappings are rejected. POSIX unlink
retains its existing nonrecursive behavior.

The fix applies to Windows, Android and Apple Silicon Mac. Original game
rules and the optional collection settings are unchanged. No IPA or player
saves are included. See VALIDATION.md and the release's DELETE_SAVE_CHECKS.json
for the actual game checks and remaining platform limits.

# Previous changes: 0.2.0-preview.7

Add an optional fresh campaign with all 353 regular monster forms initially
at level 1. This collection is created only when starting a new in-game slot;
monsters level normally afterwards. It grants no extra money, tickets, items,
recipes, ocarinas or summons. Original tutorial rewards still apply.

Normal campaigns, full level-50 test copies and fresh level-one campaigns use
three separate save directories. The two collection controls are mutually
exclusive; either can be combined with the other three gameplay mods. The local
save history preserves which testing mode was used, including early tutorial
checkpoints that do not yet contain quest flags.

The controls are available in the Windows, Android and Apple Silicon Mac
launchers. See TEST_PROFILE.md for usage, save transfer behavior and metadata
limits. Matching sources, dependency sources and licenses accompany the builds.
Native and game checks are described in VALIDATION.md and release-checks.json.
Actual Hunter Island gameplay on Mac and the physical Android collection flow
remain unverified.

# Previous changes: 0.2.0-preview.6

Add an optional **Create / use separate test save** launcher checkbox on
Windows, Android and Apple Silicon Mac. Create and save a normal game first.
The first test start copies its checkpoints into a separate directory; later
starts resume that testing collection without refilling resources. Unticking
the option returns to normal saves, independently of the other three mods.

The Hunter Island 2.02 collection includes 353 regular monster forms, 360
monsterpedia records, seven summons, all 57 ocarinas and other unique equipment,
58 fusion recipes, 9,999,999 silver, a 9,999,999 gold allowance and 9,999 tickets.
Newly added monsters are level 50. Existing party members, equipped items,
quest progress and random-generator state are retained. Summon bosses use
their original summon system rather than joining the regular monster bench.

The test save has its own profile identifier and a persistent local testing
marker. Artificial grants do not count as captures or currency exchanges.
See TEST_PROFILE.md for paths, save transfer behavior and metadata limits.

Windows gameplay verified creation, party selection, equipment ownership,
spending, save/reload, switching to normal saves with other mods and returning
to the same testing collection. Android is build/package-tested; the new flow
has not been exercised on a physical device. Mac native test evidence is
attached separately; actual Hunter Island Mac gameplay remains unverified.

Android version code is 16 and retains the existing project update signature.
The Mac app is experimental and ad-hoc signed, not notarized. Matching sources,
dependency sources, notices and checksums accompany the packages. No IPA,
game catalogs, player saves or private signing keys are included.

## Previous changes: 0.2.0-preview.5

Fix the crash when opening the gravestone question in Orlen. The original
game's text-entry and reward logic now runs through the emulator's UIKit
implementation, without patching the game or granting a replacement reward.

- Implement CALayer border properties and rectangular border drawing.
- Support editable UITextView responders, desktop/Android keyboard events,
  delegate validation, Return, backspace and pasted text.
- Handle the game's reentrant close callback without recursion or stale view
  access. Optional mods are not required for this fix.
- Publish Windows, Android ARM64 and experimental Apple Silicon Mac packages
  with matching sources, dependency sources, notices and checksums.

On Windows the Orlen dialog, wrong answer, cancellation and `dragon island`
reward path were exercised using an isolated private save copy. The original
game awarded Penguini. Android is build/package-tested; the current APK has
not been tested on a connected physical device. The Mac build runs native
framework, launcher, isolation and graphics tests; actual Hunter Island Mac
gameplay remains unverified. See VALIDATION.md and the release's checks.

Windows users can replace their extracted program directory while keeping
their existing profile. Android version code is 15 and uses the existing
project update signature. The Mac app is ad-hoc signed, not notarized.
No IPA, game assets, player saves or private signing keys are distributed.

## Previous changes: 0.2.0-preview.3

Add a saved **Spiel-Sprache / Game language** dropdown to the Windows and
Android launchers. English is the default, including for older profiles with
no explicit language choice. Available choices are English, Deutsch, Français,
Español, Italiano, 日本語, and the device's preferred system languages.

Changes are saved immediately and apply on the next game start. Users do not
need to enter launch arguments. This setting affects the original game;
launcher and custom mod labels remain in German.

The emulator binaries are unchanged from preview.2. Android's application
version code increases to 13 and retains the project update signature.
See VALIDATION.md for the checks performed for this local build.

## Previous changes: 0.2.0-preview.2

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
