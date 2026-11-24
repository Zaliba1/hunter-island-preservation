# Android port

The ARM64 Android app imports the user's own compatible Hunter Island 2.02 IPA.
It uses SDL's Android activity and a native GLES3 context to support guest GLES2.
The default iPhone Retina mode uses 480 × 320 logical points and a 960 × 640
backing surface. The launcher also offers ordinary iPhone and iPad modes.

The Easy view offers resolution and recommended Anime4K and local HD artwork. Open **Advanced** to customize graphics, language, mods and save transfers. The launcher supplies opt-in mod arguments to the same emulator
rules used by Windows. Settings are stored locally and take effect at the next
game launch. Save import/export validates bounded paths, sizes, hashes and
checkpoint-only manifests, with a pre-import backup and interrupted-import
recovery under the import/start lock.

## Build and tests

BUILD.md documents the exact tools, pinned source preparation and fresh Android
build. VALIDATION.md records results for the current release.
Earlier Pixel 7a tests confirmed title, gameplay, sound and save import in
previous versions. The current release must be judged by its own stated
validation scope.

## Runtime limits

Other GPUs, the full story, all combat effects, long sessions and background
resume require further tests. The current Home/background policy exits the
game cleanly. Android cloud backup is disabled. APKs use neither Internet nor
broad shared-storage permission.

Source delivery and library replacement/relinking are described in
ANDROID-SOURCE.md and ANDROID-LICENSES.md.
