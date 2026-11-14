/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Trusted callbacks and compiler-ABI fixtures, never game code.
use super::*;
use crate::objc::{msg, HostObject};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Instant;

struct Fixture {
    literal: Handle,
    descriptor: MutPtr<u32>,
}
fn callback(env: &mut Environment, block: Handle) {
    let capture = block.cast::<u32>() + 5;
    let tag = env.mem.read(capture);
    let action = env.mem.read(capture + 1);
    let argument: Handle = env.mem.read((capture + 2).cast());
    let thread = env.current_thread;
    if action == 1 {
        // deliberate yielding inside a serial callback
        state(env).trace.push((tag, thread));
        env.sleep(Duration::from_millis(5));
        state(env).trace.push((tag + 1000, thread));
        return;
    } else if action == 2 {
        let deadline = dispatch_time(env, 0, 500_000_000);
        assert_eq!(dispatch_semaphore_wait(env, argument, deadline), 0);
    } else if action == 3 {
        assert_eq!(dispatch_semaphore_signal(env, argument), 1);
    } else if action == 4 {
        let once_block: Handle = env.mem.read((capture + 3).cast());
        dispatch_once(env, argument.cast(), once_block);
        assert_eq!(env.mem.read(argument.cast::<i32>()), -1);
    } else if action == 5 {
        env.sleep(Duration::from_millis(5));
        let counter = argument.cast::<u32>();
        let n = env.mem.read(counter);
        env.mem.write(counter, n + 1);
    } else if action == 6 {
        let nested: Handle = env.mem.read((capture + 3).cast());
        dispatch_sync(env, argument, nested);
    } else if action == 7 {
        assert_eq!(current_queue(env), argument);
    } else if action == 8 {
        let nested: Handle = env.mem.read((capture + 3).cast());
        state(env).trace.push((tag, thread));
        dispatch_async(env, argument, nested);
        drain_main(env);
        assert!(
            !trace(env).contains(&(tag + 1)),
            "Main queue reentered itself"
        );
        state(env).trace.push((tag + 1000, thread));
        return;
    }
    state(env).trace.push((tag, thread));
}
fn fixture(
    env: &mut Environment,
    invoke: GuestFunction,
    words: &[u32],
    helpers: bool,
    global: bool,
) -> Fixture {
    let size = 20 + u32::try_from(words.len()).unwrap() * 4;
    let descriptor: MutPtr<u32> = env.mem.alloc(if helpers { 16 } else { 8 }).cast();
    env.mem.write(descriptor, 0);
    env.mem.write(descriptor + 1, size);
    if helpers {
        let (name, f) = export_c_func!(copy_captures(_, _));
        let copy = env.dyld.create_guest_function(&mut env.mem, name, f);
        let (name, f) = export_c_func!(dispose_captures(_));
        let dispose = env.dyld.create_guest_function(&mut env.mem, name, f);
        env.mem.write((descriptor + 2).cast(), copy);
        env.mem.write((descriptor + 3).cast(), dispose);
    }
    // Instantiate class constants without relying on the game's linker.
    let (_, constant) = &blocks::CONSTANTS[if global { 2 } else { 0 }];
    let HostConstant::Custom(make) = constant else {
        unreachable!()
    };
    let isa = make(env).cast().cast_mut();
    let header = blocks::Header {
        isa,
        flags: (if helpers { blocks::HAS_HELPERS } else { 0 })
            | (if global { blocks::IS_GLOBAL } else { 0 }),
        reserved: 0,
        invoke,
        descriptor,
    };
    let literal = env.mem.alloc(size);
    env.mem.write(literal.cast(), header);
    for (index, value) in words.iter().enumerate() {
        env.mem
            .write(literal.cast::<u32>() + 5 + index as u32, *value);
    }
    Fixture {
        literal,
        descriptor,
    }
}
fn task(
    env: &mut Environment,
    fixtures: &mut Vec<Fixture>,
    invoke: GuestFunction,
    tag: u32,
    action: u32,
    arg: Handle,
    extra: Handle,
) -> Handle {
    let f = fixture(
        env,
        invoke,
        &[tag, action, arg.to_bits(), extra.to_bits()],
        false,
        false,
    );
    let block = f.literal;
    fixtures.push(f);
    block
}
fn wait(env: &mut Environment, condition: impl Fn(&mut Environment) -> bool) {
    let limit = Instant::now() + Duration::from_secs(2);
    while !condition(env) {
        assert!(Instant::now() < limit, "Framework probe timed out");
        drain_main(env);
        env.sleep(Duration::from_millis(1));
    }
}
fn trace(env: &mut Environment) -> Vec<u32> {
    state(env).trace.iter().map(|(tag, _)| *tag).collect()
}
fn copy_captures(env: &mut Environment, destination: Handle, source: Handle) {
    for (index, flags) in [(5, 3), (6, 8), (7, 7)] {
        let value: Handle = env.mem.read((source.cast::<u32>() + index).cast());
        blocks::_Block_object_assign(
            env,
            (destination.cast::<u32>() + index).cast(),
            value,
            flags,
        );
    }
    let thread = env.current_thread;
    state(env).trace.push((9000, thread));
}
fn dispose_captures(env: &mut Environment, block: Handle) {
    for (index, flags) in [(5, 3), (6, 8), (7, 7)] {
        let value: Handle = env.mem.read((block.cast::<u32>() + index).cast());
        blocks::_Block_object_dispose(env, value, flags);
    }
    let thread = env.current_thread;
    state(env).trace.push((9001, thread));
}
struct Lifetime(Arc<AtomicUsize>);
impl HostObject for Lifetime {}
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn main(env: &mut Environment) {
    assert_eq!(env.current_thread, 0);
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let (name, function) = export_c_func!(callback(_));
    let invoke = env.dyld.create_guest_function(&mut env.mem, name, function);
    let mut fixtures = Vec::new();
    let null = Handle::null();
    let label = env.mem.alloc_and_write_cstr(b"preservation.serial");
    let q = dispatch_queue_create(env, label.cast_const(), null);
    env.mem.free(label.cast());
    let returned_label = dispatch_queue_get_label(env, q);
    assert_eq!(env.mem.cstr_at(returned_label), b"preservation.serial");
    let g = dispatch_group_create(env);
    let first = task(env, &mut fixtures, invoke, 1, 1, null, null);
    let second = task(env, &mut fixtures, invoke, 2, 0, null, null);
    dispatch_group_async(env, g, q, first);
    dispatch_group_async(env, g, q, second);
    env.mem.free(first);
    fixtures[0].literal = Handle::null();
    assert!(trace(env).is_empty(), "dispatch_async ran inline");
    let deadline = dispatch_time(env, 0, 1_000_000_000);
    assert_eq!(dispatch_group_wait(env, g, deadline), 0);
    assert_eq!(trace(env), [1, 1001, 2]);
    assert!(state(env).trace.iter().all(|(_, thread)| *thread != 0));
    state(env).trace.clear();
    echo!("DISPATCH_SERIAL_FIFO_PASS");

    // Main-queue work is deferred and executed only by thread 0.
    let main = main_queue(env);
    let b = task(env, &mut fixtures, invoke, 10, 0, null, null);
    dispatch_async(env, main, b);
    assert!(trace(env).is_empty());
    drain_main(env);
    assert_eq!(state(env).trace, [(10, 0)]);
    state(env).trace.clear();
    let nested = task(env, &mut fixtures, invoke, 12, 0, null, null);
    let outer = task(env, &mut fixtures, invoke, 11, 8, main, nested);
    dispatch_async(env, main, outer);
    drain_main(env);
    assert_eq!(trace(env), [11, 1011]);
    drain_main(env);
    assert_eq!(trace(env), [11, 1011, 12]);
    state(env).trace.clear();

    // Concurrent workers must let a queued signal progress while another waits.
    let global = dispatch_get_global_queue(env, 0, 0);
    let sem = dispatch_semaphore_create(env, 0);
    let waiting = task(env, &mut fixtures, invoke, 20, 2, sem, null);
    let signal = task(env, &mut fixtures, invoke, 21, 3, sem, null);
    dispatch_group_async(env, g, global, waiting);
    dispatch_group_async(env, g, global, signal);
    let deadline = dispatch_time(env, 0, 1_000_000_000);
    assert_eq!(dispatch_group_wait(env, g, deadline), 0);
    assert_eq!(trace(env), [21, 20]);
    assert_eq!(dispatch_semaphore_wait(env, sem, 0), 1);
    let deadline = dispatch_time(env, 0, 5_000_000);
    assert_eq!(dispatch_semaphore_wait(env, sem, deadline), 1);
    assert_eq!(dispatch_semaphore_signal(env, sem), 0);
    assert_eq!(dispatch_semaphore_wait(env, sem, 0), 0);
    dispatch_release(env, sem);
    state(env).trace.clear();
    echo!("DISPATCH_CONCURRENT_SEMAPHORE_TIMEOUT_PASS");

    // Once remains pending while its initializer yields; all contenders wait.
    let predicate = env.mem.alloc_and_write(0i32);
    let counter = env.mem.alloc_and_write(0u32);
    let once = task(env, &mut fixtures, invoke, 30, 5, counter.cast(), null);
    for tag in 31..35 {
        let wrapper = task(env, &mut fixtures, invoke, tag, 4, predicate.cast(), once);
        dispatch_group_async(env, g, global, wrapper);
    }
    let deadline = dispatch_time(env, 0, 1_000_000_000);
    assert_eq!(dispatch_group_wait(env, g, deadline), 0);
    assert_eq!(env.mem.read(counter), 1);
    assert_eq!(env.mem.read(predicate), -1);
    assert_eq!(trace(env).iter().filter(|tag| **tag == 30).count(), 1);
    state(env).trace.clear();
    echo!("DISPATCH_ONCE_CONTENTION_PASS");

    // Serial sync obeys previously enqueued work; notification is deferred.
    let a = task(env, &mut fixtures, invoke, 40, 0, null, null);
    let b = task(env, &mut fixtures, invoke, 41, 0, null, null);
    dispatch_async(env, q, a);
    dispatch_sync(env, q, b);
    assert_eq!(trace(env), [40, 41]);
    state(env).trace.clear();
    dispatch_group_enter(env, g);
    let notify = task(env, &mut fixtures, invoke, 42, 0, null, null);
    dispatch_group_notify(env, g, main, notify);
    assert_eq!(dispatch_group_wait(env, g, 0), 1);
    drain_main(env);
    assert!(trace(env).is_empty());
    dispatch_group_leave(env, g);
    assert!(trace(env).is_empty());
    drain_main(env);
    assert_eq!(state(env).trace, [(42, 0)]);
    state(env).trace.clear();

    // A future task must neither run early nor block immediate queue work.
    let delayed = task(env, &mut fixtures, invoke, 50, 0, null, null);
    let immediate = task(env, &mut fixtures, invoke, 51, 0, null, null);
    let began = Instant::now();
    let deadline = dispatch_time(env, 0, 25_000_000);
    dispatch_after(env, deadline, q, delayed);
    dispatch_sync(env, q, immediate);
    assert_eq!(trace(env), [51]);
    wait(env, |env| trace(env).contains(&50));
    assert!(began.elapsed() >= Duration::from_millis(25));
    state(env).trace.clear();
    assert_eq!(dispatch_time(env, FOREVER, -100), FOREVER);
    assert_eq!(dispatch_time(env, 1, -100), 1);
    echo!("DISPATCH_SYNC_GROUP_NOTIFY_AFTER_PASS");

    // Concurrent child jobs target one serial parent and retain logical identity.
    let attr = concurrent_attribute(env);
    let child = dispatch_queue_create(env, ConstPtr::null(), attr);
    dispatch_set_target_queue(env, child, q);
    for tag in 60..63 {
        let block = task(env, &mut fixtures, invoke, tag, 1, null, null);
        dispatch_group_async(env, g, child, block);
    }
    let deadline = dispatch_time(env, 0, 1_000_000_000);
    assert_eq!(dispatch_group_wait(env, g, deadline), 0);
    let observed = trace(env);
    for pair in observed.chunks_exact(2) {
        assert_eq!(pair[1], pair[0] + 1000);
    }
    state(env).trace.clear();
    let check_queue = task(env, &mut fixtures, invoke, 65, 7, child, null);
    dispatch_sync(env, child, check_queue);
    let deadline = dispatch_time(env, 0, 1_000_000_000);
    assert_eq!(dispatch_group_wait(env, g, deadline), 0);
    state(env).trace.clear();
    dispatch_release(env, child);
    wait(env, |env| !state(env).objects.contains_key(&child));

    // Background sync onto the main queue really executes on the main thread.
    let main_callback = task(env, &mut fixtures, invoke, 70, 0, null, null);
    let background = task(env, &mut fixtures, invoke, 71, 6, main, main_callback);
    dispatch_group_async(env, g, global, background);
    wait(env, |env| group(env, g).count == 0);
    assert_eq!(trace(env), [70, 71]);
    assert_eq!(state(env).trace[0], (70, 0));
    assert_ne!(state(env).trace[1].1, 0);
    state(env).trace.clear();
    echo!("DISPATCH_MAIN_THREAD_AND_TARGET_QUEUE_PASS");

    // Legal nested sync from a serial child on a serial target must not be
    // mistaken for a cycle of logical queue identities.
    let serial_child = dispatch_queue_create(env, ConstPtr::null(), null);
    dispatch_set_target_queue(env, serial_child, q);
    let nested_sync = task(env, &mut fixtures, invoke, 75, 0, null, null);
    let targeted_sync = task(env, &mut fixtures, invoke, 76, 6, global, nested_sync);
    dispatch_sync(env, serial_child, targeted_sync);
    assert_eq!(trace(env), [75, 76]);
    dispatch_release(env, serial_child);
    wait(env, |env| !state(env).objects.contains_key(&serial_child));
    state(env).trace.clear();
    // Also exercise a held serial gate while executing on a concurrent target.
    let other_child = dispatch_queue_create(env, ConstPtr::null(), null);
    dispatch_set_target_queue(env, other_child, global);
    let nested_sync = task(env, &mut fixtures, invoke, 77, 0, null, null);
    let targeted_sync = task(env, &mut fixtures, invoke, 78, 6, q, nested_sync);
    dispatch_sync(env, other_child, targeted_sync);
    assert_eq!(trace(env), [77, 78]);
    dispatch_release(env, other_child);
    wait(env, |env| !state(env).objects.contains_key(&other_child));
    state(env).trace.clear();
    echo!("DISPATCH_NESTED_SYNC_AND_MAIN_NONREENTRANCY_PASS");

    // A real Thumb block callback writes a captured counter through ARM memory.
    let code = env.mem.alloc(10);
    env.mem
        .bytes_at_mut(code.cast(), 10)
        .copy_from_slice(&[0x41, 0x69, 0x0a, 0x68, 0x01, 0x32, 0x0a, 0x60, 0x70, 0x47]);
    let arm = GuestFunction::from_addr_and_thumb_flag(code.to_bits(), true);
    env.mem.write(counter, 0);
    let arm_fixture = fixture(env, arm, &[counter.to_bits()], false, false);
    dispatch_sync(env, q, arm_fixture.literal);
    assert_eq!(env.mem.read(counter), 1);
    fixtures.push(arm_fixture);

    // Heap promotion, helpers, nested captured Blocks and shared byref storage.
    let drops = Arc::new(AtomicUsize::new(0));
    let cls = env.objc.get_known_class("NSObject", &mut env.mem);
    let object = env
        .objc
        .alloc_object(cls, Box::new(Lifetime(drops.clone())), &mut env.mem);
    let byref: MutPtr<u32> = env.mem.alloc(20).cast();
    env.mem.write(byref, 0);
    env.mem.write((byref + 1).cast(), byref);
    env.mem.write(byref + 2, 0);
    env.mem.write(byref + 3, 20);
    env.mem.write(byref + 4, 123);
    let nested = fixture(env, arm, &[counter.to_bits()], false, false);
    let outer = fixture(
        env,
        arm,
        &[object.to_bits(), byref.to_bits(), nested.literal.to_bits()],
        true,
        false,
    );
    let first = blocks::_Block_copy(env, outer.literal);
    let second = blocks::_Block_copy(env, outer.literal);
    let again = blocks::_Block_copy(env, first);
    assert_eq!(first, again);
    assert_eq!(trace(env), [9000, 9000]);
    objc_release(env, object);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let shared: MutPtr<u32> = env.mem.read((first.cast::<u32>() + 6).cast());
    assert_ne!(shared, byref);
    assert_eq!(
        env.mem
            .read((second.cast::<u32>() + 6).cast::<MutPtr<u32>>()),
        shared
    );
    assert_eq!(env.mem.read((byref + 1).cast::<MutPtr<u32>>()), shared);
    env.mem.write(shared + 4, 456);
    blocks::_Block_release(env, first);
    blocks::_Block_release(env, again);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    blocks::_Block_release(env, second);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(env.mem.read(shared + 4), 456);
    blocks::_Block_object_dispose(env, byref.cast(), 8);
    assert!(env.libc_state.blocks.heap_byrefs.is_empty());
    assert!(env.libc_state.blocks.heap_blocks.is_empty());
    fixtures.extend([nested, outer]);
    state(env).trace.clear();

    // Objective-C copy/retain/autorelease on a raw literal (no HostObjectEntry).
    let raw = fixture(env, arm, &[counter.to_bits()], false, false);
    let raw_id: id = raw.literal.cast();
    let copy: id = msg![env; raw_id copy];
    assert_ne!(copy, raw_id);
    let retained: id = msg![env; copy retain];
    assert_eq!(copy, retained);
    objc_release(env, retained);
    let inner_pool: id = msg_class![env; NSAutoreleasePool new];
    let _: id = msg![env; copy autorelease];
    objc_release(env, inner_pool);
    assert!(env.libc_state.blocks.heap_blocks.is_empty());
    let global_block = fixture(env, arm, &[counter.to_bits()], false, true);
    assert_eq!(
        blocks::_Block_copy(env, global_block.literal),
        global_block.literal
    );
    blocks::_Block_release(env, global_block.literal);
    fixtures.extend([raw, global_block]);
    echo!("BLOCKS_ARM_CAPTURE_COPY_DISPOSE_BYREF_OBJC_PASS");

    dispatch_release(env, q);
    dispatch_release(env, g);
    wait(env, |env| {
        state(env)
            .objects
            .values()
            .all(|object| match &object.kind {
                Kind::Queue(q) => q.active == 0,
                _ => true,
            })
    });
    assert!(state(env).delayed.is_empty());
    assert!(state(env).waits.is_empty());
    assert!(state(env).once_owners.is_empty());
    assert!(
        state(env).executing.is_empty(),
        "finished work must not retain per-thread execution entries"
    );
    wait(env, |env| {
        env.threads.iter().skip(1).all(|thread| !thread.is_alive())
    });
    assert_eq!(
        env.living_thread_count(),
        1,
        "finished workers must leave the scheduler index"
    );
    for f in fixtures {
        if !f.literal.is_null() {
            env.mem.free(f.literal);
        }
        env.mem.free(f.descriptor.cast());
    }
    for p in [code, counter.cast(), predicate.cast(), byref.cast()] {
        env.mem.free(p);
    }
    objc_release(env, pool);
    echo!("DISPATCH_BLOCKS_PROBE_PASS");
}
pub(super) fn run() -> Result<(), String> {
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
    use super::*;
    #[test]
    fn dispatch_blocks_scheduler_integration() {
        let env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                popup_errors: false,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        env.run_framework_probe(main);
    }
    #[test]
    fn rejects_serial_wait_cycles_without_scheduling_work() {
        let mut env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                popup_errors: false,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let a = Handle::from_bits(0x1000);
        let b = Handle::from_bits(0x2000);
        let _ = wait_edge(&mut env, a, b);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| wait_edge(&mut env, b, a)))
                .is_err()
        );
        assert_eq!(state(&mut env).waits.len(), 1);
    }
}
