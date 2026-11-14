# Third-party attribution

The combined runtime inventories accompany the release.

- Windows license-inventory.json maps every shipped executable, guest library
  and font to component, hash and imported system libraries.
- Android android-license-inventory.json maps DEX, native/guest libraries and
  fonts to source, notice files and hashes.
- android-rust-dependencies.json maps Rust dependencies to license notices.
- guest-library-provenance.json pins the original upstream guest libraries.
- toolchain-provenance.json and android-build-provenance.json identify the
  compiler environments.

Original attribution and license texts remain in licenses/, emulator/ and the
matching SourceDependencies asset. See LICENSES.md and ANDROID-LICENSES.md.
General-purpose SDK/compiler/Gradle tools are separately installed build tools.
