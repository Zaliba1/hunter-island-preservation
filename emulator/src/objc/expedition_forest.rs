// SPDX-License-Identifier: MPL-2.0
//! Environmental effects in Reidia's three fixed battles at the root basin.
use super::{frost::DAMAGE_REPORT, report_defense, BATTLE_END, BATTLE_START, CONFIRMED_DELAY, TURN_COMPLETE};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::mem::ConstPtr;
use crate::objc::{id, msg, msg_class, nil};
use crate::Environment;
use std::cell::Cell;

pub(super) const ADJUST_HP: u32 = 0xb7391;
// The original damage resolver calls Living.adjustHP once after defense, listeners and HP limits.
const AFTER_HIT_ADJUST: u32 = 0xb89a6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hazard { Spores, Landslide }

#[derive(Clone, Copy)]
struct Forest {
    hazard: Hazard,
    enemy_turns: u32,
    cover: u8,
    attacker: id,
    hit_targets: [id; 3],
}

thread_local! { static FOREST: Cell<Option<Forest>> = const { Cell::new(None) }; }

fn spore_loss(max_hp: i32, hp: i32, base: i32) -> i32 {
    let spores = (i64::from(max_hp.max(0)) * 2 + 24) / 25;
    (spores as i32).min(hp.saturating_sub(base).max(0))
}

fn start(env: &mut Environment, battle: id) {
    FOREST.set(None);
    let key: id = msg![env; battle questKey];
    if key == nil { return; }
    let hazard = match to_rust_string(env, key).as_ref() {
        "Expedition.HiWurzel1" => Hazard::Spores,
        "Expedition.HiWurzel2" => Hazard::Landslide,
        "Expedition.HiWurzel3" => {
            let teams: id = msg![env; battle teams];
            let enemy: id = msg![env; teams objectAtIndex:1u32];
            let members: id = msg![env; enemy monsterMembers];
            let first: id = msg![env; members objectAtIndex:0u32];
            let name: id = msg![env; first name];
            if name == nil { return; }
            let value = to_rust_string(env, name);
            match value.rsplit('.').next() {
                Some("Boxbunny02") => Hazard::Landslide,
                Some("Naturalegg01") => Hazard::Spores,
                _ => { log!("Expedition forest: unexpected final formation {:?}", value); return; }
            }
        }
        _ => return,
    };
    FOREST.set(Some(Forest { hazard, enemy_turns: 0, cover: 0, attacker: nil, hit_targets: [nil; 3] }));
    let message = match hazard {
        Hazard::Spores => "HiBattleSpores",
        Hazard::Landslide => "HiBattleLandslide",
    };
    report_defense(env, battle, message, &[]);
    log!("Expedition forest: started {:?}", hazard);
}

// Called only for the game's own final damage adjustment. The ordinary method still
// handles HP, health notifications, death and replacements with the original attack source.
pub(super) fn adjusted_hp_delta(env: &mut Environment, victim: id) -> Option<i32> {
    let state = FOREST.get()?;
    let regs = *env.cpu.regs();
    let delta = regs[2] as i32;
    if state.hazard != Hazard::Spores || state.cover > 0 || delta >= 0 ||
        regs[14] & !1 != AFTER_HIT_ADJUST { return None; }
    let battle: id = msg_class![env; Battle sharedInstance];
    let source: id = env.mem.read(ConstPtr::<id>::from_bits(regs[13]));
    if super::team_number(env, battle, victim) != Some(0) ||
        super::team_number(env, battle, source) != Some(1) { return None; }
    let hp: i32 = msg![env; victim hp];
    let max_hp: i32 = msg![env; victim valueForCachedStat:0i32];
    let extra = spore_loss(max_hp, hp, delta.saturating_neg());
    if extra == 0 { return None; }
    let name: id = msg![env; victim localizedName];
    let name = to_rust_string(env, name).into_owned();
    report_defense(env, battle, "HiBattleSporeDamage", &[name, extra.to_string()]);
    log!("Expedition forest: spore HP {} of {}, hit {}, bonus {}, remaining {}",hp,max_hp,-delta,extra,hp+delta-extra);
    Some(delta-extra)
}

fn displace(env: &mut Environment, battle: id, targets: [id; 3]) {
    let teams: id = msg![env; battle teams];
    let team: id = msg![env; teams objectAtIndex:0u32];
    let members: id = msg![env; team monsterMembers];
    for target in targets {
        if target == nil { continue; }
        let active: bool = msg![env; members containsObject:target];
        if !active { continue; }
        let hp: i32 = msg![env; target hp];
        if hp <= 0 { continue; }
        let name: id = msg![env; target localizedName];
        let name = to_rust_string(env, name).into_owned();
        let reserves: id = msg![env; team replacements];
        let count: u32 = msg![env; reserves count];
        if count == 0 {
            let before: i32 = msg![env; battle timeDelayForLiving:target];
            let _: () = msg![env; battle addTimeDelay:120i32 toLiving:target];
            let after: i32 = msg![env; battle timeDelayForLiving:target];
            report_defense(env, battle, "HiBattleRockDelay", &[name]);
            log!("Expedition forest: delay {} -> {}",before,after);
        } else {
            // Same order as SendMonsterToBackAbilityComponent in Hunter Island 2.02.
            let waiting: id = msg_class![env; NSArray arrayWithObject:target];
            let _: id = msg![env; team replaceMonster:target];
            let _: () = msg![env; team loadMonsters:waiting frontLoadReplacements:false];
            report_defense(env, battle, "HiBattleDisplaced", &[name]);
            let queue: id = msg![env; team replacements];
            let last: id = msg![env; queue lastObject];
            log!("Expedition forest: displaced; target at end of queue: {}",last==target);
        }
        break;
    }
}

pub(super) fn event(env: &mut Environment, receiver: id, address: u32) {
    if address == BATTLE_END { FOREST.set(None); return; }
    if address == BATTLE_START { start(env,receiver); return; }
    let Some(mut state) = FOREST.get() else { return; };
    let regs = *env.cpu.regs();
    let battle: id = msg_class![env; Battle sharedInstance];
    match address {
        CONFIRMED_DELAY => {
            state.cover=3;
            FOREST.set(Some(state));
            report_defense(env,battle,"HiBattleSlopeCover", &[]);
        }
        DAMAGE_REPORT if state.hazard == Hazard::Landslide && state.cover == 0 && state.enemy_turns % 3 == 2 => {
            let victim = id::from_bits(regs[2]);
            let damage = id::from_bits(regs[3]);
            if damage == nil || super::team_number(env,battle,victim) != Some(0) { return; }
            let source_id: u32 = msg![env; damage sourceID];
            let source: id = msg![env; battle livingForUniqueID:source_id];
            if super::team_number(env,battle,source) != Some(1) { return; }
            if state.attacker != source { state.hit_targets=[nil;3]; state.attacker=source; }
            if !state.hit_targets.contains(&victim) {
                if let Some(slot)=state.hit_targets.iter_mut().find(|slot| **slot==nil) { *slot=victim; }
            }
            FOREST.set(Some(state));
        }
        TURN_COMPLETE => {
            let enemy = super::team_number(env,battle,receiver) == Some(1);
            if enemy { state.enemy_turns+=1; }
            let landslide = enemy && state.hazard==Hazard::Landslide && state.enemy_turns%3==0 &&
                state.cover==0 && state.attacker==receiver;
            let victims=state.hit_targets;
            state.hit_targets=[nil;3];
            state.attacker=nil;
            if enemy { state.cover=state.cover.saturating_sub(1); }
            FOREST.set(Some(state));
            if landslide { displace(env,battle,victims); }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spores_round_up_and_stop_at_remaining_hp() {
        assert_eq!(spore_loss(2360,2360,100),189);
        assert_eq!(spore_loss(2360,102,100),2);
        assert_eq!(spore_loss(2360,99,99),0);
        assert_eq!(spore_loss(1,1,0),1);
    }
}
