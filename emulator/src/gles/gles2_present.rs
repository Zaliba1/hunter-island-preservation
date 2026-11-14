/* SPDX-License-Identifier: MPL-2.0 */
//! GLES3-only presenter. No GLES1 matrix/client-array entry points are used.
use super::{shader_raw as gl,GLES};
use crate::matrix::Matrix;
use std::ffi::CString;

pub struct Presenter {program:u32,vao:u32,vbo:u32,texture:u32,size:(i32,i32),rotation:i32}
unsafe fn shader(kind:u32,source:&str)->u32 {
    let shader=gl::CreateShader(kind);let text=CString::new(source).unwrap();
    gl::ShaderSource(shader,1,&text.as_ptr(),std::ptr::null());gl::CompileShader(shader);
    let mut ok=0;gl::GetShaderiv(shader,gl::COMPILE_STATUS,&mut ok);
    if ok==0 {let mut log=vec![0u8;4096];let mut n=0;gl::GetShaderInfoLog(shader,4096,&mut n,log.as_mut_ptr().cast());panic!("Android presenter shader: {}",String::from_utf8_lossy(&log[..n.max(0) as usize]));}
    shader
}
impl Presenter {
    pub unsafe fn new()->Self {
        let vertex=shader(gl::VERTEX_SHADER,"#version 100\nattribute vec2 position; attribute vec2 texcoord; uniform mat2 rotation; varying vec2 uv; void main(){gl_Position=vec4(position,0.,1.); uv=rotation*(texcoord-vec2(.5))+vec2(.5);}");
        let fragment=shader(gl::FRAGMENT_SHADER,"#version 100\nprecision mediump float; uniform sampler2D image; varying vec2 uv; void main(){gl_FragColor=texture2D(image,uv);}");
        let program=gl::CreateProgram();gl::AttachShader(program,vertex);gl::AttachShader(program,fragment);
        gl::BindAttribLocation(program,0,c"position".as_ptr());gl::BindAttribLocation(program,1,c"texcoord".as_ptr());gl::LinkProgram(program);
        let mut ok=0;gl::GetProgramiv(program,gl::LINK_STATUS,&mut ok);assert_ne!(ok,0,"Android presenter program link failed");
        gl::DeleteShader(vertex);gl::DeleteShader(fragment);
        let mut vao=0;let mut vbo=0;let mut texture=0;gl::GenVertexArrays(1,&mut vao);gl::GenBuffers(1,&mut vbo);gl::GenTextures(1,&mut texture);
        Self{program,vao,vbo,texture,size:(0,0),rotation:gl::GetUniformLocation(program,c"rotation".as_ptr())}
    }
    pub unsafe fn draw(&mut self,context:&mut dyn GLES,viewport:(u32,u32,u32,u32),rotation:Matrix<2>) {
        let state=super::gles2_shaders::pause_program(context);
        fn int(name:u32)->i32{let mut n=0;unsafe{gl::GetIntegerv(name,&mut n)};n}
        let old_fbo=int(gl::FRAMEBUFFER_BINDING) as u32;
        let old_active=int(gl::ACTIVE_TEXTURE) as u32;
        let mut old_viewport=[0i32;4];gl::GetIntegerv(gl::VIEWPORT,old_viewport.as_mut_ptr());
        let mut old_clear=[0f32;4];gl::GetFloatv(gl::COLOR_CLEAR_VALUE,old_clear.as_mut_ptr());
        let mut old_mask=[0u8;4];gl::GetBooleanv(gl::COLOR_WRITEMASK,old_mask.as_mut_ptr());
        let capabilities=[gl::BLEND,gl::CULL_FACE,gl::DEPTH_TEST,gl::SCISSOR_TEST,gl::STENCIL_TEST,gl::POLYGON_OFFSET_FILL,gl::SAMPLE_ALPHA_TO_COVERAGE,gl::SAMPLE_COVERAGE,gl::RASTERIZER_DISCARD];
        let old_caps=capabilities.map(|cap|gl::IsEnabled(cap));
        for cap in capabilities{gl::Disable(cap)}
        gl::ActiveTexture(gl::TEXTURE0);let old_texture=int(gl::TEXTURE_BINDING_2D) as u32;
        let mut width=0;let mut height=0;
        gl::GetRenderbufferParameteriv(gl::RENDERBUFFER,gl::RENDERBUFFER_WIDTH,&mut width);
        gl::GetRenderbufferParameteriv(gl::RENDERBUFFER,gl::RENDERBUFFER_HEIGHT,&mut height);
        assert!(width>0 && height>0 && (width as u64)*(height as u64)<=16*1024*1024,"Invalid Android drawable dimensions");
        let mut fbo=0;gl::GenFramebuffers(1,&mut fbo);gl::BindFramebuffer(gl::FRAMEBUFFER,fbo);
        let renderbuffer=int(gl::RENDERBUFFER_BINDING) as u32;
        gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,gl::COLOR_ATTACHMENT0,gl::RENDERBUFFER,renderbuffer);
        assert_eq!(gl::CheckFramebufferStatus(gl::FRAMEBUFFER),gl::FRAMEBUFFER_COMPLETE,"Android drawable framebuffer incomplete");
        gl::BindTexture(gl::TEXTURE_2D,self.texture);
        if self.size!=(width,height) {
            gl::CopyTexImage2D(gl::TEXTURE_2D,0,gl::RGBA,0,0,width,height,0);self.size=(width,height);
        } else {gl::CopyTexSubImage2D(gl::TEXTURE_2D,0,0,0,0,0,width,height);}
        gl::TexParameteri(gl::TEXTURE_2D,gl::TEXTURE_MIN_FILTER,gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D,gl::TEXTURE_MAG_FILTER,gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D,gl::TEXTURE_WRAP_S,gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D,gl::TEXTURE_WRAP_T,gl::CLAMP_TO_EDGE as i32);
        gl::BindFramebuffer(gl::FRAMEBUFFER,0);gl::DeleteFramebuffers(1,&fbo);
        gl::ColorMask(gl::TRUE,gl::TRUE,gl::TRUE,gl::TRUE);gl::ClearColor(0.,0.,0.,1.);gl::Clear(gl::COLOR_BUFFER_BIT);
        gl::Viewport(viewport.0 as i32,viewport.1 as i32,viewport.2 as i32,viewport.3 as i32);
        gl::UseProgram(self.program);gl::Uniform1i(gl::GetUniformLocation(self.program,c"image".as_ptr()),0);
        gl::UniformMatrix2fv(self.rotation,1,gl::FALSE,rotation.columns().as_ptr().cast());
        let vertices:[f32;16]=[-1.,-1.,0.,0., 1.,-1.,1.,0., -1.,1.,0.,1., 1.,1.,1.,1.];
        gl::BindVertexArray(self.vao);gl::BindBuffer(gl::ARRAY_BUFFER,self.vbo);
        gl::BufferData(gl::ARRAY_BUFFER,std::mem::size_of_val(&vertices) as isize,vertices.as_ptr().cast(),gl::STREAM_DRAW);
        gl::EnableVertexAttribArray(0);gl::EnableVertexAttribArray(1);
        gl::VertexAttribPointer(0,2,gl::FLOAT,gl::FALSE,16,std::ptr::null());
        gl::VertexAttribPointer(1,2,gl::FLOAT,gl::FALSE,16,8usize as *const _);
        gl::DrawArrays(gl::TRIANGLE_STRIP,0,4);
        gl::BindTexture(gl::TEXTURE_2D,old_texture);gl::ActiveTexture(old_active);
        gl::BindFramebuffer(gl::FRAMEBUFFER,old_fbo);
        gl::Viewport(old_viewport[0],old_viewport[1],old_viewport[2],old_viewport[3]);
        gl::ClearColor(old_clear[0],old_clear[1],old_clear[2],old_clear[3]);
        gl::ColorMask(old_mask[0],old_mask[1],old_mask[2],old_mask[3]);
        for (cap,enabled) in capabilities.into_iter().zip(old_caps){if enabled!=0{gl::Enable(cap)}else{gl::Disable(cap)}}
        // The presenter used its own VAO; restore the saved default VAO state
        // before returning the guest VAO, as expected by restore_program().
        gl::BindVertexArray(0);super::gles2_shaders::restore_program(context,state);
    }
}
