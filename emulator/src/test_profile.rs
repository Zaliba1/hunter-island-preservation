/* SPDX-License-Identifier: MPL-2.0 */
//! Opt-in, separate testing collection. Catalogs come from the player's IPA.
use crate::libc::crypto::{decode_local_save, transform_local};
use crate::preservation::{self, Record};
use plist::{Dictionary, Value};
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

const BUNDLE: &str = "pockettrend.hunterisland";
pub(crate) const PROFILE: &str = "pockettrend.hunterisland-test-v1";
pub(crate) const LEVEL_ONE_PROFILE: &str = "pockettrend.hunterisland-level1-v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode { Full, LevelOne }
impl Mode {
    pub(crate) fn from_flags(full: bool, level_one: bool) -> Result<Option<Self>, String> {
        match (full,level_one) {
            (true,true) => Err("Choose only one testing collection. / Bitte nur eine Testsammlung wählen.".into()),
            (true,false) => Ok(Some(Self::Full)),
            (false,true) => Ok(Some(Self::LevelOne)),
            (false,false) => Ok(None),
        }
    }
    fn folder(self) -> &'static str { match self {Self::Full=>PROFILE,Self::LevelOne=>LEVEL_ONE_PROFILE} }
    fn key(self) -> &'static str { match self {Self::Full=>"full",Self::LevelOne=>"level1"} }
    fn level(self) -> u32 { match self {Self::Full=>LEVEL,Self::LevelOne=>1} }
    fn bit(self) -> u32 { match self {Self::Full=>1,Self::LevelOne=>2} }
}
const MARKER: &str = "test-profile.json";
const LEVEL: u32 = 50;
const MONEY: u32 = 9_999_999;
const LIMIT: u64 = 64 * 1024 * 1024;

fn dict(value: &mut Value) -> Result<&mut Dictionary, String> {
    value
        .as_dictionary_mut()
        .ok_or_else(|| "Invalid game dictionary".into())
}
fn nested<'a>(value: &'a mut Value, key: &str) -> Result<&'a mut Value, String> {
    dict(value)?
        .get_mut(key)
        .ok_or_else(|| format!("Missing game field: {key}"))
}
fn ensure_dictionary<'a>(value: &'a mut Dictionary, key: &str) -> Result<&'a mut Dictionary, String> {
    if !value.contains_key(key) { value.insert(key.into(),Dictionary::new().into()); }
    dict(value.get_mut(key).unwrap())
}
fn object(name: &str, file: &str, monster: bool, level: u32) -> Value {
    let mut properties = Dictionary::new();
    let mut custom = Dictionary::new();
    let mut arguments = Dictionary::new();
    if monster {
        properties.insert("JobLevelProp".into(), (level as f64).into());
        properties.insert("tier".into(), 0.into());
        let mut job = Dictionary::new();
        job.insert("level".into(), level.into());
        arguments.insert("jobArguments".into(), job.into());
        custom.insert("monsterSaveData".into(), Dictionary::new().into());
    }
    let mut entity = Dictionary::new();
    entity.insert("properties".into(), properties.into());
    custom.insert("entitySaveData".into(), entity.into());
    let mut d = Dictionary::new();
    d.insert("name".into(), name.into());
    d.insert("creationFile".into(), file.into());
    d.insert("creationArguments".into(), arguments.into());
    d.insert("additionalChildren".into(), Value::Array(vec![]));
    d.insert("customSaveData".into(), custom.into());
    d.into()
}

struct Catalog {
    monsters: Vec<(String, String)>,
    summons: Vec<String>,
    items: Vec<String>,
    recipes: Vec<String>,
}
impl Catalog {
    fn parse(monsters: &Value, items: &Value, recipes: &Value) -> Result<Self, String> {
        let mut result = Self {
            monsters: vec![],
            summons: vec![],
            items: vec![],
            recipes: vec![],
        };
        let mut names = BTreeSet::new();
        for (group, children) in monsters.as_dictionary().ok_or("Invalid monster catalog")? {
            if group == "RootMonster" {
                continue;
            }
            for (short, blueprint) in children.as_dictionary().ok_or("Invalid monster category")? {
                let b = blueprint
                    .as_dictionary()
                    .ok_or("Invalid monster definition")?;
                if !b.contains_key("index") {
                    continue;
                }
                if !names.insert(short.clone()) {
                    return Err("Duplicate monster short name".into());
                }
                match b.get("category").and_then(Value::as_string) {
                    Some("Summon") => result.summons.push(short.clone()),
                    Some("Normal" | "Combine" | "Egg") => result
                        .monsters
                        .push((format!("{group}.{short}"), short.clone())),
                    _ => return Err(format!("Unsupported monster category: {group}.{short}")),
                }
            }
        }
        let uniques = items
            .as_dictionary()
            .and_then(|d| d.get("Gems"))
            .and_then(Value::as_dictionary)
            .and_then(|d| d.get("Uniques"))
            .and_then(Value::as_dictionary)
            .ok_or("Missing unique item catalog")?;
        for (name, item) in uniques {
            if item
                .as_dictionary()
                .and_then(|d| d.get("class"))
                .and_then(Value::as_string)
                == Some("Gem")
            {
                result.items.push(format!("Gems.Uniques.{name}"));
            }
        }
        result.recipes = recipes
            .as_dictionary()
            .ok_or("Invalid recipe catalog")?
            .keys()
            .cloned()
            .collect();
        if result.monsters.is_empty()
            || result.monsters.len() > 4096
            || result.summons.is_empty()
            || result.summons.len() > 64
            || result.items.len() > 2048
            || result.recipes.len() > 2048
        {
            return Err("Unexpected testing catalog size".into());
        }
        result.monsters.sort();
        result.summons.sort();
        result.items.sort();
        result.recipes.sort();
        Ok(result)
    }
    fn load(ipa: &Path, key: &[u8; 32]) -> Result<Self, String> {
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(ipa).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let prefix = archive
            .file_names()
            .find(|n| {
                n.starts_with("Payload/")
                    && n.ends_with(".app/Info.plist")
                    && n.split('/').count() == 3
            })
            .ok_or("Missing game bundle")?
            .trim_end_matches("Info.plist")
            .to_owned();
        let mut asset_key = [0; 16];
        for (i, byte) in asset_key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(
                std::str::from_utf8(&key[i * 2..i * 2 + 2]).map_err(|_| "Invalid data key")?,
                16,
            )
            .map_err(|_| "Invalid data key")?;
        }
        let mut read = |name: &str| -> Result<Value, String> {
            let mut data = Vec::new();
            archive
                .by_name(&(prefix.clone() + name))
                .map_err(|e| e.to_string())?
                .take(LIMIT + 1)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            if data.len() as u64 > LIMIT {
                return Err("Oversized game catalog".into());
            }
            let plain = transform_local(1, 1, &asset_key, &[0; 16], &data)
                .map_err(|_| "Cannot decode game catalog")?;
            let mut decoded = Vec::new();
            flate2::read::GzDecoder::new(&plain[..])
                .take(LIMIT + 1)
                .read_to_end(&mut decoded)
                .map_err(|e| e.to_string())?;
            if decoded.len() as u64 > LIMIT {
                return Err("Oversized decoded catalog".into());
            }
            Value::from_reader(Cursor::new(decoded)).map_err(|e| e.to_string())
        };
        Self::parse(
            &read("Monsters.plist")?,
            &read("Items.plist")?,
            &read("Combines.plist")?,
        )
    }
}

fn populate(save: &mut Value, catalog: &Catalog, mode: Mode) -> Result<(), String> {
    // Preserve the original party/order, individual monsters and story state.
    let player = nested(save, "playerData")?;
    let owned = nested(player, "savedMonsters")?
        .as_array_mut()
        .ok_or("Invalid monster collection")?;
    let mut present: BTreeSet<_> = owned
        .iter()
        .filter_map(|v| {
            v.as_dictionary()?
                .get("name")?
                .as_string()
                .map(str::to_owned)
        })
        .collect();
    for (full, _) in &catalog.monsters {
        if present.insert(full.clone()) {
            owned.push(object(full, "Monsters.plist", true, mode.level()));
        }
    }
    if mode == Mode::Full {
    let equipped = dict(player)?
        .get("savedGemsInLicense")
        .and_then(Value::as_array)
        .ok_or("Invalid equipment")?;
    let mut items: BTreeSet<_> = equipped
        .iter()
        .filter_map(|v| {
            v.as_dictionary()?
                .get("name")?
                .as_string()
                .map(str::to_owned)
        })
        .collect();
    let inventory = nested(player, "savedItems")?
        .as_array_mut()
        .ok_or("Invalid item inventory")?;
    items.extend(inventory.iter().filter_map(|v| {
        v.as_dictionary()?
            .get("name")?
            .as_string()
            .map(str::to_owned)
    }));
    for name in &catalog.items {
        if items.insert(name.clone()) {
            inventory.push(object(name, "Items.plist", false, mode.level()));
        }
    }
    }
    let props = dict(nested(save, "props")?)?;
    let highest = ensure_dictionary(props,"highestLevelMonsters")?;
    for short in catalog
        .monsters
        .iter()
        .map(|(_, s)| s)
        .chain(catalog.summons.iter().filter(|_| mode == Mode::Full))
    {
        let level = highest
            .get(short)
            .and_then(Value::as_unsigned_integer)
            .unwrap_or(0)
            .max(mode.level() as u64);
        highest.insert(short.clone(), level.into());
    }
    if mode == Mode::Full {
    let spirits = ensure_dictionary(props,"spiritsData")?;
    for short in &catalog.summons {
        let spirit = ensure_dictionary(spirits,short)?;
        // Unlock at the game's own baseline summon level; do not inject bosses
        // into the regular bench or invent capture/evolution records for them.
        if !spirit.contains_key("xp") {
            spirit.insert("xp".into(), 1.into());
        }
    }
    let recipes = ensure_dictionary(props,"combineRecipes")?;
    for name in &catalog.recipes {
        recipes.insert(name.clone(), true.into());
    }
    let d = dict(save)?;
    d.insert("silver".into(), MONEY.into());
    let spent = d
        .get("goldSpent")
        .and_then(Value::as_unsigned_integer)
        .ok_or("Invalid gold ledger")?;
    let scale=d.get("adw").and_then(Value::as_unsigned_integer).filter(|n|*n>0).ok_or("Invalid gold multiplier")?;
    let allowance=(MONEY as u64).checked_mul(scale).ok_or("Gold multiplier overflow")?;
    let found = spent
        .checked_add(allowance)
        .filter(|n| *n < i32::MAX as u64)
        .ok_or("Gold ledger exceeds test range")?;
    d.insert("goldFound".into(), found.into());
    for name in ["tickets"] {
        d.insert(name.into(), 9_999u32.into());
    }
    }
    let d = dict(save)?;
    let previous = d
        .get(preservation::FIELD)
        .and_then(Value::as_data)
        .and_then(|b| Record::decode(b).ok());
    let mut record = previous.unwrap_or_else(|| Record::new(*uuid::Uuid::new_v4().as_bytes(), 0));
    if !record.test_profile || record.test_profile_modes & mode.bit() == 0 { record.id = *uuid::Uuid::new_v4().as_bytes(); }
    record.test_profile = true;
    record.test_profile_modes |= mode.bit();
    record.game_hash = preservation::game_hash(save)?;
    dict(save)?.insert(preservation::FIELD.into(), Value::Data(record.encode()?));
    Ok(())
}

fn plain_file(path: &Path) -> Result<std::fs::Metadata, String> {
    let m = std::fs::symlink_metadata(path).map_err(|e| format!("Cannot inspect test/save entry {}: {e}",path.display()))?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err("Linked test/save paths are unsupported".into());
        }
    }
    if m.file_type().is_symlink() {
        return Err("Linked test/save paths are unsupported".into());
    }
    Ok(m)
}
fn copy_tree(
    source: &Path,
    target: &Path,
    count: &mut usize,
    total: &mut u64,
) -> Result<(), String> {
    let m = plain_file(source)?;
    *count += 1;
    *total += m.len();
    if *count > 10000 || *total > 512 * 1024 * 1024 {
        return Err("Save copy exceeds test-profile limits".into());
    }
    if m.is_dir() {
        std::fs::create_dir(target).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_tree(&entry.path(), &target.join(entry.file_name()), count, total)?;
        }
    } else if m.is_file() {
        // Copy data, not Windows security descriptors into the AppContainer.
        let mut input=std::fs::File::open(source).map_err(|e|format!("Cannot read testing seed: {e}"))?;
        let mut output=std::fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(|e|format!("Cannot create testing seed copy: {e}"))?;
        std::io::copy(&mut input,&mut output).map_err(|e|format!("Cannot copy testing seed: {e}"))?;
    } else {
        return Err("Unsupported save entry".into());
    }
    Ok(())
}
fn number(name: &str, prefix: &str) -> Option<u64> {
    let s = name.strip_prefix(prefix)?;
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

pub(crate) fn prepare(ipa: &Path, base: &Path, mode: Mode) -> Result<PathBuf, String> {
    let key = preservation::save_key(ipa).map_err(|e|format!("Test-profile IPA inspection failed: {e}"))?;
    log!("Test profile: IPA metadata and data key checked.");
    let sandbox = base.join(crate::paths::SANDBOX_DIR);
    std::fs::create_dir_all(&sandbox).map_err(|e|e.to_string())?;
    plain_file(&sandbox)?;
    let source = sandbox.join(BUNDLE);
    let target = sandbox.join(mode.folder());
    if target.exists() {
        plain_file(&target)?;
        let marker = target.join(MARKER);
        if plain_file(&marker)?.len() > 16384 {
            return Err("Oversized test profile marker".into());
        }
        let data = std::fs::read(&marker).map_err(|_| {
            "Incomplete test profile; keep it for inspection and use a fresh test directory"
        })?;
        let marker: serde_json::Value =
            serde_json::from_slice(&data).map_err(|_| "Invalid test profile marker")?;
        if marker["schema"] != 1 || marker["test_profile"] != true {
            return Err("Unrecognized test profile".into());
        }
        if marker.get("mode").and_then(serde_json::Value::as_str).unwrap_or("full") != mode.key() {
            return Err("This directory belongs to a different testing mode".into());
        }
        // Validate all children before handing the directory to the guest FS.
        validate_tree(&target, &mut 0, &mut 0)?;
        log!("Using existing separate testing collection.");
        return Ok(target);
    }
    if mode == Mode::LevelOne {
        // Empty sandbox: the game itself creates each new slot. Grant its
        // collection through the verified new-player hook, never from a copy
        // of an existing campaign and never during load/save operations.
        let staging = sandbox.join(format!(".test-profile-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir(&staging).map_err(|e|e.to_string())?;
        let result = (|| -> Result<(),String> {
            for dir in ["Documents","Library","tmp"] {
                std::fs::create_dir(staging.join(dir)).map_err(|e|e.to_string())?;
            }
            let marker=serde_json::json!({"schema":1,"mode":mode.key(),"test_profile":true,"fresh_game":true});
            std::fs::write(staging.join(MARKER),serde_json::to_vec_pretty(&marker).unwrap()).map_err(|e|e.to_string())?;
            std::fs::rename(&staging,&target).map_err(|e|e.to_string())
        })();
        if result.is_err() { let _=std::fs::remove_dir_all(&staging); }
        result?;
        return Ok(target);
    }
    let documents = source.join("Documents");
    if !documents.is_dir() {
        return Err("First create and save a normal game, then enable the separate test save. / Bitte zuerst ein normales Spiel speichern.".into());
    }
    plain_file(&source)?;
    plain_file(&documents)?;
    let catalog = Catalog::load(ipa, &key).map_err(|e|format!("Test profile catalog: {e}"))?;
    log!("Test profile: catalogs checked; creating separate save copy.");
    let staging = sandbox.join(format!(".test-profile-{}", uuid::Uuid::new_v4().simple()));
    let create = || -> Result<(), String> {
        copy_tree(&source, &staging, &mut 0, &mut 0)?;
        log!("Test profile: normal save copied; expanding copied checkpoints.");
        let docs = staging.join("Documents");
        let mut slots = 0;
        for entry in std::fs::read_dir(&docs).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if number(&entry.file_name().to_string_lossy(), "saves").is_none() {
                continue;
            }
            let dir = entry.path();
            if !plain_file(&dir)?.is_dir() {
                return Err("Invalid save slot directory".into());
            }
            let mut files = Vec::new();
            for file in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let file = file.map_err(|e| e.to_string())?;
                if let Some(n) = number(&file.file_name().to_string_lossy(), "sv_") {
                    files.push((n, file.path()));
                }
            }
            files.sort_by_key(|(n, _)| *n);
            let Some((_, latest)) = files.last() else {
                continue;
            };
            if plain_file(latest)?.len() > LIMIT {
                return Err("Oversized save".into());
            }
            let data = std::fs::read(latest).map_err(|e| e.to_string())?;
            let data = decode_local_save(&key, &data)?;
            let mut value = Value::from_reader(Cursor::new(data)).map_err(|e| e.to_string())?;
            populate(&mut value, &catalog, mode)?;
            let mut plain = Vec::new();
            value
                .to_writer_binary(&mut plain)
                .map_err(|e| e.to_string())?;
            let encoded = transform_local(0, 3, &key, &[0; 16], &plain)
                .map_err(|_| "Cannot encode testing save")?;
            std::fs::write(latest, &encoded).map_err(|e| e.to_string())?;
            // Retain only the transformed checkpoint inside this new copy.
            for (_, old) in &files {
                if old != latest {
                    std::fs::remove_file(old).map_err(|e| e.to_string())?;
                }
            }
            slots += 1;
        }
        if slots == 0 {
            return Err("First create and save a normal game, then enable the separate test save. / Bitte zuerst ein normales Spiel speichern.".into());
        }
        let full=mode == Mode::Full;
        let marker = serde_json::json!({"schema":1,"mode":mode.key(),"test_profile":true,"normal_save_untouched":true,"slots":slots,"monsters":catalog.monsters.len(),"summons":if full{catalog.summons.len()}else{0},"equipment":if full{catalog.items.len()}else{0},"recipes":if full{catalog.recipes.len()}else{0},"new_monster_level":mode.level(),"silver":if full{MONEY}else{0},"gold_found_bonus":if full{MONEY}else{0}});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(staging.join(MARKER))
            .map_err(|e| e.to_string())?;
        file.write_all(serde_json::to_string_pretty(&marker).unwrap().as_bytes())
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        if target.exists() {
            return Err("Another test profile already exists".into());
        }
        std::fs::rename(&staging, &target).map_err(|e| format!("Cannot commit separate test directory: {e}"))?;
        log!("Created separate test collection: {} monsters, {} summons, {} equipment items, {} recipes.",catalog.monsters.len(),catalog.summons.len(),catalog.items.len(),catalog.recipes.len());
        Ok(())
    };
    if let Err(error) = create() {
        // Only this invocation's newly generated staging directory is removed.
        if staging.is_dir() && staging.parent() == Some(sandbox.as_path()) {
            let _ = std::fs::remove_dir_all(&staging);
        }
        return Err(error);
    }
    Ok(target)
}

pub(crate) fn level_one_catalog(ipa: &Path) -> Result<Vec<String>,String> {
    let key=preservation::save_key(ipa)?;
    Ok(Catalog::load(ipa,&key)?.monsters.into_iter().map(|(_,short)|short).collect())
}
fn validate_tree(path: &Path, count: &mut usize, total: &mut u64) -> Result<(), String> {
    let m = plain_file(path)?;
    *count += 1;
    *total += m.len();
    if *count > 10000 || *total > 512 * 1024 * 1024 {
        return Err("Oversized test directory".into());
    }
    if m.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            validate_tree(&entry.map_err(|e| e.to_string())?.path(), count, total)?;
        }
    } else if !m.is_file() {
        return Err("Unsupported test directory entry".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
