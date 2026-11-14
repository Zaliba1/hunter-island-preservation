/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGGeometry.h` (`CGPoint`, `CGSize`, `CGRect`, etc)
//!
//! See also [crate::frameworks::uikit::ui_geometry].

use std::ops::{Add, Mul, Sub};

use super::CGFloat;
use crate::abi::{impl_GuestRet_for_large_struct, GuestArg};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::SafeRead;
use crate::Environment;

fn parse_tuple(s: &str) -> Result<(f32, f32), ()> {
    let (a, b) = s.split_once(',').ok_or(())?;
    Ok((
        a.trim().parse().map_err(|_| ())?,
        b.trim().parse().map_err(|_| ())?,
    ))
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}
unsafe impl SafeRead for CGPoint {}
impl_GuestRet_for_large_struct!(CGPoint);
impl GuestArg for CGPoint {
    const REG_COUNT: usize = 2;

    fn from_regs(regs: &[u32]) -> Self {
        CGPoint {
            x: GuestArg::from_regs(&regs[0..1]),
            y: GuestArg::from_regs(&regs[1..2]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.x.to_regs(&mut regs[0..1]);
        self.y.to_regs(&mut regs[1..2]);
    }
}
impl std::str::FromStr for CGPoint {
    type Err = ();
    fn from_str(s: &str) -> Result<CGPoint, ()> {
        let s = s
            .trim()
            .strip_prefix('{')
            .ok_or(())?
            .strip_suffix('}')
            .ok_or(())?;
        let (x, y) = parse_tuple(s)?;
        Ok(CGPoint { x, y })
    }
}
impl std::fmt::Display for CGPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let &CGPoint { x, y } = self;
        write!(f, "{{{x}, {y}}}")
    }
}
// Implemented to aid animation code.
// Theres are the operations needed for the interpolation.
impl Mul<f32> for CGPoint {
    type Output = CGPoint;

    fn mul(self, rhs: f32) -> Self::Output {
        CGPoint {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}
impl Add<CGPoint> for CGPoint {
    type Output = CGPoint;

    fn add(self, rhs: CGPoint) -> Self::Output {
        CGPoint {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}
impl Sub<CGPoint> for CGPoint {
    type Output = CGPoint;

    fn sub(self, rhs: CGPoint) -> Self::Output {
        CGPoint {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
// This function is rare because it is usually inlined.
fn CGPointEqualToPoint(_env: &mut Environment, a: CGPoint, b: CGPoint) -> bool {
    a == b
}

pub const CGPointZero: CGPoint = CGPoint { x: 0.0, y: 0.0 };

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}
unsafe impl SafeRead for CGSize {}
impl_GuestRet_for_large_struct!(CGSize);
impl GuestArg for CGSize {
    const REG_COUNT: usize = 2;

    fn from_regs(regs: &[u32]) -> Self {
        CGSize {
            width: GuestArg::from_regs(&regs[0..1]),
            height: GuestArg::from_regs(&regs[1..2]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.width.to_regs(&mut regs[0..1]);
        self.height.to_regs(&mut regs[1..2]);
    }
}
impl std::str::FromStr for CGSize {
    type Err = ();
    fn from_str(s: &str) -> Result<CGSize, ()> {
        let s = s
            .trim()
            .strip_prefix('{')
            .ok_or(())?
            .strip_suffix('}')
            .ok_or(())?;
        let (w, h) = parse_tuple(s)?;
        Ok(CGSize {
            width: w,
            height: h,
        })
    }
}
impl std::fmt::Display for CGSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let &CGSize { width, height } = self;
        write!(f, "{{{width}, {height}}}")
    }
}
// Implemented to aid animation code.
// Theres are the operations needed for the interpolation.
impl Mul<f32> for CGSize {
    type Output = CGSize;

    fn mul(self, rhs: f32) -> Self::Output {
        CGSize {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}
impl Add<CGSize> for CGSize {
    type Output = CGSize;

    fn add(self, rhs: CGSize) -> Self::Output {
        CGSize {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}
impl Sub<CGSize> for CGSize {
    type Output = CGSize;

    fn sub(self, rhs: CGSize) -> Self::Output {
        CGSize {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}
// This function is rare because it is usually inlined.
fn CGSizeEqualToSize(_env: &mut Environment, a: CGSize, b: CGSize) -> bool {
    a == b
}

pub const CGSizeZero: CGSize = CGSize {
    width: 0.0,
    height: 0.0,
};

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}
unsafe impl SafeRead for CGRect {}
impl_GuestRet_for_large_struct!(CGRect);
impl GuestArg for CGRect {
    const REG_COUNT: usize = 4;

    fn from_regs(regs: &[u32]) -> Self {
        CGRect {
            origin: GuestArg::from_regs(&regs[0..2]),
            size: GuestArg::from_regs(&regs[2..4]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.origin.to_regs(&mut regs[0..2]);
        self.size.to_regs(&mut regs[2..4]);
    }
}
impl std::str::FromStr for CGRect {
    type Err = ();
    fn from_str(s: &str) -> Result<CGRect, ()> {
        let s = s
            .trim()
            .strip_prefix('{')
            .ok_or(())?
            .strip_suffix('}')
            .ok_or(())?
            .trim();
        let first_end = s.find('}').ok_or(())?;
        let origin: CGPoint = s[..=first_end].parse()?;
        let size: CGSize = s[first_end + 1..]
            .trim_start()
            .strip_prefix(',')
            .ok_or(())?
            .parse()?;
        Ok(CGRect { origin, size })
    }
}
impl std::fmt::Display for CGRect {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let &CGRect { origin, size } = self;
        write!(f, "{{{origin}, {size}}}")
    }
}
// Implemented to aid animation code.
// Theres are the operations needed for the interpolation.
impl Mul<f32> for CGRect {
    type Output = CGRect;

    fn mul(self, rhs: f32) -> Self::Output {
        CGRect {
            origin: self.origin * rhs,
            size: self.size * rhs,
        }
    }
}
impl Add<CGRect> for CGRect {
    type Output = CGRect;

    fn add(self, rhs: CGRect) -> Self::Output {
        CGRect {
            origin: self.origin + rhs.origin,
            size: self.size + rhs.size,
        }
    }
}
impl Sub<CGRect> for CGRect {
    type Output = CGRect;

    fn sub(self, rhs: CGRect) -> Self::Output {
        CGRect {
            origin: self.origin - rhs.origin,
            size: self.size - rhs.size,
        }
    }
}
// This function is rare because it is usually inlined.
fn CGRectEqualToRect(_env: &mut Environment, a: CGRect, b: CGRect) -> bool {
    a == b
}

pub const CGRectZero: CGRect = CGRect {
    origin: CGPointZero,
    size: CGSizeZero,
};

fn standardize(r: CGRect) -> CGRect {
    let x = r.origin.x.min(r.origin.x + r.size.width);
    let y = r.origin.y.min(r.origin.y + r.size.height);
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize {
            width: r.size.width.abs(),
            height: r.size.height.abs(),
        },
    }
}

fn rect_union(a: CGRect, b: CGRect) -> CGRect {
    if a == CGRectNull {
        return if b == CGRectNull {
            CGRectNull
        } else {
            standardize(b)
        };
    }
    if b == CGRectNull {
        return standardize(a);
    }
    let a = standardize(a);
    let b = standardize(b);
    let x = a.origin.x.min(b.origin.x);
    let y = a.origin.y.min(b.origin.y);
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize {
            width: (a.origin.x + a.size.width).max(b.origin.x + b.size.width) - x,
            height: (a.origin.y + a.size.height).max(b.origin.y + b.size.height) - y,
        },
    }
}

fn CGRectUnion(_env: &mut Environment, a: CGRect, b: CGRect) -> CGRect {
    rect_union(a, b)
}

#[cfg(test)]
mod rect_union_tests {
    use super::*;
    fn rect(x: f32, y: f32, width: f32, height: f32) -> CGRect {
        CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        }
    }
    #[test]
    fn union_contains_both_rectangles_and_standardizes_negative_extents() {
        let a = rect(5.0, 6.0, -4.0, -4.0);
        let b = rect(-2.0, 4.0, 3.0, 9.0);
        assert_eq!(rect_union(a, b), rect(-2.0, 2.0, 7.0, 11.0));
        assert_eq!(rect_union(b, a), rect_union(a, b));
        assert_eq!(rect_union(a, a), rect(1.0, 2.0, 4.0, 4.0));
        assert_eq!(
            rect_union(rect(0.0, 0.0, 3.0, 3.0), rect(3.0, 3.0, 2.0, 2.0)),
            rect(0.0, 0.0, 5.0, 5.0)
        );
    }
    #[test]
    fn null_is_identity_and_zero_is_a_real_origin() {
        let a = rect(2.0, 3.0, 4.0, 5.0);
        assert_eq!(rect_union(CGRectNull, a), a);
        assert_eq!(rect_union(a, CGRectNull), a);
        assert_eq!(rect_union(CGRectNull, CGRectNull), CGRectNull);
        assert_eq!(rect_union(CGRectZero, a), rect(0.0, 0.0, 6.0, 8.0));
    }
}

fn rect_contains_point(rect: CGRect, point: CGPoint) -> bool {
    // CGGeometry APIs implicitly standardize negative extents. In particular,
    // rotated sprites may pass a rectangle with a negative width or height.
    if rect == CGRectNull {
        return false;
    }
    let rect = standardize(rect);
    rect.origin.x <= point.x
        && rect.origin.x + rect.size.width > point.x
        && rect.origin.y <= point.y
        && rect.origin.y + rect.size.height > point.y
}

fn CGRectContainsPoint(env: &mut Environment, rect: CGRect, point: CGPoint) -> bool {
    let result = rect_contains_point(rect, point);
    if env.options.capture_frames && (rect.size.width < 0.0 || rect.size.height < 0.0) {
        log!("CGRECT_NEGATIVE_EXTENT_HIT_TEST: rect={rect} point={point} inside={result}");
    }
    result
}

#[cfg(test)]
mod rect_contains_tests {
    use super::*;

    #[test]
    fn equivalent_rectangles_with_negative_extents_have_the_same_membership() {
        for (x, width) in [(10.0, 20.0), (30.0, -20.0)] {
            for (y, height) in [(40.0, 60.0), (100.0, -60.0)] {
                let rect = CGRect {
                    origin: CGPoint { x, y },
                    size: CGSize { width, height },
                };
                for (point, expected) in [
                    (CGPoint { x: 10.0, y: 40.0 }, true),
                    (CGPoint { x: 20.0, y: 70.0 }, true),
                    (CGPoint { x: 30.0, y: 70.0 }, false),
                    (CGPoint { x: 20.0, y: 100.0 }, false),
                    (CGPoint { x: 9.0, y: 70.0 }, false),
                ] {
                    assert_eq!(rect_contains_point(rect, point), expected);
                }
            }
        }
    }

    #[test]
    fn null_and_empty_rectangles_do_not_contain_points() {
        for rect in [
            CGRectNull,
            CGRectZero,
            CGRect {
                origin: CGPoint { x: 10.0, y: 20.0 },
                size: CGSize {
                    width: -10.0,
                    height: 0.0,
                },
            },
        ] {
            assert!(!rect_contains_point(rect, CGPointZero));
        }
    }
}

fn CGRectIntersectsRect(_env: &mut Environment, rect1: CGRect, rect2: CGRect) -> bool {
    rect1.origin.x.max(rect2.origin.x)
        <= (rect1.origin.x + rect1.size.width).min(rect2.origin.x + rect2.size.width)
        && rect1.origin.y.max(rect2.origin.y)
            <= (rect1.origin.y + rect1.size.height).min(rect2.origin.y + rect2.size.height)
}

pub(super) fn CGRectIntersection(_env: &mut Environment, rect1: CGRect, rect2: CGRect) -> CGRect {
    if rect1 == CGRectNull || rect2 == CGRectNull {
        return CGRectNull;
    }
    assert!(rect1.size.height > 0.0 && rect1.size.width > 0.0); // TODO
    assert!(rect2.size.height > 0.0 && rect2.size.width > 0.0); // TODO
    let x = rect1.origin.x.max(rect2.origin.x);
    let y = rect1.origin.y.max(rect2.origin.y);
    let width = (rect1.origin.x + rect1.size.width).min(rect2.origin.x + rect2.size.width) - x;
    let height = (rect1.origin.y + rect1.size.height).min(rect2.origin.y + rect2.size.height) - y;
    if width < 0.0 || height < 0.0 {
        return CGRectNull;
    }
    assert!(height != 0.0 || width != 0.0); // TODO
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

fn CGRectGetMinX(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.x
}

fn CGRectGetMidX(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.x + rect.size.width / 2.0
}

fn CGRectGetMaxX(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.x + rect.size.width
}

fn CGRectGetMinY(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.y
}

fn CGRectGetMidY(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.y + rect.size.height / 2.0
}

fn CGRectGetMaxY(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.y + rect.size.height
}

fn CGRectGetHeight(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.size.height
}

fn CGRectGetWidth(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.size.width
}

fn CGRectMake(
    _env: &mut Environment,
    x: CGFloat,
    y: CGFloat,
    width: CGFloat,
    height: CGFloat,
) -> CGRect {
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

pub const CGRectNull: CGRect = CGRect {
    origin: CGPoint {
        x: f32::INFINITY,
        y: f32::INFINITY,
    },
    size: CGSizeZero,
};

fn CGRectIsNull(_env: &mut Environment, rect: CGRect) -> bool {
    rect == CGRectNull
}

fn CGRectOffset(_env: &mut Environment, rect: CGRect, dx: CGFloat, dy: CGFloat) -> CGRect {
    assert!(rect != CGRectNull); // TODO
    CGRect {
        origin: CGPoint {
            x: rect.origin.x + dx,
            y: rect.origin.y + dy,
        },
        size: rect.size,
    }
}

fn CGRectInset(_env: &mut Environment, rect: CGRect, dx: CGFloat, dy: CGFloat) -> CGRect {
    let res = CGRect {
        origin: CGPoint {
            x: rect.origin.x + dx,
            y: rect.origin.y + dy,
        },
        size: CGSize {
            width: rect.size.width - 2.0 * dx,
            height: rect.size.height - 2.0 * dy,
        },
    };
    assert!(res.size.width >= 0.0); // TODO return a null rectangle
    assert!(res.size.height >= 0.0); // TODO return a null rectangle

    // center invariant
    assert!(rect.origin.x + rect.size.width / 2.0 == res.origin.x + res.size.width / 2.0);
    assert!(rect.origin.y + rect.size.height / 2.0 == res.origin.y + res.size.height / 2.0);
    res
}

pub(super) fn CGRectIntegral(_env: &mut Environment, rect: CGRect) -> CGRect {
    if rect == CGRectNull {
        return rect;
    }
    assert!(
        rect.size.width >= 0.0 && rect.size.height >= 0.0,
        "unexpected {}",
        rect
    );
    let new_x = rect.origin.x.floor();
    let new_y = rect.origin.y.floor();
    let new_width = (rect.origin.x + rect.size.width).ceil() - new_x;
    let new_height = (rect.origin.y + rect.size.height).ceil() - new_y;
    CGRect {
        origin: CGPoint { x: new_x, y: new_y },
        size: CGSize {
            width: new_width,
            height: new_height,
        },
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGPointEqualToPoint(_, _)),
    export_c_func!(CGSizeEqualToSize(_, _)),
    export_c_func!(CGRectEqualToRect(_, _)),
    export_c_func!(CGRectContainsPoint(_, _)),
    export_c_func!(CGRectIntersectsRect(_, _)),
    export_c_func!(CGRectIntersection(_, _)),
    export_c_func!(CGRectUnion(_, _)),
    export_c_func!(CGRectGetMinX(_)),
    export_c_func!(CGRectGetMidX(_)),
    export_c_func!(CGRectGetMaxX(_)),
    export_c_func!(CGRectGetMinY(_)),
    export_c_func!(CGRectGetMidY(_)),
    export_c_func!(CGRectGetMaxY(_)),
    export_c_func!(CGRectGetHeight(_)),
    export_c_func!(CGRectGetWidth(_)),
    export_c_func!(CGRectMake(_, _, _, _)),
    export_c_func!(CGRectIsNull(_)),
    export_c_func!(CGRectOffset(_, _, _)),
    export_c_func!(CGRectInset(_, _, _)),
    export_c_func!(CGRectIntegral(_)),
];

pub const CONSTANTS: ConstantExports = &[
    (
        "_CGSizeZero",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGSizeZero).cast().cast_const()),
    ),
    (
        "_CGPointZero",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGPointZero).cast().cast_const()),
    ),
    (
        "_CGRectZero",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGRectZero).cast().cast_const()),
    ),
    (
        "_CGRectNull",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGRectNull).cast().cast_const()),
    ),
];

#[cfg(test)]
mod string_geometry_tests {
    use super::*;
    #[test]
    fn compact_and_spaced_geometry_including_signs_and_exponents() {
        for text in [
            "{300,120}",
            "{300, 120}",
            "  { 300 , 120 }  ",
            "{3e2,+1.2e2}",
        ] {
            assert_eq!(
                text.parse::<CGSize>(),
                Ok(CGSize {
                    width: 300.0,
                    height: 120.0
                })
            );
        }
        assert_eq!(
            "{-.25, +.5}".parse::<CGPoint>(),
            Ok(CGPoint { x: -0.25, y: 0.5 })
        );
        for text in [
            "{{1,2},{3,4}}",
            "{{1, 2}, {3, 4}}",
            " { { 1 , 2 } , { 3 , 4 } } ",
        ] {
            assert_eq!(
                text.parse::<CGRect>(),
                Ok(CGRect {
                    origin: CGPoint { x: 1.0, y: 2.0 },
                    size: CGSize {
                        width: 3.0,
                        height: 4.0
                    }
                })
            );
        }
    }
    #[test]
    fn malformed_geometry_does_not_silently_drop_fields_or_suffixes() {
        for text in [
            "",
            "1,2",
            "{1}",
            "{1,}",
            "{,2}",
            "{1,2,3}",
            "{1,2}tail",
            "{1x,2}",
        ] {
            assert!(text.parse::<CGPoint>().is_err(), "{text}");
            assert!(text.parse::<CGSize>().is_err(), "{text}");
        }
        for text in [
            "{{1,2},{3,4}",
            "{{1,2}{3,4}}",
            "{{1,2},{3,4},{5,6}}",
            "{{1,2},{3,4}}tail",
        ] {
            assert!(text.parse::<CGRect>().is_err(), "{text}");
        }
    }
}
