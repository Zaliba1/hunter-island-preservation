// SPDX-License-Identifier: MPL-2.0
//! CPU inference for the fixed Real-ESRGAN animevideov3 ×4 SRVGG network.
//! All supported targets can use this path without a graphics compute API.

const WEIGHTS: &[u8] = include_bytes!("realesr-animevideov3-x4.bin");
const DEPTH: usize = 18;
const BLOCK: usize = 16;
const TILE: usize = 96;

#[cfg(test)]
#[test]
fn matches_upstream_animev3_on_synthetic_edges() {
    let mut image = Vec::new();
    for y in 0..48 {
        for x in 0..48 {
            image.extend([x*3+y, x+4*y, if x>y {175} else {50}, 255]);
        }
    }
    let output = Model::load().upscale(&image,48,48);
    // Center samples from the upstream v0.2.5.0 NCNN Vulkan executable.
    for (x,y,expected) in [(80,86,[96,105,33]),(104,96,[109,126,189]),
        (91,93,[107,111,46]),(95,94,[96,111,92]),(96,94,[98,109,139]),(95,96,[111,109,59])] {
        let offset = (y*192+x)*4;
        for c in 0..3 {
            assert!(output[offset+c].abs_diff(expected[c])<=5,"({x},{y}): {:?} vs {expected:?}",&output[offset..offset+3]);
        }
        assert_eq!(output[offset+3],255);
    }
}

struct Layer {
    input: usize,
    output: usize,
    weights: Vec<[f32; BLOCK]>,
    bias: Vec<f32>,
    slope: Vec<f32>,
}

pub struct Model(Vec<Layer>);

fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let mantissa = (bits & 1023) as f32;
    let exponent = (bits >> 10) & 31;
    if exponent == 0 { sign * mantissa * 2.0f32.powi(-24) }
    else { sign * (1.0 + mantissa / 1024.0) * 2.0f32.powi(i32::from(exponent) - 15) }
}

impl Model {
    pub fn load() -> Self {
        let mut bytes = WEIGHTS;
        let mut layers = Vec::new();
        for i in 0..DEPTH {
            let input = if i == 0 { 3 } else { 64 };
            let output = if i == DEPTH - 1 { 48 } else { 64 };
            // ncnn float16 weights, then float32 convolution bias and PReLU slopes.
            assert_eq!(&bytes[..4], &[0x47, 0x6b, 0x30, 0x01]);
            bytes = &bytes[4..];
            let mut weights = vec![[0.0; BLOCK]; output / BLOCK * 9 * input];
            for out in 0..output {
                for channel in 0..input {
                    for tap in 0..9 {
                        let idx = (out / BLOCK * 9 + tap) * input + channel;
                        weights[idx][out % BLOCK] = half(u16::from_le_bytes([bytes[0],bytes[1]]));
                        bytes = &bytes[2..];
                    }
                }
            }
            let mut floats = |count| (0..count).map(|_| {
                let f = f32::from_le_bytes(bytes[..4].try_into().unwrap());
                bytes = &bytes[4..];
                f
            }).collect();
            let bias = floats(output);
            let slope = floats(if i == DEPTH-1 { 0 } else { output });
            layers.push(Layer { input, output, weights, bias, slope });
        }
        assert!(bytes.is_empty());
        Self(layers)
    }

    /// Source and result are premultiplied RGBA. Neural inference operates on straight RGB.
    pub fn upscale(&self, src: &[u8], w: usize, h: usize) -> Vec<u8> {
        assert_eq!(src.len(), w*h*4);
        let mut out = vec![0; w*h*64];
        for y in (0..h).step_by(TILE) {
            for x in (0..w).step_by(TILE) {
                let right = (x+TILE).min(w);
                let bottom = (y+TILE).min(h);
                let left = x.saturating_sub(DEPTH);
                let top = y.saturating_sub(DEPTH);
                let tw = (right+DEPTH).min(w)-left;
                let th = (bottom+DEPTH).min(h)-top;
                let mut rgb = Vec::with_capacity(tw*th*3);
                for row in top..top+th {
                    for p in src[(row*w+left)*4..(row*w+left+tw)*4].chunks_exact(4) {
                        let a = f32::from(p[3].max(1));
                        rgb.extend(p[..3].iter().map(|&v| f32::from(v)/a));
                    }
                }
                let mut features = rgb.clone();
                for layer in &self.0 {
                    features = layer.forward(&features,tw,th);
                }
                for row in y..bottom {
                    for col in x..right {
                        let src_i = (row-top)*tw + col-left;
                        for dy in 0..4 {
                            for dx in 0..4 {
                                let xx = col*4+dx;
                                let yy = row*4+dy;
                                let a = alpha(src,w,h,xx,yy);
                                let p = &mut out[(yy*w*4 + xx)*4..][..4];
                                for c in 0..3 {
                                    let v = rgb[src_i*3+c] + features[src_i*48+c*16+dy*4+dx];
                                    p[c] = (v.clamp(0.0,1.0)*a).round() as u8;
                                }
                                p[3] = a.round() as u8;
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

// Bilinear coverage avoids ringing outside sprite outlines.
fn alpha(src: &[u8], w: usize, h: usize, x: usize, y: usize) -> f32 {
    let fx = ((x as f32+0.5)/4.0-0.5).clamp(0.0,(w-1) as f32);
    let fy = ((y as f32+0.5)/4.0-0.5).clamp(0.0,(h-1) as f32);
    let (x,y) = (fx as usize,fy as usize);
    let (u,v) = (fx-x as f32,fy-y as f32);
    let at = |i,j| f32::from(src[(j*w+i)*4+3]);
    let nx = (x+1).min(w-1);
    let ny = (y+1).min(h-1);
    (at(x,y)*(1.0-u)+at(nx,y)*u)*(1.0-v) + (at(x,ny)*(1.0-u)+at(nx,ny)*u)*v
}

impl Layer {
    fn forward(&self, src: &[f32], w: usize, h: usize) -> Vec<f32> {
        let mut out = vec![0.; w*h*self.output];
        // Pack a row of 3x3 neighborhoods once, then accumulate output channels
        // in contiguous blocks. LLVM can vectorize the block on ARM64 and x86.
        let k = self.input*9;
        let mut row = vec![0.; w*k];
        for y in 0..h {
            for x in 0..w {
                let patch = &mut row[x*k..(x+1)*k];
                for dy in 0..3 {
                    for dx in 0..3 {
                        let ix = (x+dx).wrapping_sub(1);
                        let iy = (y+dy).wrapping_sub(1);
                        let start = (dy*3+dx)*self.input;
                        let dest = &mut patch[start..start+self.input];
                        if ix<w && iy<h {
                            let p = (iy*w+ix)*self.input;
                            dest.copy_from_slice(&src[p..p+self.input]);
                        } else { dest.fill(0.); }
                    }
                }
            }
            for block in 0..self.output/BLOCK {
                let offset = block*BLOCK;
                let weights = &self.weights[block*k..(block+1)*k];
                let bias: [f32; BLOCK] = self.bias[offset..offset+BLOCK].try_into().unwrap();
                for (x,patch) in row.chunks_exact(k).enumerate() {
                    let mut sum = bias;
                    for (&v,filter) in patch.iter().zip(weights) {
                        for lane in 0..BLOCK { sum[lane] += v*filter[lane]; }
                    }
                    if !self.slope.is_empty() {
                        for (v,&s) in sum.iter_mut().zip(&self.slope[offset..]) {
                            if *v<0.0 { *v *= s; }
                        }
                    }
                    let p = (y*w+x)*self.output+offset;
                    out[p..p+BLOCK].copy_from_slice(&sum);
                }
            }
        }
        out
    }
}
