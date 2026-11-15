# Validation for 0.2.0-preview.3

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
