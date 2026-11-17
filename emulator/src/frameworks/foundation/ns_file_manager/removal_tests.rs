/* SPDX-License-Identifier: MPL-2.0 */
use super::*;

#[test]
fn foundation_removes_a_populated_slot_and_reports_missing_readonly_and_io_errors() {
    let root = std::env::temp_dir().join(format!("hi-file-manager-tests-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(root.join("Documents/saves0/nested")).unwrap();
    std::fs::create_dir_all(root.join("Documents/saves1")).unwrap();
    std::fs::write(root.join("Documents/saves0/nested/sv_1"), b"first").unwrap();
    std::fs::write(root.join("Documents/saves1/sv_1"), b"second").unwrap();
    let mut environment = Environment::new_without_app(
        crate::options::Options {
            headless: true,
            ..Default::default()
        },
        crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
    )
    .unwrap();
    let env = &mut environment;
    *env.fs = crate::fs::Fs::from_test_directory(&root, true);
    let pool: id = msg_class![env;NSAutoreleasePool new];
    let manager: id = msg_class![env;NSFileManager defaultManager];
    let path = ns_string::from_rust_string(env, "/Documents/saves0".into());
    let sentinel = id::from_bits(0x1234);
    let output = env.mem.alloc_and_write(sentinel);
    let ok: bool = msg![env;manager removeItemAtPath:path error:output];
    assert!(ok);
    assert_eq!(env.mem.read(output), sentinel); // NSError** is untouched on success.
    assert!(!root.join("Documents/saves0").exists());
    assert_eq!(
        std::fs::read(root.join("Documents/saves1/sv_1")).unwrap(),
        b"second"
    );
    let ok: bool = msg![env;manager removeItemAtPath:path error:output];
    assert!(!ok);
    let error = env.mem.read(output);
    let code: super::super::NSInteger = msg![env;error code];
    assert_eq!(code, NSFileNoSuchFileError);
    let domain: id = msg![env;error domain];
    assert_eq!(ns_string::to_rust_string(env, domain), NSCocoaErrorDomain);
    let none: MutPtr<id> = Ptr::null();
    let ok: bool = msg![env;manager removeItemAtPath:path error:none];
    assert!(!ok);
    let ok: bool = msg![env;manager removeItemAtPath:nil error:output];
    assert!(!ok);
    release(env, path);
    let path = ns_string::from_rust_string(env, "/Documents/saves1/sv_1".into());
    *env.fs = crate::fs::Fs::from_test_directory(&root, false);
    let ok: bool = msg![env;manager removeItemAtPath:path error:output];
    assert!(!ok);
    let error = env.mem.read(output);
    let code: super::super::NSInteger = msg![env;error code];
    assert_eq!(code, NSFileWriteNoPermissionError);
    *env.fs = crate::fs::Fs::from_test_directory(&root, true);
    std::fs::remove_file(root.join("Documents/saves1/sv_1")).unwrap();
    std::fs::create_dir(root.join("Documents/saves1/sv_1")).unwrap();
    let ok: bool = msg![env;manager removeItemAtPath:path error:output];
    assert!(!ok);
    let error = env.mem.read(output);
    let code: super::super::NSInteger = msg![env;error code];
    assert!(code == NSFileWriteUnknownError || code == NSFileWriteNoPermissionError);
    release(env, path);
    release(env, pool);
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("hi-file-manager-tests-"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn removal_errors_use_write_and_missing_file_codes_without_panicking() {
    assert_eq!(removal_error_code(&FsError::DoesNotExist), 4);
    assert_eq!(removal_error_code(&FsError::ReadonlyParentDir), 513);
    assert_eq!(removal_error_code(&FsError::DirectoryNotEmpty), 512);
    for (kind, expected) in [
        (std::io::ErrorKind::NotFound, 4),
        (std::io::ErrorKind::PermissionDenied, 513),
        (std::io::ErrorKind::Other, 512),
    ] {
        assert_eq!(
            removal_error_code(&FsError::IoError(std::io::Error::new(kind, "fixture"))),
            expected
        );
    }
}
