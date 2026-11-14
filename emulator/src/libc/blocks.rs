/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! 32-bit Apple Blocks ABI, with guest copy/dispose/invoke callbacks.
//! References: Clang Block-ABI-Apple and compiler-rt BlocksRuntime.
use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::{MutPtr, MutVoidPtr, SafeRead};
use crate::objc::{id, msg_class, nil, objc_classes, release, retain, Class, ClassExports, SEL};
use crate::Environment;
use std::collections::HashSet;

pub const NEEDS_FREE: u32 = 1 << 24;
pub const HAS_HELPERS: u32 = 1 << 25;
pub const IS_GLOBAL: u32 = 1 << 28;
const REFCOUNT: u32 = 0xffff;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Header {
    pub isa: Class,
    pub flags: u32,
    pub reserved: u32,
    pub invoke: GuestFunction,
    pub descriptor: MutPtr<u32>,
}
unsafe impl SafeRead for Header {}

#[derive(Default)]
pub struct State {
    classes: [Option<Class>; 3],
    pub(super) heap_blocks: HashSet<MutVoidPtr>,
    pub(super) heap_byrefs: HashSet<MutVoidPtr>,
}
pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);
    @implementation _NSConcreteStackBlock: NSObject
    @end
    @implementation _NSConcreteMallocBlock: NSObject
    @end
    @implementation _NSConcreteGlobalBlock: NSObject
    @end
};
fn class(env: &mut Environment, kind: usize) -> Class {
    if let Some(class) = env.libc_state.blocks.classes[kind] {
        return class;
    }
    let name = [
        "_NSConcreteStackBlock",
        "_NSConcreteMallocBlock",
        "_NSConcreteGlobalBlock",
    ][kind];
    let class = env.objc.get_known_class(name, &mut env.mem);
    env.libc_state.blocks.classes[kind] = Some(class);
    class
}
pub fn is_block(env: &Environment, value: id) -> bool {
    if value == nil {
        return false;
    }
    let isa: Class = env.mem.read(value.cast());
    env.libc_state.blocks.classes.contains(&Some(isa))
}
fn change_count(env: &mut Environment, flags: MutPtr<u32>, increment: bool) -> bool {
    let value = env.mem.read(flags);
    let count = value & REFCOUNT;
    if count == REFCOUNT {
        return false;
    } // ABI saturated refcounts are immortal
    let count = if increment {
        count.checked_add(1).unwrap()
    } else {
        count.checked_sub(1).expect("Block refcount underflow")
    };
    env.mem.write(flags, (value & !REFCOUNT) | count);
    count == 0
}
pub fn _Block_copy(env: &mut Environment, value: MutVoidPtr) -> MutVoidPtr {
    if value.is_null() {
        return value;
    }
    let header = env.mem.read(value.cast::<Header>());
    assert_eq!(header.flags & (1 << 27), 0, "GC Blocks are unsupported");
    if header.flags & IS_GLOBAL != 0 {
        return value;
    }
    if header.flags & NEEDS_FREE != 0 {
        assert!(env.libc_state.blocks.heap_blocks.contains(&value));
        change_count(env, value.cast::<u32>() + 1, true);
        return value;
    }
    let size = env.mem.read(header.descriptor + 1);
    assert!(
        (20..=16 * 1024 * 1024).contains(&size),
        "Invalid Block descriptor size"
    );
    let bytes = env.mem.bytes_at(value.cast(), size).to_vec();
    let copy = env.mem.alloc(size);
    env.mem
        .bytes_at_mut(copy.cast(), size)
        .copy_from_slice(&bytes);
    let mut copied = header;
    copied.flags = (header.flags & !REFCOUNT) | NEEDS_FREE | 1;
    copied.isa = class(env, 1);
    env.mem.write(copy.cast(), copied);
    env.libc_state.blocks.heap_blocks.insert(copy);
    if header.flags & HAS_HELPERS != 0 {
        let helper = env
            .mem
            .read((header.descriptor + 2).cast::<GuestFunction>());
        let (): () = helper.call_from_host(env, (copy, value));
    }
    copy
}
pub fn _Block_release(env: &mut Environment, value: MutVoidPtr) {
    if value.is_null() {
        return;
    }
    let header = env.mem.read(value.cast::<Header>());
    if header.flags & IS_GLOBAL != 0 || header.flags & NEEDS_FREE == 0 {
        return;
    }
    assert!(env.libc_state.blocks.heap_blocks.contains(&value));
    if !change_count(env, value.cast::<u32>() + 1, false) {
        return;
    }
    if header.flags & HAS_HELPERS != 0 {
        let helper = env
            .mem
            .read((header.descriptor + 3).cast::<GuestFunction>());
        let (): () = helper.call_from_host(env, (value,));
    }
    assert!(env.libc_state.blocks.heap_blocks.remove(&value));
    env.mem.free(value);
}
pub fn invoke(env: &mut Environment, value: MutVoidPtr) {
    let header = env.mem.read(value.cast::<Header>());
    let invoke = header.invoke;
    let (): () = invoke.call_from_host(env, (value,));
}
fn copy_byref(env: &mut Environment, value: MutVoidPtr) -> MutVoidPtr {
    let original = value.cast::<u32>();
    let forwarding: MutPtr<u32> = env.mem.read((original + 1).cast());
    let flags = env.mem.read(forwarding + 2);
    if flags & NEEDS_FREE != 0 {
        assert!(env
            .libc_state
            .blocks
            .heap_byrefs
            .contains(&forwarding.cast()));
        change_count(env, forwarding + 2, true);
        return forwarding.cast();
    }
    let size = env.mem.read(forwarding + 3);
    assert!((16..=16 * 1024 * 1024).contains(&size));
    let bytes = env.mem.bytes_at(forwarding.cast(), size).to_vec();
    let copy: MutPtr<u32> = env.mem.alloc(size).cast();
    env.mem
        .bytes_at_mut(copy.cast(), size)
        .copy_from_slice(&bytes);
    env.mem.write((copy + 1).cast(), copy);
    env.mem.write((original + 1).cast(), copy);
    env.mem.write((forwarding + 1).cast(), copy);
    env.mem
        .write(copy + 2, (flags & !REFCOUNT) | NEEDS_FREE | 2); // stack + heap capture
    env.libc_state.blocks.heap_byrefs.insert(copy.cast());
    if flags & HAS_HELPERS != 0 {
        assert!(size >= 24);
        let helper = env.mem.read((forwarding + 4).cast::<GuestFunction>());
        let (): () = helper.call_from_host(env, (copy.cast_void(), forwarding.cast_void()));
    }
    copy.cast()
}
fn release_byref(env: &mut Environment, value: MutVoidPtr) {
    let forwarding: MutPtr<u32> = env.mem.read((value.cast::<u32>() + 1).cast());
    let flags = env.mem.read(forwarding + 2);
    if flags & NEEDS_FREE == 0 {
        return;
    }
    assert!(env
        .libc_state
        .blocks
        .heap_byrefs
        .contains(&forwarding.cast()));
    if !change_count(env, forwarding + 2, false) {
        return;
    }
    if flags & HAS_HELPERS != 0 {
        let helper = env.mem.read((forwarding + 5).cast::<GuestFunction>());
        let (): () = helper.call_from_host(env, (forwarding.cast_void(),));
    }
    env.libc_state.blocks.heap_byrefs.remove(&forwarding.cast());
    env.mem.free(forwarding.cast());
}
pub fn _Block_object_assign(
    env: &mut Environment,
    destination: MutPtr<MutVoidPtr>,
    object: MutVoidPtr,
    flags: i32,
) {
    assert_eq!(
        flags & 16,
        0,
        "Weak Block captures require zeroing-weak support"
    );
    let value = if flags & 128 != 0 {
        object
    } else if flags & 8 != 0 {
        if object.is_null() {
            object
        } else {
            copy_byref(env, object)
        }
    } else if flags & 7 == 7 {
        _Block_copy(env, object)
    } else if flags & 3 == 3 {
        retain(env, object.cast()).cast()
    } else {
        panic!("Unsupported Block capture flags {flags:#x}")
    };
    env.mem.write(destination, value);
}
pub fn _Block_object_dispose(env: &mut Environment, object: MutVoidPtr, flags: i32) {
    assert_eq!(
        flags & 16,
        0,
        "Weak Block captures require zeroing-weak support"
    );
    if object.is_null() {
        return;
    }
    if flags & 8 != 0 {
        release_byref(env, object);
    } else if flags & 128 != 0 { /* non-owning MRC byref helper assignment */
    } else if flags & 7 == 7 {
        _Block_release(env, object);
    } else if flags & 3 == 3 {
        release(env, object.cast());
    } else {
        panic!("Unsupported Block capture flags {flags:#x}");
    }
}

/// Raw compiler-emitted Blocks have no HostObjectEntry. Handle their NSObject
/// surface before the ordinary message dispatcher dereferences that entry.
pub fn message(env: &mut Environment, receiver: id, selector: SEL) -> bool {
    if !is_block(env, receiver) {
        return false;
    }
    let name = selector.as_str(&env.mem).to_owned();
    let value = receiver.cast_void();
    let result = match name.as_str() {
        "copy" | "copyWithZone:" => _Block_copy(env, value).to_bits(),
        "retain" => {
            let header = env.mem.read(value.cast::<Header>());
            if header.flags & NEEDS_FREE != 0 {
                change_count(env, value.cast::<u32>() + 1, true);
            }
            value.to_bits()
        }
        "release" => {
            _Block_release(env, value);
            0
        }
        "autorelease" => {
            let (): () = msg_class![env; NSAutoreleasePool addObject:receiver];
            value.to_bits()
        }
        "class" => env.mem.read(value.cast::<Class>()).to_bits(),
        "hash" => value.to_bits(),
        "isEqual:" => u32::from(env.cpu.regs()[2] == receiver.to_bits()),
        "isKindOfClass:" => {
            let requested = Class::from_bits(env.cpu.regs()[2]);
            u32::from(
                env.objc
                    .class_is_subclass_of(env.mem.read(value.cast()), requested),
            )
        }
        "retainCount" => {
            let flags = env.mem.read(value.cast::<u32>() + 1);
            if flags & IS_GLOBAL != 0 {
                u32::MAX
            } else {
                (flags & REFCOUNT).max(1)
            }
        }
        _ => panic!("Unsupported Block message {name}"),
    };
    env.cpu.regs_mut()[0] = result;
    true
}
pub const CONSTANTS: ConstantExports = &[
    (
        "__NSConcreteStackBlock",
        HostConstant::Custom(|env| class(env, 0).cast_void().cast_const()),
    ),
    (
        "__NSConcreteMallocBlock",
        HostConstant::Custom(|env| class(env, 1).cast_void().cast_const()),
    ),
    (
        "__NSConcreteGlobalBlock",
        HostConstant::Custom(|env| class(env, 2).cast_void().cast_const()),
    ),
];
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(_Block_copy(_)),
    export_c_func!(_Block_release(_)),
    export_c_func!(_Block_object_assign(_, _, _)),
    export_c_func!(_Block_object_dispose(_, _)),
];
