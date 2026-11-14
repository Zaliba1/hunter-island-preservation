/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Own operation subclasses and ARM/Blocks fixtures; no game code.
use super::*;
use crate::abi::GuestFunction;
use crate::dyld::export_c_func;
use crate::mem::MutPtr;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Instant;

type Trace = Rc<RefCell<Vec<(u32, usize)>>>;
#[derive(Default)]
struct Probe {
    superclass: Operation,
    tag: u32,
    expected_queue: id,
    trace: Trace,
    active: Rc<Cell<u32>>,
    peak: Rc<Cell<u32>>,
    destroyed: Option<Arc<AtomicUsize>>,
    async_executing: bool,
    async_finished: bool,
    wait_for_queue: id,
}
impl_HostObject_with_superclass!(Probe);
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(counter) = &self.destroyed {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    }
}
pub(crate) const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation _touchHLE_OperationProbe: NSOperation
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::<Probe>::default(), &mut env.mem) }
- (())main {
    let current: id = msg_class![env; NSOperationQueue currentQueue];
    let p = env.objc.borrow::<Probe>(this);
    assert_eq!(current, p.expected_queue);
    let (tag, trace, active, peak) = (p.tag, p.trace.clone(), p.active.clone(), p.peak.clone());
    active.set(active.get()+1); peak.set(peak.get().max(active.get()));
    trace.borrow_mut().push((tag, env.current_thread));
    let waiting_for = env.objc.borrow::<Probe>(this).wait_for_queue;
    if waiting_for != nil { () = msg![env; waiting_for waitUntilAllOperationsAreFinished]; }
    else { env.sleep(Duration::from_millis(8)); }
    let executing: bool = msg![env; this isExecuting];
    assert!(executing);
    trace.borrow_mut().push((tag+1000, env.current_thread));
    active.set(active.get()-1);
}
@end
@implementation _touchHLE_AsyncOperationProbe: _touchHLE_OperationProbe
- (bool)isConcurrent { true }
- (bool)isAsynchronous { true }
- (bool)isExecuting { env.objc.borrow::<Probe>(this).async_executing }
- (bool)isFinished { env.objc.borrow::<Probe>(this).async_finished }
- (())start {
    let p = env.objc.borrow_mut::<Probe>(this);
    assert!(!p.async_executing && !p.async_finished);
    p.async_executing = true;
    p.trace.borrow_mut().push((p.tag, env.current_thread));
    retain(env, this);
    let (name, function) = export_c_func!(finish_async(_));
    let function = env.dyld.create_guest_function(&mut env.mem, name, function);
    env.new_thread(function, this.cast(), crate::mem::Mem::SECONDARY_THREAD_DEFAULT_STACK_SIZE);
}
@end
};
fn finish_async(env: &mut Environment, pointer: MutVoidPtr) -> MutVoidPtr {
    let operation = pointer.cast();
    env.sleep(Duration::from_millis(12));
    let p = env.objc.borrow_mut::<Probe>(operation);
    p.trace
        .borrow_mut()
        .push((p.tag + 1000, env.current_thread));
    p.async_executing = false;
    p.async_finished = true;
    release(env, operation);
    MutVoidPtr::null()
}
fn wait(env: &mut Environment, mut predicate: impl FnMut(&mut Environment) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate(env) {
        assert!(Instant::now() < deadline, "Operation probe timed out");
        queue::drain_main(env);
        env.sleep(Duration::from_millis(1));
    }
}
fn queuedone(env: &mut Environment, q: id) {
    wait(env, |env| {
        let count: u32 = msg![env; q operationCount];
        count == 0
    });
}
fn operation(
    env: &mut Environment,
    q: id,
    tag: u32,
    trace: &Trace,
    active: &Rc<Cell<u32>>,
    peak: &Rc<Cell<u32>>,
) -> id {
    let operation: id = msg_class![env; _touchHLE_OperationProbe new];
    let p = env.objc.borrow_mut::<Probe>(operation);
    p.tag = tag;
    p.expected_queue = q;
    p.trace = trace.clone();
    p.active = active.clone();
    p.peak = peak.clone();
    operation
}
struct Fixture {
    literal: MutVoidPtr,
    descriptor: MutPtr<u32>,
}
fn fixture(env: &mut Environment, invoke: GuestFunction, captures: &[u32]) -> Fixture {
    let size = 20 + captures.len() as u32 * 4;
    let descriptor = env.mem.alloc(8).cast::<u32>();
    env.mem.write(descriptor, 0);
    env.mem.write(descriptor + 1, size);
    let isa = env
        .objc
        .get_known_class("_NSConcreteStackBlock", &mut env.mem);
    let literal = env.mem.alloc(size);
    env.mem.write(
        literal.cast(),
        blocks::Header {
            isa,
            flags: 0,
            reserved: 0,
            invoke,
            descriptor,
        },
    );
    for (index, &capture) in captures.iter().enumerate() {
        env.mem
            .write(literal.cast::<u32>() + 5 + index as u32, capture);
    }
    Fixture {
        literal,
        descriptor,
    }
}
fn block_callback(env: &mut Environment, block: MutVoidPtr) {
    let capture = block.cast::<u32>() + 5;
    let counter = MutPtr::<u32>::from_bits(env.mem.read(capture));
    let action = env.mem.read(capture + 1);
    if action == 3 {
        let operation = MutPtr::from_bits(env.mem.read(capture + 2));
        let finished: bool = msg![env; operation isFinished];
        let executing: bool = msg![env; operation isExecuting];
        assert!(finished && !executing);
        () = msg![env; operation waitUntilFinished];
    }
    if action == 1 {
        let deadline = Instant::now() + Duration::from_secs(2);
        while env.mem.read(counter) == 0 {
            assert!(
                Instant::now() < deadline,
                "NSBlockOperation did not run its blocks concurrently"
            );
            env.sleep(Duration::from_millis(1));
        }
    }
    let value = env.mem.read(counter);
    env.mem.write(counter, value + 1);
}
fn main(env: &mut Environment) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let trace: Trace = Default::default();
    let active = Rc::new(Cell::new(0));
    let peak = Rc::new(Cell::new(0));
    let q: id = msg_class![env; NSOperationQueue new];
    () = msg![env; q setMaxConcurrentOperationCount:1i32];
    () = msg![env; q setSuspended:true];
    let first = operation(env, q, 10, &trace, &active, &peak);
    let dependent = operation(env, q, 20, &trace, &active, &peak);
    let cancelled = operation(env, q, 30, &trace, &active, &peak);
    () = msg![env; dependent addDependency:first];
    () = msg![env; dependent addDependency:first]; // idempotent and ownership-balanced
    () = msg![env; dependent setQueuePriority:8i32];
    let deps: id = msg![env; dependent dependencies];
    let count: u32 = msg![env; deps count];
    assert_eq!(count, 1);
    () = msg![env; q addOperation:dependent];
    () = msg![env; q addOperation:first];
    () = msg![env; q addOperation:cancelled];
    () = msg![env; cancelled cancel];
    env.sleep(Duration::from_millis(3));
    assert!(trace.borrow().is_empty());
    () = msg![env; q setSuspended:false];
    queuedone(env, q);
    assert_eq!(
        trace
            .borrow()
            .iter()
            .map(|&(tag, _)| tag)
            .collect::<Vec<_>>(),
        [10, 1010, 20, 1020]
    );
    let finished: bool = msg![env; cancelled isFinished];
    assert!(finished);
    let was_cancelled: bool = msg![env; cancelled isCancelled];
    assert!(was_cancelled);
    assert_eq!(peak.get(), 1);
    () = msg![env; q waitUntilAllOperationsAreFinished];
    echo!("OPERATION_DEPENDENCIES_SUSPENSION_CANCELLATION_PASS");

    trace.borrow_mut().clear();
    peak.set(0);
    () = msg![env; q setMaxConcurrentOperationCount:2i32];
    let mut more = Vec::new();
    for tag in [40, 50, 60] {
        let op = operation(env, q, tag, &trace, &active, &peak);
        () = msg![env; q addOperation:op];
        more.push(op);
    }
    wait(env, |_| active.get() == 2);
    () = msg![env; (more[0]) cancel];
    let finished: bool = msg![env; (more[0]) isFinished];
    assert!(!finished);
    queuedone(env, q);
    assert_eq!(peak.get(), 2);
    assert!(trace.borrow().iter().all(|&(_, thread)| thread != 0));
    echo!("OPERATION_CONCURRENCY_AND_RUNNING_CANCEL_PASS");

    trace.borrow_mut().clear();
    () = msg![env; q setSuspended:true];
    () = msg![env; q setMaxConcurrentOperationCount:1i32];
    let low = operation(env, q, 70, &trace, &active, &peak);
    let high = operation(env, q, 80, &trace, &active, &peak);
    () = msg![env; low setQueuePriority:(-8i32)];
    () = msg![env; high setQueuePriority:8i32];
    () = msg![env; q addOperation:low];
    () = msg![env; q addOperation:high];
    () = msg![env; q setSuspended:false];
    queuedone(env, q);
    assert_eq!(
        trace
            .borrow()
            .iter()
            .map(|&(tag, _)| tag)
            .collect::<Vec<_>>(),
        [80, 1080, 70, 1070]
    );
    echo!("OPERATION_READY_PRIORITY_PASS");

    let code = env.mem.alloc(10);
    env.mem
        .bytes_at_mut(code.cast(), 10)
        .copy_from_slice(&[0x41, 0x69, 0x0a, 0x68, 0x01, 0x32, 0x0a, 0x60, 0x70, 0x47]);
    let invoke = GuestFunction::from_addr_with_thumb_bit(code.to_bits() | 1);
    let counter = env.mem.alloc(4).cast::<u32>();
    env.mem.write(counter, 0);
    let complete_count = env.mem.alloc(4).cast::<u32>();
    env.mem.write(complete_count, 0);
    let f = fixture(env, invoke, &[counter.to_bits()]);
    let complete = fixture(env, invoke, &[complete_count.to_bits()]);
    let block_op: id = msg_class![env; NSBlockOperation new];
    () = msg![env; block_op addExecutionBlock:(f.literal)];
    () = msg![env; block_op setCompletionBlock:(complete.literal)];
    env.mem.free(f.literal);
    env.mem.free(complete.literal); // only the actual heap copies remain
    () = msg![env; q setSuspended:true];
    () = msg![env; q addOperation:block_op];
    assert_eq!(env.mem.read(counter), 0);
    () = msg![env; q setSuspended:false];
    queuedone(env, q);
    assert_eq!(env.mem.read(counter), 1);
    assert_eq!(env.mem.read(complete_count), 1);
    let copied: MutVoidPtr = msg![env; block_op completionBlock];
    assert!(copied.is_null());
    let (name, cb) = export_c_func!(block_callback(_));
    let cb = env.dyld.create_guest_function(&mut env.mem, name, cb);
    env.mem.write(counter, 0);
    let a = fixture(env, cb, &[counter.to_bits(), 1]);
    let b = fixture(env, cb, &[counter.to_bits(), 2]);
    let group_op: id = msg_class![env; NSBlockOperation new];
    () = msg![env; group_op addExecutionBlock:(a.literal)];
    () = msg![env; group_op addExecutionBlock:(b.literal)];
    () = msg![env; q addOperation:group_op];
    queuedone(env, q);
    assert_eq!(env.mem.read(counter), 2);
    echo!("OPERATION_BLOCK_ARM_COPY_COMPLETION_AND_GROUP_PASS");

    env.mem.write(counter, 0);
    env.mem.write(complete_count, 0);
    let skipped = fixture(env, invoke, &[counter.to_bits()]);
    let cancelled_block: id = msg_class![env; NSBlockOperation new];
    let cancelled_completion = fixture(
        env,
        cb,
        &[complete_count.to_bits(), 3, cancelled_block.to_bits()],
    );
    () = msg![env; cancelled_block addExecutionBlock:(skipped.literal)];
    () = msg![env; cancelled_block setCompletionBlock:(cancelled_completion.literal)];
    () = msg![env; q setSuspended:true];
    () = msg![env; q addOperation:cancelled_block];
    () = msg![env; q cancelAllOperations];
    assert_eq!(env.mem.read(complete_count), 0);
    () = msg![env; q setSuspended:false];
    queuedone(env, q);
    assert_eq!(env.mem.read(counter), 0);
    assert_eq!(env.mem.read(complete_count), 1);
    let waiter_queue: id = msg_class![env; NSOperationQueue new];
    let target = operation(env, q, 85, &trace, &active, &peak);
    let waiter = operation(env, waiter_queue, 86, &trace, &active, &peak);
    env.objc.borrow_mut::<Probe>(waiter).wait_for_queue = q;
    () = msg![env; q addOperation:target];
    () = msg![env; waiter_queue addOperation:waiter];
    queuedone(env, waiter_queue);
    let observations = trace.borrow();
    assert!(
        observations
            .iter()
            .position(|&(tag, _)| tag == 1085)
            .unwrap()
            < observations
                .iter()
                .position(|&(tag, _)| tag == 1086)
                .unwrap()
    );
    drop(observations);
    echo!("OPERATION_CANCELLED_COMPLETION_AND_CROSS_QUEUE_WAIT_PASS");

    trace.borrow_mut().clear();
    let main_queue: id = msg_class![env; NSOperationQueue mainQueue];
    let current: id = msg_class![env; NSOperationQueue currentQueue];
    assert_eq!(main_queue, current);
    let on_main = operation(env, main_queue, 90, &trace, &active, &peak);
    () = msg![env; main_queue addOperation:on_main];
    env.sleep(Duration::from_millis(2));
    assert!(trace.borrow().is_empty());
    queue::drain_main(env);
    queuedone(env, main_queue);
    assert!(trace.borrow().iter().all(|&(_, thread)| thread == 0));
    trace.borrow_mut().clear();
    let async_op: id = msg_class![env; _touchHLE_AsyncOperationProbe new];
    {
        let p = env.objc.borrow_mut::<Probe>(async_op);
        p.tag = 100;
        p.trace = trace.clone();
    }
    () = msg![env; main_queue addOperation:async_op];
    queue::drain_main(env);
    let finished: bool = msg![env; async_op isFinished];
    assert!(!finished, "Async main operation blocked the main runloop");
    queuedone(env, main_queue);
    assert_eq!(trace.borrow()[0], (100, 0));
    assert_ne!(trace.borrow()[1].1, 0);
    echo!("OPERATION_MAIN_QUEUE_AND_ASYNC_SUBCLASS_PASS");

    let destroyed = Arc::new(AtomicUsize::new(0));
    let owned_q: id = msg_class![env; NSOperationQueue new];
    let owned = operation(env, owned_q, 110, &trace, &active, &peak);
    env.objc.borrow_mut::<Probe>(owned).destroyed = Some(destroyed.clone());
    () = msg![env; owned_q addOperation:owned];
    release(env, owned);
    release(env, owned_q);
    wait(env, |_| destroyed.load(Ordering::SeqCst) == 1);
    for op in [
        first,
        dependent,
        cancelled,
        low,
        high,
        block_op,
        group_op,
        cancelled_block,
        target,
        waiter,
        on_main,
        async_op,
    ]
    .into_iter()
    .chain(more)
    {
        release(env, op);
    }
    release(env, q);
    release(env, waiter_queue);
    wait(env, |env| {
        env.threads.iter().skip(1).all(|thread| !thread.is_alive())
    });
    for literal in [
        a.literal,
        b.literal,
        skipped.literal,
        cancelled_completion.literal,
    ] {
        env.mem.free(literal);
    }
    for pointer in [
        f.descriptor,
        complete.descriptor,
        a.descriptor,
        b.descriptor,
        skipped.descriptor,
        cancelled_completion.descriptor,
    ] {
        env.mem.free(pointer.cast());
    }
    for pointer in [code, counter.cast(), complete_count.cast()] {
        env.mem.free(pointer);
    }
    release(env, pool);
    echo!("OPERATION_OWNERSHIP_LIFETIME_PASS");
    crate::frameworks::foundation::ns_thread::probe::run(env);
    crate::libc::crypto::guest_probe(env);
    echo!("OPERATION_QUEUES_PROBE_PASS");
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
    fn operations_scheduler_lifecycle_integration() {
        let env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                popup_errors: false,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.run_framework_probe(super::main);
    }
}
