# Apple Silicon Mac preview

The Apple Silicon preview is included in the shared source tree on `main`.
Building a new preview does not replace older downloaded application packages.

The target is an ARM64 **Hunter Island.app** for macOS 13 or later. Intel Macs
are not included in this preview. Players do not need Python, Rust, Homebrew,
or a Windows `.cmd` file. Developers need the tools described below.

## Using a successful test build

1. Open [Releases](https://github.com/Zaliba1/hunter-island-preservation/releases)
   and download `HunterIsland-macOS-arm64-test-….zip` from the desired release.
   A public release download does not require a GitHub account.
2. Extract the downloaded ZIP.
   Move **Hunter Island.app** to a local folder such as Applications.
3. Open the app and select **Import your IPA…**. Supply your own already
   decrypted Hunter Island 2.02 IPA. Other games and encrypted executables
   are rejected. The app contains no game files or player saves.
4. Choose **Game language**, **Display mode**, and any optional mod checkboxes.
   English is the default, and mods start off. The silver exchange, when
   enabled, uses the selected game language.
5. Select **Play**. Close the game window to return to the launcher.

The launcher remembers **Start in fullscreen** and **Integer scaling**.
During play, **F11** switches between a window and full screen. **F10** changes
the fullscreen scaling mode. Integer scaling keeps pixels sharp and can leave
larger borders, depending on the display and game resolution.

The test build is an experimental preview, **ad-hoc signed, not notarized**.
The starter does not disable Gatekeeper or remove quarantine flags.
Follow macOS's own review/approval flow for software you have chosen to trust.

Data is stored separately from the app in
`~/Library/Application Support/HunterIslandPreservation/`. The launcher opens
this folder with **Open saves, backups and local logs**. Import/settings persist
when replacing the app. Normal game saves are under
`profiles/default/run/touchHLE_sandbox/pockettrend.hunterisland/`.
Backups are made before and after a game run; the latest 20 are retained.
Automatic discovery/import of Windows or Android saves is not performed.
There is not yet a Mac save-import button or a Mac diagnostic-export button.

## Isolation

The preview uses the system `sandbox-exec` with the supplied Seatbelt profile.
Before each game launch a packaged probe must establish that a canary under
the user's real home cannot be read, an outside write is denied, a write in
the run directory succeeds, and an IPv4 loopback connection is denied. If the
probe fails, the launcher does not start the game without isolation.

The profile denies network access, denies reads below the real home directory
except the app/profile, and restricts writes to the run directory. It permits
other system access needed by graphics/audio. This is a different boundary
from Windows AppContainer and does not claim a universal filesystem jail.
`sandbox-exec` is a legacy system interface; future macOS changes may require
a replacement. The launcher itself needs access to user-selected import files.

## Building and source delivery

Use the source ZIP matching the app's commit. Its source dependencies
are included as `HunterIsland-SourceDependencies-test-<commit>.zip` in the same
artifact. This includes the added SHA-2 source, unlike the older preview.2
dependency archive alone. Retain both source archives when sharing this build.
`SHA256SUMS.txt` and `checks.json` record the exact dependency archive digest.
Per-file license terms remain applicable; see LICENSES.md and LICENSE_REVIEW.md.

On an Apple Silicon development Mac, install Xcode Command Line Tools, Rust
1.90.0 and CMake 3.31.6. Python 3.11+ is used for build/package checks only.
Prepare sources in a fresh checkout, then run:

```sh
python3 tools/prepare_offline_sources.py /path/to/HunterIsland-SourceDependencies-test-COMMIT.zip
bash tools/build-macos.sh
```

The build runs engine tests, synthetic IPA-import/core tests, the native
launcher UI smoke test, a packaged isolation probe, and the existing real
shader/render/readback probe. It creates the app, matching source ZIP,
compiled-file inventory and checksums under `dist/macos/`.
The graphics probe uses the packaged, signed engine through the same isolated
process setup and private environment as the Play button, without an IPA.

## Validation limits

A successful CI build and graphics probe do **not** establish that the complete
game works. No original IPA is uploaded to CI. The actual game title screen,
sound, mouse input, Retina presentation, save roundtrip and extended play must
still be tested on a user's Mac. The app's OpenGL ES 1.00 shaders are translated
to desktop GLSL 1.20 for Apple's compatibility renderer; unknown shader
extensions continue to report errors rather than being silently discarded.
The compatibility backend uses Apple's vertex-array entry points. GitHub's
Mac runner currently exposes Apple's software renderer; passing its graphics
probe does not establish performance or compatibility on a user's GPU.

There are no Orbling/Tiamat experiment switches in this build. Such local
experiments are separate from the Mac port.
