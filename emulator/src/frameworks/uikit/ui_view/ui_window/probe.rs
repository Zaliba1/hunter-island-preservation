/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Own trusted controllers for checking window ownership and appearance.
use super::*;
use crate::frameworks::core_graphics::{CGPoint, CGSize};
use crate::frameworks::uikit::ui_view_controller::UIViewControllerHostObject;
use crate::Environment;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Default)]
struct ProbeController {
    superclass: UIViewControllerHostObject,
    events: Vec<&'static str>,
    destroyed: Option<Arc<AtomicUsize>>,
}
impl_HostObject_with_superclass!(ProbeController);
impl Drop for ProbeController {
    fn drop(&mut self) {
        if let Some(counter) = &self.destroyed {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    }
}
pub(crate) const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation _touchHLE_WindowProbeController: UIViewController
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::<ProbeController>::default(), &mut env.mem)
}
- (())loadView {
    env.objc.borrow_mut::<ProbeController>(this).events.push("load");
    let view: id = msg_class![env; UIView alloc];
    let view: id = msg![env; view initWithFrame:(CGRect::default())];
    () = msg![env; this setView:view];
    release(env, view);
}
- (())viewDidLoad { env.objc.borrow_mut::<ProbeController>(this).events.push("loaded"); }
- (())viewWillAppear:(bool)_animated { env.objc.borrow_mut::<ProbeController>(this).events.push("will-appear"); }
- (())viewDidAppear:(bool)_animated { env.objc.borrow_mut::<ProbeController>(this).events.push("appeared"); }
- (())viewWillDisappear:(bool)_animated { env.objc.borrow_mut::<ProbeController>(this).events.push("will-disappear"); }
- (())viewDidDisappear:(bool)_animated { env.objc.borrow_mut::<ProbeController>(this).events.push("disappeared"); }
@end
@implementation _touchHLE_LandscapeMaskProbe: UIViewController
- (u32)supportedInterfaceOrientations { 24 }
@end
};

fn events(env: &mut Environment, controller: id) -> Vec<&'static str> {
    std::mem::take(&mut env.objc.borrow_mut::<ProbeController>(controller).events)
}
fn exercise(env: &mut Environment) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let destroyed = Arc::new(AtomicUsize::new(0));
    let window: id = msg_class![env; UIWindow alloc];
    let frame = CGRect {
        origin: CGPoint { x: 10.0, y: 20.0 },
        size: CGSize {
            width: 320.0,
            height: 480.0,
        },
    };
    let window: id = msg![env; window initWithFrame:frame];
    let initial: id = msg![env; window rootViewController];
    assert_eq!(initial, nil);
    let extra: id = msg_class![env; UIView alloc];
    let extra: id = msg![env; extra initWithFrame:(CGRect::default())];
    () = msg![env; window addSubview:extra];
    let first: id = msg_class![env; _touchHLE_WindowProbeController new];
    env.objc.borrow_mut::<ProbeController>(first).destroyed = Some(destroyed.clone());
    () = msg![env; window setRootViewController:first];
    assert_eq!(events(env, first), ["load", "loaded"]);
    let selected: id = msg![env; window rootViewController];
    assert_eq!(selected, first);
    let count: u32 = msg![env; first retainCount];
    assert_eq!(count, 2);
    () = msg![env; window setRootViewController:first];
    let count: u32 = msg![env; first retainCount];
    assert_eq!(count, 2);
    assert!(events(env, first).is_empty());
    let view: id = msg![env; first view];
    let parent: id = msg![env; view superview];
    assert_eq!(parent, window);
    let removed: id = msg![env; extra superview];
    assert_eq!(removed, nil);
    let actual: CGRect = msg![env; view frame];
    assert_eq!(actual.origin, CGPoint::default());
    assert_eq!(actual.size, frame.size);
    let responder: id = msg![env; view nextResponder];
    assert_eq!(responder, first);
    () = msg![env; window makeKeyAndVisible];
    assert_eq!(events(env, first), ["will-appear", "appeared"]);
    () = msg![env; window makeKeyAndVisible];
    assert!(events(env, first).is_empty());
    () = msg![env; window setHidden:true];
    assert_eq!(events(env, first), ["will-disappear", "disappeared"]);
    () = msg![env; window setHidden:false];
    assert_eq!(events(env, first), ["will-appear", "appeared"]);
    let second: id = msg_class![env; _touchHLE_WindowProbeController new];
    env.objc.borrow_mut::<ProbeController>(second).destroyed = Some(destroyed.clone());
    () = msg![env; window setRootViewController:second];
    assert_eq!(events(env, first), ["will-disappear", "disappeared"]);
    assert_eq!(
        events(env, second),
        ["load", "loaded", "will-appear", "appeared"]
    );
    let parent: id = msg![env; view superview];
    assert_eq!(parent, nil);
    let second_view: id = msg![env; second view];
    let new_frame = CGRect {
        origin: CGPoint::default(),
        size: CGSize {
            width: 300.0,
            height: 400.0,
        },
    };
    () = msg![env; window setBounds:new_frame];
    () = msg![env; window layoutSubviews];
    let actual: CGRect = msg![env; second_view frame];
    assert_eq!(actual.size, new_frame.size);
    () = msg![env; window setRootViewController:nil];
    assert_eq!(events(env, second), ["will-disappear", "disappeared"]);
    assert!(env
        .objc
        .borrow::<UIViewHostObject>(window)
        .subviews
        .is_empty());
    release(env, first);
    release(env, second);
    assert_eq!(destroyed.load(Ordering::SeqCst), 2);
    let third: id = msg_class![env; _touchHLE_WindowProbeController new];
    env.objc.borrow_mut::<ProbeController>(third).destroyed = Some(destroyed.clone());
    () = msg![env; window setRootViewController:third];
    release(env, third);
    assert_eq!(destroyed.load(Ordering::SeqCst), 2);
    release(env, window);
    assert_eq!(destroyed.load(Ordering::SeqCst), 3);
    assert_eq!(env.framework_state.uikit.ui_view.ui_window.key_window, None);
    assert!(!env
        .framework_state
        .uikit
        .ui_view
        .ui_window
        .windows
        .contains(&window));
    release(env, extra);
    release(env, pool);
    echo!("UIWINDOW_ROOT_CONTROLLER_PASS: ownership, load/mount/resize, appearance, replacement, removal and destruction verified");
}

pub fn framework_probe() -> Result<(), String> {
    let mut env = Environment::new_without_app(
        crate::options::Options::default(),
        crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
    )?;
    exercise(&mut env);
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn ios6_landscape_mask_and_legacy_portrait_policy_are_distinct() {
        use super::*;
        let mut environment=crate::Environment::new_without_app(
            crate::options::Options{headless:true,..Default::default()},
            crate::image::Image::from_pixel_vec(vec![0;4],(1,1)),
        ).unwrap();let env=&mut environment;
        let pool:id=msg_class![env; NSAutoreleasePool new];
        let modern:id=msg_class![env; _touchHLE_LandscapeMaskProbe new];
        for o in [3,4] {assert!(super::super::controller_accepts_orientation(env,modern,o));}
        for o in [-1,0,1,2,5] {assert!(!super::super::controller_accepts_orientation(env,modern,o));}
        let legacy:id=msg_class![env; _touchHLE_WindowProbeController new];
        assert!(super::super::controller_accepts_orientation(env,legacy,1));
        assert!(!super::super::controller_accepts_orientation(env,legacy,3));
        release(env,modern);release(env,legacy);release(env,pool);
    }
    #[test]
    fn root_controller_ownership_and_lifecycle() {
        let mut env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        super::exercise(&mut env);
    }
}
