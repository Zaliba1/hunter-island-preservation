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
        .replacement((48, 48), &vec![0; input.len()], 192, 0x1908, 0x1401, 4096)
        .is_none());
    assert!(cache.pending.is_empty());
    let mut different = input.to_vec();
    *different.last_mut().unwrap() = 0;
    assert!(cache
        .replacement((48, 48), &different, 192, 0x1908, 0x1401, 4096)
        .is_none());
    assert!(cache.pending.is_empty());
    assert!(cache
        .replacement((48, 48), input, 192, 0x1908, 0x1401, 4096)
        .is_none());
    assert_eq!(cache.pending.len(), 1);
    let deadline = Instant::now() + Duration::from_secs(45);
    let hd = loop {
        if let Some(data) = cache.replacement((48, 48), input, 192, 0x1908, 0x1401, 4096) {
            break data;
        }
        assert!(
            Instant::now() < deadline,
            "Worker did not persist the texture"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(hd.1.len(), 192 * 192 * 4);
    drop(cache);
    let mut reopened = Cache::open(&ipa, dir).unwrap();
    assert_eq!(
        reopened
            .replacement((48, 48), input, 192, 0x1908, 0x1401, 4096)
            .unwrap(),
        hd
    );
    assert!(reopened.pending.is_empty());
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn two_times_art_preserves_coverage_and_isolates_neighboring_frames() {
    use std::sync::Arc;
    let model = model::Model::load();
    let left = [40, 20, 0, 64, 0, 0, 0, 0];
    let right = [0, 255, 0, 255, 255, 255, 0, 255];
    let pixels = [left.as_slice(), right.as_slice()].concat();
    let frames = Artwork::Atlas(Arc::new(vec![[0, 0, 2, 1], [2, 0, 2, 1]]));
    let out = frames.render(&model, &pixels, 4, 1);
    let isolated = Artwork::Portrait.render(&model, &left, 2, 1);
    for row in 0..2 {
        assert_eq!(
            &out[row * 32..row * 32 + 16],
            &isolated[row * 16..row * 16 + 16]
        );
    }
    let alpha: Vec<_> = isolated.chunks_exact(4).take(4).map(|p| p[3]).collect();
    assert_eq!(alpha, [64, 48, 16, 0]);
    assert!(out
        .chunks_exact(4)
        .all(|p| p[..3].iter().all(|c| *c <= p[3])));
}

#[test]
fn recognizes_jpeg_dimensions_before_decoding() {
    let header = [
        255, 216, 255, 224, 0, 4, 0, 0, 255, 192, 0, 8, 8, 3, 0, 4, 112, 3,
    ];
    assert_eq!(artwork::dimensions(&header), Some((1136, 768)));
    assert_eq!(artwork::dimensions(&header[..16]), None);
    assert_eq!(artwork::dimensions(&[255, 216, 255, 224, 0, 1]), None);
}

#[test]
fn indexes_local_dialogs_and_heroes_with_monster_precedence() {
    let root = std::env::temp_dir().join(format!("hi-dialog-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let ipa = root.join("synthetic.ipa");
    let mut zip = zip::ZipWriter::new(File::create(&ipa).unwrap());
    let mut add = |name: &str, bytes: &[u8]| {
        zip.start_file(name, zip::write::FileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    };
    add(
        "Payload/F.app/Chats.plist",
        br#"<?xml version="1.0"?>
    <plist version="1.0"><dict>
      <key>Single</key><dict><key>images</key><array><string>Solo.png</string></array></dict>
      <key>Chapter</key><dict><key>Dialog</key><dict><key>images</key><array>
      <dict><key>image</key><string>Npc.png</string></dict><string>Creature01.png</string>
      <string>empty.png</string><string>../Ignore.png</string><string>$$AssistantImage$$</string>
      </array></dict></dict></dict></plist>"#,
    );
    let input = include_bytes!("synthetic-edge.png");
    for name in [
        "Solo",
        "Npc",
        "Creature01",
        "male_01",
        "female_01",
        "malefull_01",
        "femalefull_01",
        "Ignore",
        "empty",
        "Unknown",
    ] {
        add(&format!("Payload/F.app/{name}-hd.png"), input);
    }
    add("Payload/Other.app/Npc-hd.png", input);
    zip.finish().unwrap();
    let mut cache = Cache::open(&ipa, root.join("cache")).unwrap();
    let mut chosen = std::collections::BTreeMap::new();
    for sprite in &cache.sprites[&(48, 48)] {
        let entry = cache.archive.by_index(sprite.index).unwrap();
        assert!(entry.name().starts_with("Payload/F.app/"));
        chosen.insert(
            entry.name().split('/').last().unwrap().to_owned(),
            sprite.artwork.scale(),
        );
    }
    assert_eq!(chosen.len(), 7);
    assert_eq!(chosen["Creature01-hd.png"], 4);
    for name in [
        "Solo",
        "Npc",
        "male_01",
        "female_01",
        "malefull_01",
        "femalefull_01",
    ] {
        assert_eq!(chosen[&format!("{name}-hd.png")], 2);
    }
    drop(cache);
    fs::remove_dir_all(root).unwrap();
}
