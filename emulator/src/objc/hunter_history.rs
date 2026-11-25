/* SPDX-License-Identifier: MPL-2.0 */
//! Hunter Island 2.02 save provenance adapters. Everything remains local.
use super::{id, msg, nil, release};
use crate::environment::ThreadId;
use crate::frameworks::foundation::{ns_data, ns_string};
use crate::preservation::{self, Record};
use crate::test_profile::Mode;
use crate::Environment;
use plist::Value;
use std::collections::HashMap;
use std::time::Instant;

const INIT: u32 = 0xdc761;
const NEW_PLAYER: u32 = 0x7b4e1;
const LOAD: u32 = 0xdccc1;
const SAVE: u32 = 0xdd7cd;
const READ: u32 = 0x163c49;
const WRITE: u32 = 0x163a2d;
const CAPTURE: u32 = 0x849d1;
const EVOLVE: u32 = 0xbbe01;
const SERIALIZE: u32 = 0x800dd;
const RESTORE: u32 = 0x7fe99;
const ARGUMENTS: u32 = 0x91391;
const ENCOUNTER: u32 = 0x9607d;
const FACTORY: u32 = 0xbb509;
const PICK_INGREDIENT: u32 = 0x1a0045;
const PICK_INGREDIENT_BASE: u32 = 0xfe0e9;
const SPIRIT_XP: u32 = 0xdfe81;
// The native pickup methods keep their item receiver in r10 (MonsterJoin)
// or r4 (SpiritTotem) at these exact calls in the supported 2.02 binary.
const JOIN_BENCH_RETURN: u32 = 0xb1c07;
const TOTEM_XP_RETURN: u32 = 0xb2c39;
const ADD_BENCH: u32 = 0x7cb95;
const FUSION_ADD_RETURN: u32 = 0x1a034d;
const FUSION_BASE_ADD_RETURN: u32 = 0xfe383;
const CAPTURE_CLONE_RETURN: u32 = 0x96b45;
const ENCOUNTER_FIELD: &str = "_r9";

#[derive(Clone, Debug)]
pub(super) struct Evidence {
    pub number: u64,
    pub species: String,
    pub selected: bool,
}
struct Frame {
    thread: ThreadId,
    write: bool,
    pending: Option<(Record, [u8; 32])>,
}

#[derive(Default)]
pub(crate) struct State {
    fresh_catalog: Vec<String>,
    initial_collection: bool,
    triple_collection_pending: bool,
    compatible: Option<bool>,
    pub(super) record: Option<Record>,
    slot: Option<u32>,
    loading: HashMap<ThreadId, u32>,
    building: HashMap<ThreadId, u32>,
    fusion_nodes: HashMap<ThreadId, Vec<(id, bool)>>,
    frames: Vec<Frame>,
    pub(super) monsters: HashMap<id, String>,
    encounters: HashMap<id, Evidence>,
    current_encounter: Option<Evidence>,
    last_saved: [u8; 32],
    last_tick: Option<Instant>,
}
impl State {
    pub(crate) fn forget(&mut self, object: id) {
        self.monsters.remove(&object);
        self.encounters.remove(&object);
    }
    fn tick(&mut self) {
        let now = Instant::now();
        if let (Some(before), Some(r)) = (self.last_tick, self.record.as_mut()) {
            r.observed_milliseconds =
                r.observed_milliseconds.saturating_add(
                    now.duration_since(before).as_millis().min(u64::MAX as u128) as u64,
                );
        }
        self.last_tick = Some(now);
    }
    fn begin(&mut self, slot: u32, flags: u32) {
        self.initial_collection = false;
        self.triple_collection_pending = false;
        self.slot = Some(slot);
        self.record = Some(Record::new(*uuid::Uuid::new_v4().as_bytes(), 1));
        self.last_saved = [0; 32];
        self.monsters.clear();
        self.encounters.clear();
        self.current_encounter = None;
        self.last_tick = Some(Instant::now());
        self.record.as_mut().unwrap().activate(flags);
    }
}

pub(crate) fn enabled(env: &Environment) -> u32 {
    u32::from(env.options.hunter_rare_dragons)
        | u32::from(env.options.hunter_silver_exchange) << 1
        | u32::from(env.options.hunter_no_arena_cooldown) << 2
        | u32::from(env.options.hunter_gravestone_codes) << 3
}
fn valid_game(env: &Environment) -> bool {
    env.bundle.bundle_identifier() == "pockettrend.hunterisland"
        && env.bundle.bundle_version() == "2.02"
}
fn compatible(env: &mut Environment) -> bool {
    if let Some(yes) = env.objc.hunter_history.compatible {
        return yes;
    }
    // Identify the exact method layout before interpreting any guest arguments.
    let addresses = [
        0xdc760, 0xdccc0, 0xdd7cc, 0x163c48, 0x163a2c, 0x849d0, 0xbbe00, 0x800dc, 0x7fe98, 0x91390,
        0x9607c, 0xbb508, 0x96b28, 0x1a0044, 0x1a0338, 0xfe0e8, 0xfe36e, 0x7cb94,
    ];
    let mut code = Vec::new();
    let mut yes = valid_game(env);
    if yes {
        for address in addresses {
            match env
                .mem
                .get_bytes_fallible(crate::mem::Ptr::from_bits(address), 32)
            {
                Some(bytes) => code.extend_from_slice(bytes),
                None => {
                    yes = false;
                    break;
                }
            }
        }
        yes &= preservation::hash(&code)
            == [
                154, 177, 62, 30, 92, 183, 54, 87, 234, 106, 80, 248, 246, 170, 165, 22, 5, 174,
                131, 16, 152, 176, 212, 122, 213, 127, 103, 15, 163, 66, 245, 184,
            ];
    }
    env.objc.hunter_history.compatible = Some(yes);
    yes
}
pub(crate) fn configure_fresh_collection(env: &mut Environment, catalog: Vec<String>) -> Result<(),String> {
    if !compatible(env) || env.mem.get_bytes_fallible(crate::mem::Ptr::from_bits(0x7b4e0),4) != Some(&[0x70,0x47,0x00,0xbf][..]) {
        return Err("Fresh testing collections require the supported Hunter Island 2.02 executable layout.".into());
    }
    env.objc.hunter_history.fresh_catalog=catalog;
    Ok(())
}
fn save_slot(path: &str) -> Option<u32> {
    let parts: Vec<_> = path.split('/').collect();
    let n = parts.len();
    if n < 3 || parts[n - 3] != "Documents" {
        return None;
    }
    let slot = parts[n - 2].strip_prefix("saves")?;
    let index = parts[n - 1].strip_prefix("sv_")?;
    if slot.is_empty()
        || index.is_empty()
        || !slot.bytes().all(|b| b.is_ascii_digit())
        || !index.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    slot.parse().ok()
}
fn is_save(root: &Value) -> bool {
    root.as_dictionary().is_some_and(|d| {
        // A fresh tutorial checkpoint legitimately has no flags dictionary.
        // The verified Saving frame and slot still scope this to game saves.
        ["props", "playerData", "silver", "goldFound", "goldSpent", "adw"]
            .iter()
            .all(|k| d.contains_key(*k))
    })
}
fn key(env: &mut Environment, name: &str) -> id {
    ns_string::from_rust_string(env, name.into())
}
fn get(env: &mut Environment, object: id, name: &str) -> id {
    if object == nil {
        return nil;
    }
    let k = key(env, name);
    let value: id = msg![env;object objectForKey:k];
    release(env, k);
    value
}
fn set_bytes(env: &mut Environment, object: id, name: &str, bytes: &[u8]) {
    let p = env.mem.alloc(bytes.len() as u32);
    env.mem
        .bytes_at_mut(p.cast(), bytes.len() as u32)
        .copy_from_slice(bytes);
    let data: id =
        crate::objc::msg_class![env;NSData dataWithBytesNoCopy:p length:(bytes.len() as u32)];
    let k = key(env, name);
    () = msg![env;object setObject:data forKey:k];
    release(env, k);
}
fn bytes(env: &mut Environment, object: id) -> Option<Vec<u8>> {
    if object == nil {
        return None;
    }
    let class = env.objc.get_known_class("NSData", &mut env.mem);
    let yes: bool = msg![env;object isKindOfClass:class];
    if !yes {
        return None;
    }
    let length: u32 = msg![env;object length];
    if length == 0 || length > 2 * 1024 * 1024 {
        return None;
    }
    Some(ns_data::to_rust_slice(env, object).to_vec())
}
pub(super) fn monster_species(env: &mut Environment, monster: id) -> Option<String> {
    if monster == nil
        || env
            .objc
            .try_get_class_name(super::ObjC::read_isa(monster, &env.mem))
            != Some("Monster")
    {
        return None;
    }
    let name: id = msg![env;monster name];
    if name == nil {
        return None;
    }
    let name = ns_string::to_rust_string(env, name).into_owned();
    Some(name.rsplit('.').next().unwrap_or(&name).to_owned())
}
fn starter(species: &str) -> bool {
    matches!(
        species,
        "Earthdragon01" | "Firedragon01" | "Waterdragon01" | "Winddragon01"
    )
}

fn promote_initial_monster(env: &mut Environment, monster: id) {
    let tier: i32=msg![env;monster tier];
    assert!((0..=crate::test_profile::MONSTER_TIER).contains(&tier));
    for _ in tier..crate::test_profile::MONSTER_TIER {
        ()=msg![env;monster increaseTier];
    }
}

fn test_mode(env: &Environment) -> Option<Mode> {
    Mode::from_options(&env.options).expect("Conflicting testing collections")
}

fn grant_fresh_collection(env: &mut Environment, player: id, mode: Mode) {
    let catalog = env.objc.hunter_history.fresh_catalog.clone();
    assert!(!catalog.is_empty(), "Missing fresh testing catalog");
    let bench: id = msg![env;player monstersOnBench];
    // This array contains the full owned collection, including the lineup.
    // Count the original tutorial starter instead of granting a fourth copy.
    let count: u32 = msg![env;bench count];
    assert!(count <= 2000, "Unexpected initial collection size");
    let mut owned = HashMap::<String, u32>::new();
    for index in 0..count {
        let monster: id = msg![env;bench objectAtIndex:index];
        let species = monster_species(env, monster).expect("Invalid starting monster");
        assert!(catalog.contains(&species), "Unknown starting monster");
        *owned.entry(species).or_default() += 1;
        promote_initial_monster(env, monster);
        if mode == Mode::Triple99 {
            // Use the original job's level-up operation so the starter's
            // stats, abilities and saved XP remain consistent with its level.
            let jobs: id = msg![env;monster jobs];
            let job: id = msg![env;jobs objectAtIndex:0u32];
            let level: i32 = msg![env;job level];
            assert!((1..=99).contains(&level), "Invalid starting level");
            for _ in level..99 {
                () = msg![env;job increaseLevel];
            }
            let level: i32 = msg![env;monster level];
            assert_eq!(level, 99, "Cannot initialize level-99 starter");
        }
    }
    let save: id = crate::objc::msg_class![env;UserSavedData sharedInstance];
    let mut highest: id = msg![env;save highestLevelsByMonsterType];
    if highest == nil {
        highest = crate::objc::msg_class![env;NSMutableDictionary dictionary];
        let name = ns_string::from_rust_string(env,"highestLevelMonsters".into());
        () = msg![env;save setPropertyValue:highest forKey:name];
        release(env,name);
    }
    let level = mode.level() as i32;
    let number: id = crate::objc::msg_class![env;NSNumber numberWithInt:level];
    for short in &catalog {
        let present = owned.get(short).copied().unwrap_or(0);
        assert!(present <= mode.copies(), "Too many initial monster copies");
        let name = ns_string::from_rust_string(env, short.clone());
        for _ in present..mode.copies() {
            let monster: id = crate::objc::msg_class![env;Monster monsterOfShortType:name level:level];
            assert_ne!(monster, nil, "Cannot create testing monster");
            promote_initial_monster(env, monster);
            let actual: i32 = msg![env;monster level];
            assert_eq!(actual, level, "Incorrect testing monster level");
            // Direct storage avoids capture/reward dialogs during the intro.
            () = msg![env;bench addObject:monster];
        }
        () = msg![env;highest setObject:number forKey:name];
        release(env, name);
    }
    () = msg![env;save setSaveFileDirty];
    log!("Created fresh testing collection: {} forms, {} copies each, level {}, S rarity; no bonus resources.", catalog.len(), mode.copies(), level);
}

pub(super) struct Hook {
    address: u32,
    receiver: id,
    args: [u32; 2],
    frame: bool,
    source: Option<String>,
    ingredients: Option<Vec<(String, Option<String>)>>,
    code_join: bool,
}

pub(super) fn interesting(address: u32) -> bool {
    matches!(
        address,
        INIT | NEW_PLAYER | LOAD
            | SAVE
            | READ
            | WRITE
            | CAPTURE
            | EVOLVE
            | SERIALIZE
            | RESTORE
            | ARGUMENTS
            | ENCOUNTER
            | FACTORY
            | PICK_INGREDIENT
            | PICK_INGREDIENT_BASE
            | ADD_BENCH
            | SPIRIT_XP
    )
}

fn code_pickup(env: &mut Environment, register: usize, alias: &str) -> bool {
    if !env.options.hunter_gravestone_codes { return false; }
    let item = id::from_bits(env.cpu.regs()[register]);
    let name: id = msg![env;item name];
    if name == nil { return false; }
    let name = ns_string::to_rust_string(env,name);
    if env.options.capture_frames { log!("Gravestone pickup: {name}"); }
    name == alias || Some(&*name) == alias.rsplit('.').next()
}

pub(super) fn before(env: &mut Environment, receiver: id, address: u32) -> Option<Hook> {
    if !interesting(address) || !compatible(env) {
        return None;
    }
    let args = [env.cpu.regs()[2], env.cpu.regs()[3]];
    let mut hook = Hook {
        address,
        receiver,
        args,
        frame: false,
        source: None,
        ingredients: None,
        code_join: false,
    };
    let flags = enabled(env);
    env.objc.hunter_history.tick();
    match address {
        INIT => {
            env.objc.hunter_history.begin(args[0], flags);
            if env.options.capture_frames {log!("SAVE_HISTORY_INIT slot={}",args[0]);}
        }
        LOAD => {
            env.objc.hunter_history.initial_collection = false;
            env.objc.hunter_history.triple_collection_pending = false;
            *env.objc
                .hunter_history
                .loading
                .entry(env.current_thread)
                .or_default() += 1
        }
        SAVE => {
            if env.objc.hunter_history.triple_collection_pending {
                assert_eq!(test_mode(env), Some(Mode::Triple99));
                let player: id = crate::objc::msg_class![env;Player sharedInstance];
                grant_fresh_collection(env, player, Mode::Triple99);
                env.objc.hunter_history.triple_collection_pending = false;
            }
            *env.objc
                .hunter_history
                .building
                .entry(env.current_thread)
                .or_default() += 1
        }
        SERIALIZE
            if test_mode(env).is_some_and(Mode::fresh)
                && env.objc.hunter_history.initial_collection
                && env.objc.hunter_history.building.get(&env.current_thread).copied().unwrap_or(0) > 0
                && monster_species(env, receiver).is_some() =>
        {
            // The intro sets its original starter's tier after adding it to
            // the party. Finalize only this new campaign's starting collection
            // before its first checkpoint; later saves never promote units.
            promote_initial_monster(env, receiver);
        }
        READ | WRITE => {
            let path_id = id::from_bits(if address == READ { args[0] } else { args[1] });
            if path_id != nil {
                let path = ns_string::to_rust_string(env, path_id).into_owned();
                if env.options.capture_frames && test_mode(env).is_some_and(Mode::fresh) {log!("SAVE_HISTORY_IO write={} slot={:?} path={path}",address==WRITE,env.objc.hunter_history.slot);}
                if let Some(slot) = save_slot(&path) {
                    let s = &mut env.objc.hunter_history;
                    if s.slot == Some(slot)
                        && (address == WRITE
                            || s.loading.get(&env.current_thread).copied().unwrap_or(0) > 0)
                    {
                        s.frames.push(Frame {
                            thread: env.current_thread,
                            write: address == WRITE,
                            pending: None,
                        });
                        hook.frame = true;
                    }
                }
            }
        }
        EVOLVE => hook.source = env.objc.hunter_history.monsters.get(&receiver).cloned(),
        PICK_INGREDIENT | PICK_INGREDIENT_BASE => {
            env.objc
                .hunter_history
                .fusion_nodes
                .entry(env.current_thread)
                .or_default()
                .push((receiver, address == PICK_INGREDIENT));
        }
        SPIRIT_XP if env.cpu.regs()[crate::cpu::Cpu::LR] == TOTEM_XP_RETURN
            && code_pickup(env, 4, crate::gravestone::TOTEM) => {
            let short = ns_string::to_rust_string(env,id::from_bits(args[1])).into_owned();
            if let Some(record) = env.objc.hunter_history.record.as_mut() {
                let uid = record.code_spirits.get(&short).cloned()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
                record.redeem(&uid, &short, true);
            }
        }
        ADD_BENCH => {
            let lr = env.cpu.regs()[crate::cpu::Cpu::LR];
            hook.code_join = lr == JOIN_BENCH_RETURN && code_pickup(env, 10, crate::gravestone::JOIN);
            if let Some(&(node, hi)) = env
                .objc
                .hunter_history
                .fusion_nodes
                .get(&env.current_thread)
                .and_then(|v| v.last())
            {
                if (hi && lr == FUSION_ADD_RETURN) || (!hi && lr == FUSION_BASE_ADD_RETURN) {
                    let list: id = if hi {
                        msg![env;node ingredientsPicked]
                    } else {
                        let offset: u32 = env.mem.read(crate::mem::ConstPtr::from_bits(0x3d9854));
                        let address = node.to_bits().checked_add(offset)?;
                        let data = env
                            .mem
                            .get_bytes_fallible(crate::mem::Ptr::from_bits(address), 4)?;
                        id::from_bits(u32::from_le_bytes(data.try_into().unwrap()))
                    };
                    if list != nil {
                        let count: u32 = msg![env;list count];
                        if count > 0 && count <= 64 {
                            let mut ingredients = Vec::new();
                            for index in 0..count {
                                let monster: id = msg![env;list objectAtIndex:index];
                                let species = monster_species(env, monster)?;
                                let uid = env.objc.hunter_history.monsters.get(&monster).cloned();
                                ingredients.push((species, uid));
                            }
                            if ingredients.iter().any(|(_, uid)| uid.is_some()) {
                                hook.ingredients = Some(ingredients);
                            }
                        }
                    }
                }
            }
        }
        FACTORY if env.cpu.regs()[crate::cpu::Cpu::LR] == CAPTURE_CLONE_RETURN => {
            // Encounter.startCombat recreates captured monsters through this
            // exact factory call before adding them to the player's bench.
            // r10 holds the original captured object at this verified call site.
            let source = id::from_bits(env.cpu.regs()[10]);
            hook.source = env.objc.hunter_history.monsters.get(&source).cloned();
        }
        ENCOUNTER => {
            let dict = id::from_bits(args[0]);
            let data = get(env, dict, ENCOUNTER_FIELD);
            let tag = bytes(env, data);
            env.objc.hunter_history.current_encounter = tag.and_then(|b| {
                let value: Value = Value::from_reader(std::io::Cursor::new(b)).ok()?;
                let a = value.as_array()?;
                if a.len() != 3 {
                    return None;
                }
                Some(Evidence {
                    number: a[0].as_unsigned_integer()?,
                    species: a[1].as_string()?.into(),
                    selected: a[2].as_boolean()?,
                })
            });
        }
        _ => (),
    }
    Some(hook)
}

pub(super) fn after(env: &mut Environment, hook: Hook) {
    let result = id::from_bits(env.cpu.regs()[0]);
    match hook.address {
        NEW_PLAYER if test_mode(env).is_some_and(Mode::fresh) => {
            let mode = test_mode(env).unwrap();
            if let Some(record) = env.objc.hunter_history.record.as_mut() {
                record.test_profile = true;
                record.test_profile_modes |= mode.bit();
            }
            if mode == Mode::LevelOne {
                grant_fresh_collection(env, hook.receiver, mode);
            } else {
                // Wait until the intro has added its original starter. Fill
                // only the missing copies before the first checkpoint.
                env.objc.hunter_history.triple_collection_pending = true;
            }
            env.objc.hunter_history.initial_collection = true;
        }
        PICK_INGREDIENT | PICK_INGREDIENT_BASE => {
            if let Some(nodes) = env
                .objc
                .hunter_history
                .fusion_nodes
                .get_mut(&env.current_thread)
            {
                nodes.pop();
            }
        }
        ADD_BENCH if hook.code_join => {
            let monster = id::from_bits(hook.args[0]);
            let bench: id = msg![env;(hook.receiver) monstersOnBench];
            let accepted: bool = msg![env;bench containsObject:monster];
            if let Some(short) = monster_species(env, monster).filter(|_| accepted) {
                let s = &mut env.objc.hunter_history;
                if let Some(record) = s.record.as_mut() {
                    let uid = uuid::Uuid::new_v4().simple().to_string();
                    record.redeem(&uid, &short, false);
                    s.monsters.insert(monster, uid);
                }
            }
        }
        ADD_BENCH => {
            if let Some(ingredients) = hook.ingredients {
                let monster = id::from_bits(hook.args[0]);
                if let Some(species) = monster_species(env, monster) {
                    let bench: id = msg![env;(hook.receiver) monstersOnBench];
                    let accepted: bool = msg![env;bench containsObject:monster];
                    if accepted {
                        let s = &mut env.objc.hunter_history;
                        let uid = s
                            .monsters
                            .get(&monster)
                            .cloned()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
                        if s.record
                            .as_mut()
                            .is_some_and(|r| r.fuse(&uid, &species, &ingredients))
                        {
                            s.monsters.insert(monster, uid);
                            if env.options.capture_frames {
                                log!("SAVE_HISTORY_FUSION object={monster:?} species={species}");
                            }
                        }
                    }
                }
            }
        }
        LOAD => {
            if let Some(depth) = env.objc.hunter_history.loading.get_mut(&env.current_thread) {
                *depth = depth.saturating_sub(1);
            }
        }
        SAVE => {
            if let Some(depth) = env
                .objc
                .hunter_history
                .building
                .get_mut(&env.current_thread)
            {
                *depth = depth.saturating_sub(1);
            }
        }
        READ | WRITE if hook.frame => {
            let s = &mut env.objc.hunter_history;
            if let Some(index) = s
                .frames
                .iter()
                .rposition(|f| f.thread == env.current_thread)
            {
                let frame = s.frames.remove(index);
                if frame.write && result != nil {
                    s.initial_collection = false;
                    if let Some((staged, digest)) = frame.pending {
                        if let Some(r) = s.record.as_mut().filter(|r| r.id == staged.id) {
                            s.last_saved = digest;
                            r.save_sequence = staged.save_sequence;
                            r.game_hash = staged.game_hash;
                            r.previous_save = staged.previous_save;
                            r.incomplete |= staged.incomplete;
                            if staged.origin == 2 {
                                r.origin = 2;
                            }
                        }
                    } else if let Some(r) = s.record.as_mut() {
                        r.incomplete = true;
                    }
                }
            }
        }
        CAPTURE => {
            let monster = id::from_bits(hook.args[0]);
            if let Some(species) = monster_species(env, monster).filter(|s| starter(s)) {
                let active = env.options.hunter_rare_dragons;
                let evidence = env
                    .objc
                    .hunter_history
                    .current_encounter
                    .as_ref()
                    .filter(|e| e.selected && e.species == species)
                    .cloned();
                if active || evidence.is_some() {
                    if env.options.capture_frames {
                        log!("SAVE_HISTORY_CAPTURE object={monster:?} species={species}");
                    }
                    let s = &mut env.objc.hunter_history;
                    let uid = s
                        .monsters
                        .entry(monster)
                        .or_insert_with(|| uuid::Uuid::new_v4().simple().to_string())
                        .clone();
                    if let Some(r) = s.record.as_mut() {
                        r.capture(
                            &uid,
                            &species,
                            active,
                            evidence.map(|e| e.number).unwrap_or(0),
                        );
                    }
                }
            }
        }
        EVOLVE | FACTORY if result != nil => {
            if let Some(source) = hook.source {
                env.objc.hunter_history.monsters.insert(result, source);
            }
        }
        SERIALIZE if result != nil => {
            if env.options.capture_frames {
                if let Some(species) = monster_species(env, hook.receiver).filter(|s| starter(s)) {
                    log!(
                        "SAVE_HISTORY_SERIALIZE object={:?} species={} tagged={}",
                        hook.receiver,
                        species,
                        env.objc
                            .hunter_history
                            .monsters
                            .contains_key(&hook.receiver)
                    );
                }
            }
            if let Some(uid) = env
                .objc
                .hunter_history
                .monsters
                .get(&hook.receiver)
                .cloned()
            {
                if let Some(species) = monster_species(env, hook.receiver) {
                    set_bytes(env, result, preservation::MONSTER_FIELD, uid.as_bytes());
                    if env
                        .objc
                        .hunter_history
                        .building
                        .get(&env.current_thread)
                        .copied()
                        .unwrap_or(0)
                        > 0
                    {
                        if let Some(r) = env.objc.hunter_history.record.as_mut() {
                            r.observe_form(&uid, &species);
                        }
                    }
                }
            }
        }
        RESTORE if result != nil => {
            let data = get(
                env,
                id::from_bits(hook.args[0]),
                preservation::MONSTER_FIELD,
            );
            if let Some(data) = bytes(env, data) {
                if data.len() == 32 && data.iter().all(|b| b.is_ascii_hexdigit()) {
                    if monster_species(env, result).is_some() {
                        env.objc
                            .hunter_history
                            .monsters
                            .insert(result, String::from_utf8(data).unwrap());
                    }
                }
            }
        }
        ARGUMENTS if result != nil => {
            if let Some(e) = env
                .objc
                .hunter_history
                .encounters
                .get(&hook.receiver)
                .cloned()
            {
                let value =
                    Value::Array(vec![e.number.into(), e.species.into(), e.selected.into()]);
                let mut data = Vec::new();
                if value.to_writer_binary(&mut data).is_ok() {
                    set_bytes(env, result, ENCOUNTER_FIELD, &data);
                }
            }
        }
        ENCOUNTER if result != nil => {
            let s = &mut env.objc.hunter_history;
            if let (Some(r), Some(e)) = (s.record.as_mut(), s.current_encounter.as_ref()) {
                r.enter(e.number, e.selected);
            }
        }
        _ => (),
    }
}

pub(crate) fn read_plist(env: &mut Environment, root: &Value) {
    if !is_save(root)
        || !env
            .objc
            .hunter_history
            .frames
            .iter()
            .rev()
            .find(|f| f.thread == env.current_thread)
            .is_some_and(|f| !f.write)
    {
        return;
    }
    let flags = enabled(env);
    let data = root
        .as_dictionary()
        .unwrap()
        .get(preservation::FIELD)
        .and_then(Value::as_data);
    let mut record = match data {
        None => Record::new(*uuid::Uuid::new_v4().as_bytes(), 0),
        Some(blob) => match Record::decode(blob) {
            Ok(mut r) => {
                if preservation::game_hash(root).ok() != Some(r.game_hash) {
                    r.origin = 2;
                    r.incomplete = true;
                }
                r
            }
            Err(_) => Record::new(*uuid::Uuid::new_v4().as_bytes(), 2),
        },
    };
    record.activate(flags);
    record.test_profile |= env.options.hunter_test_save || env.options.hunter_level1_save || env.options.hunter_triple99_save;
    record.test_profile_modes |= u32::from(env.options.hunter_test_save) | u32::from(env.options.hunter_level1_save)<<1 | u32::from(env.options.hunter_triple99_save)<<2;
    let s = &mut env.objc.hunter_history;
    s.initial_collection = false;
    s.triple_collection_pending = false;
    s.record = Some(record);
    s.last_saved = data.map(preservation::hash).unwrap_or([0; 32]);
    s.last_tick = Some(Instant::now());
}

pub(crate) fn write_plist(env: &mut Environment, root: &mut Value) -> Result<(), String> {
    if env.options.capture_frames && test_mode(env).is_some_and(Mode::fresh) && is_save(root) { log!("SAVE_HISTORY_WRITE_PLIST frames={} thread={:?}",env.objc.hunter_history.frames.len(),env.current_thread); }
    if !is_save(root)
        || !env
            .objc
            .hunter_history
            .frames
            .iter()
            .rev()
            .find(|f| f.thread == env.current_thread)
            .is_some_and(|f| f.write)
    {
        return Ok(());
    }
    let flags = enabled(env);
    let s = &mut env.objc.hunter_history;
    s.tick();
    let Some(record) = s.record.as_mut() else {
        return Err("Save provenance was not initialized".into());
    };
    record.activate(flags);
    record.test_profile |= env.options.hunter_test_save || env.options.hunter_level1_save || env.options.hunter_triple99_save;
    record.test_profile_modes |= u32::from(env.options.hunter_test_save) | u32::from(env.options.hunter_level1_save)<<1 | u32::from(env.options.hunter_triple99_save)<<2;
    let mut staged = record.clone();
    staged.save_sequence = staged
        .save_sequence
        .checked_add(1)
        .ok_or("Save sequence overflow")?;
    staged.previous_save = s.last_saved;
    staged.tag_spirits(root);
    staged.game_hash = preservation::game_hash(root)?;
    let blob = match staged.encode() {
        Ok(blob) => blob,
        Err(_) => {
            // Retain aggregate usage and a permanent incomplete marker rather
            // than allowing an oversized diagnostic history to block gameplay.
            staged.incomplete = true;
            staged.origin = 2;
            staged.events.clear();
            staged.base_event = staged.event_sequence;
            staged.base_hash = staged.head;
            staged.lineages.clear();
            staged.spawned_species.clear();
            staged.captured_species.clear();
            staged.encode()?
        }
    };
    let digest = preservation::hash(&blob);
    root.as_dictionary_mut()
        .unwrap()
        .insert(preservation::FIELD.into(), Value::Data(blob));
    s.frames
        .iter_mut()
        .rev()
        .find(|f| f.thread == env.current_thread)
        .unwrap()
        .pending = Some((staged, digest));
    Ok(())
}

pub(super) fn draw(env: &mut Environment, encounter: id, species: &str, selected: bool) {
    if env.objc.hunter_history.compatible != Some(true) {
        return;
    }
    let s = &mut env.objc.hunter_history;
    s.tick();
    if let Some(r) = s.record.as_mut() {
        let number = r.draw(species, selected);
        s.encounters.insert(
            encounter,
            Evidence {
                number,
                species: species.into(),
                selected,
            },
        );
    }
}
pub(super) fn exchange_snapshot(env: &mut Environment, silver: u64, gold: u64) -> Option<Record> {
    if env.objc.hunter_history.compatible != Some(true) {
        return None;
    }
    let s = &mut env.objc.hunter_history;
    s.tick();
    let before = s.record.clone();
    if let Some(r) = s.record.as_mut() {
        r.exchange(silver, gold);
    }
    before
}
pub(super) fn exchange_rollback(env: &mut Environment, before: Option<Record>) {
    env.objc.hunter_history.record = before;
}
pub(super) fn arena(env: &mut Environment, seconds: f64) {
    if !seconds.is_finite() || seconds <= 0.0 || env.objc.hunter_history.compatible != Some(true) {
        return;
    }
    let s = &mut env.objc.hunter_history;
    s.tick();
    if let Some(r) = s.record.as_mut() {
        r.arena((seconds * 1000.0).round().min(u64::MAX as f64) as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_bench_adds_and_unrelated_call_sites_never_create_fusion_history() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 0);
        env.cpu.regs_mut()[crate::cpu::Cpu::LR] = FUSION_ADD_RETURN;
        assert!(before(&mut env, nil, ADD_BENCH)
            .unwrap()
            .ingredients
            .is_none());
        let hook = before(&mut env, nil, PICK_INGREDIENT).unwrap();
        env.cpu.regs_mut()[crate::cpu::Cpu::LR] = 0;
        assert!(before(&mut env, nil, ADD_BENCH)
            .unwrap()
            .ingredients
            .is_none());
        after(&mut env, hook);
        assert!(env.objc.hunter_history.fusion_nodes[&env.current_thread].is_empty());
        assert_eq!(
            env.objc
                .hunter_history
                .record
                .as_ref()
                .unwrap()
                .marked_fusions,
            0
        );
    }
    #[test]
    fn capture_to_owned_clone_preserves_identity_only_at_the_verified_call_site() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 0);
        let original = id::from_bits(0x1000);
        let owned = id::from_bits(0x2000);
        env.objc
            .hunter_history
            .monsters
            .insert(original, "0123456789abcdef0123456789abcdef".into());
        env.cpu.regs_mut()[10] = original.to_bits();
        env.cpu.regs_mut()[crate::cpu::Cpu::LR] = CAPTURE_CLONE_RETURN;
        let hook = before(&mut env, nil, FACTORY).unwrap();
        env.cpu.regs_mut()[0] = owned.to_bits();
        after(&mut env, hook);
        assert_eq!(
            env.objc.hunter_history.monsters[&original],
            env.objc.hunter_history.monsters[&owned]
        );
        env.cpu.regs_mut()[crate::cpu::Cpu::LR] = 0;
        let unrelated = before(&mut env, nil, FACTORY).unwrap();
        assert!(unrelated.source.is_none());
        let evolved = id::from_bits(0x3000);
        let hook = before(&mut env, owned, EVOLVE).unwrap();
        env.cpu.regs_mut()[0] = evolved.to_bits();
        after(&mut env, hook);
        assert_eq!(
            env.objc.hunter_history.monsters[&original],
            env.objc.hunter_history.monsters[&evolved]
        );
    }
    #[test]
    fn slots_are_strict_and_never_match_settings_or_traversal() {
        assert_eq!(
            save_slot("/var/mobile/Applications/id/Documents/saves0/sv_123"),
            Some(0)
        );
        for p in [
            "/Documents/settings",
            "/Documents/saves0/../sv_2",
            "/Documents/savesx/sv_1",
            "/Documents/saves0/sv_-1",
        ] {
            assert_eq!(save_slot(p), None)
        }
    }
    fn environment() -> Environment {
        let mut env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.objc.hunter_history.compatible = Some(true);
        env
    }
    fn root() -> Value {
        let mut d = plist::Dictionary::new();
        for name in ["props", "playerData", "flags"] {
            d.insert(name.into(), Value::Dictionary(Default::default()));
        }
        for name in ["silver", "goldFound", "goldSpent", "adw"] {
            d.insert(name.into(), 0u64.into());
        }
        Value::Dictionary(d)
    }
    fn frame(env: &mut Environment, write: bool) {
        env.objc.hunter_history.frames.push(Frame {
            thread: env.current_thread,
            write,
            pending: None,
        });
    }
    fn complete(env: &mut Environment, success: bool) {
        env.cpu.regs_mut()[0] = u32::from(success);
        after(
            env,
            Hook {
                address: WRITE,
                receiver: nil,
                args: [0, 0],
                frame: true,
                source: None,
                ingredients: None,
                code_join: false,
            },
        );
    }
    #[test]
    fn initial_collection_ends_on_commit_or_load_and_never_survives_slot_selection() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 0);
        env.objc.hunter_history.initial_collection = true;
        let mut saved = root();

        frame(&mut env, true);
        write_plist(&mut env, &mut saved).unwrap();
        complete(&mut env, false);
        assert!(env.objc.hunter_history.initial_collection);

        frame(&mut env, true);
        write_plist(&mut env, &mut saved).unwrap();
        complete(&mut env, true);
        assert!(!env.objc.hunter_history.initial_collection);

        env.objc.hunter_history.initial_collection = true;
        frame(&mut env, false);
        read_plist(&mut env, &saved);
        env.objc.hunter_history.frames.pop();
        assert!(!env.objc.hunter_history.initial_collection);

        env.objc.hunter_history.initial_collection = true;
        before(&mut env, nil, LOAD).unwrap();
        assert!(!env.objc.hunter_history.initial_collection);

        env.objc.hunter_history.initial_collection = true;
        env.objc.hunter_history.begin(1, 0);
        assert!(!env.objc.hunter_history.initial_collection);
    }
    #[test]
    fn fresh_checkpoint_without_flags_keeps_the_level_one_marker_on_reload() {
        let mut env=environment();env.options.hunter_level1_save=true;
        let flags=enabled(&env);env.objc.hunter_history.begin(0,flags);
        let mut saved=root();saved.as_dictionary_mut().unwrap().remove("flags");
        frame(&mut env,true);write_plist(&mut env,&mut saved).unwrap();complete(&mut env,true);
        assert_eq!(preservation::inspect_plist(&saved)["test_profile_modes"]["level_one"],true);
        assert_eq!(preservation::inspect_plist(&saved)["counts"]["hatchlings_captured_while_enabled"],0);
        env.options.hunter_level1_save=false;frame(&mut env,false);read_plist(&mut env,&saved);
        assert!(env.objc.hunter_history.record.as_ref().unwrap().test_profile);
        assert_eq!(env.objc.hunter_history.record.as_ref().unwrap().test_profile_modes,2);
    }
    #[test]
    fn save_commit_failure_reload_and_mod_disable_preserve_consistency() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 0);
        let old = exchange_snapshot(&mut env, 500, 10);
        let before = root();
        let mut proposed = before.clone();
        frame(&mut env, true);
        write_plist(&mut env, &mut proposed).unwrap();
        complete(&mut env, false);
        exchange_rollback(&mut env, old);
        assert_eq!(
            env.objc.hunter_history.record.as_ref().unwrap().counts[5],
            0
        );
        assert_eq!(
            env.objc
                .hunter_history
                .record
                .as_ref()
                .unwrap()
                .save_sequence,
            0
        );
        exchange_snapshot(&mut env, 500, 10);
        let mut committed = root();
        frame(&mut env, true);
        write_plist(&mut env, &mut committed).unwrap();
        complete(&mut env, true);
        assert_eq!(
            env.objc
                .hunter_history
                .record
                .as_ref()
                .unwrap()
                .save_sequence,
            1
        );
        let report = preservation::inspect_plist(&committed);
        assert_eq!(report["counts"]["silver_exchanged"], 500);
        assert_eq!(report["counts"]["gold_received"], 10);
        let digest = env.objc.hunter_history.last_saved;
        env.objc.hunter_history.begin(0, 0);
        frame(&mut env, false);
        read_plist(&mut env, &committed);
        env.objc.hunter_history.frames.pop();
        assert_eq!(env.objc.hunter_history.last_saved, digest);
        assert_eq!(
            env.objc
                .hunter_history
                .record
                .as_ref()
                .unwrap()
                .ever_enabled,
            preservation::EXCHANGE
        );
        frame(&mut env, true);
        write_plist(&mut env, &mut committed).unwrap();
        complete(&mut env, true);
        assert_eq!(
            preservation::inspect_plist(&committed)["counts"]["gold_received"],
            10
        );
        assert_eq!(
            preservation::game_hash(&committed).unwrap(),
            preservation::game_hash(&before).unwrap()
        );
    }
    #[test]
    fn imported_missing_damaged_and_payload_changed_history_are_never_clean() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 0);
        let mut imported = root();
        frame(&mut env, false);
        read_plist(&mut env, &imported);
        env.objc.hunter_history.frames.pop();
        assert_eq!(env.objc.hunter_history.record.as_ref().unwrap().origin, 0);
        frame(&mut env, true);
        write_plist(&mut env, &mut imported).unwrap();
        complete(&mut env, true);
        imported
            .as_dictionary_mut()
            .unwrap()
            .insert("silver".into(), 777u64.into());
        frame(&mut env, false);
        read_plist(&mut env, &imported);
        env.objc.hunter_history.frames.pop();
        assert_eq!(env.objc.hunter_history.record.as_ref().unwrap().origin, 2);
        imported
            .as_dictionary_mut()
            .unwrap()
            .insert(preservation::FIELD.into(), Value::Data(vec![0; 40]));
        frame(&mut env, false);
        read_plist(&mut env, &imported);
        env.objc.hunter_history.frames.pop();
        assert_eq!(env.objc.hunter_history.record.as_ref().unwrap().origin, 2);
    }
    #[test]
    fn frames_follow_the_guest_thread_and_never_commit_another_profile() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 0);
        frame(&mut env, true);
        env.current_thread = 1;
        frame(&mut env, false);
        env.current_thread = 0;
        let mut saved = root();
        write_plist(&mut env, &mut saved).unwrap();
        complete(&mut env, true);
        assert_eq!(env.objc.hunter_history.frames.len(), 1);
        assert_eq!(env.objc.hunter_history.frames[0].thread, 1);
        assert_eq!(
            env.objc
                .hunter_history
                .record
                .as_ref()
                .unwrap()
                .save_sequence,
            1
        );
        frame(&mut env, true);
        write_plist(&mut env, &mut saved).unwrap();
        env.objc.hunter_history.begin(1, 0);
        complete(&mut env, true);
        assert_eq!(
            env.objc
                .hunter_history
                .record
                .as_ref()
                .unwrap()
                .save_sequence,
            0
        );
        assert_eq!(env.objc.hunter_history.last_saved, [0; 32]);
    }
    #[test]
    fn oversized_details_do_not_block_game_saves_or_erase_aggregate_usage() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 0);
        exchange_snapshot(&mut env, 50, 1);
        let r = env.objc.hunter_history.record.as_mut().unwrap();
        for i in 0..33 {
            r.spawned_species.insert(i.to_string(), 1);
        }
        let mut saved = root();
        frame(&mut env, true);
        write_plist(&mut env, &mut saved).unwrap();
        complete(&mut env, true);
        let report = preservation::inspect_plist(&saved);
        assert_eq!(report["status"], "inconsistent_or_incomplete");
        assert_eq!(report["counts"]["silver_exchanged"], 50);
        assert!(env.objc.hunter_history.record.as_ref().unwrap().incomplete);
        assert_eq!(env.objc.hunter_history.record.as_ref().unwrap().origin, 2);
    }
    #[test]
    fn activation_without_actions_and_nonpositive_arena_wait_do_not_count_usage() {
        let mut env = environment();
        env.objc.hunter_history.begin(0, 7);
        for wait in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            arena(&mut env, wait);
        }
        assert_eq!(
            env.objc.hunter_history.record.as_ref().unwrap().counts,
            [0; 9]
        );
        arena(&mut env, 43200.0);
        assert_eq!(
            env.objc.hunter_history.record.as_ref().unwrap().counts[7],
            1
        );
        assert_eq!(
            env.objc.hunter_history.record.as_ref().unwrap().counts[8],
            43200000
        );
    }
}
