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

AMD FidelityFX Super Resolution 1.2 EASU and RCAS in the desktop presenter
derive from AMD's [FidelityFX SDK v1.1.4 shader reference](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/blob/v1.1.4/sdk/include/FidelityFX/gpu/fsr1/ffx_fsr1.h).
The GLSL adaptation is distributed under the MIT license in
`licenses/AMD-FSR-MIT.txt`.

Original attribution and license texts remain in licenses/, emulator/ and the
matching SourceDependencies asset. See LICENSES.md and ANDROID-LICENSES.md.
General-purpose SDK/compiler/Gradle tools are separately installed build tools.
