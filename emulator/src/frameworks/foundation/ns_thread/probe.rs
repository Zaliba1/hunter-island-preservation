/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Own fixtures for thread identity, run-loop delivery and message ownership.
use super::*;
use crate::frameworks::foundation::ns_run_loop;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

#[derive(Default)]
struct Probe {
    thread: id, // weak
    ready: bool,
    stop: bool,
    round_trip_done: bool,
    tag: u32,
    trace: Rc<RefCell<Vec<(usize, u32)>>>,
    destroyed: Option<Rc<Cell<u32>>>,
    display_times: Vec<f64>,
}
impl HostObject for Probe {}
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(count) = &self.destroyed {
            count.set(count.get() + 1);
        }
    }
}
pub(crate) const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation _touchHLE_ThreadSelectorProbe: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::<Probe>::default(), &mut env.mem) }
- (())capture:(id)arg {
    let tag = if arg == nil { 0 } else { env.objc.borrow::<Probe>(arg).tag };
    // A synchronous sender must stay blocked until the entire callback returns.
    env.sleep(Duration::from_millis(2));
    env.objc.borrow::<Probe>(this).trace.borrow_mut().push((env.current_thread, tag));
}
- (())roundTrip {
    let main: id = msg_class![env; NSThread mainThread];
    let capture = env.objc.register_host_selector("capture:".into(), &mut env.mem);
    () = msg![env; this performSelector:capture onThread:main withObject:nil waitUntilDone:true];
    env.objc.borrow_mut::<Probe>(this).round_trip_done = true;
}
- (())displayTick:(id)link {
    let timestamp: f64 = msg![env; link timestamp];
    env.sleep(Duration::from_millis(1));
    let again: f64 = msg![env; link timestamp];
    assert_eq!(timestamp, again, "Timestamp must describe a callback, not the query time");
    env.objc.borrow_mut::<Probe>(this).display_times.push(timestamp);
}
- (())service:(id)_arg {
    let current: id = msg_class![env; NSThread currentThread];
    assert_eq!(current, env.objc.borrow::<Probe>(this).thread);
    let is_main: bool = msg![env; current isMainThread];
    assert!(!is_main);
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    env.objc.borrow_mut::<Probe>(this).ready = true;
    while !env.objc.borrow::<Probe>(this).stop {
        ns_run_loop::drain_perform_requests(env, run_loop);
        env.sleep(Duration::from_millis(1));
    }
}
@end
};
fn wait(env: &mut Environment, mut done: impl FnMut(&Environment) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !done(env) {
        assert!(Instant::now() < deadline, "Thread selector probe timeout");
        let run_loop = ns_run_loop::for_thread(env, env.current_thread);
        ns_run_loop::drain_perform_requests(env, run_loop);
        env.sleep(Duration::from_millis(1));
    }
}
fn raw_identity(env: &mut Environment, _: crate::mem::MutVoidPtr) -> crate::mem::MutVoidPtr {
    let current: id = msg_class![env; NSThread currentThread];
    let again: id = msg_class![env; NSThread currentThread];
    assert_eq!(current, again);
    assert_eq!(id_for_thread(env, current), Some(env.current_thread));
    let main: id = msg_class![env; NSThread mainThread];
    assert_ne!(current, main);
    crate::mem::MutVoidPtr::null()
}
pub(crate) fn run(env: &mut Environment) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let target: id = msg_class![env; _touchHLE_ThreadSelectorProbe new];
    let main: id = msg_class![env; NSThread mainThread];
    let current: id = msg_class![env; NSThread currentThread];
    assert_eq!(main, current);
    let capture = env
        .objc
        .register_host_selector("capture:".into(), &mut env.mem);
    let trace = env.objc.borrow::<Probe>(target).trace.clone();
    () = msg![env; target performSelector:capture onThread:main withObject:nil waitUntilDone:true];
    assert_eq!(&*trace.borrow(), &[(0, 0)]);
    trace.borrow_mut().clear();
    () = msg![env; target performSelector:capture onThread:main withObject:nil waitUntilDone:false];
    assert!(trace.borrow().is_empty());
    () = msg_class![env; NSObject cancelPreviousPerformRequestsWithTarget:target selector:capture object:nil];
    let run_loop = ns_run_loop::for_thread(env, 0);
    ns_run_loop::drain_perform_requests(env, run_loop);
    assert_eq!(&*trace.borrow(), &[(0, 0)]);
    trace.borrow_mut().clear();
    // Delayed performs remain separately cancelable.
    () = msg![env; target performSelector:capture withObject:nil afterDelay:0.0f64];
    () = msg_class![env; NSObject cancelPreviousPerformRequestsWithTarget:target selector:capture object:nil];
    ns_run_loop::drain_perform_requests(env, run_loop);
    assert!(trace.borrow().is_empty());
    echo!("THREAD_SELECTOR_SAME_THREAD_AND_NONCANCELABLE_PASS");

    let thread: id = msg_class![env; NSThread alloc];
    let service = env
        .objc
        .register_host_selector("service:".into(), &mut env.mem);
    let thread: id = msg![env; thread initWithTarget:target selector:service object:nil];
    env.objc.borrow_mut::<Probe>(target).thread = thread;
    () = msg![env; thread start];
    wait(env, |env| env.objc.borrow::<Probe>(target).ready);
    let thread_id = id_for_thread(env, thread).unwrap();
    () =
        msg![env; target performSelector:capture onThread:thread withObject:nil waitUntilDone:true];
    assert_eq!(&*trace.borrow(), &[(thread_id, 0)]);
    trace.borrow_mut().clear();
    let round_trip = env
        .objc
        .register_host_selector("roundTrip".into(), &mut env.mem);
    () = msg![env; target performSelector:round_trip onThread:thread withObject:nil waitUntilDone:false];
    wait(env, |env| env.objc.borrow::<Probe>(target).round_trip_done);
    assert_eq!(&*trace.borrow(), &[(0, 0)]);
    echo!("THREAD_SELECTOR_CROSS_THREAD_AND_WAIT_PASS");

    let destroyed = Rc::new(Cell::new(0));
    let receiver: id = msg_class![env; _touchHLE_ThreadSelectorProbe new];
    let argument: id = msg_class![env; _touchHLE_ThreadSelectorProbe new];
    let delivered = env.objc.borrow::<Probe>(receiver).trace.clone();
    env.objc.borrow_mut::<Probe>(receiver).destroyed = Some(destroyed.clone());
    env.objc.borrow_mut::<Probe>(argument).destroyed = Some(destroyed.clone());
    env.objc.borrow_mut::<Probe>(argument).tag = 42;
    () = msg![env; receiver performSelector:capture onThread:main withObject:argument waitUntilDone:false];
    release(env, receiver);
    release(env, argument);
    assert_eq!(destroyed.get(), 0);
    ns_run_loop::drain_perform_requests(env, run_loop);
    assert_eq!(&*delivered.borrow(), &[(0, 42)]);
    assert_eq!(destroyed.get(), 2);
    env.objc.borrow_mut::<Probe>(target).stop = true;
    wait(env, |env| !env.threads[thread_id].is_alive());
    let finished: bool = msg![env; thread isFinished];
    assert!(finished);
    release(env, thread);
    let display_tick = env.objc.lookup_selector("displayTick:").unwrap();
    crate::frameworks::core_animation::ca_display_link::timing_probe(env, target, display_tick);
    assert_eq!(env.objc.borrow::<Probe>(target).display_times.len(), 2);
    release(env, target);
    let (name, function) = crate::dyld::export_c_func!(raw_identity(_));
    let function = env.dyld.create_guest_function(&mut env.mem, name, function);
    let worker = env.new_thread(
        function,
        MutPtr::null(),
        Mem::SECONDARY_THREAD_DEFAULT_STACK_SIZE,
    );
    wait(env, |env| !env.threads[worker].is_alive());
    release(env, pool);
    echo!("THREAD_SELECTOR_OWNERSHIP_AND_FRAMEWORK_IDENTITY_PASS");
    echo!("THREAD_SELECTORS_PROBE_PASS");
}
