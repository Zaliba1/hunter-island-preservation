// SPDX-License-Identifier: MPL-2.0
//! Hunter Island 2.02 gameplay rules. IMPs, call sites and ivars are for the
//! ARMv7 image checked by the resource overlay before the game is launched.
use super::{hunter_battle_ai, hunter_additional_arena, id, msg, msg_class, nil, release, retain};
use crate::abi::GuestFunction;
use crate::cpu::Cpu;
use crate::expansion::roguelike_bonus;
use crate::frameworks::foundation::ns_string::{get_static_str, to_rust_string};
use crate::libc::stdlib::arc4random;
use crate::mem::{ConstPtr, Ptr};
use crate::Environment;
use std::collections::{BTreeMap, HashSet};

const BATTLE_START: u32 = 0x82a01;
const MAP_SET_POI: u32 = 0x13a5c9;
const MONSTER_XP_AWARD: u32 = 0xbc87d;
const AWARD_XP: u32 = 0xb3dd9;
const XP_POPUP: u32 = 0xd1e25;
const WORLD_VICTORY: u32 = 0x9bc91;
const TOWN_VICTORY: u32 = 0x17f80d;
const BUILD_ENCOUNTER_DATA: u32 = 0x8c265;
const HAS_RECIPE: u32 = 0xdf4c9;
const RECIPE_COUNT: u32 = 0xdf489;
const UNSCHEDULE_UPDATE: u32 = 0x432a9;
const EGG_WHEEL_STOP_CALL: u32 = 0x140fc1;
const EGG_WHEEL_INIT: u32 = 0x13ed91;
const EGG_SLOT_DISPLAY: u32 = 0x1bdef5;
const EGG_PREVIEW_DISPLAY_CALL: u32 = 0x13ff6d;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Rewards {
    #[default]
    Normal,
    Wild,
    Story,
}

impl Rewards {
    fn amount(self, native: u32) -> u32 {
        match self {
            Self::Normal => native,
            Self::Wild => 0,
            Self::Story => native.saturating_mul(2),
        }
    }
}

#[derive(Default)]
pub(super) struct State {
    regions: Option<BTreeMap<String, u8>>,
    map_bonus: u8,
    rewards: Rewards,
    preview_wheel: Option<id>,
    preview_cards: Vec<(id, id)>,
}

fn bonus_at(env: &mut Environment, point: id) -> Option<u8> {
    if point == nil { return None; }
    let point = to_rust_string(env, point);
    if env.objc.hunter_roguelike.regions.is_none() {
        let path = env.bundle.bundle_path().join(crate::expansion::ROGUELIKE_REGIONS);
        let map = env.fs.read(path).ok().and_then(|raw| serde_json::from_slice(&raw).ok());
        env.objc.hunter_roguelike.regions = Some(map.unwrap_or_default());
    }
    roguelike_bonus(env.objc.hunter_roguelike.regions.as_ref().unwrap(), &point)
}

fn objects(env: &mut Environment, array: id) -> Vec<id> {
    let count: u32 = msg![env; array count];
    (0..count).map(|i| msg![env; array objectAtIndex:i]).collect()
}

fn prepare_battle(env: &mut Environment, battle: id) {
    let encounter: id = msg![env; battle encounter];
    let class = hunter_battle_ai::class_name(env, encounter);
    let ordinary_arena = matches!(class, Some("ArenaEncounter" | "TournamentEncounter"));
    let world = class == Some("WorldEncounter");
    let dungeon = class == Some("DungeonEncounter");
    let town = matches!(class, Some("TownEncounter" | "TournamentEncounter" | "ArenaEncounter"));
    let local = !matches!(class, None | Some("MultiplayerEncounter"));
    let custom_arena = hunter_additional_arena::is_current(env, encounter);
    let scripted = local && hunter_battle_ai::scripted_encounter(env, battle, encounter);
    let quest: id = msg![env; battle questKey];
    let rewards = if !local || (ordinary_arena && quest == nil) || custom_arena {
        Rewards::Normal
    } else if scripted {
        Rewards::Story
    } else {
        Rewards::Wild
    };
    env.objc.hunter_roguelike.rewards = rewards;
    if !scripted {
        if env.options.capture_frames { log!("Hunter Roguelike battle: enemies unchanged, rewards {rewards:?}"); }
        return;
    }

    let mut bonus = env.objc.hunter_roguelike.map_bonus.max(1);
    if world {
        // WorldEncounter retains the world at 56 and the POI at 60.
        let poi = id::from_bits(ivar(env, encounter, 60));
        let key: id = msg![env; poi name];
        bonus = bonus_at(env, key).unwrap_or(bonus);
    } else if town {
        let save: id = msg_class![env; UserSavedData sharedInstance];
        let last_town = get_static_str(env, "lastTown");
        let key: id = msg![env; save propertyValueForKey:last_town];
        bonus = bonus_at(env, key).unwrap_or(bonus);
    } else if dungeon {
        let location: id = msg![env; encounter gameLocationRenderable];
        let key: id = msg![env; location instanceName];
        bonus = bonus_at(env, key).unwrap_or(bonus);
    }
    let settings: id = msg_class![env; GameSettings sharedInstance];
    let formula: id = msg![env; settings xpFormula];
    let max: i32 = msg![env; formula maxLevel];
    let teams: id = msg![env; battle teams];
    let enemies: id = msg![env; teams objectAtIndex:1u32];
    let members: id = msg![env; enemies monsterMembers];
    let reserves: id = msg![env; enemies replacements];
    let monsters: HashSet<_> = objects(env, members).into_iter().chain(objects(env, reserves)).collect();
    for monster in monsters {
        let jobs: id = msg![env; monster jobs];
        for job in objects(env, jobs) {
            let level: i32 = msg![env; job level];
            for _ in level..max.min(level + i32::from(bonus)) {
                () = msg![env; job increaseLevel];
            }
        }
    }
    if env.options.capture_frames { log!("Hunter Roguelike battle: enemies +{bonus}, rewards {rewards:?}"); }
}

fn scaled_loot(env: &mut Environment, items: id) -> id {
    let rewards = env.objc.hunter_roguelike.rewards;
    if rewards == Rewards::Normal || items == nil { return items; }
    let silver_class = env.objc.get_known_class("Silver", &mut env.mem);
    let result: id = msg_class![env; NSMutableArray array];
    for item in objects(env, items) {
        let silver: bool = msg![env; item isKindOfClass:silver_class];
        let item = if silver {
            let amount: i32 = msg![env; item amount];
            let amount = rewards.amount(amount.max(0) as u32).min(i32::MAX as u32) as i32;
            if amount == 0 { continue; }
            msg_class![env; Silver pileOfSilverWithAmount:amount]
        } else { item };
        if item != nil { () = msg![env; result addObject:item]; }
    }
    result
}

fn ivar<T: crate::mem::SafeRead>(env: &Environment, object: id, offset: u32) -> T {
    env.mem.read(ConstPtr::from_bits(object.to_bits() + offset))
}

fn adjust_victory_loot(env: &mut Environment, encounter: id, offset: u32) {
    let slot = Ptr::from_bits(encounter.to_bits() + offset);
    let original: id = env.mem.read(slot);
    let scaled = scaled_loot(env, original);
    if scaled != original {
        let kept = retain(env, scaled);
        env.mem.write(slot, kept);
        release(env, original);
    }
}

fn adjust_dungeon_loot(env: &mut Environment, data: id) {
    let monsters: id = msg![env; data monsters];
    let count: u32 = msg![env; monsters count];
    if count == 0 { return; }
    let items: id = msg![env; data items];
    if items == nil { return; }
    let scripted: bool = msg![env; data wasFixedEncounter];
    let scaled: id = msg_class![env; NSMutableArray array];
    let type_key = get_static_str(env, "type");
    let args_key = get_static_str(env, "arguments");
    let amount_key = get_static_str(env, "amount");
    let silver_name = get_static_str(env, "DestroyOnPickup.Silver");
    for item in objects(env, items) {
        let kind: id = msg![env; item objectForKey:type_key];
        let silver: bool = msg![env; kind isEqualToString:silver_name];
        let item = if !silver { item } else if !scripted { continue; } else {
            let args: id = msg![env; item objectForKey:args_key];
            let amount: id = msg![env; args objectForKey:amount_key];
            let amount: i32 = msg![env; amount intValue];
            msg_class![env; Silver pileOfSilverCreationDictionaryForAmount:(amount.saturating_mul(2))]
        };
        () = msg![env; scaled addObject:item];
    }
    () = msg![env; data setItems:scaled];
}

fn guide_gold_wheel(env: &mut Environment, wheel: id, frame: u32) -> Option<id> {
    // Cocos2D calls the scheduled update by IMP. Its one unscheduleUpdate
    // message is sent once the wheel has stopped, before the native animation
    // and award are given the provider in r8 and the slot index at SP+0x58.
    let rewarder: id = id::from_bits(ivar(env, wheel, 184));
    if hunter_battle_ai::class_name(env, rewarder) != Some("Egg") { return None; }
    let suffix: id = msg![env; rewarder slotShellSuffix];
    if suffix == nil || to_rust_string(env, suffix) != "_gold" { return None; }
    let name: id = msg![env; rewarder name];
    if name != nil && to_rust_string(env, name).ends_with("MonsterEggStore") { return None; }

    // Native prizeIndex values 0, 1 and 2 populate the three right-hand rows.
    // Each row fades between its candidates. Match whichever card is most
    // visible at the stop moment to its original slot in the shuffled wheel.
    if env.objc.hunter_roguelike.preview_wheel != Some(wheel) { return None; }
    let nodes = id::from_bits(ivar(env, wheel, 180));
    let nodes = objects(env, nodes);
    let mut slots = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let provider: id = msg![env; (*node) userObject];
        slots.insert(provider.to_bits(), index);
    }
    let mut groups = [None; 3];
    let cards = env.objc.hunter_roguelike.preview_cards.clone();
    for (provider, card) in cards {
        let group: i32 = msg![env; provider prizeIndex];
        let Some(best) = groups.get_mut(group as usize) else { continue; };
        let Some(&slot) = slots.get(&provider.to_bits()) else { continue; };
        let opacity = display_opacity(env, card, 0);
        if best.is_none_or(|(prior, _, _)| opacity > prior) {
            *best = Some((opacity, slot, provider));
        }
    }
    let prizes: Vec<_> = groups.into_iter().flatten().collect();
    if prizes.is_empty() { return None; }
    let draw = arc4random(env) as usize % prizes.len();
    let (_, slot, provider) = prizes[draw];
    let position: f64 = ivar(env, wheel, 188);
    let count = nodes.len() as f64;
    let turns = ((position - slot as f64) / count).round().max(1.0);
    env.mem.write(Ptr::from_bits(wheel.to_bits() + 188),
        turns * count + slot as f64 + 0.125);
    env.mem.write(Ptr::from_bits(frame + 0x58), slot as u32);
    if env.options.capture_frames {
        let name: id = msg![env; provider instanceName];
        let names: Vec<_> = prizes.iter().filter_map(|&(_, _, candidate)| {
            let name: id = msg![env; candidate instanceName];
            (name != nil).then(|| to_rust_string(env, name))
        }).collect();
        let name = (name != nil).then(|| to_rust_string(env, name)).unwrap_or_default();
        log!("Hunter Roguelike gold egg: {name} from {:?}", names);
    }
    Some(provider)
}

fn display_opacity(env: &mut Environment, node: id, depth: u8) -> u8 {
    if node == nil || depth >= 8 { return 0; }
    let opacity_sel = env.objc.lookup_selector("opacity").unwrap();
    let has_opacity: bool = msg![env; node respondsToSelector:opacity_sel];
    if has_opacity { return msg![env; node opacity]; }
    let children: id = msg![env; node children];
    objects(env, children).into_iter().map(|child| display_opacity(env, child, depth + 1)).max().unwrap_or(0)
}

pub(super) fn before_guest_call(env: &mut Environment, receiver: id, imp: GuestFunction) -> bool {
    let address = imp.addr_with_thumb_bit();
    let regs = *env.cpu.regs();
    let lr = regs[Cpu::LR];
    match address {
        MONSTER_XP_AWARD if lr == 0x16ee73 && env.objc.hunter_roguelike.rewards == Rewards::Wild => {
            env.cpu.regs_mut()[0] = 0;
            return true;
        }
        AWARD_XP if lr == 0x16f1a7 && env.objc.hunter_roguelike.rewards == Rewards::Story => {
            env.cpu.regs_mut()[2] = Rewards::Story.amount(regs[2]);
            return false;
        }
        XP_POPUP if lr == 0x16f429 && env.objc.hunter_roguelike.rewards == Rewards::Story => {
            env.cpu.regs_mut()[3] = Rewards::Story.amount(regs[3]);
            return false;
        }
        EGG_WHEEL_INIT => {
            env.objc.hunter_roguelike.preview_wheel = None;
            env.objc.hunter_roguelike.preview_cards.clear();
            return false;
        }
        BATTLE_START | MAP_SET_POI | HAS_RECIPE | RECIPE_COUNT | WORLD_VICTORY | TOWN_VICTORY => (),
        UNSCHEDULE_UPDATE if lr == EGG_WHEEL_STOP_CALL => (),
        _ => return false,
    }
    let flags = env.cpu.cpsr();
    let restored = match address {
        BATTLE_START => { prepare_battle(env, receiver); None }
        MAP_SET_POI => {
            if let Some(bonus) = bonus_at(env, id::from_bits(regs[2])) {
                env.objc.hunter_roguelike.map_bonus = bonus;
            }
            None
        }
        WORLD_VICTORY | TOWN_VICTORY => {
            adjust_victory_loot(env, receiver, if address == WORLD_VICTORY { 64 } else { 56 });
            None
        }
        UNSCHEDULE_UPDATE => guide_gold_wheel(env, receiver, regs[Cpu::SP])
            .map(|provider| (8, provider.to_bits(), false)),
        HAS_RECIPE | RECIPE_COUNT => {
            let book: id = msg_class![env; MonsterpediaBookData sharedInstance];
            let value = if address == HAS_RECIPE {
                let species = id::from_bits(regs[2]);
                let ingredients: id = msg![env; book ingredientsForCombineType:species];
                let count: u32 = msg![env; ingredients count];
                (count > 0).then_some(1)
            } else {
                let combines: id = msg![env; book combines];
                let count: u32 = msg![env; combines count];
                (count > 0).then_some(count)
            };
            value.map(|value| (0, value, true))
        }
        _ => unreachable!(),
    };
    *env.cpu.regs_mut() = regs;
    env.cpu.set_cpsr(flags);
    if let Some((index, value, handled)) = restored {
        env.cpu.regs_mut()[index] = value;
        return handled;
    }
    false
}

pub(super) fn after_guest_call(env: &mut Environment, receiver: id, imp: GuestFunction) {
    let address = imp.addr_with_thumb_bit();
    let lr = env.cpu.regs()[Cpu::LR];
    if address == EGG_SLOT_DISPLAY && lr == EGG_PREVIEW_DISPLAY_CALL {
        let regs = env.cpu.regs();
        let wheel: id = env.mem.read(ConstPtr::from_bits(regs[Cpu::SP] + 0x58));
        let display = id::from_bits(regs[0]);
        if display != nil {
            env.objc.hunter_roguelike.preview_wheel = Some(wheel);
            env.objc.hunter_roguelike.preview_cards.push((receiver, display));
        }
    } else if address == BUILD_ENCOUNTER_DATA && lr == 0x90c77 {
        let regs = *env.cpu.regs();
        let flags = env.cpu.cpsr();
        adjust_dungeon_loot(env, id::from_bits(regs[0]));
        *env.cpu.regs_mut() = regs;
        env.cpu.set_cpsr(flags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encounter_rewards_are_independent_of_normal_rewards() {
        for value in [0, 1, 400, u32::MAX / 2] {
            assert_eq!(Rewards::Wild.amount(value), 0);
            assert_eq!(Rewards::Story.amount(value), value * 2);
            assert_eq!(Rewards::Normal.amount(value), value);
        }
    }

    #[test]
    fn towns_use_nearby_regional_waypoints() {
        let regions = BTreeMap::from([("over-24x112".into(), 1), ("over-40x51".into(), 13)]);
        assert_eq!(roguelike_bonus(&regions, "over-26x111"), Some(1));
        assert_eq!(roguelike_bonus(&regions, "over-42x47"), Some(13));
        assert_eq!(roguelike_bonus(&regions, "room-4x4"), None);
    }
}

