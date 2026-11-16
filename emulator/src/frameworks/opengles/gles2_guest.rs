/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Experimental, checked guest-memory adapters for GLES2 shader operations.
use super::gles_guest::with_ctx_and_mem;
use crate::dyld::{export_c_func, FunctionExports};
use crate::gles::gles2_shaders::{api, ShaderApi};
use crate::mem::ConstVoidPtr;
use crate::mem::{ConstPtr, Mem, MutPtr};
use crate::Environment;
mod client_probe;
pub(crate) use client_probe::client_render_probe;

#[derive(Clone, Copy)]
pub(super) struct ClientAttribute {
    size: i32,
    kind: u32,
    normalized: u8,
    stride: i32,
    pointer: ConstVoidPtr,
}
fn client_state(env: &mut Environment) -> &mut std::collections::HashMap<u32, ClientAttribute> {
    let context = env
        .framework_state
        .opengles
        .current_ctxs
        .get(&env.current_thread)
        .copied()
        .flatten()
        .unwrap();
    &mut env
        .objc
        .borrow_mut::<super::eagl::EAGLContextHostObject>(context)
        .client_attributes
}

pub(super) fn integer_parameter(
    env: &mut Environment,
    parameter: u32,
    output: MutPtr<i32>,
) -> bool {
    use touchHLE_gl_bindings::gl21compat as gl;
    if super::current_api(env) != Some(2) {
        return false;
    }
    let (host_parameter, divisor) = match parameter {
        gl::CURRENT_PROGRAM
        | gl::MAX_VERTEX_ATTRIBS
        | gl::VERTEX_ARRAY_BINDING
        | gl::MAX_COMBINED_TEXTURE_IMAGE_UNITS
        | gl::MAX_TEXTURE_IMAGE_UNITS
        | gl::MAX_VERTEX_TEXTURE_IMAGE_UNITS => (parameter, 1),
        0x8dfb => (gl::MAX_VERTEX_UNIFORM_COMPONENTS, 4),
        0x8dfd => (gl::MAX_FRAGMENT_UNIFORM_COMPONENTS, 4),
        0x8dfc => (gl::MAX_VARYING_FLOATS, 4),
        _ => return false,
    };
    with_api(env, |api, mem| {
        mem.write(output, api.integer(host_parameter) / divisor)
    });
    true
}
#[cfg(target_os = "macos")]
fn generated_vertex_arrays(env: &mut Environment) -> Option<&mut std::collections::HashSet<u32>> {
    let context = env.framework_state.opengles.current_ctxs.get(&env.current_thread).copied().flatten()?;
    Some(&mut env.objc.borrow_mut::<super::eagl::EAGLContextHostObject>(context).generated_vertex_arrays)
}

pub(super) fn glGenVertexArraysOES(env: &mut Environment, count: i32, output: MutPtr<u32>) {
    let _names = with_api(env, |api, mem| {
        if count > 0 {
            let _ = mem.bytes_at_mut(output.cast(), (count as u32).checked_mul(4).unwrap());
        }
        let names = api.gen_vertex_arrays(count);
        for (index, &name) in names.iter().enumerate() {
            mem.write(output + index as u32, name);
        }
        names
    });
    #[cfg(target_os = "macos")]
    if let Some(allocated) = generated_vertex_arrays(env) { allocated.extend(_names); }
}
pub(super) fn glDeleteVertexArraysOES(env: &mut Environment, count: i32, source: ConstPtr<u32>) {
    let _names = with_api(env, |api, mem| {
        if count < 0 {
            api.invalid_vertex_array_delete_count(count);
            return Vec::new();
        }
        if count > 0 {
            let _ = mem.bytes_at(source.cast(), (count as u32).checked_mul(4).unwrap());
        }
        let names: Vec<_> = (0..count as u32)
            .map(|index| mem.read(source + index))
            .collect();
        api.delete_vertex_arrays(&names);
        names
    });
    #[cfg(target_os = "macos")]
    if let Some(allocated) = generated_vertex_arrays(env) {
        for name in _names { allocated.remove(&name); }
    }
}
pub(super) fn glBindVertexArrayOES(env: &mut Environment, name: u32) {
    // APPLE VAOs allow binding arbitrary unused names, while OES requires a
    // name from GenVertexArraysOES. Validate without altering the old binding.
    #[cfg(target_os = "macos")]
    if name != 0 && generated_vertex_arrays(env).is_some_and(|names| !names.contains(&name)) {
        super::guest_error(env, Some(0x0502)); return;
    }
    with_api(env, |api, _| api.bind_vertex_array(name));
}
pub(super) fn glIsVertexArrayOES(env: &mut Environment, name: u32) -> u8 {
    #[cfg(target_os = "macos")]
    if generated_vertex_arrays(env).is_some_and(|names| !names.contains(&name)) { return 0; }
    with_api(env, |api, _| api.is_vertex_array(name))
}
fn glEnableVertexAttribArray(env: &mut Environment, index: u32) {
    with_api(env, |api, _| api.enable_attribute(index, true));
}
fn glDisableVertexAttribArray(env: &mut Environment, index: u32) {
    with_api(env, |api, _| api.enable_attribute(index, false));
}
fn glVertexAttribPointer(
    env: &mut Environment,
    index: u32,
    size: i32,
    kind: u32,
    normalized: u8,
    stride: i32,
    pointer: ConstVoidPtr,
) {
    let (client, error, default_array) = with_api(env, |api, _| {
        let default = api.default_vertex_array();
        match api.vertex_pointer(index, size, kind, normalized, stride, pointer.to_bits()) {
            Ok(client) => (Some(client), None, default),
            Err(error) => (None, Some(error), default),
        }
    });
    if let Some(error) = error {
        super::guest_error(env, Some(error));
    } else if let Some(client) = client {
        if default_array {
            if client {
                client_state(env).insert(
                    index,
                    ClientAttribute {
                        size,
                        kind,
                        normalized,
                        stride,
                        pointer,
                    },
                );
            } else {
                client_state(env).remove(&index);
            }
        }
    }
}

const STAGING_LIMIT: u32 = 64 * 1024 * 1024;
fn upload_length(attribute: ClientAttribute, vertices: u32) -> Result<u32, u32> {
    if vertices == 0 {
        return Ok(0);
    }
    let component_size = match attribute.kind {
        0x1400 | 0x1401 => 1,
        0x1402 | 0x1403 => 2,
        0x1406 => 4,
        _ => unreachable!(),
    };
    let element = attribute.size as u32 * component_size;
    let stride = if attribute.stride == 0 {
        element
    } else {
        attribute.stride as u32
    };
    (vertices - 1)
        .checked_mul(stride)
        .and_then(|bytes| bytes.checked_add(element))
        .filter(|&length| length <= STAGING_LIMIT)
        .ok_or(0x0505)
}
fn uploads(
    api: &mut ShaderApi<'_>,
    mem: &Mem,
    attributes: &std::collections::HashMap<u32, ClientAttribute>,
    vertices: u32,
) -> Result<Vec<crate::gles::gles2_shaders::VertexUpload>, u32> {
    let mut result = Vec::new();
    if vertices == 0 {
        return Ok(result);
    }
    let default_array = api.default_vertex_array();
    let mut total = 0u32;
    for index in 0..api.integer(0x8869) as u32 {
        if api.attribute_integer(index, 0x8622) == 0 || api.attribute_integer(index, 0x889f) != 0 {
            continue;
        }
        let attribute = if default_array {
            attributes.get(&index)
        } else {
            None
        }
        .ok_or(0x0502u32)?;
        if attribute.pointer.is_null() {
            return Err(0x0502);
        }
        let length = upload_length(*attribute, vertices)?;
        total = total
            .checked_add(length)
            .filter(|&value| value <= STAGING_LIMIT)
            .ok_or(0x0505u32)?;
        let bytes = mem.bytes_at(attribute.pointer.cast(), length).to_vec();
        result.push(crate::gles::gles2_shaders::VertexUpload {
            index,
            size: attribute.size,
            kind: attribute.kind,
            normalized: attribute.normalized,
            stride: attribute.stride,
            bytes,
        });
    }
    Ok(result)
}
pub(super) fn draw_arrays(env: &mut Environment, mode: u32, first: i32, count: i32) {
    let error = if mode > 6 {
        Some(0x0500)
    } else if first < 0 || count < 0 {
        Some(0x0501)
    } else {
        None
    };
    if let Some(error) = error {
        super::guest_error(env, Some(error));
        return;
    }
    let attributes = client_state(env).clone();
    let result = with_api(env, |api, mem| {
        Some((|| {
            let vertices = if count == 0 {
                0
            } else {
                (first as u32).checked_add(count as u32).ok_or(0x0505u32)?
            };
            let uploads = uploads(api, mem, &attributes, vertices)?;
            api.draw_uploaded(
                &uploads,
                crate::gles::gles2_shaders::DrawCommand::Arrays { mode, first, count },
            )?;
            Ok::<(), u32>(())
        })())
    })
    .unwrap();
    if let Err(error) = result {
        super::guest_error(env, Some(error));
    }
}
pub(super) fn draw_elements(
    env: &mut Environment,
    mode: u32,
    count: i32,
    kind: u32,
    pointer: ConstVoidPtr,
) {
    let width = match kind {
        0x1401 => 1,
        0x1403 => 2,
        _ => 0,
    };
    let error = if mode > 6 || width == 0 {
        Some(0x0500)
    } else if count < 0 {
        Some(0x0501)
    } else {
        None
    };
    if let Some(error) = error {
        super::guest_error(env, Some(error));
        return;
    }
    let attributes = client_state(env).clone();
    let result = with_api(env, |api, mem| {
        Some((|| {
            let length = (count as u32)
                .checked_mul(width)
                .filter(|&bytes| bytes <= STAGING_LIMIT)
                .ok_or(0x0505u32)?;
            let buffered = api.integer(0x8895) != 0;
            let bytes = if length == 0 {
                Vec::new()
            } else if buffered {
                api.element_bytes(pointer.to_bits(), length)?
            } else {
                if pointer.is_null() {
                    return Err(0x0502);
                }
                mem.bytes_at(pointer.cast(), length).to_vec()
            };
            let vertices = if width == 1 {
                bytes.iter().map(|&n| n as u32 + 1).max().unwrap_or(0)
            } else {
                bytes
                    .chunks_exact(2)
                    .map(|n| u16::from_le_bytes([n[0], n[1]]) as u32 + 1)
                    .max()
                    .unwrap_or(0)
            };
            let uploads = uploads(api, mem, &attributes, vertices)?;
            let indices = if buffered {
                crate::gles::gles2_shaders::Indices::Buffer(pointer.to_bits())
            } else {
                crate::gles::gles2_shaders::Indices::Client(bytes)
            };
            api.draw_uploaded(
                &uploads,
                crate::gles::gles2_shaders::DrawCommand::Elements {
                    mode,
                    count,
                    kind,
                    indices,
                },
            )?;
            Ok::<(), u32>(())
        })())
    })
    .unwrap();
    if let Err(error) = result {
        super::guest_error(env, Some(error));
    }
}

pub(crate) fn vertex_array_probe(env: &mut Environment) -> Result<(), String> {
    use crate::objc::{id, msg, msg_class};
    let scratch = env.mem.alloc(16).cast::<u32>();
    glGenVertexArraysOES(env, 0, MutPtr::null());
    glDeleteVertexArraysOES(env, 0, ConstPtr::null());
    glGenVertexArraysOES(env, 2, scratch);
    let first = env.mem.read(scratch);
    let second = env.mem.read(scratch + 1);
    assert_ne!(first, 0);
    assert_ne!(first, second);
    assert_eq!(glIsVertexArrayOES(env, first), 0);
    glBindVertexArrayOES(env, first);
    assert_eq!(glIsVertexArrayOES(env, first), 1);
    assert_eq!(glIsVertexArrayOES(env, 0), 0);
    glEnableVertexAttribArray(env, 1);
    glDisableVertexAttribArray(env, 1);
    glVertexAttribPointer(env, 1, 2, 0x1406, 0, 0, ConstPtr::from_bits(4096));
    assert_eq!(super::gles_guest::glGetError(env), 0x0502);
    glVertexAttribPointer(env, 1, 5, 0x1406, 0, 0, ConstPtr::null());
    assert_eq!(super::gles_guest::glGetError(env), 0x0501);
    glVertexAttribPointer(env, 1, 2, 0x140a, 0, 0, ConstPtr::null());
    assert_eq!(super::gles_guest::glGetError(env), 0x0500);
    let buffer = with_ctx_and_mem(env, |gles, _| unsafe {
        let mut buffer = 0;
        gles.GenBuffers(1, &mut buffer);
        gles.BindBuffer(0x8892, buffer);
        gles.BufferData(0x8892, 32, [0u8; 32].as_ptr().cast(), 0x88e4);
        buffer
    });
    glVertexAttribPointer(env, 1, 2, 0x1406, 0, 12, ConstPtr::from_bits(4));
    glEnableVertexAttribArray(env, 1);
    with_api(env, |api, _| {
        assert_eq!(api.attribute_integer(1, 0x889f) as u32, buffer);
        assert_eq!(api.attribute_integer(1, 0x8624), 12);
        assert_eq!(api.attribute_integer(1, 0x8622), 1);
        assert_eq!(api.attribute_offset(1), 4);
    });
    glBindVertexArrayOES(env, second);
    assert_eq!(with_api(env, |api, _| api.attribute_integer(1, 0x8622)), 0);
    glBindVertexArrayOES(env, first);
    assert_eq!(with_api(env, |api, _| api.attribute_integer(1, 0x8622)), 1);
    with_ctx_and_mem(env, |gles, _| unsafe {
        gles.DeleteBuffers(1, &buffer);
    });
    assert!(integer_parameter(env, 0x85b5, (scratch + 2).cast()));
    assert_eq!(env.mem.read(scratch + 2), first);
    env.mem.write(scratch + 2, 0xaabbccdd);
    super::gles_guest::glGetBooleanv(env, 0x85b5, (scratch + 2).cast());
    assert_eq!(env.mem.read(scratch + 2), 0xaabbcc01);
    super::gles_guest::glGetFloatv(env, 0x85b5, (scratch + 3).cast());
    assert_eq!(env.mem.read((scratch + 3).cast::<f32>()), first as f32);
    glBindVertexArrayOES(env, u32::MAX);
    assert_eq!(super::gles_guest::glGetError(env), 0x0502);
    assert_eq!(with_api(env, |api, _| api.integer(0x85b5)) as u32, first);
    assert_eq!(glIsVertexArrayOES(env, u32::MAX), 0);
    glGenVertexArraysOES(env, -1, MutPtr::null());
    assert_eq!(with_api(env, |api, _| api.error()), 0x0501);
    glDeleteVertexArraysOES(env, -1, ConstPtr::null());
    assert_eq!(with_api(env, |api, _| api.error()), 0x0501);

    let original: id = msg_class![env; EAGLContext currentContext];
    let other: id = msg_class![env; EAGLContext alloc];
    let other: id = msg![env; other initWithAPI:2u32];
    super::guest_error(env, Some(0x0502));
    let selected: bool = msg_class![env; EAGLContext setCurrentContext:other];
    assert!(selected);
    assert_eq!(super::gles_guest::glGetError(env), 0);
    assert_eq!(
        glIsVertexArrayOES(env, first),
        0,
        "VAOs must be context-local"
    );
    let selected: bool = msg_class![env; EAGLContext setCurrentContext:original];
    assert!(selected);
    assert_eq!(super::gles_guest::glGetError(env), 0x0502);
    let _: () = msg![env; other release];
    assert_eq!(glIsVertexArrayOES(env, first), 1);
    glDeleteVertexArraysOES(env, 1, scratch.cast_const());
    assert!(integer_parameter(env, 0x85b5, (scratch + 2).cast()));
    assert_eq!(
        env.mem.read(scratch + 2),
        0,
        "Deleting bound VAO must select zero"
    );
    assert_eq!(glIsVertexArrayOES(env, first), 0);
    glBindVertexArrayOES(env, first);
    assert_eq!(super::gles_guest::glGetError(env), 0x0502);
    glBindVertexArrayOES(env, second);
    glDeleteVertexArraysOES(env, 2, scratch.cast_const());
    assert_eq!(with_api(env, |api, _| api.error()), 0);
    env.mem.free(scratch.cast());
    echo!("GLES2_VAO_GUEST_PASS: generation, binding, queries, deletion, invalid names/counts and context isolation verified");
    echo!("GLES2_VERTEX_ATTRIB_GUEST_PASS: enable/disable, VBO format/stride/offset, VAO isolation, checked queries and context-local errors verified");
    Ok(())
}

fn with_api<R: Default>(
    env: &mut Environment,
    call: impl FnOnce(&mut ShaderApi<'_>, &mut Mem) -> R,
) -> R {
    assert!(
        env.options.experimental_gles2,
        "GLES2 shader operations require --experimental-gles2"
    );
    with_ctx_and_mem(env, |context, mem| call(&mut api(context), mem))
}
fn glCreateShader(env: &mut Environment, kind: u32) -> u32 {
    with_api(env, |api, _| api.create_shader(kind))
}
fn glDeleteShader(env: &mut Environment, shader: u32) {
    with_api(env, |api, _| api.delete_shader(shader))
}
fn glCompileShader(env: &mut Environment, shader: u32) {
    with_api(env, |api, _| {
        api.compile(shader);
        if api.shader_parameter(shader, 0x8b81) == 0 {
            let required = api.shader_parameter(shader, 0x8b84).clamp(0, 16384) as usize;
            let log = api.shader_log(shader, required);
            echo!("GLSL shader {} compilation failed: {}", shader, String::from_utf8_lossy(&log));
        }
    })
}
fn glShaderSource(
    env: &mut Environment,
    shader: u32,
    count: i32,
    strings: ConstPtr<ConstPtr<u8>>,
    lengths: ConstPtr<i32>,
) {
    let count = u32::try_from(count).expect("Negative shader string count");
    with_api(env, |api, mem| {
        if count > 0 {
            let _ = mem.bytes_at(strings.cast(), count.checked_mul(4).unwrap());
        }
        if count > 0 && !lengths.is_null() {
            let _ = mem.bytes_at(lengths.cast(), count.checked_mul(4).unwrap());
        }
        let parts: Vec<Vec<u8>> = (0..count)
            .map(|index| {
                let text = mem.read(strings + index);
                let length = if lengths.is_null() {
                    -1
                } else {
                    mem.read(lengths + index)
                };
                if length < 0 {
                    mem.cstr_at(text).to_vec()
                } else if length == 0 {
                    Vec::new()
                } else {
                    mem.bytes_at(text, length as u32).to_vec()
                }
            })
            .collect();
        api.source(shader, &parts)
    })
}
fn glGetShaderiv(env: &mut Environment, shader: u32, parameter: u32, output: MutPtr<i32>) {
    with_api(env, |api, mem| {
        let value = api.shader_parameter(shader, parameter);
        mem.write(output, value);
    })
}
fn glGetShaderSource(env: &mut Environment, shader: u32, buffer_size: i32, length: MutPtr<i32>, output: MutPtr<u8>) {
    if buffer_size < 0 {
        super::guest_error(env, Some(0x0501));
        return;
    }
    with_api(env, |api, mem| {
        let required = api.shader_parameter(shader, 0x8b88).max(0);
        let bytes = api.shader_source(shader, buffer_size.min(required) as usize);
        copy_log(mem, buffer_size, length, output, &bytes)
    })
}
fn glGetProgramiv(env: &mut Environment, program: u32, parameter: u32, output: MutPtr<i32>) {
    with_api(env, |api, mem| {
        let value = api.program_parameter(program, parameter);
        mem.write(output, value);
    })
}
fn copy_log(
    mem: &mut Mem,
    buffer_size: i32,
    length: MutPtr<i32>,
    output: MutPtr<u8>,
    bytes: &[u8],
) {
    let capacity = u32::try_from(buffer_size).expect("Negative log buffer size");
    let used = if capacity == 0 {
        0
    } else {
        let target = mem.bytes_at_mut(output, capacity);
        let n = bytes.len().min(target.len() - 1);
        target[..n].copy_from_slice(&bytes[..n]);
        target[n] = 0;
        n
    };
    if !length.is_null() {
        mem.write(length, used as i32);
    }
}
fn glGetShaderInfoLog(
    env: &mut Environment,
    shader: u32,
    buffer_size: i32,
    length: MutPtr<i32>,
    output: MutPtr<u8>,
) {
    with_api(env, |api, mem| {
        let required = api.shader_parameter(shader, 0x8b84).max(0);
        let bytes = api.shader_log(shader, usize::try_from(buffer_size.min(required)).unwrap());
        copy_log(mem, buffer_size, length, output, &bytes)
    })
}
fn glGetProgramInfoLog(
    env: &mut Environment,
    program: u32,
    buffer_size: i32,
    length: MutPtr<i32>,
    output: MutPtr<u8>,
) {
    with_api(env, |api, mem| {
        let required = api.program_parameter(program, 0x8b84).max(0);
        let bytes = api.program_log(program, usize::try_from(buffer_size.min(required)).unwrap());
        copy_log(mem, buffer_size, length, output, &bytes)
    })
}
fn glCreateProgram(env: &mut Environment) -> u32 {
    with_api(env, |api, _| api.create_program())
}
fn glDeleteProgram(env: &mut Environment, program: u32) {
    with_api(env, |api, _| api.delete_program(program))
}
fn glAttachShader(env: &mut Environment, program: u32, shader: u32) {
    with_api(env, |api, _| api.attach(program, shader))
}
fn glLinkProgram(env: &mut Environment, program: u32) {
    with_api(env, |api, _| api.link(program))
}
fn glUseProgram(env: &mut Environment, program: u32) {
    with_api(env, |api, _| api.use_program(program))
}
fn glBindAttribLocation(env: &mut Environment, program: u32, index: u32, name: ConstPtr<u8>) {
    with_api(env, |api, mem| {
        api.bind_attribute(program, index, mem.cstr_at(name))
    })
}
fn glGetUniformLocation(env: &mut Environment, program: u32, name: ConstPtr<u8>) -> i32 {
    with_api(env, |api, mem| {
        api.uniform_location(program, mem.cstr_at(name))
    })
}
fn glUniform1i(env: &mut Environment, location: i32, value: i32) {
    with_api(env, |api, _| api.uniform_1i(location, value))
}
fn glUniform1f(env: &mut Environment, location: i32, x: f32) {
    with_api(env, |api, _| api.uniform_floats(location, 1, &[x]))
}
fn glUniform2f(env: &mut Environment, location: i32, x: f32, y: f32) {
    with_api(env, |api, _| api.uniform_floats(location, 2, &[x, y]))
}
fn glUniform3f(env: &mut Environment, location: i32, x: f32, y: f32, z: f32) {
    with_api(env, |api, _| api.uniform_floats(location, 3, &[x, y, z]))
}
fn glUniform4f(env: &mut Environment, location: i32, x: f32, y: f32, z: f32, w: f32) {
    with_api(env, |api, _| api.uniform_floats(location, 4, &[x, y, z, w]))
}
fn floats(mem: &Mem, source: ConstPtr<f32>, count: i32, width: u32) -> Vec<f32> {
    let count = u32::try_from(count).unwrap().checked_mul(width).unwrap();
    if count == 0 {
        return Vec::new();
    }
    let _ = mem.bytes_at(source.cast(), count.checked_mul(4).unwrap());
    (0..count).map(|i| mem.read(source + i)).collect()
}
fn glUniform2fv(env: &mut Environment, location: i32, count: i32, source: ConstPtr<f32>) {
    with_api(env, |api, mem| {
        api.uniform_floats(location, 2, &floats(mem, source, count, 2))
    })
}
fn glUniform3fv(env: &mut Environment, location: i32, count: i32, source: ConstPtr<f32>) {
    with_api(env, |api, mem| {
        api.uniform_floats(location, 3, &floats(mem, source, count, 3))
    })
}
fn glUniform4fv(env: &mut Environment, location: i32, count: i32, source: ConstPtr<f32>) {
    with_api(env, |api, mem| {
        api.uniform_floats(location, 4, &floats(mem, source, count, 4))
    })
}
fn glUniformMatrix4fv(
    env: &mut Environment,
    location: i32,
    count: i32,
    transpose: u8,
    source: ConstPtr<f32>,
) {
    assert_eq!(transpose, 0, "GLES2 matrix transpose must be false");
    with_api(env, |api, mem| {
        api.uniform_matrix4(location, &floats(mem, source, count, 16))
    })
}
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(glEnableVertexAttribArray(_)),
    export_c_func!(glDisableVertexAttribArray(_)),
    export_c_func!(glVertexAttribPointer(_, _, _, _, _, _)),
    export_c_func!(glGenVertexArraysOES(_, _)),
    export_c_func!(glDeleteVertexArraysOES(_, _)),
    export_c_func!(glBindVertexArrayOES(_)),
    export_c_func!(glIsVertexArrayOES(_)),
    export_c_func!(glCreateShader(_)),
    export_c_func!(glDeleteShader(_)),
    export_c_func!(glCompileShader(_)),
    export_c_func!(glShaderSource(_, _, _, _)),
    export_c_func!(glGetShaderiv(_, _, _)),
    export_c_func!(glGetShaderSource(_, _, _, _)),
    export_c_func!(glGetProgramiv(_, _, _)),
    export_c_func!(glGetShaderInfoLog(_, _, _, _)),
    export_c_func!(glGetProgramInfoLog(_, _, _, _)),
    export_c_func!(glCreateProgram()),
    export_c_func!(glDeleteProgram(_)),
    export_c_func!(glAttachShader(_, _)),
    export_c_func!(glLinkProgram(_)),
    export_c_func!(glUseProgram(_)),
    export_c_func!(glBindAttribLocation(_, _, _)),
    export_c_func!(glGetUniformLocation(_, _)),
    export_c_func!(glUniform1i(_, _)),
    export_c_func!(glUniform1f(_, _)),
    export_c_func!(glUniform2f(_, _, _)),
    export_c_func!(glUniform3f(_, _, _, _)),
    export_c_func!(glUniform4f(_, _, _, _, _)),
    export_c_func!(glUniform2fv(_, _, _)),
    export_c_func!(glUniform3fv(_, _, _)),
    export_c_func!(glUniform4fv(_, _, _)),
    export_c_func!(glUniformMatrix4fv(_, _, _, _)),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_upload_ranges_and_resource_limits() {
        let mut attribute = ClientAttribute {
            size: 3,
            kind: 0x1406,
            normalized: 0,
            stride: 24,
            pointer: ConstPtr::null(),
        };
        assert_eq!(upload_length(attribute, 0), Ok(0));
        assert_eq!(upload_length(attribute, 4), Ok(84));
        attribute.stride = 0;
        assert_eq!(upload_length(attribute, 4), Ok(48));
        assert_eq!(upload_length(attribute, u32::MAX), Err(0x0505));
        assert_eq!(upload_length(attribute, STAGING_LIMIT), Err(0x0505));
    }
    #[test]
    fn logs_preserve_length_terminator_and_capacity() {
        let mut mem = Mem::new();
        mem.reserve(0, 4096);
        let buffer = mem.alloc(8).cast::<u8>();
        let length = mem.alloc(4).cast::<i32>();
        mem.bytes_at_mut(buffer, 8).fill(0xaa);
        copy_log(&mut mem, 4, length, buffer, b"abcdef");
        assert_eq!(
            mem.bytes_at(buffer, 8),
            &[b'a', b'b', b'c', 0, 0xaa, 0xaa, 0xaa, 0xaa]
        );
        assert_eq!(mem.read(length), 3);
        copy_log(&mut mem, 0, length, MutPtr::null(), b"abc");
        assert_eq!(mem.read(length), 0);
    }
}
