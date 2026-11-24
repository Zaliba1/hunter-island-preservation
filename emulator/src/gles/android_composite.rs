// SPDX-License-Identifier: MPL-2.0
//! Transfer the final GLES1 composition to an independent GLES3 context on the GPU.
use super::{fsr::FsrPresenter, present::UpscaleMode, shader_raw as gl};
use crate::{matrix::Matrix, window::{GLContext, GLVersion, Window}};
use std::ffi::{c_char, c_void};

type Handle = *mut c_void;
type CreateImage = unsafe extern "C" fn(Handle, Handle, u32, Handle, *const i32) -> Handle;
type DestroyImage = unsafe extern "C" fn(Handle, Handle) -> u32;
type BindImage = unsafe extern "C" fn(u32, Handle);

#[link(name = "EGL")]
extern "C" {
    fn eglGetCurrentContext() -> Handle;
    fn eglGetCurrentDisplay() -> Handle;
    fn eglGetProcAddress(name: *const c_char) -> *const c_void;
}

pub(crate) struct CompositeFilter {
    context: GLContext,
    vao: u32,
    texture: u32,
    source: Option<(u32, (u32, u32), bool)>,
    create: CreateImage,
    destroy: DestroyImage,
    bind: BindImage,
    filters: FsrPresenter,
}

impl CompositeFilter {
    pub(crate) fn new(window: &Window) -> Result<Self, String> {
        // Mali inherits GLES1 from the share group even when GLES3 was requested.
        // Share just the compositor image, keeping the context versions independent.
        let context = window.create_gl_context(GLVersion::GLES30)?;
        unsafe {
            window.make_gl_context_current(&context);
            gl::load_with(|name| window.gl_get_proc_address(name));
            let create = eglGetProcAddress(c"eglCreateImageKHR".as_ptr());
            let destroy = eglGetProcAddress(c"eglDestroyImageKHR".as_ptr());
            let bind = eglGetProcAddress(c"glEGLImageTargetTexture2DOES".as_ptr());
            if create.is_null() || destroy.is_null() || bind.is_null() {
                return Err("EGL image transfer unavailable".into());
            }
            let mut vao = 0;
            let mut buffer = 0;
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);
            gl::GenBuffers(1, &mut buffer);
            gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
            let quad: [f32; 8] = [-1., -1., 1., -1., -1., 1., 1., 1.];
            gl::BufferData(gl::ARRAY_BUFFER, std::mem::size_of_val(&quad) as isize,
                           quad.as_ptr().cast(), gl::STATIC_DRAW);
            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 0, std::ptr::null());
            Ok(Self {context, vao, texture, source: None, filters: Default::default(),
                     create: std::mem::transmute::<*const c_void, CreateImage>(create),
                     destroy: std::mem::transmute::<*const c_void, DestroyImage>(destroy),
                     bind: std::mem::transmute::<*const c_void, BindImage>(bind)})
        }
    }

    /// Enter with the compositor context current and all writes to its texture complete.
    pub(crate) fn draw(&mut self, window: &Window, texture: u32, size: (u32, u32),
                       viewport: (u32, u32, u32, u32), rotation: Matrix<2>) -> bool {
        unsafe {
            if self.source != Some((texture, size, true)) {
                if self.source == Some((texture, size, false)) { return false; }
                let display = eglGetCurrentDisplay();
                let attrs = [0x30d2, 1, 0x3038]; // EGL_IMAGE_PRESERVED_KHR, EGL_TRUE, EGL_NONE
                let image = (self.create)(display, eglGetCurrentContext(), 0x30b1, // EGL_GL_TEXTURE_2D_KHR
                                          texture as usize as Handle, attrs.as_ptr());
                self.source = Some((texture, size, false));
                if image.is_null() {
                    log!("Could not export the Android compositor texture");
                    return false;
                }
                window.make_gl_context_current(&self.context);
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, self.texture);
                (self.bind)(gl::TEXTURE_2D, image);
                let error = gl::GetError();
                (self.destroy)(display, image); // both texture siblings now own the storage
                if error != gl::NO_ERROR {
                    log!("Could not import the Android compositor image: GL error 0x{error:x}");
                    return false;
                }
                for param in [gl::TEXTURE_MAG_FILTER, gl::TEXTURE_MIN_FILTER] {
                    gl::TexParameteri(gl::TEXTURE_2D, param, gl::LINEAR as i32);
                }
                for param in [gl::TEXTURE_WRAP_S, gl::TEXTURE_WRAP_T] {
                    gl::TexParameteri(gl::TEXTURE_2D, param, gl::CLAMP_TO_EDGE as i32);
                }
                self.source = Some((texture, size, true));
            } else {
                window.make_gl_context_current(&self.context);
            }
            gl::BindVertexArray(self.vao);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::ClearColor(0., 0., 0., 1.);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            let drawn = self.filters.draw(UpscaleMode::Anime4K, size, viewport, rotation).is_some();
            if drawn { gl::Finish(); } // release the source before the compositor writes again
            drawn
        }
    }
}
