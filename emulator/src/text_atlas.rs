// SPDX-License-Identifier: MPL-2.0
//! Hunter Island's dialog uses a shipped bitmap font with fixed glyph metrics.
use crate::image::Image;
use crate::Environment;
use std::io::Read;

pub(crate) struct TextAtlas {
    alpha: Vec<u8>,
    glyphs: Vec<[usize; 4]>,
}

const SIDE: usize = 512;

impl TextAtlas {
    pub(crate) fn load(env: &Environment) -> Option<Self> {
        let resource = |name: &str, limit| {
            let path = env.bundle.bundle_path().join(name);
            let file = env.fs.open(path).ok()?;
            let mut bytes = Vec::new();
            file.take(limit + 1).read_to_end(&mut bytes).ok()?;
            (bytes.len() as u64 <= limit).then_some(bytes)
        };
        let png = resource("arial_18_bold_0-hd.png", 2 * 1024 * 1024)?;
        // Bound the allocation before decoding the local font page.
        if !png.starts_with(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR")
            || png.get(16..24)? != [0, 0, 2, 0, 0, 0, 2, 0]
        {
            return None;
        }
        let image = Image::from_bytes(&png).ok()?;
        let alpha = image.pixels().chunks_exact(4).map(|p| p[3]).collect();
        let descriptor = resource("arial_18_bold-hd.fnt", 512 * 1024)?;
        let text = std::str::from_utf8(&descriptor).ok()?;
        let mut glyphs = Vec::new();
        let mut total_area = 0;
        for line in text.lines().filter_map(|l| l.strip_prefix("char ")) {
            let number = |key: &str| {
                line.split_ascii_whitespace()
                    .find_map(|s| s.strip_prefix(key))?
                    .parse::<usize>()
                    .ok()
            };
            let (x, y, w, h) = (
                number("x=")?,
                number("y=")?,
                number("width=")?,
                number("height=")?,
            );
            if number("page=")? != 0
                || x > SIDE
                || y > SIDE
                || w > SIDE - x
                || h > SIDE - y
                || glyphs.len() >= 4096
            {
                return None;
            }
            total_area += w * h;
            if total_area > 2 * SIDE * SIDE {
                return None;
            }
            if w > 0 && h > 0 {
                glyphs.push([x, y, w, h]);
            }
        }
        (!glyphs.is_empty()).then_some(Self { alpha, glyphs })
    }

    // Match the complete page at upload, including coverage. Accept straight
    // or premultiplied white; keep the game's blend convention in the result.
    pub(crate) fn replacement(&self, raw: &[u8]) -> Option<Vec<u8>> {
        if raw.len() != 4 * self.alpha.len() {
            return None;
        }
        let (mut premult, mut straight) = (true, true);
        for (p, &a) in raw.chunks_exact(4).zip(&self.alpha) {
            if p[3] != a {
                return None;
            }
            premult &= p[..3] == [a; 3];
            straight &= a == 0 || p[..3] == [255; 3];
            if !premult && !straight {
                return None;
            }
        }
        let mut pixels = vec![0; 4 * SIDE * SIDE * 4];
        if !premult {
            for p in pixels.chunks_exact_mut(4) {
                p[..3].fill(255);
            }
        }
        for &[left, top, w, h] in &self.glyphs {
            for y in 0..h {
                for x in 0..w {
                    let at = |xx: usize, yy: usize| self.alpha[(top + yy) * SIDE + left + xx];
                    let a = at(x, y);
                    let (l, r) = (x.saturating_sub(1), (x + 1).min(w - 1));
                    let (t, b) = (y.saturating_sub(1), (y + 1).min(h - 1));
                    let dx = i32::from(at(r, t)) + 2 * i32::from(at(r, y)) + i32::from(at(r, b))
                        - i32::from(at(l, t))
                        - 2 * i32::from(at(l, y))
                        - i32::from(at(l, b));
                    let dy = i32::from(at(l, b)) + 2 * i32::from(at(x, b)) + i32::from(at(r, b))
                        - i32::from(at(l, t))
                        - 2 * i32::from(at(x, t))
                        - i32::from(at(r, t));
                    for (i, sub) in coverage_2x(a, dx, dy).into_iter().enumerate() {
                        let index =
                            ((2 * (top + y) + i / 2) * 2 * SIDE + 2 * (left + x) + i % 2) * 4;
                        let color = if premult { sub } else { 255 };
                        pixels[index..index + 4].copy_from_slice(&[color, color, color, sub]);
                    }
                }
            }
        }
        Some(pixels)
    }
}

// Reconstruct a straight local edge with the input pixel's covered area.
// A line projected onto a square has a trapezoidal coverage distribution.
fn coverage_2x(alpha: u8, dx: i32, dy: i32) -> [u8; 4] {
    if alpha == 0 || alpha == 255 || (dx == 0 && dy == 0) {
        return [alpha; 4];
    }
    let (x, y) = (dx.abs() as f32, dy.abs() as f32);
    let (hi, lo) = (x.max(y), x.min(y));
    let p = f32::from(alpha) / 255.;
    let cut = lo / (2. * hi);
    let limit = if p < cut {
        (2. * hi * lo * p).sqrt()
    } else if p > 1. - cut {
        hi + lo - (2. * hi * lo * (1. - p)).sqrt()
    } else {
        hi * p + lo / 2.
    };
    std::array::from_fn(|i| {
        let offset_x = if (i % 2 == 0) == (dx > 0) { x } else { 0. };
        let offset_y = if (i / 2 == 0) == (dy > 0) { y } else { 0. };
        let t = (2. * limit - offset_x - offset_y).clamp(0., hi + lo);
        let area = if lo == 0. {
            (t / hi).clamp(0., 1.)
        } else if t < lo {
            t * t / (2. * hi * lo)
        } else if t > hi {
            1. - (hi + lo - t).powi(2) / (2. * hi * lo)
        } else {
            (t - lo / 2.) / hi
        };
        (area * 255.).round() as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_ink_and_edge_direction() {
        for (gx, gy) in [(0, 0), (8, 0), (0, -8), (98, 253), (-937, 55), (2, -2)] {
            for a in 0..=255 {
                let block = coverage_2x(a, gx, gy);
                assert!(
                    (block.iter().map(|v| i32::from(*v)).sum::<i32>() - 4 * i32::from(a)).abs()
                        <= 2
                );
            }
        }
        assert_eq!(coverage_2x(128, 10, 0), [1, 255, 1, 255]);
    }
}
