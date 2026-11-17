/* SPDX-License-Identifier: MPL-2.0 */
//! Local save provenance. Checksums detect inconsistency, not an adversarial
//! owner who can replace this program and rewrite its complete local history.
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Write};

pub const FIELD: &str = "_r7";
pub const MONSTER_FIELD: &str = "_r8";
pub const RARE: u32 = 1;
pub const EXCHANGE: u32 = 2;
pub const ARENA: u32 = 4;
const MAGIC: &[u8] = &[0x91, 0xc7, 0x36, 1];
const LIMIT: usize = 2 * 1024 * 1024;
const JOURNAL: usize = 256;

pub fn hash(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}
pub fn hex(data: &[u8]) -> String {
    data.iter().map(|n| format!("{n:02x}")).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lineage {
    #[serde(rename = "0")]
    pub origin_species: String,
    #[serde(rename = "1")]
    pub current_species: String,
    #[serde(rename = "2")]
    // Bit 1: capture while enabled; bit 2: capture from an affected encounter.
    // These are rare-capture provenance bits, not the profile's mod-option mask.
    pub flags: u32,
    #[serde(rename = "3")]
    pub encounter: u64,
    #[serde(rename = "4")]
    pub captured_at_event: u64,
    #[serde(rename = "5")]
    pub evolutions: u64,
    #[serde(default, rename = "6", skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<String>,
    #[serde(default, rename = "7", skip_serializing_if = "Vec::is_empty")]
    pub ingredients: Vec<(String, Option<String>)>,
    #[serde(default, rename = "8")]
    pub fused_at_event: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "0")]
    pub number: u64,
    #[serde(rename = "1")]
    pub kind: u8,
    #[serde(rename = "2")]
    pub details: serde_json::Value,
    #[serde(rename = "3")]
    pub digest: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    #[serde(rename = "0")]
    pub version: u32,
    #[serde(rename = "1")]
    pub id: [u8; 16],
    // 0: imported/unknown before first observation; 1: fresh slot observed;
    // 2: damaged/mismatched metadata. Missing metadata is never clean evidence.
    #[serde(rename = "2")]
    pub origin: u8,
    #[serde(rename = "3")]
    pub save_sequence: u64,
    #[serde(rename = "4")]
    pub ever_enabled: u32,
    #[serde(rename = "5")]
    pub game_hash: [u8; 32],
    #[serde(rename = "6")]
    pub previous_save: [u8; 32],
    // encounters, spawned, captures(active), captures(influenced), exchanges,
    // silver spent, gold obtained, early arena battles, skipped milliseconds.
    #[serde(rename = "7")]
    pub counts: [u64; 9],
    #[serde(rename = "8")]
    pub spawned_species: BTreeMap<String, u64>,
    #[serde(rename = "9")]
    pub captured_species: BTreeMap<String, u64>,
    #[serde(rename = "10")]
    pub lineages: BTreeMap<String, Lineage>,
    #[serde(rename = "11")]
    pub events: VecDeque<Event>,
    #[serde(rename = "12")]
    pub event_sequence: u64,
    #[serde(rename = "13")]
    pub base_event: u64,
    #[serde(rename = "14")]
    pub base_hash: [u8; 32],
    #[serde(rename = "15")]
    pub head: [u8; 32],
    #[serde(rename = "16")]
    pub incomplete: bool,
    #[serde(rename = "17")]
    pub source_version: String,
    #[serde(rename = "18")]
    pub observed_milliseconds: u64,
    #[serde(default, rename = "19")]
    pub entered_encounters: u64,
    #[serde(default, rename = "20")]
    pub hatchlings_seen: u64,
    #[serde(default, rename = "21")]
    pub marked_fusions: u64,
    #[serde(default, rename = "22")]
    pub test_profile: bool,
    #[serde(default, rename = "23")]
    pub test_profile_modes: u32,
}

fn add(n: &mut u64, amount: u64) {
    *n = n.saturating_add(amount);
}
fn bump(map: &mut BTreeMap<String, u64>, key: &str) {
    add(map.entry(key.into()).or_default(), 1);
}

impl Record {
    pub fn new(id: [u8; 16], origin: u8) -> Self {
        Self {
            version: 2,
            id,
            origin,
            save_sequence: 0,
            ever_enabled: 0,
            game_hash: [0; 32],
            previous_save: [0; 32],
            counts: [0; 9],
            spawned_species: BTreeMap::new(),
            captured_species: BTreeMap::new(),
            lineages: BTreeMap::new(),
            events: VecDeque::new(),
            event_sequence: 0,
            base_event: 0,
            base_hash: [0; 32],
            head: [0; 32],
            incomplete: false,
            source_version: crate::VERSION.into(),
            observed_milliseconds: 0,
            entered_encounters: 0,
            hatchlings_seen: 0,
            marked_fusions: 0,
            test_profile: false,
            test_profile_modes: 0,
        }
    }
    fn event(&mut self, kind: u8, details: serde_json::Value) {
        self.event_sequence = self.event_sequence.saturating_add(1);
        let encoded = serde_json::to_vec(&(self.event_sequence, kind, &details)).unwrap();
        let mut h = Sha256::new();
        h.update(self.head);
        h.update(encoded);
        self.head = h.finalize().into();
        self.events.push_back(Event {
            number: self.event_sequence,
            kind,
            details,
            digest: self.head,
        });
        if self.events.len() > JOURNAL {
            let old = self.events.pop_front().unwrap();
            self.base_event = old.number;
            self.base_hash = old.digest;
        }
    }
    pub fn activate(&mut self, flags: u32) {
        let added = flags & !self.ever_enabled;
        self.ever_enabled |= flags;
        if added != 0 {
            self.event(1, serde_json::json!([added]));
        }
    }
    pub fn draw(&mut self, species: &str, selected: bool) -> u64 {
        self.activate(RARE);
        add(&mut self.counts[0], 1);
        if selected {
            add(&mut self.counts[1], 1);
            bump(&mut self.spawned_species, species);
        }
        self.event(2, serde_json::json!([species, selected]));
        self.event_sequence
    }
    pub fn capture(&mut self, id: &str, species: &str, active: bool, encounter: u64) {
        if self.lineages.contains_key(id) {
            return;
        }
        if !active && encounter == 0 {
            return;
        }
        if active {
            self.activate(RARE);
            add(&mut self.counts[2], 1);
        }
        if encounter != 0 {
            add(&mut self.counts[3], 1);
        }
        bump(&mut self.captured_species, species);
        self.event(3, serde_json::json!([id, species, active, encounter]));
        self.lineages.insert(
            id.into(),
            Lineage {
                origin_species: species.into(),
                current_species: species.into(),
                flags: u32::from(active) | if encounter != 0 { 2 } else { 0 },
                encounter,
                captured_at_event: self.event_sequence,
                evolutions: 0,
                parents: Vec::new(),
                ingredients: Vec::new(),
                fused_at_event: 0,
            },
        );
    }
    pub fn enter(&mut self, number: u64, selected: bool) {
        add(&mut self.entered_encounters, 1);
        if selected {
            add(&mut self.hatchlings_seen, 1);
        }
        self.event(7, serde_json::json!([number, selected]));
    }
    pub fn fuse(
        &mut self,
        id: &str,
        species: &str,
        ingredients: &[(String, Option<String>)],
    ) -> bool {
        let parents: Vec<_> = ingredients
            .iter()
            .filter_map(|(_, id)| id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if parents.is_empty() || self.lineages.contains_key(id) {
            return false;
        }
        let mut flags = 0;
        for parent in &parents {
            if let Some(lineage) = self.lineages.get(parent) {
                flags |= lineage.flags;
            } else {
                self.incomplete = true;
            }
        }
        self.version = 2;
        self.event(8, serde_json::json!([id, species, ingredients]));
        add(&mut self.marked_fusions, 1);
        self.lineages.insert(
            id.into(),
            Lineage {
                origin_species: species.into(),
                current_species: species.into(),
                flags,
                encounter: 0,
                captured_at_event: 0,
                evolutions: 0,
                parents,
                ingredients: ingredients.to_vec(),
                fused_at_event: self.event_sequence,
            },
        );
        true
    }
    pub fn ancestors(&self, id: &str) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        let mut pending = self
            .lineages
            .get(id)
            .map(|l| l.parents.clone())
            .unwrap_or_default();
        while let Some(parent) = pending.pop() {
            if seen.insert(parent.clone()) {
                if let Some(lineage) = self.lineages.get(&parent) {
                    pending.extend(lineage.parents.iter().cloned());
                }
            }
        }
        seen.into_iter().collect()
    }
    pub fn observe_form(&mut self, id: &str, species: &str) {
        let Some(lineage) = self.lineages.get_mut(id) else {
            return;
        };
        if lineage.current_species == species {
            return;
        }
        lineage.current_species = species.into();
        add(&mut lineage.evolutions, 1);
        self.event(4, serde_json::json!([id, species]));
    }
    pub fn exchange(&mut self, silver: u64, gold: u64) {
        self.activate(EXCHANGE);
        add(&mut self.counts[4], 1);
        add(&mut self.counts[5], silver);
        add(&mut self.counts[6], gold);
        self.event(5, serde_json::json!([silver, gold]));
    }
    pub fn arena(&mut self, milliseconds: u64) {
        if milliseconds == 0 {
            return;
        }
        self.activate(ARENA);
        add(&mut self.counts[7], 1);
        add(&mut self.counts[8], milliseconds);
        self.event(6, serde_json::json!([milliseconds]));
    }
    pub fn verify(&self) -> Result<(), String> {
        if !matches!(self.version, 1 | 2)
            || self.origin > 2
            || self.ever_enabled & !7 != 0
            || self.test_profile_modes & !7 != 0
            || (self.test_profile_modes != 0 && !self.test_profile)
            || self.events.len() > JOURNAL
            || self.lineages.len() > 16384
            || self.spawned_species.len() > 32
            || self.captured_species.len() > 32
            || self.base_event > self.event_sequence
        {
            return Err("Unsupported or oversized history".into());
        }
        for (id, l) in &self.lineages {
            if l.parents.len() > 64 || l.ingredients.len() > 64 {
                return Err("Too many fusion ingredients".into());
            }
            for parent in &l.parents {
                if parent == id || l.fused_at_event == 0 {
                    return Err("Invalid fusion ancestry".into());
                }
                if let Some(p) = self.lineages.get(parent) {
                    if p.captured_at_event.max(p.fused_at_event) >= l.fused_at_event {
                        return Err("Fusion ancestry is not chronological".into());
                    }
                } else if !self.incomplete {
                    return Err("Missing fusion ancestor".into());
                }
            }
        }
        let mut h = self.base_hash;
        let mut n = self.base_event;
        for e in &self.events {
            n = n.checked_add(1).ok_or("Event overflow")?;
            if e.number != n {
                return Err("Event sequence mismatch".into());
            }
            let encoded =
                serde_json::to_vec(&(e.number, e.kind, &e.details)).map_err(|e| e.to_string())?;
            let mut sha = Sha256::new();
            sha.update(h);
            sha.update(encoded);
            h = sha.finalize().into();
            if h != e.digest {
                return Err("Event digest mismatch".into());
            }
        }
        if h != self.head || n != self.event_sequence {
            return Err("History head mismatch".into());
        }
        if !self.incomplete {
            if self.counts[5] != self.counts[6].checked_mul(50).ok_or("Currency overflow")? {
                return Err("Exchange totals mismatch".into());
            }
            if self.counts[1] > self.counts[0] || self.hatchlings_seen > self.entered_encounters {
                return Err("Encounter totals mismatch".into());
            }
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.verify()?;
        let raw = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if raw.len() > LIMIT {
            return Err("History exceeds the size limit".into());
        }
        let mut out = MAGIC.to_vec();
        out.extend(hash(&raw));
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(&raw).map_err(|e| e.to_string())?;
        out.extend(z.finish().map_err(|e| e.to_string())?);
        Ok(out)
    }
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.len() < 37 || data.len() > LIMIT || &data[..4] != MAGIC {
            return Err("Invalid history envelope".into());
        }
        let mut z = ZlibDecoder::new(&data[36..]);
        let mut raw = Vec::new();
        (&mut z)
            .take((LIMIT + 1) as u64)
            .read_to_end(&mut raw)
            .map_err(|e| e.to_string())?;
        if raw.len() > LIMIT
            || z.total_in() as usize != data.len() - 36
            || hash(&raw) != data[4..36]
        {
            return Err("History checksum/length mismatch".into());
        }
        let mut record: Self = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        record.verify()?;
        record.version = 2;
        Ok(record)
    }
    pub fn report(&self) -> serde_json::Value {
        let status = if self.test_profile {
            "test_profile"
        } else if self.origin == 2 || self.incomplete {
            "inconsistent_or_incomplete"
        } else if self.ever_enabled != 0 {
            "mod_activation_recorded"
        } else if self.origin == 0 {
            "prior_history_unknown"
        } else {
            "observed_from_fresh_slot_no_mod_activation"
        };
        serde_json::json!({"status":status,"history_schema":self.version,"profile_id":hex(&self.id),"origin":self.origin,"save_sequence":self.save_sequence,
            "ever_enabled":{"rare_dragons":self.ever_enabled&RARE!=0,"silver_exchange":self.ever_enabled&EXCHANGE!=0,"no_arena_cooldown":self.ever_enabled&ARENA!=0},
            "counts":{"generated_encounters":self.counts[0],"hatchlings_selected_in_generated_packs":self.counts[1],"entered_affected_encounters":self.entered_encounters,"hatchlings_seen":self.hatchlings_seen,"hatchlings_captured_while_enabled":self.counts[2],"hatchlings_captured_from_affected_encounters":self.counts[3],"successful_exchanges":self.counts[4],"silver_exchanged":self.counts[5],"gold_received":self.counts[6],"arena_begin_overrides":self.counts[7],"skipped_arena_milliseconds":self.counts[8],"marked_fusions":self.marked_fusions},
            "spawned_species":self.spawned_species,"captured_species":self.captured_species,"lineages":self.lineages.iter().map(|(id,l)|serde_json::json!({"id":id,"original_species":l.origin_species,"current_species":l.current_species,"captured_while_enabled":l.captured_at_event!=0 && l.flags&1!=0,"inherited_rare_provenance_flags":l.flags,"contains_mod_enabled_capture":l.flags&1!=0,"contains_affected_encounter_capture":l.flags&2!=0,"affected_encounter":l.encounter,"evolutions":l.evolutions,"fusion_event":l.fused_at_event,"parents":l.parents,"ingredients":l.ingredients,"ancestor_lineage_ids":self.ancestors(id)})).collect::<Vec<_>>(),
            "event_count":self.event_sequence,"retained_events":self.events.len(),"observed_loaded_runtime_ms":self.observed_milliseconds,
            "test_profile":self.test_profile,"test_profile_modes":{"all_inclusive":self.test_profile_modes&1!=0,"level_one":self.test_profile_modes&2!=0,"triple_level99":self.test_profile_modes&4!=0},"source_version":self.source_version,"local_history_is_not_authenticated":true})
    }
}

fn canonical(value: &plist::Value, top: bool) -> plist::Value {
    match value {
        plist::Value::Dictionary(d) => {
            let sorted: BTreeMap<_, _> = d
                .iter()
                .filter(|(k, _)| !top || k.as_str() != FIELD)
                .map(|(k, v)| (k.clone(), canonical(v, false)))
                .collect();
            plist::Value::Dictionary(sorted.into_iter().collect())
        }
        plist::Value::Array(a) => {
            plist::Value::Array(a.iter().map(|v| canonical(v, false)).collect())
        }
        other => other.clone(),
    }
}
pub fn game_hash(value: &plist::Value) -> Result<[u8; 32], String> {
    let mut data = Vec::new();
    canonical(value, true)
        .to_writer_binary(&mut data)
        .map_err(|e| e.to_string())?;
    Ok(hash(&data))
}

pub fn inspect_plist(value: &plist::Value) -> serde_json::Value {
    let Some(blob) = value
        .as_dictionary()
        .and_then(|d| d.get(FIELD))
        .and_then(plist::Value::as_data)
    else {
        return serde_json::json!({"status":"history_missing","prior_history_unknown":true,"local_history_is_not_authenticated":true});
    };
    match Record::decode(blob) {
        Ok(r) => {
            if game_hash(value).ok() != Some(r.game_hash) {
                return serde_json::json!({"status":"save_payload_mismatch","local_history_is_not_authenticated":true});
            }
            let mut report = r.report();
            let mut ids = std::collections::BTreeSet::new();
            fn owned(value: &plist::Value, ids: &mut std::collections::BTreeSet<String>) {
                match value {
                    plist::Value::Dictionary(d) => {
                        if let Some(data) = d.get(MONSTER_FIELD).and_then(plist::Value::as_data) {
                            if data.len() == 32 && data.iter().all(|b| b.is_ascii_hexdigit()) {
                                ids.insert(String::from_utf8_lossy(data).into_owned());
                            }
                        }
                        for (_, v) in d {
                            owned(v, ids);
                        }
                    }
                    plist::Value::Array(a) => {
                        for v in a {
                            owned(v, ids);
                        }
                    }
                    _ => (),
                }
            }
            if let Some(player) = value.as_dictionary().and_then(|d| d.get("playerData")) {
                owned(player, &mut ids);
            }
            report["owned_lineage_ids"] = serde_json::json!(ids);
            report
        }
        Err(e) => {
            serde_json::json!({"status":"invalid_history","reason":e,"local_history_is_not_authenticated":true})
        }
    }
}

/// Static inspection only: recover the game's existing data key from the
/// user's own IPA. No game key or proprietary executable is distributed.
pub fn inspect_file(
    ipa: &std::path::Path,
    save: &std::path::Path,
) -> Result<serde_json::Value, String> {
    let key = save_key(ipa)?;
    let mut bytes = Vec::new();
    std::fs::File::open(save)
        .map_err(|e| e.to_string())?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Save exceeds the size limit".into());
    }
    let plain = if bytes.starts_with(b"bplist00") {
        bytes
    } else {
        crate::libc::crypto::decode_local_save(&key, &bytes)?
    };
    let value = plist::Value::from_reader(std::io::Cursor::new(plain))
        .map_err(|_| "Decoded data is not a game property list")?;
    if !value
        .as_dictionary()
        .is_some_and(|d| d.contains_key("playerData") && d.contains_key("props"))
    {
        return Err("This is not a Hunter Island game save".into());
    }
    Ok(inspect_plist(&value))
}

pub(crate) fn save_key(ipa: &std::path::Path) -> Result<[u8; 32], String> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(ipa).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if archive.len() > 50000 {
        return Err("Too many IPA entries".into());
    }
    let infos: Vec<_> = archive
        .file_names()
        .filter(|n| {
            n.starts_with("Payload/") && n.ends_with(".app/Info.plist") && n.split('/').count() == 3
        })
        .map(str::to_owned)
        .collect();
    if infos.len() != 1 {
        return Err("Expected one application in the IPA".into());
    }
    let mut info = Vec::new();
    archive
        .by_name(&infos[0])
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut info)
        .map_err(|e| e.to_string())?;
    if info.len() > 1024 * 1024 {
        return Err("Oversized app metadata".into());
    }
    let meta = plist::Value::from_reader(std::io::Cursor::new(info)).map_err(|e| e.to_string())?;
    let d = meta.as_dictionary().ok_or("Invalid app metadata")?;
    if d.get("CFBundleIdentifier")
        .and_then(plist::Value::as_string)
        != Some("pockettrend.hunterisland")
        || d.get("CFBundleShortVersionString")
            .or_else(|| d.get("CFBundleVersion"))
            .and_then(plist::Value::as_string)
            != Some("2.02")
    {
        return Err("A compatible Hunter Island 2.02 IPA is required".into());
    }
    let executable = d
        .get("CFBundleExecutable")
        .and_then(plist::Value::as_string)
        .ok_or("Missing executable name")?;
    if executable.is_empty() || executable.contains(['/', '\\']) {
        return Err("Invalid executable name".into());
    }
    let name = infos[0].trim_end_matches("Info.plist").to_owned() + executable;
    let mut binary = Vec::new();
    archive
        .by_name(&name)
        .map_err(|e| e.to_string())?
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut binary)
        .map_err(|e| e.to_string())?;
    if binary.len() > 128 * 1024 * 1024 {
        return Err("Oversized executable".into());
    }
    key_from_macho(&binary)
}

fn key_from_macho(binary: &[u8]) -> Result<[u8; 32], String> {
    fn u32_at(b: &[u8], p: usize, big: bool) -> Result<u32, String> {
        let a: [u8; 4] = b
            .get(p..p.checked_add(4).ok_or("Offset overflow")?)
            .ok_or("Truncated executable")?
            .try_into()
            .unwrap();
        Ok(if big {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        })
    }
    let b = if binary.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
        let count = u32_at(binary, 4, true)? as usize;
        if count == 0 || count > 16 {
            return Err("Invalid fat executable".into());
        }
        let mut chosen = None;
        for i in 0..count {
            let p = 8 + i * 20;
            let cpu = u32_at(binary, p, true)?;
            let sub = u32_at(binary, p + 4, true)?;
            let off = u32_at(binary, p + 8, true)? as usize;
            let len = u32_at(binary, p + 12, true)? as usize;
            let slice = binary
                .get(off..off.checked_add(len).ok_or("Slice overflow")?)
                .ok_or("Invalid executable slice")?;
            if cpu == 12 && sub & 0xff == 9 {
                chosen = Some(slice)
            }
        }
        chosen.ok_or("The IPA has no ARMv7 executable")?
    } else {
        binary
    };
    if u32_at(b, 0, false)? != 0xfeedface || u32_at(b, 4, false)? != 12 {
        return Err("Expected the decrypted ARMv7 executable".into());
    }
    let count = u32_at(b, 16, false)? as usize;
    let end = 28usize
        .checked_add(u32_at(b, 20, false)? as usize)
        .ok_or("Command overflow")?;
    if count > 10000 || end > b.len() {
        return Err("Invalid executable commands".into());
    }
    let mut segments = Vec::new();
    let mut p = 28;
    for _ in 0..count {
        let command = u32_at(b, p, false)?;
        let size = u32_at(b, p + 4, false)? as usize;
        if size < 8 || p + size > end {
            return Err("Invalid executable command".into());
        }
        if command == 1 {
            if size < 56 {
                return Err("Truncated segment".into());
            }
            segments.push((
                u32_at(b, p + 24, false)? as u64,
                u32_at(b, p + 32, false)? as u64,
                u32_at(b, p + 36, false)? as u64,
            ));
        }
        if command == 0x21 && (size < 20 || u32_at(b, p + 16, false)? != 0) {
            return Err("The IPA executable is encrypted".into());
        }
        p += size;
    }
    let offset = |address: u64, len: usize| -> Result<usize, String> {
        for &(va, file, size) in &segments {
            if address >= va && address - va + (len as u64) <= size {
                let pos =
                    usize::try_from(file + address - va).map_err(|_| "Invalid file offset")?;
                if pos.checked_add(len).is_some_and(|e| e <= b.len()) {
                    return Ok(pos);
                }
            }
        }
        Err("The IPA does not match the supported 2.02 layout".into())
    };
    // CFStrings returned by the eight lookupPrefix methods, in Saving's
    // stringWithFormat argument order. Read values from the user's executable.
    let addresses = [
        0x3c93e4u64,
        0x3c9034,
        0x3c91f4,
        0x3c8db4,
        0x3c7ca4,
        0x3c8a14,
        0x3c89b4,
        0x3c8434,
    ];
    let mut bytes = Vec::new();
    for address in addresses {
        let p = offset(address, 16)?;
        let flags = u32_at(b, p + 4, false)?;
        let ptr = u32_at(b, p + 8, false)?;
        let len = u32_at(b, p + 12, false)? as usize;
        if flags != 0x7c8 || len == 0 || len > 128 {
            return Err("Unexpected game key data".into());
        }
        let p = offset(ptr as u64, len)?;
        bytes.extend_from_slice(&b[p..p + len]);
    }
    let mut key = [0; 32];
    let n = bytes.len().min(32);
    key[..n].copy_from_slice(&bytes[..n]);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fusion_merges_all_marked_inputs_and_keeps_multigeneration_ancestry() {
        let mut r = Record::new([6; 16], 1);
        r.capture("a", "Firedragon01", true, 0);
        r.capture("b", "Waterdragon01", true, 0);
        let captures = r.counts;
        assert!(r.fuse(
            "c",
            "TestFusion",
            &[
                ("Clean".into(), None),
                ("Firedragon01".into(), Some("a".into()))
            ]
        ));
        assert!(r.fuse(
            "d",
            "NextFusion",
            &[
                ("Waterdragon01".into(), Some("b".into())),
                ("TestFusion".into(), Some("c".into()))
            ]
        ));
        assert_eq!(r.ancestors("d"), vec!["a", "b", "c"]);
        assert_eq!(r.lineages["d"].flags, 1);
        assert_eq!(r.marked_fusions, 2);
        assert_eq!(r.counts, captures);
        assert!(!r.fuse("d", "NextFusion", &[("a".into(), Some("a".into()))]));
        r.observe_form("d", "EvolvedFusion");
        let loaded = Record::decode(&r.encode().unwrap()).unwrap();
        assert_eq!(loaded.lineages["d"].current_species, "EvolvedFusion");
        assert_eq!(loaded.ancestors("d"), vec!["a", "b", "c"]);
        assert_eq!(loaded.marked_fusions, 2);
        assert_eq!(
            loaded.report()["lineages"][3]["captured_while_enabled"],
            false
        );
        let mut damaged = loaded.clone();
        damaged
            .lineages
            .get_mut("d")
            .unwrap()
            .parents
            .push("d".into());
        assert!(damaged.verify().is_err());
    }
    #[test]
    fn unmarked_inputs_do_not_create_history_and_unknown_parents_are_incomplete() {
        let mut r = Record::new([7; 16], 1);
        assert!(!r.fuse("a", "Plain", &[("Clean".into(), None)]));
        assert_eq!(r.event_sequence, 0);
        assert!(r.fuse(
            "b",
            "Uncertain",
            &[("Unknown".into(), Some("missing".into()))]
        ));
        assert_eq!(
            Record::decode(&r.encode().unwrap()).unwrap().report()["status"],
            "inconsistent_or_incomplete"
        );
    }
    #[test]
    fn legacy_v1_history_migrates_without_losing_capture_or_exchange_totals() {
        let mut r = Record::new([8; 16], 0);
        r.capture("a", "Earthdragon01", true, 0);
        r.exchange(50, 1);
        r.version = 1;
        let mut json = serde_json::to_value(&r).unwrap();
        json.as_object_mut().unwrap().remove("21");
        let lineage = json["10"]["a"].as_object_mut().unwrap();
        for key in ["6", "7", "8"] {
            lineage.remove(key);
        }
        let raw = serde_json::to_vec(&json).unwrap();
        let mut blob = MAGIC.to_vec();
        blob.extend(hash(&raw));
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(&raw).unwrap();
        blob.extend(z.finish().unwrap());
        let loaded = Record::decode(&blob).unwrap();
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.counts, r.counts);
        assert_eq!(loaded.lineages["a"].origin_species, "Earthdragon01");
        assert_eq!(loaded.origin, 0);
        assert_eq!(loaded.marked_fusions, 0);
    }
    #[test]
    fn roundtrip_counters_events_and_sticky_activation() {
        let mut r = Record::new([1; 16], 1);
        r.activate(RARE);
        let encounter = r.draw("Firedragon01", true);
        r.capture("a", "Firedragon01", false, encounter);
        r.capture("a", "Firedragon01", true, encounter);
        r.observe_form("a", "Omegawyrm");
        r.exchange(500, 10);
        r.arena(43200000);
        let decoded = Record::decode(&r.encode().unwrap()).unwrap();
        assert_eq!(decoded.counts, [1, 1, 0, 1, 1, 500, 10, 1, 43200000]);
        assert_eq!(decoded.ever_enabled, 7);
        assert_eq!(decoded.lineages["a"].current_species, "Omegawyrm");
        assert_eq!(decoded.report()["status"], "mod_activation_recorded");
    }
    #[test]
    fn corruption_trailing_data_and_old_saves_fail_closed() {
        let r = Record::new([2; 16], 0);
        assert_eq!(r.report()["status"], "prior_history_unknown");
        let mut blob = r.encode().unwrap();
        blob[4] ^= 1;
        assert!(Record::decode(&blob).is_err());
        let mut blob = r.encode().unwrap();
        blob.push(0);
        assert!(Record::decode(&blob).is_err());
        assert_eq!(
            inspect_plist(&plist::Value::Dictionary(Default::default()))["status"],
            "history_missing"
        );
    }
    #[test]
    fn bounded_journal_verifies_after_rollover() {
        let mut r = Record::new([3; 16], 1);
        for _ in 0..1000 {
            r.draw("Waterdragon01", false);
        }
        assert_eq!(r.events.len(), JOURNAL);
        r.verify().unwrap();
        Record::decode(&r.encode().unwrap()).unwrap();
        r.events[0].kind = 99;
        assert!(r.verify().is_err());
    }
    #[test]
    fn payload_binding_ignores_dictionary_order_and_self_field() {
        let mut a = plist::Dictionary::new();
        a.insert("z".into(), 1u64.into());
        a.insert("a".into(), 2u64.into());
        let original = plist::Value::Dictionary(a.clone());
        let digest = game_hash(&original).unwrap();
        let mut r = Record::new([4; 16], 1);
        r.game_hash = digest;
        a.insert(FIELD.into(), plist::Value::Data(r.encode().unwrap()));
        let mut value = plist::Value::Dictionary(a);
        assert_eq!(game_hash(&value).unwrap(), digest);
        assert_eq!(
            inspect_plist(&value)["status"],
            "observed_from_fresh_slot_no_mod_activation"
        );
        value
            .as_dictionary_mut()
            .unwrap()
            .insert("z".into(), 3u64.into());
        assert_eq!(inspect_plist(&value)["status"], "save_payload_mismatch");
    }
    #[test]
    fn entered_battles_and_persistent_monster_tags_are_reported_separately() {
        let mut r = Record::new([5; 16], 1);
        let number = r.draw("Earthdragon01", true);
        r.draw("Earthdragon01", true);
        r.enter(number, true);
        let uid = "0123456789abcdef0123456789abcdef";
        r.capture(uid, "Earthdragon01", true, number);
        r.observe_form(uid, "Earthdragon02");
        let mut monster = plist::Dictionary::new();
        monster.insert(
            MONSTER_FIELD.into(),
            plist::Value::Data(uid.as_bytes().to_vec()),
        );
        let mut root = plist::Dictionary::new();
        root.insert(
            "playerData".into(),
            plist::Value::Array(vec![plist::Value::Dictionary(monster)]),
        );
        let mut root = plist::Value::Dictionary(root);
        r.game_hash = game_hash(&root).unwrap();
        root.as_dictionary_mut()
            .unwrap()
            .insert(FIELD.into(), plist::Value::Data(r.encode().unwrap()));
        let report = inspect_plist(&root);
        assert_eq!(report["counts"]["generated_encounters"], 2);
        assert_eq!(report["counts"]["entered_affected_encounters"], 1);
        assert_eq!(report["owned_lineage_ids"][0], uid);
        assert_eq!(report["lineages"][0]["current_species"], "Earthdragon02");
        assert_eq!(report["lineages"][0]["original_species"], "Earthdragon01");
    }
    #[test]
    fn testing_modes_roundtrip_and_legacy_histories_still_decode() {
        let mut r=Record::new([7;16],1);
        r.test_profile=true; r.test_profile_modes=2; r.activate(7);
        let decoded=Record::decode(&r.encode().unwrap()).unwrap();
        assert_eq!(decoded.test_profile_modes,2);
        assert_eq!(decoded.report()["test_profile_modes"]["level_one"],true);
        assert_eq!(decoded.ever_enabled,7);
        r.test_profile_modes=0; // preview.6 records lack the optional mode field.
        let mut json=serde_json::to_value(&r).unwrap();json.as_object_mut().unwrap().remove("23");
        let old:Record=serde_json::from_value(json).unwrap();old.verify().unwrap();
        assert!(old.test_profile);assert_eq!(old.test_profile_modes,0);
        r.test_profile_modes=4;
        let triple=Record::decode(&r.encode().unwrap()).unwrap();
        assert_eq!(triple.report()["test_profile_modes"]["triple_level99"],true);
        r.test_profile_modes=8;assert!(r.verify().is_err());
    }
}
