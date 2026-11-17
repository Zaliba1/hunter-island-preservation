/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use super::*;
use crate::objc::HostObject;

#[derive(Default)]
struct Delegate {
    ranges: Vec<(u32, u32, String)>,
    changed: usize,
    ended: usize,
    veto: bool,
    veto_end: bool,
    destroy_on_end: bool,
}
impl HostObject for Delegate {}
pub(super) const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation _touchHLE_TextViewProbe: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::<Delegate>::default(), &mut env.mem) }
- (bool)textView:(id)view shouldChangeTextInRange:(NSRange)range replacementText:(id)replacement {
    let text = to_rust_string(env, replacement).into_owned();
    env.objc.borrow_mut::<Delegate>(this).ranges.push((range.location, range.length, text.clone()));
    if text == "\n" { let _: bool = msg![env; view resignFirstResponder]; return false; }
    !env.objc.borrow::<Delegate>(this).veto
}
- (())textViewDidChange:(id)_view { env.objc.borrow_mut::<Delegate>(this).changed += 1; }
- (bool)textViewShouldEndEditing:(id)_view { !env.objc.borrow::<Delegate>(this).veto_end }
- (())textViewDidEndEditing:(id)view {
    env.objc.borrow_mut::<Delegate>(this).ended += 1;
    let _: bool = msg![env; view resignFirstResponder]; // Original game's reentrant callback.
    if env.objc.borrow::<Delegate>(this).destroy_on_end {
        () = msg![env; view removeFromSuperview];
        () = msg![env; view setDelegate:nil];
        release(env, view); // Drop caller's ownership while the handler is active.
    }
}
@end
};

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
fn view(env: &mut Environment) -> id {
    let view: id = msg_class![env; UITextView alloc];
    msg![env; view initWithFrame:(CGRect { origin: CGPointZero, size: CGSize { width: 200.0, height: 80.0 } })]
}
fn text(env: &mut Environment, view: id) -> String {
    let text: id = msg![env; view text];
    to_rust_string(env, text).into_owned()
}

#[test]
fn unicode_paste_backspace_delegate_veto_and_return_cleanup() {
    let mut environment = environment();
    let env = &mut environment;
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let view = view(env);
    let delegate: id = msg_class![env; _touchHLE_TextViewProbe new];
    () = msg![env; view setDelegate:delegate];
    assert!(msg![env; view becomeFirstResponder]);
    assert!(msg![env; view isFirstResponder]);
    handle_input(env, view, TextInputEvent::Text("a😀".to_owned()));
    assert_eq!(text(env, view), "a😀");
    handle_input(env, view, TextInputEvent::Backspace);
    assert_eq!(text(env, view), "a");
    assert_eq!(
        env.objc.borrow::<Delegate>(delegate).ranges[1],
        (1, 2, String::new())
    );
    env.objc.borrow_mut::<Delegate>(delegate).veto = true;
    handle_input(env, view, TextInputEvent::Text("blocked".to_owned()));
    assert_eq!(text(env, view), "a");
    assert_eq!(env.objc.borrow::<Delegate>(delegate).changed, 2);
    env.objc.borrow_mut::<Delegate>(delegate).veto_end = true;
    assert!(!msg![env; view resignFirstResponder]);
    assert!(msg![env; view isFirstResponder]);
    env.objc.borrow_mut::<Delegate>(delegate).veto_end = false;
    env.objc.borrow_mut::<Delegate>(delegate).destroy_on_end = true;
    handle_input(env, view, TextInputEvent::Return);
    assert_eq!(env.framework_state.uikit.ui_responder.first_responder, nil);
    assert_eq!(env.objc.borrow::<Delegate>(delegate).ended, 1);
    assert!(env.objc.get_host_object(view).is_none());
    release(env, delegate);
    release(env, pool);
}

#[test]
fn editable_defaults_and_switching_responder_preserve_text() {
    let mut environment = environment();
    let env = &mut environment;
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let a = view(env);
    let b = view(env);
    assert!(msg![env; a isEditable]);
    assert!(msg![env; a becomeFirstResponder]);
    handle_input(env, a, TextInputEvent::Text("dragon island".to_owned()));
    assert!(msg![env; b becomeFirstResponder]);
    assert!(!msg![env; a isFirstResponder]);
    handle_input(env, a, TextInputEvent::Text("ignored".to_owned()));
    assert_eq!(text(env, a), "dragon island");
    () = msg![env; b setEditable:false];
    assert!(!msg![env; b becomeFirstResponder]);
    assert_eq!(env.framework_state.uikit.ui_responder.first_responder, nil);
    () = msg![env; a setText:nil];
    assert_eq!(text(env, a), "");
    release(env, a);
    release(env, b);
    release(env, pool);
}
