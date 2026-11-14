/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Operations using the guest's cooperative scheduler, never host guest-code threads.
mod probe;
pub mod queue;
use super::{ns_array, NSInteger, NSUInteger};
use crate::libc::{blocks, dispatch};
use crate::mem::MutVoidPtr;
use crate::objc::{
    autorelease, id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes,
    release, retain, ClassExports, HostObject, NSZonePtr,
};
use crate::Environment;
pub use probe::framework_probe;
pub(crate) use probe::CLASSES as PROBE_CLASSES;
pub use queue::State;
use std::collections::HashSet;
use std::time::Duration;

#[derive(Default)]
struct Operation {
    executing: bool,
    finished: bool,
    cancelled: bool,
    started: bool,
    executing_thread: Option<crate::ThreadId>,
    enqueued: bool,
    completion_done: bool,
    completion: MutVoidPtr,
    dependencies: Vec<id>,
    priority: NSInteger,
    queue: id, // weak; the queue's workers retain their queue
}
impl HostObject for Operation {}
#[derive(Default)]
struct BlockOperation {
    superclass: Operation,
    blocks: Vec<MutVoidPtr>,
}
impl_HostObject_with_superclass!(BlockOperation);

fn ready(env: &mut Environment, operation: id) -> bool {
    let state = env.objc.borrow::<Operation>(operation);
    if state.executing || state.finished {
        return false;
    }
    let cancelled: bool = msg![env; operation isCancelled];
    if cancelled {
        return true;
    }
    let dependencies = env.objc.borrow::<Operation>(operation).dependencies.clone();
    dependencies
        .into_iter()
        .all(|dep| msg![env; dep isFinished])
}
fn depends_on(env: &Environment, operation: id, other: id) -> bool {
    let mut pending = vec![operation];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if current == other {
            return true;
        }
        if visited.insert(current) {
            pending.extend(
                env.objc
                    .borrow::<Operation>(current)
                    .dependencies
                    .iter()
                    .copied(),
            );
        }
    }
    false
}
fn snapshot(env: &mut Environment, objects: Vec<id>) -> id {
    for &object in &objects {
        retain(env, object);
    }
    let array = ns_array::from_vec(env, objects);
    autorelease(env, array)
}
fn completion(env: &mut Environment, operation: id) {
    let state = env.objc.borrow_mut::<Operation>(operation);
    if state.completion_done {
        return;
    }
    state.completion_done = true;
    let block = std::mem::take(&mut state.completion);
    if !block.is_null() {
        blocks::invoke(env, block);
        blocks::_Block_release(env, block);
    }
}
fn wait_finished(env: &mut Environment, operation: id) {
    let finished: bool = msg![env; operation isFinished];
    if finished {
        return;
    }
    assert_ne!(
        env.objc.borrow::<Operation>(operation).executing_thread,
        Some(env.current_thread),
        "Operation cannot wait from its own main method"
    );
    assert_ne!(
        queue::current_operation(env),
        operation,
        "An operation cannot wait for itself"
    );
    let q = env.objc.borrow::<Operation>(operation).queue;
    assert!(
        finished || env.current_thread != 0 || q == nil || !queue::is_main(env, q),
        "Main thread cannot block waiting for its own operation queue"
    );
    while !msg![env; operation isFinished] {
        env.sleep(Duration::from_millis(1));
    }
}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation NSOperation: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::<Operation>::default(), &mut env.mem)
}
- (())dealloc {
    let state = env.objc.borrow_mut::<Operation>(this);
    assert!(!state.executing && state.queue == nil, "Operation destroyed while executing or queued");
    let completion = std::mem::take(&mut state.completion);
    let dependencies = std::mem::take(&mut state.dependencies);
    blocks::_Block_release(env, completion);
    for dependency in dependencies { release(env, dependency); }
    env.objc.dealloc_object(this, &mut env.mem);
}
- (bool)isCancelled { env.objc.borrow::<Operation>(this).cancelled }
- (bool)isExecuting { env.objc.borrow::<Operation>(this).executing }
- (bool)isFinished { env.objc.borrow::<Operation>(this).finished }
- (bool)isReady { ready(env, this) }
- (bool)isConcurrent { false }
- (bool)isAsynchronous { false }
- (())cancel { env.objc.borrow_mut::<Operation>(this).cancelled = true; }
- (())start {
    let state = env.objc.borrow::<Operation>(this);
    assert!(!state.started && !state.executing && !state.finished, "Operation cannot be restarted");
    assert!(ready(env, this), "Operation started before dependencies finished");
    let cancelled: bool = msg![env; this isCancelled];
    let state = env.objc.borrow_mut::<Operation>(this);
    state.started = true;
    state.executing = !cancelled;
    state.executing_thread = (!cancelled).then_some(env.current_thread);
    if !cancelled { () = msg![env; this main]; }
    let state = env.objc.borrow_mut::<Operation>(this);
    state.executing = false;
    state.executing_thread = None;
    state.finished = true;
    completion(env, this);
}
- (())main { /* The abstract base has no work of its own. */ }
- (())waitUntilFinished { wait_finished(env, this); }
- (())addDependency:(id)dependency {
    assert!(dependency != nil && !depends_on(env, dependency, this), "Operation dependency cycle or nil dependency");
    let state = env.objc.borrow_mut::<Operation>(this);
    assert!(!state.started && !state.finished, "Cannot change dependencies after starting");
    if !state.dependencies.contains(&dependency) {
        state.dependencies.push(dependency);
        retain(env, dependency);
    }
}
- (())removeDependency:(id)dependency {
    let state = env.objc.borrow_mut::<Operation>(this);
    assert!(!state.started && !state.finished, "Cannot change dependencies after starting");
    if let Some(index) = state.dependencies.iter().position(|&d| d == dependency) {
        state.dependencies.remove(index);
        release(env, dependency);
    }
}
- (id)dependencies { let dependencies = env.objc.borrow::<Operation>(this).dependencies.clone(); snapshot(env, dependencies) }
- (NSInteger)queuePriority { env.objc.borrow::<Operation>(this).priority }
- (())setQueuePriority:(NSInteger)priority { env.objc.borrow_mut::<Operation>(this).priority = priority.clamp(-8, 8); }
- (MutVoidPtr)completionBlock { env.objc.borrow::<Operation>(this).completion }
- (())setCompletionBlock:(MutVoidPtr)block {
    assert!(!env.objc.borrow::<Operation>(this).finished || block.is_null(), "Completion block installed after operation finished");
    let copied = blocks::_Block_copy(env, block);
    let old = std::mem::replace(&mut env.objc.borrow_mut::<Operation>(this).completion, copied);
    blocks::_Block_release(env, old);
}
@end
@implementation NSBlockOperation: NSOperation
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::<BlockOperation>::default(), &mut env.mem)
}
+ (id)blockOperationWithBlock:(MutVoidPtr)block {
    let operation: id = msg![env; this new];
    () = msg![env; operation addExecutionBlock:block];
    autorelease(env, operation)
}
- (())dealloc {
    let blocks = std::mem::take(&mut env.objc.borrow_mut::<BlockOperation>(this).blocks);
    for block in blocks { blocks::_Block_release(env, block); }
    () = msg_super![env; this dealloc];
}
- (())addExecutionBlock:(MutVoidPtr)block {
    assert!(!block.is_null());
    let state = env.objc.borrow::<Operation>(this);
    assert!(!state.started && !state.finished, "Execution block added after start");
    let block = blocks::_Block_copy(env, block);
    env.objc.borrow_mut::<BlockOperation>(this).blocks.push(block);
}
- (id)executionBlocks {
    let blocks = env.objc.borrow::<BlockOperation>(this).blocks.clone();
    let objects = blocks.into_iter().map(|block| blocks::_Block_copy(env, block).cast()).collect();
    let array = ns_array::from_vec(env, objects);
    autorelease(env, array)
}
- (())main {
    let work = env.objc.borrow::<BlockOperation>(this).blocks.clone();
    if work.len() == 1 { blocks::invoke(env, work[0]); }
    else if !work.is_empty() {
        let group = dispatch::dispatch_group_create(env);
        let global = dispatch::dispatch_get_global_queue(env, 0, 0);
        for block in work { dispatch::dispatch_group_async(env, group, global, block); }
        assert_eq!(dispatch::dispatch_group_wait(env, group, u64::MAX), 0);
        dispatch::dispatch_release(env, group);
    }
}
@end
};
