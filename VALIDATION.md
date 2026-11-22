# Validation for 0.2.0-preview.9-fsr.1

Windows development checks covered the F9/F10/F11 transitions through SDL
and all three F9 selections in the packaged game. A Radeon RX 7800 XT rendered
FSR 1.2 and FSR 1.2 with RCAS at 1920 × 1440. Fresh isolated playthroughs
reached the Fang tutorial with both bilinear and FSR + RCAS and created a
checkpoint. See release-checks.json for the final package validation.

## Previous validation: 0.2.0-preview.9

Verified using private, isolated copies. No game data or player saves are
included in the source or runtime packages.

- Windows native library: 184 tests passed, 2 pre-existing ignored. Launcher:
  38 tests passed, including actual Tk controls, mutual exclusion across all
  three collection modes, profile routing and normal-save isolation.
- Fresh triple level-99 campaign: exactly 353 forms with three copies each,
  1,059 monsters at level 99 and S rarity. The original tutorial starter is
  included in that total. Starting balances remain 1,000 silver and 250 gold;
  no summons or other bonus resources are granted.
- Saving, restarting, loading and writing another checkpoint preserve the
  collection without granting extra copies or changing its levels. Normal,
  full and level-one profile files retain their original hashes.
- New full collections: all 353 regular forms are S; copied party levels stay
  unchanged. New level-one campaigns: all 353 granted forms plus the original
  starter are level 1 and S. Older full and level-one campaigns resume without
  changing their existing rarity or level values.
- Separate sticky history modes roundtrip, including the new triple-level-99
  identifier. Artificial grants do not increment capture or exchange counters.
- Android: 54 Robolectric/JUnit tests passed, covering launcher controls and
  separate exports for the fourth save area. Version code 19 keeps the existing
  project signer. Static signature, permissions, ARM64/16-KiB alignment,
  assets, notices and known-private-marker checks are performed on the APK.

Final source correspondence, fresh-source rebuild evidence, exact Windows ZIP
execution and package hashes accompany the release as release-checks.json and
TEST_PROFILE_CHECKS.json. Native Mac evidence is in macos-checks.json. Actual
Hunter Island gameplay on Mac and the new collection flow on a physical Android
device remain unverified. Bit-identical reproducibility is not claimed.

## Previous validation: 0.2.0-preview.8

Verified using separate private copies; original player data is excluded.

- Reproduced the game crash after selecting a populated slot's trash icon and
  confirming **Erase Game?**. The original build panicked in NSFileManager's
  unimplemented error handling for a nonempty directory. Its test files remained
  unchanged after the failed operation.
- The corrected Windows build returned safely to the title screen after
  confirmation and removed the selected slot's checkpoint directory. A second
  occupied slot and both other save collections retained identical file hashes.
  Cancelling the confirmation retained identical hashes for every save file.
- Windows native library: 182 tests passed, 2 pre-existing ignored. New tests
  exercise populated/nested directories, hidden files, reuse of a removed path,
  protected descendants, unrelated host mappings and host I/O failures. The
  guest NSFileManager API returns ordinary NSError objects for failures.
- Windows launcher/package: 37 tests passed. Android framework: 50 tests passed
  on API 26 and 35; the signed update uses version code 18.

The operation follows Foundation's documented recursive removal behavior:
[removeItem(atPath:)](https://developer.apple.com/documentation/foundation/filemanager/removeitem(atpath:)).
POSIX unlink still uses the existing nonrecursive primitive. Linked directories
remain unsupported and are rejected before traversing the tree.

Final package hashes, game-test details and source correspondence accompany the
release as release-checks.json and DELETE_SAVE_CHECKS.json. Native Mac tests and
package evidence are supplied as macos-checks.json. Actual Hunter Island gameplay
on Mac and this new deletion flow on a physical Android device remain unverified.
Fresh-source builds do not establish bit-identical reproducibility.

## Previous validation: 0.2.0-preview.7

Verified on 17 November 2026 using private, isolated QA profiles.

- Windows native library: 177 tests passed, 2 pre-existing ignored.
- Windows launcher/package: 37 tests passed, including real Tk mutual exclusion
  and all valid combinations with the other gameplay mods.
- Android: 50 Robolectric/JUnit tests passed on API 26 and 35, including
  profile selection, persistence, fresh-start preflight and exports of each
  independent collection. The signed update uses version code 17.
- A fresh Windows profile with no normal seed reached the original tutorial,
  starter selection, open world and first capture. Its first world save had
  353 granted regular forms plus the original tutorial starter, all at level 1,
  and 353 monsterpedia records. No items, equipment, recipes or summons were
  granted. Original starting resources were 1,000 silver, 250 gold and 400 tickets;
  subsequent original tutorial rewards still apply.
- The first tutorial checkpoint retained the level-one history marker, including
  checkpoints without a quest-flags dictionary. All three other gameplay mods
  were enabled; artificial grants did not increment their action counters.

Exact package hashes, source correspondence, save/reload checks, fresh-source
Android rebuild checks and native Mac results accompany the release. See
release-checks.json and macos-checks.json for the completed checks and limits.
Actual Hunter Island Mac gameplay and physical Android testing of this new
collection remain unverified. Bit-identical rebuilds are not claimed.

## Previous validation: 0.2.0-preview.6

Verified on 17 November 2026 using separate private QA copies.

- Windows emulator library tests: 174 passed, 2 pre-existing ignored.
- Windows launcher/package tests: 36 passed, including the actual Tk test-save
  checkbox, option persistence and independent normal/test profile paths.
- Android Robolectric/JUnit tests on API 26 and 35: 46 passed, including
  independent option combinations, selected-profile exports, baseline-save
  preflight and recovery/import behavior.
- Windows game test: 353 regular forms, 360 monsterpedia entries, seven summons,
  57 ocarinas, 58 named unique equipment items and 58 recipes survive a real
  game save. A level-50 added monster was selected into the line-up. Shop
  ownership labels and both 9,999,999 currency balances were checked.
- Test gold spent on an egg remains spent after restarting; the collection is
  not recreated or refilled. The original normal files stayed unchanged while
  testing. With only the three other mods enabled, normal saves loaded with
  their original collection and balances, leaving the test directory unchanged.
- The testing marker and distinct profile identifier survive game save/reload;
  artificial additions do not increment capture or exchange counters.
- The exact local Windows ZIP was extracted, manifest-checked and run to its
  populated test save in a zero-capability AppContainer. APK signature, native
  architecture/alignment, notices and private-data checks passed.

Release checks and engine/APK hashes accompany the final packages. Mac CI
performs native framework, Swift core, launcher layout, isolation and graphics
tests; its actual results are in macos-checks.json. Actual Hunter Island Mac
gameplay and the physical Android testing-collection flow remain unverified.
Bit-identical reproducibility and full-story/all-device coverage are not claimed.

## Previous validation: 0.2.0-preview.5

Verified on 17 November 2026 on the Windows build PC. Detailed private game
captures and saves are excluded from source and release packages.

- Baseline crash reproduced at the Orlen gravestone: CALayer did not respond
  to `setBorderColor:`. Static inspection also identified missing editable
  UITextView keyboard/delegate support behind that first failure.
- Windows native framework tests: 171 passed, 2 pre-existing ignored.
  New tests cover multi-character input, UTF-16 delegate ranges, surrogate-pair
  backspace, vetoes, responder switching and a delegate that resigns again and
  releases its view while Return is being handled.
- Windows launcher/package tests: 34 passed.
- Real-game Windows Orlen test: dialog opens and renders; an incorrect answer
  produces "Nothing happened"; tapping outside closes safely; `dragon island`
  follows the original reward path to Penguini. A saved private test copy
  contains one Mafiapenguin01. All optional mods were disabled.

The exact distributed Windows ZIP is extracted and run before publication;
its engine digest and result are recorded in release-checks.json. Android
unit-test/package evidence accompanies the APK. Mac native test evidence is
in macos-checks.json. Neither a physical Android Orlen run nor actual Mac
Hunter Island gameplay is claimed. Full text selection, marked-text/IME
composition and rounded CALayer border paths remain incomplete.

## Previous validation: 0.2.0-preview.3

Verified on 15 November 2026 on the Windows build PC.

- Windows launcher/package tests: 31 passed. These include the actual Tk
  language dropdown, immediate persistence without starting the game,
  restoration after reopening, and forwarding every supported language to
  the game launch. System-language mode omits the language override.
- Android Robolectric/JUnit tests: 42 passed on API 26 and 35, including
  language selection, activity recreation, persistence, English defaults
  for existing installs and fallback for unsupported stored values.
- Android Java application rebuilt and signed with the existing project key;
  application version code 13. Native libraries are copied byte for byte
  from the verified preview.2 APK. The Windows engine is also unchanged.
- Windows startup with English as the unset-profile default and an explicit
  German selection: both reached the title screen in separate AppContainers
  with zero capabilities. Logs confirm the game requested and received the
  selected language. Both runs were deliberately ended after 20 seconds;
  exit 124 denotes that test timeout, not a game panic.

No original game content or user saves are included in the packages. The
language setting changes the original game, not the German launcher/mod labels.
Physical-device installation and gameplay of this new APK have not been
tested. All six translations have not been reviewed throughout the full game.

The engine checks below apply to the unchanged preview.2 engine binaries.

## Previous validation: 0.2.0-preview.2

Verified on 15 November 2026 on the Windows build PC.

| Check | Result |
| --- | --- |
| Matching source copied into a separate clean checkout | Passed |
| Pinned source/dependency preparation | 23 archives, 148 Rust packages, 9,974 vendored Rust files verified |
| Windows GNU build in a new output directory | Passed |
| Engine library tests | 151 passed, 2 existing tests ignored |
| Windows launcher/package unit tests | 28 passed |
| Android ARM64 build in a new output directory | Passed |
| Android Robolectric/JUnit tests on API 26 and 35 | 36 passed |
| APK signature, ARM64, 16-KiB alignment, permissions and source notices | Passed |
| APK signer compared with preview.1 | Same project signer |
| Arena Rewards opening, scrolling to lower classes, closing and reopening | Passed on Windows |
| Arena test AppContainer / normal exit / original save seed | Zero capabilities / exit 0 / seed unchanged |

The game test used a separate local save copy. Private game files, captures and
detailed execution logs are excluded from the source and release packages.
The tested binary hashes are in validation.json. Final archive extraction/start
evidence and downloadable asset checksums accompany the release in
release-checks.json and SHA256SUMS.txt.

The clean Android build exposed a compile-time dependency on an ignored local
options file. It now uses a tracked, neutral template containing no user
settings. Neither build requires an IPA, save or private runtime settings.

The framework tests cover modal ownership, appearance callbacks and completion
blocks; table rows, index paths and reloads; NIB archive selection and numeric
font values; image alpha formats; scaled/nested image contexts; passive view hit
testing; and button image ownership.

## Limits

The new APK has not been executed or installed on a physical Android device in
this review. Same-signer comparison establishes the intended update-signing
relationship, rather than a completed device update test.

Native UI rendering remains incomplete. The Rewards close control can still
display "Button", table clipping is incomplete, and the underlying arena can
show through the modal background. The verified interaction path completes
despite these cosmetic issues. KNOWN_ISSUES.md lists further limits.

The full story, all combat effects, all devices, long sessions and a second
Windows PC remain unverified. Historical Apple guest binaries have not been
independently rebuilt byte for byte; see LICENSE_REVIEW.md.

preview.1 retains its own binaries and checks. Its source archive has received
the original comment-only Android build template as a corresponding-source
correction; the binary bytes are unchanged.
