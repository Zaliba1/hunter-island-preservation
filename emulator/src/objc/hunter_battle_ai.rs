// SPDX-License-Identifier: MPL-2.0
//! Bridge Hunter Island's native NPC turns to the locally embedded Hardmode V2 policy.
use super::{id, msg, msg_class, nil, release, ObjC};
use crate::frameworks::foundation::ns_string;
use crate::libc::stdlib::arc4random;
use crate::mem::Ptr;
use crate::Environment;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::io::{Cursor, Read};
use std::ptr::NonNull;

// JS_Eval reads a final NUL at input[length].
const SCRIPT: &str = concat!(include_str!("../hunter_ai_model.js"), "\0");
const INFINITE_DURATION: i32 = i32::MAX;
const CATALOG_LIMIT: u64 = 16 * 1024 * 1024;

extern "C" {
    fn hunter_ai_new(script: *const c_char, length: usize) -> *mut c_void;
    fn hunter_ai_call(
        runtime: *mut c_void, method: *const c_char, json: *const c_char, length: usize,
        ok: *mut c_int,
    ) -> *const c_char;
    fn hunter_ai_free_text(runtime: *mut c_void, text: *const c_char);
    fn hunter_ai_free(runtime: *mut c_void);
}

struct Runtime(NonNull<c_void>);

impl Runtime {
    fn new() -> Result<Self, String> {
        NonNull::new(unsafe { hunter_ai_new(SCRIPT.as_ptr().cast(), SCRIPT.len() - 1) })
            .map(Self)
            .ok_or_else(|| "could not load the embedded policy".into())
    }

    fn call(&self, method: &CStr, input: &str) -> Result<String, String> {
        let runtime = self.0.as_ptr();
        let mut ok = 0;
        let reply = unsafe {
            hunter_ai_call(runtime, method.as_ptr(), input.as_ptr().cast(), input.len(), &mut ok)
        };
        if reply.is_null() {
            return Err("the policy returned no result".into());
        }
        let text = unsafe { CStr::from_ptr(reply) }.to_string_lossy().into_owned();
        unsafe { hunter_ai_free_text(runtime, reply) };
        if ok == 0 { Err(text) } else { Ok(text) }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe { hunter_ai_free(self.0.as_ptr()) };
    }
}

#[derive(Default)]
pub(super) struct State {
    compatible: Option<bool>,
    policy: Option<Result<Runtime, String>>,
    fallback_logged: bool,
}

fn bytes(env: &Environment, address: u32, length: u32) -> Option<&[u8]> {
    env.mem.get_bytes_fallible(Ptr::from_bits(address), length)
}

fn word(env: &Environment, address: u32) -> Option<u32> {
    Some(u32::from_le_bytes(bytes(env, address, 4)?.try_into().ok()?))
}

fn ivar(env: &Environment, object: id, offset: u32) -> u32 {
    word(env, object.to_bits() + offset).unwrap_or(0)
}

pub(super) fn compatible(env: &mut Environment) -> bool {
    if let Some(result) = env.objc.hunter_battle_ai.compatible {
        return result;
    }
    let result = env.bundle.bundle_identifier() == "pockettrend.hunterisland"
        && env.bundle.bundle_version() == "2.02"
        && ([0x81240, 0x85974, 0x86544, 0xb5c04, 0xa0c40]
            .into_iter()
            .try_fold(Vec::new(), |mut code, addr| {
                code.extend_from_slice(bytes(env, addr, 32)?);
                Some(code)
            }))
            .is_some_and(|code| crate::preservation::hash(&code) == [
                0x2a, 0x84, 0x96, 0xaf, 0x0e, 0x44, 0x2b, 0x00,
                0x58, 0xa0, 0x70, 0x03, 0xb0, 0x8d, 0x16, 0x0d,
                0x13, 0x68, 0x9d, 0xd1, 0x73, 0x97, 0xce, 0xc1,
                0xe9, 0xda, 0xdb, 0x92, 0xb2, 0x48, 0xe6, 0x71,
            ]);
    env.objc.hunter_battle_ai.compatible = Some(result);
    result
}

fn resource_key(env: &Environment) -> Option<[u8; 16]> {
    // These are the eight CFString fragments used by the original resource loaders.
    let mut hex = Vec::new();
    for address in [
        0xbcf44 + 0x30c4a0, 0xb49c0 + 0x314674, 0xb9a34 + 0x30f7c0, 0xb18e0 + 0x3174d4,
        0x9e4dc + 0x3297c8, 0xadfc8 + 0x31aa4c, 0xac5e8 + 0x31c3cc, 0xa5ca8 + 0x32278c,
    ] {
        let length = word(env, address + 12)?;
        if word(env, address + 4)? != 0x7c8 || !(1..=16).contains(&length) {
            return None;
        }
        hex.extend_from_slice(bytes(env, word(env, address + 8)?, length)?);
    }
    let mut key = [0; 16];
    if hex.len() != 32 {
        return None;
    }
    for (byte, pair) in key.iter_mut().zip(hex.chunks_exact(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(key)
}

fn load_policy(env: &Environment) -> Result<Runtime, String> {
    let key = resource_key(env);
    let mut catalog = Map::new();
    for name in ["Monsters", "Jobs", "Abilities", "AbilityComponents", "Buffs", "BuffComponents", "GameSettings"] {
        let path = env.bundle.bundle_path().join(&format!("{name}.plist"));
        let raw = env.fs.read(path).map_err(|_| format!("could not read {name}"))?;
        let plist = match plist::Value::from_reader(Cursor::new(&raw)) {
            Ok(plist) => plist,
            Err(_) => {
                let key = key.ok_or("could not read the game resource metadata")?;
                let decoded = crate::libc::crypto::aes_transform(1, 1, &key, &[0; 16], &raw)
                    .map_err(|_| format!("could not open {name}"))?;
                let mut xml = Vec::new();
                flate2::read::GzDecoder::new(decoded.as_slice()).take(CATALOG_LIMIT + 1)
                    .read_to_end(&mut xml).map_err(|_| format!("could not decompress {name}"))?;
                if xml.len() as u64 > CATALOG_LIMIT {
                    return Err(format!("resource exceeds its limit: {name}"));
                }
                plist::Value::from_reader(Cursor::new(xml)).map_err(|err| format!("{name}: {err}"))?
            }
        };
        catalog.insert(name.into(), serde_json::to_value(plist).map_err(|err| err.to_string())?);
    }
    let policy = Runtime::new()?;
    policy.call(CStr::from_bytes_with_nul(b"hunterAiLoad\0").unwrap(), &Value::Object(catalog).to_string())?;
    Ok(policy)
}

pub(super) fn class_name(env: &Environment, object: id) -> Option<&str> {
    (object != nil).then(|| env.objc.try_get_class_name(ObjC::read_isa(object, &env.mem))).flatten()
}

pub(super) fn scripted_encounter(env: &mut Environment, battle: id, encounter: id) -> bool {
    match class_name(env, encounter) {
        Some("ArenaEncounter" | "TownEncounter" | "TournamentEncounter") => return true,
        None | Some("MultiplayerEncounter") => return false,
        _ => {}
    }
    let fixed: bool = msg![env; encounter fixedEncounter];
    let quest: id = msg![env; battle questKey];
    let unique: id = msg![env; battle uniqueKey];
    let reward: id = msg![env; battle battleRewardKey];
    let events = id::from_bits(ivar(env, encounter, 28));
    let timed_events: u32 = msg![env; events count];
    fixed || quest != nil || unique != nil || reward != nil || timed_events != 0
        || [12, 16, 20, 24, 32, 36, 40].into_iter().any(|offset| ivar(env, encounter, offset) != 0)
}

fn list(env: &mut Environment, array: id, limit: u32) -> Option<Vec<id>> {
    let count: u32 = msg![env; array count];
    if count > limit {
        return None;
    }
    Some((0..count).map(|index| msg![env; array objectAtIndex:index]).collect())
}

fn unique_id(env: &mut Environment, member: id) -> u32 {
    msg![env; member uniqueID]
}

fn timed_effects(env: &mut Environment, member: id, state: &mut Value) {
    let buff_list: id = msg![env; member buffs];
    let mut haste = Vec::new();
    let mut guard = Vec::new();
    let mut sleep = Vec::new();
    let mut poison = Vec::new();
    let mut stun = Vec::new();
    let mut barrier = Vec::new();
    let mut haste_seen = 0.0;
    for buff in list(env, buff_list, 128).unwrap_or_default() {
        let timed: bool = msg![env; buff hasDuration];
        let until: i32 = if timed { msg![env; buff durationLeft] } else { INFINITE_DURATION };
        if until <= 0 { continue; }
        let name: id = msg![env; buff stackName];
        let name = if name == nil { String::new() } else { ns_string::to_rust_string(env, name).into_owned() };
        let modifier: f32 = msg![env; buff calculateHaste];
        haste_seen += modifier;
        if modifier != 0.0 { haste.push(json!({ "until": until, "modifier": modifier })); }
        let priority: i32 = msg![env; buff targetPriorityAdjustment];
        if priority != 0 { guard.push(json!({ "until": until, "priority": priority })); }
        if name.to_ascii_lowercase().contains("sleep") { sleep.push(json!({ "until": until })); }
        let component_list: id = msg![env; buff components];
        let mut reductions = Vec::new();
        let mut charges = None;
        for component in list(env, component_list, 64).unwrap_or_default() {
            match class_name(env, component) {
                Some("DamageOverTimeBuffComponent") => {
                    let rate = f32::from_bits(ivar(env, component, 72));
                    let carry = f32::from_bits(ivar(env, component, 76));
                    if rate.is_finite() && rate > 0.0 && carry.is_finite() {
                        poison.push(json!({ "until": until, "rate": rate, "carry": carry,
                            "sourceId": state["id"], "stat": "HP" }));
                    }
                }
                Some("StunProcBuffComponent") => {
                    stun.push(json!(ivar(env, component, 68)));
                }
                Some("PreventDamageBuffComponent") => {
                    let flat = ivar(env, component, 68);
                    let reduction = f32::from_bits(ivar(env, component, 72));
                    let elements = ["Life", "Death", "Water", "Fire", "Earth", "Air", "Arcane"];
                    let flags = bytes(env, component.to_bits() + 76, elements.len() as u32).unwrap_or_default();
                    let elements: Vec<_> = elements.into_iter().zip(flags).filter_map(|(name, &set)| (set != 0).then_some(name)).collect();
                    if reduction.is_finite() && (0.0..=1.0).contains(&reduction) {
                        reductions.push(json!({ "name": name, "flatReduction": flat,
                            "percentReduction": reduction, "allowedElements": elements }));
                    }
                }
                Some("RemoveOnHurtBuffComponent") => charges = Some(ivar(env, component, 68)),
                _ => {}
            }
        }
        barrier.extend(reductions.into_iter().map(|mut reduction| {
            reduction["remainingHits"] = json!(charges.unwrap_or(u32::MAX));
            reduction
        }));
    }
    let native_haste: f32 = msg![env; member calculateHaste];
    if native_haste.is_finite() && (native_haste - haste_seen).abs() > f32::EPSILON {
        haste.push(json!({ "until": INFINITE_DURATION, "modifier": native_haste - haste_seen }));
    }
    for (key, effects) in [
        ("hasteEffects", haste), ("guardEffects", guard), ("sleepEffects", sleep),
        ("poisonEffects", poison), ("stunGifts", stun), ("buffs", barrier),
    ] {
        if !effects.is_empty() { state[key] = json!(effects); }
    }
}

fn member(env: &mut Environment, battle: id, living: id, active: bool) -> Option<Value> {
    let species = super::hunter_history::monster_species(env, living)?;
    let hp: i32 = msg![env; living hp];
    let level: i32 = msg![env; living level];
    let kills: i32 = msg![env; living killsMade];
    let elapsed: i32 = msg![env; living timeInCombat];
    let wait: i32 = if active { msg![env; battle timeDelayForLiving:living] } else { 0 };
    let mut stats = Map::new();
    for (index, key) in ["MaxHP", "Attack", "Magic", "Speed", "Defense", "Resist"].into_iter().enumerate() {
        let stat: i32 = msg![env; living valueForCachedStat:(index as i32)];
        stats.insert(key.into(), json!(stat));
    }
    let mut state = json!({
        "id": unique_id(env, living).to_string(), "monsterId": species, "hp": hp.max(0),
        "level": level, "kills": kills, "enteredAt": -elapsed.max(0), "wait": wait.max(0), "stats": stats,
    });
    if active { timed_effects(env, living, &mut state); }
    Some(state)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Move {
    slot: i32,
    target_ids: Vec<String>,
}

fn take_turn(env: &mut Environment, ai: id, actor: id, battle: id, arena: bool) -> Result<bool, String> {
    if env.objc.hunter_battle_ai.policy.is_none() {
        let policy = load_policy(env);
        env.objc.hunter_battle_ai.policy = Some(policy);
    }
    let teams: id = msg![env; battle teams];
    let teams = list(env, teams, 2).filter(|teams| teams.len() == 2).ok_or("unexpected teams")?;
    let mut snapshot_teams = Vec::new();
    let mut targets = std::collections::HashMap::new();
    let actor_id = unique_id(env, actor).to_string();
    let mut order = Vec::new();
    for (side, team) in teams.into_iter().enumerate() {
        let active: id = msg![env; team targetableMembers];
        let reserve: id = msg![env; team replacements];
        let dead: id = msg![env; team deadMembers];
        let mut active_states = Vec::new();
        for living in list(env, active, 4).ok_or("unexpected active party")? {
            let state = member(env, battle, living, true).ok_or("unsupported active combatant")?;
            if state["hp"].as_i64() == Some(0) { continue; }
            let key = state["id"].as_str().unwrap().to_owned();
            order.push((state["wait"].as_i64().unwrap(), key != actor_id, key.clone()));
            targets.insert(key, (side, living));
            active_states.push(state);
        }
        let reserve_states: Option<Vec<_>> = list(env, reserve, 150).ok_or("unexpected reserve")?
            .into_iter().map(|living| member(env, battle, living, false)).collect();
        let dead_count: u32 = msg![env; dead count];
        snapshot_teams.push(json!({
            "active": active_states, "reserve": reserve_states.ok_or("unsupported reserve combatant")?, "dead": dead_count.min(150),
        }));
    }
    // Use the native queue as the tie-breaker, then move the actual active actor to the front.
    let queue: id = msg![env; battle queOrder];
    let queue_index: std::collections::HashMap<_, _> = list(env, queue, 32).unwrap_or_default()
        .into_iter().enumerate().map(|(index, living)| (living.to_bits(), index)).collect();
    order.sort_by_key(|(wait, other, key)| (*other, *wait, queue_index.get(&targets[key].1.to_bits()).copied()));
    let order: Vec<_> = order.into_iter().map(|(_, _, key)| key).collect();
    let usable = env.objc.get_known_class("UsableAbility", &mut env.mem);
    let ability_list: id = msg![env; actor abilities];
    let mut abilities = std::collections::BTreeMap::new();
    for ability in list(env, ability_list, 64).ok_or("unexpected ability list")? {
        let valid: bool = msg![env; ability isKindOfClass:usable];
        if valid {
            let slot: i32 = msg![env; ability slot];
            if slot > 0 { abilities.insert(slot, ability); }
        }
    }
    let allow_retreat: bool = msg![env; battle allowRetreat];
    let snapshot = json!({
        "actorId": actor_id, "order": order, "allowRetreat": allow_retreat, "teams": snapshot_teams,
        "slots": abilities.keys().collect::<Vec<_>>(), "arena": arena,
    });
    let runtime = env.objc.hunter_battle_ai.policy.as_ref().unwrap().as_ref().map_err(Clone::clone)?;
    let output = runtime.call(CStr::from_bytes_with_nul(b"hunterAiChoose\0").unwrap(), &snapshot.to_string())?;
    let choices: Vec<Move> = serde_json::from_str(&output).map_err(|err| err.to_string())?;
    for choice in choices {
        let Some(&ability) = abilities.get(&choice.slot) else { continue; };
        let selected: Option<Vec<_>> = choice.target_ids.iter().map(|key| targets.get(key).copied()).collect();
        if let Some(selected) = selected {
            if use_ability(env, ai, actor, battle, ability, &selected) {
                if env.options.capture_frames {
                    let version = if arena { "V3" } else { "V2" };
                    log!("Hardmode {version} native turn: slot {} targets {:?}", choice.slot, choice.target_ids);
                }
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn put(env: &mut Environment, dictionary: id, key: &str, value: id) {
    let key = ns_string::from_rust_string(env, key.into());
    () = msg![env; dictionary setObject:value forKey:key];
    release(env, key);
}

fn use_ability(env: &mut Environment, ai: id, actor: id, battle: id, ability: id, targets: &[(usize, id)]) -> bool {
    let picker: id = msg![env; ability targetPickerData];
    let self_only: bool = msg![env; picker selfOnly];
    let random_all: bool = msg![env; picker randomAll];
    let exclude_self: bool = msg![env; picker allButSelf];
    let all_friends: bool = msg![env; picker allFriends];
    let all_enemies: bool = msg![env; picker allEnemies];
    let friends_needed: i32 = msg![env; picker friendlyTargetsNeeded];
    let enemies_needed: i32 = msg![env; picker enemyTargetsNeeded];
    let matching: bool = msg![env; picker addMatchingElementMonsters];
    let mut selected = if self_only { vec![(1, actor)] } else { targets.to_vec() };
    let friendly_team: id = msg![env; battle teamForLiving:actor];
    let enemy_team: id = msg![env; battle enemyTeamForLiving:actor];
    if random_all {
        let side = usize::from((arc4random(env) & 1) == 0);
        let team = if side == 1 { friendly_team } else { enemy_team };
        let active: id = msg![env; team targetableMembers];
        let count: u32 = msg![env; active count];
        if count == 0 { return false; }
        let index = arc4random(env) % count;
        let living: id = msg![env; active objectAtIndex:index];
        selected = vec![(side, living)];
    }
    for (side, team, all, count) in [
        (0, enemy_team, all_enemies || exclude_self, enemies_needed),
        (1, friendly_team, all_friends || exclude_self, friends_needed),
    ] {
        if random_all || self_only { break; }
        if all {
            let active: id = msg![env; team targetableMembers];
            let Some(active) = list(env, active, 4) else { return false; };
            selected.retain(|&(target_side, _)| target_side != side);
            selected.extend(active.into_iter().filter(|&member| !exclude_self || member != actor).map(|member| (side, member)));
        } else {
            let active: id = msg![env; team targetableMembers];
            let available: u32 = msg![env; active count];
            let required = count.max(0).min(available as i32) as usize;
            let supplied = selected.iter().filter(|&&(s, _)| s == side).count();
            if supplied != required && !(matching && side == 0 && supplied >= required) {
                return false;
            }
        }
    }
    let arguments: id = msg_class![env; NSMutableDictionary dictionary];
    let friends: id = msg_class![env; NSMutableArray array];
    let enemies: id = msg_class![env; NSMutableArray array];
    let once: bool = msg![env; ability aiOncePerTarget];
    let instance_name: id = msg![env; ability instanceName];
    let mut history: id = msg![env; ai abilitiesUsedOnTargets];
    let previous: id = if once && instance_name != nil { msg![env; history objectForKey:instance_name] } else { nil };
    let mut used = Vec::new();
    for (side, living) in selected {
        let uid = unique_id(env, living);
        if once {
            if instance_name == nil { return false; }
            let key = ns_string::from_rust_string(env, uid.to_string());
            let seen: id = msg![env; previous objectForKey:key];
            release(env, key);
            if seen != nil { return false; }
            used.push(uid);
        }
        let target: id = msg_class![env; NSNumber numberWithUnsignedInt:uid];
        let list = if side == 1 { friends } else { enemies };
        () = msg![env; list addObject:target];
    }
    let chosen_side = random_all.then(|| {
        let count: u32 = msg![env; friends count];
        usize::from(count != 0)
    });
    if chosen_side != Some(0) { put(env, arguments, "friendlyTargets", friends); }
    if !self_only && chosen_side != Some(1) { put(env, arguments, "enemyTargets", enemies); }
    let blocked: id = msg![env; ability blockUseReasonForLiving:actor inCombat:true abilityArguments:arguments];
    if blocked != nil { return false; }
    if !used.is_empty() {
        if history == nil {
            history = msg_class![env; NSMutableDictionary dictionary];
            () = msg![env; ai setAbilitiesUsedOnTargets:history];
        }
        let seen = if previous == nil {
            let dictionary: id = msg_class![env; NSMutableDictionary dictionary];
            () = msg![env; history setObject:dictionary forKey:instance_name];
            dictionary
        } else { previous };
        let yes: id = msg_class![env; NSNumber numberWithBool:true];
        for uid in used { put(env, seen, &uid.to_string(), yes); }
    }
    () = msg![env; actor queAbility:ability withArguments:arguments abilityPeformEnvironment:battle];
    true
}

pub(super) fn try_assign(env: &mut Environment, ai: id, actor: id) -> bool {
    if actor == nil || !compatible(env) { return false; }
    let battle: id = msg_class![env; Battle sharedInstance];
    let active: id = msg![env; battle activeLiving];
    let queued: id = msg![env; actor queuedAbility];
    if active != actor || queued != nil { return false; }
    let team: id = msg![env; battle teamForLiving:actor];
    let side: i32 = msg![env; team teamNumber];
    let encounter: id = msg![env; battle encounter];
    if side != 1 {
        return false;
    }
    let arena = super::hunter_additional_arena::is_current(env, encounter);
    let v3 = arena || (env.options.hunter_roguelike && encounter != nil);
    let eligible = v3 || (env.options.hunter_hardmode_v2 && scripted_encounter(env, battle, encounter));
    let version = if v3 { "V3" } else { "V2" };
    if env.options.capture_frames {
        log!("Hardmode {version} encounter: {:?}, enabled={eligible}", class_name(env, encounter));
    }
    if !eligible {
        return false;
    }
    match take_turn(env, ai, actor, battle, v3) {
        Ok(true) => true,
        result => {
            if !env.objc.hunter_battle_ai.fallback_logged || env.options.capture_frames {
                let reason = result.err().unwrap_or_else(|| "no legal policy move".into());
                log!("Hardmode {version} native fallback: {reason}");
                env.objc.hunter_battle_ai.fallback_logged = true;
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;

    #[test]
    fn embedded_hardmode_loads_in_the_native_runtime() {
        assert!(Runtime::new().is_ok());
    }
}
