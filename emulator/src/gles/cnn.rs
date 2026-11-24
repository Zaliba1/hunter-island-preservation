// SPDX-License-Identifier: MPL-2.0
//! Desktop RGB adapter for the two bundled mpv neural enlargement shaders.
//! Feature maps use signed half floats, independent of the guest GL state.
use super::{gl, Program, UpscaleMode};
use crate::matrix::Matrix;
use std::ffi::CString;
use std::ptr;

const ANIME: &str = include_str!("filters/Anime4K_Upscale_CNN_x2_S.glsl");
const FSRCNNX: &str = include_str!("filters/FSRCNNX_x2_8-0-4-1.glsl");
// Slot 0 is oriented, top-down RGB; slot 7 is the final 2x RGB image.
// The bundled graphs reuse the other slots only after their last consumer.
fn slot(name: &str) -> usize {
    match name {
        "MAIN" | "LUMA" => 0,
        "conv2d_tf" | "conv2d_2_tf" | "FEATURE1" => 1,
        "conv2d_1_tf" | "conv2d_last_tf" | "FEATURE2" => 2,
        "MODEL21" | "RES1" => 3,
        "MODEL22" | "RES2" => 4,
        "MODEL1" | "SUBCONV1" => 5,
        "MODEL2" => 6,
        _ => unreachable!("unknown bundled feature map"),
    }
}
const VERT: &str =
    "#version 120\nvarying vec2 uv; void main(){gl_Position=gl_Vertex; uv=gl_Vertex.xy*0.5+0.5;}";
const HEADER: &str = "#version 120\nvarying vec2 uv; uniform vec2 imageSize;\n";
struct Pass {
    program: Program,
    inputs: Vec<(usize, i32)>,
    output: usize,
}

impl Pass {
    unsafe fn new(fragment: &str, names: &[&str], output: usize) -> Result<Self, String> {
        let program = Program::link(VERT, fragment)?;
        let inputs = names
            .iter()
            .map(|name| {
                let u = CString::new(format!("tex_{name}")).unwrap();
                (slot(name), gl::GetUniformLocation(program.id, u.as_ptr()))
            })
            .collect();
        Ok(Self {
            program,
            inputs,
            output,
        })
    }
}
struct Pipeline(Vec<Pass>);
impl Pipeline {
    unsafe fn compile(anime: bool) -> Result<Self, String> {
        let src = if anime { ANIME } else { FSRCNNX };
        let parts: Vec<_> = src.split("//!HOOK ").skip(1).collect();
        let mut passes = Vec::<Pass>::new();
        for (i, block) in parts.iter().enumerate() {
            let final_pass = i + 1 == parts.len();
            let mut names: Vec<_> = block
                .lines()
                .filter_map(|l| l.strip_prefix("//!BIND "))
                .map(str::trim)
                .collect();
            if !anime && final_pass {
                names.push("MAIN");
            }
            let output = if final_pass {
                7
            } else {
                slot(
                    block
                        .lines()
                        .find_map(|l| l.strip_prefix("//!SAVE "))
                        .unwrap()
                        .trim(),
                )
            };
            assert!(!names.iter().any(|n| slot(n) == output));
            let mut fragment = String::from(HEADER);
            for name in &names {
                fragment.push_str(&format!("uniform sampler2D tex_{name};\n#define {name}_pos uv\n#define {name}_pt (1.0/imageSize)\n#define {name}_size imageSize\n"));
                let sample = format!("texture2D(tex_{name},(p))");
                let value = if *name == "LUMA" {
                    format!("vec4(dot({sample}.rgb,vec3(.2126,.7152,.0722)))")
                } else {
                    sample
                };
                fragment.push_str(&format!("#define {name}_tex(p) {value}\n#define {name}_texOff(o) {name}_tex(uv+vec2(o)/imageSize)\n"));
            }
            for line in block.lines().skip(1).filter(|l| !l.starts_with("//!")) {
                fragment.push_str(line);
                fragment.push('\n');
            }
            if final_pass && !anime {
                fragment.push_str("void main(){vec3 rgb=MAIN_tex(uv).rgb; float y=dot(rgb,vec3(.2126,.7152,.0722)); gl_FragColor=vec4(rgb+(hook().r-y),1.0);}");
            } else {
                fragment.push_str("void main(){gl_FragColor=hook();}");
            }
            match Pass::new(&fragment, &names, output) {
                Ok(pass) => passes.push(pass),
                Err(e) => {
                    for p in passes {
                        gl::DeleteProgram(p.program.id);
                    }
                    return Err(format!("pass {}: {e}", i + 1));
                }
            }
        }
        Ok(Self(passes))
    }
}

#[derive(Default)]
pub(super) struct Presenter {
    anime: Option<Option<Pipeline>>,
    fsrcnnx: Option<Option<Pipeline>>,
    copy: Option<Option<Program>>,
    fbo: u32,
    textures: [u32; 8],
    sizes: [Option<((u32, u32), bool)>; 8],
}

fn cached<T>(cache: &mut Option<Option<T>>, init: impl FnOnce() -> Result<T, String>) -> bool {
    cache
        .get_or_insert_with(|| {
            init()
                .map_err(|e| echo!("Neural upscale unavailable: {e}"))
                .ok()
        })
        .is_some()
}

impl Presenter {
    unsafe fn target(&mut self, i: usize, size: (u32, u32)) -> bool {
        if let Some((current, ready)) = self.sizes[i] {
            if current == size {
                return ready;
            }
        }
        #[cfg(target_os = "android")]
        {
            // A failed reallocation must not leave the previous size attached.
            gl::DeleteTextures(1, &self.textures[i]);
            gl::GenTextures(1, &mut self.textures[i]);
        }
        gl::BindTexture(gl::TEXTURE_2D, self.textures[i]);
        for param in [gl::TEXTURE_MIN_FILTER, gl::TEXTURE_MAG_FILTER] {
            gl::TexParameteri(gl::TEXTURE_2D, param, gl::LINEAR as i32);
        }
        for param in [gl::TEXTURE_WRAP_S, gl::TEXTURE_WRAP_T] {
            gl::TexParameteri(gl::TEXTURE_2D, param, gl::CLAMP_TO_EDGE as i32);
        }
        // GL_RGBA16F is also available through ARB_texture_float on GL 2.1.
        let format = if i == 0 || i == 7 { gl::RGBA8 } else { 0x881a };
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            format as i32,
            size.0 as i32,
            size.1 as i32,
            0,
            gl::RGBA,
            gl::FLOAT,
            ptr::null(),
        );
        gl::FramebufferTexture2DEXT(
            gl::FRAMEBUFFER_EXT,
            gl::COLOR_ATTACHMENT0_EXT,
            gl::TEXTURE_2D,
            self.textures[i],
            0,
        );
        #[cfg(not(target_os = "android"))]
        let allocated = {
        let mut width = 0;
        let mut height = 0;
        gl::GetTexLevelParameteriv(gl::TEXTURE_2D, 0, gl::TEXTURE_WIDTH, &mut width);
        gl::GetTexLevelParameteriv(gl::TEXTURE_2D, 0, gl::TEXTURE_HEIGHT, &mut height);
        (width as u32, height as u32) == size
        };
        // GLES 3.0 has no texture-level query. Each target size is checked by
        // attaching a freshly allocated image and checking framebuffer status.
        #[cfg(target_os = "android")]
        let allocated = true;
        let ready = allocated && gl::CheckFramebufferStatusEXT(gl::FRAMEBUFFER_EXT) == gl::FRAMEBUFFER_COMPLETE_EXT;
        self.sizes[i] = Some((size, ready));
        ready
    }

    pub(super) unsafe fn draw(
        &mut self,
        mode: UpscaleMode,
        src: (u32, u32),
        vp: (u32, u32, u32, u32),
        rot: Matrix<2>,
    ) -> bool {
        let anime = mode == UpscaleMode::Anime4K;
        let size = if rot.columns()[0][0].abs() < 0.5 {
            (src.1, src.0)
        } else {
            src
        };
        let threshold = if anime { 1.2 } else { 1.3 };
        if vp.2 as f64 <= size.0 as f64 * threshold
            || vp.3 as f64 <= size.1 as f64 * threshold
            || !gl::BindFramebufferEXT::is_loaded()
            || !gl::GenFramebuffersEXT::is_loaded()
            || !gl::FramebufferTexture2DEXT::is_loaded()
            || !gl::CheckFramebufferStatusEXT::is_loaded()
        {
            return false;
        }
        // Match the activation thresholds in the upstream shaders.
        if !cached(&mut self.copy, || {
            Program::link(VERT,&format!("{HEADER}uniform sampler2D image; uniform mat2 sourceRotation; void main(){{gl_FragColor=texture2D(image,fract(sourceRotation*vec2(uv.x,1.-uv.y)));}}"))
        }) || !cached(
            if anime {
                &mut self.anime
            } else {
                &mut self.fsrcnnx
            },
            || Pipeline::compile(anime),
        ) {
            return false;
        }
        let mut max = 0;
        gl::GetIntegerv(gl::MAX_TEXTURE_SIZE, &mut max);
        if size.0.max(size.1) > max as u32 / 2 {
            return false;
        }
        let mut old_unit = 0;
        let mut old_fbo = 0;
        let mut old_program = 0;
        let mut src_tex = 0;
        gl::GetIntegerv(gl::ACTIVE_TEXTURE, &mut old_unit);
        gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut src_tex);
        gl::GetIntegerv(gl::FRAMEBUFFER_BINDING_EXT, &mut old_fbo);
        gl::GetIntegerv(gl::CURRENT_PROGRAM, &mut old_program);
        let mut bound = [0; 3];
        for (i, b) in bound.iter_mut().enumerate() {
            gl::ActiveTexture(gl::TEXTURE0 + i as u32);
            gl::GetIntegerv(gl::TEXTURE_BINDING_2D, b);
        }
        gl::ActiveTexture(gl::TEXTURE0);
        if self.fbo == 0 {
            gl::GenFramebuffersEXT(1, &mut self.fbo);
            gl::GenTextures(8, self.textures.as_mut_ptr());
        }
        gl::BindFramebufferEXT(gl::FRAMEBUFFER_EXT, self.fbo);
        let doubled = (size.0 * 2, size.1 * 2);
        let ok = (0..=if anime { 2 } else { 6 }).all(|i| self.target(i, size))
            && self.target(7, doubled);
        if ok {
            let copy = self.copy.as_ref().unwrap().as_ref().unwrap();
            let draw_copy = |tex, rotation: *const f32| {
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::UseProgram(copy.id);
                gl::Uniform1i(copy.image, 0);
                gl::UniformMatrix2fv(copy.rotation, 1, gl::FALSE, rotation);
                super::draw_quad();
            };
            gl::FramebufferTexture2DEXT(
                gl::FRAMEBUFFER_EXT,
                gl::COLOR_ATTACHMENT0_EXT,
                gl::TEXTURE_2D,
                self.textures[0],
                0,
            );
            gl::Viewport(0, 0, size.0 as i32, size.1 as i32);
            draw_copy(src_tex as u32, rot.columns().as_ptr().cast());
            let pipeline = (if anime { &self.anime } else { &self.fsrcnnx })
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap();
            for pass in &pipeline.0 {
                gl::FramebufferTexture2DEXT(
                    gl::FRAMEBUFFER_EXT,
                    gl::COLOR_ATTACHMENT0_EXT,
                    gl::TEXTURE_2D,
                    self.textures[pass.output],
                    0,
                );
                if pass.output == 7 {
                    gl::Viewport(0, 0, doubled.0 as i32, doubled.1 as i32);
                }
                gl::UseProgram(pass.program.id);
                gl::Uniform2f(pass.program.image_size, size.0 as f32, size.1 as f32);
                for (unit, &(slot, location)) in pass.inputs.iter().enumerate() {
                    gl::ActiveTexture(gl::TEXTURE0 + unit as u32);
                    gl::BindTexture(gl::TEXTURE_2D, self.textures[slot]);
                    gl::Uniform1i(location, unit as i32);
                }
                super::draw_quad();
            }
            gl::BindFramebufferEXT(gl::FRAMEBUFFER_EXT, old_fbo as _);
            gl::Viewport(vp.0 as _, vp.1 as _, vp.2 as _, vp.3 as _);
            draw_copy(self.textures[7], [1., 0., 0., 1.].as_ptr());
        } else {
            gl::BindFramebufferEXT(gl::FRAMEBUFFER_EXT, old_fbo as _);
        }
        gl::UseProgram(old_program as _);
        for (i, b) in bound.iter().enumerate() {
            gl::ActiveTexture(gl::TEXTURE0 + i as u32);
            gl::BindTexture(gl::TEXTURE_2D, *b as _);
        }
        gl::ActiveTexture(old_unit as _);
        ok
    }
}
