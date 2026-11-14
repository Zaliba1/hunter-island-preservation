/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSDateFormatter`.
//!
//! Resources:
//! - Apple's [Introduction to Data Formatting Programming Guide For Cocoa](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/DataFormatting/DataFormatting.html)
//! - [Unicode Technical Standard #35](https://unicode.org/reports/tr35/tr35-10.html#Date_Format_Patterns)

use crate::frameworks::core_foundation::time::CFAbsoluteTimeGetGregorianDate;
use crate::frameworks::foundation::{ns_string, NSTimeInterval, NSUInteger};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, ClassExports, HostObject,
    NSZonePtr,
};

struct NSDateFormatterHostObject {
    date_format: Option<id>,
    time_zone: id,
    date_style: NSUInteger,
    time_style: NSUInteger,
    locale: id,
}
impl HostObject for NSDateFormatterHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSDateFormatter: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSDateFormatterHostObject {
        date_format: None,
        time_zone: nil,
        date_style: 0,
        time_style: 0,
        locale: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())setDateFormat:(id)format { // NSString *
    let date_format: id = msg![env; format copy];
    let old = env.objc.borrow_mut::<NSDateFormatterHostObject>(this).date_format.replace(date_format);
    if let Some(old) = old { release(env, old); }
    log!("NSDateFormatter format: {:?}", ns_string::to_rust_string(env, date_format));
}

- (id)dateFormat {
    let host = env.objc.borrow::<NSDateFormatterHostObject>(this);
    if let Some(format) = host.date_format { return format; }
    let (date,time)=(host.date_style,host.time_style);
    let german=formatter_is_german(env,this);
    let pattern = localized_style_pattern(date,time,german);
    let pattern = ns_string::from_rust_string(env,pattern);
    autorelease(env,pattern)
}
- (())setDateStyle:(NSUInteger)style {
    assert!(style<=4,"Unsupported NSDateFormatter date style");
    let host=env.objc.borrow_mut::<NSDateFormatterHostObject>(this);
    host.date_style=style;
    let old=host.date_format.take();
    if let Some(old)=old { release(env,old); }
}
- (())setTimeStyle:(NSUInteger)style {
    assert!(style<=4,"Unsupported NSDateFormatter time style");
    let host=env.objc.borrow_mut::<NSDateFormatterHostObject>(this);
    host.time_style=style;
    let old=host.date_format.take();
    if let Some(old)=old { release(env,old); }
}
- (NSUInteger)dateStyle { env.objc.borrow::<NSDateFormatterHostObject>(this).date_style }
- (NSUInteger)timeStyle { env.objc.borrow::<NSDateFormatterHostObject>(this).time_style }
- (())setLocale:(id)locale {
    let copied: id = msg![env; locale copy];
    let old=std::mem::replace(&mut env.objc.borrow_mut::<NSDateFormatterHostObject>(this).locale,copied);
    release(env,old);
}
- (id)locale {
    let locale=env.objc.borrow::<NSDateFormatterHostObject>(this).locale;
    if locale==nil { msg_class![env; NSLocale currentLocale] } else { locale }
}
- (())setTimeZone:(id)zone {
    let copied: id = msg![env; zone copy];
    let old = std::mem::replace(&mut env.objc.borrow_mut::<NSDateFormatterHostObject>(this).time_zone, copied);
    release(env, old);
}
- (id)timeZone {
    let zone = env.objc.borrow::<NSDateFormatterHostObject>(this).time_zone;
    if zone == nil { msg_class![env; NSTimeZone defaultTimeZone] } else { zone }
}
- (())dealloc {
    let host = env.objc.borrow::<NSDateFormatterHostObject>(this);
    let (format, zone,locale) = (host.date_format, host.time_zone,host.locale);
    if let Some(format) = format { release(env, format); }
    release(env, zone);
    release(env,locale);
    env.objc.dealloc_object(this, &mut env.mem);
}
- (id)stringFromDate:(id)date {
    if date == nil { return nil; }
    let format: id = msg![env; this dateFormat];
    let format = ns_string::to_rust_string(env, format).into_owned();
    let zone: id = msg![env; this timeZone];
    let ti: NSTimeInterval = msg![env; date timeIntervalSinceReferenceDate];
    let offset = super::ns_time_zone::offset_seconds(env, zone, ti);
    let greg_date = CFAbsoluteTimeGetGregorianDate(env, ti, zone);
    let german=formatter_is_german(env,this);
    let result = format_date_localized(&format, greg_date, offset,german).unwrap_or_else(|error| panic!("Unsupported NSDateFormatter format: {error}"));
    let result = ns_string::from_rust_string(env, result);
    autorelease(env, result)
}

@end

};

fn formatter_is_german(env: &mut crate::Environment, formatter: id) -> bool {
    let locale: id = msg![env; formatter locale];
    let key = ns_string::get_static_str(env, "NSLocaleLanguageCode");
    let language: id = msg![env; locale objectForKey:key];
    language != nil && ns_string::to_rust_string(env, language).starts_with("de")
}

// English/German style templates with 24-hour time. Other locale templates
// and user 12/24-hour preferences are not implemented yet.
#[cfg(test)]
fn style_pattern(date: u32, time: u32) -> String {
    localized_style_pattern(date, time, false)
}
fn localized_style_pattern(date: u32, time: u32, german: bool) -> String {
    let date = if german {
        match date {
            0 => "",
            1 => "dd.MM.yy",
            2 => "dd.MM.yyyy",
            3 => "d. MMMM yyyy",
            4 => "EEEE, d. MMMM yyyy",
            _ => unreachable!(),
        }
    } else {
        match date {
            0 => "",
            1 => "M/d/yy",
            2 => "MMM d, yyyy",
            3 => "MMMM d, yyyy",
            4 => "EEEE, MMMM d, yyyy",
            _ => unreachable!(),
        }
    };
    let time = match time {
        0 => "",
        1 => "HH:mm",
        2 => "HH:mm:ss",
        3 | 4 => "HH:mm:ss Z",
        _ => unreachable!(),
    };
    [date, time]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
fn format_date(
    pattern: &str,
    date: crate::frameworks::core_foundation::time::CFGregorianDate,
    offset: i32,
) -> Result<String, String> {
    format_date_localized(pattern, date, offset, false)
}
fn format_date_localized(
    pattern: &str,
    date: crate::frameworks::core_foundation::time::CFGregorianDate,
    offset: i32,
    german: bool,
) -> Result<String, String> {
    let (year, month, day, hour, minute, seconds) = (
        date.year,
        date.month,
        date.day,
        date.hours,
        date.minutes,
        date.seconds,
    );
    let mut chars = pattern.chars().peekable();
    let mut quoted = false;
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                out.push('\'');
            } else {
                quoted = !quoted;
            }
        } else if quoted || !c.is_ascii_alphabetic() {
            out.push(c);
        } else {
            let mut count = 1;
            while chars.peek() == Some(&c) {
                chars.next();
                count += 1;
            }
            let part = match (c, count) {
                ('y', 2) => format!("{:02}", year.rem_euclid(100)),
                ('y', 1..=4) => format!("{year:0count$}"),
                ('M', 1..=2) => format!("{month:0count$}"),
                ('M', 3) if german => [
                    "Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.",
                    "Nov.", "Dez.",
                ][(month - 1) as usize]
                    .into(),
                ('M', 4) if german => [
                    "Januar",
                    "Februar",
                    "März",
                    "April",
                    "Mai",
                    "Juni",
                    "Juli",
                    "August",
                    "September",
                    "Oktober",
                    "November",
                    "Dezember",
                ][(month - 1) as usize]
                    .into(),
                ('M', 3) => [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ][(month - 1) as usize]
                    .into(),
                ('M', 4) => [
                    "January",
                    "February",
                    "March",
                    "April",
                    "May",
                    "June",
                    "July",
                    "August",
                    "September",
                    "October",
                    "November",
                    "December",
                ][(month - 1) as usize]
                    .into(),
                ('E', 1..=4) => {
                    let y = year - i32::from(month < 3);
                    let shifts = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
                    let index = (y + y.div_euclid(4) - y.div_euclid(100)
                        + y.div_euclid(400)
                        + shifts[(month - 1) as usize]
                        + day as i32)
                        .rem_euclid(7) as usize;
                    let days = if german && count == 4 {
                        [
                            "Sonntag",
                            "Montag",
                            "Dienstag",
                            "Mittwoch",
                            "Donnerstag",
                            "Freitag",
                            "Samstag",
                        ]
                    } else if german {
                        ["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."]
                    } else if count == 4 {
                        [
                            "Sunday",
                            "Monday",
                            "Tuesday",
                            "Wednesday",
                            "Thursday",
                            "Friday",
                            "Saturday",
                        ]
                    } else {
                        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                    };
                    days[index].into()
                }
                ('d', 1..=2) => format!("{day:0count$}"),
                ('H', 1..=2) => format!("{hour:0count$}"),
                ('m', 1..=2) => format!("{minute:0count$}"),
                ('s', 1..=2) => format!("{:0count$}", seconds.floor() as u32),
                ('S', 1..=9) => format!(
                    "{:0count$}",
                    (seconds.fract() * 10f64.powi(count as i32)).floor() as u32
                ),
                ('Z', 1..=3) | ('Z', 5) => {
                    if count == 5 && offset == 0 {
                        "Z".into()
                    } else {
                        let sign = if offset < 0 { '-' } else { '+' };
                        let magnitude = offset.unsigned_abs();
                        let separator = if count == 5 { ":" } else { "" };
                        format!(
                            "{sign}{:02}{separator}{:02}",
                            magnitude / 3600,
                            (magnitude / 60) % 60
                        )
                    }
                }
                _ => return Err(format!("field {}", c.to_string().repeat(count))),
            };
            out.push_str(&part);
        }
    }
    if quoted {
        return Err("Unterminated quoted literal".into());
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Environment;
    #[test]
    fn style_fallback_combines_date_time_and_explicit_format_override() {
        let date = crate::frameworks::core_foundation::time::CFGregorianDate {
            year: 2001,
            month: 1,
            day: 1,
            hours: 0,
            minutes: 0,
            seconds: 2.0,
        };
        for (d, t, expected) in [
            (0, 0, ""),
            (1, 1, "1/1/01 00:00"),
            (2, 2, "Jan 1, 2001 00:00:02"),
            (3, 0, "January 1, 2001"),
            (4, 4, "Monday, January 1, 2001 00:00:02 +0000"),
        ] {
            assert_eq!(
                format_date(&style_pattern(d, t), date, 0).unwrap(),
                expected
            );
        }
        assert_eq!(
            format_date_localized(&localized_style_pattern(4, 1, true), date, 0, true).unwrap(),
            "Montag, 1. Januar 2001 00:00"
        );
        let mut env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut env;
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let formatter: id = msg_class![env; NSDateFormatter new];
        let language = ns_string::from_rust_string(env, "de".into());
        let locale: id = msg_class![env; NSLocale alloc];
        let locale: id = msg![env; locale initWithLocaleIdentifier:language];
        () = msg![env; formatter setLocale:locale];
        release(env, locale);
        release(env, language);
        () = msg![env; formatter setDateStyle:4u32];
        () = msg![env; formatter setTimeStyle:1u32];
        let date: id = msg_class![env; NSDate dateWithTimeIntervalSinceReferenceDate:0f64];
        let text: id = msg![env; formatter stringFromDate:date];
        assert_eq!(
            ns_string::to_rust_string(env, text),
            "Montag, 1. Januar 2001 00:00"
        );
        let pattern = ns_string::from_rust_string(env, "yyyy".into());
        () = msg![env; formatter setDateFormat:pattern];
        release(env, pattern);
        let text: id = msg![env; formatter stringFromDate:date];
        assert_eq!(ns_string::to_rust_string(env, text), "2001");
        () = msg![env; formatter setDateStyle:0u32];
        () = msg![env; formatter setTimeStyle:0u32];
        let text: id = msg![env; formatter stringFromDate:date];
        assert_eq!(ns_string::to_rust_string(env, text), "");
        release(env, formatter);
        release(env, pool);
        assert!(env.objc.get_host_object(locale).is_none());
    }
    #[test]
    fn formatter_fixed_zones_quotes_fraction_and_reference_epoch_ownership() {
        let mut env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut env;
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let formatter: id = msg_class![env; NSDateFormatter new];
        let pattern =
            ns_string::mutable_from_rust_string(env, "yyyy-MM-dd'T'HH:mm:ss.SSS Z".into());
        () = msg![env; formatter setDateFormat:pattern];
        let changed = ns_string::from_rust_string(env, "invalid".into());
        () = msg![env; pattern setString:changed];
        release(env, pattern);
        release(env, changed);
        let date: id = msg_class![env; NSDate dateWithTimeIntervalSinceReferenceDate:0.25f64];
        for (offset, expected) in [
            (19800i32, "2001-01-01T05:30:00.250 +0530"),
            (-12600, "2000-12-31T20:30:00.250 -0330"),
        ] {
            let zone: id = msg_class![env; NSTimeZone timeZoneForSecondsFromGMT:offset];
            () = msg![env; formatter setTimeZone:zone];
            let formatted: id = msg![env; formatter stringFromDate:date];
            assert_eq!(ns_string::to_rust_string(env, formatted), expected);
        }
        () = msg![env; formatter setTimeZone:nil];
        let text: id = msg![env; formatter stringFromDate:date];
        assert_eq!(
            ns_string::to_rust_string(env, text),
            "2001-01-01T00:00:00.250 +0000"
        );
        let old: id = msg_class![env; NSDate dateWithTimeIntervalSinceReferenceDate:(-31622400f64)];
        let text: id = msg![env; formatter stringFromDate:old];
        assert!(ns_string::to_rust_string(env, text).starts_with("2000-01-01"));
        let name = ns_string::from_rust_string(env, "PST".into());
        let zone: id = msg_class![env; NSTimeZone timeZoneWithName:name];
        release(env, name);
        assert_ne!(zone, nil);
        () = msg![env; formatter setTimeZone:zone];
        for (unix, expected) in [
            (1704067200f64, "2023-12-31T16:00:00.000 -0800"),
            (1719792000f64, "2024-06-30T17:00:00.000 -0700"),
        ] {
            let date: id = msg_class![env; NSDate dateWithTimeIntervalSince1970:unix];
            let text: id = msg![env; formatter stringFromDate:date];
            assert_eq!(ns_string::to_rust_string(env, text), expected);
        }
        release(env, formatter);
        release(env, pool);
        assert!(env.objc.get_host_object(formatter).is_none());
    }
}
