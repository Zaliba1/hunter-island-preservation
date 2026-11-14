/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSSet` class cluster, including `NSMutableSet` and `NSCountedSet`.

use super::ns_array;
use super::ns_dictionary::DictionaryHostObject;
use super::ns_enumerator::{fast_enumeration_helper, NSFastEnumerationState};
use super::NSUInteger;
use crate::abi::DotDotDot;
use crate::environment::Environment;
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, retain, ClassExports, HostObject, NSZonePtr,
};

/// Belongs to _touchHLE_NSSet
#[derive(Debug, Default)]
struct SetHostObject {
    dict: DictionaryHostObject,
}
impl HostObject for SetHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSSet is an abstract class. A subclass must provide:
// - (NSUInteger)count;
// - (id)member:(id)object;
// - (NSEnumerator*)objectEnumerator;
// We can pick whichever subclass we want for the various alloc methods.
// For the time being, that will always be _touchHLE_NSSet.
@implementation NSSet: NSObject

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSSet might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSSet", &mut env.mem));
    msg_class![env; _touchHLE_NSSet allocWithZone:zone]
}

+ (id)set {
    let set: id = msg![env; this new];
    autorelease(env, set)
}

+ (id)setWithArray:(id)array { // NSArray *
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithArray:array];
    autorelease(env, new)
}

+ (id)setWithObject:(id)object {
    assert!(object != nil);
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithObject:object];
    autorelease(env, new)
}

+ (id)setWithObjects:(id)first_obj, ...args {
    assert!(this == env.objc.get_known_class("NSSet", &mut env.mem));
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<SetHostObject>(new).dict = set_from_objects(env, first_obj, args);
    autorelease(env, new)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

- (id)member:(id)object {
    if object==nil { return nil; }
    let enumerator: id=msg![env; this objectEnumerator];
    loop {
        let stored:id=msg![env; enumerator nextObject];
        if stored==nil { return nil; }
        if msg![env; stored isEqual:object] { return stored; }
    }
}
- (bool)containsObject:(id)object { let stored:id=msg![env; this member:object]; stored!=nil }

- (bool)isEqualToSet:(id)other {
    if this == other { return true; }
    if other == nil { return false; }
    let count: NSUInteger = msg![env; this count];
    let other_count: NSUInteger = msg![env; other count];
    if count != other_count { return false; }
    // Use the abstract primitives so subclasses participate normally. In
    // particular, no collection borrow may survive an element's isEqual: call.
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let object: id = msg![env; enumerator nextObject];
        if object == nil { return true; }
        let member: id = msg![env; other member:object];
        if member == nil { return false; }
    }
}

- (bool)isEqual:(id)other {
    if this == other { return true; }
    let set_class = env.objc.get_known_class("NSSet", &mut env.mem);
    if !msg![env; other isKindOfClass:set_class] { return false; }
    msg![env; this isEqualToSet:other]
}

- (NSUInteger)hash {
    // Equal sets need equal hashes, including when used as collection keys.
    // Foundation uses the number of distinct members for this hash.
    msg![env; this count]
}

- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let objects:id=msg![env; this allObjects];
    let copy:id=msg_class![env; NSMutableSet alloc];
    msg![env; copy initWithArray:objects]
}

@end

// NSMutableSet is an abstract class. A subclass must provide everything
// NSSet provides, plus:
// - (void)addObject:(id)object;
// - (void)removeObject:(id)object;
// Note that it inherits from NSSet, so we must ensure we override any default
// methods that would be inappropriate for mutability.
@implementation NSMutableSet: NSSet

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSSet might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSMutableSet", &mut env.mem));
    msg_class![env; _touchHLE_NSMutableSet allocWithZone:zone]
}

+ (id)setWithObjects:(id)first_obj, ...args {
    assert!(this == env.objc.get_known_class("NSMutableSet", &mut env.mem));
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<SetHostObject>(new).dict = set_from_objects(env, first_obj, args);
    autorelease(env, new)
}

+ (id)setWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    let objects:id=msg![env; this allObjects];
    let copy:id=msg_class![env; NSSet alloc];
    msg![env; copy initWithArray:objects]
}

@end

// Our private subclass that is the single implementation of NSSet for the
// time being.
@implementation _touchHLE_NSSet: NSSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SetHostObject {
        dict: Default::default(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithObject:(id)object {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, object, null, /* copy_key: */ false);

    env.objc.borrow_mut::<SetHostObject>(this).dict = dict;

    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_objects(env, first_obj, args);
    this
}

- (id)initWithArray:(id)array {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_array(env, array);
    this
}

- (())dealloc {
    std::mem::take(&mut env.objc.borrow_mut::<SetHostObject>(this).dict).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: more init methods, etc

// TODO: accessors
- (NSUInteger)count {
    env.objc.borrow_mut::<SetHostObject>(this).dict.count
}

- (id)anyObject {
    let object_or_none = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().next();
    match object_or_none {
        Some(object) => object,
        None => nil
    }
}

- (id)allObjects {
    let objects:Vec<id> = env.objc.borrow::<SetHostObject>(this).dict.iter_keys().collect();
    for &object in &objects { retain(env,object); }
    let array=ns_array::from_vec(env,objects);
    autorelease(env,array)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // We assume that order in which objects are reported is consistent
    // between calls!
    let objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

@end

// Our private subclass that is the single implementation of NSMutableSet for
// the time being.
@implementation _touchHLE_NSMutableSet: NSMutableSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SetHostObject {
        dict: Default::default(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithObject:(id)object {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, object, null, /* copy_key: */ false);

    env.objc.borrow_mut::<SetHostObject>(this).dict = dict;

    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_objects(env, first_obj, args);
    this
}

- (id)initWithArray:(id)array {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_array(env, array);
    this
}

- (id)initWithCapacity:(NSUInteger)_capacity {
    // TODO: capacity
    msg![env; this init]
}

- (())dealloc {
    std::mem::take(&mut env.objc.borrow_mut::<SetHostObject>(this).dict).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: init methods etc

- (NSUInteger)count {
    env.objc.borrow_mut::<SetHostObject>(this).dict.count
}

- (id)anyObject {
    let object_or_none = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().next();
    match object_or_none {
        Some(object) => object,
        None => nil
    }
}

- (id)allObjects {
    let objects:Vec<id> = env.objc.borrow::<SetHostObject>(this).dict.iter_keys().collect();
    for &object in &objects { retain(env,object); }
    let array=ns_array::from_vec(env,objects);
    autorelease(env,array)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // TODO: check that set wasn't mutated!
    // We assume that order in which objects are reported is consistent
    // between calls!
    let objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

// TODO: more mutation methods

- (())addObject:(id)object {
    let null: id = msg_class![env; NSNull null];
    let mut host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.dict.insert(env, object, null, /* copy_key: */ false);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeObject:(id)object {
    let mut host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.dict.remove(env, object);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeAllObjects {
    let mut old_host_obj = std::mem::replace(
        env.objc.borrow_mut(this),
        SetHostObject {
            dict: Default::default(),
        },
    );
    old_host_obj.dict.release(env);
}

- (())unionSet:(id)other { // NSSet *
    let enumerator: id = msg![env; other objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
}

@end

};

/// Helper method shared between `initWithObjects:` of `_touchHLE_NSSet` and
/// `_touchHLE_NSMutableSet`
fn set_from_objects(env: &mut Environment, first_obj: id, args: DotDotDot) -> DictionaryHostObject {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, first_obj, null, /* copy_key: */ false);
    let mut varargs = args.start();
    loop {
        let next_arg: id = varargs.next(env);
        if next_arg == nil {
            break;
        }
        dict.insert(env, next_arg, null, /* copy_key: */ false);
    }
    dict
}

/// Helper method shared between `initWithArray:` of `_touchHLE_NSSet` and
/// `_touchHLE_NSMutableSet`
fn set_from_array(env: &mut Environment, array: id) -> DictionaryHostObject {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    let count: NSUInteger = msg![env; array count];
    for i in 0..count {
        let next: id = msg![env; array objectAtIndex:i];
        dict.insert(env, next, null, /* copy_key: */ false);
    }
    dict
}

/// Shared by the library test and the isolated invocation/framework probe.
pub(crate) fn equality_probe(env: &mut Environment) {
    use super::ns_string::from_rust_string;
    use crate::objc::release;
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let a = from_rust_string(env, "chest-a".into());
    let equal_a = from_rust_string(env, "chest-a".into());
    let b = from_rust_string(env, "chest-b".into());
    let c = from_rust_string(env, "chest-c".into());
    assert_ne!(a, equal_a);
    let left: id = msg_class![env; NSMutableSet new];
    let right: id = msg_class![env; NSMutableSet new];
    assert!(msg![env; left isEqualToSet:right]);
    assert!(msg![env; left isEqual:left]);
    assert!(!msg![env; left isEqual:nil]);
    assert!(!msg![env; left isEqualToSet:nil]);
    assert!(!msg![env; left isEqual:a]);
    for object in [a, b, a] {
        () = msg![env; left addObject:object];
    }
    assert!(!msg![env; left isEqualToSet:right]);
    for object in [b, equal_a] {
        () = msg![env; right addObject:object];
    }
    assert!(msg![env; left isEqualToSet:right]);
    assert!(msg![env; right isEqualToSet:left]);
    let immutable: id = msg![env; right copy];
    assert!(msg![env; left isEqual:immutable]);
    assert!(msg![env; immutable isEqual:left]);
    let left_hash: NSUInteger = msg![env; left hash];
    let right_hash: NSUInteger = msg![env; immutable hash];
    assert_eq!(left_hash, 2);
    assert_eq!(left_hash, right_hash);
    () = msg![env; right removeObject:b];
    () = msg![env; right addObject:c];
    assert!(!msg![env; left isEqualToSet:right]);
    assert!(msg![env; left isEqual:immutable]);

    // A nested set forces element isEqual: to reenter the set implementation;
    // dictionary lookup also checks the equality/hash contract together.
    let nested: id = msg_class![env; NSMutableSet new];
    () = msg![env; nested addObject:immutable];
    () = msg![env; nested addObject:left];
    let count: NSUInteger = msg![env; nested count];
    assert_eq!(count, 1);
    let dictionary: id = msg_class![env; NSMutableDictionary new];
    () = msg![env; dictionary setObject:c forKey:immutable];
    let found: id = msg![env; dictionary objectForKey:left];
    assert_eq!(found, c);
    let member: id = msg![env; nested member:left];
    assert_eq!(member, immutable);
    let count: NSUInteger = msg![env; left count];
    assert_eq!(count, 2);
    for object in [dictionary, nested, immutable, right, left, a, equal_a, b, c] {
        release(env, object);
    }
    release(env, pool);
    echo!("NSSET_VALUE_EQUALITY_ORDER_DUPLICATES_MUTABILITY_NESTING_AND_HASH_PASS");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objc::release;
    use std::cell::Cell;
    use std::rc::Rc;
    struct Lifetime(Rc<Cell<u32>>);
    impl HostObject for Lifetime {}
    impl Drop for Lifetime {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[test]
    fn set_equality_and_collection_key_contract() {
        let mut env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        equality_probe(&mut env);
    }
    #[test]
    fn canonical_member_copy_independence_and_snapshot_lifetime() {
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
        let first = super::super::ns_string::from_rust_string(env, "same".into());
        let query = super::super::ns_string::from_rust_string(env, "same".into());
        assert_ne!(first, query);
        let set: id = msg_class![env; NSMutableSet new];
        () = msg![env; set addObject:first];
        let found: id = msg![env; set member:query];
        assert_eq!(found, first);
        let absent: id = msg![env; set member:nil];
        assert_eq!(absent, nil);
        let copied: id = msg![env; set copy];
        let mutable: id = msg![env; copied mutableCopy];
        () = msg![env; set removeAllObjects];
        let found: id = msg![env; copied member:query];
        assert_eq!(found, first);
        () = msg![env; mutable removeObject:query];
        let count: u32 = msg![env; copied count];
        assert_eq!(count, 1);
        for object in [first, query, set, copied, mutable] {
            release(env, object);
        }
        release(env, pool);

        let pool: id = msg_class![env; NSAutoreleasePool new];
        let dropped = Rc::new(Cell::new(0));
        let class = env.objc.get_known_class("NSObject", &mut env.mem);
        let object =
            env.objc
                .alloc_object(class, Box::new(Lifetime(dropped.clone())), &mut env.mem);
        let set: id = msg_class![env; NSMutableSet new];
        () = msg![env; set addObject:object];
        () = msg![env; set addObject:object];
        let count: u32 = msg![env; set count];
        assert_eq!(count, 1);
        release(env, object);
        let snapshot: id = msg![env; set allObjects];
        retain(env, snapshot);
        release(env, set);
        assert_eq!(dropped.get(), 0);
        release(env, pool);
        assert_eq!(dropped.get(), 0);
        let kept: id = msg![env; snapshot objectAtIndex:0u32];
        assert_eq!(kept, object);
        release(env, snapshot);
        assert_eq!(dropped.get(), 1);
    }
}
