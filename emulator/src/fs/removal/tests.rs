/* SPDX-License-Identifier: MPL-2.0 */
use super::*;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("hi-delete-tests-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        for (name, data) in [
            ("Documents/saves0/sv_1", b"old".as_slice()),
            ("Documents/saves0/sub/nested", b"nested"),
            ("Documents/saves0/.hidden", b"hidden"),
            ("Documents/saves1/sv_2", b"neighbor"),
        ] {
            let file = path.join(name);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, data).unwrap();
        }
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(self
            .0
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("hi-delete-tests-"));
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn populated_slot_is_removed_without_touching_neighbors_and_can_be_recreated() {
    let fixture = Fixture::new();
    let mut fs = Fs::from_test_directory(&fixture.0, true);
    let slot = GuestPath::new("/Documents/saves0");
    assert!(matches!(fs.remove(slot), Err(FsError::DirectoryNotEmpty)));
    fs.remove_recursively(GuestPath::new("Documents/saves0/"))
        .unwrap();
    assert!(!fs.exists(slot) && !fixture.0.join("Documents/saves0").exists());
    assert_eq!(fs.read("/Documents/saves1/sv_2").unwrap(), b"neighbor");
    assert_eq!(
        std::fs::read(fixture.0.join("Documents/saves1/sv_2")).unwrap(),
        b"neighbor"
    );
    fs.create_dir_all(slot).unwrap();
    fs.write(slot.join("sv_1"), b"new").unwrap();
    assert_eq!(fs.read(slot.join("sv_1")).unwrap(), b"new");
    fs.remove_recursively(&slot.join("sv_1")).unwrap();
    fs.remove_recursively(slot).unwrap();
    assert!(matches!(
        fs.remove_recursively(slot),
        Err(FsError::DoesNotExist)
    ));
}

#[test]
fn readonly_descendant_and_other_mount_are_rejected_before_deleting_anything() {
    let fixture = Fixture::new();
    let mut fs = Fs::from_test_directory(&fixture.0, true);
    let (parent, name) = fs
        .lookup_parent_node(GuestPath::new("/Documents/saves0/sv_1"))
        .unwrap();
    let FsNode::Directory { children, .. } = parent else {
        panic!()
    };
    let FsNode::File { writeable, .. } = children.get_mut(&name).unwrap() else {
        panic!()
    };
    *writeable = false;
    assert!(matches!(
        fs.remove_recursively(GuestPath::new("/Documents/saves0")),
        Err(FsError::AccessDenied)
    ));
    assert_eq!(
        std::fs::read(fixture.0.join("Documents/saves0/sub/nested")).unwrap(),
        b"nested"
    );
    let mut fs = Fs::from_test_directory(&fixture.0, true);
    let (parent, name) = fs
        .lookup_parent_node(GuestPath::new("/Documents/saves0/sub"))
        .unwrap();
    let FsNode::Directory { children, .. } = parent else {
        panic!()
    };
    let FsNode::Directory { writeable, .. } = children.get_mut(&name).unwrap() else {
        panic!()
    };
    *writeable = Some(fixture.0.join("Documents/saves1"));
    assert!(matches!(
        fs.remove_recursively(GuestPath::new("/Documents/saves0")),
        Err(FsError::AccessDenied)
    ));
    assert!(fixture.0.join("Documents/saves0/sv_1").is_file());
    assert_eq!(
        std::fs::read(fixture.0.join("Documents/saves1/sv_2")).unwrap(),
        b"neighbor"
    );
    let mut fs = Fs::from_test_directory(&fixture.0, false);
    assert!(matches!(
        fs.remove_recursively(GuestPath::new("/Documents/saves0")),
        Err(FsError::ReadonlyParentDir)
    ));
    assert!(fs.remove_recursively(GuestPath::new("/")).is_err());
}

#[test]
fn host_io_failure_returns_an_error_and_keeps_the_virtual_entry() {
    let fixture = Fixture::new();
    let mut fs = Fs::from_test_directory(&fixture.0, true);
    let file = fixture.0.join("Documents/saves0/sv_1");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(matches!(
        fs.remove_recursively(GuestPath::new("/Documents/saves0/sv_1")),
        Err(FsError::IoError(_))
    ));
    assert!(fs.exists(GuestPath::new("/Documents/saves0/sv_1")));
    std::fs::remove_dir(&file).unwrap();
    std::fs::write(&file, b"recovered").unwrap();
    fs.remove_recursively(GuestPath::new("/Documents/saves0/sv_1"))
        .unwrap();
    assert!(!file.exists() && !fs.exists(GuestPath::new("/Documents/saves0/sv_1")));
}

#[cfg(unix)]
#[test]
fn host_directory_links_are_not_followed() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(
        fixture.0.join("Documents/saves1"),
        fixture.0.join("Documents/saves0/link"),
    )
    .unwrap();
    let mut fs = Fs::from_test_directory(&fixture.0, true);
    assert!(matches!(
        fs.remove_recursively(GuestPath::new("/Documents/saves0")),
        Err(FsError::AccessDenied)
    ));
    assert_eq!(
        std::fs::read(fixture.0.join("Documents/saves1/sv_2")).unwrap(),
        b"neighbor"
    );
    assert!(fixture.0.join("Documents/saves0/sv_1").is_file());
}
