/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Own host setters, Thumb setter and packed ivar fixtures.
use super::*;
use crate::objc::{objc_classes, ClassExports, HostObject, NSZonePtr};
use std::cell::Cell;
use std::rc::Rc;
#[derive(Default)]
struct Probe {
    float: f32,
    double: f64,
    signed: i32,
    wide: u64,
    flag: bool,
    object: id,
    nil_calls: u32,
    undefined_calls: u32,
    dropped: Option<Rc<Cell<u32>>>,
}
impl HostObject for Probe {}
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(counter) = &self.dropped {
            counter.set(counter.get() + 1);
        }
    }
}
pub(crate) const CLASSES: ClassExports = objc_classes! {
(env,this,_cmd);
@implementation _touchHLE_KVCProbe: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this,Box::<Probe>::default(),&mut env.mem) }
- (())setFloat:(f32)value { env.objc.borrow_mut::<Probe>(this).float=value; }
- (())setDouble:(f64)value { env.objc.borrow_mut::<Probe>(this).double=value; }
- (())setSigned:(i32)value { env.objc.borrow_mut::<Probe>(this).signed=value; }
- (())setWide:(u64)value { env.objc.borrow_mut::<Probe>(this).wide=value; }
- (())setFlag:(bool)value { env.objc.borrow_mut::<Probe>(this).flag=value; }
- (())setObject:(id)value { retain(env,value); let old=env.objc.borrow::<Probe>(this).object; env.objc.borrow_mut::<Probe>(this).object=value; release(env,old); }
- (())setNilValueForKey:(id)_key { env.objc.borrow_mut::<Probe>(this).nil_calls+=1; }
- (())setValue:(id)_value forUndefinedKey:(id)_key { env.objc.borrow_mut::<Probe>(this).undefined_calls+=1; }
- (())dealloc {
    let held=env.objc.borrow::<Probe>(this).object; release(env,held);
    let raw: id=env.mem.read(MutVoidPtr::from_bits(this.to_bits()+20).cast()); release(env,raw);
    env.objc.dealloc_object(this,&mut env.mem);
}
@end
};
fn set(env: &mut Environment, target: id, key: &str, value: id) {
    let key = ns_string::from_rust_string(env, key.into());
    () = msg![env; target setValue:value forKey:key];
    release(env, key);
}
fn main(env: &mut Environment) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let class = env.objc.get_known_class("_touchHLE_KVCProbe", &mut env.mem);
    env.objc.borrow_mut::<ClassHostObject>(class).instance_size = 40;
    for (name, offset, encoding, size) in [
        ("_rawFloat", 16, "f", 4),
        ("_rawObject", 20, "@", 4),
        ("_rawByte", 24, "c", 1),
    ] {
        let offset = env.mem.alloc_and_write(offset).cast_const();
        let encoding = env
            .mem
            .alloc_and_write_cstr(encoding.as_bytes())
            .cast_const();
        let host = env.objc.borrow_mut::<ClassHostObject>(class);
        host.ivars.insert(name.into(), (offset, 1));
        host.ivar_types.insert(name.into(), (encoding, size));
    }
    // Own Thumb setter: str r2,[r0,#8]; str r3,[r0,#12]; bx lr.
    // It checks actual f64 argument alignment in the 32-bit guest ABI.
    let code = env.mem.alloc(6);
    env.mem
        .bytes_at_mut(code.cast(), 6)
        .copy_from_slice(&[0x82, 0x60, 0xc3, 0x60, 0x70, 0x47]);
    let setter = env
        .objc
        .register_host_selector("setGuestDouble:".into(), &mut env.mem);
    let signature = env.mem.alloc_and_write_cstr(b"v16@0:4d8").cast_const();
    let host = env.objc.borrow_mut::<ClassHostObject>(class);
    host.methods.insert(
        setter,
        IMP::Guest(crate::abi::GuestFunction::from_addr_and_thumb_flag(
            code.to_bits(),
            true,
        )),
    );
    host.guest_method_signatures.insert(setter, signature);
    let target: id = msg![env; class new];
    let number: id = msg_class![env; NSNumber numberWithDouble:39.5f64];
    set(env, target, "float", number);
    set(env, target, "double", number);
    set(env, target, "guestDouble", number);
    assert_eq!(env.objc.borrow::<Probe>(target).float, 39.5);
    assert_eq!(env.objc.borrow::<Probe>(target).double, 39.5);
    assert_eq!(
        env.mem
            .read::<f64, true>(MutVoidPtr::from_bits(target.to_bits() + 8).cast()),
        39.5
    );
    let signed: id = msg_class![env; NSNumber numberWithInt:(-17i32)];
    set(env, target, "signed", signed);
    let wide: id = msg_class![env; NSNumber numberWithUnsignedLongLong:0xfedcba9876543210u64];
    set(env, target, "wide", wide);
    set(env, target, "flag", number);
    assert_eq!(env.objc.borrow::<Probe>(target).signed, -17);
    assert_eq!(env.objc.borrow::<Probe>(target).wide, 0xfedcba9876543210);
    assert!(env.objc.borrow::<Probe>(target).flag);
    set(env, target, "object", number);
    assert_eq!(env.objc.borrow::<Probe>(target).object, number);
    set(env, target, "object", nil);
    assert_eq!(env.objc.borrow::<Probe>(target).object, nil);
    set(env, target, "double", nil);
    assert_eq!(env.objc.borrow::<Probe>(target).nil_calls, 1);
    assert_eq!(env.objc.borrow::<Probe>(target).double, 39.5);
    set(env, target, "missing", number);
    set(env, target, "", number);
    assert_eq!(env.objc.borrow::<Probe>(target).undefined_calls, 2);
    let numeric_string = ns_string::from_rust_string(env, " 4294967295units".into());
    set(env, target, "wide", numeric_string);
    assert_eq!(env.objc.borrow::<Probe>(target).wide, 4294967295);
    release(env, numeric_string);
    let signed_string = ns_string::from_rust_string(env, "-17suffix".into());
    set(env, target, "rawByte", signed_string);
    assert_eq!(
        env.mem
            .read(MutVoidPtr::from_bits(target.to_bits() + 24).cast::<i8>()),
        -17
    );
    release(env, signed_string);
    echo!("KVC_TYPED_HOST_AND_ARM_SETTERS_PASS");

    set(env, target, "rawFloat", number);
    assert_eq!(
        env.mem
            .read::<f32, true>(MutVoidPtr::from_bits(target.to_bits() + 16).cast()),
        39.5
    );
    let adjacent = MutVoidPtr::from_bits(target.to_bits() + 24);
    env.mem.bytes_at_mut(adjacent.cast(), 4).fill(0xab);
    set(env, target, "rawByte", signed);
    assert_eq!(
        env.mem.bytes_at(adjacent.cast(), 4),
        [0xef, 0xab, 0xab, 0xab]
    );
    let dropped = Rc::new(Cell::new(0));
    let held: id = msg![env; class new];
    env.objc.borrow_mut::<Probe>(held).dropped = Some(dropped.clone());
    set(env, target, "rawObject", held);
    release(env, held);
    assert_eq!(dropped.get(), 0);
    set(env, target, "rawObject", held);
    assert_eq!(dropped.get(), 0);
    set(env, target, "rawObject", nil);
    assert_eq!(dropped.get(), 1);
    release(env, target);
    release(env, pool);
    env.mem.free(code);
    env.mem.free(signature.cast_mut().cast());
    echo!("KVC_IVAR_WIDTH_OWNERSHIP_NIL_AND_UNDEFINED_PASS");
    echo!("KVC_FRAMEWORK_PROBE_PASS");
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
    fn typed_host_arm_setters_ivar_widths_and_lifetimes() {
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
