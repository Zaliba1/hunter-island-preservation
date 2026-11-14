/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Exact six-instruction Thumb ivar getter shortcut. No address/name whitelist,
//! guest-code rewrite or mutable-code cache. Recheck all instructions each call.
use crate::{abi::GuestFunction, mem::Ptr, Environment};

fn ivar_slot(code: &[u8], pc: u32) -> Option<u32> {
    let words: [u16; 8] =
        std::array::from_fn(|i| u16::from_le_bytes([code[i * 2], code[i * 2 + 1]]));
    if words[0] & 0xfbf0 != 0xf240
        || words[1] & 0x8f00 != 0x0100
        || words[2] & 0xfbf0 != 0xf2c0
        || words[3] & 0x8f00 != 0x0100
        || words[4..] != [0x4479, 0x6809, 0x5840, 0x4770]
    {
        return None;
    }
    let imm = |a: u16, b: u16| {
        u32::from(((a & 15) << 12) | ((a & 0x400) << 1) | ((b & 0x7000) >> 4) | (b & 255))
    };
    // ADD r1, pc uses this instruction's address + 4, without alignment.
    Some(
        (imm(words[0], words[1]) | (imm(words[2], words[3]) << 16))
            .wrapping_add(pc.wrapping_add(12)),
    )
}

pub(super) fn try_call(env: &mut Environment, imp: GuestFunction) -> bool {
    // Keep instruction stepping/breakpoints and unusual IT state on the CPU.
    if !imp.is_thumb() || env.is_debugging_enabled() || env.cpu.cpsr() & 0x0700_fe00 != 0 {
        return false;
    }
    let pc = imp.addr_without_thumb_bit();
    // No speculative decode fault may replace the normal CPU error path.
    let Some(code) = env.mem.get_bytes_fallible(Ptr::from_bits(pc), 16) else {
        return false;
    };
    let Some(slot) = ivar_slot(code, pc) else {
        return false;
    };
    let Some(bytes) = env.mem.get_bytes_fallible(Ptr::from_bits(slot), 4) else {
        return false;
    };
    let offset = u32::from_le_bytes(bytes.try_into().unwrap());
    let addr = env.cpu.regs()[0].wrapping_add(offset);
    // Preserve native handling for unaligned/invalid loads as well.
    if slot & 3 != 0 || addr & 3 != 0 {
        return false;
    }
    let Some(bytes) = env.mem.get_bytes_fallible(Ptr::from_bits(addr), 4) else {
        return false;
    };
    let value = u32::from_le_bytes(bytes.try_into().unwrap());
    // MOVW/MOVT, ADD-high, LDR and BX do not alter NZCV. The normal
    // call_without_pushing_stack_frame restores its original PC/LR/Thumb state.
    env.cpu.regs_mut()[0] = value;
    env.cpu.regs_mut()[1] = offset;
    #[cfg(feature = "guest-cpu-profile")]
    crate::cpu::guest_profile::record_leaf_getter();
    true
}

pub(super) fn probe(env: &mut Environment) {
    let saved = *env.cpu.regs();
    let saved_cpsr = env.cpu.cpsr();
    let code = env.mem.alloc(32);
    let object = env.mem.alloc(512);
    let pc = code.to_bits();
    let slot = pc + 16;
    let delta = slot.wrapping_sub(pc + 12);
    let mov = |base: u16, imm: u16| {
        [
            base | ((imm >> 1) & 0x400) | (imm >> 12),
            0x0100 | ((imm & 0x700) << 4) | (imm & 255),
        ]
    };
    let low = mov(0xf240, delta as u16);
    let high = mov(0xf2c0, (delta >> 16) as u16);
    let words = [
        low[0], low[1], high[0], high[1], 0x4479, 0x6809, 0x5840, 0x4770,
    ];
    for (i, w) in words.into_iter().enumerate() {
        env.mem.write((code.cast::<u16>()) + i as u32, w);
    }
    let imp = GuestFunction::from_addr_and_thumb_flag(pc, true);
    env.cpu.invalidate_cache_range(pc, 32);
    for i in 0..512u32 {
        let offset = (i % 128) * 4;
        let value = i.wrapping_mul(0x9e3779b1);
        env.mem.write(Ptr::<u32, true>::from_bits(slot), offset);
        env.mem.write(object.cast::<u32>() + offset / 4, value);
        let initial: [u32; 16] = std::array::from_fn(|r| (r as u32).wrapping_mul(0x1020304) ^ i);
        *env.cpu.regs_mut() = saved;
        env.cpu.regs_mut()[..13].copy_from_slice(&initial[..13]);
        env.cpu.regs_mut()[0] = object.to_bits();
        env.cpu
            .set_cpsr((saved_cpsr & !0xf600_fc00) | ((i & 15) << 28));
        let before = *env.cpu.regs();
        let cpsr = env.cpu.cpsr();
        imp.call_without_pushing_stack_frame(env);
        let expected = *env.cpu.regs();
        let expected_cpsr = env.cpu.cpsr();
        *env.cpu.regs_mut() = before;
        env.cpu.set_cpsr(cpsr);
        assert!(try_call(env, imp));
        assert_eq!(*env.cpu.regs(), expected, "getter registers case {i}");
        assert_eq!(env.cpu.cpsr(), expected_cpsr, "getter flags case {i}");
    }
    // Mutable ivar slots are re-read. Altering an instruction rejects the shortcut.
    env.mem.write(code.cast::<u16>() + 7, 0x4778u16);
    let before = *env.cpu.regs();
    assert!(!try_call(env, imp));
    assert_eq!(*env.cpu.regs(), before);
    *env.cpu.regs_mut() = saved;
    env.cpu.set_cpsr(saved_cpsr);
    env.mem.free(object);
    env.mem.free(code);
    echo!("THUMB_LEAF_GETTER_512_NATIVE_DIFFERENTIAL_AND_MUTATION_PASS");
}
