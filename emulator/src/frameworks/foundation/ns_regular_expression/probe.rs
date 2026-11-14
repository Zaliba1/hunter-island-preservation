/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Own Unicode, capture and ownership fixtures; no game code or resources.
use super::*;
fn main(env: &mut Environment) {
    enumeration_probe(env);
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let pattern = ns_string::from_rust_string(env, r#"(?<=T@\")([^\"]+)(?=\")"#.into());
    let regex: id = msg_class![env; NSRegularExpression alloc];
    let error = env.mem.alloc(4).cast::<id>();
    env.mem.write(error, nil);
    let regex: id = msg![env; regex initWithPattern:pattern options:0u32 error:error];
    release(env, pattern);
    assert!(regex != nil && env.mem.read(error) == nil);
    let groups: u32 = msg![env; regex numberOfCaptureGroups];
    assert_eq!(groups, 1);
    let text = ns_string::from_rust_string(env, "😀 T@\"Hero\",N".into());
    let length: u32 = msg![env; text length];
    let matched: id =
        msg![env; regex firstMatchInString:text options:0u32 range:(range((0,length)))];
    assert!(matched != nil);
    retain(env, matched);
    let found: NSRange = msg![env; matched rangeAtIndex:1u32];
    assert_eq!((found.location, found.length), (6, 4));
    let retained: id = msg![env; matched regularExpression];
    assert_eq!(retained, regex);
    let no_match: NSRange =
        msg![env; regex rangeOfFirstMatchInString:text options:0u32 range:(range((0,2)))];
    assert_eq!((no_match.location, no_match.length), NOT_FOUND);
    release(env, regex);
    let kept_pattern: id = msg![env; retained pattern];
    assert!(ns_string::to_rust_string(env, kept_pattern).contains("T@"));
    echo!("REGEX_UTF16_LOOKAROUND_CAPTURE_AND_OWNERSHIP_PASS");

    let p = ns_string::from_rust_string(env, "(a)(b)?".into());
    let regex: id = msg_class![env; NSRegularExpression regularExpressionWithPattern:p options:0u32 error:error];
    release(env, p);
    let input = ns_string::mutable_from_rust_string(env, "a ab".into());
    let template = ns_string::from_rust_string(env, r"$2:$1:\$".into());
    let count: u32 = msg![env; regex replaceMatchesInString:input options:0u32 range:(range((0,4))) withTemplate:template];
    assert_eq!(count, 2);
    assert_eq!(ns_string::to_rust_string(env, input), ":a:$ b:a:$");
    let bad = ns_string::from_rust_string(env, "(".into());
    let failed: id = msg_class![env; NSRegularExpression regularExpressionWithPattern:bad options:0u32 error:error];
    assert_eq!(failed, nil);
    let error_value = env.mem.read(error);
    assert!(error_value != nil);
    let code: i32 = msg![env; error_value code];
    assert_eq!(code, 2048);
    for object in [bad, input, template, text] {
        release(env, object);
    }
    release(env, pool);
    // The checking result, and consequently its expression and pattern, survive
    // draining the pool and releasing the expression's original ownership.
    let retained: id = msg![env; matched regularExpression];
    let count: u32 = msg![env; retained numberOfCaptureGroups];
    assert_eq!(count, 1);
    release(env, matched);
    env.mem.free(error.cast());
    echo!("REGEX_REPLACEMENT_AND_ERROR_PASS");
    echo!("REGEX_FRAMEWORK_PROBE_PASS");
}
fn enumeration_callback(
    env: &mut Environment,
    block: MutVoidPtr,
    matched: id,
    flags: u32,
    stop: MutPtr<u8>,
) {
    assert!(!stop.is_null());
    let state: MutPtr<u32> = env.mem.read((block.cast::<u32>() + 5).cast());
    if matched != nil {
        assert_eq!(flags, 0);
        let matches: id = env.mem.read(state.cast());
        () = msg![env; matches addObject:matched];
        let count = env.mem.read(state + 1) + 1;
        env.mem.write(state + 1, count);
        if count == env.mem.read(state + 4) {
            env.mem.write(stop, 1);
        }
    } else if flags & 1 != 0 {
        assert_eq!(flags, 1);
        let count = env.mem.read(state + 2);
        env.mem.write(state + 2, count + 1);
        if env.mem.read(state + 5) != 0 {
            env.mem.write(stop, 1);
        }
    } else {
        assert_eq!(flags, 2);
        let count = env.mem.read(state + 3);
        env.mem.write(state + 3, count + 1);
    }
}
fn enumeration_probe(env: &mut Environment) {
    use crate::dyld::export_c_func;
    let (name, callback) = export_c_func!(enumeration_callback(_, _, _, _));
    let invoke = env.dyld.create_guest_function(&mut env.mem, name, callback);
    for (pattern, text, options, stop_after, stop_progress, expected, completed) in [
        (
            "(a)(b)?",
            "😀 a ab",
            0u32,
            0u32,
            false,
            vec![(3, 1), (5, 2)],
            0,
        ),
        ("(a)(b)?", "😀 a ab", 2, 1, false, vec![(3, 1)], 0),
        ("a", "a a", 3, 0, false, vec![(0, 1), (2, 1)], 1),
        ("a", "a a", 3, 0, true, vec![], 0),
        ("z", "a a", 2, 0, false, vec![], 1),
        ("a", "aa a", 4, 0, false, vec![(0, 1), (1, 1)], 0),
        ("", "😀", 0, 0, false, vec![(0, 0), (2, 0)], 0),
    ] {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let p = ns_string::from_rust_string(env, pattern.into());
        let regex: id = msg_class![env; NSRegularExpression regularExpressionWithPattern:p options:0u32 error:(MutPtr::<id>::null())];
        release(env, p);
        let input = ns_string::from_rust_string(env, text.into());
        let length: u32 = msg![env; input length];
        let matches: id = msg_class![env; NSMutableArray new];
        let state = env.mem.alloc(24).cast::<u32>();
        env.mem.write(state.cast(), matches);
        for i in 1..6 {
            env.mem.write(state + i, 0);
        }
        env.mem.write(state + 4, stop_after);
        env.mem.write(state + 5, u32::from(stop_progress));
        let block = env.mem.alloc(24);
        env.mem.write(
            block.cast(),
            crate::libc::blocks::Header {
                isa: nil,
                flags: crate::libc::blocks::IS_GLOBAL,
                reserved: 0,
                invoke,
                descriptor: MutPtr::null(),
            },
        );
        env.mem.write((block.cast::<u32>() + 5).cast(), state);
        () = msg![env; regex enumerateMatchesInString:input options:options range:(range((0,length))) usingBlock:(block.cast_void())];
        assert_eq!(env.mem.read(state + 1), expected.len() as u32);
        assert_eq!(env.mem.read(state + 3), completed);
        if options & 1 != 0 {
            assert!(env.mem.read(state + 2) > 0);
        }
        env.mem.free(block);
        env.mem.free(state.cast());
        release(env, input);
        // Array-retained results must own their expression after its pool dies.
        release(env, pool);
        for (i, expected) in expected.into_iter().enumerate() {
            let matched: id = msg![env; matches objectAtIndex:(i as u32)];
            let actual: NSRange = msg![env; matched range];
            assert_eq!((actual.location, actual.length), expected);
            let expression: id = msg![env; matched regularExpression];
            let p: id = msg![env; expression pattern];
            assert_eq!(ns_string::to_rust_string(env, p), pattern);
            if pattern == "(a)(b)?" && i == 0 {
                let optional: NSRange = msg![env; matched rangeAtIndex:2u32];
                assert_eq!((optional.location, optional.length), NOT_FOUND);
            }
        }
        release(env, matches);
    }
    echo!("REGEX_GUEST_BLOCK_UTF16_STOP_COMPLETION_AND_LIFETIME_PASS");
}
pub fn framework_probe() -> Result<(), String> {
    let env = Environment::new_without_app(
        crate::options::Options {
            popup_errors: false,
            ..Default::default()
        },
        crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
    )?;
    env.run_framework_probe(main);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guest_block_enumeration_utf16_early_stop_completion_and_lifetime() {
        let env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.run_framework_probe(enumeration_probe);
    }
    #[test]
    fn unicode_regions_empty_matches_optional_captures_and_errors() {
        let re = compile(r"a(\d+)(b)?", 0).unwrap();
        let found = search(&re, "😀 a12!", 0, (0, 7), false).unwrap();
        assert_eq!(found, vec![vec![(3, 3), (4, 2), NOT_FOUND]]);
        assert!(search(&re, "😀 a12!", 4, (0, 7), false).unwrap().is_empty());
        assert!(search(&re, "😀 a12!", 0, (1, 2), false).is_err());
        assert!(search(&re, "😀 a12!", 0, (0, u32::MAX), false).is_err());
        assert!(search(&re, "😀 a12!", 8, (3, 3), false).is_err());
        let empty = compile("", 0).unwrap();
        assert_eq!(
            search(&empty, "😀", 0, (0, 2), false).unwrap(),
            vec![vec![(0, 0)], vec![(2, 0)]]
        );
        let anchor = compile("^a", 0).unwrap();
        assert_eq!(
            search(&anchor, "za", 0, (1, 1), false).unwrap(),
            vec![vec![(1, 1)]]
        );
        assert!(compile("(", 0).is_err());
        assert!(compile("a", 64).is_err());
        let literal = compile("C++", 5).unwrap();
        assert_eq!(
            search(&literal, "c++", 0, (0, 3), true).unwrap(),
            vec![vec![(0, 3)]]
        );
    }
    #[test]
    fn framework_utf16_ranges_replacements_and_lifetimes() {
        let env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.run_framework_probe(main);
    }
}
