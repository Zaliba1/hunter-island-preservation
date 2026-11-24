// SPDX-License-Identifier: MPL-2.0
//! Combat rules for Carogi's and Hyren's frost encounters.
use super::{report_defense, team_number, ALLOW_DEATH, BATTLE_END, BATTLE_START, CONFIRMED_DELAY, TURN_COMPLETE};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::objc::{id, msg, msg_class, nil};
use crate::Environment;
use std::cell::Cell;

pub(super) const HIT: u32 = 0xb84e9; // Living.hitWithDamage:abilityEnvironment:
pub(super) const DAMAGE_REPORT: u32 = 0x844d9; // Battle.notifyLiving:tookDamage:fromSource:

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Trial { Guardian, Wards }

#[derive(Clone, Copy)]
struct Frost {
    trial: Trial,
    enemy_turns: u32,
    player_turns: u32,
    covered_turns: u32,
    wardens: [id; 3],
}

impl Frost {
    fn charged(self) -> bool { self.enemy_turns % 2 == 1 }
    fn ward_slot(self) -> usize { (self.player_turns / 2).min(2) as usize }
    fn strength(self, amount: u32, target: id, enemy_attacks: bool) -> u32 {
        match self.trial {
            Trial::Guardian if enemy_attacks && self.charged() && self.covered_turns == 0 =>
                amount.saturating_mul(2),
            Trial::Wards if !enemy_attacks && target == self.wardens[self.ward_slot()] && amount > 0 =>
                (amount / 3).max(1),
            _ => amount,
        }
    }
}

thread_local! { static FROST: Cell<Option<Frost>> = const { Cell::new(None) }; }

fn report_ward(env: &mut Environment, battle: id, state: Frost) {
    let slot = state.ward_slot();
    let warden = state.wardens[slot];
    let name: id = msg![env; warden localizedName];
    let position = (slot + 1).to_string();
    if name == nil {
        report_defense(env, battle, "HiBattleWardSlot", &[position]);
    } else {
        let name = to_rust_string(env, name).into_owned();
        report_defense(env, battle, "HiBattleWardName", &[position, name]);
    }
}

fn start(env: &mut Environment, battle: id) {
    FROST.set(None);
    let key: id = msg![env; battle questKey];
    if key == nil { return; }
    let trial = match to_rust_string(env,key).as_ref() {
        "Expedition.HiEwigeseis3" => Trial::Guardian,
        "Expedition.HiEisrezept2" => Trial::Wards,
        _ => return,
    };
    let teams: id = msg![env; battle teams];
    let enemy: id = msg![env; teams objectAtIndex:1u32];
    let members: id = msg![env; enemy monsterMembers];
    let count: u32 = msg![env; members count];
    let mut wardens = [nil; 3];
    for (i, slot) in wardens.iter_mut().enumerate().take(count as usize) {
        let index = i as u32;
        *slot = msg![env; members objectAtIndex:index];
    }
    let state = Frost { trial, wardens, enemy_turns:0, player_turns:0, covered_turns:0 };
    FROST.set(Some(state));
    if trial == Trial::Wards {
        report_ward(env, battle, state);
    }
    log!("Expedition frost: started {:?}, {} enemies in front",trial,count);
}

pub(super) fn event(env: &mut Environment, receiver: id, address: u32) -> Option<u32> {
    if address == BATTLE_END { FROST.set(None); return None; }
    if address == BATTLE_START { start(env,receiver); return None; }
    let mut state = FROST.get()?;
    let regs = *env.cpu.regs();
    let battle: id = msg_class![env; Battle sharedInstance];
    match address {
        CONFIRMED_DELAY if state.trial == Trial::Guardian => {
            state.covered_turns = 3;
            FROST.set(Some(state));
            report_defense(env,battle,"HiBattleFrostCover", &[]);
        }
        TURN_COMPLETE => {
            let side = team_number(env,battle,receiver)?;
            let old_slot = state.ward_slot();
            if side == 0 {
                state.player_turns += 1;
            } else if side == 1 {
                state.enemy_turns += 1;
                state.covered_turns = state.covered_turns.saturating_sub(1);
            }
            FROST.set(Some(state));
            if state.trial == Trial::Guardian && side == 1 && state.charged() {
                report_defense(env,battle,"HiBattleCharged", &[]);
            } else if state.trial == Trial::Wards && state.ward_slot() != old_slot {
                report_ward(env, battle, state);
            }
        }
        HIT => {
            let damage = id::from_bits(regs[2]);
            if damage == nil { return None; }
            let source_id: u32 = msg![env; damage sourceID];
            let source: id = msg![env; battle livingForUniqueID:source_id];
            let source_side = team_number(env,battle,source)?;
            let target_side = team_number(env,battle,receiver)?;
            if source_side == target_side { return None; }
            let amount: u32 = msg![env; damage amount];
            let adjusted = state.strength(amount,receiver,source_side == 1);
            if adjusted != amount {
                // The retail damage path applies element, buffs, defense and HP limits afterwards.
                let _: () = msg![env; damage setAmount:adjusted];
            }
            log!("Expedition frost hit: {:?} turn={} ward={} cover={} from={} to={} strength={} -> {}",
                state.trial,state.enemy_turns,state.ward_slot()+1,state.covered_turns,source_side,target_side,amount,adjusted);
        }
        DAMAGE_REPORT => {
            let damage = id::from_bits(regs[3]);
            if damage != nil {
                let amount: u32 = msg![env; damage amount];
                log!("Expedition frost: resolved damage {}",amount);
            }
        }
        ALLOW_DEATH if state.trial == Trial::Guardian && state.enemy_turns < 2 &&
                state.wardens[0] == receiver => {
            log!("Expedition frost: guardian holds at 1 HP before its second action");
            return Some(0);
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(trial: Trial) -> Frost {
        Frost { trial, enemy_turns:0, player_turns:0, covered_turns:0,
            wardens:[id::from_bits(1),id::from_bits(2),id::from_bits(3)] }
    }
    #[test]
    fn only_the_uncovered_charged_enemy_attack_doubles() {
        let mut frost=state(Trial::Guardian);
        assert_eq!(frost.strength(100,nil,true),100);
        frost.enemy_turns=1;
        assert_eq!(frost.strength(100,nil,false),100);
        assert_eq!(frost.strength(100,nil,true),200);
        frost.covered_turns=1;
        assert_eq!(frost.strength(100,nil,true),100);
    }
    #[test]
    fn wards_follow_the_first_three_wardens_after_two_and_four_player_actions() {
        let mut frost=state(Trial::Wards);
        for (turn, slot) in [0,0,1,1,2,2,2].into_iter().enumerate() {
            frost.player_turns=turn as u32;
            assert_eq!(frost.ward_slot(),slot);
            for (i,&target) in frost.wardens.iter().enumerate() {
                assert_eq!(frost.strength(90,target,false),if i==slot {30} else {90});
                assert_eq!(frost.strength(90,target,true),90);
            }
        }
    }
}
