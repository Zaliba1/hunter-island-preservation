/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Read the stable 32-bit Objective-C 2 protocol prefix and adoption lists.
//! Dynamic protocol construction and Protocol-object messaging are not supplied.
use super::{nil, Class, ClassHostObject, ObjC};
use crate::mem::{ConstPtr, ConstVoidPtr, Mem, SafeRead};
use std::collections::HashSet;

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct ProtocolPrefix {
    _isa: ConstVoidPtr,
    name: ConstPtr<u8>,
    inherited: ConstVoidPtr,
}
unsafe impl SafeRead for ProtocolPrefix {}

pub(crate) fn name(mem: &Mem, protocol: ConstVoidPtr) -> ConstPtr<u8> {
    assert!(
        !protocol.is_null(),
        "Protocol name requires a non-null protocol"
    );
    let prefix: ProtocolPrefix = mem.read(protocol.cast());
    let _ = mem.cstr_at(prefix.name);
    prefix.name
}

pub(super) fn read_list(mem: &Mem, list: ConstVoidPtr) -> Vec<ConstVoidPtr> {
    if list.is_null() {
        return Vec::new();
    }
    let count = mem.read(list.cast::<u32>());
    let bytes = count.checked_add(1).unwrap().checked_mul(4).unwrap();
    let _ = mem.bytes_at(list.cast(), bytes);
    let entries = list.cast::<ConstVoidPtr>() + 1;
    (0..count)
        .map(|index| mem.read(entries + index))
        .filter(|p| !p.is_null())
        .collect()
}

fn adopts(mem: &Mem, protocols: &[ConstVoidPtr], name: &[u8]) -> bool {
    let mut pending = protocols.to_vec();
    let mut visited = HashSet::new();
    while let Some(protocol) = pending.pop() {
        if protocol.is_null() || !visited.insert(protocol) {
            continue;
        }
        let prefix: ProtocolPrefix = mem.read(protocol.cast());
        if mem.cstr_at(prefix.name) == name {
            return true;
        }
        pending.extend(read_list(mem, prefix.inherited));
    }
    false
}

impl ObjC {
    pub fn class_and_superclasses_conform_to_protocol(
        &self,
        mut class: Class,
        protocol: ConstVoidPtr,
        mem: &Mem,
    ) -> bool {
        if class == nil || protocol.is_null() {
            return false;
        }
        let prefix: ProtocolPrefix = mem.read(protocol.cast());
        let name = mem.cstr_at(prefix.name);
        let mut visited = HashSet::new();
        while class != nil && visited.insert(class) {
            let Some(host) = self.get_host_object(class) else {
                return false;
            };
            let Some(host) = host.as_any().downcast_ref::<ClassHostObject>() else {
                return false;
            };
            if adopts(mem, &host.protocols, name)
                || (host.name == "NSObject" && name == b"NSObject")
            {
                return true;
            }
            class = host.superclass;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn protocol(mem: &mut Mem, name: &[u8]) -> ConstVoidPtr {
        let name = mem.alloc_and_write_cstr(name).cast_const();
        let raw = mem.alloc(12);
        mem.write(
            raw.cast(),
            ProtocolPrefix {
                _isa: ConstPtr::null(),
                name,
                inherited: ConstPtr::null(),
            },
        );
        raw.cast_const()
    }
    fn list(mem: &mut Mem, values: &[ConstVoidPtr]) -> ConstVoidPtr {
        let raw = mem.alloc((values.len() as u32 + 1) * 4).cast::<u32>();
        mem.write(raw, values.len() as u32);
        for (index, &value) in values.iter().enumerate() {
            mem.write((raw + index as u32 + 1).cast(), value);
        }
        raw.cast().cast_const()
    }
    #[test]
    fn protocol_adoption_names_transitivity_and_cycles() {
        let mut mem = Mem::new();
        mem.reserve(0, 4096);
        let parent = protocol(&mut mem, b"Parent");
        let child = protocol(&mut mem, b"Child");
        let inherited = list(&mut mem, &[parent]);
        mem.write((child.cast::<u32>() + 2).cast().cast_mut(), inherited);
        let cycle = list(&mut mem, &[child]);
        mem.write((parent.cast::<u32>() + 2).cast().cast_mut(), cycle);
        assert!(adopts(&mem, &[child], b"Parent"));
        assert!(adopts(&mem, &[parent], b"Child"));
        assert!(!adopts(&mem, &[parent], b"Unknown"));
        assert!(read_list(&mem, ConstPtr::null()).is_empty());
    }
    #[test]
    fn class_and_instance_conformance_with_superclass_and_duplicate_protocols() {
        use crate::objc::{msg, msg_class, TrivialHostObject};
        let mut environment = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut environment;
        let parent = protocol(&mut env.mem, b"Parent");
        let duplicate = protocol(&mut env.mem, b"Parent");
        let child = protocol(&mut env.mem, b"Child");
        let unrelated = protocol(&mut env.mem, b"Unrelated");
        let inherited = list(&mut env.mem, &[parent]);
        env.mem
            .write((child.cast::<u32>() + 2).cast().cast_mut(), inherited);
        let base = env.objc.get_known_class("NSString", &mut env.mem);
        env.objc
            .borrow_mut::<ClassHostObject>(base)
            .protocols
            .push(child);
        let class = env.objc.get_known_class("NSMutableString", &mut env.mem);
        let instance = env
            .objc
            .alloc_object(class, Box::new(TrivialHostObject), &mut env.mem);
        assert!(env
            .objc
            .class_and_superclasses_conform_to_protocol(class, duplicate, &env.mem));
        let conforms: bool = msg![env; instance conformsToProtocol:duplicate];
        assert!(conforms);
        let conforms: bool = msg_class![env; NSMutableString conformsToProtocol:duplicate];
        assert!(conforms);
        let conforms: bool = msg![env; instance conformsToProtocol:unrelated];
        assert!(!conforms);
        let conforms: bool = msg![env; instance conformsToProtocol:(ConstVoidPtr::null())];
        assert!(!conforms);
        let root = protocol(&mut env.mem, b"NSObject");
        let conforms: bool = msg![env; instance conformsToProtocol:root];
        assert!(conforms);
    }
}
