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

fn patch_map(bytes: Vec<u8>, cells: &BTreeMap<String, Vec<(usize, u32)>>) -> Result<Vec<u8>> {
    let mut map = String::from_utf8(bytes)?;
    for (name, updates) in cells {
        let layer = map.find(&format!("<layer name=\"{name}\"")).ok_or("missing map layer")?;
        let data = "<data encoding=\"base64\" compression=\"zlib\">";
        let start = layer + map[layer..].find(data).ok_or("missing layer data")? + data.len();
        let end = start + map[start..].find("</data>").ok_or("invalid map layer")?;
        let encoded: Vec<u8> = map[start..end].bytes().filter(|c| !c.is_ascii_whitespace()).collect();
        let compressed = STANDARD.decode(encoded)?;
        let mut bytes = read(ZlibDecoder::new(&compressed[..]))?;
        if bytes.len() != 300*280*4 { return Err("unsupported map dimensions".into()); }
        for &(i, tile) in updates { bytes.get_mut(4*i..4*i+4).ok_or("invalid cell")?.copy_from_slice(&tile.to_le_bytes()); }
        let mut compressed = ZlibEncoder::new(Vec::new(), Compression::default());
        compressed.write_all(&bytes)?;
        map.replace_range(start..end, &STANDARD.encode(compressed.finish()?));
    }
    Ok(map.into_bytes())
}

pub(crate) fn build(ipa: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let encoded = include_bytes!("expansion.json.gz");
    let recipe: Recipe = serde_json::from_reader(GzDecoder::new(&encoded[..]))?;
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
    for (name, digest) in &recipe.sources {
        let raw = read(archive.by_name(&format!("{prefix}{name}"))?)?;
        if crate::preservation::hex(&crate::preservation::hash(&raw)) != *digest {
            return Err(format!("Zalibas Expansion requires the original 2.02 resource {name}").into());
        }
        if name == "overworld.tmx" {
            output.insert(name.clone(), patch_map(raw, &recipe.cells)?);
            continue;
        }
        let plain = if raw.starts_with(b"bplist") || raw.starts_with(b"<?xml") { raw } else {
            read(GzDecoder::new(&crypt(1, &raw)?[..]))?
        };
        stock.insert(name.clone(), Value::from_reader(Cursor::new(plain))?);
    }
    for (name, edit) in recipe.resources {
        let result = recipe.edits[edit].apply(stock.get(&name), &stock, &recipe.edits)?;
        let mut bytes = Vec::new();
        result.to_writer_binary(&mut bytes)?;
        if name != "Chats.plist" && !name.ends_with(".strings") {
            let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
            compressed.write_all(&bytes)?;
            bytes = crypt(0, &compressed.finish()?)?;
        }
        output.insert(name, bytes);
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
    fn local_original_matches_authored_expansion() -> Result<()> {
        let Some(original) = std::env::var_os("HI_QA_ORIGINAL_IPA") else { return Ok(()) };
        let reference = std::env::var_os("HI_QA_EXPANSION_IPA").ok_or("missing private QA reference")?;
        let overlay = build(Path::new(&original))?;
        let mut archive = zip::ZipArchive::new(std::fs::File::open(reference)?)?;
        let prefix = archive.file_names().find(|n| n.ends_with(".app/overworld.tmx")).unwrap().trim_end_matches("overworld.tmx").to_owned();
        let text = crate::preservation::save_key(Path::new(&original))?;
        let mut key = [0;16];
        for (out, pair) in key.iter_mut().zip(text.chunks_exact(2)) {
            *out = u8::from_str_radix(std::str::from_utf8(pair)?,16)?;
        }
        for (name, bytes) in overlay {
            let expected = read(archive.by_name(&(prefix.clone()+&name))?)?;
            if name == "overworld.tmx" {
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
