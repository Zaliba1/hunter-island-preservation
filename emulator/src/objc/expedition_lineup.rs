// SPDX-License-Identifier: MPL-2.0
//! Keep the original player's batting order for the three-stage expedition trials.
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::objc::{id, msg, msg_class, nil};
use crate::Environment;

pub(super) const SHOW_QUESTION: u32 = 0x1c87c5;
pub(super) const CONFIRM_QUESTION: u32 = 0x1c88c9;

struct Trial {
    prompts: [&'static str; 3],
    stages: [&'static str; 3],
    signature_key: &'static str,
    restart_message: &'static str,
}

static TRIALS: [Trial; 2] = [
    Trial {
        prompts: ["HiBlockade1ActTitle","HiBlockade2ActTitle","HiBlockade3ActTitle"],
        stages: ["Expedition.HiBlockade1","Expedition.HiBlockade2","Expedition.HiBlockade3"],
        signature_key: "Expedition.Lineup.Convoy",
        restart_message: "HiConvoyRestartQuestion",
    },
    Trial {
        prompts: ["HiEwigeseis1ActTitle","HiEwigeseis2ActTitle","HiEwigeseis3ActTitle"],
        stages: ["Expedition.HiEwigeseis1","Expedition.HiEwigeseis2","Expedition.HiEwigeseis3"],
        signature_key: "Expedition.Lineup.Frost",
        restart_message: "HiFrostRestartQuestion",
    },
];

fn ordered_members(env: &mut Environment, save: id, assign_missing: bool) -> Option<String> {
    let player: id = msg_class![env; Player sharedInstance];
    // false includes injured party members, while still excluding the bench.
    let party: id = msg![env; player monstersFromBattingOrderWithAlive:false];
    let count: u32 = msg![env; party count];
    if count == 0 { return None; }
    let member_key = get_static_str(env, "ExpeditionMemberSerial");
    let sequence_key = get_static_str(env, "Expedition.NextMemberSerial");
    let mut order = Vec::new();
    for index in 0..count {
        let monster: id = msg![env; party objectAtIndex:index];
        let mut serial: id = msg![env; monster valueForProperty:member_key];
        if serial == nil {
            if !assign_missing { return None; }
            let previous: id = msg![env; save propertyValueForKey:sequence_key];
            let sequence: u32 = msg![env; previous unsignedIntValue];
            let next = sequence.checked_add(1)?;
            serial = msg_class![env; NSNumber numberWithUnsignedInt:next];
            let _: () = msg![env; monster setValue:serial forProperty:member_key permanent:true];
            let _: () = msg![env; save setPropertyValue:serial forKey:sequence_key];
        }
        let value: u32 = msg![env; serial unsignedIntValue];
        let species: id = msg![env; monster name];
        order.push(format!("{}/{}",value,to_rust_string(env,species)));
    }
    Some(order.join("|"))
}

fn restart(env: &mut Environment, save: id, trial: &Trial, stage: usize) -> bool {
    let player: id = msg_class![env; Player sharedInstance];
    let objects: id = msg![env; player questObjects];
    let active_key = get_static_str(env, "activeQuests");
    let active: id = msg![env; save valueOfFlag:active_key];
    let finished_key = get_static_str(env, "finishedQuests");
    let finished: id = msg![env; save valueOfFlag:finished_key];
    let current = get_static_str(env, trial.stages[stage]);
    let index: u32 = msg![env; active indexOfObject:current];
    let count: u32 = msg![env; active count];
    if index >= count || finished == nil { return false; }
    let object_count: u32 = msg![env; objects count];
    let slot = (0..object_count).find(|&slot| {
        let quest: id = msg![env; objects objectAtIndex:slot];
        let name: id = msg![env; quest name];
        let matches: bool = msg![env; name isEqualToString:current];
        matches
    });
    let Some(slot) = slot else { return false; };
    let first = get_static_str(env, trial.stages[0]);
    let first_quest: id = msg_class![env; Quest questForName:first];
    if first_quest == nil { return false; }
    // Replace the active quest in both lists before the next save. The retail
    // clearQuest: command immediately saves a state with no active trial.
    let _: () = msg![env; active replaceObjectAtIndex:index withObject:first];
    let _: () = msg![env; objects replaceObjectAtIndex:slot withObject:first_quest];
    for name in &trial.stages[..=stage] {
        let key = get_static_str(env, name);
        let _: () = msg![env; finished removeObject:key];
        let _: () = msg![env; save setQuestProgressData:nil forQuestName:key];
    }
    let key = get_static_str(env, trial.signature_key);
    let _: () = msg![env; save removeProperty:key];
    let _: () = msg![env; first_quest notifyStarted];
    let _: () = msg![env; save setSaveFileDirty];
    log!("Expedition lineup: restarted {} from stage {}",trial.stages[0],stage+1);
    true
}

pub(super) fn question(env: &mut Environment, prompt: id, action: u32) -> Option<u32> {
    let title: id = msg![env; prompt titleKey];
    if title == nil { return None; }
    let title = to_rust_string(env, title);
    let (trial, stage) = TRIALS.iter().find_map(|trial|
        trial.prompts.iter().position(|&key| key == title.as_ref()).map(|i| (trial,i)))?;
    let save: id = msg_class![env; UserSavedData sharedInstance];
    let key = get_static_str(env, trial.signature_key);
    let prior: id = msg![env; save propertyValueForKey:key];
    let mismatch = stage > 0 && prior != nil &&
        ordered_members(env,save,false).as_deref() != Some(to_rust_string(env,prior).as_ref());
    if action == SHOW_QUESTION {
        if mismatch {
            let message = get_static_str(env, trial.restart_message);
            let _: () = msg![env; prompt setMessageKey:message];
            log!("Expedition lineup: changed at {}",trial.stages[stage]);
        } else {
            let original = from_rust_string(env, title.trim_end_matches("Title").to_string()+"Question");
            let _: () = msg![env; prompt setMessageKey:original];
            let _: () = msg![env; original release];
        }
        return None;
    }
    if mismatch {
        if !restart(env,save,trial,stage) { log!("Expedition lineup: couldn't reset {}",trial.stages[stage]); }
        let _: () = msg![env; prompt next];
        return Some(0);
    }
    // A save from an earlier release may reach stage two or three without a
    // register. Bind its real party on the first confirmed fight after update.
    if stage == 0 || prior == nil {
        let signature = ordered_members(env,save,true)?;
        let value = from_rust_string(env,signature);
        let _: () = msg![env; save setPropertyValue:value forKey:key];
        let _: () = msg![env; value release];
        log!("Expedition lineup: recorded {}",trial.stages[stage]);
    }
    None
}