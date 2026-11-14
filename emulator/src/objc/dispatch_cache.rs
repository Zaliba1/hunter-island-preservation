/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Bounded positive method lookup cache. Class mutations invalidate all entries.
use super::{Class, HostIMP, IMP, SEL};
use crate::abi::GuestFunction;

const SLOTS: usize = 4096;

// IMP::clone intentionally only supports guest methods. Copy the static host
// reference explicitly instead of changing that public runtime behavior.
#[derive(Clone, Copy)]
enum CachedImp {
    Host(&'static dyn HostIMP),
    Guest(GuestFunction),
}
impl CachedImp {
    fn from_imp(imp: &IMP) -> Self {
        match imp {
            IMP::Host(host) => Self::Host(*host),
            IMP::Guest(guest) => Self::Guest(*guest),
        }
    }
    fn into_imp(self) -> IMP {
        match self {
            Self::Host(host) => IMP::Host(host),
            Self::Guest(guest) => IMP::Guest(guest),
        }
    }
}

#[derive(Clone, Copy)]
struct Entry {
    generation: u64,
    class: Class,
    selector: SEL,
    super_send: bool,
    imp: CachedImp,
}

pub(super) struct MethodCache {
    slots: Box<[Option<Entry>]>,
    generation: u64,
    pub hits: u64,
    pub misses: u64,
}
impl MethodCache {
    pub fn new() -> Self {
        Self {
            slots: vec![None; SLOTS].into_boxed_slice(),
            generation: 0,
            hits: 0,
            misses: 0,
        }
    }

    fn slot(class: Class, selector: SEL, super_send: bool) -> usize {
        // These are already interned identities. No attacker-controlled string
        // hash table grows: collisions replace one fixed slot and verify keys.
        let mixed = (class.to_bits() >> 2).wrapping_mul(0x9e37_79b1)
            ^ selector.to_bits().rotate_left(13).wrapping_mul(0x85eb_ca6b)
            ^ u32::from(super_send).wrapping_mul(0xc2b2_ae35);
        ((mixed ^ (mixed >> 16)) as usize) & (SLOTS - 1)
    }

    pub fn get(&mut self, class: Class, selector: SEL, super_send: bool) -> Option<IMP> {
        let entry = self.slots[Self::slot(class, selector, super_send)];
        if let Some(entry) = entry.filter(|entry| {
            entry.generation == self.generation
                && entry.class == class
                && entry.selector == selector
                && entry.super_send == super_send
        }) {
            self.hits = self.hits.wrapping_add(1);
            Some(entry.imp.into_imp())
        } else {
            self.misses = self.misses.wrapping_add(1);
            None
        }
    }

    pub fn insert(&mut self, class: Class, selector: SEL, super_send: bool, imp: &IMP) {
        self.slots[Self::slot(class, selector, super_send)] = Some(Entry {
            generation: self.generation,
            class,
            selector,
            super_send,
            imp: CachedImp::from_imp(imp),
        });
    }

    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.slots.fill(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{abi::GuestArg, mem::Ptr};
    fn selector(bits: u32) -> SEL {
        <SEL as GuestArg>::from_regs(&[bits])
    }
    fn guest(bits: u32) -> IMP {
        IMP::Guest(GuestFunction::from_addr_with_thumb_bit(bits))
    }
    fn address(imp: IMP) -> u32 {
        let IMP::Guest(imp) = imp else {
            panic!("Expected guest")
        };
        imp.to_ptr().to_bits()
    }

    #[test]
    fn fixed_cache_verifies_collisions_and_distinguishes_super_dispatch() {
        let mut cache = MethodCache::new();
        let class = Ptr::from_bits(0x1000);
        let sel = selector(0x2000);
        cache.insert(class, sel, false, &guest(0x3001));
        cache.insert(class, sel, true, &guest(0x4001));
        assert_eq!(address(cache.get(class, sel, false).unwrap()), 0x3001);
        assert_eq!(address(cache.get(class, sel, true).unwrap()), 0x4001);
        // Force a direct-map collision without ever allowing the wrong key.
        let collision = (0x2001..100_000)
            .map(selector)
            .find(|s| MethodCache::slot(class, *s, false) == MethodCache::slot(class, sel, false))
            .unwrap();
        cache.insert(class, collision, false, &guest(0x5001));
        assert!(cache.get(class, sel, false).is_none());
        assert_eq!(address(cache.get(class, collision, false).unwrap()), 0x5001);
        for bits in 1..100_000 {
            cache.insert(
                Ptr::from_bits(bits * 4),
                selector(bits),
                false,
                &guest(0x6001),
            );
        }
        assert_eq!(cache.slots.len(), SLOTS);
    }

    #[test]
    fn generation_invalidates_inherited_and_super_entries_including_wraparound() {
        let mut cache = MethodCache::new();
        let class = Ptr::from_bits(0x1000);
        let sel = selector(0x2000);
        for is_super in [false, true] {
            cache.insert(class, sel, is_super, &guest(0x3001));
        }
        cache.invalidate();
        for is_super in [false, true] {
            assert!(cache.get(class, sel, is_super).is_none());
        }
        cache.generation = u64::MAX;
        cache.insert(class, sel, false, &guest(0x4001));
        cache.invalidate();
        assert!(cache.slots.iter().all(Option::is_none));
    }
}
