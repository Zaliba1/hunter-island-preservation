/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Immutable index paths, including UIKit's section/row convenience methods.
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::mem::ConstPtr;
use crate::objc::{autorelease, id, msg, nil, objc_classes, retain, ClassExports, HostObject, NSZonePtr};

#[derive(Default)]
struct IndexPath { indexes: Vec<NSUInteger> }
impl HostObject for IndexPath {}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation NSIndexPath: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::<IndexPath>::default(), &mut env.mem)
}
+ (id)indexPathWithIndex:(NSUInteger)index {
    let result = env.objc.alloc_object(this, Box::new(IndexPath { indexes: vec![index] }), &mut env.mem);
    autorelease(env,result)
}
+ (id)indexPathWithIndexes:(ConstPtr<NSUInteger>)indexes length:(NSUInteger)length {
    assert!(length <= 1024, "Index path exceeds supported length");
    let indexes = (0..length).map(|i| env.mem.read(indexes+i)).collect();
    let result = env.objc.alloc_object(this, Box::new(IndexPath { indexes }), &mut env.mem);
    autorelease(env,result)
}
+ (id)indexPathForRow:(NSInteger)row inSection:(NSInteger)section {
    assert!(row >= 0 && section >= 0);
    let result = env.objc.alloc_object(this, Box::new(IndexPath { indexes: vec![section as u32,row as u32] }), &mut env.mem);
    autorelease(env,result)
}
- (NSUInteger)length { env.objc.borrow::<IndexPath>(this).indexes.len() as NSUInteger }
- (NSUInteger)indexAtPosition:(NSUInteger)position {
    env.objc.borrow::<IndexPath>(this).indexes[position as usize]
}
- (NSInteger)section { env.objc.borrow::<IndexPath>(this).indexes[0] as NSInteger }
- (NSInteger)row { env.objc.borrow::<IndexPath>(this).indexes[1] as NSInteger }
- (id)copyWithZone:(NSZonePtr)_zone { retain(env,this) }
- (bool)isEqual:(id)other {
    if this==other {return true;}
    if other==nil {return false;}
    let this_class = env.objc.get_known_class("NSIndexPath",&mut env.mem);
    let compatible:bool=msg![env; other isKindOfClass:this_class];
    compatible && env.objc.borrow::<IndexPath>(this).indexes==env.objc.borrow::<IndexPath>(other).indexes
}
@end
};
