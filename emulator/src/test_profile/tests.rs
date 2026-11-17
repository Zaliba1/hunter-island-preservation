/* SPDX-License-Identifier: MPL-2.0 */
use super::*;

fn properties(monster: &mut Value) -> &mut Dictionary {
    let custom = nested(monster, "customSaveData").unwrap();
    let entity = nested(custom, "entitySaveData").unwrap();
    dict(nested(entity, "properties").unwrap()).unwrap()
}

fn save() -> Value {
    let mut d = Dictionary::new();
    let mut player = Dictionary::new();
    let mut props = Dictionary::new();
    let mut owned = object("Normal.Owned", "Monsters.plist", true, LEVEL);
    properties(&mut owned).insert("tier".into(), 0.into());
    player.insert("savedMonsters".into(), Value::Array(vec![owned]));
    player.insert("savedItems".into(), Value::Array(vec![]));
    player.insert(
        "savedGemsInLicense".into(),
        Value::Array(vec![object(
            "Gems.Uniques.EquippedOcarina",
            "Items.plist",
            false,
            LEVEL,
        )]),
    );
    player.insert("savedLicenseName".into(), "Novice".into());
    props.insert("lastTown".into(), "unchanged-town".into());
    d.insert("playerData".into(), player.into());
    d.insert("props".into(), props.into());
    d.insert("goldSpent".into(), 7.into());
    d.insert("goldPurchased".into(), 0.into());
    d.insert("adw".into(), 30.into());
    d.insert("flags".into(), Dictionary::new().into());
    d.insert("poiData".into(), Dictionary::new().into());
    d.insert(
        "seeders".into(),
        Value::Array(vec![Value::Dictionary(Dictionary::new())]),
    );
    d.into()
}
fn catalog() -> Catalog {
    Catalog {
        monsters: vec![
            ("Normal.Owned".into(), "Owned".into()),
            ("Dragon.Test".into(), "Test".into()),
        ],
        summons: vec!["TestSummon".into()],
        items: vec![
            "Gems.Uniques.EquippedOcarina".into(),
            "Gems.Uniques.TestOcarina".into(),
        ],
        recipes: vec!["TestRecipe".into()],
    }
}
#[test]
fn profiles_are_exclusive_but_independent_from_the_other_mods() {
    assert_eq!(Mode::from_flags(false, false, false).unwrap(), None);
    assert_eq!(
        Mode::from_flags(true, false, false).unwrap(),
        Some(Mode::Full)
    );
    assert_eq!(
        Mode::from_flags(false, true, false).unwrap(),
        Some(Mode::LevelOne)
    );
    assert!(Mode::from_flags(true, true, false).is_err());
    assert_eq!(
        Mode::from_flags(false, false, true).unwrap(),
        Some(Mode::Triple99)
    );
    assert!(Mode::from_flags(true, false, true).is_err());
    assert!(Mode::from_flags(false, true, true).is_err());
    assert!(Mode::from_flags(true, true, true).is_err());
    assert_ne!(Mode::Triple99.folder(), Mode::LevelOne.folder());
    assert_ne!(Mode::Triple99.folder(), Mode::Full.folder());
    assert_eq!(
        (
            Mode::Triple99.level(),
            Mode::Triple99.copies(),
            Mode::Triple99.bit()
        ),
        (99, 3, 4)
    );
    assert_ne!(Mode::Full.folder(), Mode::LevelOne.folder());
    let unit = object("Dragon.Test", "Monsters.plist", true, 1);
    assert_eq!(
        unit.as_dictionary().unwrap()["creationArguments"]
            .as_dictionary()
            .unwrap()["jobArguments"]
            .as_dictionary()
            .unwrap()["level"]
            .as_unsigned_integer(),
        Some(1)
    );
}
#[test]
fn test_collection_is_idempotent_and_preserves_party_story_and_equipment() {
    let mut data = save();
    // An ordinary seed unit starts below S. Only the new copy is promoted;
    // its original level, equipment and party position remain intact.
    let original = data.clone();
    let mut promoted = original.as_dictionary().unwrap()["playerData"]
        .as_dictionary()
        .unwrap()["savedMonsters"]
        .as_array()
        .unwrap()[0]
        .clone();
    properties(&mut promoted).insert("tier".into(), MONSTER_TIER.into());
    populate(&mut data, &catalog(), Mode::Full).unwrap();
    let d = data.as_dictionary().unwrap();
    let p = d["playerData"].as_dictionary().unwrap();
    let old = original.as_dictionary().unwrap();
    assert_eq!(d["flags"], old["flags"]);
    assert_eq!(d["poiData"], old["poiData"]);
    assert_eq!(d["seeders"], old["seeders"]);
    assert_eq!(
        d["props"].as_dictionary().unwrap()["lastTown"],
        "unchanged-town".into()
    );
    assert_eq!(p["savedMonsters"].as_array().unwrap()[0], promoted);
    assert_eq!(
        p["savedGemsInLicense"],
        old["playerData"].as_dictionary().unwrap()["savedGemsInLicense"]
    );
    assert_eq!(p["savedMonsters"].as_array().unwrap().len(), 2);
    for monster in p["savedMonsters"].as_array().unwrap() {
        let mut monster = monster.clone();
        assert_eq!(
            properties(&mut monster)["tier"].as_signed_integer(),
            Some(5)
        );
    }
    assert_eq!(p["savedItems"].as_array().unwrap().len(), 1);
    assert_eq!(
        d["goldFound"].as_unsigned_integer(),
        Some(MONEY as u64 * 30 + 7)
    );
    let r = Record::decode(d[preservation::FIELD].as_data().unwrap()).unwrap();
    assert!(r.test_profile);
    assert_eq!(r.counts, [0; 9]);
    assert_eq!(r.game_hash, preservation::game_hash(&data).unwrap());
    assert_eq!(r.report()["status"], "test_profile");
    let before = data.clone();
    populate(&mut data, &catalog(), Mode::Full).unwrap();
    assert_eq!(before, data);
}
#[test]
fn only_monster_grants_receive_s_rarity_at_the_requested_starting_level() {
    for level in [1, 50, 99] {
        let mut monster = object("Dragon.Test", "Monsters.plist", true, level);
        assert_eq!(
            properties(&mut monster)["tier"].as_signed_integer(),
            Some(5)
        );
        assert_eq!(
            properties(&mut monster)["JobLevelProp"].as_real(),
            Some(level as f64)
        );
        let mut item = object("Gems.Uniques.Test", "Items.plist", false, level);
        assert!(!properties(&mut item).contains_key("tier"));
    }
}
#[test]
fn catalog_keeps_bonus_monsters_but_routes_summons_separately() {
    let mut mons = Dictionary::new();
    let mut group = Dictionary::new();
    for (name, category, bonus) in [
        ("Ordinary", "Normal", false),
        ("Bonus", "Egg", true),
        ("Summon", "Summon", false),
    ] {
        let mut d = Dictionary::new();
        d.insert("index".into(), 1.into());
        d.insert("category".into(), category.into());
        d.insert("bonusMonster".into(), bonus.into());
        group.insert(name.into(), d.into());
    }
    mons.insert("Group".into(), group.into());
    let mut items = Dictionary::new();
    let mut gems = Dictionary::new();
    gems.insert("Uniques".into(), Dictionary::new().into());
    items.insert("Gems".into(), gems.into());
    let c = Catalog::parse(&mons.into(), &items.into(), &Dictionary::new().into()).unwrap();
    assert_eq!(c.monsters.len(), 2);
    assert_eq!(c.summons, ["Summon"]);
}
#[test]
fn copy_failure_does_not_change_original_and_rejects_linked_children() {
    let root =
        std::env::temp_dir().join(format!("hi-test-profile-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir(&root).unwrap();
    let original = root.join("normal");
    let copy = root.join("copy");
    std::fs::create_dir(&original).unwrap();
    std::fs::write(original.join("sv_3"), b"original").unwrap();
    copy_tree(&original, &copy, &mut 0, &mut 0).unwrap();
    std::fs::write(copy.join("sv_3"), b"testing").unwrap();
    assert_eq!(std::fs::read(original.join("sv_3")).unwrap(), b"original");
    assert!(copy_tree(&original, &root.join("limited"), &mut 10000, &mut 0).is_err());
    assert!(number("sv_../3", "sv_").is_none());
    assert!(number("saves0-extra", "saves").is_none());
    assert_eq!(number("sv_123", "sv_"), Some(123));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original.join("sv_3"), original.join("linked")).unwrap();
        assert!(copy_tree(&original, &root.join("linked-copy"), &mut 0, &mut 0).is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}
