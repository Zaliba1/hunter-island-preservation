/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! UIKit context stacks and bitmap image contexts, in logical points.
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::core_graphics::cg_context::{
    CGContextRef, CGContextRelease, CGContextRetain, CGContextClearRect,
    CGContextTranslateCTM, CGContextScaleCTM,
};
use crate::frameworks::core_graphics::cg_bitmap_context::{CGBitmapContextCreate,CGBitmapContextCreateImage};
use crate::frameworks::core_graphics::cg_color_space::{CGColorSpaceCreateDeviceRGB,CGColorSpaceRelease};
use crate::frameworks::core_graphics::cg_image::{CGImageRelease,kCGImageAlphaPremultipliedLast,kCGImageAlphaNoneSkipLast};
use crate::objc::{id,msg,msg_class,nil};
use crate::mem::MutPtr;
use crate::Environment;

#[derive(Default)]
pub(super) struct State {
    pub(super) context_stack: Vec<CGContextRef>,
    image_contexts: Vec<(CGContextRef,CGFloat)>,
}

pub fn UIGraphicsPushContext(env: &mut Environment, context: CGContextRef) {
    CGContextRetain(env,context);
    env.framework_state.uikit.ui_graphics.context_stack.push(context);
}
pub fn UIGraphicsPopContext(env: &mut Environment) {
    let context=env.framework_state.uikit.ui_graphics.context_stack.pop();
    CGContextRelease(env,context.unwrap());
}
pub fn UIGraphicsGetCurrentContext(env: &mut Environment) -> CGContextRef {
    env.framework_state.uikit.ui_graphics.context_stack.last().copied().unwrap_or(nil)
}
pub(super) fn is_image_context(env: &Environment, context: CGContextRef) -> bool {
    env.framework_state.uikit.ui_graphics.image_contexts.iter().any(|&(c,_)|c==context)
}

/// UIKit uses a top-left origin, and a zero scale selects the screen scale.
/// Transitions between contexts retain the previous drawing target.
fn UIGraphicsBeginImageContextWithOptions(env: &mut Environment, size: CGSize, opaque: bool, scale: CGFloat) {
    let scale=if scale==0. {
        let screen:id=msg_class![env; UIScreen mainScreen];msg![env; screen scale]
    } else {scale};
    if !size.width.is_finite() || !size.height.is_finite() || !scale.is_finite() ||
       size.width<=0. || size.height<=0. || scale<=0. {return;}
    let width=(size.width*scale).ceil() as u32;
    let height=(size.height*scale).ceil() as u32;
    let space=CGColorSpaceCreateDeviceRGB(env);
    let bitmap=if opaque {kCGImageAlphaNoneSkipLast} else {kCGImageAlphaPremultipliedLast};
    let context=CGBitmapContextCreate(env,MutPtr::null(),width,height,8,0,space,bitmap);
    CGColorSpaceRelease(env,space);
    if context==nil {return;}
    CGContextClearRect(env,context,CGRect{origin:CGPoint::default(),size:CGSize{width:width as f32,height:height as f32}});
    CGContextTranslateCTM(env,context,0.,height as f32);
    CGContextScaleCTM(env,context,scale,-scale);
    UIGraphicsPushContext(env,context);
    env.framework_state.uikit.ui_graphics.image_contexts.push((context,scale));
    CGContextRelease(env,context);
}
fn UIGraphicsBeginImageContext(env: &mut Environment,size:CGSize) {
    UIGraphicsBeginImageContextWithOptions(env,size,false,1.);
}
fn UIGraphicsGetImageFromCurrentImageContext(env: &mut Environment) -> id {
    let context=UIGraphicsGetCurrentContext(env);
    let Some(&(created,scale))=env.framework_state.uikit.ui_graphics.image_contexts.last() else {return nil;};
    if context!=created {return nil;}
    let image=CGBitmapContextCreateImage(env,context);
    let result:id=msg_class![env; UIImage imageWithCGImage:image scale:scale orientation:0i32];
    CGImageRelease(env,image);
    result
}
fn UIGraphicsEndImageContext(env: &mut Environment) {
    let Some(&(created,_))=env.framework_state.uikit.ui_graphics.image_contexts.last() else {return;};
    if UIGraphicsGetCurrentContext(env)!=created {return;}
    env.framework_state.uikit.ui_graphics.image_contexts.pop();
    UIGraphicsPopContext(env);
}
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(UIGraphicsPushContext(_)),
    export_c_func!(UIGraphicsPopContext()),
    export_c_func!(UIGraphicsGetCurrentContext()),
    export_c_func!(UIGraphicsBeginImageContext(_)),
    export_c_func!(UIGraphicsBeginImageContextWithOptions(_,_,_)),
    export_c_func!(UIGraphicsGetImageFromCurrentImageContext()),
    export_c_func!(UIGraphicsEndImageContext()),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::core_graphics::{cg_context::*,cg_image};
    use crate::objc::release;
    #[test]
    fn scaled_nested_opaque_image_contexts_keep_top_left_coordinates_and_ownership() {
        for (retina,requested,expected,opaque) in [(false,1.,1.,false),(false,2.,2.,true),(true,0.,2.,false)] {
            let mut environment=Environment::new_without_app(
                crate::options::Options{headless:true,experimental_retina:retina,..Default::default()},
                crate::image::Image::from_pixel_vec(vec![0;4],(1,1)),
            ).unwrap();let env=&mut environment;
            let pool:id=msg_class![env; NSAutoreleasePool new];
            assert_eq!(UIGraphicsGetImageFromCurrentImageContext(env),nil);
            UIGraphicsBeginImageContextWithOptions(env,CGSize{width:2.,height:2.},opaque,requested);
            let outer=UIGraphicsGetCurrentContext(env);assert_ne!(outer,nil);
            UIGraphicsBeginImageContext(env,CGSize{width:1.,height:1.});
            assert_ne!(UIGraphicsGetCurrentContext(env),outer);
            UIGraphicsEndImageContext(env);assert_eq!(UIGraphicsGetCurrentContext(env),outer);
            CGContextSetRGBFillColor(env,outer,1.,0.,0.,1.);
            CGContextFillRect(env,outer,CGRect{origin:CGPoint::default(),size:CGSize{width:2.,height:1.}});
            CGContextSetRGBFillColor(env,outer,0.,0.,1.,1.);
            CGContextFillRect(env,outer,CGRect{origin:CGPoint{x:0.,y:1.},size:CGSize{width:2.,height:1.}});
            let image=UIGraphicsGetImageFromCurrentImageContext(env);
            let actual:f32=msg![env; image scale];assert_eq!(actual,expected);
            let size:CGSize=msg![env; image size];assert_eq!(size,CGSize{width:2.,height:2.});
            let cg:id=msg![env; image CGImage];
            let backing=cg_image::borrow_image(&env.objc,cg);
            let pixels=backing.pixels();assert_eq!(&pixels[..4],&[255,0,0,255]);
            assert_eq!(&pixels[pixels.len()-4..],&[0,0,255,255]);
            UIGraphicsEndImageContext(env);
            assert_eq!(UIGraphicsGetCurrentContext(env),nil);
            assert!(env.objc.get_host_object(outer).is_none());
            release(env,pool);
        }
    }
    #[test]
    fn drawing_uiimage_into_an_image_context_preserves_image_orientation() {
        let mut environment=Environment::new_without_app(
            crate::options::Options{headless:true,..Default::default()},
            crate::image::Image::from_pixel_vec(vec![0;4],(1,1)),
        ).unwrap();let env=&mut environment;
        let pool:id=msg_class![env; NSAutoreleasePool new];
        let pixels=vec![255,0,0,255,0,0,255,255];
        let source=cg_image::from_image(env,crate::image::Image::from_pixel_vec(pixels.clone(),(1,2)));
        let ui:id=msg_class![env; UIImage imageWithCGImage:source];
        UIGraphicsBeginImageContext(env,CGSize{width:1.,height:2.});
        ()=msg![env; ui drawInRect:(CGRect{origin:CGPoint::default(),size:CGSize{width:1.,height:2.}})];
        let result=UIGraphicsGetImageFromCurrentImageContext(env);
        let cg:id=msg![env; result CGImage];
        assert_eq!(cg_image::borrow_image(&env.objc,cg).pixels(),pixels);
        UIGraphicsEndImageContext(env);CGImageRelease(env,source);release(env,pool);
    }
}
