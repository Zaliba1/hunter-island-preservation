# SPDX-License-Identifier: MPL-2.0
"""Generate the explicit GLES trait adapter from the checked-in trait and Khronos registry.

Only GLES2/3 common functions are forwarded. GLES1 fixed-function entry points
fail explicitly in this shader context; the separate GLES1 renderer is retained.
"""
from pathlib import Path
import re,xml.etree.ElementTree as ET
root=Path(__file__).resolve().parents[1];engine=root/'emulator';src=engine/'src'
registry=ET.parse(engine/'vendor/rust/khronos_api-3.1.0/api/xml/gl.xml').getroot()
commands=set()
for feature in registry.findall('feature'):
 if feature.attrib.get('api')=='gles2' and float(feature.attrib['number'])<=3:
  for require in feature.findall('require'):
   commands.update(c.attrib['name'][2:] for c in require.findall('command'))
trait=(src/'gles/gles_generic.rs').read_text()
methods=re.findall(r'unsafe fn (\w+)\s*\((.*?)\)\s*(?:->\s*([^;]+))?;',trait,re.S)
assert len(methods)>100
head='''/* SPDX-License-Identifier: MPL-2.0 */
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
    fn present_shader_renderbuffer(&mut self,viewport:(u32,u32,u32,u32),rotation:crate::matrix::Matrix<2>)->bool {
        let mut presenter=self.presenter.take().unwrap_or_else(||unsafe{super::gles2_present::Presenter::new()});
        unsafe{presenter.draw(self,viewport,rotation)};
        *self.presenter=Some(presenter);true
    }
    fn supports_gles2_shaders(&self)->bool{true}
    fn supports_vertex_arrays(&self)->bool{gl::GenVertexArrays::is_loaded() && gl::BindVertexArray::is_loaded()}
    fn max_vertex_attributes(&self)->GLint{let mut n=0;unsafe{gl::GetIntegerv(gl::MAX_VERTEX_ATTRIBS,&mut n)};n}
'''
special={
 'driver_description':'''[gl::VERSION,gl::VENDOR,gl::RENDERER].map(|name| {
    let p=gl::GetString(name);if p.is_null(){"unknown".into()}else{std::ffi::CStr::from_ptr(p.cast()).to_string_lossy().into_owned()}
 }).join(" / ")''',
 'TexImage2D':'''let internalformat=if format==0x80e1 {0x80e1}else{internalformat};
 gl::TexImage2D(target,level,internalformat,width,height,border,format,type_,pixels)''',
 'CompressedTexImage2D':'''assert!(image_size>=0);
 let bytes=std::slice::from_raw_parts(data.cast::<u8>(),image_size as usize);
 if !super::util::try_decode_pvrtc(self,target,level,internalformat,width,height,border,bytes) {
    gl::CompressedTexImage2D(target,level,internalformat,width,height,border,image_size,data);
 }''',
 'MapBufferOES':'''assert_eq!(access,0x88b9,"Only OES write-only buffer mapping is supported");
 let mut length=0;gl::GetBufferParameteriv(target,gl::BUFFER_SIZE,&mut length);
 gl::MapBufferRange(target,0,length as isize,gl::MAP_WRITE_BIT)''',
 'UnmapBufferOES':'gl::UnmapBuffer(target)',
}
out=[head];forwarded=[];unsupported=[]
for name,signature,returns in methods:
 signature=' '.join(signature.split());ret=(' -> '+returns.strip()) if returns else ''
 args=[part.strip().split(':',1)[0].strip() for part in signature.split(',') if part.strip() and not part.strip().startswith('&')]
 native=name[:-3] if name.endswith('OES') else name
 if name in special:body=special[name];forwarded.append(name)
 elif native in commands:body='gl::'+native+'('+','.join(args)+')';forwarded.append(name)
 else:body='unimplemented!("GLES1 fixed-function call '+name+' in native Android shader context")';unsupported.append(name)
 out.append('    unsafe fn '+name+'('+signature+')'+ret+' { '+body+' }\n')
out.append('}\n')
(src/'gles/gles2_native.rs').write_text(''.join(out))
print(f'Native shader context: {len(forwarded)} implemented operations; {len(unsupported)} GLES1-only calls rejected explicitly.')
