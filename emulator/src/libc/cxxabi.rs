/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `cxxabi.h`
//!
//! Resources:
//! - [Itanium C++ ABI specification](https://itanium-cxx-abi.github.io/cxx-abi/abi.html#dso-dtor-runtime-api)

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{Mem, MutVoidPtr};
use crate::{Environment, ThreadId};
use std::collections::HashMap;

/// SjLj frame registration only. Raising/resuming exceptions is still unsupported.
#[derive(Default)]
pub(super) struct State {
    sjlj_top: HashMap<ThreadId, MutVoidPtr>,
}

impl State {
    fn register(&mut self, mem: &mut Mem, thread: ThreadId, context: MutVoidPtr) {
        assert!(!context.is_null());
        let previous = self.sjlj_top.get(&thread).copied().unwrap_or_default();
        assert_ne!(previous, context, "SjLj frame registered twice");
        // First ARM ABI field is the previous function-context pointer.
        mem.write(context.cast::<MutVoidPtr>(), previous);
        self.sjlj_top.insert(thread, context);
    }

    fn unregister(&mut self, mem: &Mem, thread: ThreadId, context: MutVoidPtr) {
        assert_eq!(
            self.sjlj_top.get(&thread),
            Some(&context),
            "SjLj frames must unregister in stack order"
        );
        let previous: MutVoidPtr = mem.read(context.cast());
        if previous.is_null() {
            self.sjlj_top.remove(&thread);
        } else {
            self.sjlj_top.insert(thread, previous);
        }
    }
}

fn _Unwind_SjLj_Register(env: &mut Environment, context: MutVoidPtr) {
    let thread = env.current_thread;
    env.libc_state
        .cxxabi
        .register(&mut env.mem, thread, context);
}

fn _Unwind_SjLj_Unregister(env: &mut Environment, context: MutVoidPtr) {
    let thread = env.current_thread;
    env.libc_state.cxxabi.unregister(&env.mem, thread, context);
}

fn __cxa_atexit(
    _env: &mut Environment,
    func: GuestFunction, // void (*func)(void *)
    p: MutVoidPtr,
    d: MutVoidPtr,
) -> i32 {
    // TODO: when this is implemented, make sure it's properly compatible with
    // C atexit.
    log!(
        "TODO: __cxa_atexit({:?}, {:?}, {:?}) (unimplemented)",
        func,
        p,
        d
    );
    0 // success
}

fn __cxa_finalize(_env: &mut Environment, d: MutVoidPtr) {
    log!("TODO: __cxa_finalize({:?}) (unimplemented)", d);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(_Unwind_SjLj_Register(_)),
    export_c_func!(_Unwind_SjLj_Unregister(_)),
    export_c_func!(__cxa_atexit(_, _, _)),
    export_c_func!(__cxa_finalize(_)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sjlj_frames_are_nested_and_thread_local() {
        let mut state = State::default();
        let mut mem = Mem::new();
        mem.reserve(0, 4096);
        let a = mem.alloc(44);
        let b = mem.alloc(44);
        let c = mem.alloc(44);
        state.register(&mut mem, 0, a);
        state.register(&mut mem, 1, b);
        state.register(&mut mem, 0, c);
        assert_eq!(mem.read(c.cast::<MutVoidPtr>()), a);
        assert!(mem.read(b.cast::<MutVoidPtr>()).is_null());
        state.unregister(&mem, 0, c);
        assert_eq!(state.sjlj_top.get(&0), Some(&a));
        state.unregister(&mem, 1, b);
        assert!(!state.sjlj_top.contains_key(&1));
        state.unregister(&mem, 0, a);
        assert!(state.sjlj_top.is_empty());
        for p in [a, b, c] {
            mem.free(p);
        }
    }
}
