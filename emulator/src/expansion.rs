// SPDX-License-Identifier: MPL-2.0
//! Build the optional read-only resource overlay from the player's original IPA.
use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::{read::{GzDecoder, ZlibDecoder}, write::{GzEncoder, ZlibEncoder}, Compression};
use plist::Value;
use serde::Deserialize;
use serde_json::Value as Json;
use std::{collections::BTreeMap, error::Error, io::{Cursor, Read, Write}, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Catalog = BTreeMap<String, Value>;
const LIMIT: u64 = 32 * 1024 * 1024;

#[derive(Deserialize)]
struct Recipe {
    sources: BTreeMap<String, String>,
    resources: BTreeMap<String, usize>,
    edits: Vec<Edit>,
    cells: BTreeMap<String, Vec<(usize, u32)>>,
    greenpath: BTreeMap<String, Vec<(usize, u32)>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Edit {
    Ref(Vec<Json>),
    Map(BTreeMap<String, usize>, Vec<String>),
    Array(usize, usize, Vec<usize>),
    Value(Json),
}

impl Edit {
    fn apply(&self, before: Option<&Value>, stock: &Catalog, edits: &[Edit]) -> Result<Value> {
        match self {
            Self::Ref(path) => {
                let mut value = stock.get(path[0].as_str().ok_or("invalid resource name")?).ok_or("missing reference")?;
                for key in &path[1..] {
                    value = match key {
                        Json::String(k) => value.as_dictionary().and_then(|d| d.get(k)),
                        Json::Number(n) => n.as_u64().and_then(|i| value.as_array()?.get(i as usize)),
                        _ => None,
                    }.ok_or("missing original resource field")?;
                }
                Ok(value.clone())
            }
            Self::Map(changes, removed) => {
                let mut dict = before.and_then(Value::as_dictionary).cloned().unwrap_or_default();
                for key in removed { dict.remove(key); }
                for (key, edit) in changes {
                    let result = edits[*edit].apply(dict.get(key), stock, edits)?;
                    dict.insert(key.clone(), result);
                }
                Ok(dict.into())
            }
            Self::Array(start, end, items) => {
                let original = before.and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
                let removed = original.get(*start..*end).ok_or("invalid array bounds")?;
                let mut result = original[..*start].to_vec();
                for (i, &edit) in items.iter().enumerate() { result.push(edits[edit].apply(removed.get(i), stock, edits)?); }
                result.extend_from_slice(&original[*end..]);
                Ok(Value::Array(result))
            }
            Self::Value(v) => Ok(match v {
                Json::Bool(b) => (*b).into(),
                Json::String(s) => s.clone().into(),
                Json::Number(n) if n.is_i64() => n.as_i64().unwrap().into(),
                Json::Number(n) if n.is_u64() => n.as_u64().unwrap().into(),
                Json::Number(n) => n.as_f64().ok_or("invalid number")?.into(),
                _ => return Err("unsupported literal".into()),
            }),
        }
    }
}

fn read(mut reader: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    (&mut reader).take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT { return Err("oversized expansion resource".into()); }
    Ok(bytes)
}

fn patch_map(bytes: Vec<u8>, cells: &BTreeMap<String, Vec<(usize, u32)>>, tile_count: usize) -> Result<Vec<u8>> {
    let mut map = String::from_utf8(bytes)?;
    for (name, updates) in cells {
        let layer = map.find(&format!("<layer name=\"{name}\"")).ok_or("missing map layer")?;
        let data = "<data encoding=\"base64\" compression=\"zlib\">";
        let start = layer + map[layer..].find(data).ok_or("missing layer data")? + data.len();
        let end = start + map[start..].find("</data>").ok_or("invalid map layer")?;
        let encoded: Vec<u8> = map[start..end].bytes().filter(|c| !c.is_ascii_whitespace()).collect();
        let compressed = STANDARD.decode(encoded)?;
        let mut bytes = read(ZlibDecoder::new(&compressed[..]))?;
        if bytes.len() != tile_count*4 { return Err("unsupported map dimensions".into()); }
        for &(i, tile) in updates { bytes.get_mut(4*i..4*i+4).ok_or("invalid cell")?.copy_from_slice(&tile.to_le_bytes()); }
        let mut compressed = ZlibEncoder::new(Vec::new(), Compression::default());
        compressed.write_all(&bytes)?;
        map.replace_range(start..end, &STANDARD.encode(compressed.finish()?));
    }
    Ok(map.into_bytes())
}

pub(crate) fn build(ipa: &Path, expansion: bool, codes: bool) -> Result<BTreeMap<String, Vec<u8>>> {
    let encoded = include_bytes!("expansion.json.gz");
    let recipe: Option<Recipe> = if expansion { Some(serde_json::from_reader(GzDecoder::new(&encoded[..]))?) } else { None };
    let text = crate::preservation::checked_save_key(ipa, Some("5b9572690e5dba80c3f6244c2e45d1c50479b46ed91045f6fd036780178e3b08"))?;
    let mut key = [0; 16];
    for (out, digits) in key.iter_mut().zip(text.chunks_exact(2)) {
        *out = u8::from_str_radix(std::str::from_utf8(digits)?, 16)?;
    }
    let crypt = |op, data: &[u8]| crate::libc::crypto::transform_local(op, 1, &key, &[0;16], data)
        .map_err(|_| "invalid encrypted catalog");
    let mut archive = zip::ZipArchive::new(std::fs::File::open(ipa)?)?;
    let prefix = archive.file_names().find(|n| n.starts_with("Payload/") && n.ends_with(".app/overworld.tmx") && n.split('/').count() == 3)
        .ok_or("missing Hunter Island map")?.trim_end_matches("overworld.tmx").to_owned();
    let mut stock = Catalog::new();
    let mut output = BTreeMap::new();
    let decode = |raw: Vec<u8>| -> Result<Value> {
        let plain = if raw.starts_with(b"bplist") || raw.starts_with(b"<?xml") { raw } else {
            read(GzDecoder::new(&crypt(1, &raw)?[..]))?
        };
        Ok(Value::from_reader(Cursor::new(plain))?)
    };
    let encode = |name: &str, result: &Value| -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        result.to_writer_binary(&mut bytes)?;
        if name != "Chats.plist" && !name.ends_with(".strings") {
            let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
            compressed.write_all(&bytes)?;
            bytes = crypt(0, &compressed.finish()?)?;
        }
        Ok(bytes)
    };
    if let Some(recipe) = recipe {
        for (name, digest) in &recipe.sources {
            let raw = read(archive.by_name(&format!("{prefix}{name}"))?)?;
            if crate::preservation::hex(&crate::preservation::hash(&raw)) != *digest {
                return Err(format!("Zalibas Expansion requires the original 2.02 resource {name}").into());
            }
            if name == "overworld.tmx" {
                output.insert(name.clone(), patch_map(raw, &recipe.cells, 300*280)?);
                continue;
            }
            if name == "GreenpathOverlay.tmx" {
                output.insert(name.clone(), patch_map(raw, &recipe.greenpath, 8*24)?);
                continue;
            }
            stock.insert(name.clone(), decode(raw)?);
        }
        for (name, edit) in recipe.resources {
            let result = recipe.edits[edit].apply(stock.get(&name), &stock, &recipe.edits)?;
            let bytes = encode(&name, &result)?;
            output.insert(name, bytes);
        }
    }
    if codes {
        let mut load = |name: &str, overlay: &BTreeMap<String, Vec<u8>>| -> Result<Value> {
            if let Some(raw) = overlay.get(name) { return decode(raw.clone()); }
            if let Some(value) = stock.get(name) { return Ok(value.clone()); }
            decode(read(archive.by_name(&format!("{prefix}{name}"))?)?)
        };
        let monsters = load("Monsters.plist", &output)?;
        let mut towns = load("Towns.plist", &output)?;
        crate::gravestone::towns(&mut towns, &monsters)?;
        output.insert("Towns.plist".into(), encode("Towns.plist", &towns)?);
        let mut items = load("Items.plist", &output)?;
        crate::gravestone::items(&mut items)?;
        output.insert("Items.plist".into(), encode("Items.plist", &items)?);
        for (locale, hint) in crate::gravestone::PROMPTS {
            let name = format!("{locale}.lproj/Localizable.strings");
            let mut strings = load(&name, &output)?;
            let strings_dict = strings.as_dictionary_mut().ok_or("Invalid localization")?;
            // MonsterJoin and SpiritTotem format the species' localized name
            // through the pickup type's own format string.
            for (original,alias) in [("MonsterJoin","GravestoneJoin"), ("SpiritTotem","GravestoneTotem")] {
                let format = strings_dict.get(&format!("Renderable{original}NameKey"))
                    .ok_or("Missing localized pickup name")?.clone();
                strings_dict.insert(format!("Renderable{alias}NameKey"), format);
            }
            let key = "DI.DIQuestionChatKey";
            let prompt = strings_dict.get(key).and_then(Value::as_string).ok_or("Missing gravestone question")?;
            strings_dict.insert(key.into(), format!("{prompt} {hint}").into());
            output.insert(name.clone(), encode(&name, &strings)?);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_recipe_has_six_languages_and_map_changes() {
        let bytes = include_bytes!("expansion.json.gz");
        let recipe: Recipe = serde_json::from_reader(GzDecoder::new(&bytes[..])).unwrap();
        assert_eq!(recipe.resources.len(), 54);
        assert_eq!(recipe.resources.keys().filter(|name| name.ends_with(".strings")).count(), 6);
        assert_eq!(recipe.cells.values().map(Vec::len).sum::<usize>(), 184);
    }

    #[test]
    fn local_gravestone_codes_follow_book_order_before_and_after_the_riddle() -> Result<()> {
        let Some(path) = std::env::var_os("HI_QA_ORIGINAL_IPA") else { return Ok(()) };
        let ipa = Path::new(&path);
        let text = crate::preservation::save_key(ipa)?;
        let mut key = [0; 16];
        for (byte, pair) in key.iter_mut().zip(text.chunks_exact(2)) {
            *byte = u8::from_str_radix(std::str::from_utf8(pair)?,16)?;
        }
        for expansion in [false,true] {
            let overlay = build(ipa,expansion,true)?;
            let plain = crate::libc::crypto::transform_local(1,1,&key,&[0;16],&overlay["Towns.plist"]).unwrap();
            let towns = Value::from_reader(Cursor::new(read(GzDecoder::new(&plain[..]))?))?;
            let buildings = towns.as_dictionary().unwrap()["over-26x111"].as_dictionary().unwrap()["buildings"].as_array().unwrap();
            let mut states = Vec::new();
            for b in buildings.iter().filter_map(Value::as_dictionary) {
                let Some(args) = b.get("arguments").and_then(Value::as_dictionary) else { continue };
                if args.get("promptEvent").and_then(Value::as_string)!=Some("chat:DI.DIQuestion") { continue; }
                let answers = args["answerEvents"].as_dictionary().unwrap();
                let mut species = std::collections::BTreeSet::new();
                for n in 1..=360 {
                    let event = &answers[&n.to_string()];
                    assert_eq!(event, &answers[&format!("{n:03}")]);
                    let grant = event.as_dictionary().unwrap()["arguments"].as_dictionary().unwrap();
                    assert_eq!(grant["forceNotUnique"].as_boolean(), Some(true));
                    let loot = grant["treasureData"].as_dictionary().unwrap()["items"].as_array().unwrap();
                    let item = loot[0].as_dictionary().unwrap()["data"].as_dictionary().unwrap();
                    assert_eq!(item["type"].as_string(), Some(if n<=353 {crate::gravestone::JOIN} else {crate::gravestone::TOTEM}));
                    let short = item["arguments"].as_dictionary().unwrap()["shortType"].as_string().unwrap();
                    assert!(species.insert(short));
                    if n==1 {assert_eq!(short,"Electricgorilla01");}
                    if n==360 {assert_eq!(short,"Tiamat");}
                }
                let condition = b["conditions"].as_dictionary().unwrap()["chats"].as_string().unwrap();
                assert_eq!(answers.contains_key("DIQuestionAnswer"),condition.starts_with('!'));
                assert!(!answers.contains_key("0") && !answers.contains_key("361"));
                states.push(condition);
            }
            assert_eq!(states, ["!DI.DIRightAnswer","DI.DIRightAnswer"]);
            for (locale,hint) in crate::gravestone::PROMPTS {
                let strings = Value::from_reader(Cursor::new(&overlay[&format!("{locale}.lproj/Localizable.strings")]))?;
                assert!(strings.as_dictionary().unwrap()["DI.DIQuestionChatKey"].as_string().unwrap().contains(hint));
            }
        }
        Ok(())
    }

    #[test]
    fn local_original_matches_authored_expansion() -> Result<()> {
        let Some(original) = std::env::var_os("HI_QA_ORIGINAL_IPA") else { return Ok(()) };
        let reference = std::env::var_os("HI_QA_EXPANSION_IPA").ok_or("missing private QA reference")?;
        let overlay = build(Path::new(&original), true, false)?;
        let mut archive = zip::ZipArchive::new(std::fs::File::open(reference)?)?;
        let prefix = archive.file_names().find(|n| n.ends_with(".app/overworld.tmx")).unwrap().trim_end_matches("overworld.tmx").to_owned();
        let text = crate::preservation::save_key(Path::new(&original))?;
        let mut key = [0;16];
        for (out, pair) in key.iter_mut().zip(text.chunks_exact(2)) {
            *out = u8::from_str_radix(std::str::from_utf8(pair)?,16)?;
        }
        for (name, bytes) in overlay {
            let expected = read(archive.by_name(&(prefix.clone()+&name))?)?;
            if name.ends_with(".tmx") {
                // Keep the XML structure, compare the actual tiles after inflating each layer.
                fn contents(raw: Vec<u8>) -> Result<(String, Vec<Vec<u8>>)> {
                    let text = String::from_utf8(raw)?;
                    let tag = "<data encoding=\"base64\" compression=\"zlib\">";
                    let mut xml = String::new();
                    let mut tiles = Vec::new();
                    let mut rest = text.as_str();
                    while let Some(i) = rest.find(tag) {
                        let start = i+tag.len();
                        xml.push_str(&rest[..start]);
                        rest = &rest[start..];
                        let end = rest.find("</data>").ok_or("unterminated layer")?;
                        let encoded: Vec<u8> = rest[..end].bytes().filter(|c|!c.is_ascii_whitespace()).collect();
                        tiles.push(read(ZlibDecoder::new(&STANDARD.decode(encoded)?[..]))?);
                        rest = &rest[end..];
                    }
                    xml.push_str(rest);
                    Ok((xml,tiles))
                }
                assert!(contents(bytes)? == contents(expected)?, "world map mismatch");
            } else {
                let decode = |data: Vec<u8>| -> Result<Value> {
                    let raw = if data.starts_with(b"bplist") {data} else {
                        let plain = crate::libc::crypto::transform_local(1,1,&key,&[0;16],&data).map_err(|_| "invalid reference")?;
                        read(GzDecoder::new(&plain[..]))?
                    };
                    Ok(Value::from_reader(Cursor::new(raw))?)
                };
                assert!(decode(bytes)? == decode(expected)?, "expansion mismatch in {name}");
            }
        }
        Ok(())
    }
}
