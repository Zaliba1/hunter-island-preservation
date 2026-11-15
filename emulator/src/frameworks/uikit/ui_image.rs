/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIImage`.

use crate::frameworks::core_graphics::cg_context::CGContextDrawImage;
use crate::frameworks::core_graphics::cg_image::{
    self, CGImageRef, CGImageRelease, CGImageRetain,
};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::{ns_data, ns_string, NSInteger};
use crate::frameworks::uikit::ui_graphics::UIGraphicsGetCurrentContext;
use crate::fs::GuestPath;
use crate::image::Image;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;
use std::collections::HashMap;

const CACHE_SIZE: usize = 60;

#[derive(Default)]
pub struct State {
    /// Cache of images for `[UIImage imageNamed:]` method.
    /// Images are explicitly retained.
    cached_images: HashMap<String, id>,
}
impl State {
    fn get(env: &Environment) -> &Self {
        &env.framework_state.uikit.ui_image
    }
    fn get_mut(env: &mut Environment) -> &mut Self {
        &mut env.framework_state.uikit.ui_image
    }
}

struct UIImageHostObject {
    cg_image: CGImageRef,
    oriented_image: CGImageRef,
    scale: CGFloat,
    orientation: NSInteger,
}
impl HostObject for UIImageHostObject {}

fn oriented_dimensions((width, height): (u32, u32), orientation: NSInteger) -> (u32, u32) {
    match orientation {
        2 | 3 | 6 | 7 => (height, width),
        _ => (width, height),
    }
}

/// Drawing applies UIImage orientation. The public CGImage property still
/// exposes the original pixels, including when the caller retained that image.
pub(super) fn drawing_image(env: &mut Environment, image: id) -> CGImageRef {
    if image == nil {
        return nil;
    }
    let host = env.objc.borrow::<UIImageHostObject>(image);
    if host.orientation == 0 || host.cg_image == nil {
        return host.cg_image;
    }
    if host.oriented_image != nil {
        return host.oriented_image;
    }
    let orientation = host.orientation;
    let source = cg_image::borrow_image(&env.objc, host.cg_image);
    let (width, height) = source.dimensions();
    let (out_width, out_height) = oriented_dimensions((width, height), orientation);
    let mut pixels = Vec::with_capacity(source.pixels().len());
    for y in 0..out_height {
        for x in 0..out_width {
            let (sx, sy) = match orientation {
                1 => (width - 1 - x, height - 1 - y), // Down
                2 => (width - 1 - y, x),             // Left
                3 => (y, height - 1 - x),            // Right
                4 => (width - 1 - x, y),             // UpMirrored
                5 => (x, height - 1 - y),            // DownMirrored
                6 => (y, x),                         // LeftMirrored
                7 => (width - 1 - y, height - 1 - x), // RightMirrored
                _ => unreachable!(),
            };
            let index = (sy as usize * width as usize + sx as usize) * 4;
            pixels.extend_from_slice(&source.pixels()[index..index + 4]);
        }
    }
    let oriented = cg_image::from_image(env, Image::from_pixel_vec(pixels, (out_width, out_height)));
    env.objc.borrow_mut::<UIImageHostObject>(image).oriented_image = oriented;
    oriented
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIImage: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIImageHostObject {
        cg_image: nil,
        oriented_image: nil,
        scale: 1.0,
        orientation: 0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)imageWithCGImage:(CGImageRef)cg_image {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCGImage:cg_image];
    autorelease(env, new)
}

+ (id)imageWithCGImage:(CGImageRef)cg_image scale:(CGFloat)scale orientation:(NSInteger)orientation {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCGImage:cg_image scale:scale orientation:orientation];
    autorelease(env, new)
}

+ (id)imageNamed:(id)name { // NSString*
    // TODO: figure out whether this is actually correct in all cases
    let bundle: id = msg_class![env; NSBundle mainBundle];
    let path: id = msg![env; bundle pathForResource:name ofType:nil];
    let name_str = ns_string::to_rust_string(env, name).to_string();
    if path == nil {
        log!("Warning: [UIImage imageNamed:{:?}] => nil", name_str);
        return nil;
    }
    // TODO: find a better eviction policy
    if State::get(env).cached_images.len() > CACHE_SIZE {
        let cache = std::mem::take(&mut State::get_mut(env).cached_images);
        log_dbg!("Evicting {} images from UIImage cache.", cache.len());
        for (_, img) in cache {
            release(env, img);
        }
    }
    if !State::get(env).cached_images.contains_key(&name_str) {
        let img = msg![env; this imageWithContentsOfFile:path];
        retain(env, img);
        State::get_mut(env).cached_images.insert(name_str.clone(), img);
    }
    *State::get(env).cached_images.get(&name_str).unwrap()
}

+ (id)imageWithContentsOfFile:(id)path { // NSString*
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfFile:path];
    autorelease(env, new)
}

+ (id)imageWithData:(id)data { // NSData*
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithData:data];
    autorelease(env, new)
}

- (())dealloc {
    let &UIImageHostObject { cg_image, oriented_image, .. } = env.objc.borrow(this);
    CGImageRelease(env, cg_image);
    CGImageRelease(env, oriented_image);

    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)initWithCGImage:(CGImageRef)cg_image {
    msg![env; this initWithCGImage:cg_image scale:1.0f32 orientation:0i32]
}

- (id)initWithCGImage:(CGImageRef)cg_image scale:(CGFloat)scale orientation:(NSInteger)orientation {
    if cg_image == nil {
        release(env, this);
        return nil;
    }
    CGImageRetain(env, cg_image);
    let host = env.objc.borrow_mut::<UIImageHostObject>(this);
    let old_image = std::mem::replace(&mut host.cg_image, cg_image);
    let old_oriented = std::mem::replace(&mut host.oriented_image, nil);
    host.scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    host.orientation = if (0..=7).contains(&orientation) { orientation } else { 0 };
    CGImageRelease(env, old_image);
    CGImageRelease(env, old_oriented);
    this
}

- (id)initWithContentsOfFile:(id)path { // NSString*
    if path == nil {
        return nil;
    }
    let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
    let Ok(bytes) = env.fs.read(GuestPath::new(&path)) else {
        log!("Warning: couldn't read image file at {:?}, returning nil", path);
        release(env, this);
        return nil;
    };
    // TODO: Real error handling. For now, most errors are likely to be caused
    //       by a functionality gap in touchHLE, not the app actually trying to
    //       load a broken file, so panicking is most useful.
    let image = Image::from_bytes(&bytes).unwrap();
    let cg_image = cg_image::from_image(env, image);
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)initWithData:(id)data { // NSData*
    let slice = ns_data::to_rust_slice(env, data);
    // TODO: refactor common parts
    let image = Image::from_bytes(slice).unwrap();
    let cg_image = cg_image::from_image(env, image);
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)stretchableImageWithLeftCapWidth:(NSInteger)_leftCapWidth
                          topCapHeight:(NSInteger)_topCapHeight {
    log!("TODO: properly support stretchableImageWithLeftCapWidth:topCapHeight:");
    retain(env, this)
}

// TODO: more init methods
// TODO: more accessors

- (CGImageRef)CGImage {
    env.objc.borrow::<UIImageHostObject>(this).cg_image
}

- (NSInteger)imageOrientation {
    env.objc.borrow::<UIImageHostObject>(this).orientation
}

- (CGSize)size {
    let host = env.objc.borrow::<UIImageHostObject>(this);
    let dimensions = cg_image::borrow_image(&env.objc, host.cg_image).dimensions();
    let (width, height) = oriented_dimensions(dimensions, host.orientation);
    CGSize {
        width: width as CGFloat / host.scale,
        height: height as CGFloat / host.scale,
    }
}

- (CGFloat)scale {
    env.objc.borrow::<UIImageHostObject>(this).scale
}

- (())drawInRect:(CGRect)rect {
    let context = UIGraphicsGetCurrentContext(env);
    let image = drawing_image(env, this);
    draw_ui_image(env, context, rect, image);
}

- (())drawAtPoint:(CGPoint)point {
    let context = UIGraphicsGetCurrentContext(env);
    if context == nil {
        log!("Warning: [(UIImage*){:?} drawAtPoint:{:?}] is called with nil context, ignoring.", this, point);
        return;
    }
    let size: CGSize = msg![env; this size];
    let image = drawing_image(env, this);
    let rect = CGRect {
        origin: point,
        size,
    };
    draw_ui_image(env, context, rect, image);
}

@end

// Undocumented class used in NIBs
// TODO: It's not clear _why_ placeholder is needed?
@implementation UIImageNibPlaceholder: UIImage

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    release(env, this);

    // TODO: decode other attributes
    let key_ns_string = get_static_str(env, "UIResourceName");
    let resource_name: id = msg![env; coder decodeObjectForKey:key_ns_string];

    let res = msg_class![env; UIImage imageNamed:resource_name];
    // TODO: It is not clear if we need to additionally retain here?
    retain(env, res)
}

@end

};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_cg_image_initializer_preserves_raw_image_and_draws_oriented_pixels() {
        let mut env = Environment::new_without_app(
            crate::options::Options { headless: true, ..Default::default() },
            Image::from_pixel_vec(vec![0; 4], (1, 1)),
        ).unwrap();
        let env = &mut env;
        let pixels = [1, 2, 3, 4, 5, 6].into_iter().flat_map(|red| [red, 0, 0, 255]).collect();
        let source = cg_image::from_image(env, Image::from_pixel_vec(pixels, (3, 2)));
        let object: id = msg_class![env; UIImage alloc];
        let image: id = msg![env; object initWithCGImage:source scale:2.0f32 orientation:3i32];
        assert_eq!(object, image);
        CGImageRelease(env, source);
        let raw: CGImageRef = msg![env; image CGImage];
        let scale: CGFloat = msg![env; image scale];
        let orientation: NSInteger = msg![env; image imageOrientation];
        let size: CGSize = msg![env; image size];
        assert_eq!(raw, source);
        assert_eq!(scale, 2.0);
        assert_eq!(orientation, 3);
        assert_eq!(size, CGSize { width: 1.0, height: 1.5 });
        let oriented = drawing_image(env, image);
        let drawn = cg_image::borrow_image(&env.objc, oriented);
        assert_eq!(drawn.dimensions(), (2, 3));
        assert_eq!(drawn.pixels().iter().step_by(4).copied().collect::<Vec<_>>(), [4, 1, 5, 2, 6, 3]);
        assert_eq!(drawing_image(env, image), oriented);
        release(env, image);
        assert!(env.objc.get_host_object(source).is_none());
        assert!(env.objc.get_host_object(oriented).is_none());
    }
}

/// UIImage compensates for the top-left UIKit image-context CTM; raw
/// CGContextDrawImage intentionally retains Quartz's bottom-left convention.
fn draw_ui_image(env: &mut Environment, context: crate::frameworks::core_graphics::cg_context::CGContextRef, rect: CGRect, image: CGImageRef) {
    use crate::frameworks::core_graphics::cg_context::{CGContextSaveGState,CGContextRestoreGState,CGContextTranslateCTM,CGContextScaleCTM};
    if context==nil {return;}
    if super::ui_graphics::is_image_context(env,context) {
        CGContextSaveGState(env,context);
        CGContextTranslateCTM(env,context,rect.origin.x,rect.origin.y+rect.size.height);
        CGContextScaleCTM(env,context,1.,-1.);
        CGContextDrawImage(env,context,CGRect{origin:CGPoint::default(),size:rect.size},image);
        CGContextRestoreGState(env,context);
    } else {CGContextDrawImage(env,context,rect,image);}
}
