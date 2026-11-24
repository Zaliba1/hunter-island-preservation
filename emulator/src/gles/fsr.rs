// SPDX-License-Identifier: MPL-2.0
//! FSR 1.2 for the desktop compatibility-profile presenter.

use super::present::UpscaleMode;
use super::shader_raw as gl;
use crate::matrix::Matrix;
use std::ffi::CString;
use std::ptr;

#[path = "cnn.rs"]
mod cnn;

const VERTEX: &str = "#version 120\n\
    uniform mat2 sourceRotation;\n\
    varying vec2 uv;\n\
    void main() {\n\
        gl_Position = gl_Vertex;\n\
        uv = sourceRotation * (gl_Vertex.xy * 0.5 + 0.5);\n\
    }\n";

struct FilterNotice {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl FilterNotice {
    fn new(label: &str) -> Self {
        use crate::font::{Font, TextAlignment};

        const FONT_SIZE: f32 = 20.0;
        const PADDING: u32 = 10;
        let font = Font::sans_bold();
        let (text_width, text_height) = font.calculate_text_size(FONT_SIZE, label, None);
        let width = text_width.ceil() as u32 + 2 * PADDING;
        let height = text_height.ceil() as u32 + 2 * PADDING;
        let mut pixels = [20, 25, 32, 255].repeat((width * height) as usize);
        font.draw(
            FONT_SIZE,
            label,
            (PADDING as f32, PADDING as f32),
            None,
            TextAlignment::Left,
            1.0,
            |glyph| {
                let (left, top) = glyph.origin();
                let (glyph_width, glyph_height) = glyph.dimensions();
                for y in 0..glyph_height {
                    for x in 0..glyph_width {
                        let dst_x = left as i32 + x;
                        let dst_y = top as i32 + y;
                        if (0..width as i32).contains(&dst_x) && (0..height as i32).contains(&dst_y)
                        {
                            let index =
                                (((height - 1 - dst_y as u32) * width + dst_x as u32) * 4) as usize;
                            let coverage = glyph.pixel_at((x, y));
                            for channel in &mut pixels[index..index + 3] {
                                *channel =
                                    (*channel as f32 + (248.0 - *channel as f32) * coverage) as u8;
                            }
                        }
                    }
                }
            },
        );
        Self {
            width,
            height,
            pixels,
        }
    }
}

struct Program {
    id: u32,
    image: i32,
    image_size: i32,
    rotation: i32,
    origin: i32,
}

impl Program {
    unsafe fn new(sharpen: bool) -> Result<Self, String> {
        let define = if sharpen { "#define SHARPEN\n" } else { "" };
        let source = format!("#version 120\n{define}{}", include_str!("fsr.glsl"));
        Self::link(VERTEX, &source)
    }

    unsafe fn link(vertex_source: &str, fragment_source: &str) -> Result<Self, String> {
        let vertex = compile(gl::VERTEX_SHADER, vertex_source)?;
        let fragment = compile(gl::FRAGMENT_SHADER, fragment_source);
        let fragment = match fragment {
            Ok(fragment) => fragment,
            Err(error) => {
                gl::DeleteShader(vertex);
                return Err(error);
            }
        };
        let id = gl::CreateProgram();
        gl::AttachShader(id, vertex);
        gl::AttachShader(id, fragment);
        gl::LinkProgram(id);
        gl::DeleteShader(vertex);
        gl::DeleteShader(fragment);

        let mut linked = 0;
        gl::GetProgramiv(id, gl::LINK_STATUS, &mut linked);
        if linked == 0 {
            let mut buffer = [0u8; 2048];
            let mut length = 0;
            gl::GetProgramInfoLog(
                id,
                buffer.len() as i32,
                &mut length,
                buffer.as_mut_ptr().cast(),
            );
            gl::DeleteProgram(id);
            return Err(String::from_utf8_lossy(&buffer[..length as usize]).into_owned());
        }
        Ok(Self {
            id,
            image: gl::GetUniformLocation(id, c"image".as_ptr()),
            image_size: gl::GetUniformLocation(id, c"imageSize".as_ptr()),
            rotation: gl::GetUniformLocation(id, c"sourceRotation".as_ptr()),
            origin: gl::GetUniformLocation(id, c"viewportOrigin".as_ptr()),
        })
    }
}

unsafe fn compile(kind: u32, source: &str) -> Result<u32, String> {
    let shader = gl::CreateShader(kind);
    let source = CString::new(source).unwrap();
    gl::ShaderSource(shader, 1, &source.as_ptr(), ptr::null());
    gl::CompileShader(shader);
    let mut compiled = 0;
    gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut compiled);
    if compiled == 0 {
        let mut buffer = [0u8; 2048];
        let mut length = 0;
        gl::GetShaderInfoLog(
            shader,
            buffer.len() as i32,
            &mut length,
            buffer.as_mut_ptr().cast(),
        );
        gl::DeleteShader(shader);
        return Err(String::from_utf8_lossy(&buffer[..length as usize]).into_owned());
    }
    Ok(shader)
}

#[derive(Default)]
pub(super) struct FsrPresenter {
    // These GL objects are owned by the context and are released with it.
    neural: cnn::Presenter,
    easu: Option<Option<Program>>,
    rcas: Option<Option<Program>>,
    framebuffer: u32,
    texture: u32,
    target: Option<((u32, u32), bool)>,
    logged: Option<UpscaleMode>,
    notice: Option<(UpscaleMode, FilterNotice)>,
}

fn program(result: Result<Program, String>, pass: &str) -> Option<Program> {
    match result {
        Ok(program) => Some(program),
        Err(error) => {
            echo!("FSR 1.2 {pass} unavailable: {error}");
            None
        }
    }
}

impl FsrPresenter {
    /// Draw using the quad arrays that the desktop presenter has already set up.
    pub(super) unsafe fn draw(
        &mut self,
        mode: UpscaleMode,
        source_size: (u32, u32),
        viewport: (u32, u32, u32, u32),
        rotation: Matrix<2>,
    ) -> Option<UpscaleMode> {
        if matches!(mode, UpscaleMode::Anime4K | UpscaleMode::Fsrcnnx) {
            if !self.neural.draw(mode, source_size, viewport, rotation) {
                return None;
            }
            if self.logged != Some(mode) {
                echo!("{} active: {:?} -> {:?}", mode.label(), source_size, (viewport.2, viewport.3));
                self.logged = Some(mode);
            }
            return Some(mode);
        }
        if self.easu.is_none() {
            self.easu = Some(program(Program::new(false), "EASU"));
        }
        if self.easu.as_ref().and_then(Option::as_ref).is_none() {
            return None;
        }

        let mut unit = 0;
        let mut previous_program = 0;
        gl::GetIntegerv(gl::ACTIVE_TEXTURE, &mut unit);
        gl::GetIntegerv(gl::CURRENT_PROGRAM, &mut previous_program);
        let sampler = unit - gl::TEXTURE0 as i32;
        let (x, y, width, height) = viewport;
        let output_size = (width, height);
        let mut applied = UpscaleMode::Fsr;

        let mut source_texture = 0;
        let mut previous_framebuffer = 0;
        let sharpen = mode == UpscaleMode::FsrSharpened
            && gl::BindFramebufferEXT::is_loaded()
            && gl::FramebufferTexture2DEXT::is_loaded()
            && gl::CheckFramebufferStatusEXT::is_loaded()
            && gl::GenFramebuffersEXT::is_loaded();
        let sharpen = if sharpen {
            if self.rcas.is_none() {
                self.rcas = Some(program(Program::new(true), "RCAS"));
            }
            if self.rcas.as_ref().and_then(Option::as_ref).is_some() {
                gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut source_texture);
                gl::GetIntegerv(gl::FRAMEBUFFER_BINDING_EXT, &mut previous_framebuffer);
                let ready = self.prepare_target(output_size);
                gl::BindTexture(gl::TEXTURE_2D, source_texture as u32);
                if !ready {
                    gl::BindFramebufferEXT(gl::FRAMEBUFFER_EXT, previous_framebuffer as u32);
                }
                ready
            } else {
                false
            }
        } else {
            false
        };

        if sharpen {
            gl::Viewport(0, 0, width as i32, height as i32);
        }
        let easu = self.easu.as_ref().unwrap().as_ref().unwrap();
        gl::UseProgram(easu.id);
        gl::Uniform1i(easu.image, sampler);
        gl::Uniform2f(easu.image_size, source_size.0 as f32, source_size.1 as f32);
        gl::UniformMatrix2fv(
            easu.rotation,
            1,
            gl::FALSE,
            rotation.columns().as_ptr().cast(),
        );
        gl::DrawArrays(gl::TRIANGLES, 0, 6);

        if sharpen {
            gl::BindFramebufferEXT(gl::FRAMEBUFFER_EXT, previous_framebuffer as u32);
            gl::Viewport(x as i32, y as i32, width as i32, height as i32);
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            let rcas = self.rcas.as_ref().unwrap().as_ref().unwrap();
            gl::UseProgram(rcas.id);
            gl::Uniform1i(rcas.image, sampler);
            gl::Uniform2f(rcas.image_size, width as f32, height as f32);
            gl::Uniform2f(rcas.origin, x as f32, y as f32);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            gl::BindTexture(gl::TEXTURE_2D, source_texture as u32);
            applied = UpscaleMode::FsrSharpened;
        }
        gl::UseProgram(previous_program as u32);

        if self.logged != Some(applied) {
            echo!(
                "{} active: {:?} -> {:?}",
                applied.label(),
                source_size,
                output_size
            );
            self.logged = Some(applied);
        }
        Some(applied)
    }

    pub(super) unsafe fn draw_notice(
        &mut self,
        mode: UpscaleMode,
        (x, y, width, height): (u32, u32, u32, u32),
    ) {
        if self.notice.as_ref().map(|(mode, _)| *mode) != Some(mode) {
            self.notice = Some((mode, FilterNotice::new(mode.label())));
        }
        let bitmap = &self.notice.as_ref().unwrap().1;
        const MARGIN: u32 = 16;
        if width < bitmap.width + MARGIN || height < bitmap.height + MARGIN {
            return;
        }
        let mut unpack_alignment = 0;
        gl::GetIntegerv(gl::UNPACK_ALIGNMENT, &mut unpack_alignment);
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);
        // Compatibility-profile DrawPixels also runs fixed-function texturing.
        let was_textured = gl::IsEnabled(gl::TEXTURE_2D) == gl::TRUE;
        gl::Disable(gl::TEXTURE_2D);
        gl::WindowPos2i(
            (x + MARGIN) as i32,
            (y + height - bitmap.height - MARGIN) as i32,
        );
        gl::DrawPixels(
            bitmap.width as i32,
            bitmap.height as i32,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            bitmap.pixels.as_ptr().cast(),
        );
        if was_textured {
            gl::Enable(gl::TEXTURE_2D);
        }
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, unpack_alignment);
    }

    /// Bind the reusable EASU output target and update its storage on resize.
    unsafe fn prepare_target(&mut self, size: (u32, u32)) -> bool {
        if self.framebuffer == 0 {
            gl::GenFramebuffersEXT(1, &mut self.framebuffer);
            gl::GenTextures(1, &mut self.texture);
        }
        gl::BindFramebufferEXT(gl::FRAMEBUFFER_EXT, self.framebuffer);
        if let Some((current, ready)) = self.target {
            if size == current {
                return ready;
            }
        }

        let mut max_size = 0;
        gl::GetIntegerv(gl::MAX_TEXTURE_SIZE, &mut max_size);
        let ready = if size.0 <= max_size as u32 && size.1 <= max_size as u32 {
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as i32,
                size.0 as i32,
                size.1 as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                ptr::null(),
            );
            gl::FramebufferTexture2DEXT(
                gl::FRAMEBUFFER_EXT,
                gl::COLOR_ATTACHMENT0_EXT,
                gl::TEXTURE_2D,
                self.texture,
                0,
            );
            gl::CheckFramebufferStatusEXT(gl::FRAMEBUFFER_EXT) == gl::FRAMEBUFFER_COMPLETE_EXT
        } else {
            false
        };
        self.target = Some((size, ready));
        if !ready {
            echo!(
                "FSR 1.2 RCAS target unavailable at {}x{}; using EASU only",
                size.0,
                size.1
            );
        }
        ready
    }
}
