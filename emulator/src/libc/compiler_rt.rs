/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Compiler-emitted integer helpers, using their 32-bit Darwin ARM ABI.
use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;

fn unsigned_remainder(dividend: u32, divisor: u32) -> u32 {
    assert_ne!(divisor, 0, "__umodsi3 division by zero");
    dividend % divisor
}

fn __umodsi3(_env: &mut Environment, dividend: u32, divisor: u32) -> u32 {
    unsigned_remainder(dividend, divisor)
}

fn unsigned_division(dividend: u32, divisor: u32) -> u32 {
    assert_ne!(divisor, 0, "__udivsi3 division by zero");
    dividend / divisor
}

fn __udivsi3(_env: &mut Environment, dividend: u32, divisor: u32) -> u32 {
    unsigned_division(dividend, divisor)
}

fn signed_division(dividend: i32, divisor: i32) -> i32 {
    assert_ne!(divisor, 0, "__divsi3 division by zero");
    // compiler-rt computes the unsigned magnitude and restores the sign;
    // MIN / -1 therefore preserves the low 32 bits instead of panicking.
    dividend.wrapping_div(divisor)
}

fn __divsi3(_env: &mut Environment, dividend: i32, divisor: i32) -> i32 {
    signed_division(dividend, divisor)
}

fn signed_remainder(dividend: i32, divisor: i32) -> i32 {
    assert_ne!(divisor, 0, "__modsi3 division by zero");
    // The remainder has the dividend's sign. Like compiler-rt's unsigned
    // magnitude calculation, MIN % -1 yields zero without signed overflow.
    dividend.wrapping_rem(divisor)
}

fn __modsi3(_env: &mut Environment, dividend: i32, divisor: i32) -> i32 {
    signed_remainder(dividend, divisor)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(__umodsi3(_, _)),
    export_c_func!(__udivsi3(_, _)),
    export_c_func!(__divsi3(_, _)),
    export_c_func!(__modsi3(_, _)),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_remainder_keeps_dividend_sign_and_handles_minimum() {
        for (a, b, expected) in [
            (19, 4, 3),
            (-19, 4, -3),
            (19, -4, 3),
            (-19, -4, -3),
            (-1, 2, -1),
            (0, -3, 0),
            (i32::MIN, 1, 0),
            (i32::MIN, -1, 0),
            (i32::MIN, 3, -2),
            (i32::MIN, i32::MAX, -1),
            (i32::MAX, i32::MIN, i32::MAX),
        ] {
            assert_eq!(signed_remainder(a, b), expected);
        }
        assert!(std::panic::catch_unwind(|| signed_remainder(7, 0)).is_err());
    }
    #[test]
    fn signed_division_truncates_towards_zero_and_preserves_arm_width() {
        for (a, b, expected) in [
            (19, 4, 4),
            (-19, 4, -4),
            (19, -4, -4),
            (-19, -4, 4),
            (-1, 2, 0),
            (0, -3, 0),
            (i32::MIN, 1, i32::MIN),
            (i32::MIN, -1, i32::MIN),
            (i32::MIN, i32::MAX, -1),
        ] {
            assert_eq!(signed_division(a, b), expected);
        }
        assert!(std::panic::catch_unwind(|| signed_division(7, 0)).is_err());
    }
    #[test]
    fn unsigned_division_high_bit_exact_and_truncation() {
        for (a, b, expected) in [
            (0, 3, 0),
            (7, 9, 0),
            (19, 4, 4),
            (u32::MAX, 16, 0x0fff_ffff),
            (u32::MAX, 0x8000_0000, 1),
            (0x8000_0000, 3, 715827882),
            (u32::MAX, 1, u32::MAX),
        ] {
            assert_eq!(unsigned_division(a, b), expected);
        }
        assert!(std::panic::catch_unwind(|| unsigned_division(7, 0)).is_err());
    }
    #[test]
    fn unsigned_remainder_preserves_high_bit_and_rejects_zero_division() {
        for (a, b, expected) in [
            (0, 3, 0),
            (7, 9, 7),
            (19, 4, 3),
            (u32::MAX, 16, 15),
            (u32::MAX, u32::MAX, 0),
            (u32::MAX, 0x8000_0000, 0x7fff_ffff),
            (0x8000_0000, 3, 2),
            (u32::MAX, 1, 0),
        ] {
            assert_eq!(unsigned_remainder(a, b), expected);
        }
        assert!(std::panic::catch_unwind(|| unsigned_remainder(7, 0)).is_err());
    }
}
