/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Only owned, checked snapshots cross into native draw calls.
use super::{gl, ShaderApi};
pub struct VertexUpload {
    pub index: u32,
    pub size: i32,
    pub kind: u32,
    pub normalized: u8,
    pub stride: i32,
    pub bytes: Vec<u8>,
}
pub enum Indices {
    Client(Vec<u8>),
    Buffer(u32),
}
pub enum DrawCommand {
    Arrays {
        mode: u32,
        first: i32,
        count: i32,
    },
    Elements {
        mode: u32,
        count: i32,
        kind: u32,
        indices: Indices,
    },
}
impl ShaderApi<'_> {
    pub fn default_vertex_array(&mut self) -> bool {
        !super::vertex_arrays_available(self._context)
            || self.integer(gl::VERTEX_ARRAY_BINDING) == 0
    }
    pub fn element_bytes(&mut self, offset: u32, length: u32) -> Result<Vec<u8>, u32> {
        let mut size = 0;
        unsafe { gl::GetBufferParameteriv(gl::ELEMENT_ARRAY_BUFFER, gl::BUFFER_SIZE, &mut size) };
        if size < 0
            || offset
                .checked_add(length)
                .is_none_or(|end| end > size as u32)
        {
            return Err(gl::INVALID_OPERATION);
        }
        let mut bytes = vec![0; length as usize];
        if length != 0 {
            unsafe {
                #[cfg(not(target_os = "android"))]
                gl::GetBufferSubData(
                    gl::ELEMENT_ARRAY_BUFFER,
                    offset as _,
                    length as _,
                    bytes.as_mut_ptr().cast(),
                );
                #[cfg(target_os = "android")]
                {
                    // GLES has no glGetBufferSubData. Read only the validated
                    // range through an ES3 map, then release it before drawing.
                    let mapped=gl::MapBufferRange(gl::ELEMENT_ARRAY_BUFFER,offset as _,length as _,gl::MAP_READ_BIT);
                    if mapped.is_null(){return Err(gl::INVALID_OPERATION);}
                    std::ptr::copy_nonoverlapping(mapped.cast::<u8>(),bytes.as_mut_ptr(),bytes.len());
                    if gl::UnmapBuffer(gl::ELEMENT_ARRAY_BUFFER)==gl::FALSE{return Err(gl::INVALID_OPERATION);}
                }
            };
        }
        Ok(bytes)
    }
    pub fn draw_uploaded(
        &mut self,
        uploads: &[VertexUpload],
        command: DrawCommand,
    ) -> Result<(), u32> {
        let old_buffer = self.integer(gl::ARRAY_BUFFER_BINDING) as u32;
        let mut buffers = vec![0; uploads.len()];
        let mut result = Ok(());
        unsafe {
            gl::GenBuffers(buffers.len().try_into().unwrap(), buffers.as_mut_ptr());
            for (upload, &buffer) in uploads.iter().zip(&buffers) {
                if buffer == 0 {
                    result = Err(gl::OUT_OF_MEMORY);
                    break;
                }
                gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    upload.bytes.len().try_into().unwrap(),
                    upload.bytes.as_ptr().cast(),
                    gl::STREAM_DRAW,
                );
                let mut size = 0;
                gl::GetBufferParameteriv(gl::ARRAY_BUFFER, gl::BUFFER_SIZE, &mut size);
                if size < 0 || size as usize != upload.bytes.len() {
                    result = Err(gl::OUT_OF_MEMORY);
                    break;
                }
                gl::VertexAttribPointer(
                    upload.index,
                    upload.size,
                    upload.kind,
                    upload.normalized,
                    upload.stride,
                    std::ptr::null(),
                );
            }
            if result.is_ok() {
                match command {
                    DrawCommand::Arrays { mode, first, count } => {
                        gl::DrawArrays(mode, first, count)
                    }
                    DrawCommand::Elements {
                        mode,
                        count,
                        kind,
                        indices,
                    } => {
                        let pointer = match &indices {
                            Indices::Client(bytes) => bytes.as_ptr().cast(),
                            Indices::Buffer(offset) => *offset as usize as *const _,
                        };
                        gl::DrawElements(mode, count, kind, pointer);
                    }
                }
            }
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            for upload in uploads {
                gl::VertexAttribPointer(
                    upload.index,
                    upload.size,
                    upload.kind,
                    upload.normalized,
                    upload.stride,
                    std::ptr::null(),
                );
            }
            gl::DeleteBuffers(buffers.len().try_into().unwrap(), buffers.as_ptr());
            gl::BindBuffer(gl::ARRAY_BUFFER, old_buffer);
        }
        result
    }
}
