/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Darwin's process-wide stack guard and fatal failure entrypoint.
//! ABI reference: apple-oss-distributions/Libc/sys/OpenBSD/stack_protector.c.
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::{ConstVoidPtr, MutPtr};
use crate::Environment;

#[derive(Default)]
pub struct State {
    guard: Option<MutPtr<u32>>,
}

fn guard(env: &mut Environment) -> ConstVoidPtr {
    if let Some(guard) = env.libc_state.stack_protector.guard {
        return guard.cast_void().cast_const();
    }
    // Eight 32-bit Darwin longs. Use the random portion of UUID v4 (which
    // obtains OS randomness); do not use the emulated deterministic rand().
    let guard: MutPtr<u32> = env.mem.alloc(8 * 4).cast();
    for index in 0..8 {
        let random = uuid::Uuid::new_v4();
        let word = u32::from_le_bytes(random.as_bytes()[..4].try_into().unwrap()).max(1);
        env.mem.write(guard + index, word);
    }
    env.libc_state.stack_protector.guard = Some(guard);
    guard.cast_void().cast_const()
}

fn __stack_chk_fail(_env: &mut Environment) {
    panic!("Guest stack protector detected stack corruption; terminating emulation");
}

pub const CONSTANTS: ConstantExports = &[("___stack_chk_guard", HostConstant::Custom(guard))];
pub const FUNCTIONS: FunctionExports = &[export_c_func!(__stack_chk_fail())];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stack_guard_is_stable_and_failure_is_fatal() {
        let mut environment = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut environment;
        let first = guard(env);
        let value = env.mem.bytes_at(first.cast(), 32).to_vec();
        let _unrelated = env.mem.alloc(32);
        assert_eq!(first, guard(env));
        assert_eq!(env.mem.bytes_at(first.cast(), 32), value);
        assert_ne!(env.mem.read(first.cast::<u32>()), 0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| __stack_chk_fail(env)))
                .is_err()
        );
    }
}
