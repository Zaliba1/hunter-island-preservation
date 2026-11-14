//! Things from `NSObjCRuntime.h`.

use super::ns_string;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::ConstVoidPtr;
use crate::objc::{autorelease, id, nil, Class, SEL};
use crate::Environment;

fn NSStringFromSelector(env: &mut Environment, selector: SEL) -> id {
    // TODO: caching?
    let string = selector.as_str(&env.mem).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSStringFromProtocol(env: &mut Environment, protocol: ConstVoidPtr) -> id {
    if protocol.is_null() {
        return nil;
    }
    let name = crate::objc::protocol_name(&env.mem, protocol);
    let name = env
        .mem
        .cstr_at_utf8(name)
        .expect("Protocol name is not UTF-8")
        .to_owned();
    let result = ns_string::from_rust_string(env, name);
    autorelease(env, result)
}

fn NSSelectorFromString(env: &mut Environment, string: id) -> SEL {
    // TODO: avoid copy?
    let string = ns_string::to_rust_string(env, string);
    env.objc.register_host_selector(string.into(), &mut env.mem)
}

pub fn NSStringFromClass(env: &mut Environment, class: Class) -> id {
    if class.is_null() {
        return nil;
    }
    // TODO: caching?
    let string = env.objc.get_class_name(class).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSClassFromString(env: &mut Environment, string: id) -> Class {
    if string == nil {
        return nil;
    }
    // TODO: avoid copy?
    let string = ns_string::to_rust_string(env, string);

    // While this method is supposed to return nil if the class is not found,
    // touchHLE is missing many classes that apps might expect to be present,
    // so this could be troublesome. So, let's use get_known_class, which panics
    // when it can't find the class. We could except certain classes or apps if
    // we need to.
    env.objc.get_known_class(&string, &mut env.mem)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSStringFromSelector(_)),
    export_c_func!(NSSelectorFromString(_)),
    export_c_func!(NSClassFromString(_)),
    export_c_func!(NSStringFromClass(_)),
    export_c_func!(NSStringFromProtocol(_)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objc::{msg_class, release, retain};
    #[test]
    fn protocol_string_copies_the_real_guest_name_and_survives_retained_pool_drain() {
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
        let name = env.mem.alloc_and_write_cstr(b"OwnProtocol");
        let raw = env.mem.alloc(12).cast::<u32>();
        env.mem.write(raw, 0u32);
        env.mem.write(raw + 1, name.to_bits());
        env.mem.write(raw + 2, 0u32);
        let result = NSStringFromProtocol(env, raw.cast_const().cast());
        retain(env, result);
        env.mem.write(name, b'X');
        release(env, pool);
        assert_eq!(ns_string::to_rust_string(env, result), "OwnProtocol");
        assert_eq!(NSStringFromProtocol(env, ConstVoidPtr::null()), nil);
        release(env, result);
        env.mem.free(name.cast());
        env.mem.free(raw.cast());
    }
}
