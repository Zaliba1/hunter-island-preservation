/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Typed KVC writes: method ABI and ivar widths come from metadata, not values.
mod probe;
use super::{
    id, msg, msg_class, msg_send, nil, release, retain, Class, ClassHostObject, ObjC, IMP, SEL,
};
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string;
use crate::mem::{guest_size_of, MutVoidPtr};
use crate::Environment;
pub use probe::framework_probe;
pub(crate) use probe::CLASSES;
use std::any::TypeId;
use std::collections::HashSet;

fn tokens(signature: &str) -> Vec<String> {
    fn end(bytes: &[u8], mut at: usize) -> usize {
        while at < bytes.len() && b"rnNoORV".contains(&bytes[at]) {
            at += 1;
        }
        let first = *bytes.get(at).expect("Incomplete Objective-C encoding");
        at += 1;
        if first == b'^' {
            return end(bytes, at);
        }
        if first == b'@' && bytes.get(at) == Some(&b'?') {
            return at + 1;
        }
        if first == b'@' && bytes.get(at) == Some(&b'"') {
            at += 1;
            while bytes.get(at) != Some(&b'"') {
                assert!(at < bytes.len());
                at += 1;
            }
            return at + 1;
        }
        if let Some(index) = b"{[(".iter().position(|&c| c == first) {
            let close = b"}])"[index];
            while bytes.get(at) != Some(&close) {
                assert!(
                    at < bytes.len(),
                    "Unterminated Objective-C aggregate encoding"
                );
                if bytes[at] == b'"' {
                    at += 1;
                    while bytes.get(at) != Some(&b'"') {
                        assert!(at < bytes.len());
                        at += 1;
                    }
                    at += 1;
                } else if b"{[(^".contains(&bytes[at]) {
                    at = end(bytes, at);
                } else {
                    at += 1;
                }
            }
            return at + 1;
        }
        at
    }
    let mut result = Vec::new();
    let mut at = 0;
    while at < signature.len() {
        let stop = end(signature.as_bytes(), at);
        result.push(
            signature[at..stop]
                .trim_start_matches(|c| "rnNoORV".contains(c))
                .to_owned(),
        );
        at = stop;
        while at < signature.len()
            && (signature.as_bytes()[at].is_ascii_digit()
                || b"+-".contains(&signature.as_bytes()[at]))
        {
            at += 1;
        }
    }
    result
}
fn setter_type(env: &Environment, mut class: Class, selector: SEL) -> String {
    let mut seen = HashSet::new();
    while class != nil && seen.insert(class) {
        let host = env.objc.borrow::<ClassHostObject>(class);
        if let Some(method) = host.methods.get(&selector) {
            match method {
                IMP::Guest(_) => {
                    let signature = *host
                        .guest_method_signatures
                        .get(&selector)
                        .expect("KVC setter has no type encoding");
                    let types = tokens(env.mem.cstr_at_utf8(signature).unwrap());
                    assert!(
                        types.len() == 4 && types[0] == "v" && types[1] == "@" && types[2] == ":",
                        "Invalid KVC setter signature"
                    );
                    return types[3].clone();
                }
                IMP::Host(method) => {
                    let actual = method.type_info().0;
                    macro_rules! check { ($($typ:ty => $encoding:expr),* $(,)?) => { $(if actual==TypeId::of::<((),(id,SEL,$typ))>() { return $encoding.into(); })* }; }
                    check!(id=>"@",bool=>"B",i8=>"c",u8=>"C",i16=>"s",u16=>"S",i32=>"i",u32=>"I",i64=>"q",u64=>"Q",f32=>"f",f64=>"d",CGPoint=>"{CGPoint=ff}",CGSize=>"{CGSize=ff}",CGRect=>"{CGRect={CGPoint=ff}{CGSize=ff}}");
                    panic!("Unsupported host KVC setter signature");
                }
            }
        }
        class = host.superclass;
    }
    panic!("Missing KVC setter");
}
enum Target {
    Setter(SEL),
    Ivar(MutVoidPtr, u32),
}
fn assign(env: &mut Environment, object: id, value: id, key: id, encoding: &str, target: Target) {
    if value == nil && !encoding.starts_with(['@', '#']) {
        () = msg![env; object setNilValueForKey:key];
        return;
    }
    let string_integer = if "cCsSiIlLqQ".contains(encoding.as_bytes()[0] as char) {
        let string_class = env.objc.get_known_class("NSString", &mut env.mem);
        if env
            .objc
            .class_is_subclass_of(ObjC::read_isa(value, &env.mem), string_class)
        {
            let number: i64 = msg![env; value longLongValue];
            Some(number)
        } else {
            None
        }
    } else {
        None
    };
    macro_rules! integer { ($typ:ty,$selector:ident) => {{ if let Some(number)=string_integer { number as $typ } else { let number:$typ=msg![env; value $selector]; number } }}; }
    macro_rules! store {
        ($typ:ty,$expression:expr) => {{
            let unboxed: $typ = $expression;
            match target {
                Target::Setter(selector) => {
                    () = msg_send(env, (object, selector, unboxed));
                }
                Target::Ivar(pointer, size) => {
                    assert!(
                        size >= guest_size_of::<$typ>(),
                        "KVC ivar is smaller than its type"
                    );
                    env.mem.write(pointer.cast::<$typ>(), unboxed);
                }
            }
        }};
    }
    match encoding.as_bytes()[0] {
        b'@' | b'#' => match target {
            Target::Setter(selector) => {
                () = msg_send(env, (object, selector, value));
            }
            Target::Ivar(pointer, size) => {
                assert!(size >= 4);
                let old: id = env.mem.read(pointer.cast());
                retain(env, value);
                env.mem.write(pointer.cast(), value);
                release(env, old);
            }
        },
        b'c' => store!(i8, integer!(i8, charValue)),
        b'C' => store!(u8, {
            let v: u32 = integer!(u32, unsignedIntValue);
            v as u8
        }),
        b's' => store!(i16, integer!(i16, shortValue)),
        b'S' => store!(u16, integer!(u16, unsignedShortValue)),
        b'i' | b'l' => store!(i32, integer!(i32, intValue)),
        b'I' | b'L' => store!(u32, integer!(u32, unsignedIntValue)),
        b'q' => store!(i64, integer!(i64, longLongValue)),
        b'Q' => store!(u64, integer!(u64, unsignedLongLongValue)),
        b'f' => store!(f32, msg![env; value floatValue]),
        b'd' => store!(f64, msg![env; value doubleValue]),
        b'B' => store!(bool, msg![env; value boolValue]),
        b'{' if encoding.starts_with("{CGPoint=") => store!(CGPoint, msg![env; value CGPointValue]),
        b'{' if encoding.starts_with("{CGSize=") => store!(CGSize, msg![env; value CGSizeValue]),
        b'{' if encoding.starts_with("{CGRect=") => store!(CGRect, msg![env; value CGRectValue]),
        _ => panic!("Unsupported KVC value encoding {encoding}"),
    }
}
pub(crate) fn set_value(env: &mut Environment, object: id, value: id, key: id) {
    assert!(key != nil, "KVC requires a key");
    let name = ns_string::to_rust_string(env, key);
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        () = msg![env; object setValue:value forUndefinedKey:key];
        return;
    };
    let camel = first.to_uppercase().collect::<String>() + chars.as_str();
    let class = ObjC::read_isa(object, &env.mem);
    for candidate in [format!("set{camel}:"), format!("_set{camel}:")] {
        if let Some(selector) = env.objc.lookup_selector(&candidate) {
            if env.objc.class_has_method(class, selector) {
                let encoding = setter_type(env, class, selector);
                assign(env, object, value, key, &encoding, Target::Setter(selector));
                return;
            }
        }
    }
    let direct: bool = msg![env; class accessInstanceVariablesDirectly];
    if direct {
        for candidate in [
            format!("_{name}"),
            format!("_is{camel}"),
            name.to_string(),
            format!("is{camel}"),
        ] {
            let mut owner = class;
            while owner != nil {
                let host = env.objc.borrow::<ClassHostObject>(owner);
                if let Some(&(offset, _)) = host.ivars.get(&candidate) {
                    let &(encoding, size) = host
                        .ivar_types
                        .get(&candidate)
                        .expect("KVC ivar has no type encoding");
                    let offset = env.mem.read(offset);
                    let allocation = env.objc.borrow::<ClassHostObject>(class).instance_size;
                    assert!(
                        offset >= 4
                            && offset
                                .checked_add(size)
                                .is_some_and(|end| end <= allocation),
                        "KVC ivar lies outside object allocation"
                    );
                    let encoding = env
                        .mem
                        .cstr_at_utf8(encoding)
                        .unwrap()
                        .trim_start_matches(|c| "rnNoORV".contains(c))
                        .to_owned();
                    let pointer = env
                        .objc
                        .object_lookup_ivar(&env.mem, object, &candidate)
                        .unwrap()
                        .cast();
                    assign(
                        env,
                        object,
                        value,
                        key,
                        &encoding,
                        Target::Ivar(pointer, size),
                    );
                    return;
                }
                owner = host.superclass;
            }
        }
    }
    () = msg![env; object setValue:value forUndefinedKey:key];
}

#[cfg(test)]
mod tests {
    #[test]
    fn setter_type_tokens_include_qualifiers_objects_64bit_and_nested_structs() {
        assert_eq!(super::tokens("v16@0:4rd8"), ["v", "@", ":", "d"]);
        assert_eq!(
            super::tokens("v12@0:4@\"NSNumber\"8"),
            ["v", "@", ":", "@\"NSNumber\""]
        );
        assert_eq!(
            super::tokens("v24@0:4{CGRect={CGPoint=ff}{CGSize=ff}}8"),
            ["v", "@", ":", "{CGRect={CGPoint=ff}{CGSize=ff}}"]
        );
    }
}
