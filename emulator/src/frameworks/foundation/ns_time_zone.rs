/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSTimeZone`.

use crate::frameworks::foundation::{ns_string, NSInteger};
use crate::objc::{autorelease, id, nil, release, retain, ClassExports, HostObject, NSZonePtr};
use crate::{msg, objc_classes};
use chrono::Offset;

#[derive(Default)]
pub struct State {
    system_time_zone: Option<id>,
}

struct NSTimeZoneHostObject {
    // NSString*
    time_zone: id,
}
impl HostObject for NSTimeZoneHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSTimeZone: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSTimeZoneHostObject {
        time_zone: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)timeZoneWithName:(id)tz_name {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithName:tz_name];
    autorelease(env, new)
}

+ (id)timeZoneForSecondsFromGMT:(NSInteger)seconds {
    assert!(seconds.unsigned_abs() <= 18 * 3600 && seconds % 60 == 0, "Only minute-aligned fixed zones within 18 hours are supported");
    let sign = if seconds < 0 { '-' } else { '+' };
    let magnitude = seconds.unsigned_abs();
    let name = ns_string::from_rust_string(env, format!("GMT{sign}{:02}{:02}", magnitude / 3600, (magnitude / 60) % 60));
    let zone: id = msg![env; this timeZoneWithName:name];
    release(env, name);
    zone
}

+ (id)localTimeZone {
    // According to docs, `localTimeZone` is not cached in contrast to
    // `systemTimeZone`
    let gmt_tz_name: id = ns_string::get_static_str(env, "GMT");
    msg![env; this timeZoneWithName:gmt_tz_name]
}

+ (id)systemTimeZone {
    if let Some(system_time_zone) = env.framework_state.foundation.ns_time_zone.system_time_zone {
        system_time_zone
    } else {
        let new: id = msg![env; this alloc];
        let gmt_tz_name: id = ns_string::get_static_str(env, "GMT");
        let new: id = msg![env; new initWithName:gmt_tz_name];
        env.framework_state.foundation.ns_time_zone.system_time_zone = Some(new);
        new
    }
}

+ (id)defaultTimeZone {
    // TODO: implement setting a default time zone
    msg![env; this systemTimeZone]
}

- (())dealloc {
    let tz_name = env.objc.borrow_mut::<NSTimeZoneHostObject>(this).time_zone;
    release(env, tz_name);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)initWithName:(id)tz_name { // NSString *
    assert_ne!(tz_name, nil);
    let name = ns_string::to_rust_string(env, tz_name);
    if parse_fixed_offset(&name).is_none() && named_zone(&name).is_none() {
        release(env, this);
        return nil;
    }
    let copied: id = msg![env; tz_name copy];
    let old = std::mem::replace(&mut env.objc.borrow_mut::<NSTimeZoneHostObject>(this).time_zone, copied);
    release(env, old);
    this
}

- (id)name {
    env.objc.borrow_mut::<NSTimeZoneHostObject>(this).time_zone
}

- (id)abbreviation {
    let at = current_reference_time();
    abbreviation(env, this, at)
}
- (id)abbreviationForDate:(id)date {
    let at: f64 = msg![env; date timeIntervalSinceReferenceDate];
    abbreviation(env, this, at)
}
- (NSInteger)secondsFromGMT {
    offset_seconds(env, this, current_reference_time())
}
- (NSInteger)secondsFromGMTForDate:(id)date {
    let at: f64 = msg![env; date timeIntervalSinceReferenceDate];
    offset_seconds(env, this, at)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

@end

};

fn parse_fixed_offset(name: &str) -> Option<i32> {
    if ["GMT", "UTC", "Etc/GMT", "Etc/UTC", "Zulu"].contains(&name) {
        return Some(0);
    }
    let suffix = name
        .strip_prefix("GMT")
        .or_else(|| name.strip_prefix("UTC"))?;
    let sign = match suffix.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let raw = &suffix[1..];
    if raw.len() != 4 && !(raw.len() == 5 && raw.as_bytes()[2] == b':') {
        return None;
    }
    let digits = raw.replace(':', "");
    if digits.len() != 4 || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hour: i32 = digits[..2].parse().ok()?;
    let minute: i32 = digits[2..].parse().ok()?;
    if minute >= 60 || hour > 18 || (hour == 18 && minute != 0) {
        return None;
    }
    Some(sign * (hour * 3600 + minute * 60))
}
fn named_zone(name: &str) -> Option<chrono_tz::Tz> {
    // ICU's legacy PST identifier denotes the Los Angeles rule set, including
    // PDT in summer, rather than an invented permanent UTC-8 offset.
    let name = if name == "PST" {
        "America/Los_Angeles"
    } else {
        name
    };
    name.parse().ok()
}
fn unix_timestamp(at: f64) -> f64 {
    at + crate::frameworks::core_foundation::time::SECS_FROM_UNIX_TO_APPLE_EPOCHS as f64
}
fn named_date(name: &str, unix: f64) -> Option<chrono::DateTime<chrono_tz::Tz>> {
    if !unix.is_finite() {
        return None;
    }
    let utc = chrono::DateTime::from_timestamp(unix.floor() as i64, 0)?;
    Some(utc.with_timezone(&named_zone(name)?))
}
fn offset_at(name: &str, unix: f64) -> Option<i32> {
    parse_fixed_offset(name)
        .or_else(|| Some(named_date(name, unix)?.offset().fix().local_minus_utc()))
}
fn current_reference_time() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        - crate::frameworks::core_foundation::time::SECS_FROM_UNIX_TO_APPLE_EPOCHS as f64
}
pub(crate) fn offset_seconds(env: &mut crate::Environment, zone: id, at: f64) -> i32 {
    if zone == nil {
        return 0;
    }
    let name: id = msg![env; zone name];
    let name = ns_string::to_rust_string(env, name);
    offset_at(&name, unix_timestamp(at))
        .unwrap_or_else(|| panic!("Unsupported or out-of-range time zone {name}"))
}
fn abbreviation(env: &mut crate::Environment, zone: id, at: f64) -> id {
    let name: id = msg![env; zone name];
    let name = ns_string::to_rust_string(env, name);
    let value = if parse_fixed_offset(&name).is_some() {
        name.into_owned()
    } else {
        named_date(&name, unix_timestamp(at))
            .expect("Unsupported time zone/date")
            .format("%Z")
            .to_string()
    };
    let result = ns_string::from_rust_string(env, value);
    autorelease(env, result)
}
#[cfg(test)]
mod tests {
    #[test]
    fn fixed_offset_names_validate_signed_minutes_without_iana_guessing() {
        for (name, offset) in [
            ("UTC", 0),
            ("GMT+0530", 19800),
            ("UTC-03:30", -12600),
            ("GMT+1800", 64800),
        ] {
            assert_eq!(super::parse_fixed_offset(name), Some(offset));
        }
        for name in [
            "Europe/Berlin",
            "GMT+1860",
            "GMT+1900",
            "UTC+18:01",
            "GMT+01x0",
        ] {
            assert_eq!(super::parse_fixed_offset(name), None);
        }
    }
}

#[cfg(test)]
mod named_tests {
    #[test]
    fn iana_and_legacy_pst_use_the_requested_dates_dst_transition() {
        for (name, unix, expected) in [
            ("PST", 1704067200f64, -28800),
            ("PST", 1719792000f64, -25200),
            ("America/Los_Angeles", 1710064799f64, -28800),
            ("America/Los_Angeles", 1710064800f64, -25200),
            ("Europe/Berlin", 1704067200f64, 3600),
            ("Europe/Berlin", 1719792000f64, 7200),
            ("Asia/Kathmandu", 1719792000f64, 20700),
        ] {
            assert_eq!(super::offset_at(name, unix), Some(expected));
        }
        assert_eq!(super::offset_at("Invalid/OwnZone", 1704067200f64), None);
    }
}
