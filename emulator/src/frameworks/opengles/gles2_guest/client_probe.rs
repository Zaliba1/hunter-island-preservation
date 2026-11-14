/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use super::*;
fn clear(env: &mut Environment) {
    with_ctx_and_mem(env, |gles, _| unsafe {
        gles.ClearColor(0.0, 0.0, 0.0, 1.0);
        gles.Clear(0x4000);
    });
}
fn pixel(env: &mut Environment, expected: [u8; 4]) {
    let actual = with_ctx_and_mem(env, |gles, _| unsafe {
        let mut bytes = [0u8; 4];
        gles.ReadPixels(16, 16, 1, 1, 0x1908, 0x1401, bytes.as_mut_ptr().cast());
        bytes
    });
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(&a, b)| (a as i16 - b as i16).abs() <= 2),
        "Client draw pixel {actual:?}, expected {expected:?}"
    );
    assert_eq!(super::super::gles_guest::glGetError(env), 0);
}
pub(crate) fn client_render_probe(env: &mut Environment) -> Result<(), String> {
    let (program, vertex, fragment) = with_api(env, |api, _| {
        let vertex = api.create_shader(0x8b31);
        let fragment = api.create_shader(0x8b30);
        api.source(vertex, &[b"attribute vec2 position; attribute vec4 colour; varying vec4 tint; void main(){gl_Position=vec4(position,0.0,1.0); tint=colour;}".to_vec()]);
        api.source(
            fragment,
            &[
                b"precision mediump float; varying vec4 tint; void main(){gl_FragColor=tint;}"
                    .to_vec(),
            ],
        );
        api.compile(vertex);
        api.compile(fragment);
        assert_eq!(api.shader_parameter(vertex, 0x8b81), 1);
        assert_eq!(api.shader_parameter(fragment, 0x8b81), 1);
        let program = api.create_program();
        api.attach(program, vertex);
        api.attach(program, fragment);
        api.bind_attribute(program, 0, b"position");
        api.bind_attribute(program, 1, b"colour");
        api.link(program);
        assert_eq!(api.program_parameter(program, 0x8b82), 1);
        api.use_program(program);
        (program, vertex, fragment)
    });
    let positions = env.mem.alloc(48).cast::<f32>();
    let data: [f32; 12] = [
        -8.0, -8.0, -8.0, -8.0, -8.0, -8.0, -1.0, -1.0, 3.0, -1.0, -1.0, 3.0,
    ];
    for (index, &value) in data.iter().enumerate() {
        env.mem.write(positions + index as u32, value);
    }
    let colour_buffer = with_ctx_and_mem(env, |gles, _| unsafe {
        let mut name = 0;
        gles.GenBuffers(1, &mut name);
        gles.BindBuffer(0x8892, name);
        let colours = [0.25f32, 0.5, 0.75, 1.0].repeat(6);
        gles.BufferData(0x8892, 96, colours.as_ptr().cast(), 0x88e4);
        name
    });
    glVertexAttribPointer(env, 1, 4, 0x1406, 0, 0, ConstPtr::null());
    with_ctx_and_mem(env, |gles, _| unsafe {
        gles.BindBuffer(0x8892, 0);
    });
    glVertexAttribPointer(env, 0, 2, 0x1406, 0, 0, positions.cast().cast_const());
    glEnableVertexAttribArray(env, 0);
    glEnableVertexAttribArray(env, 1);
    // Preserve an independent global ARRAY_BUFFER binding across staging.
    with_ctx_and_mem(env, |gles, _| unsafe {
        gles.BindBuffer(0x8892, colour_buffer);
    });
    clear(env);
    draw_arrays(env, 4, 3, 3);
    pixel(env, [64, 128, 191, 255]);
    assert_eq!(
        with_api(env, |api, _| api.integer(0x8894)) as u32,
        colour_buffer
    );
    assert_eq!(with_api(env, |api, _| api.attribute_integer(0, 0x889f)), 0);
    assert_eq!(
        with_api(env, |api, _| api.attribute_integer(1, 0x889f)) as u32,
        colour_buffer
    );
    // The client address is sampled at draw time, not at pointer registration.
    for index in 6..12 {
        env.mem.write(positions + index, -8.0);
    }
    clear(env);
    draw_arrays(env, 4, 3, 3);
    pixel(env, [0, 0, 0, 255]);
    for (index, &value) in data.iter().enumerate() {
        env.mem.write(positions + index as u32, value);
    }
    let indices = env.mem.alloc(3).cast::<u8>();
    env.mem.bytes_at_mut(indices, 3).copy_from_slice(&[3, 4, 5]);
    clear(env);
    draw_elements(env, 4, 3, 0x1401, indices.cast().cast_const());
    pixel(env, [64, 128, 191, 255]);
    let element_buffer = with_ctx_and_mem(env, |gles, _| unsafe {
        let mut name = 0;
        gles.GenBuffers(1, &mut name);
        gles.BindBuffer(0x8893, name);
        let elements: [u16; 4] = [0, 3, 4, 5];
        gles.BufferData(0x8893, 8, elements.as_ptr().cast(), 0x88e4);
        name
    });
    clear(env);
    draw_elements(env, 4, 3, 0x1403, ConstPtr::from_bits(2));
    pixel(env, [64, 128, 191, 255]);
    draw_elements(env, 4, 4, 0x1403, ConstPtr::from_bits(2));
    assert_eq!(super::super::gles_guest::glGetError(env), 0x0502);
    draw_arrays(env, 4, -1, 3);
    assert_eq!(super::super::gles_guest::glGetError(env), 0x0501);
    draw_arrays(env, 4, 0, 0);
    assert_eq!(super::super::gles_guest::glGetError(env), 0);
    glDisableVertexAttribArray(env, 0);
    glDisableVertexAttribArray(env, 1);
    with_ctx_and_mem(env, |gles, _| unsafe {
        gles.DeleteBuffers(1, &colour_buffer);
        gles.DeleteBuffers(1, &element_buffer);
    });
    with_api(env, |api, _| {
        api.use_program(0);
        api.delete_program(program);
        api.delete_shader(vertex);
        api.delete_shader(fragment);
    });
    env.mem.free(positions.cast());
    env.mem.free(indices.cast());
    echo!("GLES2_CLIENT_ARRAY_RENDER_PASS: checked draw-time snapshots, mixed client/VBO attributes, nonzero first, byte client indices, ushort EBO offset, state restoration and GPU pixels verified");
    Ok(())
}
