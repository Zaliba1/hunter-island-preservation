/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use super::*;
use crate::abi::GuestFunction;
use crate::dyld::export_c_func;
use crate::mem::Mem;
use crate::ThreadId;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    main: Option<id>,
    coordinator: Option<GuestFunction>,
    worker: Option<GuestFunction>,
    jobs: HashMap<MutVoidPtr, (id, id, bool)>,
    current: HashMap<ThreadId, Vec<(id, id)>>,
}
struct Queue {
    pending: Vec<id>,
    running: Vec<id>,
    suspended: bool,
    main: bool,
    max: NSInteger,
    coordinator_running: bool,
    name: id,
}
impl Default for Queue {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            running: Vec::new(),
            suspended: false,
            main: false,
            max: -1,
            coordinator_running: false,
            name: nil,
        }
    }
}
impl HostObject for Queue {}
fn state(env: &mut Environment) -> &mut State {
    &mut env.framework_state.foundation.ns_operation
}
pub(super) fn is_main(env: &Environment, queue: id) -> bool {
    env.objc.borrow::<Queue>(queue).main
}
pub(super) fn current_operation(env: &mut Environment) -> id {
    let thread = env.current_thread;
    state(env)
        .current
        .get(&thread)
        .and_then(|stack| stack.last())
        .map(|&(_, op)| op)
        .unwrap_or(nil)
}
fn main_queue(env: &mut Environment) -> id {
    if let Some(main) = state(env).main {
        return main;
    }
    let main: id = msg_class![env; NSOperationQueue new];
    let queue = env.objc.borrow_mut::<Queue>(main);
    queue.main = true;
    queue.max = 1;
    state(env).main = Some(main); // owns the original allocation reference
    main
}
fn current_queue(env: &mut Environment) -> id {
    let thread = env.current_thread;
    if let Some(&(queue, _)) = state(env)
        .current
        .get(&thread)
        .and_then(|stack| stack.last())
    {
        return queue;
    }
    if thread == 0 {
        main_queue(env)
    } else {
        nil
    }
}
fn callback(env: &mut Environment, coordinator: bool) -> GuestFunction {
    let cached = if coordinator {
        state(env).coordinator
    } else {
        state(env).worker
    };
    if let Some(callback) = cached {
        return callback;
    }
    let (name, callback) = if coordinator {
        export_c_func!(coordinate(_))
    } else {
        export_c_func!(worker(_))
    };
    let callback = env.dyld.create_guest_function(&mut env.mem, name, callback);
    if coordinator {
        state(env).coordinator = Some(callback);
    } else {
        state(env).worker = Some(callback);
    }
    callback
}
fn ensure_coordinator(env: &mut Environment, queue: id) {
    let q = env.objc.borrow_mut::<Queue>(queue);
    if q.coordinator_running || q.pending.is_empty() {
        return;
    }
    q.coordinator_running = true;
    retain(env, queue);
    let entry = callback(env, true);
    env.new_thread(
        entry,
        queue.cast(),
        Mem::SECONDARY_THREAD_DEFAULT_STACK_SIZE,
    );
}
fn take_ready(env: &mut Environment, queue: id) -> Option<id> {
    let q = env.objc.borrow::<Queue>(queue);
    let limit = if q.main {
        1
    } else if q.max == -1 {
        4
    } else {
        q.max as usize
    };
    if q.suspended || q.running.len() >= limit {
        return None;
    }
    let pending = q.pending.clone();
    let mut ready = Vec::new();
    for (index, operation) in pending.into_iter().enumerate() {
        let is_ready: bool = msg![env; operation isReady];
        if is_ready {
            let priority: NSInteger = msg![env; operation queuePriority];
            ready.push((operation, priority, index));
        }
    }
    ready.sort_by_key(|&(_, priority, index)| (std::cmp::Reverse(priority), index));
    let operation = ready.first()?.0;
    let q = env.objc.borrow_mut::<Queue>(queue);
    let limit = if q.main {
        1
    } else if q.max == -1 {
        4
    } else {
        q.max as usize
    };
    if q.suspended || q.running.len() >= limit {
        return None;
    }
    let index = q.pending.iter().position(|&op| op == operation)?;
    q.pending.remove(index);
    q.running.push(operation);
    Some(operation)
}
fn run(env: &mut Environment, queue: id, operation: id, monitor: bool) {
    let thread = env.current_thread;
    state(env)
        .current
        .entry(thread)
        .or_default()
        .push((queue, operation));
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let finished: bool = msg![env; operation isFinished];
    if !finished && !monitor {
        () = msg![env; operation start];
    }
    let finished: bool = msg![env; operation isFinished];
    if is_main(env, queue) && !monitor && !finished {
        // An asynchronous main-queue operation must not block the main runloop.
        release(env, pool);
        assert_eq!(
            state(env).current.get_mut(&thread).unwrap().pop(),
            Some((queue, operation))
        );
        let token = env.mem.alloc(4);
        state(env).jobs.insert(token, (queue, operation, true));
        let entry = callback(env, false);
        env.new_thread(entry, token, Mem::SECONDARY_THREAD_DEFAULT_STACK_SIZE);
        return; // transfer the existing queue/operation ownership to the monitor
    }
    while !msg![env; operation isFinished] {
        env.sleep(Duration::from_millis(1));
    }
    completion(env, operation);
    release(env, pool);
    assert_eq!(
        state(env).current.get_mut(&thread).unwrap().pop(),
        Some((queue, operation))
    );
    let q = env.objc.borrow_mut::<Queue>(queue);
    let index = q.running.iter().position(|&op| op == operation).unwrap();
    q.running.remove(index);
    env.objc.borrow_mut::<Operation>(operation).queue = nil;
    release(env, operation); // release the queue's ownership
    release(env, queue); // release the worker's ownership
}
fn worker(env: &mut Environment, token: MutVoidPtr) -> MutVoidPtr {
    let (queue, operation, monitor) = state(env).jobs.remove(&token).unwrap();
    env.mem.free(token);
    run(env, queue, operation, monitor);
    MutVoidPtr::null()
}
fn coordinate(env: &mut Environment, queue: MutVoidPtr) -> MutVoidPtr {
    let queue = queue.cast();
    loop {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        if !is_main(env, queue) {
            while let Some(operation) = take_ready(env, queue) {
                retain(env, queue);
                let token = env.mem.alloc(4);
                state(env).jobs.insert(token, (queue, operation, false));
                let entry = callback(env, false);
                env.new_thread(entry, token, Mem::SECONDARY_THREAD_DEFAULT_STACK_SIZE);
            }
        }
        release(env, pool);
        let q = env.objc.borrow::<Queue>(queue);
        if q.pending.is_empty() && q.running.is_empty() {
            break;
        }
        env.sleep(Duration::from_millis(1));
    }
    env.objc.borrow_mut::<Queue>(queue).coordinator_running = false;
    release(env, queue);
    MutVoidPtr::null()
}
pub fn drain_main(env: &mut Environment) {
    assert_eq!(env.current_thread, 0);
    let Some(main) = state(env).main else {
        return;
    };
    // A bounded batch keeps the main runloop responsive to timers and input.
    for _ in 0..64 {
        let Some(operation) = take_ready(env, main) else {
            break;
        };
        retain(env, main);
        run(env, main, operation, false);
    }
}
fn add(env: &mut Environment, queue: id, operation: id) {
    assert!(operation != nil);
    let executing: bool = msg![env; operation isExecuting];
    let finished: bool = msg![env; operation isFinished];
    let op = env.objc.borrow_mut::<Operation>(operation);
    assert!(
        !op.enqueued && !op.started && !executing && !finished,
        "An operation may be queued only once before starting"
    );
    op.enqueued = true;
    op.queue = queue;
    retain(env, operation);
    env.objc.borrow_mut::<Queue>(queue).pending.push(operation);
    ensure_coordinator(env, queue);
}
fn all_operations(env: &Environment, queue: id) -> Vec<id> {
    let q = env.objc.borrow::<Queue>(queue);
    q.pending.iter().chain(&q.running).copied().collect()
}
fn wait_all(env: &mut Environment, queue: id) {
    let thread = env.current_thread;
    let own = state(env)
        .current
        .get(&thread)
        .is_some_and(|stack| stack.iter().any(|&(q, _)| q == queue));
    assert!(!own, "Operation cannot wait for its own queue to empty");
    assert!(
        thread != 0 || !is_main(env, queue) || all_operations(env, queue).is_empty(),
        "Main thread cannot wait for its own queue"
    );
    while !all_operations(env, queue).is_empty() {
        env.sleep(Duration::from_millis(1));
    }
}
pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation NSOperationQueue: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::<Queue>::default(), &mut env.mem) }
+ (id)mainQueue { main_queue(env) }
+ (id)currentQueue { current_queue(env) }
- (())dealloc {
    let q = env.objc.borrow_mut::<Queue>(this);
    assert!(q.pending.is_empty() && q.running.is_empty() && !q.coordinator_running, "Queue destroyed with unfinished operations");
    let name = std::mem::take(&mut q.name);
    release(env, name);
    env.objc.dealloc_object(this, &mut env.mem);
}
- (())addOperation:(id)operation { add(env, this, operation); }
- (())addOperationWithBlock:(MutVoidPtr)block {
    let operation: id = msg_class![env; NSBlockOperation new];
    () = msg![env; operation addExecutionBlock:block];
    add(env, this, operation);
    release(env, operation);
}
- (())addOperations:(id)operations waitUntilFinished:(bool)wait {
    let count: NSUInteger = msg![env; operations count];
    for index in 0..count { let op: id = msg![env; operations objectAtIndex:index]; add(env, this, op); }
    if wait { wait_all(env, this); }
}
- (id)operations { snapshot(env, all_operations(env, this)) }
- (NSUInteger)operationCount { all_operations(env, this).len().try_into().unwrap() }
- (())cancelAllOperations {
    let operations = all_operations(env, this);
    for &operation in &operations { retain(env, operation); }
    for operation in operations { () = msg![env; operation cancel]; release(env, operation); }
}
- (bool)isSuspended { env.objc.borrow::<Queue>(this).suspended }
- (())setSuspended:(bool)suspended { env.objc.borrow_mut::<Queue>(this).suspended = suspended; ensure_coordinator(env, this); }
- (NSInteger)maxConcurrentOperationCount { env.objc.borrow::<Queue>(this).max }
- (())setMaxConcurrentOperationCount:(NSInteger)count {
    assert!(count >= -1, "Invalid maximum operation count");
    let q = env.objc.borrow_mut::<Queue>(this);
    assert!(!q.main || count == 1 || count == -1, "Main queue must remain serial");
    q.max = if q.main { 1 } else { count };
    ensure_coordinator(env, this);
}
- (id)name { env.objc.borrow::<Queue>(this).name }
- (())setName:(id)name {
    let copy: id = msg![env; name copy];
    let old = std::mem::replace(&mut env.objc.borrow_mut::<Queue>(this).name, copy);
    release(env, old);
}
- (())waitUntilAllOperationsAreFinished { wait_all(env, this); }
@end
};
