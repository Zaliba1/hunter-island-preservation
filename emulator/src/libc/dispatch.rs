/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Cooperative GCD queues and Blocks for the 32-bit Darwin guest.
//! No native host threads execute guest code; all work uses Environment's
//! existing coroutine scheduler. The main queue is drained by the main runloop.
mod probe;
use super::blocks;
use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr};
use crate::objc::{id, msg_class, objc_classes, release as objc_release, ClassExports, SEL};
use crate::{Environment, ThreadId};
use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type Handle = MutVoidPtr;
const FOREVER: u64 = u64::MAX;
#[derive(Default)]
pub struct State {
    objects: HashMap<Handle, Object>,
    object_class: Option<crate::objc::Class>,
    main: Option<Handle>,
    draining_main: bool,
    globals: HashMap<i32, Handle>,
    concurrent_attribute: Option<Handle>,
    worker: Option<GuestFunction>,
    timer: Option<GuestFunction>,
    delayed: HashMap<Handle, (u64, Handle, Work)>,
    executing: HashMap<ThreadId, Vec<Handle>>,
    once_owners: HashMap<MutPtr<i32>, ThreadId>,
    waits: HashMap<Handle, Handle>,
    // Only the trusted framework probe writes these observations.
    trace: Vec<(u32, ThreadId)>,
}
struct Object {
    references: Option<u32>,
    kind: Kind,
}
enum Kind {
    Queue(Queue),
    Group(Group),
    Semaphore(Semaphore),
}
struct Queue {
    concurrent: bool,
    main: bool,
    active: usize,
    work: VecDeque<Work>,
    label: MutPtr<u8>,
    target: Option<Handle>,
}
#[derive(Default)]
struct Group {
    count: usize,
    notifications: Vec<(Handle, Work)>,
}
struct Semaphore {
    value: i64,
    waiting: VecDeque<ThreadId>,
    granted: HashSet<ThreadId>,
}
struct Work {
    block: Handle,
    owned: bool,
    logical_queue: Handle,
    group: Option<Handle>,
    done: Option<Rc<Cell<bool>>>,
}
pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);
    @implementation _touchHLE_DispatchObject: NSObject
    @end
};
fn state(env: &mut Environment) -> &mut State {
    &mut env.libc_state.dispatch
}
fn queue(env: &mut Environment, handle: Handle) -> &mut Queue {
    match &mut state(env)
        .objects
        .get_mut(&handle)
        .expect("Invalid dispatch object")
        .kind
    {
        Kind::Queue(q) => q,
        _ => panic!("Expected dispatch queue"),
    }
}
fn group(env: &mut Environment, handle: Handle) -> &mut Group {
    match &mut state(env)
        .objects
        .get_mut(&handle)
        .expect("Invalid dispatch object")
        .kind
    {
        Kind::Group(g) => g,
        _ => panic!("Expected dispatch group"),
    }
}
fn semaphore(env: &mut Environment, handle: Handle) -> &mut Semaphore {
    match &mut state(env)
        .objects
        .get_mut(&handle)
        .expect("Invalid dispatch object")
        .kind
    {
        Kind::Semaphore(s) => s,
        _ => panic!("Expected dispatch semaphore"),
    }
}
fn object(env: &mut Environment, kind: Kind, immortal: bool) -> Handle {
    let class = env
        .objc
        .get_known_class("_touchHLE_DispatchObject", &mut env.mem);
    state(env).object_class = Some(class);
    let handle = env.mem.alloc_and_write(class).cast_void();
    state(env).objects.insert(
        handle,
        Object {
            references: (!immortal).then_some(1),
            kind,
        },
    );
    handle
}
fn make_queue(
    env: &mut Environment,
    label: &[u8],
    concurrent: bool,
    main: bool,
    immortal: bool,
) -> Handle {
    let label = env.mem.alloc_and_write_cstr(label);
    object(
        env,
        Kind::Queue(Queue {
            label,
            concurrent,
            main,
            active: 0,
            work: VecDeque::new(),
            target: None,
        }),
        immortal,
    )
}
fn main_queue(env: &mut Environment) -> Handle {
    if let Some(main) = state(env).main {
        return main;
    }
    let main = make_queue(env, b"com.apple.main-thread", false, true, true);
    state(env).main = Some(main);
    main
}
fn concurrent_attribute(env: &mut Environment) -> Handle {
    if let Some(attr) = state(env).concurrent_attribute {
        return attr;
    }
    let attr = env.mem.alloc_and_write(0x434f4e43u32).cast_void();
    state(env).concurrent_attribute = Some(attr);
    attr
}
fn dispatch_queue_create(env: &mut Environment, label: ConstPtr<u8>, attr: Handle) -> Handle {
    let label = if label.is_null() {
        Vec::new()
    } else {
        env.mem.cstr_at(label).to_vec()
    };
    let concurrent = !attr.is_null();
    if concurrent {
        assert_eq!(
            attr,
            concurrent_attribute(env),
            "Unsupported dispatch queue attribute"
        );
    }
    let handle = make_queue(env, &label, concurrent, false, false);
    log!(
        "Created {} dispatch queue {:?}",
        if concurrent { "concurrent" } else { "serial" },
        handle
    );
    handle
}
fn dispatch_get_main_queue(env: &mut Environment) -> Handle {
    main_queue(env)
}
pub(crate) fn dispatch_get_global_queue(
    env: &mut Environment,
    priority: i32,
    flags: u32,
) -> Handle {
    assert_eq!(flags, 0);
    assert!(
        matches!(priority, -32768 | -2 | 0 | 2),
        "Unsupported dispatch priority"
    );
    if let Some(q) = state(env).globals.get(&priority) {
        return *q;
    }
    let q = make_queue(
        env,
        format!("com.apple.root.{priority}").as_bytes(),
        true,
        false,
        true,
    );
    state(env).globals.insert(priority, q);
    q
}
fn dispatch_queue_get_label(env: &mut Environment, handle: Handle) -> ConstPtr<u8> {
    let handle = if handle.is_null() {
        current_queue(env)
    } else {
        handle
    };
    queue(env, handle).label.cast_const()
}
fn current_queue(env: &mut Environment) -> Handle {
    let thread = env.current_thread;
    if let Some(q) = state(env)
        .executing
        .get(&thread)
        .and_then(|stack| stack.last())
    {
        return *q;
    }
    if thread == 0 {
        main_queue(env)
    } else {
        dispatch_get_global_queue(env, 0, 0)
    }
}
fn dispatch_get_current_queue(env: &mut Environment) -> Handle {
    current_queue(env)
}
pub fn dispatch_retain(env: &mut Environment, handle: Handle) {
    let object = state(env)
        .objects
        .get_mut(&handle)
        .expect("Retain of invalid dispatch object");
    if let Some(count) = object.references.as_mut() {
        *count = count.checked_add(1).unwrap();
    }
}
pub fn dispatch_release(env: &mut Environment, handle: Handle) {
    let object = state(env)
        .objects
        .get_mut(&handle)
        .expect("Release of invalid dispatch object");
    let Some(count) = object.references.as_mut() else {
        return;
    };
    *count = count.checked_sub(1).unwrap();
    if *count != 0 {
        return;
    }
    let object = state(env).objects.remove(&handle).unwrap();
    match object.kind {
        Kind::Queue(q) => {
            assert!(q.work.is_empty() && q.active == 0);
            env.mem.free(q.label.cast());
            if let Some(parent) = q.target {
                dispatch_release(env, parent);
            }
        }
        Kind::Group(g) => assert!(g.count == 0 && g.notifications.is_empty()),
        Kind::Semaphore(s) => assert!(s.waiting.is_empty() && s.granted.is_empty()),
    }
    env.mem.free(handle);
}
pub fn message(env: &mut Environment, receiver: id, selector: SEL) -> bool {
    // Most Objective-C messages target ordinary game/framework objects.
    // Reject those by isa before hashing into the dispatch-object registry.
    let Some(class) = env.libc_state.dispatch.object_class else {
        return false;
    };
    if receiver == crate::objc::nil {
        return false;
    }
    let receiver_class: crate::objc::Class = env.mem.read(receiver.cast());
    if receiver_class != class {
        return false;
    }
    let handle = receiver.cast_void();
    if !env.libc_state.dispatch.objects.contains_key(&handle) {
        return false;
    }
    let name = selector.as_str(&env.mem).to_owned();
    let result = match name.as_str() {
        "retain" => {
            dispatch_retain(env, handle);
            handle.to_bits()
        }
        "release" => {
            dispatch_release(env, handle);
            0
        }
        "autorelease" => {
            let (): () = msg_class![env; NSAutoreleasePool addObject:receiver];
            handle.to_bits()
        }
        "class" => env.mem.read(handle.cast::<id>()).to_bits(),
        "hash" => handle.to_bits(),
        "isEqual:" => u32::from(env.cpu.regs()[2] == handle.to_bits()),
        _ => panic!("Unsupported dispatch-object message {name}"),
    };
    env.cpu.regs_mut()[0] = result;
    true
}

fn worker_function(env: &mut Environment, timer: bool) -> GuestFunction {
    if let Some(f) = if timer {
        state(env).timer
    } else {
        state(env).worker
    } {
        return f;
    }
    let (name, f) = if timer {
        export_c_func!(timer_worker(_))
    } else {
        export_c_func!(queue_worker(_))
    };
    let f = env.dyld.create_guest_function(&mut env.mem, name, f);
    if timer {
        state(env).timer = Some(f);
    } else {
        state(env).worker = Some(f);
    }
    f
}
fn enqueue(env: &mut Environment, handle: Handle, work: Work) {
    let q = queue(env, handle);
    q.work.push_back(work);
    if q.main || (!q.concurrent && q.active > 0) {
        return;
    }
    q.active += 1;
    dispatch_retain(env, handle); // worker owns queue through callback/target waits
    let f = worker_function(env, false);
    env.new_thread(
        f,
        handle,
        crate::mem::Mem::SECONDARY_THREAD_DEFAULT_STACK_SIZE,
    );
}
fn run_work(env: &mut Environment, work: Work) {
    let thread = env.current_thread;
    state(env)
        .executing
        .entry(thread)
        .or_default()
        .push(work.logical_queue);
    let pool: id = msg_class![env; NSAutoreleasePool new];
    blocks::invoke(env, work.block);
    if work.owned {
        blocks::_Block_release(env, work.block);
    }
    objc_release(env, pool);
    assert_eq!(
        state(env).executing.get_mut(&thread).unwrap().pop(),
        Some(work.logical_queue)
    );
    if state(env).executing[&thread].is_empty() {
        state(env).executing.remove(&thread);
    }
    if let Some(g) = work.group {
        dispatch_group_leave(env, g);
        dispatch_release(env, g);
    }
    if let Some(done) = work.done {
        done.set(true);
    }
}
fn wait_edge(env: &mut Environment, from: Handle, to: Handle) -> Option<Handle> {
    let mut cursor = to;
    let mut visited = HashSet::new();
    while visited.insert(cursor) {
        assert_ne!(cursor, from, "dispatch_sync serial queue dependency cycle");
        match state(env).waits.get(&cursor) {
            Some(next) => cursor = *next,
            None => break,
        }
    }
    state(env).waits.insert(from, to)
}
fn restore_wait(env: &mut Environment, from: Handle, previous: Option<Handle>) {
    if let Some(previous) = previous {
        state(env).waits.insert(from, previous);
    } else {
        state(env).waits.remove(&from);
    }
}
fn queue_worker(env: &mut Environment, handle: Handle) -> Handle {
    loop {
        let Some(mut work) = queue(env, handle).work.pop_front() else {
            break;
        };
        if let Some(target) = queue(env, handle).target {
            let done = work
                .done
                .get_or_insert_with(|| Rc::new(Cell::new(false)))
                .clone();
            let serial = !queue(env, handle).concurrent;
            let previous = if serial {
                wait_edge(env, handle, target)
            } else {
                None
            };
            enqueue(env, target, work);
            while !done.get() {
                env.sleep(Duration::from_millis(1));
            }
            if serial {
                restore_wait(env, handle, previous);
            }
        } else {
            run_work(env, work);
        }
        if queue(env, handle).concurrent {
            break;
        }
    }
    queue(env, handle).active -= 1;
    dispatch_release(env, handle);
    Handle::null()
}
pub fn drain_main(env: &mut Environment) {
    assert_eq!(env.current_thread, 0);
    if state(env).draining_main {
        return;
    }
    let Some(main) = state(env).main else {
        return;
    };
    let count = queue(env, main).work.len().min(64);
    state(env).draining_main = true;
    for _ in 0..count {
        let work = queue(env, main).work.pop_front().unwrap();
        run_work(env, work);
    }
    state(env).draining_main = false;
}
fn owned_work(env: &mut Environment, q: Handle, block: Handle) -> Work {
    assert!(!block.is_null());
    let _ = queue(env, q);
    Work {
        block: blocks::_Block_copy(env, block),
        owned: true,
        logical_queue: q,
        group: None,
        done: None,
    }
}
fn dispatch_async(env: &mut Environment, q: Handle, block: Handle) {
    let work = owned_work(env, q, block);
    enqueue(env, q, work);
}
fn target_contains(env: &mut Environment, start: Handle, wanted: Handle) -> bool {
    let mut cursor = Some(start);
    let mut visited = HashSet::new();
    while let Some(q) = cursor {
        assert!(visited.insert(q), "Dispatch target cycle");
        if q == wanted {
            return true;
        }
        cursor = queue(env, q).target;
    }
    false
}
fn dispatch_set_target_queue(env: &mut Environment, q: Handle, target: Handle) {
    assert_eq!(
        queue(env, q).active,
        0,
        "Retargeting active dispatch queues is not implemented"
    );
    assert!(
        state(env).objects.get(&q).unwrap().references.is_some(),
        "Cannot retarget a global/main queue"
    );
    if target.is_null() {
        if let Some(old) = queue(env, q).target.take() {
            dispatch_release(env, old);
        }
        return;
    }
    assert!(!target_contains(env, target, q), "Dispatch target cycle");
    dispatch_retain(env, target);
    if let Some(old) = queue(env, q).target.replace(target) {
        dispatch_release(env, old);
    }
}
fn dispatch_sync(env: &mut Environment, q: Handle, block: Handle) {
    let current = current_queue(env);
    if !queue(env, q).concurrent {
        assert_ne!(
            current, q,
            "dispatch_sync to the current serial queue would deadlock"
        );
        assert!(
            !target_contains(env, current, q),
            "dispatch_sync to the executing serial target would deadlock"
        );
    }
    if env.current_thread == 0 {
        let main = main_queue(env);
        assert!(
            !target_contains(env, q, main),
            "dispatch_sync waits for the blocked main thread"
        );
    }
    // Include the serial gates of target queues. A logical child can run on
    // another serial queue, or hold its own serial gate on a concurrent target.
    let mut cursor = Some(current);
    let mut source = None;
    while let Some(held) = cursor {
        if !queue(env, held).concurrent {
            assert!(
                !target_contains(env, q, held),
                "dispatch_sync waits for an executing serial ancestor"
            );
            source = Some(held);
        }
        cursor = queue(env, held).target;
    }
    let previous = source.and_then(|source| wait_edge(env, source, q));
    let done = Rc::new(Cell::new(false));
    enqueue(
        env,
        q,
        Work {
            block,
            owned: false,
            logical_queue: q,
            group: None,
            done: Some(done.clone()),
        },
    );
    while !done.get() {
        env.sleep(Duration::from_millis(1));
    }
    if let Some(source) = source {
        restore_wait(env, source, previous);
    }
}
fn dispatch_once(env: &mut Environment, predicate: MutPtr<i32>, block: Handle) {
    loop {
        match env.mem.read(predicate) {
            -1 => return,
            0 => {
                let thread = env.current_thread;
                env.mem.write(predicate, 1);
                state(env).once_owners.insert(predicate, thread);
                blocks::invoke(env, block);
                env.mem.write(predicate, -1);
                state(env).once_owners.remove(&predicate);
                return;
            }
            1 => {
                let owner = state(env)
                    .once_owners
                    .get(&predicate)
                    .copied()
                    .expect("Unknown dispatch_once predicate");
                assert_ne!(
                    owner, env.current_thread,
                    "Recursive dispatch_once would deadlock"
                );
                env.sleep(Duration::from_millis(1));
            }
            _ => panic!("Invalid dispatch_once predicate"),
        }
    }
}
fn monotonic(env: &Environment) -> u64 {
    u64::try_from(env.startup_time.elapsed().as_nanos())
        .unwrap()
        .max(1)
}
fn wall_now() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i128
}
fn dispatch_time(env: &mut Environment, when: u64, delta: i64) -> u64 {
    if when == FOREVER {
        return FOREVER;
    }
    if (when as i64) < 0 {
        let time = -i128::from(when as i64) + i128::from(delta);
        if time > i128::from(i64::MAX) {
            FOREVER
        } else if time <= 0 {
            1
        } else {
            (-time as i64) as u64
        }
    } else {
        let time = i128::from(if when == 0 { monotonic(env) } else { when }) + i128::from(delta);
        if time > i128::from(i64::MAX) {
            FOREVER
        } else {
            time.max(1) as u64
        }
    }
}
fn remaining(env: &Environment, deadline: u64) -> Option<Duration> {
    if deadline == FOREVER {
        return None;
    }
    let nanos = if (deadline as i64) < 0 {
        -i128::from(deadline as i64) - wall_now()
    } else {
        i128::from(deadline) - i128::from(monotonic(env))
    };
    Some(Duration::from_nanos(nanos.max(0).try_into().unwrap()))
}
fn dispatch_after(env: &mut Environment, deadline: u64, q: Handle, block: Handle) {
    let work = owned_work(env, q, block);
    dispatch_retain(env, q);
    let context = env.mem.alloc_and_write(0u32).cast_void();
    state(env).delayed.insert(context, (deadline, q, work));
    if deadline != FOREVER {
        let f = worker_function(env, true);
        env.new_thread(
            f,
            context,
            crate::mem::Mem::SECONDARY_THREAD_DEFAULT_STACK_SIZE,
        );
    }
}
fn timer_worker(env: &mut Environment, context: Handle) -> Handle {
    let (deadline, q, work) = state(env).delayed.remove(&context).unwrap();
    env.mem.free(context);
    while let Some(delay) = remaining(env, deadline) {
        if delay.is_zero() {
            break;
        }
        env.sleep(delay);
    }
    enqueue(env, q, work);
    dispatch_release(env, q);
    Handle::null()
}
pub(crate) fn dispatch_group_create(env: &mut Environment) -> Handle {
    object(env, Kind::Group(Group::default()), false)
}
fn dispatch_group_enter(env: &mut Environment, g: Handle) {
    if group(env, g).count == 0 {
        dispatch_retain(env, g);
    }
    group(env, g).count = group(env, g).count.checked_add(1).unwrap();
}
fn dispatch_group_leave(env: &mut Environment, g: Handle) {
    let group = group(env, g);
    group.count = group
        .count
        .checked_sub(1)
        .expect("Unbalanced dispatch_group_leave");
    if group.count != 0 {
        return;
    }
    let notifications = std::mem::take(&mut group.notifications);
    for (q, work) in notifications {
        enqueue(env, q, work);
        dispatch_release(env, q);
    }
    dispatch_release(env, g);
}
pub(crate) fn dispatch_group_async(env: &mut Environment, g: Handle, q: Handle, block: Handle) {
    let mut work = owned_work(env, q, block);
    dispatch_group_enter(env, g);
    dispatch_retain(env, g);
    work.group = Some(g);
    enqueue(env, q, work);
}
fn dispatch_group_notify(env: &mut Environment, g: Handle, q: Handle, block: Handle) {
    let work = owned_work(env, q, block);
    if group(env, g).count == 0 {
        enqueue(env, q, work);
    } else {
        dispatch_retain(env, q);
        group(env, g).notifications.push((q, work));
    }
}
pub(crate) fn dispatch_group_wait(env: &mut Environment, g: Handle, deadline: u64) -> i32 {
    dispatch_retain(env, g);
    let result = loop {
        if group(env, g).count == 0 {
            break 0;
        }
        if remaining(env, deadline).is_some_and(|time| time.is_zero()) {
            break 1;
        }
        env.sleep(Duration::from_millis(1));
    };
    dispatch_release(env, g);
    result
}
fn dispatch_semaphore_create(env: &mut Environment, value: i32) -> Handle {
    if value < 0 {
        return Handle::null();
    }
    object(
        env,
        Kind::Semaphore(Semaphore {
            value: value.into(),
            waiting: VecDeque::new(),
            granted: HashSet::new(),
        }),
        false,
    )
}
fn dispatch_semaphore_signal(env: &mut Environment, s: Handle) -> i32 {
    let sem = semaphore(env, s);
    if let Some(thread) = sem.waiting.pop_front() {
        assert!(sem.granted.insert(thread));
        1
    } else {
        sem.value = sem.value.checked_add(1).unwrap();
        0
    }
}
fn dispatch_semaphore_wait(env: &mut Environment, s: Handle, deadline: u64) -> i32 {
    let thread = env.current_thread;
    if semaphore(env, s).value > 0 {
        semaphore(env, s).value -= 1;
        return 0;
    }
    if remaining(env, deadline).is_some_and(|time| time.is_zero()) {
        return 1;
    }
    dispatch_retain(env, s);
    assert!(!semaphore(env, s).waiting.contains(&thread));
    semaphore(env, s).waiting.push_back(thread);
    let result = loop {
        if semaphore(env, s).granted.remove(&thread) {
            break 0;
        }
        if remaining(env, deadline).is_some_and(|time| time.is_zero()) {
            semaphore(env, s).waiting.retain(|t| *t != thread);
            break 1;
        }
        env.sleep(Duration::from_millis(1));
    };
    dispatch_release(env, s);
    result
}
pub const CONSTANTS: ConstantExports = &[
    (
        "__dispatch_main_q",
        HostConstant::Custom(|env| main_queue(env).cast_const()),
    ),
    (
        "__dispatch_queue_attr_concurrent",
        HostConstant::Custom(|env| concurrent_attribute(env).cast_const()),
    ),
];
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(dispatch_queue_create(_, _)),
    export_c_func!(dispatch_get_main_queue()),
    export_c_func!(dispatch_get_global_queue(_, _)),
    export_c_func!(dispatch_get_current_queue()),
    export_c_func!(dispatch_queue_get_label(_)),
    export_c_func!(dispatch_set_target_queue(_, _)),
    export_c_func!(dispatch_retain(_)),
    export_c_func!(dispatch_release(_)),
    export_c_func!(dispatch_async(_, _)),
    export_c_func!(dispatch_sync(_, _)),
    export_c_func!(dispatch_once(_, _)),
    export_c_func!(dispatch_time(_, _)),
    export_c_func!(dispatch_after(_, _, _)),
    export_c_func!(dispatch_group_create()),
    export_c_func!(dispatch_group_enter(_)),
    export_c_func!(dispatch_group_leave(_)),
    export_c_func!(dispatch_group_async(_, _, _)),
    export_c_func!(dispatch_group_notify(_, _, _)),
    export_c_func!(dispatch_group_wait(_, _)),
    export_c_func!(dispatch_semaphore_create(_)),
    export_c_func!(dispatch_semaphore_signal(_)),
    export_c_func!(dispatch_semaphore_wait(_, _)),
];
pub fn framework_probe() -> Result<(), String> {
    probe::run()
}
