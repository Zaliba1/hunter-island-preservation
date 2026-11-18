/* SPDX-License-Identifier: MPL-2.0 */
//! Native Android GLES3 context for the guest GLES2 shader API.
//! GLES1 UI rendering continues to use its own existing GLES1 context.
use super::{gles11_raw::types::*, GLES, GLESContext};
use super::shader_raw as gl;
use crate::window::{GLContext, GLVersion, Window};
use std::marker::PhantomData;
pub struct GLES2NativeContext { context: GLContext, loaded: bool, presenter:Option<super::gles2_present::Presenter> }
impl GLESContext for GLES2NativeContext {
    fn description() -> &'static str { "Native Android GLES3 for guest GLES2" }
    fn new(window: &mut Window) -> Result<Self,String> {
        assert!(window.on_main_stack(), "Android GLES context creation requires the parent stack");
        Ok(Self {context:window.create_gl_context(GLVersion::GLES30)?, loaded:false,presenter:None})
    }
    fn make_current<'a,'w:'a>(&'a mut self, window:&'w mut Window)->Box<dyn GLES+'a> {
        unsafe { window.make_gl_context_current(&self.context); }
        if !self.loaded {gl::load_with(|name|window.gl_get_proc_address(name));self.loaded=true;}
        Box::new(GLES2Native{_lifetime:PhantomData,presenter:&mut self.presenter})
    }
    unsafe fn make_current_unchecked_for_window<'a>(&'a mut self,
        make:&mut dyn FnMut(&GLContext),load:&mut dyn FnMut(&'static str)->*const std::ffi::c_void)->Box<dyn GLES+'a> {
        make(&self.context);
        if !self.loaded {gl::load_with(load);self.loaded=true;}
        Box::new(GLES2Native{_lifetime:PhantomData,presenter:&mut self.presenter})
    }
}
pub struct GLES2Native<'a>{_lifetime:PhantomData<&'a ()>,presenter:&'a mut Option<super::gles2_present::Presenter>}
#[allow(unused_variables)]
impl GLES for GLES2Native<'_> {
    fn present_shader_renderbuffer(&mut self,viewport:(u32,u32,u32,u32),rotation:crate::matrix::Matrix<2>,nearest:bool)->bool {
        let mut presenter=self.presenter.take().unwrap_or_else(||unsafe{super::gles2_present::Presenter::new()});
        unsafe{presenter.draw(self,viewport,rotation,nearest)};
        *self.presenter=Some(presenter);true
    }
    fn supports_gles2_shaders(&self)->bool{true}
    fn supports_vertex_arrays(&self)->bool{gl::GenVertexArrays::is_loaded() && gl::BindVertexArray::is_loaded()}
    fn max_vertex_attributes(&self)->GLint{let mut n=0;unsafe{gl::GetIntegerv(gl::MAX_VERTEX_ATTRIBS,&mut n)};n}
    unsafe fn driver_description(&self) -> String { [gl::VERSION,gl::VENDOR,gl::RENDERER].map(|name| {
    let p=gl::GetString(name);if p.is_null(){"unknown".into()}else{std::ffi::CStr::from_ptr(p.cast()).to_string_lossy().into_owned()}
 }).join(" / ") }
    unsafe fn GetError(&mut self) -> GLenum { gl::GetError() }
    unsafe fn Enable(&mut self, cap: GLenum) { gl::Enable(cap) }
    unsafe fn IsEnabled(&mut self, cap: GLenum) -> GLboolean { gl::IsEnabled(cap) }
    unsafe fn Disable(&mut self, cap: GLenum) { gl::Disable(cap) }
    unsafe fn ClientActiveTexture(&mut self, texture: GLenum) { unimplemented!("GLES1 fixed-function call ClientActiveTexture in native Android shader context") }
    unsafe fn EnableClientState(&mut self, array: GLenum) { unimplemented!("GLES1 fixed-function call EnableClientState in native Android shader context") }
    unsafe fn DisableClientState(&mut self, array: GLenum) { unimplemented!("GLES1 fixed-function call DisableClientState in native Android shader context") }
    unsafe fn GetBooleanv(&mut self, pname: GLenum, params: *mut GLboolean) { gl::GetBooleanv(pname,params) }
    unsafe fn GetFloatv(&mut self, pname: GLenum, params: *mut GLfloat) { gl::GetFloatv(pname,params) }
    unsafe fn GetIntegerv(&mut self, pname: GLenum, params: *mut GLint) { gl::GetIntegerv(pname,params) }
    unsafe fn GetTexEnviv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) { unimplemented!("GLES1 fixed-function call GetTexEnviv in native Android shader context") }
    unsafe fn GetTexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfloat) { unimplemented!("GLES1 fixed-function call GetTexEnvfv in native Android shader context") }
    unsafe fn GetPointerv(&mut self, pname: GLenum, params: *mut *const GLvoid) { unimplemented!("GLES1 fixed-function call GetPointerv in native Android shader context") }
    unsafe fn Hint(&mut self, target: GLenum, mode: GLenum) { gl::Hint(target,mode) }
    unsafe fn Finish(&mut self) { gl::Finish() }
    unsafe fn Flush(&mut self) { gl::Flush() }
    unsafe fn GetString(&mut self, name: GLenum) -> *const GLubyte { gl::GetString(name) }
    unsafe fn AlphaFunc(&mut self, func: GLenum, ref_: GLclampf) { unimplemented!("GLES1 fixed-function call AlphaFunc in native Android shader context") }
    unsafe fn AlphaFuncx(&mut self, func: GLenum, ref_: GLclampx) { unimplemented!("GLES1 fixed-function call AlphaFuncx in native Android shader context") }
    unsafe fn BlendFunc(&mut self, sfactor: GLenum, dfactor: GLenum) { gl::BlendFunc(sfactor,dfactor) }
    unsafe fn BlendEquationOES(&mut self, mode: GLenum) { gl::BlendEquation(mode) }
    unsafe fn ColorMask(&mut self, red: GLboolean, green: GLboolean, blue: GLboolean, alpha: GLboolean,) { gl::ColorMask(red,green,blue,alpha) }
    unsafe fn ClipPlanef(&mut self, plane: GLenum, equation: *const GLfloat) { unimplemented!("GLES1 fixed-function call ClipPlanef in native Android shader context") }
    unsafe fn ClipPlanex(&mut self, plane: GLenum, equation: *const GLfixed) { unimplemented!("GLES1 fixed-function call ClipPlanex in native Android shader context") }
    unsafe fn CullFace(&mut self, mode: GLenum) { gl::CullFace(mode) }
    unsafe fn DepthFunc(&mut self, func: GLenum) { gl::DepthFunc(func) }
    unsafe fn DepthMask(&mut self, flag: GLboolean) { gl::DepthMask(flag) }
    unsafe fn DepthRangef(&mut self, near: GLclampf, far: GLclampf) { gl::DepthRangef(near,far) }
    unsafe fn DepthRangex(&mut self, near: GLclampx, far: GLclampx) { unimplemented!("GLES1 fixed-function call DepthRangex in native Android shader context") }
    unsafe fn FrontFace(&mut self, mode: GLenum) { gl::FrontFace(mode) }
    unsafe fn PolygonOffset(&mut self, factor: GLfloat, units: GLfloat) { gl::PolygonOffset(factor,units) }
    unsafe fn PolygonOffsetx(&mut self, factor: GLfixed, units: GLfixed) { unimplemented!("GLES1 fixed-function call PolygonOffsetx in native Android shader context") }
    unsafe fn SampleCoverage(&mut self, value: GLclampf, invert: GLboolean) { gl::SampleCoverage(value,invert) }
    unsafe fn SampleCoveragex(&mut self, value: GLclampx, invert: GLboolean) { unimplemented!("GLES1 fixed-function call SampleCoveragex in native Android shader context") }
    unsafe fn ShadeModel(&mut self, mode: GLenum) { unimplemented!("GLES1 fixed-function call ShadeModel in native Android shader context") }
    unsafe fn Scissor(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { gl::Scissor(x,y,width,height) }
    unsafe fn Viewport(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { gl::Viewport(x,y,width,height) }
    unsafe fn LineWidth(&mut self, val: GLfloat) { gl::LineWidth(val) }
    unsafe fn LineWidthx(&mut self, val: GLfixed) { unimplemented!("GLES1 fixed-function call LineWidthx in native Android shader context") }
    unsafe fn StencilFunc(&mut self, func: GLenum, ref_: GLint, mask: GLuint) { gl::StencilFunc(func,ref_,mask) }
    unsafe fn StencilOp(&mut self, sfail: GLenum, dpfail: GLenum, dppass: GLenum) { gl::StencilOp(sfail,dpfail,dppass) }
    unsafe fn StencilMask(&mut self, mask: GLuint) { gl::StencilMask(mask) }
    unsafe fn LogicOp(&mut self, opcode: GLenum) { unimplemented!("GLES1 fixed-function call LogicOp in native Android shader context") }
    unsafe fn PointSize(&mut self, size: GLfloat) { unimplemented!("GLES1 fixed-function call PointSize in native Android shader context") }
    unsafe fn PointSizex(&mut self, size: GLfixed) { unimplemented!("GLES1 fixed-function call PointSizex in native Android shader context") }
    unsafe fn PointParameterf(&mut self, pname: GLenum, param: GLfloat) { unimplemented!("GLES1 fixed-function call PointParameterf in native Android shader context") }
    unsafe fn PointParameterx(&mut self, pname: GLenum, param: GLfixed) { unimplemented!("GLES1 fixed-function call PointParameterx in native Android shader context") }
    unsafe fn PointParameterfv(&mut self, pname: GLenum, params: *const GLfloat) { unimplemented!("GLES1 fixed-function call PointParameterfv in native Android shader context") }
    unsafe fn PointParameterxv(&mut self, pname: GLenum, params: *const GLfixed) { unimplemented!("GLES1 fixed-function call PointParameterxv in native Android shader context") }
    unsafe fn Fogf(&mut self, pname: GLenum, param: GLfloat) { unimplemented!("GLES1 fixed-function call Fogf in native Android shader context") }
    unsafe fn Fogx(&mut self, pname: GLenum, param: GLfixed) { unimplemented!("GLES1 fixed-function call Fogx in native Android shader context") }
    unsafe fn Fogfv(&mut self, pname: GLenum, params: *const GLfloat) { unimplemented!("GLES1 fixed-function call Fogfv in native Android shader context") }
    unsafe fn Fogxv(&mut self, pname: GLenum, params: *const GLfixed) { unimplemented!("GLES1 fixed-function call Fogxv in native Android shader context") }
    unsafe fn Lightf(&mut self, light: GLenum, pname: GLenum, param: GLfloat) { unimplemented!("GLES1 fixed-function call Lightf in native Android shader context") }
    unsafe fn Lightx(&mut self, light: GLenum, pname: GLenum, param: GLfixed) { unimplemented!("GLES1 fixed-function call Lightx in native Android shader context") }
    unsafe fn Lightfv(&mut self, light: GLenum, pname: GLenum, params: *const GLfloat) { unimplemented!("GLES1 fixed-function call Lightfv in native Android shader context") }
    unsafe fn Lightxv(&mut self, light: GLenum, pname: GLenum, params: *const GLfixed) { unimplemented!("GLES1 fixed-function call Lightxv in native Android shader context") }
    unsafe fn LightModelf(&mut self, pname: GLenum, param: GLfloat) { unimplemented!("GLES1 fixed-function call LightModelf in native Android shader context") }
    unsafe fn LightModelx(&mut self, pname: GLenum, param: GLfixed) { unimplemented!("GLES1 fixed-function call LightModelx in native Android shader context") }
    unsafe fn LightModelfv(&mut self, pname: GLenum, params: *const GLfloat) { unimplemented!("GLES1 fixed-function call LightModelfv in native Android shader context") }
    unsafe fn LightModelxv(&mut self, pname: GLenum, params: *const GLfixed) { unimplemented!("GLES1 fixed-function call LightModelxv in native Android shader context") }
    unsafe fn Materialf(&mut self, face: GLenum, pname: GLenum, param: GLfloat) { unimplemented!("GLES1 fixed-function call Materialf in native Android shader context") }
    unsafe fn Materialx(&mut self, face: GLenum, pname: GLenum, param: GLfixed) { unimplemented!("GLES1 fixed-function call Materialx in native Android shader context") }
    unsafe fn Materialfv(&mut self, face: GLenum, pname: GLenum, params: *const GLfloat) { unimplemented!("GLES1 fixed-function call Materialfv in native Android shader context") }
    unsafe fn Materialxv(&mut self, face: GLenum, pname: GLenum, params: *const GLfixed) { unimplemented!("GLES1 fixed-function call Materialxv in native Android shader context") }
    unsafe fn IsBuffer(&mut self, buffer: GLuint) -> GLboolean { gl::IsBuffer(buffer) }
    unsafe fn GenBuffers(&mut self, n: GLsizei, buffers: *mut GLuint) { gl::GenBuffers(n,buffers) }
    unsafe fn DeleteBuffers(&mut self, n: GLsizei, buffers: *const GLuint) { gl::DeleteBuffers(n,buffers) }
    unsafe fn BindBuffer(&mut self, target: GLenum, buffer: GLuint) { gl::BindBuffer(target,buffer) }
    unsafe fn BufferData(&mut self, target: GLenum, size: GLsizeiptr, data: *const GLvoid, usage: GLenum,) { gl::BufferData(target,size,data,usage) }
    unsafe fn BufferSubData(&mut self, target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *const GLvoid,) { gl::BufferSubData(target,offset,size,data) }
    unsafe fn Color4f(&mut self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) { unimplemented!("GLES1 fixed-function call Color4f in native Android shader context") }
    unsafe fn Color4x(&mut self, red: GLfixed, green: GLfixed, blue: GLfixed, alpha: GLfixed) { unimplemented!("GLES1 fixed-function call Color4x in native Android shader context") }
    unsafe fn Color4ub(&mut self, red: GLubyte, green: GLubyte, blue: GLubyte, alpha: GLubyte) { unimplemented!("GLES1 fixed-function call Color4ub in native Android shader context") }
    unsafe fn Normal3f(&mut self, nx: GLfloat, ny: GLfloat, nz: GLfloat) { unimplemented!("GLES1 fixed-function call Normal3f in native Android shader context") }
    unsafe fn Normal3x(&mut self, nx: GLfixed, ny: GLfixed, nz: GLfixed) { unimplemented!("GLES1 fixed-function call Normal3x in native Android shader context") }
    unsafe fn ColorPointer(&mut self, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const GLvoid,) { unimplemented!("GLES1 fixed-function call ColorPointer in native Android shader context") }
    unsafe fn NormalPointer(&mut self, type_: GLenum, stride: GLsizei, pointer: *const GLvoid) { unimplemented!("GLES1 fixed-function call NormalPointer in native Android shader context") }
    unsafe fn TexCoordPointer(&mut self, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const GLvoid,) { unimplemented!("GLES1 fixed-function call TexCoordPointer in native Android shader context") }
    unsafe fn VertexPointer(&mut self, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const GLvoid,) { unimplemented!("GLES1 fixed-function call VertexPointer in native Android shader context") }
    unsafe fn DrawArrays(&mut self, mode: GLenum, first: GLint, count: GLsizei) { gl::DrawArrays(mode,first,count) }
    unsafe fn DrawElements(&mut self, mode: GLenum, count: GLsizei, type_: GLenum, indices: *const GLvoid,) { gl::DrawElements(mode,count,type_,indices) }
    unsafe fn Clear(&mut self, mask: GLbitfield) { gl::Clear(mask) }
    unsafe fn ClearColor(&mut self, red: GLclampf, green: GLclampf, blue: GLclampf, alpha: GLclampf,) { gl::ClearColor(red,green,blue,alpha) }
    unsafe fn ClearColorx(&mut self, red: GLclampx, green: GLclampx, blue: GLclampx, alpha: GLclampx,) { unimplemented!("GLES1 fixed-function call ClearColorx in native Android shader context") }
    unsafe fn ClearDepthf(&mut self, depth: GLclampf) { gl::ClearDepthf(depth) }
    unsafe fn ClearDepthx(&mut self, depth: GLclampx) { unimplemented!("GLES1 fixed-function call ClearDepthx in native Android shader context") }
    unsafe fn ClearStencil(&mut self, s: GLint) { gl::ClearStencil(s) }
    unsafe fn PixelStorei(&mut self, pname: GLenum, param: GLint) { gl::PixelStorei(pname,param) }
    unsafe fn ReadPixels(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *mut GLvoid,) { gl::ReadPixels(x,y,width,height,format,type_,pixels) }
    unsafe fn GenTextures(&mut self, n: GLsizei, textures: *mut GLuint) { gl::GenTextures(n,textures) }
    unsafe fn DeleteTextures(&mut self, n: GLsizei, textures: *const GLuint) { gl::DeleteTextures(n,textures) }
    unsafe fn ActiveTexture(&mut self, texture: GLenum) { gl::ActiveTexture(texture) }
    unsafe fn IsTexture(&mut self, texture: GLuint) -> GLboolean { gl::IsTexture(texture) }
    unsafe fn BindTexture(&mut self, target: GLenum, texture: GLuint) { gl::BindTexture(target,texture) }
    unsafe fn TexParameteri(&mut self, target: GLenum, pname: GLenum, param: GLint) { gl::TexParameteri(target,pname,param) }
    unsafe fn TexParameterf(&mut self, target: GLenum, pname: GLenum, param: GLfloat) { gl::TexParameterf(target,pname,param) }
    unsafe fn TexParameterx(&mut self, target: GLenum, pname: GLenum, param: GLfixed) { unimplemented!("GLES1 fixed-function call TexParameterx in native Android shader context") }
    unsafe fn TexParameteriv(&mut self, target: GLenum, pname: GLenum, params: *const GLint) { gl::TexParameteriv(target,pname,params) }
    unsafe fn TexParameterfv(&mut self, target: GLenum, pname: GLenum, params: *const GLfloat) { gl::TexParameterfv(target,pname,params) }
    unsafe fn TexParameterxv(&mut self, target: GLenum, pname: GLenum, params: *const GLfixed) { unimplemented!("GLES1 fixed-function call TexParameterxv in native Android shader context") }
    unsafe fn TexImage2D(&mut self, target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const GLvoid,) { let internalformat=if format==0x80e1 {0x80e1}else{internalformat};
 gl::TexImage2D(target,level,internalformat,width,height,border,format,type_,pixels) }
    unsafe fn TexSubImage2D(&mut self, target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *const GLvoid,) { gl::TexSubImage2D(target,level,xoffset,yoffset,width,height,format,type_,pixels) }
    unsafe fn CompressedTexImage2D(&mut self, target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, height: GLsizei, border: GLint, image_size: GLsizei, data: *const GLvoid,) { assert!(image_size>=0);
 let bytes=std::slice::from_raw_parts(data.cast::<u8>(),image_size as usize);
 if !super::util::try_decode_pvrtc(self,target,level,internalformat,width,height,border,bytes) {
    gl::CompressedTexImage2D(target,level,internalformat,width,height,border,image_size,data);
 } }
    unsafe fn CopyTexImage2D(&mut self, target: GLenum, level: GLint, internalformat: GLenum, x: GLint, y: GLint, width: GLsizei, height: GLsizei, border: GLint,) { gl::CopyTexImage2D(target,level,internalformat,x,y,width,height,border) }
    unsafe fn CopyTexSubImage2D(&mut self, target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, x: GLint, y: GLint, width: GLsizei, height: GLsizei,) { gl::CopyTexSubImage2D(target,level,xoffset,yoffset,x,y,width,height) }
    unsafe fn TexEnvf(&mut self, target: GLenum, pname: GLenum, param: GLfloat) { unimplemented!("GLES1 fixed-function call TexEnvf in native Android shader context") }
    unsafe fn TexEnvx(&mut self, target: GLenum, pname: GLenum, param: GLfixed) { unimplemented!("GLES1 fixed-function call TexEnvx in native Android shader context") }
    unsafe fn TexEnvi(&mut self, target: GLenum, pname: GLenum, param: GLint) { unimplemented!("GLES1 fixed-function call TexEnvi in native Android shader context") }
    unsafe fn TexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *const GLfloat) { unimplemented!("GLES1 fixed-function call TexEnvfv in native Android shader context") }
    unsafe fn TexEnvxv(&mut self, target: GLenum, pname: GLenum, params: *const GLfixed) { unimplemented!("GLES1 fixed-function call TexEnvxv in native Android shader context") }
    unsafe fn TexEnviv(&mut self, target: GLenum, pname: GLenum, params: *const GLint) { unimplemented!("GLES1 fixed-function call TexEnviv in native Android shader context") }
    unsafe fn MultiTexCoord4f(&mut self, target: GLenum, s: GLfloat, t: GLfloat, r: GLfloat, q: GLfloat,) { unimplemented!("GLES1 fixed-function call MultiTexCoord4f in native Android shader context") }
    unsafe fn MultiTexCoord4x(&mut self, target: GLenum, s: GLfixed, t: GLfixed, r: GLfixed, q: GLfixed,) { unimplemented!("GLES1 fixed-function call MultiTexCoord4x in native Android shader context") }
    unsafe fn MatrixMode(&mut self, mode: GLenum) { unimplemented!("GLES1 fixed-function call MatrixMode in native Android shader context") }
    unsafe fn LoadIdentity(&mut self) { unimplemented!("GLES1 fixed-function call LoadIdentity in native Android shader context") }
    unsafe fn LoadMatrixf(&mut self, m: *const GLfloat) { unimplemented!("GLES1 fixed-function call LoadMatrixf in native Android shader context") }
    unsafe fn LoadMatrixx(&mut self, m: *const GLfixed) { unimplemented!("GLES1 fixed-function call LoadMatrixx in native Android shader context") }
    unsafe fn MultMatrixf(&mut self, m: *const GLfloat) { unimplemented!("GLES1 fixed-function call MultMatrixf in native Android shader context") }
    unsafe fn MultMatrixx(&mut self, m: *const GLfixed) { unimplemented!("GLES1 fixed-function call MultMatrixx in native Android shader context") }
    unsafe fn PushMatrix(&mut self) { unimplemented!("GLES1 fixed-function call PushMatrix in native Android shader context") }
    unsafe fn PopMatrix(&mut self) { unimplemented!("GLES1 fixed-function call PopMatrix in native Android shader context") }
    unsafe fn Orthof(&mut self, left: GLfloat, right: GLfloat, bottom: GLfloat, top: GLfloat, near: GLfloat, far: GLfloat,) { unimplemented!("GLES1 fixed-function call Orthof in native Android shader context") }
    unsafe fn Orthox(&mut self, left: GLfixed, right: GLfixed, bottom: GLfixed, top: GLfixed, near: GLfixed, far: GLfixed,) { unimplemented!("GLES1 fixed-function call Orthox in native Android shader context") }
    unsafe fn Frustumf(&mut self, left: GLfloat, right: GLfloat, bottom: GLfloat, top: GLfloat, near: GLfloat, far: GLfloat,) { unimplemented!("GLES1 fixed-function call Frustumf in native Android shader context") }
    unsafe fn Frustumx(&mut self, left: GLfixed, right: GLfixed, bottom: GLfixed, top: GLfixed, near: GLfixed, far: GLfixed,) { unimplemented!("GLES1 fixed-function call Frustumx in native Android shader context") }
    unsafe fn Rotatef(&mut self, angle: GLfloat, x: GLfloat, y: GLfloat, z: GLfloat) { unimplemented!("GLES1 fixed-function call Rotatef in native Android shader context") }
    unsafe fn Rotatex(&mut self, angle: GLfixed, x: GLfixed, y: GLfixed, z: GLfixed) { unimplemented!("GLES1 fixed-function call Rotatex in native Android shader context") }
    unsafe fn Scalef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) { unimplemented!("GLES1 fixed-function call Scalef in native Android shader context") }
    unsafe fn Scalex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) { unimplemented!("GLES1 fixed-function call Scalex in native Android shader context") }
    unsafe fn Translatef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) { unimplemented!("GLES1 fixed-function call Translatef in native Android shader context") }
    unsafe fn Translatex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) { unimplemented!("GLES1 fixed-function call Translatex in native Android shader context") }
    unsafe fn GenFramebuffersOES(&mut self, n: GLsizei, framebuffers: *mut GLuint) { gl::GenFramebuffers(n,framebuffers) }
    unsafe fn GenRenderbuffersOES(&mut self, n: GLsizei, renderbuffers: *mut GLuint) { gl::GenRenderbuffers(n,renderbuffers) }
    unsafe fn IsFramebufferOES(&mut self, framebuffer: GLuint) -> GLboolean { gl::IsFramebuffer(framebuffer) }
    unsafe fn IsRenderbufferOES(&mut self, renderbuffer: GLuint) -> GLboolean { gl::IsRenderbuffer(renderbuffer) }
    unsafe fn BindFramebufferOES(&mut self, target: GLenum, framebuffer: GLuint) { gl::BindFramebuffer(target,framebuffer) }
    unsafe fn BindRenderbufferOES(&mut self, target: GLenum, renderbuffer: GLuint) { gl::BindRenderbuffer(target,renderbuffer) }
    unsafe fn RenderbufferStorageOES(&mut self, target: GLenum, internalformat: GLenum, width: GLsizei, height: GLsizei,) { gl::RenderbufferStorage(target,internalformat,width,height) }
    unsafe fn FramebufferRenderbufferOES(&mut self, target: GLenum, attachment: GLenum, renderbuffertarget: GLenum, renderbuffer: GLuint,) { gl::FramebufferRenderbuffer(target,attachment,renderbuffertarget,renderbuffer) }
    unsafe fn FramebufferTexture2DOES(&mut self, target: GLenum, attachment: GLenum, textarget: GLenum, texture: GLuint, level: i32,) { gl::FramebufferTexture2D(target,attachment,textarget,texture,level) }
    unsafe fn GetFramebufferAttachmentParameterivOES(&mut self, target: GLenum, attachment: GLenum, pname: GLenum, params: *mut GLint,) { gl::GetFramebufferAttachmentParameteriv(target,attachment,pname,params) }
    unsafe fn GetRenderbufferParameterivOES(&mut self, target: GLenum, pname: GLenum, params: *mut GLint,) { gl::GetRenderbufferParameteriv(target,pname,params) }
    unsafe fn CheckFramebufferStatusOES(&mut self, target: GLenum) -> GLenum { gl::CheckFramebufferStatus(target) }
    unsafe fn DeleteFramebuffersOES(&mut self, n: GLsizei, framebuffers: *const GLuint) { gl::DeleteFramebuffers(n,framebuffers) }
    unsafe fn DeleteRenderbuffersOES(&mut self, n: GLsizei, renderbuffers: *const GLuint) { gl::DeleteRenderbuffers(n,renderbuffers) }
    unsafe fn GenerateMipmapOES(&mut self, target: GLenum) { gl::GenerateMipmap(target) }
    unsafe fn GetBufferParameteriv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) { gl::GetBufferParameteriv(target,pname,params) }
    unsafe fn MapBufferOES(&mut self, target: GLenum, access: GLenum) -> *mut GLvoid { assert_eq!(access,0x88b9,"Only OES write-only buffer mapping is supported");
 let mut length=0;gl::GetBufferParameteriv(target,gl::BUFFER_SIZE,&mut length);
 gl::MapBufferRange(target,0,length as isize,gl::MAP_WRITE_BIT) }
    unsafe fn UnmapBufferOES(&mut self, target: GLenum) -> GLboolean { gl::UnmapBuffer(target) }
}
