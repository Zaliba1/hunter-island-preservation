/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Declared property metadata in 32-bit Objective-C 2 binaries.
use super::classes::{objc_property, objc_property_t};
use super::{nil, Class, ClassHostObject};
use crate::mem::{ConstPtr, ConstVoidPtr, Mem, MutPtr};
use crate::Environment;
use std::collections::HashSet;

pub(super) fn read_list(mem: &Mem, list: ConstVoidPtr) -> Vec<objc_property_t> {
    if list.is_null() {
        return Vec::new();
    }
    let size = mem.read(list.cast::<u32>());
    let count = mem.read(list.cast::<u32>() + 1);
    assert!(
        size >= 8 && count <= 1_000_000,
        "Invalid Objective-C property list"
    );
    let bytes = count.checked_mul(size).unwrap().checked_add(8).unwrap();
    let _ = mem.bytes_at(list.cast(), bytes);
    (0..count)
        .map(|index| {
            let pointer = MutPtr::<objc_property>::from_bits(
                list.to_bits().checked_add(8 + index * size).unwrap(),
            );
            let entry = mem.read(pointer);
            let _ = mem.cstr_at(entry.name);
            let _ = mem.cstr_at(entry.attributes);
            pointer
        })
        .collect()
}
pub(super) fn class_get_property(
    env: &mut Environment,
    mut class: Class,
    name: ConstPtr<u8>,
) -> objc_property_t {
    if class == nil || name.is_null() {
        return MutPtr::null();
    }
    let name = env.mem.cstr_at(name);
    let initial = env.objc.get_class_name(class);
    let mut seen = HashSet::new();
    while class != nil && seen.insert(class) {
        let Some(object) = env.objc.get_host_object(class) else {
            break;
        };
        let Some(host) = object.as_any().downcast_ref::<ClassHostObject>() else {
            break;
        };
        for &property in host.properties.iter().rev() {
            if env.mem.cstr_at(env.mem.read(property).name) == name {
                log_dbg!(
                    "class_getProperty({}, {}) -> declared property",
                    initial,
                    String::from_utf8_lossy(name)
                );
                return property;
            }
        }
        class = host.superclass;
    }
    // Host ClassTemplates do not yet declare property metadata. Do not infer
    // a property's attributes from the mere existence of similarly named methods.
    log_dbg!(
        "class_getProperty({}, {}) -> no declaration in loaded metadata",
        initial,
        String::from_utf8_lossy(name)
    );
    MutPtr::null()
}
pub(super) fn class_copyPropertyList(
    env: &mut Environment,
    class: Class,
    count: MutPtr<u32>,
) -> MutPtr<objc_property_t> {
    let entries = if class == nil {
        Vec::new()
    } else {
        env.objc
            .get_host_object(class)
            .and_then(|object| object.as_any().downcast_ref::<ClassHostObject>())
            .map(|host| host.properties.iter().rev().copied().collect())
            .unwrap_or_default()
    };
    if !count.is_null() {
        env.mem.write(count, entries.len() as u32);
    }
    if entries.is_empty() {
        return MutPtr::null();
    }
    // A null terminator is supplied in addition to the returned count.
    let output = env
        .mem
        .alloc(((entries.len() + 1) * 4).try_into().unwrap())
        .cast::<objc_property_t>();
    for (index, pointer) in entries.into_iter().chain([MutPtr::null()]).enumerate() {
        env.mem.write(output + index as u32, pointer);
    }
    output
}
pub(super) fn property_getName(env: &mut Environment, property: objc_property_t) -> ConstPtr<u8> {
    if property.is_null() {
        ConstPtr::null()
    } else {
        env.mem.read(property).name
    }
}
pub(super) fn property_getAttributes(
    env: &mut Environment,
    property: objc_property_t,
) -> ConstPtr<u8> {
    if property.is_null() {
        ConstPtr::null()
    } else {
        env.mem.read(property).attributes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(mem: &mut Mem, name: &[u8], attributes: &[u8]) -> ConstVoidPtr {
        let name = mem.alloc_and_write_cstr(name);
        let attributes = mem.alloc_and_write_cstr(attributes);
        let list = mem.alloc(20).cast::<u32>();
        mem.write(list, 12); // padded entry: respect entsize, not only sizeof
        mem.write(list + 1, 1);
        mem.write(list + 2, name.to_bits());
        mem.write(list + 3, attributes.to_bits());
        mem.write(list + 4, 0x12345678);
        list.cast_const().cast()
    }
    #[test]
    fn declared_properties_inheritance_category_precedence_and_copy_ownership() {
        let mut env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let parent = env.objc.get_known_class("NSObject", &mut env.mem);
        let child = env.objc.get_known_class("NSString", &mut env.mem);
        let base = fixture(&mut env.mem, b"item", b"T@,R");
        let category = fixture(&mut env.mem, b"item", b"T@,C,N,V_item");
        let base_entries = read_list(&env.mem, base);
        let category_entries = read_list(&env.mem, category);
        env.objc.borrow_mut::<ClassHostObject>(parent).properties = base_entries.clone();
        let name = env.mem.alloc_and_write_cstr(b"item").cast_const();
        assert_eq!(class_get_property(&mut env, child, name), base_entries[0]);
        env.objc
            .borrow_mut::<ClassHostObject>(child)
            .properties
            .extend(category_entries.clone());
        let found = class_get_property(&mut env, child, name);
        assert_eq!(found, category_entries[0]);
        let attribute = property_getAttributes(&mut env, found);
        assert_eq!(env.mem.cstr_at(attribute), b"T@,C,N,V_item");
        let returned_name = property_getName(&mut env, found);
        assert_eq!(env.mem.cstr_at(returned_name), b"item");
        let count = env.mem.alloc(4).cast::<u32>();
        let copy = class_copyPropertyList(&mut env, child, count);
        assert_eq!(env.mem.read(count), 1); // own declarations only, not inherited
        assert_eq!(env.mem.read(copy), found);
        assert!(env.mem.read(copy + 1).is_null());
        env.mem.free(copy.cast()); // freeing the array does not free properties
        assert_eq!(class_get_property(&mut env, child, name), found);
        let absent = env.mem.alloc_and_write_cstr(b"absent").cast_const();
        assert!(class_get_property(&mut env, child, absent).is_null());
        assert!(class_get_property(&mut env, nil, name).is_null());
        assert!(property_getAttributes(&mut env, MutPtr::null()).is_null());
        assert!(property_getName(&mut env, MutPtr::null()).is_null());
        assert!(class_copyPropertyList(&mut env, nil, count).is_null());
        assert_eq!(env.mem.read(count), 0);
    }
}
