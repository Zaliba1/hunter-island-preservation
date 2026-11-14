/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSInvocation`: owned argument slots and scalar/pointer ARM return values.
use crate::abi::{extend_stack_for_args, write_next_arg, GuestArg};
use crate::cpu::Cpu;
use crate::frameworks::foundation::NSInteger;
use crate::libc::string::strdup;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr};
use crate::msg;
pub use crate::objc::invocation_probe::framework_probe;
pub(crate) use crate::objc::invocation_probe::CLASSES as PROBE_CLASSES;
use crate::objc::{
    autorelease, id, nil, objc_classes, objc_msgSend, release, retain, ClassExports, HostObject,
    SEL,
};
use crate::Environment;

struct NSInvocationHostObject {
    sig: id,
    argument_types: Vec<String>,
    target: id,
    selector: Option<SEL>,
    // Each slot starts at zero, including omitted optional object arguments.
    arguments: Vec<MutVoidPtr>,
    return_value: Vec<u8>,
    arguments_retained: bool,
    retained_objects: Vec<(usize, id)>,
    copied_strings: Vec<(usize, MutPtr<u8>)>,
}
impl HostObject for NSInvocationHostObject {}

pub(super) fn value_size(encoding: &str) -> u32 {
    match encoding {
        "v" => 0,
        "c" | "C" | "B" => 1,
        "s" | "S" => 2,
        "@" | "#" | ":" | "*" | "i" | "I" | "l" | "L" | "f" => 4,
        "q" | "Q" | "d" => 8,
        _ if encoding.starts_with('^') => 4,
        _ => {
            unimplemented!("NSInvocation type {encoding}; aggregates and blocks are not supported")
        }
    }
}
fn own_slot(env: &mut Environment, this: id, index: usize) {
    let host = env.objc.borrow::<NSInvocationHostObject>(this);
    let slot = host.arguments[index];
    match host.argument_types[index].as_str() {
        "@" => {
            let object: id = env.mem.read(slot.cast());
            retain(env, object);
            env.objc
                .borrow_mut::<NSInvocationHostObject>(this)
                .retained_objects
                .push((index, object));
        }
        "*" => {
            let string: MutPtr<u8> = env.mem.read(slot.cast());
            if !string.is_null() {
                let copy = strdup(env, string.cast_const());
                env.mem.write(slot.cast(), copy);
                env.objc
                    .borrow_mut::<NSInvocationHostObject>(this)
                    .copied_strings
                    .push((index, copy));
            }
        }
        _ => {}
    }
}
fn replace_argument(env: &mut Environment, this: id, index: usize, source: MutVoidPtr) {
    let host = env.objc.borrow::<NSInvocationHostObject>(this);
    let size = value_size(&host.argument_types[index]);
    let destination = host.arguments[index];
    // Snapshot first: the caller's storage may alias the existing slot.
    let bytes = env.mem.bytes_at(source.cast(), size).to_vec();
    let host = env.objc.borrow_mut::<NSInvocationHostObject>(this);
    let retained = host.arguments_retained;
    let old_object = host
        .retained_objects
        .iter()
        .position(|(i, _)| *i == index)
        .map(|at| host.retained_objects.swap_remove(at).1);
    let old_string = host
        .copied_strings
        .iter()
        .position(|(i, _)| *i == index)
        .map(|at| host.copied_strings.swap_remove(at).1);
    env.mem
        .bytes_at_mut(destination.cast(), size)
        .copy_from_slice(&bytes);
    // Acquire the new ownership before releasing the previous object.
    if retained {
        own_slot(env, this, index);
    }
    if let Some(object) = old_object {
        release(env, object);
    }
    if let Some(string) = old_string {
        env.mem.free(string.cast());
    }
}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation NSInvocation: NSObject
+ (id)invocationWithMethodSignature:(id)sig {
    assert!(!sig.is_null(), "NSInvocation requires a method signature");
    let count: u32 = msg![env; sig numberOfArguments];
    assert!((2..=1024).contains(&count), "Invalid invocation argument count");
    let mut argument_types = Vec::new();
    let mut arguments = Vec::new();
    for index in 0..count {
        let encoding: ConstPtr<u8> = msg![env; sig getArgumentTypeAtIndex:index];
        let encoding = env.mem.cstr_at_utf8(encoding).unwrap().to_owned();
        arguments.push(env.mem.calloc(value_size(&encoding)));
        argument_types.push(encoding);
    }
    let ret: ConstPtr<u8> = msg![env; sig methodReturnType];
    let return_size = value_size(env.mem.cstr_at_utf8(ret).unwrap());
    retain(env,sig);
    let host = Box::new(NSInvocationHostObject {
        sig,argument_types,arguments,target:nil,selector:None,
        return_value:vec![0;return_size as usize],arguments_retained:false,
        retained_objects:Vec::new(),copied_strings:Vec::new(),
    });
    let result = env.objc.alloc_object(this,host,&mut env.mem);
    autorelease(env,result)
}
- (id)methodSignature { env.objc.borrow::<NSInvocationHostObject>(this).sig }
- (id)target { env.objc.borrow::<NSInvocationHostObject>(this).target }
- (())setTarget:(id)target {
    let host = env.objc.borrow_mut::<NSInvocationHostObject>(this);
    let old = host.target;
    host.target = target;
    if host.arguments_retained { retain(env,target); release(env,old); }
}
- (SEL)selector { env.objc.borrow::<NSInvocationHostObject>(this).selector.unwrap_or_else(|| SEL::from_regs(&[0])) }
- (())setSelector:(SEL)selector { env.objc.borrow_mut::<NSInvocationHostObject>(this).selector=Some(selector); }
- (bool)argumentsRetained { env.objc.borrow::<NSInvocationHostObject>(this).arguments_retained }
- (())retainArguments {
    let host = env.objc.borrow::<NSInvocationHostObject>(this);
    if !host.arguments_retained {
        let target=host.target;
        let count=host.arguments.len();
        retain(env,target);
        for index in 2..count { own_slot(env,this,index); }
        env.objc.borrow_mut::<NSInvocationHostObject>(this).arguments_retained=true;
    }
}
- (())setArgument:(MutVoidPtr)source atIndex:(NSInteger)index {
    let count=env.objc.borrow::<NSInvocationHostObject>(this).arguments.len();
    assert!(index>=0 && (index as usize)<count,"NSInvocation argument index out of bounds");
    match index {
        0 => { let target:id=env.mem.read(source.cast()); ()=msg![env; this setTarget:target]; }
        1 => { let selector:SEL=env.mem.read(source.cast()); ()=msg![env; this setSelector:selector]; }
        _ => replace_argument(env,this,index as usize,source),
    }
}
- (())getArgument:(MutVoidPtr)destination atIndex:(NSInteger)index {
    let host=env.objc.borrow::<NSInvocationHostObject>(this);
    assert!(index>=0 && (index as usize)<host.arguments.len(),"NSInvocation argument index out of bounds");
    match index {
        0 => env.mem.write(destination.cast(),host.target),
        1 => env.mem.write(destination.cast(),host.selector.unwrap_or_else(|| SEL::from_regs(&[0]))),
        _ => {
            let size=value_size(&host.argument_types[index as usize]);
            let bytes=env.mem.bytes_at(host.arguments[index as usize].cast(),size).to_vec();
            env.mem.bytes_at_mut(destination.cast(),size).copy_from_slice(&bytes);
        }
    }
}
- (())getReturnValue:(MutVoidPtr)destination {
    let value=env.objc.borrow::<NSInvocationHostObject>(this).return_value.clone();
    if !value.is_empty() { env.mem.bytes_at_mut(destination.cast(),value.len() as u32).copy_from_slice(&value); }
}
- (())setReturnValue:(MutVoidPtr)source {
    let size=env.objc.borrow::<NSInvocationHostObject>(this).return_value.len();
    if size!=0 {
        let value=env.mem.bytes_at(source.cast(),size as u32).to_vec();
        env.objc.borrow_mut::<NSInvocationHostObject>(this).return_value=value;
    }
}
- (())invokeWithTarget:(id)target { ()=msg![env; this setTarget:target]; ()=msg![env; this invoke]; }
- (())invoke {
    let host=env.objc.borrow::<NSInvocationHostObject>(this);
    let target=host.target;
    // A nil target does not dispatch and yields zero scalar/pointer results.
    if target==nil { env.objc.borrow_mut::<NSInvocationHostObject>(this).return_value.fill(0); return; }
    let selector=host.selector.expect("NSInvocation selector has not been set");
    assert!(!selector.is_null(),"NSInvocation selector has not been set");
    let types=host.argument_types.clone();
    assert_eq!(&types[..2], &["@", ":"]);
    let slots=host.arguments.clone();
    let reg_count=types.iter().map(|t| value_size(t).div_ceil(4) as usize).sum();
    let old_sp=extend_stack_for_args(reg_count,env.cpu.regs_mut());
    let mut offset=0;
    write_next_arg(&mut offset,env.cpu.regs_mut(),&mut env.mem,target);
    write_next_arg(&mut offset,env.cpu.regs_mut(),&mut env.mem,selector);
    for index in 2..types.len() {
        let slot=slots[index];
        let value=match types[index].as_str() {
            "c" => env.mem.read::<i8,true>(slot.cast()) as u32,
            "C" | "B" => env.mem.read::<u8,true>(slot.cast()) as u32,
            "s" => env.mem.read::<i16,true>(slot.cast()) as u32,
            "S" => env.mem.read::<u16,true>(slot.cast()) as u32,
            _ if value_size(&types[index])==8 => {
                let value=env.mem.read::<u64,true>(slot.cast());
                write_next_arg(&mut offset,env.cpu.regs_mut(),&mut env.mem,value);
                continue;
            }
            _ => env.mem.read::<u32,true>(slot.cast()),
        };
        write_next_arg(&mut offset,env.cpu.regs_mut(),&mut env.mem,value);
    }
    objc_msgSend(env,target,selector);
    // Snapshot registers before any further Objective-C calls can change them.
    let regs=env.cpu.regs();
    let mut bytes=Vec::from(regs[0].to_le_bytes());
    bytes.extend_from_slice(&regs[1].to_le_bytes());
    let host=env.objc.borrow_mut::<NSInvocationHostObject>(this);
    let size=host.return_value.len();
    host.return_value.copy_from_slice(&bytes[..size]);
    env.cpu.regs_mut()[Cpu::SP]=old_sp;
}
- (())dealloc {
    let host=env.objc.borrow_mut::<NSInvocationHostObject>(this);
    let sig=host.sig;
    let target=host.target;
    let retained=host.arguments_retained;
    let objects=std::mem::take(&mut host.retained_objects);
    let strings=std::mem::take(&mut host.copied_strings);
    let slots=std::mem::take(&mut host.arguments);
    release(env,sig);
    if retained { release(env,target); }
    for (_,object) in objects { release(env,object); }
    for (_,string) in strings { env.mem.free(string.cast()); }
    for slot in slots { env.mem.free(slot); }
    env.objc.dealloc_object(this,&mut env.mem);
}
@end
};
