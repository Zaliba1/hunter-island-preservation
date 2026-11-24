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

The desktop filter test includes two unmodified GLSL shader files under `emulator/src/gles/filters/`:
- [Anime4K CNN x2 S](https://github.com/bloc97/Anime4K/blob/7684e9586f8dcc738af08a1cdceb024cc184f426/glsl/Upscale/Anime4K_Upscale_CNN_x2_S.glsl), copyright bloc97, MIT. The full notice is in the shader.
- [FSRCNNX x2 8-0-4-1, release 1.1](https://github.com/igv/FSRCNN-TensorFlow/releases/tag/1.1), copyright igv, LGPL-3.0-or-later. See `licenses/LGPL-3.0.txt` and `licenses/GPL-3.0.txt`. Rebuilding the emulator includes changes to these shader sources.

Original attribution and license texts remain in licenses/, emulator/ and the
matching SourceDependencies asset. See LICENSES.md and ANDROID-LICENSES.md.
General-purpose SDK/compiler/Gradle tools are separately installed build tools.
