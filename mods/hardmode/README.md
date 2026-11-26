# Hunter Island Hardmode V2

`v2-entry.mjs` adapts a live Hunter Island 2.02 encounter to the trained
team battle policy from the Hunter Island browser simulator. The game still
validates and queues each proposed action. The Windows, macOS and Android
launchers use `--hunter-hardmode-v2` for scripted encounters and
`--hunter-additional-arena-fights` for the separate V3 arena matches.

The checked-in `emulator/src/hunter_ai_model.js` is the build input for all three
targets. Run `node mods/hardmode/bundle-v2.mjs mods/hardmode/browser-engine` from
the emulator repository to regenerate it from the included source modules.
It obtains the combat catalog locally from the installed 2.02 IPA at runtime.

The QuickJS core is vendored under `emulator/vendor/hunter-quickjs` from
[QuickJS 2025-09-13-2](https://bellard.org/quickjs/quickjs-2025-09-13-2.tar.xz).
The original archive SHA-256 is
`996c6b5018fc955ad4d06426d0e9cb713685a00c825aa5c0418bd53f7df8b0b4`.
