# License scope and source delivery

The combined emulator executable is distributed under **GPL-3.0-or-later**, as specified by upstream. Its project/core source files remain **MPL-2.0**, and third-party files retain their original licenses. This preview combines separately licensed components. The root LICENSE is **MPL-2.0** for project code; it does not replace third-party licenses. Original notices remain in their files and in `licenses/`. Modification, rebuilding, and debugging modifications to the open-source components are permitted, including replacing and relinking OpenAL Soft.

| Component | Terms | Source / notices |
| --- | --- | --- |
| Launcher, touchHLE core and preservation changes | MPL-2.0 | Matching Source ZIP; per-file notices, root and emulator LICENSE |
| Static OpenAL Soft | GNU Library GPL v2 or later (LGPL-2.0-or-later) | SourceDependencies `native/openal-soft.zip`; licenses/OpenAL-Soft-LGPL-2.0.txt; offline rebuild/relink instructions in SOURCE.md |
| Dynarmic | 0BSD; externals retain MIT/BSD/Boost terms | Complete native source including external subtrees; licenses/dynarmic_license.txt |
| SDL2 / rust-sdl2 | zlib / MIT as marked | Native source and Cargo-vendored Git source, original notices |
| Boost 1.81.0 | BSL-1.0 | Full original Boost archive; licenses/Boost.txt |
| stb / PVRTDecompress | Upstream MIT/public-domain or marked terms | Complete native or emulator vendor source; original license files |
| Rust crates | Per-package terms | Complete Cargo-vendored sources/checksums; licenses/rust_dependencies.txt; original notices |
| libgcc_s.1.dylib | GPL v2 or later with historical runtime exception | Complete Apple gcc-5664 source; original GPL and exception headers |
| libstdc++.6.0.9.dylib | GPL v2 or later with historical linking exception | Complete Apple libstdcxx-39 source; original GPL and exception headers |
| Guest zlib / SQLite / libxml2 | zlib / public domain / MIT-style | Original notices beside dylibs, versioned guest source archives |
| Liberation / Noto fonts | SIL Open Font License as supplied | Unmodified fonts and notices in touchHLE_fonts |
| GNU C++/GCC runtime code | GPL-3.0-or-later with GCC Runtime Library Exception 3.1, as marked in runtime sources | Original COPYING3 and COPYING.RUNTIME in licenses/gnu-runtime; GCC 15.2.0 sources supplied |
| MinGW-w64 / Winpthreads runtime code | Original ZPL, MIT, BSD/public-domain terms as marked | Complete supplied copyright/license/disclaimer files in licenses/gnu-runtime; MinGW-w64 13.0.0 sources supplied |
| Python/Tk and Windows/.NET | Separately installed prerequisites | No interpreter, PyInstaller bootloader, VC runtime DLL, OS DLL or .NET Framework is redistributed |

`license-inventory.json` maps every shipped executable, DLL, PYD, guest dylib and font to a component and hash. Packaging rejects unknown compiled components and excludes build-machine `api-ms-win-*.dll` and `ucrtbase.dll`; Windows 10/11 supplies those OS components.

## Copyleft source delivery

Offer the matching Source and SourceDependencies assets beside the Windows asset on the same release page. Full OpenAL source and the work using it are supplied as source, with offline build and clean/relink commands. This uses accompanying source rather than relying on a future written offer. Keep all three assets accessible together.

Guest FSF libraries are byte-identical to the pinned public touchHLE tree, as recorded in guest-library-provenance.json. The source mapping follows its README. Complete published Apple source releases, original acknowledgements and historical exception headers are supplied. Apple's historical SDK binaries have not been independently bit-reproduced. The original projects' build files are in those archives; rebuilding requires the appropriate platform/compiler environment. No proprietary Apple SDK is distributed. Guest libraries are separate files and do not license or supply the game.

## Build and prerequisite boundary

This release uses GCC/MinGW-w64 and the GNU Windows Rust toolchain. No MSVC-built emulator or copied Microsoft VC runtime DLL is shipped. GCC's original runtime exception permits eligible combinations with independent modules; the source components retain their own licenses. The GNU runtime and MinGW notices are preserved.

The user installs official Python/Tk separately. Windows 10/11 supplies OS libraries and .NET Framework, including the C# compiler used locally for the tiny isolation probe. Those products are subject to their own terms and are not included in the download. Supplementary Python/PyInstaller/Microsoft reference notices or public source archives retained from the earlier packaging investigation do not indicate that those binaries are shipped. Earlier MSVC candidates are not the release approved by this checklist.

## Source and executable licenses

Upstream explicitly distinguishes MPL-2.0 core source from GPL-3.0-or-later combined binaries. The original GPL notice in the emulator is preserved. GPL-3.0.txt accompanies the executable. Full matching sources, dependency sources and rebuild/relink instructions are delivered beside it. Original source-file notices and dependency exceptions remain in force. The launcher source is MPL-2.0 and is supplied as source, separately from the emulator executable.

No IPA, original game data, private saves or private analysis material is supplied. Rights in a game copy remain separate.

Rust notice supplement: Rust 1.90.0 standard-library copyright/license notices are included at licenses/rust-standard-library/COPYRIGHT-library.html. Full Apache-2.0 texts and retained generator/Khronos/Rust-SDL notices are explicitly associated with their package directories in licenses/rust/. The emulator core and launcher behavior are unchanged from test5.

## Android runtime

The ARM64 APK adds dynamic SDL2 under zlib and LLVM libc++_shared under
Apache-2.0 with LLVM exception, with the original Android NDK notices.
Robolectric/JUnit are host-only tests. ANDROID-LICENSES.md contains the
Android-specific inventory scope. Both platforms share the matching
Source and SourceDependencies assets described in SOURCE.md.
