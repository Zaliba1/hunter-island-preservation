/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C classes and metaclasses.
//!
//! Note that metaclasses are just a special case of classes.
//!
//! Resources:
//! - [[objc explain]: Classes and metaclasses](http://www.sealiesoftware.com/blog/archive/2009/04/14/objc_explain_Classes_and_metaclasses.html), especially [the PDF diagram](http://www.sealiesoftware.com/blog/class%20diagram.pdf)

use super::{
    id, ivar_list_t, method_list_t, nil, objc_object, AnyHostObject, HostIMP, HostObject, ObjC,
    IMP, SEL,
};
use crate::libc::string::strdup;
use crate::mach_o::MachO;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, Mem, MutPtr, Ptr, SafeRead};
use crate::Environment;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

/// Generic pointer to an Objective-C class or metaclass.
///
/// The name is standard Objective-C.
///
/// Apple's runtime has a `objc_classes` definition similar to `objc_object`.
/// We could do the same thing here, but it doesn't seem worth it, as we can't
/// get the same unidirectional type safety.
pub type Class = id;

/// Our internal representation of a class, e.g. this is where `objc_msgSend`
/// will look up method implementations.
///
/// Note: `superclass` can be `nil`!
pub(super) struct ClassHostObject {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
    pub(super) superclass: Class,
    pub(super) methods: HashMap<SEL, IMP>,
    pub(super) guest_method_signatures: HashMap<SEL, ConstPtr<u8>>,
    pub(super) protocols: Vec<ConstVoidPtr>,
    pub(super) properties: Vec<objc_property_t>,
    /// Maps ivar name to a tuple of an offset (as pointer) and an alignment.
    /// (Alignment is used during ivar reconciliation.)
    pub(super) ivars: HashMap<String, (ConstPtr<GuestUSize>, u32)>,
    pub(super) ivar_types: HashMap<String, (ConstPtr<u8>, GuestUSize)>,
    /// Offset into the allocated memory for the object where the ivars of
    /// instances of this class or metaclass (respectively: normal objects or
    /// classes) should live. This is always >= the value in the superclass.
    pub(super) instance_start: GuestUSize,
    /// Size of the allocated memory for instances of this class or metaclass.
    /// This is always >= the value in the superclass.
    pub(super) instance_size: GuestUSize,
    /// Checks if +initialize has been called yet.
    pub(super) is_initialized: InitializationStatus,
}
impl HostObject for ClassHostObject {}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum InitializationStatus {
    NotInitialized,
    Initializing,
    Initialized,
}

/// Placeholder object for classes and metaclasses referenced by the app that
/// we don't have an implementation for.
///
/// This lets us delay errors about missing implementations until the first
/// time the app actually uses them (e.g. when a message is sent).
pub(super) struct UnimplementedClass {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
}
impl HostObject for UnimplementedClass {}

/// Substitute object for classes and metaclasses from the guest app that we do
/// not want to support (see [substitute_classes]).
///
/// Messages sent to this class will behave as if messaging [nil].
pub(super) struct FakeClass {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
}
impl HostObject for FakeClass {}

/// The layout of a class in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
#[allow(dead_code)]
struct class_t {
    isa: Class, // note that this matches objc_object
    superclass: Class,
    _cache: ConstVoidPtr,
    _vtable: ConstVoidPtr,
    data: ConstPtr<class_rw_t>,
}
unsafe impl SafeRead for class_t {}

/// The layout of the main class data in an app binary.
///
/// The name, field names and field layout are based on what Ghidra's output.
#[repr(C, packed)]
#[allow(dead_code)]
struct class_rw_t {
    _flags: u32,
    instance_start: GuestUSize,
    instance_size: GuestUSize,
    _reserved: u32,
    name: ConstPtr<u8>,
    base_methods: ConstPtr<method_list_t>,
    base_protocols: ConstVoidPtr,
    ivars: ConstPtr<ivar_list_t>,
    _weak_ivar_layout: u32,
    base_properties: ConstVoidPtr,
}
unsafe impl SafeRead for class_rw_t {}

/// The layout of a category in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
struct category_t {
    name: ConstPtr<u8>,
    class: Class,
    instance_methods: ConstPtr<method_list_t>,
    class_methods: ConstPtr<method_list_t>,
    protocols: ConstVoidPtr,
    property_list: ConstVoidPtr,
}
unsafe impl SafeRead for category_t {}

#[repr(C, packed)]
pub struct objc_property {
    pub(super) name: ConstPtr<u8>,
    pub(super) attributes: ConstPtr<u8>,
}
unsafe impl SafeRead for objc_property {}

/// An opaque type that represents an Objective-C declared property.
#[allow(non_camel_case_types)]
pub(super) type objc_property_t = MutPtr<objc_property>;

/// A template for a class defined with [objc_classes].
///
/// Host implementations of libraries can use these to expose classes to the
/// application. The runtime will create the actual class ([ClassHostObject]
/// etc) from the template on-demand. See also [ClassExports].
pub struct ClassTemplate {
    pub name: &'static str,
    pub superclass: Option<&'static str>,
    pub class_methods: &'static [(&'static str, &'static dyn HostIMP)],
    pub instance_methods: &'static [(&'static str, &'static dyn HostIMP)],
}

/// Type for lists of classes exported by host implementations of frameworks.
///
/// Each module that wants to expose functions to guest code should export a
/// constant using this type. See [objc_classes] for an example.
///
/// All the constants like this can then be collected into a
/// [crate::dyld::HostDylib].
///
/// The strings are the class names.
///
/// See also [crate::dyld::ConstantExports] and [crate::dyld::FunctionExports].
pub type ClassExports = &'static [(&'static str, ClassTemplate)];

#[doc(hidden)]
#[macro_export]
macro_rules! _objc_superclass {
    (: $name:ident) => {
        Some(stringify!($name))
    };
    () => {
        None
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! _objc_method {
    (
        $env:ident,
        $this:ident,
        $_cmd:ident,
        $cmd_name:ident,
        $retty:ty,
        $block:block
        $(, $ty:ty, $arg:ident)*
        $(, ...$va_arg:ident: $va_type:ty)?
    ) => {
        // The closure must be explicitly casted because a bare closure defaults
        // to a different type than a pure fn pointer, which is the type that
        // HostIMP and CallFromGuest are implemented on.
        &((|
            #[allow(unused_variables)]
            $env: &mut $crate::Environment,
            #[allow(unused_variables)]
            $this: $crate::objc::id,
            #[allow(unused_variables)]
            $_cmd: $crate::objc::SEL,
            $($arg: $ty,)*
            $(#[allow(unused_mut)] mut $va_arg: $va_type,)?
        | -> $retty {
            const _OBJC_CURRENT_SELECTOR: &str = stringify!($cmd_name);
            $block
        }) as fn(
            &mut $crate::Environment,
            $crate::objc::id,
            $crate::objc::SEL,
            $($ty,)*
            $($va_type,)?
        ) -> $retty)
    }
}

/// Macro for creating a list of [ClassTemplate]s (i.e. [ClassExports]).
/// It imitates the Objective-C class definition syntax.
///
/// ```ignore
/// pub const CLASSES: ClassExports = objc_classes! {
/// (env, this, _cmd); // Specify names of HostIMP implicit parameters.
///                    // The second one should be `self` to match Objective-C,
///                    // but that's reserved in Rust, hence `this`.
///
/// @implementation MyClass: NSObject
///
/// + (id)foo {
///     // ...
/// }
///
/// - (id)barWithQux:(u32)qux {
///     // ...
/// }
///
/// - (id)barWithVaArgs:(u32)qux, ...dots {
///     // ...
/// }
///
/// @end
/// };
/// ```
///
/// will desugar to approximately:
///
/// ```ignore
/// pub const CLASSES: ClassExports = &[
///     ("MyClass", ClassTemplate {
///         name: "MyClass",
///         superclass: Some("NSObject"),
///         class_methods: &[
///             ("foo", &(|env: &mut Environment, this: id, _cmd: SEL| -> id {
///                 // ...
///             } as fn(&mut Environment, id, SEL) -> id)),
///         ],
///         instance_methods: &[
///             ("barWithQux:", &(|
///                 env: &mut Environment,
///                 this: id,
///                 _cmd: SEL,
///                 qux: u32
///             | -> id {
///                 // ...
///             } as &fn(&mut Environment, id, SEL, u32) -> id)),
///             ("barWithVaArgs:", &(|
///                 env: &mut Environment,
///                 this: id,
///                 _cmd: SEL,
///                 qux: u32,
///                 va_args: DotDotDot
///             | -> id {
///                 // ...
///             } as &fn(&mut Environment, id, SEL, u32, DotDotDot) -> id)),
///         ],
///     })
/// ];
/// ```
///
/// Note that the instance methods must be preceded by the class methods.
#[macro_export] // documentation comment links are annoying without this
macro_rules! objc_classes {
    {
        // Rust's macro hygiene prevents the macro's own names for these
        // parameters being visible, so we have to get names supplied by the
        // macro user.
        ($env:ident, $this:ident, $_cmd:ident);
        $(
            @implementation $class_name:ident $(: $superclass_name:ident)?

            $( + ($cm_type:ty) $cm_name:ident $(:($cm_type1:ty) $cm_arg1:ident $($($cm_namen:ident)?:($cm_typen:ty) $cm_argn:ident)*)?
                              $(, ...$cm_va_arg:ident)?
                 $cm_block:block )*

            $( - ($im_type:ty) $im_name:ident $(:($im_type1:ty) $im_arg1:ident $($($im_namen:ident)?:($im_typen:ty) $im_argn:ident)*)?
                              $(, ...$im_va_arg:ident)?
                 $im_block:block )*

            @end
        )+
    } => {
        &[
            $({
                // This constant is for `msg_super!`, which needs to know which
                // class it is has been written within (not the same as the
                // runtime type of `this`, which could be a subclass). This is
                // a constant instead of a let binding because that escapes
                // Rust's macro hygiene.
                const _OBJC_CURRENT_CLASS: &str = stringify!($class_name);

                (_OBJC_CURRENT_CLASS, $crate::objc::ClassTemplate {
                    name: _OBJC_CURRENT_CLASS,
                    superclass: $crate::_objc_superclass!($(: $superclass_name)?),
                    class_methods: &[
                        $(
                            (
                                $crate::objc::selector!(
                                    $(($cm_type1);)?
                                    $cm_name
                                    $($(, $($cm_namen)?)*)?
                                ),
                                $crate::_objc_method!(
                                    $env,
                                    $this,
                                    $_cmd,
                                    $cm_name,
                                    $cm_type,
                                    { $cm_block }
                                    $(, $cm_type1, $cm_arg1 $(, $cm_typen, $cm_argn)*)?
                                    $(, ...$cm_va_arg: $crate::abi::DotDotDot)?
                                )
                            )
                        ),*
                    ],
                    instance_methods: &[
                        $(
                            (
                                $crate::objc::selector!(
                                    $(($im_type1);)?
                                    $im_name
                                    $($(, $($im_namen)?)*)?
                                ),
                                $crate::_objc_method!(
                                    $env,
                                    $this,
                                    $_cmd,
                                    $im_name,
                                    $im_type,
                                    { $im_block }
                                    $(, $im_type1, $im_arg1 $(, $im_typen, $im_argn)*)?
                                    $(, ...$im_va_arg: $crate::abi::DotDotDot)?
                                )
                            )
                        ),*
                    ],
                })
            }),+
        ]
    }
}
pub use crate::objc_classes; // #[macro_export] is weird...

impl ClassHostObject {
    fn from_template(
        template: &ClassTemplate,
        is_metaclass: bool,
        superclass: Class,
        objc: &ObjC,
    ) -> Self {
        // For our host implementations we store all data in host objects, so
        // there are no ivars and the size is always just the isa pointer.
        // This is true for both classes and normal objects.
        let size = guest_size_of::<objc_object>();
        ClassHostObject {
            name: template.name.to_string(),
            is_metaclass,
            superclass,
            methods: HashMap::from_iter(
                (if is_metaclass {
                    template.class_methods
                } else {
                    template.instance_methods
                })
                .iter()
                .map(|&(name, host_imp)| {
                    // The selector should already have been registered by
                    // [ObjC::register_host_selectors], so we can panic
                    // if it hasn't been.
                    (objc.selectors[name], IMP::Host(host_imp))
                }),
            ),
            guest_method_signatures: HashMap::default(),
            protocols: Vec::new(),
            properties: Vec::new(),
            // maybe this should be 0 for NSObject? does it matter?
            instance_start: size,
            instance_size: size,
            ivars: HashMap::default(),
            ivar_types: HashMap::default(),
            is_initialized: InitializationStatus::NotInitialized,
        }
    }

    fn from_bin(class: Class, is_metaclass: bool, mem: &Mem, objc: &mut ObjC) -> Self {
        let class_t {
            superclass, data, ..
        } = mem.read(class.cast());
        let class_rw_t {
            instance_start,
            instance_size,
            name,
            base_methods,
            base_protocols,
            base_properties,
            ivars,
            ..
        } = mem.read(data);

        let name = mem.cstr_at_utf8(name).unwrap().to_string();

        let mut host_object = ClassHostObject {
            name,
            is_metaclass,
            superclass,
            methods: HashMap::new(),
            guest_method_signatures: HashMap::new(),
            protocols: super::protocols::read_list(mem, base_protocols),
            properties: super::property_metadata::read_list(mem, base_properties),
            instance_start,
            instance_size,
            ivars: HashMap::new(),
            ivar_types: HashMap::new(),
            is_initialized: InitializationStatus::NotInitialized,
        };

        if !base_methods.is_null() {
            host_object.add_methods_from_bin(base_methods, mem, objc);
        }

        if !ivars.is_null() {
            host_object.add_ivars_from_bin(ivars, mem);
        }

        host_object
    }

    // See methods.rs for binary method parsing
}

/// Decide whether a certain class/metaclass pair from the guest app should use
/// fake class host objects and return the substitutions if so.
///
/// This function is called when registering classes from the guest app. It
/// detects certain problematic classes that are, for example, too complex for
/// touchHLE to currently support, but which can be easily replaced with simple
/// fakes.
fn substitute_classes(
    mem: &Mem,
    class: Class,
    metaclass: Class,
    class_names: &HashMap<Class, String>,
) -> Option<(Box<FakeClass>, Box<FakeClass>)> {
    let class_t { data, .. } = mem.read(class.cast());
    let class_rw_t { name, .. } = mem.read(data);
    let name = mem.cstr_at_utf8(name).unwrap();

    // Currently the only thing we try to substitute: classes that seem to be
    // from various third-party advertising or social network SDKs.
    // Naturally it makes a lot of use of UIKit and networking in ways we
    // don't support yet. This isn't "ad blocking" because ads no longer work
    // on real devices anyway :)
    let superclass = mem
        .read::<class_t, false>(class.cast().cast_const())
        .superclass;
    let parent_name = class_names.get(&superclass).map(String::as_str);
    if !should_substitute_sdk_class(name, parent_name) {
        return None;
    }

    {
        let class_t { data, .. } = mem.read(metaclass.cast());
        let class_rw_t {
            name: metaclass_name,
            ..
        } = mem.read(data);
        let metaclass_name = mem.cstr_at_utf8(metaclass_name).unwrap();
        assert!(name == metaclass_name);
    }

    log!(
        "Note: substituting fake class for {} to improve compatibility",
        name
    );

    let class_host_object = Box::new(FakeClass {
        name: name.to_string(),
        is_metaclass: false,
    });
    let metaclass_host_object = Box::new(FakeClass {
        name: name.to_string(),
        is_metaclass: true,
    });
    Some((class_host_object, metaclass_host_object))
}

impl ObjC {
    /// Iterator over all known classes and their names.
    pub fn all_classes(&self) -> impl Iterator<Item = (&String, &Class)> {
        self.classes.iter()
    }

    fn get_class(&self, name: &str, is_metaclass: bool, mem: &Mem) -> Option<Class> {
        let class = self.classes.get(name).copied()?;
        Some(if is_metaclass {
            Self::read_isa(class, mem)
        } else {
            class
        })
    }

    fn find_template(name: &str) -> Option<&'static ClassTemplate> {
        crate::dyld::search_host_dylibs(|dylib| dylib.class_exports, name)
            .map(|&(_name, ref template)| template)
    }

    /// For use by [crate::dyld]: get the class or metaclass referenced by an
    /// external relocation in the app binary. If we don't have an
    /// implementation of the class, a placeholder is used.
    pub fn link_class(&mut self, name: &str, is_metaclass: bool, mem: &mut Mem) -> Class {
        self.link_class_inner(name, is_metaclass, mem, true)
    }

    /// For use by host functions: get a particular class. If we don't have an
    /// implementation of the class, panic.
    pub fn get_known_class(&mut self, name: &str, mem: &mut Mem) -> Class {
        self.link_class_inner(name, /* is_metaclass: */ false, mem, false)
    }

    fn link_class_inner(
        &mut self,
        name: &str,
        is_metaclass: bool,
        mem: &mut Mem,
        use_placeholder: bool,
    ) -> Class {
        // The class and metaclass must be created together and tracked
        // together, so even though this function only returns one pointer, it
        // must create both. The function must not care whether the metaclass
        // is requested first, or if the class is requested first.

        if let Some(class) = self.get_class(name, is_metaclass, mem) {
            return class;
        };

        let class_host_object: Box<dyn AnyHostObject>;
        let metaclass_host_object: Box<dyn AnyHostObject>;
        if let Some(template) = Self::find_template(name) {
            // We have a template (host implementation) for this class, use it.

            if let Some(superclass_name) = template.superclass {
                // Make sure we actually have a template for the superclass
                // before we try to link it, else we might get an unimplemented
                // class back and have weird problems down the line
                assert!(Self::find_template(superclass_name).is_some());
            }

            class_host_object = Box::new(ClassHostObject::from_template(
                template,
                /* is_metaclass: */ false,
                /* superclass: */
                template
                    .superclass
                    .map(|name| {
                        self.link_class(name, /* is_metaclass: */ false, mem)
                    })
                    .unwrap_or(nil),
                self,
            ));
            metaclass_host_object = Box::new(ClassHostObject::from_template(
                template,
                /* is_metaclass: */ true,
                /* superclass: */
                template
                    .superclass
                    .map(|name| {
                        self.link_class(name, /* is_metaclass: */ true, mem)
                    })
                    .unwrap_or(nil),
                self,
            ));
        } else {
            if !use_placeholder {
                panic!("Missing implementation for class {name}!");
            }

            // We don't have a real implementation for this class, use a
            // placeholder.

            class_host_object = Box::new(UnimplementedClass {
                name: name.to_string(),
                is_metaclass: false,
            });
            metaclass_host_object = Box::new(UnimplementedClass {
                name: name.to_string(),
                is_metaclass: true,
            });
        }

        // NSObject's metaclass is special: it is its own metaclass, and it's
        // the superclass of all other metaclasses.
        // (FIXME: this actually should apply to any class hiearchy root.)
        // This creates a chicken-and-egg problem so it has a special path.
        let metaclass = if name == "NSObject" {
            let metaclass = mem.alloc_and_write(objc_object { isa: nil });
            mem.write(metaclass, objc_object { isa: metaclass });
            self.register_static_object(metaclass, metaclass_host_object);
            metaclass
        } else {
            let isa = self.link_class("NSObject", /* is_metaclass: */ true, mem);
            self.alloc_static_object(isa, metaclass_host_object, mem)
        };

        let class = self.alloc_static_object(metaclass, class_host_object, mem);

        if name == "NSObject" {
            // NSObject's metaclass has its class as the superclass.
            self.borrow_mut::<ClassHostObject>(metaclass).superclass = class;
        }

        self.classes.insert(name.to_string(), class);

        if is_metaclass {
            metaclass
        } else {
            class
        }
    }

    /// For use by [crate::dyld]: register all the classes from the application
    /// binary.
    pub fn register_bin_classes(&mut self, bin: &MachO, mem: &mut Mem) {
        let Some(list) = bin.get_section("__objc_classlist") else {
            return;
        };

        assert!(list.size % 4 == 0);
        let base: ConstPtr<Class> = Ptr::from_bits(list.addr);
        let class_names: HashMap<Class, String> = (0..list.size / 4)
            .map(|index| {
                let class = mem.read(base + index);
                let data = mem.read::<class_t, false>(class.cast().cast_const()).data;
                let name = mem.read(data).name;
                (class, mem.cstr_at_utf8(name).unwrap().to_owned())
            })
            .collect();
        for i in 0..(list.size / 4) {
            let class = mem.read(base + i);
            let metaclass = Self::read_isa(class, mem);

            let name = if let Some(fakes) = substitute_classes(mem, class, metaclass, &class_names)
            {
                let (class_host_object, metaclass_host_object) = fakes;

                assert!(class_host_object.name == metaclass_host_object.name);
                let name = class_host_object.name.clone();

                self.register_static_object(class, class_host_object);
                self.register_static_object(metaclass, metaclass_host_object);
                name
            } else {
                let class_host_object = Box::new(ClassHostObject::from_bin(
                    class, /* is_metaclass: */ false, mem, self,
                ));
                let metaclass_host_object = Box::new(ClassHostObject::from_bin(
                    metaclass, /* is_metaclass: */ true, mem, self,
                ));

                assert!(class_host_object.name == metaclass_host_object.name);
                let name = class_host_object.name.clone();

                self.register_static_object(class, class_host_object);
                self.register_static_object(metaclass, metaclass_host_object);
                name
            };

            self.classes.insert(name.to_string(), class);
        }

        let mut queue = VecDeque::<Class>::new();
        let mut found_ns_object = false;
        // Second pass to build an inverted inheritance graph
        let mut inverted_inheritance = HashMap::<Class, Vec<Class>>::new();
        for (name, class) in self.classes.iter() {
            log_dbg!("class name {}", name);
            if name == "NSObject" {
                assert!(!found_ns_object);
                found_ns_object = true;
            }
            let class_host_object = self
                .get_host_object(*class)
                .unwrap()
                .as_any()
                .downcast_ref();
            let Some(ClassHostObject { superclass, .. }) = class_host_object else {
                // Skip FakeClass or UnimplementedClass
                continue;
            };

            if *superclass != nil {
                inverted_inheritance
                    .entry(*superclass)
                    .and_modify(|v| v.push(*class))
                    .or_insert(vec![*class]);
            } else {
                assert!(!queue.contains(class));
                queue.push_back(*class);
            }
        }
        // At least NSObject should be found as a root object
        assert!(found_ns_object);

        // Third pass to ensure no superclass has "grown into" any of its
        // subclasses.
        // (https://alwaysprocessing.blog/2023/03/12/objc-ivar-abi)

        // BFS starting from root(s) for ivar reconciliation
        //
        // It is required to be traversed in this order,
        // as one overgrown class potentially implies
        // reconciliation for _all of subclasses_
        while !queue.is_empty() {
            let next = queue.pop_front().unwrap();
            self.reconcile_class_ivars(next, mem);
            if let Some(subclasses) = inverted_inheritance.get(&next) {
                queue.extend(subclasses);
            }
        }
    }

    /// Slide the *values* in the compiler's shared ivar-offset globals. The
    /// pointer identities must remain unchanged: compiled guest getters and
    /// setters load from those globals directly, rather than our metadata map.
    pub(super) fn reconcile_class_ivars(&mut self, class: Class, mem: &mut Mem) {
        let (need, diff) = self.need_ivar_reconciliation(class);
        if !need {
            return;
        }
        let host = self.borrow::<ClassHostObject>(class);
        let alignment = host
            .ivars
            .values()
            .filter(|(p, _)| !p.is_null())
            .map(|(_, raw)| ivar_alignment(*raw))
            .max()
            .unwrap_or(1);
        let mask = alignment - 1;
        let slide = diff.checked_add(mask).expect("Ivar slide overflow") & !mask;
        let offsets = host
            .ivars
            .values()
            .filter(|(p, _)| !p.is_null())
            .map(|(p, _)| *p)
            .collect::<std::collections::HashSet<_>>();
        for pointer in offsets {
            let offset = mem.read(pointer);
            mem.write(
                pointer.cast_mut(),
                offset.checked_add(slide).expect("Ivar offset overflow"),
            );
        }
        let host = self.borrow_mut::<ClassHostObject>(class);
        host.instance_start = host
            .instance_start
            .checked_add(slide)
            .expect("Class start overflow");
        host.instance_size = host
            .instance_size
            .checked_add(slide)
            .expect("Class size overflow");
    }

    fn need_ivar_reconciliation(&mut self, class: Class) -> (bool, u32) {
        let class_host_object = self.get_host_object(class).unwrap().as_any().downcast_ref();
        let Some(ClassHostObject {
            name,
            superclass,
            instance_start,
            instance_size,
            ..
        }) = class_host_object
        else {
            // The class might be a FakeClass or UnimplementedClass
            // In those cases we move on as they don't have ivars
            return (false, 0);
        };
        log_dbg!(
            "Checking need_ivar_reconciliation for {}, start {}, size {}",
            name,
            instance_start,
            instance_size
        );

        if *superclass == nil {
            return (false, 0);
        }

        let superclass_host_object = self
            .get_host_object(*superclass)
            .unwrap()
            .as_any()
            .downcast_ref();
        let Some(ClassHostObject {
            instance_size: superclass_instance_size,
            ..
        }) = superclass_host_object
        else {
            // Superclass could also be a FakeClass or UnimplementedClass
            return (false, 0);
        };

        let need = instance_start < superclass_instance_size;
        let diff = if need {
            superclass_instance_size - instance_start
        } else {
            0
        };
        (need, diff)
    }

    /// Dumps all classes available to the emulator in JSON to stdout.
    ///
    /// The JSON has the following form:
    /// ```json
    /// {
    ///     "object": "classes",
    ///     "classes": [
    ///         {
    ///             "name": ((name of class)),
    ///             "super": ((name of superclass, if available)),
    ///             "class_type": (("normal" | "unimplemented" | "fake"))
    ///         },
    ///         ...
    ///     ]
    /// }
    /// ```
    pub fn dump_classes(&self, file: &mut std::fs::File) -> Result<(), std::io::Error> {
        use std::io::Write;
        writeln!(file, "{{\n    \"object\": \"classes\",\n    \"classes\": [")?;
        for (i, (_, o)) in self.classes.iter().enumerate() {
            // Why doesn't json allow trailing commas...
            let comma = if i == self.classes.len() - 1 { "" } else { "," };

            let host_obj = self.get_host_object(*o).unwrap();

            if let Some(ClassHostObject {
                name,
                superclass: sup,
                ..
            }) = host_obj.as_any().downcast_ref()
            {
                if *sup == nil {
                    writeln!(
                        file,
                        "        {{ \"name\": \"{name}\", \"class_type\": \"normal\" }}{comma}"
                    )?;
                } else {
                    writeln!(
                        file,
                        "        {{ \"name\": \"{}\", \"super\": \"{}\", \"class_type\": \"normal\" }}{}",
                        name, self.get_class_name(*sup), comma
                    )?;
                }
            } else if let Some(UnimplementedClass { name, .. }) = host_obj.as_any().downcast_ref() {
                writeln!(
                    file,
                    "        {{ \"name\": \"{name}\", \"class_type\": \"unimplemented\" }}{comma}"
                )?;
            } else if let Some(FakeClass { name, .. }) = host_obj.as_any().downcast_ref() {
                writeln!(
                    file,
                    "        {{ \"name\": \"{name}\", \"class_type\": \"fake\" }}{comma}"
                )?;
            } else {
                panic!("Unrecognized class type!");
            }
        }
        writeln!(file, "    ]\n}}")
    }

    /// For use by [crate::dyld]: register all the categories from the
    /// application binary.
    pub fn register_bin_categories(&mut self, bin: &MachO, mem: &mut Mem) {
        let Some(list) = bin.get_section("__objc_catlist") else {
            return;
        };

        assert!(list.size % 4 == 0);
        let base: ConstPtr<ConstPtr<category_t>> = Ptr::from_bits(list.addr);
        for i in 0..(list.size / 4) {
            let cat_ptr = mem.read(base + i);
            let data = mem.read(cat_ptr);

            let name = mem.cstr_at_utf8(data.name).unwrap();
            let class = data.class;
            let metaclass = Self::read_isa(class, mem);
            let category_protocols = super::protocols::read_list(mem, data.protocols);

            for (class, methods) in [
                (class, data.instance_methods),
                (metaclass, data.class_methods),
            ] {
                if methods.is_null()
                    && (class == metaclass
                        || (category_protocols.is_empty() && data.property_list.is_null()))
                {
                    continue;
                }

                let any = self.get_host_object(class).unwrap().as_any();
                if any.is::<FakeClass>() || any.is::<UnimplementedClass>() {
                    continue;
                }

                // Horrible workaround to avoid double-borrowing self:
                // temporarily replace the class object.
                let mut host_obj = std::mem::replace(
                    self.borrow_mut::<ClassHostObject>(class),
                    ClassHostObject {
                        name: Default::default(),
                        is_metaclass: Default::default(),
                        superclass: nil,
                        methods: Default::default(),
                        guest_method_signatures: Default::default(),
                        protocols: Vec::new(),
                        properties: Vec::new(),
                        instance_start: Default::default(),
                        instance_size: Default::default(),
                        ivars: Default::default(),
                        ivar_types: Default::default(),
                        is_initialized: InitializationStatus::NotInitialized,
                    },
                );
                log_dbg!(
                    "Adding {} methods from guest app category \"{}\" {:?} to {} \"{}\" {:?}",
                    if host_obj.is_metaclass {
                        "class"
                    } else {
                        "instance"
                    },
                    name,
                    cat_ptr,
                    if host_obj.is_metaclass {
                        "metaclass"
                    } else {
                        "class"
                    },
                    host_obj.name,
                    class,
                );
                if !methods.is_null() {
                    host_obj.add_methods_from_bin(methods, mem, self);
                }
                if !host_obj.is_metaclass {
                    host_obj
                        .properties
                        .extend(super::property_metadata::read_list(mem, data.property_list));
                    host_obj
                        .protocols
                        .extend(category_protocols.iter().copied());
                }
                *self.borrow_mut::<ClassHostObject>(class) = host_obj;
            }
        }
    }

    pub fn class_is_subclass_of(&self, class: Class, superclass: Class) -> bool {
        if class == superclass {
            return true;
        }

        let mut class = class;
        loop {
            let &ClassHostObject {
                superclass: next, ..
            } = self.borrow(class);
            if next == nil {
                return false;
            } else if next == superclass {
                return true;
            } else {
                class = next;
            }
        }
    }

    pub fn get_class_name(&self, class: Class) -> &str {
        self.try_get_class_name(class)
            .expect("Could not get class name!")
    }

    pub fn get_superclass(&self, class: Class) -> Class {
        let &ClassHostObject { superclass, .. } = self.borrow(class);
        superclass
    }

    pub fn try_get_class_name(&self, class: Class) -> Option<&str> {
        let host_object = self.get_host_object(class)?;
        if let Some(ClassHostObject { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else if let Some(UnimplementedClass { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else if let Some(FakeClass { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else {
            None
        }
    }

    pub fn is_unimplemented_class(&self, class: Class) -> bool {
        if class == nil {
            return false;
        }
        let host_object = self.get_host_object(class).unwrap();
        matches!(
            host_object.as_any().downcast_ref(),
            Some(UnimplementedClass { .. })
        )
    }

    pub fn is_fake_class(&self, class: Class) -> bool {
        if class == nil {
            return false;
        }
        let host_object = self.get_host_object(class).unwrap();
        matches!(host_object.as_any().downcast_ref(), Some(FakeClass { .. }))
    }
}

pub(super) fn objc_getClass(env: &mut Environment, name: ConstPtr<u8>) -> id {
    let name_str = env.mem.cstr_at_utf8(name).unwrap();
    env.objc
        .get_class(name_str, false, &env.mem)
        .unwrap_or_else(|| panic!("objc_getClass() for unimplemented class {name_str}"))
}

pub(super) fn class_getSuperclass(env: &mut Environment, cls: Class) -> Class {
    if cls == nil {
        nil
    } else {
        env.objc.borrow::<ClassHostObject>(cls).superclass
    }
}

pub(super) fn class_getInstanceSize(env: &mut Environment, cls: Class) -> GuestUSize {
    if cls == nil {
        0
    } else {
        env.objc.borrow::<ClassHostObject>(cls).instance_size
    }
}

pub(super) fn class_getProperty(
    env: &mut Environment,
    cls: Class,
    name: ConstPtr<u8>,
) -> objc_property_t {
    super::property_metadata::class_get_property(env, cls, name)
}

pub(super) fn class_replaceMethod(
    env: &mut Environment,
    cls: Class,
    name: SEL,
    imp: IMP,
    types: ConstPtr<u8>,
) -> IMP {
    // TODO: avoid unnecessary copy of types
    let types_copy = strdup(env, types).cast_const();
    let &mut ClassHostObject {
        ref mut methods,
        ref mut guest_method_signatures,
        ..
    } = env.objc.borrow_mut(cls);
    if let Entry::Vacant(e) = methods.entry(name) {
        // TODO: use `class_addMethod` once implemented
        log_dbg!(
            "class_replaceMethod: adding new implementation {:?} for method {}",
            imp,
            name.as_str(&env.mem)
        );
        e.insert(imp);
        guest_method_signatures.insert(name, types_copy);
        return IMP::guest_null();
    }
    env.mem.free(types_copy.cast().cast_mut());
    // TODO: use `method_setImplementation` once implemented
    // Note: encoding types are ignored
    let existing = methods.insert(name, imp.clone()).unwrap();
    assert!(matches!(existing, IMP::Guest(_))); // TODO
    log_dbg!(
        "class_replaceMethod: existing {:?} replaced with {:?} for method {}",
        existing,
        imp,
        name.as_str(&env.mem)
    );
    existing
}

pub(crate) fn class_getMethodImplementation(env: &mut Environment, cls: Class, name: SEL) -> IMP {
    if cls == nil {
        return IMP::guest_null();
    }
    let mut class = cls;
    loop {
        let &ClassHostObject {
            superclass: next,
            ref methods,
            ..
        } = env.objc.borrow(class);
        if methods.contains_key(&name) {
            let method = methods.get(&name).unwrap().clone();
            assert!(matches!(method, IMP::Guest(_))); // TODO
            return method;
        } else if next == nil {
            // TODO: currently this returns NULL for unimplemented host methods
            return IMP::guest_null();
        }
        class = next;
    }
}

#[cfg(test)]
mod implementation_lookup_tests {
    use super::*;
    use crate::abi::GuestFunction;
    use crate::objc::msg;
    #[test]
    fn instance_method_address_is_inherited_and_callable() {
        let mut environment = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut environment;
        let selector = env
            .objc
            .register_host_selector("touchhleProbeInteger".into(), &mut env.mem);
        let parent = env.objc.get_known_class("NSObject", &mut env.mem);
        let child = env.objc.get_known_class("NSString", &mut env.mem);
        // Own, fully specified Thumb program: movs r0, #42; bx lr.
        let code = env.mem.alloc(4);
        env.mem
            .bytes_at_mut(code.cast(), 4)
            .copy_from_slice(&[42, 0x20, 0x70, 0x47]);
        let function = GuestFunction::from_addr_and_thumb_flag(code.to_bits(), true);
        env.objc
            .borrow_mut::<ClassHostObject>(parent)
            .methods
            .insert(selector, IMP::Guest(function));
        let implementation: IMP = msg![env; child instanceMethodForSelector:selector];
        let IMP::Guest(implementation) = implementation else {
            panic!("Expected real guest implementation");
        };
        assert_eq!(implementation.to_ptr(), function.to_ptr());
        let object: id = msg![env; parent new];
        let instance_implementation: IMP = msg![env; object methodForSelector:selector];
        let IMP::Guest(instance_implementation) = instance_implementation else {
            panic!("Expected guest method address");
        };
        assert_eq!(instance_implementation.to_ptr(), function.to_ptr());
        crate::objc::release(env, object);
        // Step this small program directly: the headless fixture has no
        // scheduler coroutine, which a general call_from_host would require.
        env.cpu
            .branch_with_link(implementation, env.dyld.return_to_host_routine());
        let mut returned = false;
        for _ in 0..8 {
            match env.cpu.run_or_step(&mut env.mem, None) {
                crate::cpu::CpuState::Normal => (),
                crate::cpu::CpuState::Svc(crate::dyld::Dyld::SVC_RETURN_TO_HOST) => {
                    returned = true;
                    break;
                }
                state => panic!("Unexpected CPU state: {state:?}"),
            }
        }
        assert!(returned);
        assert_eq!(env.cpu.regs()[0], 42);
    }
}

/// The Objective-C metadata stores log2(alignment), not byte alignment.
/// UINT32_MAX means the platform's natural pointer alignment.
fn ivar_alignment(raw: u32) -> u32 {
    if raw == u32::MAX {
        4
    } else {
        1u32.checked_shl(raw).expect("Invalid ivar alignment")
    }
}
#[cfg(test)]
mod ivar_alignment_tests {
    #[test]
    fn metadata_alignment_is_an_exponent_with_natural_alignment_sentinel() {
        assert_eq!(super::ivar_alignment(0), 1);
        assert_eq!(super::ivar_alignment(2), 4);
        assert_eq!(super::ivar_alignment(3), 8);
        assert_eq!(super::ivar_alignment(u32::MAX), 4);
    }
}

fn has_sdk_prefix(name: &str) -> bool {
    [
        "AdMob",
        "AdWhirl",
        "AltAds",
        "FB",
        "Flurry",
        "Mobclix",
        "OpenFeint",
        "Tapjoy",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}
fn should_substitute_sdk_class(name: &str, in_image_parent: Option<&str>) -> bool {
    has_sdk_prefix(name) && !in_image_parent.is_some_and(|parent| !has_sdk_prefix(parent))
}
#[cfg(test)]
mod sdk_substitution_tests {
    #[test]
    fn sdk_prefixed_game_subclasses_are_not_silently_deleted() {
        assert!(!super::should_substitute_sdk_class(
            "FlurryGameFlowEvent",
            Some("GameFlowEvent")
        ));
        assert!(!super::should_substitute_sdk_class(
            "FBGameCallback",
            Some("OwnGameCallback")
        ));
        assert!(super::should_substitute_sdk_class("Flurry", None));
        assert!(super::should_substitute_sdk_class(
            "FlurrySession",
            Some("FlurryObject")
        ));
        assert!(super::should_substitute_sdk_class("FBAppEvents", None));
        assert!(!super::should_substitute_sdk_class("GameFlowEvent", None));
    }
}
