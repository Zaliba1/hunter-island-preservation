// SPDX-License-Identifier: MPL-2.0
use super::{invalid, model::Model};
use std::io::{self, Cursor, Read};
use std::sync::Arc;
use std::{collections::HashSet, fs::File};
use zip::ZipArchive;

#[derive(Clone)]
pub(super) enum Artwork {
    Monster,
    HudSymbol,
    Painted,
    Portrait,
    Atlas(Arc<Vec<[usize; 4]>>),
}

impl Artwork {
    pub(super) fn scale(&self) -> usize {
        if matches!(self, Self::Monster) {
            4
        } else {
            2
        }
    }

    pub(super) fn render(&self, model: &Model, src: &[u8], w: usize, h: usize) -> Vec<u8> {
        match self {
            Self::Monster => model.upscale(src, w, h),
            Self::Atlas(frames) => {
                // Duplicate gutter texels without filtering across two different frames.
                let mut out = vec![0; w * h * 16];
                for y in 0..h * 2 {
                    for x in 0..w * 2 {
                        let i = (y / 2 * w + x / 2) * 4;
                        let dst = (y * w * 2 + x) * 4;
                        out[dst..dst + 4].copy_from_slice(&src[i..i + 4]);
                    }
                }
                for &[x, y, fw, fh] in frames.iter() {
                    let mut crop = Vec::with_capacity(fw * fh * 4);
                    for row in y..y + fh {
                        let i = (row * w + x) * 4;
                        crop.extend_from_slice(&src[i..i + fw * 4]);
                    }
                    let result = blended(model, &crop, fw, fh, 8);
                    for (row, pixels) in result.chunks_exact(fw * 8).enumerate() {
                        let i = ((2 * y + row) * 2 * w + 2 * x) * 4;
                        out[i..i + pixels.len()].copy_from_slice(pixels);
                    }
                }
                out
            }
            Self::HudSymbol => upscale_symbol(src, w, h),
            Self::Painted => blended(model, src, w, h, 4),
            Self::Portrait => blended(model, src, w, h, 8),
        }
    }
}

// HUD symbols have hard color boundaries and transparent outlines. Use the
// quarter-pixel phases of a Catmull-Rom kernel, clamped to its central pair.
fn upscale_symbol(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    fn row(src: &[u8], dst: &mut [u8], step: usize) {
        let count = src.len() / step;
        for x in 0..2 * count {
            let left = (x as isize - 1).div_euclid(2);
            let sample = |i: isize, c| {
                i32::from(src[i.clamp(0, count as isize - 1) as usize * step + c])
            };
            for c in 0..step {
                let a = sample(left - 1, c);
                let b = sample(left, c);
                let d = sample(left + 1, c);
                let e = sample(left + 2, c);
                let sum = if x % 2 == 0 {
                    -3 * a + 29 * b + 111 * d - 9 * e
                } else {
                    -9 * a + 111 * b + 29 * d - 3 * e
                };
                dst[x * step + c] = ((sum + 64) / 128).clamp(b.min(d), b.max(d)) as u8;
            }
        }
    }
    let mut wide = vec![0; w * h * 8];
    for (input, output) in src.chunks_exact(w * 4).zip(wide.chunks_exact_mut(w * 8)) {
        row(input, output, 4);
    }
    let mut result = vec![0; w * h * 16];
    row(&wide, &mut result, w * 8);
    for p in result.chunks_exact_mut(4) {
        let alpha = p[3];
        for color in &mut p[..3] {
            *color = (*color).min(alpha);
        }
    }
    result
}

fn blended(model: &Model, src: &[u8], w: usize, h: usize, weight: u32) -> Vec<u8> {
    let ai = model.upscale(src, w, h);
    let mut out = vec![0; w * h * 16];
    // Pixel centers at 2x fall a quarter texel from the nearest source sample.
    for y in 0..2 * h {
        let y0 = y.saturating_sub(1) / 2;
        let y1 = ((y + 1) / 2).min(h - 1);
        let v = if y % 2 == 0 { 3 } else { 1 };
        for x in 0..2 * w {
            let x0 = x.saturating_sub(1) / 2;
            let x1 = ((x + 1) / 2).min(w - 1);
            let u = if x % 2 == 0 { 3 } else { 1 };
            let dst = (y * 2 * w + x) * 4;
            for c in (0..4).rev() {
                let pixel = |xx, yy| u32::from(src[(yy * w + xx) * 4 + c]);
                let original = ((pixel(x0, y0) * (4 - u) + pixel(x1, y0) * u) * (4 - v)
                    + (pixel(x0, y1) * (4 - u) + pixel(x1, y1) * u) * v
                    + 8)
                    / 16;
                out[dst + c] = if c == 3 {
                    original as u8
                } else {
                    let i = (2 * y * 4 * w + 2 * x) * 4 + c;
                    let neural = (u32::from(ai[i])
                        + u32::from(ai[i + 4])
                        + u32::from(ai[i + 16 * w])
                        + u32::from(ai[i + 16 * w + 4])
                        + 2)
                        / 4;
                    (((original * (weight - 1) + neural + weight / 2) / weight) as u8)
                        .min(out[dst + 3])
                };
            }
        }
    }
    out
}

pub(super) fn atlas(
    archive: &mut ZipArchive<File>,
    path: &str,
    w: usize,
    h: usize,
) -> io::Result<Artwork> {
    let mut data = Vec::new();
    archive
        .by_name(&path.replace(".png", ".plist"))?
        .take(512 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 512 * 1024 {
        return Err(invalid());
    }
    let value = plist::Value::from_reader(Cursor::new(data)).map_err(|_| invalid())?;
    let dict = value.as_dictionary().ok_or_else(invalid)?;
    let frames = dict
        .get("frames")
        .and_then(plist::Value::as_dictionary)
        .ok_or_else(invalid)?;
    if frames.is_empty() || frames.len() > 512 {
        return Err(invalid());
    }
    let mut unique = HashSet::new();
    let mut occupied = vec![false; w * h];
    let mut rects = Vec::new();
    for frame in frames.values() {
        let frame = frame.as_dictionary().ok_or_else(invalid)?;
        let packed = frame
            .get("frame")
            .and_then(plist::Value::as_string)
            .ok_or_else(invalid)?;
        let nums: Vec<usize> = packed
            .trim_matches(&['{', '}'][..])
            .split(',')
            .map(|s| {
                s.trim_matches(&['{', '}', ' '][..])
                    .parse()
                    .map_err(|_| invalid())
            })
            .collect::<io::Result<_>>()?;
        let [x, y, mut fw, mut fh] = nums.try_into().map_err(|_| invalid())?;
        if frame
            .get("rotated")
            .and_then(plist::Value::as_boolean)
            .ok_or_else(invalid)?
        {
            std::mem::swap(&mut fw, &mut fh);
        }
        if fw == 0 || fh == 0 || x > w || y > h || fw > w - x || fh > h - y {
            return Err(invalid());
        }
        let rect = [x, y, fw, fh];
        if !unique.insert(rect) {
            continue;
        }
        for row in y..y + fh {
            let span = &mut occupied[row * w + x..row * w + x + fw];
            if span.contains(&true) {
                return Err(invalid());
            }
            span.fill(true);
        }
        rects.push(rect);
    }
    Ok(Artwork::Atlas(Arc::new(rects)))
}

// Limit allocations before passing the image to the native decoder.
pub(super) fn dimensions(data: &[u8]) -> Option<(usize, usize)> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") && data.get(12..16) == Some(b"IHDR") {
        let w = u32::from_be_bytes(data.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(data.get(20..24)?.try_into().ok()?);
        return Some((w as usize, h as usize));
    }
    if !data.starts_with(&[255, 216]) {
        return None;
    }
    let mut i = 2;
    while data.get(i) == Some(&255) {
        while data.get(i) == Some(&255) {
            i += 1;
        }
        let marker = *data.get(i)?;
        i += 1;
        if matches!(marker, 0xda | 0xd9) {
            return None;
        }
        let n = u16::from_be_bytes(data.get(i..i + 2)?.try_into().ok()?) as usize;
        if n < 2 {
            return None;
        }
        let segment = data.get(i..i + n)?;
        if matches!(marker, 0xc0 | 0xc1 | 0xc2) {
            let h = u16::from_be_bytes(segment.get(3..5)?.try_into().ok()?);
            let w = u16::from_be_bytes(segment.get(5..7)?.try_into().ok()?);
            return Some((w as usize, h as usize));
        }
        i += n;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::upscale_symbol;

    #[test]
    fn symbol_edges_remain_bounded_and_centered_on_both_axes() {
        let on = [152, 93, 44, 180];
        let off = [0; 4];
        let line = [off, off, on, on].concat();
        let horizontal = upscale_symbol(&line, 4, 1);
        let vertical = upscale_symbol(&line, 1, 4);
        let coverage: Vec<_> = horizontal[..32].chunks_exact(4).map(|p| p[3]).collect();
        assert_eq!(coverage, [0, 0, 0, 37, 143, 180, 180, 180]);
        for (x, pixel) in horizontal[..32].chunks_exact(4).enumerate() {
            assert_eq!(&vertical[x * 8..x * 8 + 4], pixel);
            assert!(pixel[..3].iter().all(|&c| c <= pixel[3]));
        }
        assert_eq!(upscale_symbol(&on, 1, 1), on.repeat(4));
    }
}
