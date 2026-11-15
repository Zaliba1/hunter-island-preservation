/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Optional encounter rule for the four early starter dragon tiles in Hunter Island 2.02.
use crate::abi::GuestFunction;
use crate::cpu::Cpu;
use crate::frameworks::foundation::ns_string::{from_rust_string, to_rust_string};
use crate::libc::stdlib::arc4random;
use crate::mem::Ptr;
use crate::objc::{autorelease, id, msg, nil};
use crate::Environment;

const WEIGHTED_DRAW: u32 = 0x169ec1;
const RANDOM_PACK_CALL: u32 = 0x8cdc3;
const CALL_SITE: [u8; 20] = [
    0x11, 0x99, 0x96, 0xf2, 0x46, 0xef, 0x12, 0x99, 0x4a, 0xf2,
    0x52, 0x22, 0xc0, 0xf2, 0x33, 0x02, 0x82, 0x46, 0x7a, 0x44,
];

fn is_starter(name: &str) -> bool {
    matches!(name, "Earthdragon01" | "Firedragon01" | "Waterdragon01" | "Winddragon01")
}

fn draw_index(first: bool, roll: u32) -> u32 {
    // 3 of 60 buckets place the dragon in the first slot. All other slots
    // choose among the three equally weighted original companions.
    if first && roll < 3 { 3 } else { roll % 3 }
}

const CAPTURE_STATUS: u32 = 0xbc045;
const CAPTURE_PREFIX: [u8; 20] = [
    128, 181, 77, 242, 92, 1, 111, 70, 192, 242,
    49, 1, 121, 68, 9, 104, 65, 92, 65, 177,
];

fn tiamat_status(name: &str, category: u32, capturable: bool, attempted: bool, status: u32) -> Option<u32> {
    if status == 2 && category == 1 && capturable
        && matches!(name, "Boss.Tiamat" | "Tiamat" | "Monster.Boss.Tiamat")
    {
        Some(u32::from(attempted))
    } else {
        None
    }
}

pub(super) fn after_capture_status(env: &mut Environment, monster: id, imp: GuestFunction) {
    if !env.options.hunter_test_tiamat || !env.options.hunter_test_tiamat_capture
        || imp.addr_with_thumb_bit() != CAPTURE_STATUS
        || env.cpu.regs()[0] != 2
        || env.bundle.bundle_identifier() != "pockettrend.hunterisland"
        || env.bundle.bundle_version() != "2.02"
        || env.mem.get_bytes_fallible(Ptr::from_bits(CAPTURE_STATUS & !1), 20) != Some(&CAPTURE_PREFIX)
        || env.mem.get_bytes_fallible(Ptr::from_bits(0x3d90b0), 4) != Some(&0x18cu32.to_le_bytes())
        || env.mem.get_bytes_fallible(Ptr::from_bits(0x3d90c4), 4) != Some(&0x184u32.to_le_bytes())
    {
        return;
    }
    let Some(data) = env.mem.get_bytes_fallible(Ptr::from_bits(monster.to_bits().wrapping_add(0x184)), 9) else { return; };
    let category = u32::from_le_bytes(data[..4].try_into().unwrap());
    let capturable = data[8] != 0;
    let regs = *env.cpu.regs();
    let flags = env.cpu.cpsr();
    let name: id = msg![env; monster instanceName];
    let name = to_rust_string(env, name);
    let key = from_rust_string(env, "captureAttemptMade".to_owned());
    let key = autorelease(env, key);
    let attempt: id = msg![env; monster valueForProperty:key];
    let attempted: bool = msg![env; attempt boolValue];
    *env.cpu.regs_mut() = regs;
    env.cpu.set_cpsr(flags);
    if let Some(status) = tiamat_status(&name, category, capturable, attempted, regs[0]) {
        env.cpu.regs_mut()[0] = status;
        log!("Hunter Island Tiamat capture test: name={name}, category={category}, capturable={capturable}, attempted={attempted}, status=2->{status}; original chance calculation retained");
    } else {
        log!("Hunter Island Tiamat capture test unchanged: name={name}, category={category}, capturable={capturable}, attempted={attempted}");
    }
}

pub(super) fn try_draw(env: &mut Environment, list: id, imp: GuestFunction) -> bool {
    if imp.addr_with_thumb_bit() != WEIGHTED_DRAW
        || env.cpu.regs()[Cpu::LR] != RANDOM_PACK_CALL
        || env.bundle.bundle_identifier() != "pockettrend.hunterisland"
        || env.bundle.bundle_version() != "2.02"
        || env.mem.get_bytes_fallible(Ptr::from_bits(0x8cdbc), CALL_SITE.len() as u32)
            != Some(CALL_SITE.as_slice())
    {
        return false;
    }
    // Nested guest messages can modify the caller's argument registers.
    // The ordinary weighted draw needs the original receiver and selector
    // when this list is outside the four target fields.
    let regs = *env.cpu.regs();
    let flags = env.cpu.cpsr();
    let replacement = draw_for_starter_list(env, list);
    *env.cpu.regs_mut() = regs;
    env.cpu.set_cpsr(flags);
    if let Some(monster) = replacement {
        env.cpu.regs_mut()[0] = monster.to_bits();
        true
    } else {
        false
    }
}

fn draw_for_starter_list(env: &mut Environment, list: id) -> Option<id> {
    let values: id = msg![env; list values];
    let count: u32 = msg![env; values count];
    if count != 4 {
        return None;
    }
    let entries: [id; 4] = std::array::from_fn(|i| msg![env; values objectAtIndex:(i as u32)]);
    for (entry, expected) in entries[..3].iter().zip(["Windbird01", "Birdblue01", "Crab01"]) {
        if to_rust_string(env, *entry) != expected {
            return None;
        }
    }
    let dragon = to_rust_string(env, entries[3]);
    if !is_starter(&dragon) {
        return None;
    }
    let test_monster = if dragon != "Earthdragon01" {
        None
    } else if env.options.hunter_test_tiamat {
        Some("Tiamat")
    } else if env.options.hunter_test_orbling {
        Some("Crystaldragon01")
    } else {
        None
    };
    if !env.options.hunter_rare_dragons && test_monster.is_none() {
        return None;
    }

    // At this verified call site, the guest keeps its current EncounterData at
    // SP + 0x64. Its monsters array is still empty before the first draw.
    let frame = env.cpu.regs()[Cpu::SP];
    let Some(bytes) = env.mem.get_bytes_fallible(Ptr::from_bits(frame.wrapping_add(0x64)), 4) else {
        return None;
    };
    let encounter = id::from_bits(u32::from_le_bytes(bytes.try_into().unwrap()));
    if encounter == nil || env.objc.get_host_object(encounter).is_none() {
        return None;
    }
    let monsters: id = msg![env; encounter monsters];
    let already_drawn: u32 = msg![env; monsters count];
    if let Some(monster) = test_monster {
        // Change only the first member of an ordinary wild group. The
        // separate Tiamat capture-category diagnostic is applied below.
        if already_drawn == 0 {
            let name = from_rust_string(env, monster.to_owned());
            log!("Hunter Island encounter test: Leafy pool (over-28x92), first slot {monster}, original capture rules");
            return Some(autorelease(env, name));
        }
        return None;
    }

    // arc4random produces the nonzero u32 range. Reject the partial bucket
    // before taking a remainder so that each of the 60 buckets is equal.
    let roll = loop {
        let value = arc4random(env) - 1;
        if value < (u32::MAX / 60) * 60 {
            break value % 60;
        }
    };
    let index = draw_index(already_drawn == 0, roll);
    if already_drawn == 0 && (index == 3 || env.options.capture_frames) {
        log!("Hunter Island rare dragons: tile dragon={dragon}, selected={}", index == 3);
    }
    Some(entries[index as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiamat_category_override_preserves_other_capture_restrictions() {
        assert_eq!(tiamat_status("Boss.Tiamat", 1, true, false, 2), Some(0));
        assert_eq!(tiamat_status("Boss.Tiamat", 1, true, true, 2), Some(1));
        assert_eq!(tiamat_status("Dragon.Crystaldragon01", 1, true, false, 2), None);
        assert_eq!(tiamat_status("Boss.Tiamat", 1, false, false, 2), None);
        assert_eq!(tiamat_status("Boss.Tiamat", 0, true, false, 2), None);
        for status in [0, 1, 3] {
            assert_eq!(tiamat_status("Boss.Tiamat", 1, true, false, status), None);
        }
    }

    #[test]
    fn five_percent_per_pack_for_each_supported_size() {
        for size in 1..=3 {
            let hit_count = (0..60)
                .filter(|&roll| (0..size).any(|slot| draw_index(slot == 0, roll) == 3))
                .count();
            assert_eq!(hit_count, 3);
        }
        for slot in 0..3 {
            assert_eq!((0..60).filter(|&roll| draw_index(false, roll) == slot).count(), 20);
        }
    }
}
