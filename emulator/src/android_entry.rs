/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Decode the arguments supplied by SDL's Android JNI entry point.
use std::ffi::{c_char, c_int, CStr};

/// # Safety
/// For nonzero argc, argv must point to argc readable pointers, each pointing
/// to a readable NUL-terminated C string, as guaranteed by SDL nativeRunMain.
pub unsafe fn arguments(argc: c_int, argv: *const *const c_char) -> Result<Vec<String>, String> {
    if !(0..=256).contains(&argc) {
        return Err("Invalid Android argument count".into());
    }
    if argc == 0 {
        return Ok(vec![String::new()]);
    }
    if argv.is_null() {
        return Err("Missing Android argument array".into());
    }
    let pointers = unsafe { std::slice::from_raw_parts(argv, argc as usize) };
    pointers.iter().map(|&pointer| {
        if pointer.is_null() {
            return Err("Missing Android argument".into());
        }
        unsafe { CStr::from_ptr(pointer) }.to_str()
            .map(str::to_owned).map_err(|_| "Android argument is not UTF-8".into())
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    #[test]
    fn android_game_path_and_options_reach_the_core() {
        let values = ["app_process", "/data/user/0/example/files/Spiel ü.ipa", "--experimental-gles2", "--device-family=ipad"];
        let strings: Vec<_> = values.iter().map(|v| CString::new(*v).unwrap()).collect();
        let pointers: Vec<_> = strings.iter().map(|v| v.as_ptr()).collect();
        assert_eq!(unsafe { arguments(pointers.len() as i32, pointers.as_ptr()) }.unwrap(), values);
    }
    #[test]
    fn android_empty_argument_list_keeps_default_picker_semantics() {
        assert_eq!(unsafe { arguments(0, std::ptr::null()) }.unwrap(), [""]);
    }
    #[test]
    fn android_invalid_argument_metadata_is_rejected() {
        assert!(unsafe { arguments(-1, std::ptr::null()) }.is_err());
        assert!(unsafe { arguments(257, std::ptr::null()) }.is_err());
        assert!(unsafe { arguments(1, std::ptr::null()) }.is_err());
        let pointers = [std::ptr::null()];
        assert!(unsafe { arguments(1, pointers.as_ptr()) }.is_err());
        let invalid = [0xffu8, 0];let pointers = [invalid.as_ptr().cast()];
        assert!(unsafe { arguments(1, pointers.as_ptr()) }.is_err());
    }
}
