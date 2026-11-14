/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! ARC entry points, using the documented non-optimized return-value fallback.
//!
//! No caller-instruction handoff is attempted: autoreleaseReturnValue always
//! enqueues, and retainAutoreleasedReturnValue always retains. Both sides must
//! use this same fallback for ownership to balance.
//! https://clang.llvm.org/docs/AutomaticReferenceCounting.html#runtime-support

use super::{autorelease, id, msg, msg_class, release, retain};
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{MutPtr, MutVoidPtr};
use crate::Environment;

fn objc_retain(env: &mut Environment, value: id) -> id {
    retain(env, value)
}
fn objc_retainBlock(env: &mut Environment, value: id) -> id {
    crate::libc::blocks::_Block_copy(env, value.cast()).cast()
}

fn objc_release(env: &mut Environment, value: id) {
    release(env, value)
}

fn objc_autorelease(env: &mut Environment, value: id) -> id {
    autorelease(env, value)
}

fn objc_autoreleaseReturnValue(env: &mut Environment, value: id) -> id {
    autorelease(env, value)
}

fn objc_retainAutoreleasedReturnValue(env: &mut Environment, value: id) -> id {
    retain(env, value)
}

fn objc_retainAutorelease(env: &mut Environment, value: id) -> id {
    let value = retain(env, value);
    autorelease(env, value)
}

fn objc_retainAutoreleaseReturnValue(env: &mut Environment, value: id) -> id {
    objc_retainAutorelease(env, value)
}

fn objc_unsafeClaimAutoreleasedReturnValue(_env: &mut Environment, value: id) -> id {
    // There was no optimized ownership handoff to claim or release.
    value
}

fn objc_storeStrong(env: &mut Environment, slot: MutPtr<id>, value: id) {
    let previous = env.mem.read(slot);
    let value = retain(env, value);
    env.mem.write(slot, value);
    // May run arbitrary guest dealloc code; the new value must be visible first.
    release(env, previous);
}

fn objc_autoreleasePoolPush(env: &mut Environment) -> MutVoidPtr {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    pool.cast_void()
}

fn objc_autoreleasePoolPop(env: &mut Environment, pool: MutVoidPtr) {
    let pool: id = pool.cast();
    msg![env; pool drain]
}

pub(super) const FUNCTIONS: FunctionExports = &[
    export_c_func!(objc_retain(_)),
    export_c_func!(objc_retainBlock(_)),
    export_c_func!(objc_release(_)),
    export_c_func!(objc_autorelease(_)),
    export_c_func!(objc_autoreleaseReturnValue(_)),
    export_c_func!(objc_retainAutoreleasedReturnValue(_)),
    export_c_func!(objc_retainAutorelease(_)),
    export_c_func!(objc_retainAutoreleaseReturnValue(_)),
    export_c_func!(objc_unsafeClaimAutoreleasedReturnValue(_)),
    export_c_func!(objc_storeStrong(_, _)),
    export_c_func!(objc_autoreleasePoolPush()),
    export_c_func!(objc_autoreleasePoolPop(_)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use crate::objc::{nil, HostObject};
    use crate::options::Options;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    struct LifetimeProbe(Arc<AtomicUsize>);
    impl HostObject for LifetimeProbe {}
    impl Drop for LifetimeProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn object(env: &mut Environment, drops: &Arc<AtomicUsize>) -> id {
        let class = env.objc.get_known_class("NSObject", &mut env.mem);
        env.objc
            .alloc_object(class, Box::new(LifetimeProbe(drops.clone())), &mut env.mem)
    }

    #[test]
    fn arc_ownership_and_nested_pools() {
        let options = Options {
            headless: true,
            ..Default::default()
        };
        let mut environment =
            Environment::new_without_app(options, Image::from_pixel_vec(vec![0; 4], (1, 1)))
                .unwrap();
        let env = &mut environment;
        let drops = Arc::new(AtomicUsize::new(0));

        assert_eq!(objc_retain(env, nil), nil);
        assert_eq!(objc_autoreleaseReturnValue(env, nil), nil);
        assert_eq!(objc_retainAutoreleasedReturnValue(env, nil), nil);
        objc_release(env, nil);

        let outer = objc_autoreleasePoolPush(env);
        let value = object(env, &drops);
        assert_eq!(objc_autoreleaseReturnValue(env, value), value);
        assert_eq!(objc_retainAutoreleasedReturnValue(env, value), value);
        assert_eq!(env.objc.get_refcount(value).get(), 2);
        let inner = objc_autoreleasePoolPush(env);
        let temporary = object(env, &drops);
        objc_autorelease(env, temporary);
        objc_autoreleasePoolPop(env, outer); // also drains the still-open inner
        assert!(env.objc.get_host_object(inner.cast()).is_none());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(env.objc.get_refcount(value).get(), 1);
        objc_release(env, value);
        assert_eq!(drops.load(Ordering::SeqCst), 2);

        let value = object(env, &drops);
        let slot: MutPtr<id> = env.mem.alloc(4).cast();
        env.mem.write(slot, nil);
        objc_storeStrong(env, slot, value);
        objc_release(env, value);
        objc_storeStrong(env, slot, value); // self-assignment at sole ownership
        assert_eq!(env.objc.get_refcount(value).get(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        objc_storeStrong(env, slot, nil);
        assert_eq!(env.mem.read(slot), nil);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
        env.mem.free(slot.cast());
    }
}
