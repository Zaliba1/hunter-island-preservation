/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::path::Path;

fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}

fn main() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = package_root.join("../../..");

    let mut wrapper = cc::Build::new();
    if std::env::var("TARGET").as_deref() == Ok("x86_64-pc-windows-gnu") {
        wrapper.cpp_link_stdlib(None);
    }
    wrapper
        .file(package_root.join("lib.cpp"))
        .cpp(true)
        .compile("pvrt_decompress_wrapper");
    rerun_if_changed(&package_root.join("lib.cpp"));
    rerun_if_changed(&workspace_root.join("vendor/PVRTDecompress/PVRTDecompress.cpp"));
    rerun_if_changed(&workspace_root.join("vendor/PVRTDecompress/PVRTDecompress.h"));
}
