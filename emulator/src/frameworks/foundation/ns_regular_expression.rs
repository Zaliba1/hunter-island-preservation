/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Portable subset of NSRegularExpression, with checked UTF-16 guest ranges.
mod probe;
use super::{ns_array, ns_string, NSInteger, NSNotFound, NSRange, NSUInteger};
use crate::abi::CallFromHost;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;
use fancy_regex::{Regex, RegexBuilder};
pub use probe::framework_probe;
use std::rc::Rc;
use std::time::{Duration, Instant};
type Range = (u32, u32);
const NOT_FOUND: Range = (NSNotFound as u32, 0);
#[derive(Default)]
struct Expression {
    pattern: id,
    options: u32,
    compiled: Option<Rc<Regex>>,
}
impl HostObject for Expression {}
struct Match {
    expression: id,
    ranges: Vec<Range>,
}
impl HostObject for Match {}
fn range(value: Range) -> NSRange {
    NSRange {
        location: value.0,
        length: value.1,
    }
}
fn compile(pattern: &str, options: u32) -> Result<Regex, String> {
    if options & !31 != 0 {
        return Err(format!(
            "Unsupported NSRegularExpression options {options:#x}"
        ));
    }
    if pattern.len() > 64 * 1024 {
        return Err("Regex pattern exceeds 64 KiB".into());
    }
    let pattern = if options & 4 != 0 {
        escape(pattern)
    } else {
        pattern.to_owned()
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(options & 1 != 0)
        .ignore_whitespace(options & 2 != 0)
        .dot_matches_new_line(options & 8 != 0)
        .multi_line(options & 16 != 0)
        .backtrack_limit(1_000_000)
        .delegate_size_limit(8 * 1024 * 1024)
        .delegate_dfa_size_limit(2 * 1024 * 1024)
        .build()
        .map_err(|err| err.to_string())
}
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if ".+*?()|[]{}^$\\# -\t\r\n".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
fn search(
    regex: &Regex,
    text: &str,
    options: u32,
    requested: Range,
    first: bool,
) -> Result<Vec<Vec<Range>>, String> {
    let mut results = Vec::new();
    search_each(regex, text, options, requested, |ranges| {
        results.push(ranges);
        !first
    })?;
    Ok(results)
}
fn search_each(
    regex: &Regex,
    text: &str,
    options: u32,
    requested: Range,
    mut visit: impl FnMut(Vec<Range>) -> bool,
) -> Result<(), String> {
    if text.len() > 4 * 1024 * 1024 {
        return Err("Regex input exceeds 4 MiB".into());
    }
    if options & !31 != 0 {
        return Err(format!("Unsupported matching options {options:#x}"));
    }
    let mut map = Vec::new();
    let mut units = 0u32;
    for (offset, c) in text.char_indices() {
        map.push((offset, units));
        units += c.len_utf16() as u32;
    }
    map.push((text.len(), units));
    let end = requested
        .0
        .checked_add(requested.1)
        .ok_or("Overflowing NSRange")?;
    let start = map
        .binary_search_by_key(&requested.0, |&(_, u)| u)
        .map_err(|_| "NSRange starts outside text or splits a surrogate pair")?;
    let end = map
        .binary_search_by_key(&end, |&(_, u)| u)
        .map_err(|_| "NSRange ends outside text or splits a surrogate pair")?;
    if options & 24 != 0 && requested != (0, units) {
        return Err("Transparent/nonanchoring bounds on a subrange are not implemented".into());
    }
    let start_byte = map[start].0;
    let selected = &text[start_byte..map[end].0];
    let mut matching_time = Duration::ZERO;
    let mut count = 0;
    let mut total_ranges = 0;
    let mut captures_iter = regex.captures_iter(selected);
    let mut anchor = 0;
    loop {
        let began = Instant::now();
        let captured = captures_iter.next();
        matching_time += began.elapsed();
        let Some(captured) = captured else { break };
        let captures = captured.map_err(|err| err.to_string())?;
        if options & 4 != 0 && captures.get(0).unwrap().start() != anchor {
            break;
        }
        anchor = captures.get(0).unwrap().end();
        total_ranges += captures.len();
        if count >= 100_000 || total_ranges > 1_000_000 || matching_time > Duration::from_secs(1) {
            return Err("Regex result/time budget exceeded".into());
        }
        let mut ranges = Vec::new();
        for index in 0..captures.len() {
            ranges.push(if let Some(found) = captures.get(index) {
                let a = map
                    .binary_search_by_key(&(start_byte + found.start()), |&(b, _)| b)
                    .unwrap();
                let b = map
                    .binary_search_by_key(&(start_byte + found.end()), |&(b, _)| b)
                    .unwrap();
                (map[a].1, map[b].1 - map[a].1)
            } else {
                NOT_FOUND
            });
        }
        count += 1;
        if !visit(ranges) {
            break;
        }
    }
    Ok(())
}
fn enumerate(
    env: &mut Environment,
    this: id,
    text: id,
    options: u32,
    requested: NSRange,
    block: MutVoidPtr,
) {
    assert!(!block.is_null(), "Regex enumeration requires a block");
    assert_ne!(text, nil);
    let regex = env
        .objc
        .borrow::<Expression>(this)
        .compiled
        .clone()
        .expect("Uninitialized regex");
    let text = ns_string::to_rust_string(env, text);
    let invoke = env
        .mem
        .read(block.cast::<crate::libc::blocks::Header>())
        .invoke;
    let stop = env.mem.alloc_and_write(0u8);
    retain(env, this);
    // The existing backend has no ICU hitEnd/requireEnd introspection. Do not
    // invent those flags. Progress is available between matches, not inside a
    // single backtracking operation (which retains its existing work limit).
    let outcome = search_each(
        &regex,
        &text,
        options,
        (requested.location, requested.length),
        |ranges| {
            if options & 1 != 0 {
                let (): () = invoke.call_from_host(env, (block, nil, 1u32, stop));
                if env.mem.read(stop) != 0 {
                    return false;
                }
            }
            let matched = result(env, this, ranges);
            autorelease(env, matched);
            let (): () = invoke.call_from_host(env, (block, matched, 0u32, stop));
            env.mem.read(stop) == 0
        },
    );
    if options & 2 != 0 && env.mem.read(stop) == 0 {
        let flags = 2u32 | if outcome.is_err() { 16 } else { 0 };
        let (): () = invoke.call_from_host(env, (block, nil, flags, stop));
    }
    env.mem.free(stop.cast());
    release(env, this);
    outcome.unwrap_or_else(|err| panic!("NSRegularExpression matching failed: {err}"));
}
fn find(
    env: &mut Environment,
    this: id,
    text: id,
    options: u32,
    requested: NSRange,
    first: bool,
) -> Vec<Vec<Range>> {
    assert!(text != nil, "NSRegularExpression requires a string");
    let regex = env
        .objc
        .borrow::<Expression>(this)
        .compiled
        .clone()
        .expect("Uninitialized NSRegularExpression");
    let text = ns_string::to_rust_string(env, text);
    search(
        &regex,
        &text,
        options,
        (requested.location, requested.length),
        first,
    )
    .unwrap_or_else(|err| panic!("NSRegularExpression matching failed: {err}"))
}
fn result(env: &mut Environment, expression: id, ranges: Vec<Range>) -> id {
    retain(env, expression);
    let class = env
        .objc
        .get_known_class("NSTextCheckingResult", &mut env.mem);
    env.objc
        .alloc_object(class, Box::new(Match { expression, ranges }), &mut env.mem)
}
fn substitute(text: &[u16], ranges: &[Range], offset: i32, template: &str) -> Vec<u16> {
    assert!(
        template.len() <= 4 * 1024 * 1024,
        "Regex replacement template exceeds 4 MiB"
    );
    let mut out = Vec::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let c = chars
                .next()
                .expect("Trailing escape in regex replacement template");
            out.extend(c.to_string().encode_utf16());
        } else if c == '$' {
            let mut group = chars
                .next()
                .and_then(|c| c.to_digit(10))
                .expect("Invalid regex replacement template") as usize;
            assert!(group < ranges.len(), "Replacement group out of bounds");
            while let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
                let next = group * 10 + digit as usize;
                if next >= ranges.len() {
                    break;
                }
                chars.next();
                group = next;
            }
            let (location, length) = ranges[group];
            if location != NOT_FOUND.0 {
                let start: usize = (i64::from(location) + i64::from(offset))
                    .try_into()
                    .expect("Negative adjusted regex range");
                out.extend_from_slice(
                    text.get(start..start.checked_add(length as usize).unwrap())
                        .expect("Replacement range outside string"),
                );
            }
        } else {
            out.extend(c.to_string().encode_utf16());
        }
        assert!(
            out.len() <= 4 * 1024 * 1024,
            "Regex replacement exceeds UTF-16 output budget"
        );
    }
    out
}
fn replaced(
    env: &mut Environment,
    this: id,
    text: id,
    options: u32,
    requested: NSRange,
    template: id,
) -> (String, u32) {
    let results = find(env, this, text, options, requested, false);
    let text: Vec<u16> = ns_string::to_rust_string(env, text)
        .encode_utf16()
        .collect();
    let template = ns_string::to_rust_string(env, template);
    let mut out = Vec::new();
    let mut previous = 0;
    for ranges in &results {
        let (start, len) = ranges[0];
        out.extend_from_slice(&text[previous..start as usize]);
        out.extend(substitute(&text, ranges, 0, &template));
        assert!(
            out.len() <= 4 * 1024 * 1024,
            "Regex replacement exceeds UTF-16 output budget"
        );
        previous = (start + len) as usize;
    }
    out.extend_from_slice(&text[previous..]);
    assert!(
        out.len() <= 4 * 1024 * 1024,
        "Regex replacement exceeds UTF-16 output budget"
    );
    (String::from_utf16(&out).unwrap(), results.len() as u32)
}
pub const CLASSES: ClassExports = objc_classes! {
(env,this,_cmd);
@implementation NSRegularExpression: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this,Box::<Expression>::default(),&mut env.mem) }
+ (id)regularExpressionWithPattern:(id)pattern options:(NSUInteger)options error:(MutPtr<id>)error {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithPattern:pattern options:options error:error];
    autorelease(env,new)
}
+ (id)escapedPatternForString:(id)text { let text=ns_string::to_rust_string(env,text); let value=ns_string::from_rust_string(env,escape(&text)); autorelease(env,value) }
+ (id)escapedTemplateForString:(id)text {
    let text=ns_string::to_rust_string(env,text); let text=text.replace('\\',"\\\\").replace('$',"\\$");
    let value=ns_string::from_rust_string(env,text); autorelease(env,value)
}
- (id)initWithPattern:(id)pattern options:(NSUInteger)options error:(MutPtr<id>)error {
    if !error.is_null() { env.mem.write(error,nil); }
    assert!(pattern != nil);
    let text = ns_string::to_rust_string(env,pattern);
    let compiled = match compile(&text,options) {
        Ok(regex) => regex,
        Err(reason) => {
            log!("NSRegularExpression compilation failed: {reason}");
            if !error.is_null() {
                let domain = ns_string::get_static_str(env,"NSCocoaErrorDomain");
                let value: id = msg_class![env; NSError errorWithDomain:domain code:2048i32 userInfo:nil];
                env.mem.write(error,value);
            }
            release(env,this); return nil;
        }
    };
    log!("NSRegularExpression compiled: pattern={:?} options={options} groups={}",text,compiled.captures_len()-1);
    let pattern: id = msg![env; pattern copy];
    let old = env.objc.borrow::<Expression>(this).pattern;
    let host = env.objc.borrow_mut::<Expression>(this);
    host.pattern = pattern; host.options = options; host.compiled = Some(Rc::new(compiled));
    release(env,old); this
}
- (id)pattern { env.objc.borrow::<Expression>(this).pattern }
- (NSUInteger)options { env.objc.borrow::<Expression>(this).options }
- (NSUInteger)numberOfCaptureGroups { (env.objc.borrow::<Expression>(this).compiled.as_ref().unwrap().captures_len()-1).try_into().unwrap() }
- (id)copyWithZone:(NSZonePtr)_zone { retain(env,this); this }
- (())dealloc { let pattern=env.objc.borrow::<Expression>(this).pattern; release(env,pattern); env.objc.dealloc_object(this,&mut env.mem); }
- (id)matchesInString:(id)text options:(NSUInteger)options range:(NSRange)requested {
    let results=find(env,this,text,options,requested,false).into_iter().map(|ranges| result(env,this,ranges)).collect();
    let array=ns_array::from_vec(env,results); autorelease(env,array)
}
- (())enumerateMatchesInString:(id)text options:(NSUInteger)options range:(NSRange)requested usingBlock:(MutVoidPtr)block {
    enumerate(env,this,text,options,requested,block);
}
- (id)firstMatchInString:(id)text options:(NSUInteger)options range:(NSRange)requested {
    if let Some(ranges)=find(env,this,text,options,requested,true).pop() { let value=result(env,this,ranges); autorelease(env,value) } else { nil }
}
- (NSUInteger)numberOfMatchesInString:(id)text options:(NSUInteger)options range:(NSRange)requested { find(env,this,text,options,requested,false).len().try_into().unwrap() }
- (NSRange)rangeOfFirstMatchInString:(id)text options:(NSUInteger)options range:(NSRange)requested {
    range(find(env,this,text,options,requested,true).first().map(|found| found[0]).unwrap_or(NOT_FOUND))
}
- (id)stringByReplacingMatchesInString:(id)text options:(NSUInteger)options range:(NSRange)requested withTemplate:(id)template {
    let (text,_)=replaced(env,this,text,options,requested,template); let value=ns_string::from_rust_string(env,text); autorelease(env,value)
}
- (NSUInteger)replaceMatchesInString:(id)text options:(NSUInteger)options range:(NSRange)requested withTemplate:(id)template {
    let (replacement,count)=replaced(env,this,text,options,requested,template); let replacement=ns_string::from_rust_string(env,replacement);
    ()=msg![env; text setString:replacement]; release(env,replacement); count
}
- (id)replacementStringForResult:(id)matched inString:(id)text offset:(NSInteger)offset template:(id)template {
    let ranges=env.objc.borrow::<Match>(matched).ranges.clone();
    let text: Vec<u16>=ns_string::to_rust_string(env,text).encode_utf16().collect(); let template=ns_string::to_rust_string(env,template);
    let replacement=String::from_utf16(&substitute(&text,&ranges,offset,&template)).unwrap(); let value=ns_string::from_rust_string(env,replacement); autorelease(env,value)
}
@end
@implementation NSTextCheckingResult: NSObject
+ (id)regularExpressionCheckingResultWithRanges:(ConstPtr<NSRange>)ranges count:(NSUInteger)count regularExpression:(id)expression {
    assert!(count>0 && count<=1024 && expression!=nil);
    let ranges=(0..count).map(|index| { let value=env.mem.read(ranges+index); (value.location,value.length) }).collect();
    let value=result(env,expression,ranges); autorelease(env,value)
}
- (NSRange)range { range(env.objc.borrow::<Match>(this).ranges[0]) }
- (NSRange)rangeAtIndex:(NSUInteger)index { range(*env.objc.borrow::<Match>(this).ranges.get(index as usize).expect("Capture group index out of bounds")) }
- (NSUInteger)numberOfRanges { env.objc.borrow::<Match>(this).ranges.len().try_into().unwrap() }
- (id)regularExpression { env.objc.borrow::<Match>(this).expression }
- (u64)resultType { 1u64<<10 }
- (id)copyWithZone:(NSZonePtr)_zone { retain(env,this); this }
- (id)resultByAdjustingRangesWithOffset:(NSInteger)offset {
    let host=env.objc.borrow::<Match>(this); let expression=host.expression;
    let ranges=host.ranges.iter().map(|&(location,length)| if location==NOT_FOUND.0 { NOT_FOUND } else {
        ((i64::from(location)+i64::from(offset)).try_into().expect("Invalid adjusted match range"),length)
    }).collect(); let value=result(env,expression,ranges); autorelease(env,value)
}
- (())dealloc { let expression=env.objc.borrow::<Match>(this).expression; release(env,expression); env.objc.dealloc_object(this,&mut env.mem); }
@end
};
