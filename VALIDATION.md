# Validation for 0.2.0-preview.1

Verified on 14 November 2026 on the Windows build PC.

| Check | Result |
| --- | --- |
| Sources and dependencies prepared in a fresh copy | Passed |
| Windows GNU release build in a fresh output directory | Passed |
| Engine library tests | 141 passed, 2 existing tests ignored |
| Windows launcher and package checks | 28 passed |
| Fresh Android ARM64 release build | Passed |
| Android Robolectric/JUnit tests on API 26 and 35 | 36 passed |
| APK signature, ARM64 and 16-KiB alignment | Passed |
| Windows AppContainer and title screen from ZIP | Passed |

Windows was started with a separate test profile and a locally supplied IPA.
The title screen was visible at 1024 × 768. The bounded 25-second test was
deliberately stopped by its time limit.
Private test profiles and images are outside this repository.

23 source archives and 9,974 files from 148 Rust packages were checked against
the supplied checksums. Matching sources and controlling build scripts are
included in the source assets.

Files in emulator/src and the Android app sources match the fresh build inputs.
Current runtime hashes are recorded in validation.json and the release
inventories.

## Limits

The new APK was not executed on a phone during this review. The Pixel was not
available through ADB. Earlier Pixel tests apply to their respective APKs.

The full story, all combat effects, all GPUs and long sessions remain unverified.
Historical Apple guest libraries have not been independently rebuilt byte for
byte. LICENSE_REVIEW.md describes this provenance limit.
Successful builds verify the documented rebuild route; they do not establish
cross-platform byte-identical output.
