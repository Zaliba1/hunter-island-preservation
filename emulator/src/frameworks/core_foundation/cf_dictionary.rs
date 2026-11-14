/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFDictionary` and `CFMutableDictionary`.
//!
//! These are toll-free bridged to `NSDictionary` and `NSMutableDictionary` in
//! Apple's implementation. Here they are the same types.

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::{CFHashCode, CFIndex, CFRelease, CFRetain};
use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{
    export_c_func, ConstantExports, Dyld, FunctionExports, HostConstant, HostFunction,
};
use crate::frameworks::core_foundation::cf_string::CFStringRef;
use crate::frameworks::core_foundation::cf_type::{CFEqual, CFHash};
use crate::frameworks::foundation::ns_dictionary::{
    CFDictionaryKeyCallBacks, CFDictionaryValueCallBacks,
};
use crate::frameworks::foundation::NSUInteger;
use crate::mem::{ConstPtr, ConstVoidPtr, Mem, MutVoidPtr};
use crate::objc::{id, msg, msg_class, nil};
use crate::Environment;

pub type CFDictionaryRef = super::CFTypeRef;
pub type CFMutableDictionaryRef = super::CFTypeRef;

fn CFDictionaryCreateMutable(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    capacity: CFIndex,
    key_callbacks: ConstPtr<CFDictionaryKeyCallBacks>,
    value_callbacks: ConstPtr<CFDictionaryValueCallBacks>,
) -> CFMutableDictionaryRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    assert_eq!(capacity, 0); // TODO: fixed capacity support

    let new = msg_class![env; _touchHLE_NSMutableDictionary_non_retaining alloc];
    msg![env; new initWithKeyCallbacks:key_callbacks andValueCallbacks:value_callbacks]
}

fn CFDictionaryAddValue(
    env: &mut Environment,
    dict: CFMutableDictionaryRef,
    key: ConstVoidPtr,
    value: ConstVoidPtr,
) {
    let key: id = key.cast().cast_mut();
    let res: id = msg![env; dict objectForKey:key];
    log_dbg!(
        "CFDictionaryAddValue dict {:?} k {:?} v {:?}; res {:?}",
        dict,
        key,
        value,
        res
    );
    if res == nil {
        let value: id = value.cast().cast_mut();
        msg![env; dict setObject:value forKey:key]
    }
}

fn CFDictionarySetValue(
    env: &mut Environment,
    dict: CFMutableDictionaryRef,
    key: ConstVoidPtr,
    value: ConstVoidPtr,
) {
    log_dbg!("CFDictionarySetValue k {:?} v {:?}", key, value);
    let key: id = key.cast().cast_mut();
    let value: id = value.cast().cast_mut();
    msg![env; dict setObject:value forKey:key]
}

fn CFDictionaryRemoveValue(env: &mut Environment, dict: CFMutableDictionaryRef, key: ConstVoidPtr) {
    let key: id = key.cast().cast_mut();
    log_dbg!("CFDictionaryRemoveValue dict {:?} key {:?}", dict, key);
    () = msg![env; dict removeObjectForKey:key];
}

fn CFDictionaryRemoveAllValues(env: &mut Environment, dict: CFMutableDictionaryRef) {
    // TODO: use keyEnumerator
    let keys_arr: id = msg![env; dict allKeys];
    let enumerator: id = msg![env; keys_arr objectEnumerator];
    let mut key: id;
    loop {
        key = msg![env; enumerator nextObject];
        if key == nil {
            break;
        }
        CFDictionaryRemoveValue(env, dict, key.cast().cast_const());
    }
}

fn CFDictionaryGetValue(
    env: &mut Environment,
    dict: CFMutableDictionaryRef,
    key: ConstVoidPtr,
) -> ConstVoidPtr {
    let key: id = key.cast().cast_mut();
    let res: id = msg![env; dict objectForKey:key];
    res.cast().cast_const()
}

fn CFDictionaryGetCount(env: &mut Environment, dict: CFDictionaryRef) -> CFIndex {
    let count: NSUInteger = msg![env; dict count];
    log_dbg!("CFDictionaryGetCount dict {:?} {}", dict, count);
    count.try_into().unwrap()
}

fn CFDictionaryApplyFunction(
    env: &mut Environment,
    dict: CFDictionaryRef,
    applier: GuestFunction,
    context: MutVoidPtr,
) {
    // Use the bridged collection primitives, including for dictionaries with
    // NULL callbacks whose keys/values are opaque pointers, not ObjC objects.
    // Copy pointer pairs before entering guest code: no host borrow may span
    // the callback, which can query this dictionary or enumerate another one.
    // As in CF, mutation of the enumerated dictionary is unsupported.
    let keys: id = msg![env; dict allKeys];
    let count: NSUInteger = msg![env; keys count];
    let mut entries = Vec::with_capacity(count as usize);
    for index in 0..count {
        let key: id = msg![env; keys objectAtIndex:index];
        let value: id = msg![env; dict objectForKey:key];
        entries.push((key.cast_const().cast_void(), value.cast_const().cast_void()));
    }
    log!("CFDictionaryApplyFunction: entries={count}");
    for (key, value) in entries {
        let () = applier.call_from_host(env, (key, value, context));
    }
}

fn CFDictionaryGetKeysAndValues(
    env: &mut Environment,
    dict: CFDictionaryRef,
    keys: ConstPtr<MutVoidPtr>,
    values: ConstPtr<MutVoidPtr>,
) {
    let mut key_ptr = keys.cast_mut();
    let mut val_ptr = values.cast_mut();
    // TODO: use keyEnumerator
    let keys_arr: id = msg![env; dict allKeys];
    let enumerator: id = msg![env; keys_arr objectEnumerator];
    let mut key: id;
    let mut val: id;
    loop {
        key = msg![env; enumerator nextObject];
        if key == nil {
            break;
        }
        if !key_ptr.is_null() {
            env.mem.write(key_ptr, key.cast());
            key_ptr += 1;
        }
        if !val_ptr.is_null() {
            val = msg![env; dict objectForKey:key];
            log_dbg!(
                "CFDictionaryGetKeysAndValues dict {:?} key {:?} val {:?}",
                dict,
                key,
                val
            );
            env.mem.write(val_ptr, val.cast());
            val_ptr += 1;
        }
    }
}

// Default CFDictionary callbacks
fn _touchHLE_CFDictionary_retain(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    value: ConstVoidPtr,
) -> ConstVoidPtr {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    CFRetain(env, value.cast_mut().cast()).cast_const().cast()
}
fn _touchHLE_CFDictionary_release(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    value: ConstVoidPtr,
) {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    CFRelease(env, value.cast_mut().cast());
}
fn _touchHLE_CFDictionary_copyDescription(
    _env: &mut Environment,
    _value: ConstVoidPtr,
) -> CFStringRef {
    todo!()
}
fn _touchHLE_CFDictionary_equal(
    env: &mut Environment,
    value1: ConstVoidPtr,
    value2: ConstVoidPtr,
) -> bool {
    CFEqual(env, value1.cast_mut().cast(), value2.cast_mut().cast())
}
fn _touchHLE_CFDictionary_hash(env: &mut Environment, value: ConstVoidPtr) -> CFHashCode {
    CFHash(env, value.cast_mut().cast())
}

struct DefaultCallbackFunctions {
    retain: GuestFunction,
    release: GuestFunction,
    copy_desc: GuestFunction,
    equal: GuestFunction,
    hash: GuestFunction,
}
fn create_default_callback_functions(mem: &mut Mem, dyld: &mut Dyld) -> DefaultCallbackFunctions {
    let retain_sym = "__touchHLE_CFDictionary_retain";
    let retain_hf: HostFunction =
        &(_touchHLE_CFDictionary_retain as fn(&mut Environment, _, _) -> _);
    let retain_gf = dyld.create_guest_function(mem, retain_sym, retain_hf);

    let release_sym = "__touchHLE_CFDictionary_release";
    let release_hf: HostFunction = &(_touchHLE_CFDictionary_release as fn(&mut Environment, _, _));
    let release_gf = dyld.create_guest_function(mem, release_sym, release_hf);

    let copy_desc_sym = "__touchHLE_CFDictionary_copyDescription";
    let copy_desc_hf: HostFunction =
        &(_touchHLE_CFDictionary_copyDescription as fn(&mut Environment, _) -> _);
    let copy_desc_gf = dyld.create_guest_function(mem, copy_desc_sym, copy_desc_hf);

    let equal_sym = "__touchHLE_CFDictionary_equal";
    let equal_hf: HostFunction = &(_touchHLE_CFDictionary_equal as fn(&mut Environment, _, _) -> _);
    let equal_gf = dyld.create_guest_function(mem, equal_sym, equal_hf);

    let hash_sym = "__touchHLE_CFDictionary_hash";
    let hash_hf: HostFunction = &(_touchHLE_CFDictionary_hash as fn(&mut Environment, _) -> _);
    let hash_gf = dyld.create_guest_function(mem, hash_sym, hash_hf);

    DefaultCallbackFunctions {
        retain: retain_gf,
        release: release_gf,
        copy_desc: copy_desc_gf,
        equal: equal_gf,
        hash: hash_gf,
    }
}

pub const CONSTANTS: ConstantExports = &[
    (
        "_kCFTypeDictionaryKeyCallBacks",
        HostConstant::Custom(|env| {
            let common = create_default_callback_functions(&mut env.mem, &mut env.dyld);
            let callbacks = CFDictionaryKeyCallBacks {
                version: 0, // always 0
                retain: common.retain,
                release: common.release,
                copy_desc: common.copy_desc,
                equal: common.equal,
                hash: common.hash,
            };
            env.mem.alloc_and_write(callbacks).cast_void().cast_const()
        }),
    ),
    (
        "_kCFTypeDictionaryValueCallBacks",
        HostConstant::Custom(|env| {
            // All the functions here (except `hash` one)
            // are the same as for `kCFTypeDictionaryKeyCallBacks`,
            // but we still re-create guest functions for the sake
            // of the (current) code simplicity
            // TODO: create related guest functions only once, not twice
            let common = create_default_callback_functions(&mut env.mem, &mut env.dyld);
            let callbacks = CFDictionaryValueCallBacks {
                version: 0, // always 0
                retain: common.retain,
                release: common.release,
                copy_desc: common.copy_desc,
                equal: common.equal,
            };
            env.mem.alloc_and_write(callbacks).cast_void().cast_const()
        }),
    ),
];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFDictionaryCreateMutable(_, _, _, _)),
    export_c_func!(CFDictionaryAddValue(_, _, _)),
    export_c_func!(CFDictionarySetValue(_, _, _)),
    export_c_func!(CFDictionaryRemoveValue(_, _)),
    export_c_func!(CFDictionaryRemoveAllValues(_)),
    export_c_func!(CFDictionaryGetValue(_, _)),
    export_c_func!(CFDictionaryGetCount(_)),
    export_c_func!(CFDictionaryApplyFunction(_, _, _)),
    export_c_func!(CFDictionaryGetKeysAndValues(_, _, _)),
];

fn probe_reentrant_applier(
    env: &mut Environment,
    key: ConstVoidPtr,
    value: ConstVoidPtr,
    context: MutVoidPtr,
) {
    let context = context.cast::<u32>();
    let dictionary = id::from_bits(env.mem.read(context));
    assert_eq!(CFDictionaryGetValue(env, dictionary, key), value);
    assert_eq!(CFDictionaryGetCount(env, dictionary), 3);
    let visits = env.mem.read(context + 1);
    env.mem.write(context + 1, visits + 1);
    let nested = GuestFunction::from_addr_with_thumb_bit(env.mem.read(context + 2));
    let nested_context = MutVoidPtr::from_bits(env.mem.read(context + 3));
    CFDictionaryApplyFunction(env, dictionary, nested, nested_context);
}

pub(crate) fn apply_function_probe(env: &mut Environment) {
    use crate::frameworks::foundation::{ns_dictionary, ns_string};
    use crate::objc::release;
    let pool: id = msg_class![env; NSAutoreleasePool new];
    // Own Thumb callback: append (r0=key,r1=value) to r2=context and increment
    // its count. This tests the actual guest callback ABI, not a Rust closure.
    let code = env.mem.alloc(18);
    env.mem.bytes_at_mut(code.cast(), 18).copy_from_slice(&[
        0x13, 0x68, 0xdb, 0x00, 0xd3, 0x18, 0x58, 0x60, 0x99, 0x60, 0x13, 0x68, 0x01, 0x33, 0x13,
        0x60, 0x70, 0x47,
    ]);
    let applier = GuestFunction::from_addr_and_thumb_flag(code.to_bits(), true);
    let apply = env.dyld.create_guest_function(
        &mut env.mem,
        "_CFDictionaryApplyFunction",
        &(CFDictionaryApplyFunction as fn(&mut Environment, _, _, _)),
    );
    let raw = CFDictionaryCreateMutable(
        env,
        kCFAllocatorDefault,
        0,
        ConstPtr::null(),
        ConstPtr::null(),
    );
    let context = env.mem.calloc(4 + 9 * 8).cast::<u32>();
    let () = apply.call_from_host(env, (raw, applier, context.cast_void()));
    assert_eq!(env.mem.read(context), 0);
    let expected = [(0x101u32, 0x909u32), (0x202, 0), (0x303, 0x909)];
    for (key, value) in expected {
        CFDictionarySetValue(
            env,
            raw,
            ConstVoidPtr::from_bits(key),
            ConstVoidPtr::from_bits(value),
        );
    }
    let () = apply.call_from_host(env, (raw, applier, context.cast_void()));
    assert_eq!(env.mem.read(context), 3);
    let mut actual: Vec<_> = (0..3)
        .map(|n| {
            (
                env.mem.read(context + 1 + n * 2),
                env.mem.read(context + 2 + n * 2),
            )
        })
        .collect();
    actual.sort_unstable();
    assert_eq!(actual, expected);

    let reentrant = env.dyld.create_guest_function(
        &mut env.mem,
        "_touchHLE_CFDictionary_probe_reentrant",
        &(probe_reentrant_applier as fn(&mut Environment, _, _, _)),
    );
    let state = env.mem.calloc(16).cast::<u32>();
    env.mem.write(state, raw.to_bits());
    env.mem.write(state + 2, applier.addr_with_thumb_bit());
    env.mem.write(state + 3, context.to_bits());
    env.mem.write(context, 0);
    let () = apply.call_from_host(env, (raw, reentrant, state.cast_void()));
    assert_eq!(env.mem.read(state + 1), 3);
    assert_eq!(env.mem.read(context), 9);

    let key = ns_string::from_rust_string(env, "opaque-context-check".into());
    let value: id = msg_class![env; NSNumber numberWithUnsignedInt:42u32];
    let immutable = ns_dictionary::dict_from_keys_and_objects(env, &[(key, value)]);
    env.mem.write(context, 0);
    let () = apply.call_from_host(env, (immutable, applier, context.cast_void()));
    assert_eq!(env.mem.read(context), 1);
    assert_eq!(env.mem.read(context + 1), key.to_bits());
    assert_eq!(env.mem.read(context + 2), value.to_bits());
    // NULL context is legal: this own callback simply returns, touching none
    // of the opaque pointers or context. An empty dictionary never calls it.
    let no_op = GuestFunction::from_addr_and_thumb_flag(code.to_bits() + 16, true);
    let () = apply.call_from_host(env, (raw, no_op, MutVoidPtr::null()));
    for object in [immutable, key, raw] {
        release(env, object);
    }
    release(env, pool);
    env.mem.free(context.cast());
    env.mem.free(state.cast());
    env.mem.free(code);
    echo!("CFDICTIONARY_APPLY_THUMB_ABI_OPAQUE_NULL_VALUES_CONTEXT_REENTRANT_AND_BRIDGING_PASS");
}

#[cfg(test)]
mod tests {
    #[test]
    fn apply_function_passes_opaque_pairs_context_and_allows_reentrant_reads() {
        let env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.run_framework_probe(super::apply_function_probe);
    }
}
