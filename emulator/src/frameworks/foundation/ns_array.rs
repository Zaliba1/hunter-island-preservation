/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSArray` class cluster, including `NSMutableArray`.

use super::ns_enumerator::{fast_enumeration_helper, NSFastEnumerationState};
use super::ns_property_list_serialization::{
    deserialize_plist_from_file, NSPropertyListBinaryFormat_v1_0,
};
use super::{
    _nib_archive_decoder, ns_keyed_unarchiver, ns_string, ns_url, NSComparisonResult, NSNotFound,
    NSRange, NSUInteger,
};
use crate::abi::{CallFromHost, GuestFunction};
use crate::frameworks::foundation::ns_keyed_archiver::{
    encode_object, get_value_to_encode_for_current_key,
};
use crate::fs::GuestPath;
use crate::libc::stdlib::qsort::qsort_generic;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr, Ptr};
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, nil, objc_classes, release, retain, Class,
    ClassExports, HostObject, NSZonePtr, SEL,
};
use crate::Environment;

struct ObjectEnumeratorHostObject {
    /// the enumerated collection, NSArray *
    array: id,
    /// an iterator
    iterator: std::vec::IntoIter<id>,
}
impl HostObject for ObjectEnumeratorHostObject {}

/// Belongs to _touchHLE_NSArray
#[derive(Debug, Default)]
pub(super) struct ArrayHostObject {
    pub(super) array: Vec<id>,
}
impl HostObject for ArrayHostObject {}

fn checked_subarray_range(count: NSUInteger, range: NSRange) -> std::ops::Range<NSUInteger> {
    let end = range
        .location
        .checked_add(range.length)
        .expect("NSArray subarray range overflow");
    assert!(end <= count, "NSArray subarray range exceeds array count");
    range.location..end
}

fn init_from_object_buffer(
    env: &mut Environment,
    this: id,
    source: ConstPtr<id>,
    count: NSUInteger,
) -> id {
    let mut objects = Vec::with_capacity(count as usize);
    for index in 0..count {
        let object = env.mem.read(source + index);
        assert_ne!(object, nil);
        objects.push(retain(env, object));
    }
    let previous = std::mem::replace(
        &mut env.objc.borrow_mut::<ArrayHostObject>(this).array,
        objects,
    );
    for object in previous {
        release(env, object);
    }
    this
}

pub(crate) fn array_factory_probe(env: &mut Environment) {
    for name in ["NSArray", "NSMutableArray"] {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let class = env.objc.get_known_class(name, &mut env.mem);
        let mutable = name == "NSMutableArray";
        let first = ns_string::from_rust_string(env, "room-a".into());
        let second = ns_string::from_rust_string(env, "room-b".into());
        let singleton: id = msg![env; class arrayWithObject:first];
        let source = env.mem.alloc(8).cast::<id>();
        env.mem.write(source, first);
        env.mem.write(source + 1, second);
        let buffer: id = msg![env; class arrayWithObjects:(source.cast_const()) count:2u32];
        let empty: id = msg![env; class arrayWithObjects:(ConstPtr::<id>::null()) count:0u32];
        let selector = env
            .objc
            .register_host_selector("arrayWithObjects:".into(), &mut env.mem);
        let empty_variadic: id =
            crate::objc::msg_send_no_type_checking(env, (class, selector, nil));
        let mutable_class: Class = msg_class![env; NSMutableArray class];
        for array in [singleton, buffer, empty, empty_variadic] {
            let is_mutable: bool = msg![env; array isKindOfClass:mutable_class];
            assert_eq!(is_mutable, mutable);
            retain(env, array);
        }
        for (array, expected) in [
            (singleton, 1u32),
            (buffer, 2),
            (empty, 0),
            (empty_variadic, 0),
        ] {
            let count: NSUInteger = msg![env; array count];
            assert_eq!(count, expected);
        }
        if mutable {
            for array in [singleton, buffer, empty, empty_variadic] {
                () = msg![env; array addObject:second];
            }
        }
        release(env, first);
        release(env, second);
        release(env, pool);
        let kept: id = msg![env; singleton objectAtIndex:0u32];
        assert_eq!(ns_string::to_rust_string(env, kept), "room-a");
        if mutable {
            let kept: id = msg![env; empty objectAtIndex:0u32];
            assert_eq!(ns_string::to_rust_string(env, kept), "room-b");
        }
        for array in [singleton, buffer, empty, empty_variadic] {
            release(env, array);
        }
        env.mem.free(source.cast());
    }
    echo!("ARRAY_FACTORIES_MUTABILITY_EMPTY_VARIADIC_BUFFER_AND_OWNERSHIP_PASS");
}

#[cfg(test)]
mod factory_tests {
    #[test]
    fn array_factories_preserve_receiver_mutability_and_own_elements() {
        let env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.run_framework_probe(super::array_factory_probe);
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSArray is an abstract class. A subclass must provide:
// - (NSUInteger)count;
// - (id)objectAtIndex:(NSUInteger)index;
// We can pick whichever subclass we want for the various alloc methods.
// For the time being, that will always be _touchHLE_NSArray.
@implementation NSArray: NSObject

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSArray might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSArray", &mut env.mem));
    msg_class![env; _touchHLE_NSArray allocWithZone:zone]
}

+ (id)array {
    let array: id = msg![env; this new];
    autorelease(env, array)
}

+ (id)arrayWithArray:(id)other { // NSArray*
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithArray:other];
    autorelease(env, array)
}

// These probably comes from some category related to plists.
+ (id)arrayWithContentsOfFile:(id)path { // NSString*
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithContentsOfFile:path];
    autorelease(env, array)
}
+ (id)arrayWithContentsOfURL:(id)url { // NSURL*
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithContentsOfURL:url];
    autorelease(env, array)
}

+ (id)arrayWithObject:(id)object {
    assert_ne!(object, nil);
    let objects = env.mem.alloc_and_write(object);
    // Allocate through the receiver: NSMutableArray inherits this factory.
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithObjects:(objects.cast_const()) count:1u32];
    env.mem.free(objects.cast());
    autorelease(env, array)
}
+ (id)arrayWithObjects:(id)first_obj, ...args {
    let objects = if first_obj == nil {
        vec![]
    } else {
        retain(env, first_obj);
        let mut objects = vec![first_obj];
        let mut varargs = args.start();
        loop {
            let next_arg: id = varargs.next(env);
            if next_arg.is_null() {
                break;
            }
            retain(env, next_arg);
            objects.push(next_arg);
        }
        objects
    };
    let array = from_vec(env, objects);
    autorelease(env, array)
}
+ (id)arrayWithObjects:(ConstPtr<id>)objects_ptr count:(NSUInteger)count {
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithObjects:objects_ptr count:count];
    autorelease(env, array)
}

// These probably comes from some category related to plists.
- (id)initWithContentsOfFile:(id)path { // NSString*
    release(env, this);
    let path = ns_string::to_rust_string(env, path);
    deserialize_plist_from_file(
        env,
        GuestPath::new(&path),
        /* array_expected: */ true,
    )
}
- (id)initWithContentsOfURL:(id)url { // NSURL*
    release(env, this);
    let path = ns_url::to_rust_path(env, url);
    deserialize_plist_from_file(env, &path, /* array_expected: */ true)
}

- (bool)writeToFile:(id)path // NSString*
         atomically:(bool)atomically {
    let error_desc: MutPtr<id> = Ptr::null();
    let data: id = msg_class![env; NSPropertyListSerialization
            dataFromPropertyList:this
                          format:NSPropertyListBinaryFormat_v1_0
                errorDescription:error_desc];
    let res = msg![env; data writeToFile:path atomically:atomically];
    log_dbg!(
        "[(NSArray *){:?} writeToFile:{:?} atomically:{}] -> {}",
        this,
        ns_string::to_rust_string(env, path),
        atomically,
        res
    );
    res
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

// Return an independent immutable concatenation, preserving duplicates and
// retaining every occurrence. Use primitives so foreign subclasses work too.
- (id)arrayByAddingObjectsFromArray:(id)other {
    let mut objects = Vec::new();
    for source in [this, other] {
        let count: NSUInteger = msg![env; source count];
        let total = objects.len().checked_add(count as usize).expect("Array count overflow");
        assert!(total <= 16 * 1024 * 1024, "Array concatenation exceeds 64 MiB pointer budget");
        objects.reserve(count as usize);
        for i in 0..count {
            let object: id = msg![env; source objectAtIndex:i];
            assert_ne!(object, nil, "NSArray cannot contain nil");
            objects.push(retain(env, object));
        }
    }
    let result = from_vec(env, objects);
    autorelease(env, result)
}

- (id)subarrayWithRange:(NSRange)range {
    // Use primitives to support mutable receivers and foreign subclasses.
    // The returned immutable array independently owns every selected entry.
    let count: NSUInteger = msg![env; this count];
    let selected = checked_subarray_range(count, range);
    let mut objects = Vec::with_capacity((selected.end - selected.start) as usize);
    for index in selected {
        let object: id = msg![env; this objectAtIndex:index];
        objects.push(retain(env, object));
    }
    let result = from_vec(env, objects);
    autorelease(env, result)
}

- (NSUInteger)indexOfObject:(id)object {
    let count: NSUInteger = msg![env; this count];
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let equal: bool = msg![env; object isEqual:curr_object];
        if equal {
            return i;
        }
    }
    NSNotFound as NSUInteger
}
- (bool)containsObject:(id)object {
    let idx: NSUInteger = msg![env; this indexOfObject:object];
    idx != NSNotFound as NSUInteger
}

- (id)firstObject {
    let size: NSUInteger = msg![env; this count];
    if size == 0 {
        return nil;
    }
    msg![env; this objectAtIndex:0u32]
}

- (id)lastObject {
    let size: NSUInteger = msg![env; this count];
    if size == 0 {
        return nil;
    }
    msg![env; this objectAtIndex:(size - 1)]
}

- (id)componentsJoinedByString:(id)str { // NSString *
    let res: id = msg_class![env; NSMutableString new];
    let count: NSUInteger = msg![env; this count];
    if count == 0 {
        autorelease(env, res);
        return res;
    }
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let curr_desc: id = msg![env; curr_object description];
        () = msg![env; res appendString:curr_desc];
        if i != count-1 {
            () = msg![env; res appendString:str];
        }
    }
    let res_imm = msg![env; res copy];
    release(env, res);
    autorelease(env, res_imm)
}

- (id)sortedArrayUsingFunction:(GuestFunction)comparator
                       context:(MutVoidPtr)context {
    let array = msg![env; this mutableCopy];
    () = msg![env; array sortUsingFunction:comparator context:context];
    let array_imm = msg![env; array copy];
    release(env, array);
    autorelease(env, array_imm)
}

- (id)sortedArrayUsingSelector:(SEL)comparator {
    let count: NSUInteger = msg![env; this count];
    let mut objects = Vec::with_capacity(count as usize);
    for index in 0..count {
        let object: id = msg![env; this objectAtIndex:index];
        objects.push(retain(env, object));
    }
    // The retained snapshot survives comparator callbacks without borrowing
    // or mutating the receiver. from_vec takes ownership of these references.
    objects.sort_by(|a,b| {
        let result: NSComparisonResult = msg_send(env, (*a, comparator, *b));
        result.cmp(&0)
    });
    let sorted = from_vec(env, objects);
    autorelease(env, sorted)
}

- (NSUInteger)hash {
    // TODO: define better hash
    msg![env; this count]
}
- (bool)isEqual:(id)other {
    if this == other {
        return true;
    }
    let class: Class = msg_class![env; NSArray class];
    if !msg![env; other isKindOfClass:class] {
        return false;
    }
    msg![env; this isEqualToArray:other]
}
- (bool)isEqualToArray:(id)other { // NSArray *
    if other == nil {
        return false;
    }
    let count: NSUInteger = msg![env; this count];
    let other_count: NSUInteger = msg![env; other count];
    if count != other_count {
        return false;
    }
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let curr_other_object: id = msg![env; other objectAtIndex:i];
        let equal: bool = msg![env; curr_object isEqual:curr_other_object];
        if !equal {
            return false;
        }
    }
    true
}

@end

// NSMutableArray is an abstract class. A subclass must provide everything
// NSArray provides, plus:
// - (void)insertObject:(id)object atIndex:(NSUInteger)index;
// - (void)removeObjectAtIndex:(NSUInteger)index;
// - (void)addObject:(id)object;
// - (void)removeLastObject
// - (void)replaceObjectAtIndex:(NSUInteger)index withObject:(id)object;
// Note that it inherits from NSArray, so we must ensure we override any default
// methods that would be inappropriate for mutability.
@implementation NSMutableArray: NSArray

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSArray might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSMutableArray", &mut env.mem));
    msg_class![env; _touchHLE_NSMutableArray allocWithZone:zone]
}

+ (id)arrayWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

+ (id)arrayWithArray:(id)array {
    let new: id = msg![env; this alloc];
    () = msg![env; new addObjectsFromArray:array];
    autorelease(env, new)
}

+ (id)arrayWithObjects:(id)firstObj, ...args {
    if firstObj == nil {
        let array: id = msg![env; this new];
        return autorelease(env, array);
    }
    retain(env, firstObj);
    let mut objects = vec![firstObj];
    let mut varargs = args.start();
    loop {
        let next_arg: id = varargs.next(env);
        if next_arg.is_null() {
            break;
        }
        retain(env, next_arg);
        objects.push(next_arg);
    }
    let array = mutable_from_vec(env, objects);
    autorelease(env, array)
}

// These probably comes from some category related to plists.
- (id)initWithContentsOfFile:(id)path { // NSString*
    release(env, this);
    let path = ns_string::to_rust_string(env, path);
    let tmp = deserialize_plist_from_file(
        env,
        GuestPath::new(&path),
        /* array_expected: */ true,
    );
    if tmp == nil {
        return nil;
    }
    // We should respect mutability of the top most container!
    let res = msg_class![env; NSMutableArray alloc];
    let res = msg![env; res initWithArray:tmp];
    release(env, tmp);
    res
}
- (id)initWithContentsOfURL:(id)url { // NSURL*
    release(env, this);
    let path = ns_url::to_rust_path(env, url);
    let tmp = deserialize_plist_from_file(env, &path, /* array_expected: */ true);
    if tmp == nil {
        return nil;
    }
    // We should respect mutability of the top most container!
    let res = msg_class![env; NSMutableArray alloc];
    let res = msg![env; res initWithArray:tmp];
    release(env, tmp);
    res
}

- (())addObjectsFromArray:(id)other { // NSArray*
    let enumerator: id = msg![env; other objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
}

- (())removeObjectsInArray:(id)other { // NSArray*
    // A stable, owning snapshot also handles [array removeObjectsInArray:array].
    let snapshot: id = msg![env; other copy];
    let count: NSUInteger = msg![env; this count];
    for index in (0..count).rev() {
        let object: id = msg![env; this objectAtIndex:index];
        let remove: bool = msg![env; snapshot containsObject:object];
        if remove {
            () = msg![env; this removeObjectAtIndex:index];
        }
    }
    release(env, snapshot);
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    let other: id = msg_class![env; NSArray alloc];
    let other: id = msg![env; other initWithArray:this];
    other
}

@end

// Our private subclass that is the single implementation of NSArray for the
// time being.
@implementation _touchHLE_NSArray: NSArray

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(ArrayHostObject {
        array: Vec::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    init_with_coder_inner(env, this, coder)
}
- (())encodeWithCoder:(id)coder {
    encode_with_coder_inner(env, this, coder)
}

- (id)initWithArray:(id)array { // NSArray*
    let mut objects = Vec::new();
    let enumerator: id = msg![env; array objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        objects.push(next);
        retain(env, next);
    }
    env.objc.borrow_mut::<ArrayHostObject>(this).array = objects;
    this
}

- (id)initWithObjects:(id)firstObj, ...args {
    retain(env, firstObj);
    let mut objects = vec![firstObj];
    let mut varargs = args.start();
    loop {
        let next_arg: id = varargs.next(env);
        if next_arg.is_null() {
            break;
        }
        retain(env, next_arg);
        objects.push(next_arg);
    }
    env.objc.borrow_mut::<ArrayHostObject>(this).array = objects;
    this
}

- (id)initWithObjects:(ConstPtr<id>)objects_ptr count:(NSUInteger)count {
    init_from_object_buffer(env, this, objects_ptr, count)
}

- (())dealloc {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let array = std::mem::take(&mut host_object.array);

    for object in array {
        release(env, object);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    mutable_copy_inner(env, this)
}

- (id)objectEnumerator { // NSEnumerator*
    object_enumerator_inner(env, this)
}
- (id)reverseObjectEnumerator { // NSEnumerator*
    reverse_object_enumerator_inner(env, this)
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    let count: NSUInteger = msg![env; this count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; this objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

// TODO: more init methods, etc

- (NSUInteger)count {
    env.objc.borrow::<ArrayHostObject>(this).array.len().try_into().unwrap()
}
- (id)objectAtIndex:(NSUInteger)index {
    // TODO: throw real exception rather than panic if out-of-bounds?
    env.objc.borrow::<ArrayHostObject>(this).array[index as usize]
}

- (id)description {
    build_description(env, this)
}


@end

// Special variant for use by CFArray with NULL callbacks: objects aren't
// necessarily Objective-C objects and won't be retained/released.
@implementation _touchHLE_NSArray_non_retaining: _touchHLE_NSArray

- (())dealloc {
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

@implementation _touchHLE_NSArray_ObjectEnumerator: NSEnumerator

- (id)nextObject {
    let host_obj = env.objc.borrow_mut::<ObjectEnumeratorHostObject>(this);
    host_obj.iterator.next().unwrap_or(nil)
}

- (())dealloc {
    let host_obj = env.objc.borrow::<ObjectEnumeratorHostObject>(this);
    release(env, host_obj.array);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

// Our private subclass that is the single implementation of NSMutableArray for
// the time being.
@implementation _touchHLE_NSMutableArray: NSMutableArray

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(ArrayHostObject {
        array: Vec::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCapacity:(NSUInteger)capacity {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.reserve(capacity as usize);
    this
}

- (id)initWithObjects:(ConstPtr<id>)objects_ptr count:(NSUInteger)count {
    init_from_object_buffer(env, this, objects_ptr, count)
}

- (id)initWithArray:(id)array { // NSArray*
    let mut objects = Vec::new();
    let enumerator: id = msg![env; array objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        objects.push(next);
        retain(env, next);
    }
    env.objc.borrow_mut::<ArrayHostObject>(this).array = objects;
    this
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    init_with_coder_inner(env, this, coder)
}
- (())encodeWithCoder:(id)coder {
    encode_with_coder_inner(env, this, coder)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    let arr: id = msg_class![env; NSArray alloc];
    let array = env.objc.borrow::<ArrayHostObject>(this).array.clone();
    for &object in &array {
        retain(env, object);
    }
    env.objc.borrow_mut::<ArrayHostObject>(arr).array = array;
    arr
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    mutable_copy_inner(env, this)
}

- (())dealloc {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let array = std::mem::take(&mut host_object.array);

    for object in array {
        release(env, object);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

- (())makeObjectsPerformSelector:(SEL)sel {
    let count: NSUInteger = msg![env; this count];
    for idx in 0..count {
        let obj: id = msg![env; this objectAtIndex:idx];
        let _: id = msg![env; obj performSelector:sel];
    }
}

- (id)objectEnumerator { // NSEnumerator*
    object_enumerator_inner(env, this)
}
- (id)reverseObjectEnumerator { // NSEnumerator*
    reverse_object_enumerator_inner(env, this)
}

- (())sortUsingFunction:(GuestFunction)comparator
                context:(MutVoidPtr)context {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let mut array = std::mem::take(&mut host_object.array);
    let len = array.len().try_into().unwrap();
    let mut user_data = (env, &mut array);
    qsort_generic(
        &mut user_data,
        len,
        &mut |(env, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            comparator.call_from_host(env, (array[l], array[r], context))
        },
        &mut |(_, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            array.swap(l, r);
        },
    );
    let (env, _) = user_data;
    env.objc.borrow_mut::<ArrayHostObject>(this).array = array;
}

- (())sortUsingSelector:(SEL)comparator {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let mut array = std::mem::take(&mut host_object.array);
    let len = array.len().try_into().unwrap();
    let mut user_data = (env, &mut array);
    qsort_generic(
        &mut user_data,
        len,
        &mut |(env, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            let res: NSComparisonResult = msg_send(env, (array[l], comparator, array[r]));
            res
        },
        &mut |(_, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            array.swap(l, r);
        },
    );

    let (env, _) = user_data;
    env.objc.borrow_mut::<ArrayHostObject>(this).array = array;
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // TODO: check that array wasn't mutated!
    let count: NSUInteger = msg![env; this count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; this objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

- (NSUInteger)count {
    env.objc.borrow::<ArrayHostObject>(this).array.len().try_into().unwrap()
}
- (id)objectAtIndex:(NSUInteger)index {
    // TODO: throw real exception rather than panic if out-of-bounds?
    env.objc.borrow::<ArrayHostObject>(this).array[index as usize]
}

- (id)description {
    build_description(env, this)
}

// TODO: more mutation methods

- (())insertObject:(id)object
           atIndex:(NSUInteger)index {
    retain(env, object);
    env.objc.borrow_mut::<ArrayHostObject>(this).array.insert(index as usize, object);
}

- (())addObject:(id)object {
    retain(env, object);
    env.objc.borrow_mut::<ArrayHostObject>(this).array.push(object);
}

- (())removeObject:(id)object {
    let mut to_remove = Vec::new();
    let count: NSUInteger = msg![env; this count];
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let equal: bool = msg![env; object isEqual:curr_object];
        if equal {
            to_remove.push(i);
        }
    }
    // TODO: runtime here is O(n^2), it could be O(n) instead
    for i in to_remove.into_iter().rev() {
        () = msg![env; this removeObjectAtIndex:i];
    }
}

- (())removeObjectAtIndex:(NSUInteger)index {
    let object = env.objc.borrow_mut::<ArrayHostObject>(this).array.remove(index as usize);
    release(env, object)
}

- (())replaceObjectAtIndex:(NSUInteger)index withObject:(id)obj {
    retain(env, obj);
    let object = std::mem::replace(&mut env.objc.borrow_mut::<ArrayHostObject>(this).array[index as usize], obj);
    release(env, object);
}

- (())removeLastObject {
    let object = env.objc.borrow_mut::<ArrayHostObject>(this).array.pop().unwrap();
    release(env, object)
}

- (())removeAllObjects {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let array = std::mem::take(&mut host_object.array);
    for object in array {
        release(env, object);
    }

    env.objc.borrow_mut::<ArrayHostObject>(this).array = Vec::new()
}

@end

// Special variant for use by CFArray with NULL callbacks: objects aren't
// necessarily Objective-C objects and won't be retained/released.
@implementation _touchHLE_NSMutableArray_non_retaining: _touchHLE_NSMutableArray

- (())dealloc {
    env.objc.dealloc_object(this, &mut env.mem)
}

- (())addObject:(id)object {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.push(object);
}

- (())removeObjectAtIndex:(NSUInteger)index {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.remove(index as usize);
}

- (())removeLastObject {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.pop().unwrap();
}

@end

};

/// Shortcut for host code, roughly equivalent to
/// `[[NSArray alloc] initWithObjects:count]` but without copying.
/// The elements should already be "retained by" the `Vec`.
pub fn from_vec(env: &mut Environment, objects: Vec<id>) -> id {
    let array: id = msg_class![env; NSArray alloc];
    env.objc.borrow_mut::<ArrayHostObject>(array).array = objects;
    array
}

/// Shortcut for host code, roughly equivalent to
/// `[[NSMutableArray alloc] initWithObjects:count]` but without copying.
/// The elements should already be "retained by" the `Vec`.
pub fn mutable_from_vec(env: &mut Environment, objects: Vec<id>) -> id {
    let array: id = msg_class![env; NSMutableArray alloc];
    env.objc.borrow_mut::<ArrayHostObject>(array).array = objects;
    array
}

/// A helper to build a description NSString
/// for a NSArray or a NSMutableArray.
fn build_description(env: &mut Environment, arr: id) -> id {
    // According to docs, this description should be formatted as property list.
    // But by the same docs, it's meant to be used for debugging purposes only.
    let desc: id = msg_class![env; NSMutableString new];
    let prefix: id = ns_string::from_rust_string(env, "(\n".to_string());
    () = msg![env; desc appendString:prefix];
    release(env, prefix);
    let values: Vec<id> = env.objc.borrow_mut::<ArrayHostObject>(arr).array.clone();
    for value in values {
        let value_desc: id = msg![env; value description];
        // TODO: respect nesting and padding
        let format = format!("\t{},\n", ns_string::to_rust_string(env, value_desc));
        let format = ns_string::from_rust_string(env, format);
        () = msg![env; desc appendString:format];
        release(env, format);
    }
    let suffix: id = ns_string::from_rust_string(env, ")".to_string());
    () = msg![env; desc appendString:suffix];
    release(env, suffix);
    let desc_imm = msg![env; desc copy];
    release(env, desc);
    autorelease(env, desc_imm)
}

/// A shared objectEnumerator helper method.
fn object_enumerator_inner(env: &mut Environment, arr: id) -> id {
    let array_host_object: &mut ArrayHostObject = env.objc.borrow_mut(arr);
    let vec = array_host_object.array.to_vec();
    object_enumerator_inner_helper(env, arr, vec)
}

/// A shared reverseObjectEnumerator helper method.
fn reverse_object_enumerator_inner(env: &mut Environment, arr: id) -> id {
    let array_host_object: &mut ArrayHostObject = env.objc.borrow_mut(arr);
    // TODO: avoid copying?
    let vec = array_host_object
        .array
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    object_enumerator_inner_helper(env, arr, vec)
}

fn object_enumerator_inner_helper(env: &mut Environment, arr: id, vec: Vec<id>) -> id {
    let host_object = Box::new(ObjectEnumeratorHostObject {
        array: arr,
        iterator: vec.into_iter(),
    });
    retain(env, arr);
    let class = env
        .objc
        .get_known_class("_touchHLE_NSArray_ObjectEnumerator", &mut env.mem);
    let enumerator = env.objc.alloc_object(class, host_object, &mut env.mem);
    autorelease(env, enumerator)
}

fn mutable_copy_inner(env: &mut Environment, arr: id) -> id {
    let mut_arr: id = msg_class![env; NSMutableArray alloc];
    let array = env.objc.borrow::<ArrayHostObject>(arr).array.clone();
    for &object in &array {
        retain(env, object);
    }
    env.objc.borrow_mut::<ArrayHostObject>(mut_arr).array = array;
    mut_arr
}

fn init_with_coder_inner(env: &mut Environment, arr: id, coder: id) -> id {
    let class: Class = msg![env; coder class];
    let keyed_unarch_class: Class = msg_class![env; NSKeyedUnarchiver class];
    let nib_archive_class: Class = msg_class![env; _touchHLE_NIBArchiveDecoder class];
    // It seems that every NSArray item in an NSKeyedArchiver plist looks like:
    // {
    //   "$class" => (uid of NSArray class goes here),
    //   "NS.objects" => [
    //     // objects here
    //   ]
    // }
    // Presumably we need to call a `decodeFooBarForKey:` method on the NSCoder
    // here, passing in an NSString for "NS.objects". There is no method for
    // arrays though (maybe it's `decodeObjectForKey:`), and in any case
    // allocating an NSString here would be inconvenient, so let's just take a
    // shortcut.
    let objects = if env.objc.class_is_subclass_of(class, keyed_unarch_class) {
        ns_keyed_unarchiver::decode_current_array(env, coder)
    } else if env.objc.class_is_subclass_of(class, nib_archive_class) {
        _nib_archive_decoder::decode_current_array(env, coder)
    } else {
        unimplemented!()
    };

    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(arr);
    assert!(host_object.array.is_empty());
    host_object.array = objects; // objects are already retained
    arr
}

fn encode_with_coder_inner(env: &mut Environment, arr: id, coder: id) {
    let host_obj: ArrayHostObject = std::mem::take(env.objc.borrow_mut(arr));
    let mut encoded_vals = vec![];
    for v in &host_obj.array {
        // TODO: support other type of coders, not only NSKeyedArchiver
        let vv = encode_object(env, coder, *v);
        encoded_vals.push(plist::Value::Uid(vv));
    }
    *env.objc.borrow_mut(arr) = host_obj;

    let scope = get_value_to_encode_for_current_key(env, coder);
    scope.insert("NS.objects".to_string(), plist::Value::Array(encoded_vals));
}

#[cfg(test)]
mod concatenation_tests {
    use super::*;
    #[test]
    fn concatenation_preserves_order_duplicates_and_lifetime_independently() {
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
        let a = ns_string::from_rust_string(env, "a".into());
        let b = ns_string::from_rust_string(env, "b".into());
        let first: id = msg_class![env; NSMutableArray new];
        () = msg![env; first addObject:a];
        () = msg![env; first addObject:b];
        let second: id = msg_class![env; NSArray arrayWithObject:a];
        let result: id = msg![env; first arrayByAddingObjectsFromArray:second];
        let self_copy: id = msg![env; first arrayByAddingObjectsFromArray:first];
        retain(env, result);
        retain(env, self_copy);
        let absent: id = msg![env; first arrayByAddingObjectsFromArray:nil];
        let count: u32 = msg![env; absent count];
        assert_eq!(count, 2);
        () = msg![env; first removeAllObjects];
        for object in [a, b, first, pool] {
            release(env, object);
        }
        let count: u32 = msg![env; result count];
        assert_eq!(count, 3);
        let mutable_class = env.objc.get_known_class("NSMutableArray", &mut env.mem);
        let mutable: bool = msg![env; result isKindOfClass:mutable_class];
        assert!(!mutable);
        for (array, values) in [
            (result, vec!["a", "b", "a"]),
            (self_copy, vec!["a", "b", "a", "b"]),
        ] {
            for (i, expected) in values.into_iter().enumerate() {
                let object: id = msg![env; array objectAtIndex:(i as u32)];
                assert_eq!(ns_string::to_rust_string(env, object), expected);
            }
            release(env, array);
        }
    }
}

pub(crate) fn selector_sort_probe(env: &mut Environment) {
    bulk_removal_probe(env);
    subarray_probe(env);
    let comparator = env
        .objc
        .register_host_selector("compare:".into(), &mut env.mem);
    for mutable in [false, true] {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let a = ns_string::from_rust_string(env, "a".into());
        let b = ns_string::from_rust_string(env, "b".into());
        let mut objects = vec![b, a, a];
        for object in &mut objects {
            retain(env, *object);
        }
        let source = if mutable {
            mutable_from_vec(env, objects)
        } else {
            from_vec(env, objects)
        };
        let sorted: id = msg![env; source sortedArrayUsingSelector:comparator];
        retain(env, sorted);
        let first: id = msg![env; source objectAtIndex:0u32];
        assert_eq!(first, b);
        let mutable_class = env.objc.get_known_class("NSMutableArray", &mut env.mem);
        let result_is_mutable: bool = msg![env; sorted isKindOfClass:mutable_class];
        assert!(!result_is_mutable);
        let empty: id = msg_class![env; NSArray array];
        let empty: id = msg![env; empty sortedArrayUsingSelector:comparator];
        let count: NSUInteger = msg![env; empty count];
        assert_eq!(count, 0);
        for object in [source, a, b, pool] {
            release(env, object);
        }
        for (index, expected) in ["a", "a", "b"].into_iter().enumerate() {
            let value: id = msg![env; sorted objectAtIndex:(index as u32)];
            assert_eq!(ns_string::to_rust_string(env, value), expected);
        }
        release(env, sorted);
    }
    echo!("ARRAY_SELECTOR_SORT_MUTABLE_IMMUTABLE_SOURCE_AND_LIFETIME_PASS");
}

fn subarray_probe(env: &mut Environment) {
    for mutable in [false, true] {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let a = ns_string::from_rust_string(env, "a".into());
        let b = ns_string::from_rust_string(env, "b".into());
        let objects = [b, a, a, b].into_iter().map(|o| retain(env, o)).collect();
        let source = if mutable {
            mutable_from_vec(env, objects)
        } else {
            from_vec(env, objects)
        };
        let slice: id = msg![env; source subarrayWithRange:(NSRange { location:1,length:2 })];
        let full: id = msg![env; source subarrayWithRange:(NSRange { location:0,length:4 })];
        retain(env, slice);
        retain(env, full);
        let count: NSUInteger = msg![env; source count];
        assert_eq!(count, 4);
        for location in [0u32, 4] {
            let empty: id = msg![env; source subarrayWithRange:(NSRange { location,length:0 })];
            let count: NSUInteger = msg![env; empty count];
            assert_eq!(count, 0);
        }
        let mutable_class = env.objc.get_known_class("NSMutableArray", &mut env.mem);
        let is_mutable: bool = msg![env; slice isKindOfClass:mutable_class];
        assert!(!is_mutable);
        if mutable {
            () = msg![env; source removeAllObjects];
        }
        for object in [a, b, source, pool] {
            release(env, object);
        }
        for (array, expected) in [(slice, vec!["a", "a"]), (full, vec!["b", "a", "a", "b"])] {
            let count: NSUInteger = msg![env; array count];
            assert_eq!(count, expected.len() as u32);
            for (i, expected) in expected.into_iter().enumerate() {
                let object: id = msg![env; array objectAtIndex:(i as u32)];
                assert_eq!(ns_string::to_rust_string(env, object), expected);
            }
            release(env, array);
        }
    }
    echo!("ARRAY_SUBARRAY_MUTABLE_IMMUTABLE_EMPTY_DUPLICATES_AND_LIFETIME_PASS");
}

fn bulk_removal_probe(env: &mut Environment) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let a = ns_string::from_rust_string(env, "a".into());
    let equal_a = ns_string::from_rust_string(env, "a".into());
    let b = ns_string::from_rust_string(env, "b".into());
    let c = ns_string::from_rust_string(env, "c".into());
    assert_ne!(a, equal_a);
    let objects = [a, b, a, c, b, a]
        .into_iter()
        .map(|o| retain(env, o))
        .collect();
    let receiver = mutable_from_vec(env, objects);
    let objects = [equal_a, equal_a]
        .into_iter()
        .map(|o| retain(env, o))
        .collect();
    let removals = mutable_from_vec(env, objects);
    () = msg![env; receiver removeObjectsInArray:removals];
    let count: NSUInteger = msg![env; removals count];
    assert_eq!(count, 2); // The argument remains unchanged.
    release(env, removals);
    for (index, expected) in [b, c, b].into_iter().enumerate() {
        let object: id = msg![env; receiver objectAtIndex:(index as u32)];
        assert_eq!(object, expected);
    }
    let count: NSUInteger = msg![env; receiver count];
    assert_eq!(count, 3);
    let empty: id = msg_class![env; NSArray array];
    () = msg![env; receiver removeObjectsInArray:empty];
    () = msg![env; receiver removeObjectsInArray:nil];
    let absent = ns_string::from_rust_string(env, "missing".into());
    let missing: id = msg_class![env; NSArray arrayWithObject:absent];
    () = msg![env; receiver removeObjectsInArray:missing];
    release(env, absent);
    let count: NSUInteger = msg![env; receiver count];
    assert_eq!(count, 3);
    // removeObject: must also remove duplicate positions without index shifts.
    () = msg![env; receiver removeObject:b];
    let count: NSUInteger = msg![env; receiver count];
    assert_eq!(count, 1);
    let object: id = msg![env; receiver objectAtIndex:0u32];
    assert_eq!(object, c);
    let objects = [a, b, a].into_iter().map(|o| retain(env, o)).collect();
    let self_removal = mutable_from_vec(env, objects);
    () = msg![env; self_removal removeObjectsInArray:self_removal];
    let count: NSUInteger = msg![env; self_removal count];
    assert_eq!(count, 0);
    () = msg![env; self_removal removeObjectsInArray:self_removal];
    for object in [a, equal_a, b, c, self_removal, pool] {
        release(env, object);
    }
    // Only the surviving receiver retains this object now.
    let object: id = msg![env; receiver objectAtIndex:0u32];
    assert_eq!(ns_string::to_rust_string(env, object), "c");
    release(env, receiver);
    echo!("ARRAY_BULK_REMOVAL_EQUALITY_DUPLICATES_ALIAS_AND_LIFETIME_PASS");
}

#[cfg(test)]
mod selector_sort_tests {
    #[test]
    fn subarray_range_bounds_reject_overflow_and_out_of_bounds() {
        use super::{checked_subarray_range, NSRange};
        assert_eq!(
            checked_subarray_range(
                4,
                NSRange {
                    location: 1,
                    length: 2
                }
            ),
            1..3
        );
        assert_eq!(
            checked_subarray_range(
                4,
                NSRange {
                    location: 4,
                    length: 0
                }
            ),
            4..4
        );
        assert_eq!(
            checked_subarray_range(
                0,
                NSRange {
                    location: 0,
                    length: 0
                }
            ),
            0..0
        );
        for (location, length) in [(5, 0), (4, 1), (u32::MAX, 1), (2, u32::MAX)] {
            assert!(std::panic::catch_unwind(|| checked_subarray_range(
                4,
                NSRange { location, length }
            ))
            .is_err());
        }
    }
    #[test]
    fn subarray_handles_both_receiver_classes_and_preserves_owned_snapshot() {
        let mut env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        super::subarray_probe(&mut env);
    }
    #[test]
    fn bulk_removal_preserves_survivors_and_handles_equal_duplicates_and_self_alias() {
        let mut env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        super::bulk_removal_probe(&mut env);
    }
    #[test]
    fn selector_sort_accepts_both_receivers_and_preserves_source_and_ownership() {
        let mut env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        super::selector_sort_probe(&mut env);
    }
}
