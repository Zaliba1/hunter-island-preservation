/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Experimental GLES2 shader operations on the desktop GL2 backend.
//! Every operation borrows an active context. Guest memory is never passed to
//! the shader compiler: callers supply owned/validated byte and float slices.

use super::{shader_raw as gl, GLES};
use gl::types::*;
use std::ffi::CString;
mod client;
pub use client::{DrawCommand, Indices, VertexUpload};

pub struct ShaderApi<'a> {
    _context: &'a mut dyn GLES,
}

pub fn api(context: &mut dyn GLES) -> ShaderApi<'_> {
    assert!(
        context.supports_gles2_shaders(),
        "GLES2 shader backend unavailable"
    );
    ShaderApi { _context: context }
}

/// Keep ES 1.00 precision rules and GL_ES conditionals when compiling on a
/// desktop driver with ES2 shader compatibility. Existing #version is preserved.
fn es_source(parts: &[Vec<u8>]) -> Vec<u8> {
    let joined = parts.concat();
    let mut start = 0;
    while start < joined.len() {
        if joined[start].is_ascii_whitespace() {
            start += 1;
            continue;
        }
        if joined[start..].starts_with(b"//") {
            start += joined[start..]
                .iter()
                .position(|b| *b == b'\n')
                .unwrap_or(joined.len() - start);
            continue;
        }
        if joined[start..].starts_with(b"/*") {
            let Some(end) = joined[start + 2..].windows(2).position(|s| s == b"*/") else {
                break; // Let the shader compiler report malformed input.
            };
            start += end + 4;
            continue;
        }
        break;
    }
    if joined[start..].starts_with(b"#version") {
        joined
    } else {
        [b"#version 100\n".as_slice(), joined.as_slice()].concat()
    }
}

fn desktop_source(parts: &[Vec<u8>]) -> Vec<u8> {
    let source = es_source(parts);
    #[cfg(target_os = "macos")]
    { super::shader_source::macos_source(&source) }
    #[cfg(not(target_os = "macos"))]
    { source }
}

struct AttributeState {
    enabled: GLint,
    size: GLint,
    kind: GLint,
    stride: GLint,
    normalized: GLint,
    buffer: GLint,
    pointer: *mut std::ffi::c_void,
    value: [GLfloat; 4],
}

/// State touched by the GLES1 presenter that must survive a GLES2 frame.
pub struct PresentationState {
    program: GLint,
    array_buffer: GLint,
    vertex_array: Option<GLuint>,
    element_buffer: GLint,
    attributes: Vec<AttributeState>,
}

pub fn pause_program(context: &mut dyn GLES) -> PresentationState {
    assert!(context.supports_gles2_shaders());
    unsafe {
        let mut program = 0;
        let mut array_buffer = 0;
        gl::GetIntegerv(gl::CURRENT_PROGRAM, &mut program);
        gl::GetIntegerv(gl::ARRAY_BUFFER_BINDING, &mut array_buffer);
        // The fixed-function presenter must not write into the guest VAO.
        let vertex_array = if vertex_arrays_available(context) {
            let mut bound = 0;
            gl::GetIntegerv(gl::VERTEX_ARRAY_BINDING, &mut bound);
            gl::BindVertexArray(0);
            Some(bound as GLuint)
        } else {
            None
        };
        let mut element_buffer = 0;
        gl::GetIntegerv(gl::ELEMENT_ARRAY_BUFFER_BINDING, &mut element_buffer);
        let count = context.max_vertex_attributes();
        let mut attributes = Vec::new();
        for index in 0..u32::try_from(count).unwrap() {
            let mut state = AttributeState {
                enabled: 0,
                size: 0,
                kind: 0,
                stride: 0,
                normalized: 0,
                buffer: 0,
                pointer: std::ptr::null_mut(),
                value: [0.0; 4],
            };
            gl::GetVertexAttribiv(index, gl::VERTEX_ATTRIB_ARRAY_ENABLED, &mut state.enabled);
            gl::GetVertexAttribiv(index, gl::VERTEX_ATTRIB_ARRAY_SIZE, &mut state.size);
            gl::GetVertexAttribiv(index, gl::VERTEX_ATTRIB_ARRAY_TYPE, &mut state.kind);
            gl::GetVertexAttribiv(index, gl::VERTEX_ATTRIB_ARRAY_STRIDE, &mut state.stride);
            gl::GetVertexAttribiv(
                index,
                gl::VERTEX_ATTRIB_ARRAY_NORMALIZED,
                &mut state.normalized,
            );
            gl::GetVertexAttribiv(
                index,
                gl::VERTEX_ATTRIB_ARRAY_BUFFER_BINDING,
                &mut state.buffer,
            );
            gl::GetVertexAttribPointerv(index, gl::VERTEX_ATTRIB_ARRAY_POINTER, &mut state.pointer);
            if index != 0 {
                gl::GetVertexAttribfv(index, gl::CURRENT_VERTEX_ATTRIB, state.value.as_mut_ptr());
            }
            gl::DisableVertexAttribArray(index);
            attributes.push(state);
        }
        gl::UseProgram(0);
        PresentationState {
            program,
            array_buffer,
            vertex_array,
            element_buffer,
            attributes,
        }
    }
}

pub fn restore_program(context: &mut dyn GLES, state: PresentationState) {
    assert!(context.supports_gles2_shaders());
    unsafe {
        for (index, attribute) in state.attributes.into_iter().enumerate() {
            gl::BindBuffer(gl::ARRAY_BUFFER, attribute.buffer as u32);
            gl::VertexAttribPointer(
                index as u32,
                attribute.size,
                attribute.kind as u32,
                attribute.normalized as u8,
                attribute.stride,
                attribute.pointer,
            );
            if index != 0 {
                gl::VertexAttrib4fv(index as u32, attribute.value.as_ptr());
            }
            if attribute.enabled != 0 {
                gl::EnableVertexAttribArray(index as u32);
            } else {
                gl::DisableVertexAttribArray(index as u32);
            }
        }
        gl::BindBuffer(gl::ARRAY_BUFFER, state.array_buffer as u32);
        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, state.element_buffer as u32);
        if let Some(vertex_array) = state.vertex_array {
            gl::BindVertexArray(vertex_array);
        }
        gl::UseProgram(state.program as u32);
    }
}

pub fn vertex_arrays_available(context: &dyn GLES) -> bool {
    context.supports_vertex_arrays()
}

/// Query only while the owning context is active; cache no mutable GL bindings.
pub(super) fn query_vertex_arrays_available() -> bool {
    if !(gl::GenVertexArrays::is_loaded()
        && gl::BindVertexArray::is_loaded()
        && gl::DeleteVertexArrays::is_loaded()
        && gl::IsVertexArray::is_loaded())
    {
        return false;
    }
    unsafe {
        let extensions = gl::GetString(gl::EXTENSIONS);
        !extensions.is_null()
            && std::ffi::CStr::from_ptr(extensions.cast())
                .to_bytes()
                .split(|&byte| byte == b' ')
                .any(|name| {
                    #[cfg(target_os = "macos")]
                    { name == b"GL_APPLE_vertex_array_object" }
                    #[cfg(not(target_os = "macos"))]
                    { name == b"GL_ARB_vertex_array_object" }
                })
    }
}

impl ShaderApi<'_> {
    pub fn enable_attribute(&mut self, index: u32, enable: bool) {
        unsafe {
            if enable {
                gl::EnableVertexAttribArray(index);
            } else {
                gl::DisableVertexAttribArray(index);
            }
        }
    }
    pub fn attribute_integer(&mut self, index: u32, parameter: u32) -> i32 {
        let mut value = 0;
        unsafe { gl::GetVertexAttribiv(index, parameter, &mut value) };
        value
    }
    pub fn attribute_offset(&mut self, index: u32) -> usize {
        let mut value = std::ptr::null_mut();
        unsafe { gl::GetVertexAttribPointerv(index, gl::VERTEX_ATTRIB_ARRAY_POINTER, &mut value) };
        value as usize
    }
    pub fn vertex_pointer(
        &mut self,
        index: u32,
        size: i32,
        kind: u32,
        normalized: u8,
        stride: i32,
        offset: u32,
    ) -> Result<bool, u32> {
        if index >= self.integer(gl::MAX_VERTEX_ATTRIBS) as u32
            || !(1..=4).contains(&size)
            || stride < 0
        {
            return Err(gl::INVALID_VALUE);
        }
        if ![
            gl::BYTE,
            gl::UNSIGNED_BYTE,
            gl::SHORT,
            gl::UNSIGNED_SHORT,
            gl::FLOAT,
            0x140c,
        ]
        .contains(&kind)
        {
            return Err(gl::INVALID_ENUM);
        }
        assert_ne!(
            kind, 0x140c,
            "not implemented: GLES2 fixed-point vertex attributes"
        );
        let client = self.integer(gl::ARRAY_BUFFER_BINDING) == 0;
        if client && offset != 0 {
            if vertex_arrays_available(self._context) && self.integer(gl::VERTEX_ARRAY_BINDING) != 0
            {
                return Err(gl::INVALID_OPERATION);
            }
        }
        unsafe {
            gl::VertexAttribPointer(
                index,
                size,
                kind,
                normalized,
                stride,
                if client {
                    std::ptr::null()
                } else {
                    offset as usize as *const _
                },
            )
        };
        Ok(client)
    }
    pub fn error(&mut self) -> u32 {
        unsafe { gl::GetError() }
    }
    pub fn gen_vertex_arrays(&mut self, count: i32) -> Vec<u32> {
        assert!(
            vertex_arrays_available(self._context),
            "Host vertex array objects unavailable"
        );
        if count < 0 {
            unsafe { gl::GenVertexArrays(count, std::ptr::null_mut()) };
            return Vec::new();
        }
        let mut names = vec![0; count as usize];
        unsafe { gl::GenVertexArrays(count, names.as_mut_ptr()) };
        names
    }
    pub fn delete_vertex_arrays(&mut self, names: &[u32]) {
        assert!(
            vertex_arrays_available(self._context),
            "Host ARB vertex arrays unavailable"
        );
        unsafe { gl::DeleteVertexArrays(names.len().try_into().unwrap(), names.as_ptr()) }
    }
    pub fn invalid_vertex_array_delete_count(&mut self, count: i32) {
        assert!(count < 0);
        assert!(
            vertex_arrays_available(self._context),
            "Host ARB vertex arrays unavailable"
        );
        unsafe { gl::DeleteVertexArrays(count, std::ptr::null()) }
    }
    pub fn bind_vertex_array(&mut self, name: u32) {
        assert!(
            vertex_arrays_available(self._context),
            "Host ARB vertex arrays unavailable"
        );
        unsafe { gl::BindVertexArray(name) }
    }
    pub fn is_vertex_array(&mut self, name: u32) -> u8 {
        assert!(
            vertex_arrays_available(self._context),
            "Host ARB vertex arrays unavailable"
        );
        unsafe { gl::IsVertexArray(name) }
    }
    pub fn integer(&mut self, parameter: GLenum) -> GLint {
        if parameter == gl::MAX_VERTEX_ATTRIBS {
            return self._context.max_vertex_attributes();
        }
        let mut value = 0;
        unsafe {
            gl::GetIntegerv(parameter, &mut value);
        }
        value
    }
    pub fn create_shader(&mut self, kind: GLenum) -> GLuint {
        unsafe { gl::CreateShader(kind) }
    }
    pub fn delete_shader(&mut self, shader: GLuint) {
        unsafe { gl::DeleteShader(shader) }
    }
    pub fn source(&mut self, shader: GLuint, parts: &[Vec<u8>]) {
        let source = desktop_source(parts);
        #[cfg(target_os = "android")]
        let source = super::shader_source::native_extension_preamble(&source);
        let pointer = source.as_ptr().cast();
        let len = GLint::try_from(source.len()).expect("GLSL source too large");
        unsafe {
            gl::ShaderSource(shader, 1, &pointer, &len);
        }
    }
    pub fn compile(&mut self, shader: GLuint) {
        unsafe { gl::CompileShader(shader) }
    }
    pub fn shader_parameter(&mut self, shader: GLuint, parameter: GLenum) -> GLint {
        let mut value = 0;
        unsafe {
            gl::GetShaderiv(shader, parameter, &mut value);
        }
        value
    }
    pub fn shader_log(&mut self, shader: GLuint, max_length: usize) -> Vec<u8> {
        let mut bytes = vec![0; max_length];
        let mut written = 0;
        unsafe {
            gl::GetShaderInfoLog(
                shader,
                GLsizei::try_from(max_length).unwrap(),
                &mut written,
                bytes.as_mut_ptr().cast(),
            );
        }
        bytes.truncate(usize::try_from(written).unwrap().min(bytes.len()));
        bytes
    }
    pub fn shader_source(&mut self, shader: GLuint, max_length: usize) -> Vec<u8> {
        let mut bytes = vec![0; max_length];
        let mut written = 0;
        unsafe {
            gl::GetShaderSource(shader, GLsizei::try_from(max_length).unwrap(),
                &mut written, bytes.as_mut_ptr().cast());
        }
        bytes.truncate(usize::try_from(written).unwrap().min(bytes.len()));
        bytes
    }
    pub fn create_program(&mut self) -> GLuint {
        unsafe { gl::CreateProgram() }
    }
    pub fn delete_program(&mut self, program: GLuint) {
        unsafe { gl::DeleteProgram(program) }
    }
    pub fn attach(&mut self, program: GLuint, shader: GLuint) {
        unsafe { gl::AttachShader(program, shader) }
    }
    pub fn bind_attribute(&mut self, program: GLuint, index: GLuint, name: &[u8]) {
        let name = CString::new(name).unwrap();
        unsafe { gl::BindAttribLocation(program, index, name.as_ptr()) }
    }
    pub fn link(&mut self, program: GLuint) {
        unsafe { gl::LinkProgram(program) }
    }
    pub fn use_program(&mut self, program: GLuint) {
        unsafe { gl::UseProgram(program) }
    }
    pub fn program_parameter(&mut self, program: GLuint, parameter: GLenum) -> GLint {
        let mut value = 0;
        unsafe {
            gl::GetProgramiv(program, parameter, &mut value);
        }
        value
    }
    pub fn program_log(&mut self, program: GLuint, max_length: usize) -> Vec<u8> {
        let mut bytes = vec![0; max_length];
        let mut written = 0;
        unsafe {
            gl::GetProgramInfoLog(
                program,
                GLsizei::try_from(max_length).unwrap(),
                &mut written,
                bytes.as_mut_ptr().cast(),
            );
        }
        bytes.truncate(usize::try_from(written).unwrap().min(bytes.len()));
        bytes
    }
    pub fn uniform_location(&mut self, program: GLuint, name: &[u8]) -> GLint {
        let name = CString::new(name).unwrap();
        unsafe { gl::GetUniformLocation(program, name.as_ptr()) }
    }
    pub fn uniform_1i(&mut self, location: GLint, value: GLint) {
        unsafe { gl::Uniform1i(location, value) }
    }
    pub fn uniform_floats(&mut self, location: GLint, width: usize, values: &[GLfloat]) {
        assert!((1..=4).contains(&width));
        assert_eq!(values.len() % width, 0);
        let count = GLsizei::try_from(values.len() / width).unwrap();
        unsafe {
            match width {
                1 => gl::Uniform1fv(location, count, values.as_ptr()),
                2 => gl::Uniform2fv(location, count, values.as_ptr()),
                3 => gl::Uniform3fv(location, count, values.as_ptr()),
                4 => gl::Uniform4fv(location, count, values.as_ptr()),
                _ => unreachable!(),
            }
        }
    }
    pub fn uniform_matrix4(&mut self, location: GLint, values: &[GLfloat]) {
        assert_eq!(values.len() % 16, 0);
        unsafe {
            gl::UniformMatrix4fv(
                location,
                GLsizei::try_from(values.len() / 16).unwrap(),
                gl::FALSE,
                values.as_ptr(),
            )
        }
    }
}

/// A real shader/link/uniform/VBO/draw/readback probe, separate from game success.
pub fn render_probe() -> Result<(), String> {
    use crate::objc::{id, msg, msg_class, nil};
    use crate::{image::Image, options::Options, Environment};
    let mut environment = Environment::new_without_app(
        Options {
            experimental_gles2: true,
            ..Options::default()
        },
        Image::from_pixel_vec(vec![0; 4], (1, 1)),
    )?;
    let env = &mut environment;
    super::gles1_on_gl2::capability_cache_probe(env.window_mut())?;
    let context: id = msg_class![env; EAGLContext alloc];
    let context: id = msg![env; context initWithAPI:2u32];
    if context == nil {
        return Err("Experimental EAGL API 2 context was rejected".into());
    }
    let actual_api: u32 = msg![env; context API];
    if actual_api != 2 {
        return Err(format!("Unexpected EAGL API: {actual_api}"));
    }
    let selected: bool = msg_class![env; EAGLContext setCurrentContext:context];
    if !selected {
        return Err("Failed to select EAGL context".into());
    }
    crate::frameworks::opengles::framebuffer_probe(env)?;
    crate::frameworks::opengles::vertex_array_probe(env)?;
    let mut gles = crate::frameworks::opengles::active_context_for_probe(env);
    let max_attributes = api(gles.as_mut()).integer(gl::MAX_VERTEX_ATTRIBS) as u32;
    assert_eq!(
        api(gles.as_mut()).vertex_pointer(max_attributes, 2, gl::FLOAT, gl::FALSE, 0, 0),
        Err(gl::INVALID_VALUE)
    );
    let mut shader = api(gles.as_mut());
    let vertex = shader.create_shader(gl::VERTEX_SHADER);
    let fragment = shader.create_shader(gl::FRAGMENT_SHADER);
    shader.source(
        vertex,
        &[b"attribute vec2 position; void main(){gl_Position=vec4(position,0.0,1.0);}".to_vec()],
    );
    shader.source(
        fragment,
        &[
            b"precision mediump float; uniform vec4 colour; void main(){gl_FragColor=colour;}"
                .to_vec(),
        ],
    );
    for handle in [vertex, fragment] {
        shader.compile(handle);
        if shader.shader_parameter(handle, gl::COMPILE_STATUS) != gl::TRUE as GLint {
            return Err(format!(
                "GLES2 probe shader compile failed: {}",
                String::from_utf8_lossy(&shader.shader_log(handle, 4096))
            ));
        }
    }
    let program = shader.create_program();
    shader.attach(program, vertex);
    shader.attach(program, fragment);
    shader.bind_attribute(program, 0, b"position");
    shader.link(program);
    if shader.program_parameter(program, gl::LINK_STATUS) != gl::TRUE as GLint {
        return Err(format!(
            "GLES2 probe link failed: {}",
            String::from_utf8_lossy(&shader.program_log(program, 4096))
        ));
    }
    shader.use_program(program);
    let location = shader.uniform_location(program, b"colour");
    if location < 0 {
        return Err("GLES2 probe colour uniform missing".into());
    }
    shader.uniform_floats(location, 4, &[0.25, 0.5, 0.75, 1.0]);
    drop(shader);
    let arrays = api(gles.as_mut()).gen_vertex_arrays(2);
    api(gles.as_mut()).bind_vertex_array(arrays[0]);
    let mut buffer = 0;
    let mut element_buffer = 0;
    let positions: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];
    let mut pixel = [0u8; 4];
    unsafe {
        gl::Viewport(0, 0, 32, 32);
        gl::Disable(gl::DITHER);
        gl::Disable(gl::BLEND);
        gl::Disable(gl::SCISSOR_TEST);
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
        gl::GenBuffers(1, &mut buffer);
        gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            std::mem::size_of_val(&positions) as _,
            positions.as_ptr().cast(),
            gl::STATIC_DRAW,
        );
        api(gles.as_mut())
            .vertex_pointer(0, 2, gl::FLOAT, gl::FALSE, 0, 0)
            .unwrap();
        api(gles.as_mut()).enable_attribute(0, true);
        gl::GenBuffers(1, &mut element_buffer);
        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, element_buffer);
        let indices: [u16; 3] = [0, 1, 2];
        gl::BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            6,
            indices.as_ptr().cast(),
            gl::STATIC_DRAW,
        );
        // The second VAO must begin with independent state.
        gl::BindVertexArray(arrays[1]);
        let mut value = -1;
        gl::GetVertexAttribiv(0, gl::VERTEX_ATTRIB_ARRAY_ENABLED, &mut value);
        assert_eq!(value, 0);
        gl::GetIntegerv(gl::ELEMENT_ARRAY_BUFFER_BINDING, &mut value);
        assert_eq!(value, 0);
        gl::BindVertexArray(arrays[0]);
        gl::GetIntegerv(gl::ELEMENT_ARRAY_BUFFER_BINDING, &mut value);
        assert_eq!(value as u32, element_buffer);
        gl::DrawArrays(gl::TRIANGLES, 0, 3);
        gl::ReadPixels(
            16,
            16,
            1,
            1,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            pixel.as_mut_ptr().cast(),
        );
        let error = gl::GetError();
        if error != gl::NO_ERROR {
            return Err(format!("GLES2 probe GL error: {error:#x}"));
        }
    }
    let state = pause_program(gles.as_mut());
    unsafe {
        assert_eq!(api(gles.as_mut()).integer(gl::VERTEX_ARRAY_BINDING), 0);
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::VertexAttribPointer(0, 4, gl::FLOAT, gl::FALSE, 0, std::ptr::null());
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
    restore_program(gles.as_mut(), state);
    unsafe {
        assert_eq!(
            api(gles.as_mut()).integer(gl::VERTEX_ARRAY_BINDING) as u32,
            arrays[0]
        );
        gl::DrawElements(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, std::ptr::null());
        gl::ReadPixels(
            16,
            16,
            1,
            1,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            pixel.as_mut_ptr().cast(),
        );
        let error = gl::GetError();
        gl::DisableVertexAttribArray(0);
        gl::DeleteBuffers(1, &buffer);
        gl::DeleteBuffers(1, &element_buffer);
        gl::DeleteVertexArrays(2, arrays.as_ptr());
        if error != gl::NO_ERROR {
            return Err(format!("GLES2 restore probe GL error: {error:#x}"));
        }
    }
    let mut shader = api(gles.as_mut());
    shader.use_program(0);
    shader.delete_program(program);
    shader.delete_shader(vertex);
    shader.delete_shader(fragment);
    let expected = [64i16, 128, 191, 255];
    if pixel
        .iter()
        .zip(expected)
        .any(|(&value, target)| (value as i16 - target).abs() > 2)
    {
        return Err(format!("GLES2 probe incorrect pixel: {pixel:?}"));
    }
    echo!("GLES2_SHADER_RENDER_PASS: pixel={pixel:?}; shader compilation, linking, uniform, buffered triangle, program/attribute restoration and GPU readback verified");
    echo!("GLES2_VAO_RENDER_PASS: two independent VAOs, element-buffer state, indexed triangle and presenter restoration verified; pixel={pixel:?}");
    drop(shader);
    drop(gles);
    crate::frameworks::opengles::client_render_probe(env)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn glsl_version_follows_comments_and_split_source() {
        assert_eq!(
            es_source(&[b"// test\n /*x*/ #version 100\n".to_vec()]),
            b"// test\n /*x*/ #version 100\n"
        );
        assert_eq!(
            es_source(&[b"/*#version 100*/".to_vec(), b"void main(){}".to_vec()]),
            b"#version 100\n/*#version 100*/void main(){}"
        );
        assert_eq!(es_source(&[]), b"#version 100\n");
    }
}
