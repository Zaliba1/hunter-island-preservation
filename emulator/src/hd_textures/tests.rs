use super::*;
use std::time::{Duration, Instant};

#[test]
fn generates_once_and_reloads_from_a_local_ipa_cache() {
    let root = std::env::temp_dir().join(format!("hi-hd-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let ipa = root.join("synthetic.ipa");
    let mut archive = zip::ZipWriter::new(File::create(&ipa).unwrap());
    archive
        .start_file(
            "Payload/Fixture.app/Synthetic01-hd.png",
            zip::write::FileOptions::default(),
        )
        .unwrap();
    let source = include_bytes!("synthetic-edge.png");
    archive.write_all(source).unwrap();
    archive.finish().unwrap();
    let dir = root.join("cache");
    let mut cache = Cache::open(&ipa, dir.clone()).unwrap();
    let image = Image::from_bytes(source).unwrap();
    let input = image.pixels();
    assert!(cache
        .replacement((48, 48), &vec![0; input.len()], 192, 0x1908, 0x1401)
        .is_none());
    assert!(cache.pending.is_empty());
    let mut different = input.to_vec();
    *different.last_mut().unwrap() = 0;
    assert!(cache
        .replacement((48, 48), &different, 192, 0x1908, 0x1401)
        .is_none());
    assert!(cache.pending.is_empty());
    assert!(cache
        .replacement((48, 48), input, 192, 0x1908, 0x1401)
        .is_none());
    assert_eq!(cache.pending.len(), 1);
    let deadline = Instant::now() + Duration::from_secs(45);
    let hd = loop {
        if let Some(data) = cache.replacement((48, 48), input, 192, 0x1908, 0x1401) {
            break data;
        }
        assert!(
            Instant::now() < deadline,
            "Worker did not persist the texture"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(hd.len(), 192 * 192 * 4);
    drop(cache);
    let mut reopened = Cache::open(&ipa, dir).unwrap();
    assert_eq!(
        reopened
            .replacement((48, 48), input, 192, 0x1908, 0x1401)
            .unwrap(),
        hd
    );
    assert!(reopened.pending.is_empty());
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
