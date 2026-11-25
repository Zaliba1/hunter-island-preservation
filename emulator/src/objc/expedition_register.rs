// SPDX-License-Identifier: MPL-2.0
//! Native Hunter Island expedition register. Ordinary collection challenges keep their original rules.
#[path = "expedition_lineup.rs"]
mod lineup;
#[path = "expedition_frost.rs"]
mod frost;
#[path = "expedition_forest.rs"]
mod forest;
use crate::abi::GuestFunction;
use crate::frameworks::foundation::ns_string::{get_static_str, to_rust_string};
use crate::objc::{id, msg, msg_class, nil};
use crate::Environment;
use std::collections::HashSet;

const SET_ENCOUNTER: u32 = 0x85aa1;
const SET_BREEDER: u32 = 0x847b5;
const ADD_BENCH_MONSTER: u32 = 0x7cb95;
// Encounter.performWinLogic grants the retained copy of each successful capture here.
const WON_CAPTURE_RETURN: u32 = 0x96b8b;
const UPDRAFT_CAPTURE: &str = "Expedition.UpdraftCaught";
const UPDRAFT_MISSES: &str = "Expedition.HabitatMisses.Airbluedragon01";
const START_EXPEDITION_BATTLE: u32 = 0x180015;
const COUNT_COLLECTED: u32 = 0x1af1d9;
const PREFIX: &str = "Expedition.Formenregister/";
const KAMIWYRM: &str = "Crystaldragon02";

const BATTLE_START: u32 = 0x82a01;
const BATTLE_END: u32 = 0x829ed;
const TURN_COMPLETE: u32 = 0xb827d;
const CONFIRMED_DELAY: u32 = 0xf0e61;
const ALLOW_DEATH: u32 = 0xb9095;
const CHECK_BATTLE_END: u32 = 0x834f5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Escort { Convoy, Workshop }

#[derive(Clone, Copy, Debug)]
struct Defense {
    mission: Escort,
    turns: u32,
    hull: u32,
    cover: u32,
}

impl Defense {
    fn new(mission: Escort) -> Self { Self { mission, turns: 0, hull: 4, cover: 0 } }
    fn exposed(&self) -> bool { self.mission == Escort::Workshop || self.turns < 6 }
    fn enemy_action(&mut self) {
        if self.exposed() && self.cover == 0 { self.hull = self.hull.saturating_sub(1); }
        self.turns += 1;
        self.cover = self.cover.saturating_sub(1);
    }
    fn report(&self, env: &mut Environment, battle: id) {
        match self.mission {
            Escort::Convoy => report_defense(env, battle, "HiBattleConvoy", &[self.hull.to_string(), self.turns.min(6).to_string(), self.cover.to_string()]),
            Escort::Workshop => report_defense(env, battle, "HiBattleForeman", &[self.hull.to_string(), self.cover.to_string()]),
        }
    }
}

// The game has one Battle singleton. Its start/end delimit one attempt, including retries.
thread_local! { static DEFENSE: std::cell::Cell<Option<Defense>> = const { std::cell::Cell::new(None) }; }

fn team_number(env: &mut Environment, battle: id, monster: id) -> Option<u32> {
    if monster == nil { return None; }
    let team: id = msg![env; battle teamForLiving:monster];
    if team == nil { return None; }
    Some(msg![env; team teamNumber])
}

fn report_defense(env: &mut Environment, battle: id, key: &'static str, values: &[String]) {
    let bundle: id = msg_class![env; NSBundle mainBundle];
    let key = get_static_str(env, key);
    let localized: id = msg![env; bundle localizedStringForKey:key value:nil table:nil];
    let mut message = to_rust_string(env, localized).into_owned();
    for (index, value) in values.iter().enumerate() {
        message = message.replace(&format!("{{{index}}}"), value);
    }
    log!("Expedition defense: {}", message);
    let string = crate::frameworks::foundation::ns_string::from_rust_string(env, message);
    let _: () = msg![env; battle appendStatusText:string];
    let _: () = msg![env; string release];
}

fn defense_event(env: &mut Environment, receiver: id, address: u32) -> Option<u32> {
    if address == BATTLE_END { DEFENSE.set(None); return None; }
    if address == BATTLE_START {
        DEFENSE.set(None);
        let key: id = msg![env; receiver questKey];
        if key == nil { return None; }
        let mission = match to_rust_string(env, key).as_ref() {
            "Expedition.HiBlockade2" => Escort::Convoy,
            "Expedition.HiAbnahme3" => Escort::Workshop,
            _ => return None,
        };
        DEFENSE.set(Some(Defense::new(mission)));
        log!("Expedition defense: started {:?}", mission);
        return None;
    }
    let mut state = DEFENSE.get()?;
    let battle: id = msg_class![env; Battle sharedInstance];
    match address {
        CONFIRMED_DELAY if state.exposed() => {
            state.cover = 3;
            DEFENSE.set(Some(state));
            state.report(env, battle);
        }
        TURN_COMPLETE if state.exposed() => {
            let team: id = msg![env; battle teamForLiving:receiver];
            if team == nil { return None; }
            let side: u32 = msg![env; team teamNumber];
            if side == 1 {
                state.enemy_action();
                DEFENSE.set(Some(state));
                state.report(env, battle);
            }
        }
        ALLOW_DEATH if state.mission == Escort::Convoy && state.turns < 6 && state.hull > 0 => {
            let team: id = msg![env; battle teamForLiving:receiver];
            if team == nil { return None; }
            let side: u32 = msg![env; team teamNumber];
            if side == 1 {
                let members: id = msg![env; team monsterMembers];
                let reserves: id = msg![env; team replacements];
                let active: u32 = msg![env; members count];
                let waiting: u32 = msg![env; reserves count];
                if active == 1 && waiting == 0 { return Some(0); }
            }
        }
        CHECK_BATTLE_END if state.hull == 0 => {
            // Clear before invoking the retail defeat flow, which can recursively check victory.
            DEFENSE.set(None);
            let teams: id = msg![env; battle teams];
            let opponents: id = msg![env; teams objectAtIndex:1u32];
            let _: () = msg![env; battle forceWinningTeam:opponents];
            return Some(1);
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod escort_tests {
    use super::*;
    #[test]
    fn convoy_guard_covers_three_enemy_actions_and_ends_at_six() {
        let mut battle = Defense::new(Escort::Convoy);
        battle.cover = 3;
        for _ in 0..6 { battle.enemy_action(); }
        assert_eq!((battle.hull, battle.cover), (1, 0));
        battle.enemy_action();
        assert_eq!(battle.hull, 1);
        assert!(!battle.exposed());
    }
    #[test]
    fn objectives_can_sink_and_workshop_stays_at_risk_after_six() {
        let mut battle = Defense::new(Escort::Workshop);
        battle.cover = 3;
        for _ in 0..7 { battle.enemy_action(); }
        assert_eq!(battle.hull, 0);
        let mut unguarded = Defense::new(Escort::Convoy);
        for _ in 0..4 { unguarded.enemy_action(); }
        assert_eq!(unguarded.hull, 0);
    }
}


// A synthetic chat condition lets the retail tavern evaluate the permanent
// Monsterpedia entry every time it builds its list, including older saves.
const HAS_SEEN_CHAT_DIALOG: u32 = 0xdfd69;
const POI_ENTER_EVENTS: u32 = 0xc58f1;
thread_local! {
    static RESUMED_EVENTS: std::cell::Cell<Option<(id,id)>> = const { std::cell::Cell::new(None) };
}

// Re-enter retail arrival once without opening the optional expedition prompts again.
// Other events, encounters, progression conditions and exit discovery remain in game code.
fn resume_field_arrival(env: &mut Environment, director: id, scene: id, node: id, prompt: id) {
    let current: id = msg![env; node currentPOINode];
    let poi: id = msg![env; current pointOfInterest];
    let events: id = msg![env; poi enterEvents];
    let count: u32 = msg![env; events count];
    let remaining: id = msg_class![env; NSMutableArray arrayWithCapacity:count];
    let question = env.objc.get_known_class("YesNoQuestionGameFlowEvent",&mut env.mem);
    for i in 0..count {
        let value: id = msg![env; events objectAtIndex:i];
        let candidate: id = msg_class![env; GameFlowEvent eventFromVal:value];
        let is_question: bool = msg![env; candidate isKindOfClass:question];
        if is_question {
            let title: id = msg![env; candidate titleKey];
            if title != nil {
                let key = to_rust_string(env,title);
                if key.starts_with("HiSearch") || (key.starts_with("Hi") && key.ends_with("ActTitle")) {
                    continue;
                }
            }
        }
        let _: () = msg![env; remaining addObject:value];
    }
    let _: () = msg![env; prompt next];
    let now: id = msg![env; director runningScene];
    if now != scene { return; }
    RESUMED_EVENTS.set(Some((poi,remaining)));
    let _: () = msg![env; node walkFinished];
    RESUMED_EVENTS.set(None);
    log!("Expedition world: resumed retail arrival after optional field prompt");
}

// A first-floor expedition prompt takes priority over the retail waypoint menu.
// After declining the last prompt, give the dungeon its usual opportunity to open it.
fn resume_map_after_decline(env: &mut Environment, prompt: id) -> Option<u32> {
    let title: id = msg![env; prompt titleKey];
    if title == nil || !to_rust_string(env, title).starts_with("Hi") { return None; }
    let alternative: id = msg![env; prompt noEvent];
    if alternative != nil { return None; }
    let director: id = msg_class![env; CCDirector sharedDirector];
    let scene: id = msg![env; director runningScene];
    let children: id = msg![env; scene children];
    let count: u32 = msg![env; children count];
    if count == 0 { return None; }
    let node: id = msg![env; children objectAtIndex:0u32];
    let class = env.objc.get_known_class("DungeonNode", &mut env.mem);
    let dungeon_scene: bool = msg![env; node isKindOfClass:class];
    if !dungeon_scene {
        let title = to_rust_string(env, title);
        if !title.starts_with("HiSearch") && !title.ends_with("ActTitle") { return None; }
        let world = env.objc.get_known_class("WorldNode", &mut env.mem);
        let world_scene: bool = msg![env; node isKindOfClass:world];
        if !world_scene { return None; }
        resume_field_arrival(env, director, scene, node, prompt);
        return Some(0);
    }
    let dungeon: id = msg![env; node dungeon];
    let level: u32 = msg![env; dungeon level];
    let allowed: bool = msg![env; node allowWaypointSelection];
    if level != 1 || !allowed { return None; }
    let name: id = msg![env; dungeon instanceName];
    let save: id = msg_class![env; UserSavedData sharedInstance];
    let points: id = msg![env; save waypointsForDungeon:name];
    let count: u32 = msg![env; points count];
    if count == 0 { return None; }
    let _: () = msg![env; prompt next];
    let shown: bool = msg![env; node showWaypointSelection];
    if shown {
        let _: () = msg![env; node setAllowWaypointSelection:false];
    }
    log!("Expedition waypoints: {} at floor {}, {} saved, menu {}",to_rust_string(env,name),level,count,shown);
    Some(0)
}

pub(super) fn try_handle(env: &mut Environment, receiver: id, imp: GuestFunction) -> bool {
    let address=imp.addr_with_thumb_bit();
    if !matches!(address, SET_ENCOUNTER | SET_BREEDER | ADD_BENCH_MONSTER | HAS_SEEN_CHAT_DIALOG | POI_ENTER_EVENTS | forest::ADJUST_HP | frost::HIT | frost::DAMAGE_REPORT | lineup::SHOW_QUESTION | lineup::CONFIRM_QUESTION | 0x1c8921 | COUNT_COLLECTED | START_EXPEDITION_BATTLE | 0x18eae1 | 0x18eba9 | BATTLE_START | BATTLE_END | TURN_COMPLETE | CONFIRMED_DELAY | ALLOW_DEATH | CHECK_BATTLE_END)
        || env.bundle.bundle_identifier() != "pockettrend.hunterisland"
        || env.bundle.bundle_version() != "2.02"
    {
        return false;
    }
    let regs = *env.cpu.regs();
    let flags = env.cpu.cpsr();
    if address == SET_ENCOUNTER {
        env.objc.updraft_encounter = false;
        return false;
    }
    if address == SET_BREEDER {
        let breeder = id::from_bits(regs[2]);
        let dictionary = env.objc.get_known_class("NSDictionary", &mut env.mem);
        let is_dict: bool = msg![env; breeder isKindOfClass:dictionary];
        let tag = if is_dict { field(env, breeder, "expeditionTrack") } else { nil };
        env.objc.updraft_encounter = tag != nil && to_rust_string(env, tag) == "Updraft";
        *env.cpu.regs_mut() = regs;
        env.cpu.set_cpsr(flags);
        return false;
    }
    if address == ADD_BENCH_MONSTER {
        if env.objc.updraft_encounter && regs[crate::cpu::Cpu::LR] == WON_CAPTURE_RETURN
            && super::hunter_history::monster_species(env, id::from_bits(regs[2])).as_deref() == Some("Airbluedragon01")
        {
            let save: id = msg_class![env; UserSavedData sharedInstance];
            set_updraft_caught(env, save, true);
        }
        *env.cpu.regs_mut() = regs;
        env.cpu.set_cpsr(flags);
        return false;
    }
    if address == HAS_SEEN_CHAT_DIALOG {
        let key = id::from_bits(regs[2]);
        let marker = get_static_str(env, "Expedition.HasFusedKamiwyrm");
        let matches: bool = msg![env; key isEqual:marker];
        let track = get_static_str(env, UPDRAFT_CAPTURE);
        let is_track: bool = msg![env; key isEqual:track];
        let result = if is_track {
            Some(u32::from(updraft_caught(env, receiver)))
        } else if matches {
            let species = get_static_str(env, KAMIWYRM);
            let template: id = msg_class![env; MonsterpediaBookData bookTemplateMonsterWithShortTypeName:species];
            let owned: bool = template != nil && msg![env; receiver hasMonster:template];
            Some(u32::from(owned))
        } else {
            None
        };
        *env.cpu.regs_mut() = regs;
        env.cpu.set_cpsr(flags);
        if let Some(value) = result {
            env.cpu.regs_mut()[0] = value;
            return true;
        }
        return false;
    }
    if address == POI_ENTER_EVENTS {
        if let Some((poi,array)) = RESUMED_EVENTS.get() {
            if poi == receiver {
                RESUMED_EVENTS.set(None);
                env.cpu.regs_mut()[0] = array.to_bits();
                return true;
            }
        }
        return false;
    }
    if address == forest::ADJUST_HP {
        let delta = forest::adjusted_hp_delta(env, receiver);
        *env.cpu.regs_mut() = regs;
        env.cpu.set_cpsr(flags);
        if let Some(delta) = delta { env.cpu.regs_mut()[2] = delta as u32; }
        return false;
    }
    if matches!(address, BATTLE_START | BATTLE_END | TURN_COMPLETE | CONFIRMED_DELAY | frost::DAMAGE_REPORT) {
        forest::event(env, receiver, address);
        *env.cpu.regs_mut() = regs;
        env.cpu.set_cpsr(flags);
    }
    let result = if address == 0x1c8921 {
        resume_map_after_decline(env, receiver)
    } else if matches!(address, lineup::SHOW_QUESTION | lineup::CONFIRM_QUESTION) {
        lineup::question(env, receiver, address)
    } else if address == COUNT_COLLECTED {
        count_for_register(env, receiver)
    } else if address == START_EXPEDITION_BATTLE {
        prepare_habitat_encounter(env, receiver);
        None // Let Hunter Island construct and run the actual battle.
    } else if matches!(address, BATTLE_START | BATTLE_END | TURN_COMPLETE | CONFIRMED_DELAY | ALLOW_DEATH | CHECK_BATTLE_END) {
        frost::event(env, receiver, address).or_else(|| defense_event(env, receiver, address))
    } else if matches!(address, frost::HIT | frost::DAMAGE_REPORT) {
        frost::event(env, receiver, address)
    } else if prevent_orbling_release(env, receiver) {
        Some(0)
    } else {
        None
    };
    *env.cpu.regs_mut() = regs;
    env.cpu.set_cpsr(flags);
    if let Some(count) = result {
        env.cpu.regs_mut()[0] = count;
        true
    } else {
        false
    }
}

fn field(env: &mut Environment, dictionary: id, name: &'static str) -> id {
    let key = get_static_str(env, name);
    msg![env; dictionary objectForKey:key]
}

fn set_field(env: &mut Environment, dictionary: id, name: &'static str, value: id) {
    let key = get_static_str(env, name);
    let _: () = msg![env; dictionary setObject:value forKey:key];
}

fn draw(env: &mut Environment, choices: u32) -> u32 {
    let random = u64::from(crate::libc::stdlib::arc4random(env));
    ((random * u64::from(choices)) >> 32) as u32
}

fn set_updraft_caught(env: &mut Environment, save: id, caught: bool) {
    let key = get_static_str(env, UPDRAFT_CAPTURE);
    let bit = u32::from(caught);
    let value: id = msg_class![env; NSNumber numberWithUnsignedInt:bit];
    let _: () = msg![env; save setPropertyValue:value forKey:key];
}

fn updraft_caught(env: &mut Environment, save: id) -> bool {
    let key = get_static_str(env, UPDRAFT_CAPTURE);
    let receipt: id = msg![env; save propertyValueForKey:key];
    if receipt != nil { return msg![env; receipt boolValue]; }
    // Preview 15 recorded attempts and species, but not the capture site.
    let misses = get_static_str(env, UPDRAFT_MISSES);
    let attempts: id = msg![env; save propertyValueForKey:misses];
    let caught = if attempts == nil { false } else {
        let species = get_static_str(env, "Airbluedragon01");
        let template: id = msg_class![env; MonsterpediaBookData bookTemplateMonsterWithShortTypeName:species];
        template != nil && msg![env; save hasMonster:template]
    };
    set_updraft_caught(env, save, caught);
    caught
}

fn prepare_habitat_encounter(env: &mut Environment, event: id) {
    let populator: id = msg![env; event monsterPopulator];
    if populator == nil { return; }
    let dictionary = env.objc.get_known_class("NSDictionary", &mut env.mem);
    let is_dictionary: bool = msg![env; populator isKindOfClass:dictionary];
    if !is_dictionary { return; }
    let habitat = field(env, populator, "expeditionHabitat");
    if habitat == nil { return; }
    let variants = field(env, habitat, "levelEncounters");
    let count: u32 = msg![env; variants count];
    let counter_key = field(env, habitat, "missCounter");
    if count == 0 || counter_key == nil { return; }
    let save: id = msg_class![env; UserSavedData sharedInstance];
    let recorded: id = msg![env; save propertyValueForKey:counter_key];
    let misses: u32 = if recorded == nil { 0 } else { msg![env; recorded unsignedIntValue] };
    let always = field(env, habitat, "guaranteed");
    let guaranteed: bool = msg![env; always boolValue];
    let found = guaranteed || misses >= 2 || draw(env, 100) < 15;
    let next_misses = if found { 0 } else { misses + 1 };
    let selected: id = if found {
        let index = draw(env, count);
        msg![env; variants objectAtIndex:index]
    } else {
        msg_class![env; NSArray array]
    };
    let original_args = field(env, populator, "arguments");
    let args: id = msg_class![env; NSMutableDictionary dictionaryWithDictionary:original_args];
    if found && to_rust_string(env, counter_key) == UPDRAFT_MISSES {
        let one: id = msg_class![env; NSNumber numberWithUnsignedInt:1u32];
        set_field(env, args, "minMonstersPerPack", one);
        set_field(env, args, "maxMonstersPerPack", one);
    }
    set_field(env, args, "fixedEncounters", selected);
    let copy: id = msg_class![env; NSMutableDictionary dictionaryWithDictionary:populator];
    set_field(env, copy, "arguments", args);
    let _: () = msg![env; event setMonsterPopulator:copy];
    let value: id = msg_class![env; NSNumber numberWithUnsignedInt:next_misses];
    let _: () = msg![env; save setPropertyValue:value forKey:counter_key];
    log!("Expedition habitat {}: misses {} -> {}, target {}", to_rust_string(env, counter_key), misses, next_misses, found);
}

fn prevent_orbling_release(env: &mut Environment, display: id) -> bool {
    let marker = env.bundle.bundle_path().join("HiTakranDrain.plist");
    if !env.fs.is_file(&marker) { return false; }
    let monster: id = msg![env; display living];
    if monster == nil { return false; }
    let name: id = msg![env; monster name];
    if name == nil { return false; }
    let full = to_rust_string(env, name);
    if full.rsplit('.').next() != Some("Crystaldragon01") { return false; }
    let key = get_static_str(env, "ExpeditionOrblingSafe");
    let _: id = msg_class![env; MonsterHatchPopup showPopupForLocalizedMessageName:key];
    log!("Expedition: kept earned Orbling in the party on release request");
    true
}

fn count_for_register(env: &mut Environment, quest: id) -> Option<u32> {
    let filter: id = msg![env; quest typeFilter];
    if filter == nil { return None; }
    let filter = to_rust_string(env, filter).into_owned();
    let all = filter.strip_prefix(PREFIX)?;
    let mut eligible: HashSet<_> = all.split('|').collect();
    if eligible.len() != 359 || eligible.contains(KAMIWYRM) { return Some(0); }
    let names: id = msg_class![env; MonsterpediaBookData monsterShortNames];
    let size: u32 = msg![env; names count];
    let save: id = msg_class![env; UserSavedData sharedInstance];
    let mut found = 0;
    for index in 0..size {
        let name: id = msg![env; names objectAtIndex:index];
        if !eligible.remove(to_rust_string(env, name).as_ref()) { continue; }
        let monster: id = msg_class![env; MonsterpediaBookData bookTemplateMonsterWithShortTypeName:name];
        if monster != nil {
            // The game's own predicate uses the recorded highest level for ordinary forms
            // and the spirit ledger for story summons. Both survive fusion or release.
            let owned: bool = msg![env; save hasMonster:monster];
            found += u32::from(owned);
        }
    }
    if !eligible.is_empty() {
        log!("Expedition register: {} catalog entries missing", eligible.len());
        return Some(0);
    }
    log!("Expedition register: {found}/359 forms");
    Some(found)
}
