/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Own Thumb fixtures, no game code: omitted event, char return and stack ABI.
use super::{id, nil, objc_classes, release, ClassExports};
use crate::cpu::Cpu;
use crate::mem::MutVoidPtr;
use crate::objc::{msg_class, ClassHostObject, IMP};
use crate::{msg, Environment};
pub(crate) const CLASSES: ClassExports = objc_classes! {
(env,this,_cmd);
@implementation _touchHLE_InvocationProbe: NSObject
@end
@implementation _touchHLE_LayoutParent: NSObject
@end
@implementation _touchHLE_LayoutChild: _touchHLE_LayoutParent
@end
@implementation _touchHLE_LayoutGrandchild: _touchHLE_LayoutChild
@end
};
fn invocation(env: &mut Environment, signature: &[u8]) -> id {
    let encoding = env.mem.alloc_and_write_cstr(signature);
    let sig: id = msg_class![env; NSMethodSignature signatureWithObjCTypes:(encoding.cast_const())];
    env.mem.free(encoding.cast());
    msg_class![env; NSInvocation invocationWithMethodSignature:sig]
}
fn main(env: &mut Environment) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let class = env
        .objc
        .get_known_class("_touchHLE_InvocationProbe", &mut env.mem);
    env.objc.borrow_mut::<ClassHostObject>(class).instance_size = 32;
    let target: id = msg![env; class new];
    // First entry records the object parameters and returns char 1. Second
    // records a 64-bit argument split across r3/stack and the following word.
    let code = env.mem.alloc(22);
    env.mem.bytes_at_mut(code.cast(), 22).copy_from_slice(&[
        0x82, 0x60, 0xc3, 0x60, 0x01, 0x20, 0x70, 0x47, 0x03, 0x61, 0x00, 0x9b, 0x43, 0x61, 0x01,
        0x9b, 0x83, 0x61, 0x10, 0x46, 0x70, 0x47,
    ]);
    let selector = env
        .objc
        .register_host_selector("recordTouch:event:".into(), &mut env.mem);
    let wide = env
        .objc
        .register_host_selector("recordWord:wide:last:".into(), &mut env.mem);
    let host = env.objc.borrow_mut::<ClassHostObject>(class);
    for (sel, offset) in [(selector, 0), (wide, 8)] {
        host.methods.insert(
            sel,
            IMP::Guest(crate::abi::GuestFunction::from_addr_and_thumb_flag(
                code.to_bits() + offset,
                true,
            )),
        );
    }
    let call = invocation(env, b"c16@0:4@8@12");
    () = msg![env; call setTarget:target];
    () = msg![env; call setSelector:selector];
    () = msg![env; call setSelector:selector];
    let object = crate::frameworks::foundation::ns_string::from_rust_string(env, "touch".into());
    let slot = env.mem.alloc_and_write(object);
    () = msg![env; call setArgument:(slot.cast::<std::ffi::c_void>()) atIndex:2i32];
    let guard = env.mem.alloc_and_write(0xaabbccddu32);
    () = msg![env; call getArgument:(guard.cast::<std::ffi::c_void>()) atIndex:3i32];
    assert_eq!(env.mem.read(guard), 0);
    () = msg![env; call retainArguments];
    () = msg![env; call retainArguments];
    release(env, object);
    () = msg![env; call invoke];
    assert_eq!(
        env.mem
            .read::<id, true>(MutVoidPtr::from_bits(target.to_bits() + 8).cast()),
        object
    );
    assert_eq!(
        env.mem
            .read::<id, true>(MutVoidPtr::from_bits(target.to_bits() + 12).cast()),
        nil
    );
    env.mem.write(guard, 0xaabbccddu32);
    () = msg![env; call getReturnValue:(guard.cast::<std::ffi::c_void>())];
    assert_eq!(env.mem.read(guard), 0xaabbcc01); // writes exactly one byte
    let sig: id = msg![env; call methodSignature];
    let return_length: u32 = msg![env; sig methodReturnLength];
    assert_eq!(return_length, 1);
    env.mem.write(slot, nil);
    () = msg![env; call setArgument:(slot.cast::<std::ffi::c_void>()) atIndex:2i32];
    () = msg![env; call invoke];
    assert_eq!(
        env.mem
            .read::<id, true>(MutVoidPtr::from_bits(target.to_bits() + 8).cast()),
        nil
    );
    echo!("INVOCATION_OMITTED_EVENT_CHAR_RETURN_AND_REPLACEMENT_PASS");
    let call = invocation(env, b"I24@0:4I8Q12I20");
    () = msg![env; call setSelector:wide];
    let word = env.mem.alloc_and_write(0x87654321u32);
    let wide_value = env.mem.alloc_and_write(0xfedcba9876543210u64);
    let last = env.mem.alloc_and_write(0x13579bdfu32);
    () = msg![env; call setArgument:(word.cast::<std::ffi::c_void>()) atIndex:2i32];
    () = msg![env; call setArgument:(wide_value.cast::<std::ffi::c_void>()) atIndex:3i32];
    () = msg![env; call setArgument:(last.cast::<std::ffi::c_void>()) atIndex:4i32];
    let sp = env.cpu.regs()[Cpu::SP];
    () = msg![env; call invokeWithTarget:target];
    assert_eq!(env.cpu.regs()[Cpu::SP], sp);
    () = msg![env; call getReturnValue:(guard.cast::<std::ffi::c_void>())];
    assert_eq!(env.mem.read(guard), 0x87654321);
    assert_eq!(
        env.mem
            .read::<u64, true>(MutVoidPtr::from_bits(target.to_bits() + 16).cast()),
        0xfedcba9876543210
    );
    assert_eq!(
        env.mem
            .read::<u32, true>(MutVoidPtr::from_bits(target.to_bits() + 24).cast()),
        0x13579bdf
    );
    () = msg![env; call invokeWithTarget:nil];
    () = msg![env; call getReturnValue:(guard.cast::<std::ffi::c_void>())];
    assert_eq!(env.mem.read(guard), 0);
    for p in [
        slot.cast(),
        guard.cast(),
        word.cast(),
        wide_value.cast(),
        last.cast(),
    ] {
        env.mem.free(p);
    }
    scanner_checks(env);
    layout_checks(env);
    crate::frameworks::foundation::ns_array::selector_sort_probe(env);
    crate::frameworks::foundation::ns_array::array_factory_probe(env);
    crate::frameworks::foundation::ns_set::equality_probe(env);
    crate::frameworks::foundation::ns_dictionary::buffer_constructor_probe(env);
    crate::frameworks::core_foundation::cf_dictionary::apply_function_probe(env);
    crate::frameworks::foundation::ns_value::signed_archive_roundtrip_probe(env);
    crate::frameworks::foundation::ns_property_list_serialization::legacy_error_output_probe(env);
    crate::frameworks::foundation::ns_data::writing_error_probe(env);
    release(env, pool);
    release(env, target);
    env.mem.free(code);
    echo!("INVOCATION_STACK_ABI_AND_NIL_TARGET_PASS");
    crate::frameworks::core_graphics::cg_bitmap_context::creation_probe(env);
    super::leaf_getter::probe(env);
    echo!("INVOCATION_FRAMEWORK_PROBE_PASS");
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
    #[test]
    fn thumb_optional_object_char_return_and_stack_arguments() {
        let env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.run_framework_probe(super::main);
    }
}

fn scanner_checks(env: &mut Environment) {
    let digits: id = msg_class![env; NSCharacterSet decimalDigitCharacterSet];
    let inverted: id = msg![env; digits invertedSet];
    for (code, expected) in [
        (0x0030u16, true),
        (0x0669, true),
        (0x0966, true),
        (0xff19, true),
        (0x00b2, false),
        (0x2163, false),
        (0x0041, false),
    ] {
        let member: bool = msg![env; digits characterIsMember:code];
        let inverse: bool = msg![env; inverted characterIsMember:code];
        assert_eq!(member, expected);
        assert_eq!(inverse, !expected);
    }
    let string =
        crate::frameworks::foundation::ns_string::from_rust_string(env, "poi_123.tmx".into());
    let scanner: id = msg_class![env; NSScanner scannerWithString:string];
    release(env, string);
    let discarded: bool = msg![env; scanner scanUpToCharactersFromSet:digits intoString:(crate::mem::MutPtr::<id>::null())];
    assert!(discarded);
    let output = env.mem.alloc_and_write(nil);
    let success: bool = msg![env; scanner scanCharactersFromSet:digits intoString:output];
    assert!(success);
    let scanned: id = env.mem.read(output);
    assert_eq!(
        crate::frameworks::foundation::ns_string::to_rust_string(env, scanned),
        "123"
    );
    let location: u32 = msg![env; scanner scanLocation];
    assert_eq!(location, 7);
    env.mem.free(output.cast());
    echo!("DECIMAL_CHARACTER_SET_AND_SCANNER_PASS");
    for (text, expected, position) in [
        ("  0x12Af;", Some(0x12af), 8),
        (" ffffffffffff!", Some(u32::MAX), 13),
        ("  nope", None, 0),
        ("", None, 0),
        ("  0X", None, 0),
    ] {
        let string = crate::frameworks::foundation::ns_string::from_rust_string(env, text.into());
        let scanner: id = msg_class![env; NSScanner scannerWithString:string];
        release(env, string);
        let output = env.mem.alloc_and_write(0x13579bdfu32);
        let success: bool = msg![env; scanner scanHexInt:output];
        assert_eq!(success, expected.is_some());
        assert_eq!(env.mem.read(output), expected.unwrap_or(0x13579bdf));
        let location: u32 = msg![env; scanner scanLocation];
        assert_eq!(location, position);
        () = msg![env; scanner setScanLocation:0u32];
        let discard: bool = msg![env; scanner scanHexInt:(crate::mem::MutPtr::<u32>::null())];
        assert_eq!(discard, success);
        env.mem.free(output.cast());
    }
    echo!("SCANNER_HEX_PREFIX_OVERFLOW_LOCATION_NULL_OUTPUT_PASS");
}

fn layout_checks(env: &mut Environment) {
    let parent = env
        .objc
        .get_known_class("_touchHLE_LayoutParent", &mut env.mem);
    let child = env
        .objc
        .get_known_class("_touchHLE_LayoutChild", &mut env.mem);
    let grandchild = env
        .objc
        .get_known_class("_touchHLE_LayoutGrandchild", &mut env.mem);
    env.objc.borrow_mut::<ClassHostObject>(parent).instance_size = 44;
    let field = env.mem.alloc_and_write(8u32);
    let grand_field = env.mem.alloc_and_write(16u32);
    let getter = env.mem.alloc(12);
    // Own Thumb getter: ldr r1,[pc,#4]; ldr r1,[r1]; ldr r0,[r0,r1]; bx lr;
    // literal points to the same compiler offset global as the metadata.
    env.mem
        .bytes_at_mut(getter.cast(), 8)
        .copy_from_slice(&[0x01, 0x49, 0x09, 0x68, 0x40, 0x58, 0x70, 0x47]);
    env.mem.write(
        crate::mem::MutPtr::<u32>::from_bits(getter.to_bits() + 8),
        field.to_bits(),
    );
    let selector = env
        .objc
        .register_host_selector("slidValue".into(), &mut env.mem);
    {
        let h = env.objc.borrow_mut::<ClassHostObject>(child);
        h.instance_start = 4;
        h.instance_size = 16;
        h.ivars.insert("_field".into(), (field.cast_const(), 2));
        h.methods.insert(
            selector,
            IMP::Guest(crate::abi::GuestFunction::from_addr_and_thumb_flag(
                getter.to_bits(),
                true,
            )),
        );
    }
    {
        let h = env.objc.borrow_mut::<ClassHostObject>(grandchild);
        h.instance_start = 16;
        h.instance_size = 24;
        h.ivars
            .insert("_wide".into(), (grand_field.cast_const(), 3));
    }
    env.objc.reconcile_class_ivars(child, &mut env.mem);
    env.objc.reconcile_class_ivars(grandchild, &mut env.mem);
    assert_eq!(
        env.objc.borrow::<ClassHostObject>(child).ivars["_field"].0,
        field.cast_const()
    );
    assert_eq!(env.mem.read(field), 48);
    assert_eq!(env.mem.read(grand_field), 56);
    assert_eq!(env.objc.borrow::<ClassHostObject>(child).instance_size, 56);
    assert_eq!(
        env.objc.borrow::<ClassHostObject>(grandchild).instance_size,
        64
    );
    let object: id = msg![env; grandchild new];
    env.mem.write(
        crate::mem::MutPtr::<u32>::from_bits(object.to_bits() + 8),
        0x11111111,
    );
    env.mem.write(
        crate::mem::MutPtr::<u32>::from_bits(object.to_bits() + 48),
        0x87654321,
    );
    let value: u32 = crate::objc::msg_send(env, (object, selector));
    assert_eq!(value, 0x87654321);
    let address = env
        .objc
        .object_lookup_ivar(&env.mem, object, &"_field".to_owned())
        .unwrap();
    assert_eq!(address.to_bits(), object.to_bits() + 48);
    // A second realization must not slide the fields again.
    env.objc.reconcile_class_ivars(child, &mut env.mem);
    assert_eq!(env.mem.read(field), 48);
    release(env, object);
    for p in [field.cast(), grand_field.cast(), getter] {
        env.mem.free(p);
    }
    echo!("IVAR_INHERITANCE_SLIDE_SHARED_OFFSET_THUMB_GETTER_AND_ALIGNMENT_PASS");
}
