// SPDX-License-Identifier: MPL-2.0
//! Accelerate consecutive identity-only misses in the retail Cocos2d array search.
//! Custom equality, hits, mixed classes and the loop exit still execute in the guest.
use super::{id, Class, ClassHostObject, ObjC, TrivialHostObject, IMP, SEL};
use crate::{mem::Ptr, Environment};
use sha2::{Digest, Sha256};

pub(crate) const SEARCH_RETURN: u32 = 0x6a8a3;
const MAX_BATCH: u32 = 64;
const LOOP_HASH: [u8; 32] = [
    0xd5, 0xe7, 0xe9, 0x6d, 0x49, 0xfa, 0x4f, 0x3f, 0x20, 0x32, 0xe8, 0x10, 0x7b, 0x88, 0x2d, 0xa2,
    0x86, 0xc6, 0xeb, 0xc3, 0xee, 0x24, 0xd5, 0x72, 0xe7, 0x9e, 0x9a, 0x65, 0x3b, 0x54, 0x88, 0xa4,
];

fn inherits_object_equality(objc: &ObjC, mut class: Class, selector: SEL) -> bool {
    while !class.is_null() {
        let Some(meta) = objc
            .get_host_object(class)
            .and_then(|o| o.as_any().downcast_ref::<ClassHostObject>())
        else {
            return false;
        };
        if let Some(method) = meta.methods.get(&selector) {
            return meta.name == "NSObject"
                && !meta.is_metaclass
                && meta.superclass.is_null()
                && matches!(method, IMP::Host(_));
        }
        class = meta.superclass;
    }
    false
}

fn word(env: &Environment, address: u32) -> Option<u32> {
    let bytes = env.mem.get_bytes_fallible(Ptr::from_bits(address), 4)?;
    Some(u32::from_le_bytes(bytes.try_into().unwrap()))
}

// Called only by the default NSObject implementation on a miss. Verify both
// the complete search routine and the live register layout before changing
// its index. No scan result survives a guest call or a change of array.
pub(crate) fn skip_misses(env: &mut Environment, receiver: id, selector: SEL, target: id) {
    let r = env.cpu.regs();
    let (index, array) = (r[4], r[6]);
    if env.is_debugging_enabled()
        || env.bundle.bundle_identifier() != "pockettrend.hunterisland"
        || env.bundle.bundle_version() != "2.02"
        || r[0] != receiver.to_bits()
        || r[1] != selector.to_bits()
        || r[5] != r[1]
        || r[2] != target.to_bits()
        || r[8] != r[2]
        || selector.as_str(&env.mem) != "isEqual:"
    {
        return;
    }
    let Some(code) = env.mem.get_bytes_fallible(Ptr::from_bits(0x6a874), 0x48) else {
        return;
    };
    if Sha256::digest(code)[..] != LOOP_HASH {
        return;
    }
    let class = ObjC::read_isa(receiver, &env.mem);
    if !inherits_object_equality(&env.objc, class, selector) {
        return;
    }
    let Some(count) = word(env, array) else {
        return;
    };
    let Some(items) = array.checked_add(8).and_then(|p| word(env, p)) else {
        return;
    };
    let Some(current) = index.checked_mul(4).and_then(|n| items.checked_add(n)) else {
        return;
    };
    let available = count.saturating_sub(index).min(MAX_BATCH);
    if available < 2 {
        return;
    }
    let Some(slice) = env
        .mem
        .get_bytes_fallible(Ptr::from_bits(current), available * 4)
    else {
        return;
    };
    if slice[..4] != receiver.to_bits().to_le_bytes() {
        return;
    }
    let mut skipped = 0;
    for slot in slice[4..].chunks_exact(4) {
        let candidate = id::from_bits(u32::from_le_bytes(slot.try_into().unwrap()));
        if candidate == target
            || !env
                .objc
                .get_host_object(candidate)
                .is_some_and(|o| o.as_any().is::<TrivialHostObject>())
            || ObjC::read_isa(candidate, &env.mem) != class
        {
            break;
        }
        skipped += 1;
    }
    if skipped != 0 {
        env.cpu.regs_mut()[4] = index + skipped;
        log_once!("Hunter Island array search: batching verified object identity comparisons");
    }
}
