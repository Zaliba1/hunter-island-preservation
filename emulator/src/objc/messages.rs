/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C messaging (`objc_msgSend` and friends).
//!
//! Resources:
//! - Apple's [Objective-C Runtime Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjCRuntimeGuide/Articles/ocrtHowMessagingWorks.html)
//! - [Apple's documentation of `objc_msgSend`](https://developer.apple.com/documentation/objectivec/1456712-objc_msgsend)
//! - Mike Ash's [objc_msgSend's New Prototype](https://www.mikeash.com/pyblog/objc_msgsends-new-prototype.html)
//! - Peter Steinberger's [Calling Super at Runtime in Swift](https://steipete.com/posts/calling-super-at-runtime/) explains `objc_msgSendSuper2`

use super::{id, nil, Class, ObjC, IMP, SEL};
use crate::abi::{CallFromHost, GuestRet};
use crate::environment::ThreadId;
use crate::libc::pthread::cond::{
    pthread_cond_broadcast, pthread_cond_destroy, pthread_cond_init, pthread_cond_t,
    pthread_cond_wait,
};
use crate::libc::pthread::mutex::{
    pthread_mutex_destroy, pthread_mutex_init, pthread_mutex_lock, pthread_mutex_t,
    pthread_mutex_unlock,
};
use crate::mem::{guest_size_of, ConstPtr, MutPtr, MutVoidPtr, SafeRead};
use crate::objc::classes::InitializationStatus;
use crate::Environment;
use std::any::TypeId;

pub(super) struct ThreadInitializer {
    mutex: MutPtr<pthread_mutex_t>,
    cond: MutPtr<pthread_cond_t>,
    tid: ThreadId,
    waiters: u32,
}

fn maybe_initialize_class(env: &mut Environment, receiver: id) {
    let class_host_object = env.objc.get_host_object(receiver).unwrap();
    let Some(&super::ClassHostObject {
        superclass,
        is_metaclass,
        is_initialized,
        ..
    }) = class_host_object.as_any().downcast_ref()
    else {
        // If it's here, there's one of two cases:
        //
        // 1: The receiver is an instance. The class should then have already
        // called +initialize since you need to call +alloc to create an
        // instance (this also needs to be upheld for instances created with
        // class_createInstance(), whenever we implement that)
        //
        // 2: The reciever is a fake/unimplemented class. There's no reason to
        // send +initialize to those, so we don't bother.
        return;
    };

    if is_metaclass || is_initialized == InitializationStatus::Initialized {
        // On the offchance that this is a metaclass, we don't need to send
        // +initialize to it. We also don't need to send it if the class is
        // already initialized.
        return;
    }

    // This class is not initialized, but there might be classes above it in the
    // hierarchy that also need to be checked, so check those first.
    if !superclass.is_null() {
        maybe_initialize_class(env, superclass);
    }

    if is_initialized == InitializationStatus::Initializing {
        env.objc
            .initializer_threads
            .get_mut(&receiver)
            .unwrap()
            .waiters += 1;
        let ThreadInitializer {
            mutex, cond, tid, ..
        } = *env.objc.initializer_threads.get(&receiver).unwrap();

        // The current thread is already initializing, so let it call other
        // messages while it does so.
        if tid == env.current_thread {
            return;
        }

        // We are waiting for another thread to initialize, wait for it to
        // broadcast that it has finished.
        pthread_mutex_lock(env, mutex);
        loop {
            let class_host_object = env.objc.get_host_object(receiver).unwrap();
            let &super::ClassHostObject { is_initialized, .. } =
                class_host_object.as_any().downcast_ref().unwrap();
            if is_initialized == InitializationStatus::Initialized {
                break;
            }
            pthread_cond_wait(env, cond, mutex);
        }
        pthread_mutex_unlock(env, mutex);

        let ThreadInitializer {
            ref mut waiters, ..
        } = *env.objc.initializer_threads.get_mut(&receiver).unwrap();
        *waiters -= 1;
        if *waiters == 0 {
            // We're the last waiter for this initialize, so clean up state on
            // the way out.
            pthread_cond_destroy(env, cond);
            pthread_mutex_destroy(env, mutex);
            env.objc.initializer_threads.remove(&receiver);
        }
    } else {
        log_dbg!(
            "Initializing {:?} on thread {}",
            env.objc.try_get_class_name(receiver),
            env.current_thread
        );
        let regs = *env.cpu.regs();

        let mutex = env.mem.alloc(guest_size_of::<pthread_mutex_t>()).cast();
        let cond = env.mem.alloc(guest_size_of::<pthread_cond_t>()).cast();
        pthread_mutex_init(env, mutex, ConstPtr::null());
        pthread_cond_init(env, cond, ConstPtr::null());
        env.objc.initializer_threads.insert(
            receiver,
            ThreadInitializer {
                mutex,
                cond,
                tid: env.current_thread,
                waiters: 0,
            },
        );

        let super::ClassHostObject { is_initialized, .. } = env.objc.borrow_mut(receiver);
        *is_initialized = InitializationStatus::Initializing;
        () = msg![env; receiver initialize];
        let super::ClassHostObject { is_initialized, .. } = env.objc.borrow_mut(receiver);
        *is_initialized = InitializationStatus::Initialized;
        env.cpu.regs_mut().copy_from_slice(&regs);
        log_dbg!(
            "Done initializing {:?} on thread {}",
            env.objc.try_get_class_name(receiver),
            env.current_thread
        );
        if env.objc.initializer_threads.get(&receiver).unwrap().waiters == 0 {
            // Nobody ended up waiting for this initializer, so we can just
            // destroy it.
            pthread_cond_destroy(env, cond);
            pthread_mutex_destroy(env, mutex);
            env.objc.initializer_threads.remove(&receiver);
        } else {
            pthread_mutex_lock(env, mutex);
            pthread_cond_broadcast(env, cond);
            pthread_mutex_unlock(env, mutex);
        }
    }
}

fn call_implementation(
    env: &mut Environment,
    receiver: id,
    selector: SEL,
    imp: IMP,
    message_type_info: Option<(TypeId, &'static str)>,
    tolerate_type_mismatch: bool,
) {
    match imp {
        IMP::Host(host_imp) => {
            // Cache hits must retain the exact same host ABI/type checks as
            // hierarchy lookups. Struct returns still use the original caller.
            if let Some((sent_type_id, sent_type_desc)) = message_type_info {
                let (expected_type_id, expected_type_desc) = host_imp.type_info();
                if sent_type_id != expected_type_id {
                    let msg = format!(
                        "\
Type mismatch when sending message {} to {:?}!
- Message has type: {:?} / {}
- Method expects type: {:?} / {}",
                        selector.as_str(&env.mem),
                        receiver,
                        sent_type_id,
                        sent_type_desc,
                        expected_type_id,
                        expected_type_desc
                    );
                    if tolerate_type_mismatch {
                        log!("Warning: {}", msg);
                    } else {
                        panic!("{}", msg);
                    }
                }
            }
            host_imp.call_from_guest(env)
        }
        // Keep stack arguments in place, including on a cached guest IMP.
        IMP::Guest(guest_imp) => {
            if (env.options.hunter_rare_dragons || env.options.hunter_test_orbling)
                && super::hunter_rare_dragons::try_draw(env, receiver, guest_imp)
            {
                return;
            }
            if env.options.hunter_silver_exchange
                && super::hunter_silver_exchange::register_menu_touch(env, receiver, guest_imp)
            {
                return;
            }
            if !super::leaf_getter::try_call(env, guest_imp) {
                guest_imp.call_without_pushing_stack_frame(env);
                if env.options.hunter_no_arena_cooldown
                    && matches!(guest_imp.addr_with_thumb_bit(), 0xdbec1 | 0xdbfe1)
                    && env.bundle.bundle_identifier() == "pockettrend.hunterisland"
                    && env.bundle.bundle_version() == "2.02"
                    && matches!(
                        selector.as_str(&env.mem),
                        "beginTournamentBattle" | "timeToNextTournament"
                    )
                {
                    // Hunter Island 2.02 returns the arena wait as a soft-float
                    // double in r0/r1. Preserve the guest's other side effects
                    // and let its original round, unlock and reward logic run.
                    let regs = env.cpu.regs_mut();
                    let wait = f64::from_bits(u64::from(regs[0]) | (u64::from(regs[1]) << 32));
                    if wait > 0.0 {
                        regs[..2].fill(0);
                        if env.options.capture_frames && guest_imp.addr_with_thumb_bit() == 0xdbec1 {
                            log!("Hunter Island arena cooldown skipped: {wait:.0} seconds");
                        }
                    }
                }
                if env.options.hunter_silver_exchange {
                    super::hunter_silver_exchange::after_guest_call(env, receiver, guest_imp);
                }
            }
        }
    }
}

/// The core implementation of `objc_msgSend`, the main function of Objective-C.
///
/// Note that while only two parameters (usually receiver and selector) are
/// defined by the wrappers over this function, a call to an `objc_msgSend`
/// variant may have additional arguments to be forwarded (or rather, left
/// untouched) by `objc_msgSend` when it tail-calls the method implementation it
/// looks up. This is invisible to the Rust type system; we're relying on
/// [crate::abi::CallFromGuest] here.
///
/// Similarly, the return value of `objc_msgSend` is whatever value is returned
/// by the method implementation. We are relying on CallFromGuest not
/// overwriting it.
#[allow(non_snake_case)]
fn objc_msgSend_inner(
    env: &mut Environment,
    receiver: id,
    selector: SEL,
    super2: Option<Class>,
    tolerate_type_mismatch: bool,
    skip_initialize: bool,
) {
    log_dbg!(
        "Dispatching {} for {:?}",
        selector.as_str(&env.mem),
        receiver
    );
    let message_type_info = env.objc.message_type_info.take();

    if receiver == nil {
        // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjectiveC/Chapters/ocObjectsClasses.html#//apple_ref/doc/uid/TP30001163-CH11-SW7
        log_dbg!("[nil {}]", selector.as_str(&env.mem));
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let orig_class = super2.unwrap_or_else(|| ObjC::read_isa(receiver, &env.mem));
    if crate::libc::blocks::message(env, receiver, selector) {
        return;
    }
    if crate::libc::dispatch::message(env, receiver, selector) {
        return;
    }
    if orig_class == nil {
        env.stack_trace_current();
        panic!("Invalid Objective-C class: receiver={receiver:?} selector={} super2={super2:?} registered={} guest_lr={:#x}",
            selector.as_str(&env.mem),env.objc.get_host_object(receiver).is_some(),env.cpu.regs()[crate::cpu::Cpu::LR]);
    }
    let Some(receiver_host_object) = env.objc.get_host_object(receiver) else {
        env.stack_trace_current();
        panic!("Unregistered Objective-C receiver={receiver:?} class={orig_class:?} selector={} guest_lr={:#x}", selector.as_str(&env.mem), env.cpu.regs()[crate::cpu::Cpu::LR]);
    };
    // maybe_initialize_class only acts on ClassHostObject. Use the validation
    // lookup above to avoid hashing an ordinary receiver a second time.
    let receiver_is_class = receiver_host_object.as_any().is::<super::ClassHostObject>();
    if !skip_initialize && receiver_is_class {
        maybe_initialize_class(env, receiver);
    }

    if let Some(imp) = env
        .objc
        .method_cache
        .get(orig_class, selector, super2.is_some())
    {
        call_implementation(
            env,
            receiver,
            selector,
            imp,
            message_type_info,
            tolerate_type_mismatch,
        );
        return;
    }

    // Traverse the chain of superclasses to find the method implementation.

    let mut class = orig_class;
    loop {
        if class == nil {
            assert!(class != orig_class);

            // Keep the guest call site: the Rust backtrace alone cannot show
            // which application/framework path sent the missing message.
            env.stack_trace_current();

            let class_host_object = env.objc.get_host_object(orig_class).unwrap();
            let &super::ClassHostObject {
                ref name,
                is_metaclass,
                ..
            } = class_host_object.as_any().downcast_ref().unwrap();

            panic!(
                "{} {:?} ({}class \"{}\", {:?}){} does not respond to selector \"{}\"!",
                if is_metaclass { "Class" } else { "Object" },
                receiver,
                if is_metaclass { "meta" } else { "" },
                name,
                orig_class,
                if super2.is_some() {
                    "'s superclass"
                } else {
                    ""
                },
                selector.as_str(&env.mem),
            );
        }

        let host_object = env.objc.get_host_object(class).unwrap();

        if let Some(&super::ClassHostObject {
            superclass,
            ref methods,
            ref name,
            ..
        }) = host_object.as_any().downcast_ref()
        {
            // Skip method lookup on first iteration if this is the super-call
            // variant of objc_msgSend (look up the superclass first)
            if super2.is_some() && class == orig_class {
                class = superclass;
                continue;
            }

            if let Some(imp) = methods.get(&selector) {
                log_dbg!("Found method on: {}", name);
                let imp = match imp {
                    IMP::Host(host) => IMP::Host(*host),
                    IMP::Guest(guest) => IMP::Guest(*guest),
                };
                env.objc
                    .method_cache
                    .insert(orig_class, selector, super2.is_some(), &imp);
                call_implementation(
                    env,
                    receiver,
                    selector,
                    imp,
                    message_type_info,
                    tolerate_type_mismatch,
                );
                return;
            } else {
                class = superclass;
            }
        } else if let Some(&super::UnimplementedClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {
            panic!(
                "Class \"{}\" ({:?}) is unimplemented. Call to {} method \"{}\".",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem),
            );
        } else if let Some(&super::FakeClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {
            log!(
                "Call to faked class \"{}\" ({:?}) {} method \"{}\". Behaving as if message was sent to nil.",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem),
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        } else {
            panic!(
                "Item {class:?} in superclass chain of object {receiver:?}'s class {orig_class:?} has an unexpected host object type."
            );
        }
    }
}

/// Standard variant of `objc_msgSend`. See [objc_msgSend_inner].
#[allow(non_snake_case)]
pub(crate) fn objc_msgSend(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ false,
    )
}

#[allow(non_snake_case)]
pub(crate) fn _touchHLE_objc_msgSend_tolerant(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ true,
        /* skip_initialize: */ false,
    )
}

/// Variant of `objc_msgSend` that does not trigger `+initialize`.
#[allow(non_snake_case)]
pub(crate) fn _touchHLE_objc_msgSend_no_initialize(
    env: &mut Environment,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ true,
    )
}

/// Variant of `objc_msgSend` for methods that return a struct via a pointer.
/// See [objc_msgSend_inner].
///
/// The first parameter here is the pointer for the struct return. This is an
/// ABI detail that is usually hidden and handled behind-the-scenes by
/// [crate::abi], but `objc_msgSend` is a special case because of the
/// pass-through behaviour. Of course, the pass-through only works if the [IMP]
/// also has the pointer parameter. The caller therefore has to pick the
/// appropriate `objc_msgSend` variant depending on the method it wants to call.
pub(super) fn objc_msgSend_stret(
    env: &mut Environment,
    _stret: MutVoidPtr,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ false,
    )
}

#[allow(non_snake_case)]
pub(crate) fn _touchHLE_objc_msgSend_stret_tolerant(
    env: &mut Environment,
    _stret: MutVoidPtr,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ true,
        /* skip_initialize: */ false,
    )
}

#[repr(C, packed)]
/// A pointer to this struct replaces the normal receiver parameter for
/// `objc_msgSendSuper2` and [msg_send_super2].
pub struct objc_super {
    pub receiver: id,
    /// If this is used with `objc_msgSendSuper` (not implemented here, TODO),
    /// this is a pointer to the superclass to look up the method on.
    /// If this is used with `objc_msgSendSuper2`, this is a pointer to a class
    /// and the superclass will be looked up from it.
    pub class: Class,
}
unsafe impl SafeRead for objc_super {}

/// Variant of `objc_msgSend` for supercalls. See [objc_msgSend_inner].
///
/// This variant has a weird ABI because it needs to receive an additional piece
/// of information (a class pointer), but it can't actually take this as an
/// extra parameter, because that would take one of the argument slots reserved
/// for arguments passed onto the method implementation. Hence the [objc_super]
/// pointer in place of the normal [id].
#[allow(non_snake_case)]
pub(super) fn objc_msgSendSuper2(
    env: &mut Environment,
    super_ptr: ConstPtr<objc_super>,
    selector: SEL,
) {
    let objc_super { receiver, class } = env.mem.read(super_ptr);

    // Rewrite first argument to match the normal ABI.
    crate::abi::write_next_arg(&mut 0, env.cpu.regs_mut(), &mut env.mem, receiver);

    objc_msgSend_inner(
        env,
        receiver,
        selector,
        /* super2: */ Some(class),
        /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ false,
    )
}

/// Trait that assists with type-checking of [msg_send]'s arguments.
///
/// - Statically constrains the types of [msg_send]'s arguments so that the
///   first two are always [id] and [SEL].
/// - Provides the type ID to enable dynamic type checking of subsequent
///   arguments and the return type.
///
/// See `impl_HostIMP` for implementations. See also [MsgSendSuperSignature].
pub trait MsgSendSignature: 'static {
    /// Get the [TypeId] and a human-readable description for this signature.
    fn type_info() -> (TypeId, &'static str) {
        #[cfg(debug_assertions)]
        let type_name = std::any::type_name::<Self>();
        // Avoid wasting space on type names in release builds. At the time of
        // writing this saves about 36KB.
        #[cfg(not(debug_assertions))]
        let type_name = "[description unavailable in release builds]";
        (TypeId::of::<Self>(), type_name)
    }
}

/// Wrapper around [objc_msgSend] which, together with [msg], makes it easy to
/// send messages in host code. Warning: all types are inferred from the
/// call-site and they may not be checked, so be very sure you get them correct!
pub fn msg_send<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSignature>::type_info());
    if R::SIZE_IN_MEM.is_some() {
        (objc_msgSend_stret as fn(&mut Environment, MutVoidPtr, id, SEL)).call_from_host(env, args)
    } else {
        (objc_msgSend as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

pub fn msg_send_no_type_checking<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    if R::SIZE_IN_MEM.is_some() {
        (_touchHLE_objc_msgSend_stret_tolerant as fn(&mut Environment, MutVoidPtr, id, SEL))
            .call_from_host(env, args)
    } else {
        (_touchHLE_objc_msgSend_tolerant as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

/// Variant of [msg_send] which does not trigger `+initialize` on the receiver.
///
/// This is meant for sending `+load`: the Objective-C runtime guarantees that
/// `+load` runs before `+initialize`, so it must not go through the normal
/// [msg_send] path (which would call `maybe_initialize_class` first).
pub fn msg_send_no_initialize<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    assert!(
        R::SIZE_IN_MEM.is_none(),
        "msg_send_no_initialize does not support struct returns"
    );
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSignature>::type_info());
    (_touchHLE_objc_msgSend_no_initialize as fn(&mut Environment, id, SEL))
        .call_from_host(env, args)
}

/// Counterpart of [MsgSendSignature] for [msg_send_super2].
pub trait MsgSendSuperSignature: 'static {
    /// Signature with the [objc_super] pointer replaced by [id].
    type WithoutSuper: MsgSendSignature;
}

/// [msg_send] but for super-calls (calls [objc_msgSendSuper2]). You probably
/// want to use [msg_super] rather than calling this directly.
pub fn msg_send_super2<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSuperSignature,
    R: GuestRet,
{
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSuperSignature>::WithoutSuper::type_info());
    if R::SIZE_IN_MEM.is_some() {
        todo!() // no stret yet
    } else {
        (objc_msgSendSuper2 as fn(&mut Environment, ConstPtr<objc_super>, SEL))
            .call_from_host(env, args)
    }
}

/// Macro for sending a message which imitates the Objective-C messaging syntax.
/// See [msg_send] for the underlying implementation. Warning: all types are
/// inferred from the call-site and they may not be checked, so be very sure you
/// get them correct!
///
/// ```ignore
/// msg![env; foo setBar:bar withQux:qux];
/// ```
///
/// desugars to:
///
/// ```ignore
/// {
///     let sel = env.objc.lookup_selector("setFoo:withBar").unwrap();
///     msg_send(env, (foo, sel, bar, qux))
/// }
/// ```
///
/// Note that argument values that aren't a bare single identifier like `foo`
/// need to be bracketed.
///
/// See also [msg_class], if you want to send a message to a class.
#[macro_export]
macro_rules! msg {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel)
                .expect("Unknown selector");
            let args = ($receiver, sel, $($arg1, $($argn),*)?);
            $crate::objc::msg_send($env, args)
        }
    }
}
pub use crate::msg; // #[macro_export] is weird...

/// Variant of [msg] for super-calls.
///
/// Unlike the other variants, this macro can only be used within
/// [crate::objc::objc_classes], because it relies on that macro defining a
/// constant containing the name of the current class.
///
/// ```ignore
/// msg_super![env; this init]
/// ```
///
/// desugars to something like this, if the current class is `SomeClass`:
///
/// ```ignore
/// {
///     let super_arg_ptr = push_to_stack(env, objc_super {
///         receiver: this,
///         class: env.objc.get_known_class("SomeClass", &mut env.mem),
///     });
///     let sel = env.objc.lookup_selector("init").unwrap();
///     let res = msg_send_super2(env, (super_arg_ptr, sel));
///     pop_from_stack::<objc_super>(env);
///     res
/// }
/// ```
#[macro_export]
macro_rules! msg_super {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let class = $env.objc.get_known_class(
                _OBJC_CURRENT_CLASS,
                &mut $env.mem
            );
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel)
                .expect("Unknown selector");

            let sp = &mut $env.cpu.regs_mut()[$crate::cpu::Cpu::SP];
            let old_sp = *sp;
            *sp -= $crate::mem::guest_size_of::<$crate::objc::objc_super>();
            let super_ptr = $crate::mem::Ptr::from_bits(*sp);
            $env.mem.write(super_ptr, $crate::objc::objc_super {
                receiver: $receiver,
                class,
            });

            let args = (super_ptr.cast_const(), sel, $($arg1, $($argn),*)?);
            let res = $crate::objc::msg_send_super2($env, args);

            $env.cpu.regs_mut()[$crate::cpu::Cpu::SP] = old_sp;

            res
        }
    }
}
pub use crate::msg_super; // #[macro_export] is weird...

/// Variant of [msg] for sending a message to a named class. Useful for calling
/// class methods, especially `new`.
///
/// ```ignore
/// msg_class![env; SomeClass alloc]
/// ```
///
/// desugars to:
///
/// ```ignore
/// msg![env; (env.objc.get_known_class("SomeClass", &mut env.mem)) alloc]
/// ```
#[macro_export]
macro_rules! msg_class {
    [$env:expr; $receiver_class:ident $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let class = $env.objc.get_known_class(
                stringify!($receiver_class),
                &mut $env.mem
            );
            $crate::objc::msg![$env; class $name $(: $arg1 $($($namen)?: $argn)*)?]
        }
    }
}
pub use crate::msg_class; // #[macro_export] is weird...

/// Shorthand for `let _: id = msg![env; object retain];`
pub fn retain(env: &mut Environment, object: id) -> id {
    if object == nil {
        // fast path
        return nil;
    }
    msg![env; object retain]
}

/// Shorthand for `() = msg![env; object release];`
pub fn release(env: &mut Environment, object: id) {
    if object == nil {
        // fast path
        return;
    }
    msg![env; object release]
}

/// Shorthand for `let _: id = msg![env; object autorelease];`
pub fn autorelease(env: &mut Environment, object: id) -> id {
    if object == nil {
        // fast path
        return nil;
    }
    msg![env; object autorelease]
}

#[cfg(test)]
mod cache_dispatch_tests {
    use super::*;
    use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
    use crate::objc::{ClassHostObject, TrivialHostObject};

    fn environment() -> Environment {
        Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap()
    }
    fn forty_two(_: &mut Environment, _: id, _: SEL) -> u32 {
        42
    }
    fn seventy_three(_: &mut Environment, _: id, _: SEL) -> u32 {
        73
    }
    fn rectangle(_: &mut Environment, _: id, _: SEL) -> CGRect {
        CGRect {
            origin: CGPoint { x: 3.0, y: 5.0 },
            size: CGSize {
                width: 7.0,
                height: 11.0,
            },
        }
    }

    #[test]
    fn cached_inherited_host_call_and_super_call_follow_class_mutations() {
        let mut environment = environment();
        let env = &mut environment;
        let parent = env.objc.get_known_class("NSObject", &mut env.mem);
        let child = env.objc.get_known_class("NSString", &mut env.mem);
        let sel = env
            .objc
            .register_host_selector("cacheDispatchValue".into(), &mut env.mem);
        env.objc
            .borrow_mut::<ClassHostObject>(parent)
            .methods
            .insert(
                sel,
                IMP::Host(&(forty_two as fn(&mut Environment, id, SEL) -> u32)),
            );
        let object = env
            .objc
            .alloc_object(child, Box::new(TrivialHostObject), &mut env.mem);
        assert_eq!(msg_send::<u32, _>(env, (object, sel)), 42);
        let hits = env.objc.method_cache.hits;
        assert_eq!(msg_send::<u32, _>(env, (object, sel)), 42);
        assert!(env.objc.method_cache.hits > hits);
        // Simulates a late category override, after the inherited IMP was cached.
        env.objc
            .borrow_mut::<ClassHostObject>(child)
            .methods
            .insert(
                sel,
                IMP::Host(&(seventy_three as fn(&mut Environment, id, SEL) -> u32)),
            );
        assert_eq!(msg_send::<u32, _>(env, (object, sel)), 73);
        let super_arg = env.mem.alloc_and_write(objc_super {
            receiver: object,
            class: child,
        });
        for _ in 0..2 {
            assert_eq!(
                msg_send_super2::<u32, _>(env, (super_arg.cast_const(), sel)),
                42
            );
            assert_eq!(msg_send::<u32, _>(env, (object, sel)), 73);
        }
        env.objc
            .borrow_mut::<ClassHostObject>(parent)
            .methods
            .insert(
                sel,
                IMP::Host(&(seventy_three as fn(&mut Environment, id, SEL) -> u32)),
            );
        assert_eq!(
            msg_send_super2::<u32, _>(env, (super_arg.cast_const(), sel)),
            73
        );
    }

    #[test]
    fn cached_host_struct_return_and_type_validation_remain_intact() {
        let mut environment = environment();
        let env = &mut environment;
        let class = env.objc.get_known_class("NSObject", &mut env.mem);
        let integer_sel = env
            .objc
            .register_host_selector("cacheInteger".into(), &mut env.mem);
        let rect_sel = env
            .objc
            .register_host_selector("cacheRectangle".into(), &mut env.mem);
        env.objc
            .borrow_mut::<ClassHostObject>(class)
            .methods
            .insert(
                integer_sel,
                IMP::Host(&(forty_two as fn(&mut Environment, id, SEL) -> u32)),
            );
        env.objc
            .borrow_mut::<ClassHostObject>(class)
            .methods
            .insert(
                rect_sel,
                IMP::Host(&(rectangle as fn(&mut Environment, id, SEL) -> CGRect)),
            );
        let object: id = msg![env; class new];
        for _ in 0..2 {
            let rect: CGRect = msg_send(env, (object, rect_sel));
            assert_eq!(
                (
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height
                ),
                (3.0, 5.0, 7.0, 11.0)
            );
            assert_eq!(msg_send::<u32, _>(env, (object, integer_sel)), 42);
        }
        let hits = env.objc.method_cache.hits;
        let mismatch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: u64 = msg_send(env, (object, integer_sel));
        }));
        assert!(mismatch.is_err());
        assert!(
            env.objc.method_cache.hits > hits,
            "The rejected call must take the cached path"
        );
    }
}
