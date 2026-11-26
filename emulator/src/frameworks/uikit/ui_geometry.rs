//! `UIGeometry.h`
//!
//! See also [crate::frameworks::core_graphics::cg_geometry].

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string;
use crate::objc::{autorelease, id, nil};
use crate::Environment;

// Apple's documentation says all of these return zeroes if the input is not
// well-formed.
pub fn CGPointFromString(env: &mut Environment, string: id) -> CGPoint {
    if string == nil {
        return CGPoint::default();
    }
    let text = ns_string::to_rust_string(env, string);
    if let Some(point) = hunter_warp_position(&text) {
        if env.bundle.bundle_identifier() == "pockettrend.hunterisland"
            && env.bundle.bundle_version() == "2.02"
        {
            return point;
        }
    }
    text.parse().unwrap_or_default()
}

// TownCommonNode passes these two malformed retail warp positions to UIKit.
fn hunter_warp_position(text: &str) -> Option<CGPoint> {
    let y = match text {
        "[12.5,6}" => 6.0,
        "[12.5,10}" => 10.0,
        _ => return None,
    };
    Some(CGPoint { x: 12.5, y })
}

#[cfg(test)]
#[test]
fn recognizes_only_the_two_malformed_hunter_warps() {
    assert_eq!(
        hunter_warp_position("[12.5,6}"),
        Some(CGPoint { x: 12.5, y: 6.0 })
    );
    assert_eq!(
        hunter_warp_position("[12.5,10}"),
        Some(CGPoint { x: 12.5, y: 10.0 })
    );
    assert_eq!(hunter_warp_position("[3,7}"), None);
    assert_eq!(hunter_warp_position("{12.5,6}"), None);
}

pub fn CGSizeFromString(env: &mut Environment, string: id) -> CGSize {
    if string == nil {
        return CGSize::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}
pub fn CGRectFromString(env: &mut Environment, string: id) -> CGRect {
    if string == nil {
        return CGRect::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}

pub fn NSStringFromCGPoint(env: &mut Environment, point: CGPoint) -> id {
    let s = ns_string::from_rust_string(env, point.to_string());
    autorelease(env, s)
}
pub fn NSStringFromCGSize(env: &mut Environment, size: CGSize) -> id {
    let s = ns_string::from_rust_string(env, size.to_string());
    autorelease(env, s)
}
pub fn NSStringFromCGRect(env: &mut Environment, rect: CGRect) -> id {
    let s = ns_string::from_rust_string(env, rect.to_string());
    autorelease(env, s)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGPointFromString(_)),
    export_c_func!(CGSizeFromString(_)),
    export_c_func!(CGRectFromString(_)),
    export_c_func!(NSStringFromCGPoint(_)),
    export_c_func!(NSStringFromCGSize(_)),
    export_c_func!(NSStringFromCGRect(_)),
];
