/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGBitmapContext.h`

use super::cg_affine_transform::{CGAffineTransform, CGAffineTransformIdentity};
use super::cg_color_space::{
    kCGColorSpaceGenericGray, kCGColorSpaceGenericRGB, CGColorSpaceHostObject, CGColorSpaceRef,
};
use super::cg_context::{
    kCGBlendModeCopy, kCGBlendModeDarken, kCGBlendModeLighten, kCGBlendModeMultiply,
    kCGBlendModeNormal, kCGBlendModeScreen, CGBlendMode, CGContextHostObject, CGContextRef,
    CGContextSubclass,
};
use super::cg_image::{
    self, kCGBitmapAlphaInfoMask, kCGBitmapByteOrderMask, kCGImageAlphaFirst, kCGImageAlphaLast,
    kCGImageAlphaNone, kCGImageAlphaNoneSkipFirst, kCGImageAlphaNoneSkipLast, kCGImageAlphaOnly,
    kCGImageAlphaPremultipliedFirst, kCGImageAlphaPremultipliedLast, kCGImageByteOrder32Big,
    kCGImageByteOrderDefault, CGBitmapInfo, CGImageAlphaInfo, CGImageRef,
};
use super::{CGFloat, CGPoint, CGRect};
use crate::dyld::{export_c_func, FunctionExports};
use crate::image::{gamma_decode, gamma_decode_u8, gamma_encode, Image};
use crate::mem::{GuestUSize, Mem, MutVoidPtr, Ptr};
use crate::objc::ObjC;
use crate::Environment;

#[derive(Copy, Clone)]
pub(super) struct CGBitmapContextData {
    pub(super) data: MutVoidPtr,
    pub(super) data_is_owned: bool,
    width: GuestUSize,
    height: GuestUSize,
    bits_per_component: GuestUSize,
    bytes_per_row: GuestUSize,
    color_space: &'static str,
    alpha_info: CGImageAlphaInfo,
}

pub fn CGBitmapContextCreate(
    env: &mut Environment,
    data: MutVoidPtr,
    width: GuestUSize,
    height: GuestUSize,
    bits_per_component: GuestUSize,
    bytes_per_row: GuestUSize,
    color_space: CGColorSpaceRef,
    bitmap_info: u32,
) -> CGContextRef {
    // Invalid bitmap geometry is a creation failure, not a host assertion.
    // Cocos2d can request an empty text texture before assigning label text.
    if width == 0 || height == 0 {
        return Ptr::null();
    }
    assert!(bits_per_component == 8); // TODO: support other bit depths

    let color_space = env.objc.borrow::<CGColorSpaceHostObject>(color_space).name;

    let component_count = match color_space {
        kCGColorSpaceGenericRGB => components_for_rgb(bitmap_info).unwrap(),
        kCGColorSpaceGenericGray => components_for_gray(bitmap_info).unwrap(),
        _ => unimplemented!("support other color spaces"),
    };

    let Some((bytes_per_row, total_size)) = bitmap_layout(
        width,
        height,
        component_count,
        bytes_per_row,
        data.is_null(),
    ) else {
        return Ptr::null();
    };
    let (data, data_is_owned) = if data.is_null() {
        let data = env.mem.alloc(total_size);
        (data, true)
    } else {
        (data, false)
    };

    let host_object = CGContextHostObject {
        subclass: CGContextSubclass::CGBitmapContext(CGBitmapContextData {
            data,
            data_is_owned,
            width,
            height,
            bits_per_component,
            bytes_per_row,
            color_space,
            alpha_info: bitmap_info & kCGBitmapAlphaInfoMask,
        }),
        // TODO: is this the correct default?
        rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
        font: Ptr::null(),
        font_size: 14.0,
        transform: CGAffineTransformIdentity,
        blend_mode: kCGBlendModeNormal,
        text_transform: None,
        state_stack: Vec::new(),
    };
    let isa = env
        .objc
        .get_known_class("_touchHLE_CGContext", &mut env.mem);
    env.objc
        .alloc_object(isa, Box::new(host_object), &mut env.mem)
}

/// Resolve an 8-bit-component bitmap's stride and allocation length. Only
/// Quartz-owned storage permits an automatic (zero) stride; caller-owned
/// buffers must explicitly describe a row large enough for all pixels.
fn bitmap_layout(
    width: GuestUSize,
    height: GuestUSize,
    components: GuestUSize,
    stride: GuestUSize,
    owned: bool,
) -> Option<(GuestUSize, GuestUSize)> {
    if width == 0 || height == 0 || components == 0 {
        return None;
    }
    let minimum = width.checked_mul(components)?;
    let stride = if owned && stride == 0 {
        minimum
    } else {
        stride
    };
    if stride < minimum {
        return None;
    }
    Some((stride, stride.checked_mul(height)?))
}

/// Runtime regression for the bitmap creation path, using only own test data.
pub(crate) fn creation_probe(env: &mut Environment) {
    use crate::frameworks::uikit::ui_geometry::{
        CGPointFromString, CGRectFromString, CGSizeFromString,
    };
    use crate::objc::nil;
    let point = CGPointFromString(env, nil);
    let size = CGSizeFromString(env, nil);
    let rect = CGRectFromString(env, nil);
    assert_eq!(
        (point.x, point.y, size.width, size.height),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        (
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height
        ),
        (0.0, 0.0, 0.0, 0.0)
    );
    use super::cg_color_space::CGColorSpaceCreateDeviceRGB;
    use super::cg_context::CGContextRelease;
    use crate::frameworks::core_foundation::CFRelease;
    let space = CGColorSpaceCreateDeviceRGB(env);
    let buffer = env.mem.alloc(32);
    env.mem.bytes_at_mut(buffer.cast(), 32).fill(0x5a);
    for (data, width, height, stride) in [
        (buffer, 0, 0, 0),
        (buffer, 3, 2, 0),
        (Ptr::null(), 0, 2, 0),
        (buffer, 3, 2, 11),
        (Ptr::null(), u32::MAX, 2, 0),
    ] {
        assert!(CGBitmapContextCreate(
            env,
            data,
            width,
            height,
            8,
            stride,
            space,
            kCGImageAlphaPremultipliedLast
        )
        .is_null());
    }
    let context = CGBitmapContextCreate(
        env,
        buffer,
        3,
        2,
        8,
        16,
        space,
        kCGImageAlphaPremultipliedLast,
    );
    assert!(!context.is_null());
    assert_eq!(CGBitmapContextGetData(env, context), buffer);
    assert_eq!(CGBitmapContextGetBytesPerRow(env, context), 16);
    CGContextRelease(env, context);
    // Releasing a context must not free or overwrite caller-owned storage.
    assert!(env
        .mem
        .bytes_at(buffer.cast(), 32)
        .iter()
        .all(|b| *b == 0x5a));
    env.mem.free(buffer);
    let context = CGBitmapContextCreate(
        env,
        Ptr::null(),
        3,
        2,
        8,
        0,
        space,
        kCGImageAlphaPremultipliedLast,
    );
    assert!(!context.is_null());
    assert_eq!(CGBitmapContextGetBytesPerRow(env, context), 12);
    assert!(!CGBitmapContextGetData(env, context).is_null());
    CGContextRelease(env, context);
    CFRelease(env, space);
    echo!("BITMAP_EMPTY_GEOMETRY_STRIDE_OVERFLOW_AND_OWNERSHIP_PASS");
}

#[cfg(test)]
mod bitmap_layout_tests {
    use super::bitmap_layout;

    #[test]
    fn automatic_and_explicit_padded_rows_preserve_buffer_layout() {
        assert_eq!(bitmap_layout(3, 2, 4, 0, true), Some((12, 24)));
        assert_eq!(bitmap_layout(3, 2, 4, 16, true), Some((16, 32)));
        assert_eq!(bitmap_layout(3, 2, 4, 16, false), Some((16, 32)));
        assert_eq!(bitmap_layout(3, 2, 1, 3, false), Some((3, 6)));
    }

    #[test]
    fn empty_short_and_overflowing_bitmaps_fail_without_panicking() {
        for owned in [false, true] {
            assert_eq!(bitmap_layout(0, 12, 4, 0, owned), None);
            assert_eq!(bitmap_layout(12, 0, 4, 48, owned), None);
            assert_eq!(bitmap_layout(12, 12, 4, 47, owned), None);
            assert_eq!(bitmap_layout(u32::MAX, 1, 4, 0, owned), None);
            assert_eq!(bitmap_layout(1, u32::MAX, 4, 4, owned), None);
        }
        assert_eq!(bitmap_layout(3, 2, 4, 0, false), None);
        assert_eq!(bitmap_layout(3, 2, 0, 12, true), None);
    }
}

#[cfg(test)]
#[test]
fn bitmap_byte_gamma_preserves_channel_layouts_and_alpha() {
    for color_space in [kCGColorSpaceGenericRGB, kCGColorSpaceGenericGray] {
        for alpha_info in [
            kCGImageAlphaNone,
            kCGImageAlphaPremultipliedLast,
            kCGImageAlphaLast,
            kCGImageAlphaPremultipliedFirst,
            kCGImageAlphaFirst,
            kCGImageAlphaNoneSkipLast,
            kCGImageAlphaNoneSkipFirst,
            kCGImageAlphaOnly,
        ] {
            let data = CGBitmapContextData {
                data: crate::mem::Ptr::null(),
                data_is_owned: false,
                width: 1,
                height: 1,
                bits_per_component: 8,
                bytes_per_row: 4,
                color_space,
                alpha_info,
            };
            let (r, g, b, a) = pixel_offsets(&data);
            for value in 0..=255u8 {
                let mut bytes = [
                    value,
                    255 - value,
                    value.wrapping_mul(73),
                    value.wrapping_mul(29),
                ];
                let actual = get_pixel(&data, &mut bytes, 0);
                assert_eq!(
                    actual.0.to_bits(),
                    gamma_decode(bytes[r] as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    actual.1.to_bits(),
                    gamma_decode(bytes[g] as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    actual.2.to_bits(),
                    gamma_decode(bytes[b] as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    actual.3.to_bits(),
                    a.map_or(1.0, |i| bytes[i] as f32 / 255.0).to_bits()
                );
            }
        }
    }
}

fn CGBitmapContextGetData(env: &mut Environment, context: CGContextRef) -> MutVoidPtr {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.data
}

pub fn CGBitmapContextGetWidth(env: &mut Environment, context: CGContextRef) -> GuestUSize {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.width
}

pub fn CGBitmapContextGetHeight(env: &mut Environment, context: CGContextRef) -> GuestUSize {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.height
}

fn CGBitmapContextGetBytesPerRow(env: &mut Environment, context: CGContextRef) -> GuestUSize {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.bytes_per_row
}

pub fn CGBitmapContextCreateImage(env: &mut Environment, context: CGContextRef) -> CGImageRef {
    // TODO: Image::from_pixel_vec() should not exist, and this function should
    // support any bitmap format.
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    assert!(
        bitmap_data.bits_per_component == 8
            && bitmap_data.bytes_per_row == bitmap_data.width * 4
            && bitmap_data.color_space == kCGColorSpaceGenericRGB
            && matches!(
                bitmap_data.alpha_info,
                kCGImageAlphaNoneSkipLast | kCGImageAlphaPremultipliedLast
            )
    );
    let pixels = env
        .mem
        .bytes_at(
            bitmap_data.data.cast(),
            bitmap_data.bytes_per_row * bitmap_data.height,
        )
        .to_vec();
    cg_image::from_image(
        env,
        Image::from_pixel_vec(pixels, (bitmap_data.width, bitmap_data.height)),
    )
}

fn components_for_rgb(bitmap_info: CGBitmapInfo) -> Result<GuestUSize, ()> {
    let byte_order = bitmap_info & kCGBitmapByteOrderMask;
    if byte_order != kCGImageByteOrderDefault && byte_order != kCGImageByteOrder32Big {
        return Err(()); // TODO: handle other byte orders
    }

    let alpha_info = bitmap_info & kCGBitmapAlphaInfoMask;
    if (alpha_info | byte_order) != bitmap_info {
        return Err(()); // TODO: handle other cases (float)
    }
    match alpha_info & kCGBitmapAlphaInfoMask {
        kCGImageAlphaNone => Ok(3), // RGB
        kCGImageAlphaPremultipliedLast
        | kCGImageAlphaPremultipliedFirst
        | kCGImageAlphaLast
        | kCGImageAlphaFirst
        | kCGImageAlphaNoneSkipLast
        | kCGImageAlphaNoneSkipFirst => Ok(4), // RGBA/ARGB/RGBX/XRGB
        kCGImageAlphaOnly => Ok(1), // A
        _ => Err(()),               // unknown values
    }
}

fn components_for_gray(bitmap_info: CGBitmapInfo) -> Result<GuestUSize, ()> {
    let byte_order = bitmap_info & kCGBitmapByteOrderMask;
    if byte_order != kCGImageByteOrderDefault && byte_order != kCGImageByteOrder32Big {
        return Err(()); // TODO: handle other byte orders
    }

    let alpha_info = bitmap_info & kCGBitmapAlphaInfoMask;
    if (alpha_info | byte_order) != bitmap_info {
        return Err(()); // TODO: handle other cases (float)
    }
    match alpha_info & kCGBitmapAlphaInfoMask {
        kCGImageAlphaNone => Ok(1), // gray
        kCGImageAlphaPremultipliedLast
        | kCGImageAlphaPremultipliedFirst
        | kCGImageAlphaLast
        | kCGImageAlphaFirst
        | kCGImageAlphaNoneSkipLast
        | kCGImageAlphaNoneSkipFirst => Ok(2), // gray + alpha
        kCGImageAlphaOnly => Ok(1), // A
        _ => Err(()),               // unknown values
    }
}

fn bytes_per_pixel(data: &CGBitmapContextData) -> GuestUSize {
    let &CGBitmapContextData {
        bits_per_component,
        color_space,
        alpha_info,
        ..
    } = data;
    assert!(bits_per_component == 8);
    match color_space {
        kCGColorSpaceGenericRGB => components_for_rgb(alpha_info).unwrap(),
        kCGColorSpaceGenericGray => components_for_gray(alpha_info).unwrap(),
        _ => unimplemented!("support other color spaces"),
    }
}

fn get_pixels<'a>(data: &CGBitmapContextData, mem: &'a mut Mem) -> &'a mut [u8] {
    let pixel_data_size = data.height.checked_mul(data.bytes_per_row).unwrap();
    mem.bytes_at_mut(data.data.cast(), pixel_data_size)
}

/// Blends two RGBA non gamma-encoded values, with straight alpha,
/// using a blend mode.
fn blend_straight(
    bg: (f32, f32, f32, f32),
    fg: (f32, f32, f32, f32),
    blend_mode: CGBlendMode,
) -> (f32, f32, f32, f32) {
    assert_eq!(blend_mode, kCGBlendModeNormal); // TODO
    if fg.3 == 0.0 {
        // If fg.3 == 0.0 we attempt to blend fully transparent color.
        bg
    } else {
        let neg_fg_a = 1.0 - fg.3;
        let new_a = fg.3 + bg.3 * neg_fg_a; // Can't be 0 if fg.3 != 0
        (
            (fg.0 * fg.3 + bg.0 * bg.3 * neg_fg_a) / new_a,
            (fg.1 * fg.3 + bg.1 * bg.3 * neg_fg_a) / new_a,
            (fg.2 * fg.3 + bg.2 * bg.3 * neg_fg_a) / new_a,
            new_a,
        )
    }
}

/// Blends two RGBA non gamma-encoded values, with premultiplied alpha,
/// using a blend mode.
/// See [Blending](https://www.w3.org/TR/compositing-1/#blending) for details.
fn blend_premultiplied(
    bg: (f32, f32, f32, f32),
    fg: (f32, f32, f32, f32),
    blend_mode: CGBlendMode,
) -> (f32, f32, f32, f32) {
    if blend_mode == kCGBlendModeCopy {
        return fg;
    }
    // Blend
    let blend_res = match blend_mode {
        kCGBlendModeNormal => (bg.3 * fg.0, bg.3 * fg.1, bg.3 * fg.2),
        kCGBlendModeMultiply => (bg.0 * fg.0, bg.1 * fg.1, bg.2 * fg.2),
        kCGBlendModeScreen => (
            fg.3 * bg.0 + bg.3 * fg.0 - bg.0 * fg.0,
            fg.3 * bg.1 + bg.3 * fg.1 - bg.1 * fg.1,
            fg.3 * bg.2 + bg.3 * fg.2 - bg.2 * fg.2,
        ),
        kCGBlendModeDarken => (
            (fg.3 * bg.0).min(bg.3 * fg.0),
            (fg.3 * bg.1).min(bg.3 * fg.1),
            (fg.3 * bg.2).min(bg.3 * fg.2),
        ),
        kCGBlendModeLighten => (
            (fg.3 * bg.0).max(bg.3 * fg.0),
            (fg.3 * bg.1).max(bg.3 * fg.1),
            (fg.3 * bg.2).max(bg.3 * fg.2),
        ),
        _ => unimplemented!("blend mode {}", blend_mode),
    };
    // Compose
    let neg_bg_a = 1.0 - bg.3;
    let neg_fg_a = 1.0 - fg.3;
    (
        fg.0 * neg_bg_a + blend_res.0 + bg.0 * neg_fg_a,
        fg.1 * neg_bg_a + blend_res.1 + bg.1 * neg_fg_a,
        fg.2 * neg_bg_a + blend_res.2 + bg.2 * neg_fg_a,
        fg.3 + bg.3 * neg_fg_a,
    )
}

/// per component offsets (r, g, b, a)
fn pixel_offsets(data: &CGBitmapContextData) -> (usize, usize, usize, Option<usize>) {
    match data.color_space {
        kCGColorSpaceGenericRGB => {
            match data.alpha_info {
                kCGImageAlphaNone => (0, 1, 2, None),
                kCGImageAlphaPremultipliedLast | kCGImageAlphaLast => (0, 1, 2, Some(3)),
                kCGImageAlphaPremultipliedFirst | kCGImageAlphaFirst => (1, 2, 3, Some(0)),
                kCGImageAlphaNoneSkipLast => (0, 1, 2, None),
                kCGImageAlphaNoneSkipFirst => (1, 2, 3, None),
                kCGImageAlphaOnly => (0, 0, 0, Some(0)),
                _ => unreachable!(), // checked by bytes_per_pixel
            }
        }
        kCGColorSpaceGenericGray => {
            // TODO: this is probably isn't doing RGB to grayscale conversion
            // properly
            match data.alpha_info {
                kCGImageAlphaNone => (0, 0, 0, None),
                kCGImageAlphaPremultipliedLast | kCGImageAlphaLast => (0, 0, 0, Some(1)),
                kCGImageAlphaPremultipliedFirst | kCGImageAlphaFirst => (1, 1, 1, Some(0)),
                kCGImageAlphaNoneSkipLast => (0, 0, 0, None),
                kCGImageAlphaNoneSkipFirst => (1, 1, 1, None),
                kCGImageAlphaOnly => (0, 0, 0, Some(0)),
                _ => unreachable!(), // checked by bytes_per_pixel
            }
        }
        _ => unimplemented!(),
    }
}

/// Get gamma-decoded RGBA value.
fn get_pixel(
    data: &CGBitmapContextData,
    pixels: &mut [u8],
    first_component_idx: usize,
) -> (f32, f32, f32, f32) {
    let pixel_offset = pixel_offsets(data);
    let pixel = (
        gamma_decode_u8(pixels[first_component_idx + pixel_offset.0]),
        gamma_decode_u8(pixels[first_component_idx + pixel_offset.1]),
        gamma_decode_u8(pixels[first_component_idx + pixel_offset.2]),
        if let Some(alpha_offest) = pixel_offset.3 {
            pixels[first_component_idx + alpha_offest] as f32 / 255.0
        } else {
            1.0
        },
    );

    pixel
}

fn put_pixel(
    data: &CGBitmapContextData,
    pixels: &mut [u8],
    coords: (i32, i32),
    pixel: (CGFloat, CGFloat, CGFloat, CGFloat),
    blend: bool,
    blend_mode: CGBlendMode,
) {
    let (x, y) = coords;
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as GuestUSize, y as GuestUSize);
    if x >= data.width || y >= data.height {
        return;
    }

    // CG's co-ordinate system puts the origin in the bottom-left corner, but it
    // *seems* like the rows are nonetheless in top-to-bottom order?
    let y = data.height - 1 - y;

    let pixel_size = bytes_per_pixel(data);
    let first_component_idx = (y * data.bytes_per_row + x * pixel_size) as usize;

    // Blending like this must be done in linear RGB, so this must come before
    // gamma encoding.
    let (r, g, b, a) = if blend {
        let bg_pixel = get_pixel(data, pixels, first_component_idx);
        match data.alpha_info {
            kCGImageAlphaLast | kCGImageAlphaFirst => blend_straight(bg_pixel, pixel, blend_mode),
            kCGImageAlphaPremultipliedLast | kCGImageAlphaPremultipliedFirst => {
                blend_premultiplied(bg_pixel, pixel, blend_mode)
            }
            kCGImageAlphaOnly => (
                pixel.0,
                pixel.1,
                pixel.2,
                pixel.3 + bg_pixel.3 * (1.0 - pixel.3),
            ),
            _ => {
                assert_eq!(blend_mode, kCGBlendModeNormal); // TODO
                pixel
            }
        }
    } else {
        pixel
    };

    // Alpha is always linear.
    let (r, g, b) = (gamma_encode(r), gamma_encode(g), gamma_encode(b));
    let pixel_offset = pixel_offsets(data);
    match data.alpha_info {
        kCGImageAlphaOnly => {
            pixels[first_component_idx] = (a * 255.0) as u8;
        }
        _ => {
            pixels[first_component_idx + pixel_offset.0] = (r * 255.0) as u8;
            pixels[first_component_idx + pixel_offset.1] = (g * 255.0) as u8;
            pixels[first_component_idx + pixel_offset.2] = (b * 255.0) as u8;
            if let Some(alpha_offset) = pixel_offset.3 {
                pixels[first_component_idx + alpha_offset] = (a * 255.0) as u8;
            }
        }
    }
}

/// Abstract interface for use by host code that wants to draw in a bitmap
/// context.
pub struct CGBitmapContextDrawer<'a> {
    bitmap_info: CGBitmapContextData,
    rgb_fill_color: (CGFloat, CGFloat, CGFloat, CGFloat),
    blend_mode: CGBlendMode,
    transform: CGAffineTransform,
    pixels: &'a mut [u8],
}
impl CGBitmapContextDrawer<'_> {
    pub fn new<'a>(
        objc: &ObjC,
        mem: &'a mut Mem,
        context: CGContextRef,
    ) -> CGBitmapContextDrawer<'a> {
        let &CGContextHostObject {
            subclass: CGContextSubclass::CGBitmapContext(bitmap_info),
            rgb_fill_color,
            transform,
            blend_mode,
            ..
        } = objc.borrow(context);

        let pixels = get_pixels(&bitmap_info, mem);

        CGBitmapContextDrawer {
            bitmap_info,
            rgb_fill_color,
            blend_mode,
            transform,
            pixels,
        }
    }

    pub fn width(&self) -> GuestUSize {
        self.bitmap_info.width
    }
    pub fn height(&self) -> GuestUSize {
        self.bitmap_info.height
    }
    /// Get the current fill color. The returned color is linear RGB, not sRGB.
    /// It has premultiplied alpha if the context does.
    pub fn rgb_fill_color(&self) -> (CGFloat, CGFloat, CGFloat, CGFloat) {
        let multiply_by = match self.bitmap_info.alpha_info {
            kCGImageAlphaPremultipliedLast | kCGImageAlphaPremultipliedFirst => {
                self.rgb_fill_color.3
            }
            _ => 1.0,
        };
        // Multiplying before decoding matches the Simulator's output.
        (
            gamma_decode(self.rgb_fill_color.0 * multiply_by),
            gamma_decode(self.rgb_fill_color.1 * multiply_by),
            gamma_decode(self.rgb_fill_color.2 * multiply_by),
            self.rgb_fill_color.3, // alpha is always linear
        )
    }
    /// Set the pixel at `coords` to `color`. `color` must be linear RGB, not
    /// sRGB! Note that `coords` are absolute: you must do transformation
    /// yourself.
    pub fn put_pixel(
        &mut self,
        coords: (i32, i32),
        color: (CGFloat, CGFloat, CGFloat, CGFloat),
        blend: bool,
    ) {
        put_pixel(
            &self.bitmap_info,
            self.pixels,
            coords,
            color,
            blend,
            self.blend_mode,
        )
    }

    /// Clear using exactly the existing raster coverage and channel layout.
    /// Transparent black encodes to zero in every channel, so the general
    /// floating-point color/blend path is unnecessary here.
    fn clear_rect(&mut self, rect: CGRect) {
        let data = self.bitmap_info;
        let pixel_size = bytes_per_pixel(&data);
        let (r, g, b, a) = pixel_offsets(&data);
        // Cocos2d commonly clears a complete new texture bitmap. This exact
        // case needs no per-pixel inverse transform or gamma calculation.
        // Keep the general rasterizer for partial/transformed/huge surfaces.
        if data.width > 0
            && data.height > 0
            && data.width <= 16_384
            && data.height <= 16_384
            && self.transform.a == 1.0
            && self.transform.d == 1.0
            && self.transform.b == 0.0
            && self.transform.c == 0.0
            && self.transform.tx == 0.0
            && self.transform.ty == 0.0
            && rect.origin.x == 0.0
            && rect.origin.y == 0.0
            && rect.size.width == data.width as f32
            && rect.size.height == data.height as f32
        {
            let all_channels = ((1u32 << r) | (1 << g) | (1 << b) | a.map_or(0, |a| 1 << a))
                == (1 << pixel_size) - 1;
            for row in self
                .pixels
                .chunks_exact_mut(data.bytes_per_row as usize)
                .take(data.height as usize)
            {
                let active = &mut row[..(data.width * pixel_size) as usize];
                if all_channels {
                    active.fill(0);
                } else {
                    // NoneSkipFirst/Last bytes are caller-owned and unchanged.
                    for pixel in active.chunks_exact_mut(pixel_size as usize) {
                        pixel[r] = 0;
                        pixel[g] = 0;
                        pixel[b] = 0;
                        if let Some(a) = a {
                            pixel[a] = 0;
                        }
                    }
                }
            }
            return;
        }
        for ((x, y), _) in self.iter_transformed_pixels(rect) {
            // Keep put_pixel's checks, including i32 overflow and float-rounded
            // raster bounds for very large contexts.
            if x < 0 || y < 0 || x as u32 >= data.width || y as u32 >= data.height {
                continue;
            }
            let row = data.height - 1 - y as u32;
            let base = (row * data.bytes_per_row + x as u32 * pixel_size) as usize;
            if data.alpha_info == kCGImageAlphaOnly {
                self.pixels[base] = 0;
            } else {
                self.pixels[base + r] = 0;
                self.pixels[base + g] = 0;
                self.pixels[base + b] = 0;
                if let Some(a) = a {
                    self.pixels[base + a] = 0;
                }
            }
        }
    }

    /// Takes a [CGRect] and applies the current transform to it, and iterates
    /// over the transformed, clipped, absolute integer pixel co-ordinates in
    /// raster order for the target bitmap while providing floating-point
    /// co-ordinates from (0,0) to (1,1) as a reference for sampling e.g. a
    /// texture.
    pub fn iter_transformed_pixels(
        &self,
        untransformed_rect: CGRect,
    ) -> impl Iterator<Item = ((i32, i32), (f32, f32))> {
        let bounding_rect = self.transform.apply_to_rect(untransformed_rect);

        let x_start = bounding_rect.origin.x.round().max(0.0) as GuestUSize;
        let y_start = bounding_rect.origin.y.round().max(0.0) as GuestUSize;
        let x_end = (bounding_rect.origin.x + bounding_rect.size.width)
            .round()
            .min(self.width() as f32) as GuestUSize;
        let y_end = (bounding_rect.origin.y + bounding_rect.size.height)
            .round()
            .min(self.height() as f32) as GuestUSize;

        let inverse_transform = self.transform.invert();

        // TODO: Doing a matrix multiply per-pixel is not optimally efficient.
        // A scanline rasterizer would be better, though we should probably use
        // an existing library for this.
        (y_start..y_end).flat_map(move |y| {
            (x_start..x_end).flat_map(move |x| {
                let untransformed = inverse_transform.apply_to_point(CGPoint {
                    x: x as f32 + 0.5,
                    y: y as f32 + 0.5,
                });
                let x_within =
                    (untransformed.x - untransformed_rect.origin.x) / untransformed_rect.size.width;
                let y_within = (untransformed.y - untransformed_rect.origin.y)
                    / untransformed_rect.size.height;
                if !(0.0..1.0).contains(&x_within) || !(0.0..1.0).contains(&y_within) {
                    None
                } else {
                    Some(((x as i32, y as i32), (x_within, y_within)))
                }
            })
        })
    }
}

#[cfg(test)]
#[test]
fn test_iter_transformed_pixels() {
    use super::CGSize;

    fn make_context(
        width: GuestUSize,
        height: GuestUSize,
        transform: CGAffineTransform,
    ) -> CGBitmapContextDrawer<'static> {
        CGBitmapContextDrawer {
            bitmap_info: CGBitmapContextData {
                data: crate::mem::Ptr::null(),
                data_is_owned: false,
                width,
                height,
                bits_per_component: 8,
                bytes_per_row: 3 * width,
                color_space: "kCGColorSpaceGenericRGB",
                alpha_info: 0,
            },
            rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
            blend_mode: kCGBlendModeNormal,
            transform,
            pixels: &mut [],
        }
    }

    let square_2x2_at_0_0 = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 2.0,
            height: 2.0,
        },
    };
    let square_2x2_at_2_2 = CGRect {
        origin: CGPoint { x: 2.0, y: 2.0 },
        size: CGSize {
            width: 2.0,
            height: 2.0,
        },
    };
    let square_4x4_at_0_0 = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 4.0,
            height: 4.0,
        },
    };

    let upright_square_2x2_at_0_0 = [
        ((0, 0), (0.25, 0.25)),
        ((1, 0), (0.75, 0.25)),
        ((0, 1), (0.25, 0.75)),
        ((1, 1), (0.75, 0.75)),
    ];
    let inverted_square_2x2_at_0_0 = [
        ((0, 0), (0.75, 0.75)),
        ((1, 0), (0.25, 0.75)),
        ((0, 1), (0.75, 0.25)),
        ((1, 1), (0.25, 0.25)),
    ];
    let corner_pixel_of_upright_square_2x2_at_1_1 = [((1, 1), (0.25, 0.25))];

    // Constructed by hand so the results are precise
    let rotation_by_180deg = CGAffineTransform {
        a: -1.0,
        c: 0.0,
        b: 0.0,
        d: -1.0,
        tx: 0.0,
        ty: 0.0,
    };

    assert!(make_context(2, 2, CGAffineTransformIdentity)
        .iter_transformed_pixels(square_2x2_at_0_0)
        .eq(upright_square_2x2_at_0_0.into_iter()));
    assert!(
        make_context(2, 2, CGAffineTransform::make_translation(-2.0, -2.0))
            .iter_transformed_pixels(square_2x2_at_2_2)
            .eq(upright_square_2x2_at_0_0.into_iter())
    );
    assert!(
        make_context(2, 2, CGAffineTransform::make_translation(-1.0, -1.0))
            .iter_transformed_pixels(square_2x2_at_2_2)
            .eq(corner_pixel_of_upright_square_2x2_at_1_1.into_iter())
    );
    assert!(make_context(2, 2, CGAffineTransform::make_scale(0.5, 0.5))
        .iter_transformed_pixels(square_4x4_at_0_0)
        .eq(upright_square_2x2_at_0_0.into_iter()));
    assert!(make_context(2, 2, rotation_by_180deg.translate(-2.0, -2.0))
        .iter_transformed_pixels(square_2x2_at_0_0)
        .eq(inverted_square_2x2_at_0_0.into_iter()));
}

/// Implementation of `CGContextFillRect` (`clear` == [false]) and
/// `CGContextClearRect` (`clear` == [true]) for `CGBitmapContext`.
pub(super) fn fill_rect(env: &mut Environment, context: CGContextRef, rect: CGRect, clear: bool) {
    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
    if clear {
        drawer.clear_rect(rect);
        return;
    }
    let color = drawer.rgb_fill_color();
    // TODO: correct anti-aliasing
    for ((x, y), _) in drawer.iter_transformed_pixels(rect) {
        drawer.put_pixel((x, y), color, /* blend: */ true)
    }
}

#[cfg(test)]
mod clear_tests {
    use super::*;
    use crate::frameworks::core_graphics::CGSize;

    fn drawer(
        pixels: &mut [u8],
        data: CGBitmapContextData,
        transform: CGAffineTransform,
    ) -> CGBitmapContextDrawer<'_> {
        CGBitmapContextDrawer {
            bitmap_info: data,
            rgb_fill_color: (0.3, 0.5, 0.8, 0.4),
            blend_mode: kCGBlendModeMultiply,
            transform,
            pixels,
        }
    }
    fn reference_clear(drawer: &mut CGBitmapContextDrawer<'_>, rect: CGRect) {
        for (coords, _) in drawer.iter_transformed_pixels(rect) {
            drawer.put_pixel(coords, std::hint::black_box((0.0, 0.0, 0.0, 0.0)), false);
        }
    }

    #[test]
    fn clear_preserves_reference_pixels_padding_and_skip_channels() {
        let transforms = [
            CGAffineTransformIdentity,
            CGAffineTransform::make_translation(-1.5, 0.75),
            CGAffineTransform::make_scale(0.5, 1.5),
            CGAffineTransform {
                a: -1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: 4.0,
                ty: 0.0,
            },
            CGAffineTransform {
                a: 0.0,
                b: 1.0,
                c: -1.0,
                d: 0.0,
                tx: 4.0,
                ty: 0.0,
            },
            CGAffineTransform {
                a: 1.0,
                b: 0.25,
                c: -0.5,
                d: 1.0,
                tx: 1.0,
                ty: 0.0,
            },
            CGAffineTransform::make_scale(0.0, 0.0),
        ];
        let rectangles = [
            (0.0, 0.0, 5.0, 4.0),
            (-2.0, -1.0, 4.0, 3.0),
            (0.4, 0.7, 3.4, 2.6),
            (0.0, 0.0, 4.999, 4.0),
            (0.0, 0.0, 5.0, 3.999),
            (0.5, 0.5, 3.0, 2.0),
            (9.0, 9.0, 2.0, 2.0),
            (0.0, 0.0, 0.0, 2.0),
            (0.0, 0.0, -2.0, 3.0),
        ];
        for color_space in [kCGColorSpaceGenericRGB, kCGColorSpaceGenericGray] {
            for alpha_info in [
                kCGImageAlphaNone,
                kCGImageAlphaPremultipliedLast,
                kCGImageAlphaLast,
                kCGImageAlphaPremultipliedFirst,
                kCGImageAlphaFirst,
                kCGImageAlphaNoneSkipLast,
                kCGImageAlphaNoneSkipFirst,
                kCGImageAlphaOnly,
            ] {
                let mut data = CGBitmapContextData {
                    data: Ptr::null(),
                    data_is_owned: false,
                    width: 5,
                    height: 4,
                    bits_per_component: 8,
                    bytes_per_row: 0,
                    color_space,
                    alpha_info,
                };
                data.bytes_per_row = data.width * bytes_per_pixel(&data) + 7;
                let original: Vec<u8> = (0..data.bytes_per_row * data.height)
                    .map(|i| (i as u8).wrapping_mul(73).wrapping_add(11))
                    .collect();
                for transform in transforms {
                    for (x, y, width, height) in rectangles {
                        let rect = CGRect {
                            origin: CGPoint { x, y },
                            size: CGSize { width, height },
                        };
                        let mut expected = original.clone();
                        reference_clear(&mut drawer(&mut expected, data, transform), rect);
                        let mut actual = original.clone();
                        drawer(&mut actual, data, transform).clear_rect(rect);
                        assert_eq!(
                            actual, expected,
                            "{color_space}, alpha={alpha_info}, rect=({x},{y},{width},{height})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "explicit host microbenchmark; not a whole-game speed claim"]
    fn benchmark_clear_reference_and_fast_path() {
        let data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 1024,
            height: 1024,
            bits_per_component: 8,
            bytes_per_row: 4112,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaPremultipliedLast,
        };
        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: 1024.0,
                height: 1024.0,
            },
        };
        for run in 0..3 {
            let mut expected = vec![173; (data.bytes_per_row * data.height) as usize];
            let mut actual = expected.clone();
            let start = std::time::Instant::now();
            reference_clear(
                &mut drawer(&mut expected, data, CGAffineTransformIdentity),
                rect,
            );
            let before_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = std::time::Instant::now();
            drawer(&mut actual, data, CGAffineTransformIdentity).clear_rect(rect);
            let after_ms = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(actual, expected);
            println!("BITMAP_CLEAR_BENCH run={run} reference_ms={before_ms:.3} fast_ms={after_ms:.3} bytes_equal=true");
        }
    }
}

/// Implementation of `CGContextDrawImage` for `CGBitmapContext`.
pub(super) fn draw_image(
    env: &mut Environment,
    context: CGContextRef,
    rect: CGRect,
    image: CGImageRef,
) {
    let image = cg_image::borrow_image(&env.objc, image);

    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);

    //let _ = std::fs::write(
    //  format!(
    //      "image-{:?}-{:?}.data",
    //      (image as *const _ as *const ()),
    //      image.dimensions()
    //  ),
    //  image.pixels()
    //);

    //let _ = std::fs::write(
    //  format!(
    //      "bitmap-{:?}-{:?}-before.data",
    //      (image as *const _ as *const ()),
    //      (drawer.width(), drawer.height())
    //  ),
    //  &drawer.pixels
    //);

    let fast = draw_image_on_zero_rgba(&mut drawer, rect, image);
    #[cfg(feature = "guest-cpu-profile")]
    crate::cpu::guest_profile::record_image(
        u64::from(image.dimensions().0) * u64::from(image.dimensions().1),
        fast,
    );
    if fast {
        return;
    }
    draw_image_raster(&mut drawer, rect, image);
}

/// Exact identity-sized upload onto transparent RGBA storage. Re-read the
/// destination: guest code may write through CGBitmapContextGetData at any time.
/// Preserve the old decode/encode round-trip, including its truncation by one.
fn draw_image_on_zero_rgba(
    drawer: &mut CGBitmapContextDrawer<'_>,
    rect: CGRect,
    image: &Image,
) -> bool {
    let data = drawer.bitmap_info;
    let (width, height) = image.dimensions();
    let t = drawer.transform;
    if data.color_space != kCGColorSpaceGenericRGB
        || data.alpha_info != kCGImageAlphaPremultipliedLast
        || data.bits_per_component != 8
        || drawer.blend_mode != kCGBlendModeNormal
        || width == 0
        || height == 0
        || width > 16_384
        || height > 16_384
        || width > data.width
        || height > data.height
        || t.a != 1.0
        || t.d != 1.0
        || t.b != 0.0
        || t.c != 0.0
        || t.tx != 0.0
        || t.ty != 0.0
        || rect.origin.x != 0.0
        || rect.origin.y != 0.0
        || rect.size.width != width as f32
        || rect.size.height != height as f32
    {
        return false;
    }
    let stride = data.bytes_per_row as usize;
    let row_bytes = width as usize * 4;
    let start = (data.height - height) as usize * stride;
    if drawer.pixels[start..]
        .chunks_exact(stride)
        .take(height as usize)
        .any(|row| row[..row_bytes].iter().any(|&byte| byte != 0))
    {
        return false;
    }
    static TABLES: std::sync::OnceLock<([u8; 256], [u8; 256])> = std::sync::OnceLock::new();
    let (rgb, alpha) = TABLES.get_or_init(|| {
        (
            std::array::from_fn(|i| (gamma_encode(gamma_decode_u8(i as u8)) * 255.0) as u8),
            std::array::from_fn(|i| (i as f32 / 255.0 * 255.0) as u8),
        )
    });
    for (src, dst) in image.pixels().chunks_exact(row_bytes).zip(
        drawer.pixels[start..]
            .chunks_exact_mut(stride)
            .take(height as usize),
    ) {
        for (src, dst) in src
            .chunks_exact(4)
            .zip(dst[..row_bytes].chunks_exact_mut(4))
        {
            dst[0] = rgb[src[0] as usize];
            dst[1] = rgb[src[1] as usize];
            dst[2] = rgb[src[2] as usize];
            dst[3] = alpha[src[3] as usize];
        }
    }
    true
}

fn draw_image_raster(drawer: &mut CGBitmapContextDrawer<'_>, rect: CGRect, image: &Image) {
    let (image_width, image_height) = image.dimensions();

    // TODO: non-nearest-neighbour filtering? (what does CG actually do?)

    for ((x, y), (texel_x, texel_y)) in drawer.iter_transformed_pixels(rect) {
        let texel_x = (image_width as f32 * texel_x) as i32;
        // Image is in top-to-bottom order, but the bitmap is bottom-to-top
        let texel_y = (image_height as f32 * (1.0 - texel_y)) as i32;
        // FIXME: might need alpha format conversion here
        if let Some(color) = image.get_pixel((texel_x, texel_y)) {
            drawer.put_pixel((x, y), color, /* blend: */ true)
        }
    }

    //let _ = std::fs::write(
    //  format!(
    //      "bitmap-{:?}-{:?}-after.data",
    //      (image as *const _ as *const ()),
    //      (drawer.width(), drawer.height())
    //  ),
    //  &drawer.pixels
    //);
}

#[allow(rustdoc::broken_intra_doc_links)] // https://github.com/rust-lang/rust/issues/83049
/// Shortcut for [crate::frameworks::core_animation::composition]. This is a
/// workaround for not having a `&mut Environment` that should eventually be
/// removed somehow (TODO).
pub fn get_data(objc: &ObjC, context: CGContextRef) -> (GuestUSize, GuestUSize, MutVoidPtr) {
    let host_obj = objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    (bitmap_data.width, bitmap_data.height, bitmap_data.data)
}

#[cfg(test)]
mod image_draw_tests {
    use super::*;
    use crate::frameworks::core_graphics::CGSize;
    fn run(
        width: u32,
        height: u32,
        pad: u32,
        bg: u8,
        transform: CGAffineTransform,
        scale: f32,
        blend: CGBlendMode,
        alpha: CGImageAlphaInfo,
    ) -> (bool, Vec<u8>, Vec<u8>) {
        let image = Image::from_pixel_vec(
            (0..width * height * 4)
                .map(|i| (i.wrapping_mul(73) + i / 4) as u8)
                .collect(),
            (width, height),
        );
        let data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: width + 2,
            height: height + 3,
            bits_per_component: 8,
            bytes_per_row: (width + 2) * 4 + pad,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: alpha,
        };
        let mut before = vec![bg; (data.height * data.bytes_per_row) as usize];
        // Make row padding and untouched POT margins observable.
        for row in before.chunks_exact_mut(data.bytes_per_row as usize) {
            row[(width * 4) as usize..].fill(0xa5);
        }
        let mut expected = before.clone();
        let mut actual = before;
        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: width as f32 * scale,
                height: height as f32 * scale,
            },
        };
        let drawer = |pixels| CGBitmapContextDrawer {
            bitmap_info: data,
            rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
            blend_mode: blend,
            transform,
            pixels,
        };
        draw_image_raster(&mut drawer(&mut expected), rect, &image);
        let mut target = drawer(&mut actual);
        let fast = draw_image_on_zero_rgba(&mut target, rect, &image);
        if !fast {
            draw_image_raster(&mut target, rect, &image);
        }
        (fast, expected, actual)
    }
    #[test]
    fn image_upload_matches_raster_with_alpha_padding_and_fallbacks() {
        for (w, h) in [(1, 1), (3, 7), (257, 19), (513, 257), (1024, 1019)] {
            for pad in [0, 12] {
                let (fast, expected, actual) = run(
                    w,
                    h,
                    pad,
                    0,
                    CGAffineTransformIdentity,
                    1.0,
                    kCGBlendModeNormal,
                    kCGImageAlphaPremultipliedLast,
                );
                assert!(fast);
                assert_eq!(actual, expected, "{w}x{h} pad={pad}");
            }
        }
        for (bg, t, scale, blend, alpha) in [
            (
                1,
                CGAffineTransformIdentity,
                1.0,
                kCGBlendModeNormal,
                kCGImageAlphaPremultipliedLast,
            ),
            (
                0,
                CGAffineTransform::make_translation(1.0, 2.0),
                1.0,
                kCGBlendModeNormal,
                kCGImageAlphaPremultipliedLast,
            ),
            (
                0,
                CGAffineTransformIdentity,
                0.5,
                kCGBlendModeNormal,
                kCGImageAlphaPremultipliedLast,
            ),
            (
                0,
                CGAffineTransformIdentity,
                1.0,
                kCGBlendModeMultiply,
                kCGImageAlphaPremultipliedLast,
            ),
            (
                0,
                CGAffineTransformIdentity,
                1.0,
                kCGBlendModeNormal,
                kCGImageAlphaFirst,
            ),
        ] {
            let (fast, expected, actual) = run(31, 17, 8, bg, t, scale, blend, alpha);
            assert!(!fast);
            assert_eq!(actual, expected);
        }
    }
    #[test]
    fn zero_background_roundtrip_is_exact_for_every_channel_and_alpha() {
        let data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 256,
            height: 256,
            bits_per_component: 8,
            bytes_per_row: 1024,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaPremultipliedLast,
        };
        let mut bytes = Vec::new();
        for a in 0..=255 {
            for c in 0..=255 {
                bytes.extend_from_slice(&[c, 255 - c, c ^ 0x55, a]);
            }
        }
        let image = Image::from_pixel_vec(bytes, (256, 256));
        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: 256.0,
                height: 256.0,
            },
        };
        let mut expected = vec![0; 256 * 1024];
        let mut actual = expected.clone();
        let drawer = |pixels| CGBitmapContextDrawer {
            bitmap_info: data,
            rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
            blend_mode: kCGBlendModeNormal,
            transform: CGAffineTransformIdentity,
            pixels,
        };
        draw_image_raster(&mut drawer(&mut expected), rect, &image);
        assert!(draw_image_on_zero_rgba(
            &mut drawer(&mut actual),
            rect,
            &image
        ));
        assert_eq!(actual, expected);
    }
    #[test]
    #[ignore = "explicit routine benchmark, not a game-wide measurement"]
    fn benchmark_identity_image_upload() {
        let data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 1024,
            height: 1024,
            bits_per_component: 8,
            bytes_per_row: 4096,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaPremultipliedLast,
        };
        let image = Image::from_pixel_vec(
            (0..1024 * 1019 * 4u32)
                .map(|i| i.wrapping_mul(73) as u8)
                .collect(),
            (1024, 1019),
        );
        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: 1024.0,
                height: 1019.0,
            },
        };
        let mut old_times = Vec::new();
        let mut new_times = Vec::new();
        for _ in 0..11 {
            let mut old = vec![0; 1024 * 4096];
            let mut new = old.clone();
            let drawer = |pixels| CGBitmapContextDrawer {
                bitmap_info: data,
                rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
                blend_mode: kCGBlendModeNormal,
                transform: CGAffineTransformIdentity,
                pixels,
            };
            let start = std::time::Instant::now();
            draw_image_raster(
                &mut drawer(&mut old),
                std::hint::black_box(rect),
                std::hint::black_box(&image),
            );
            old_times.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = std::time::Instant::now();
            assert!(draw_image_on_zero_rgba(
                &mut drawer(&mut new),
                std::hint::black_box(rect),
                std::hint::black_box(&image)
            ));
            new_times.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(old, new);
        }
        old_times.sort_by(f64::total_cmp);
        new_times.sort_by(f64::total_cmp);
        eprintln!("IMAGE_UPLOAD_BENCH: pixels=1043456 trials=11 old_median_ms={:.3} new_median_ms={:.3} identical=true",old_times[5],new_times[5]);
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGBitmapContextCreate(_, _, _, _, _, _, _)),
    export_c_func!(CGBitmapContextCreateImage(_)),
    export_c_func!(CGBitmapContextGetData(_)),
    export_c_func!(CGBitmapContextGetWidth(_)),
    export_c_func!(CGBitmapContextGetHeight(_)),
    export_c_func!(CGBitmapContextGetBytesPerRow(_)),
];
