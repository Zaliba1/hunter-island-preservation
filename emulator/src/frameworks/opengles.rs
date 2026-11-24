/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! OpenGL ES and EAGL.
//!
//! This module is specific to OpenGL ES's role as a part of the iPhone OS API
//! surface. See [crate::gles] for other uses and a discussion of the broader
//! topic.

mod eagl;
pub(crate) use eagl::frame_ppm;
mod gles2_guest;
mod gles_guest;
mod texture_cleanup;
pub(crate) use gles2_guest::client_render_probe;
pub(crate) use gles2_guest::vertex_array_probe;
pub(crate) use gles_guest::framebuffer_probe;

use touchHLE_gl_bindings::gles11::types::GLenum;

use crate::mem::ConstPtr;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/OpenGLES.framework/OpenGLES",
    aliases: &[],
    class_exports: &[eagl::CLASSES],
    constant_exports: &[eagl::CONSTANTS],
    function_exports: &[gles_guest::FUNCTIONS, gles2_guest::FUNCTIONS],
};

pub(crate) struct TextMask {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) original: Vec<u8>,
    pub(crate) pixels: Vec<u8>,
}

#[derive(Default)]
pub struct State {
    pub(crate) crisp_text: bool,
    pub(crate) font_atlas: Option<crate::text_atlas::TextAtlas>,
    pub(crate) text_mask: Option<TextMask>,
    pub(crate) hd_textures: Option<crate::hd_textures::Cache>,
    texture_cleanup: texture_cleanup::Cleanup,
    #[cfg(feature = "diagnostic-live-cycles")]
    generated_textures: u64,
    #[cfg(feature = "diagnostic-live-cycles")]
    deleted_textures: u64,
    #[cfg(feature = "diagnostic-live-cycles")]
    skipped_texture_deletes: u64,
    /// Current EAGLContext for each thread
    current_ctxs: std::collections::HashMap<crate::ThreadId, Option<crate::objc::id>>,
    strings_cache: std::collections::HashMap<(u32, GLenum), ConstPtr<u8>>,
}

pub(crate) fn enable_crisp_text(env: &mut crate::Environment) {
    env.framework_state.opengles.crisp_text = true;
    env.framework_state.opengles.font_atlas = crate::text_atlas::TextAtlas::load(env);
}

pub(crate) fn enable_hd_textures(env: &mut crate::Environment, cache: crate::hd_textures::Cache) {
    env.framework_state.opengles.hd_textures = Some(cache);
}

fn current_texture_group(env: &crate::Environment) -> Option<usize> {
    let context = env
        .framework_state
        .opengles
        .current_ctx_for_thread(env.current_thread)?;
    Some(
        env.objc
            .borrow::<eagl::EAGLContextHostObject>(context)
            .texture_sharegroup_key(),
    )
}
fn take_pending_textures(env: &mut crate::Environment) -> Vec<u32> {
    if !env.options.legacy_deferred_texture_deletes
        || !env.framework_state.opengles.texture_cleanup.has_pending()
    {
        return Vec::new();
    }
    let Some(group) = current_texture_group(env) else {
        return Vec::new();
    };
    env.framework_state.opengles.texture_cleanup.take(group)
}
#[cfg(feature = "diagnostic-live-cycles")]
pub(crate) fn texture_cleanup_profile(env: &crate::Environment) -> (usize, usize, u64, u64) {
    env.framework_state.opengles.texture_cleanup.counts()
}

fn current_api(env: &crate::Environment) -> Option<u32> {
    let context = env
        .framework_state
        .opengles
        .current_ctxs
        .get(&env.current_thread)
        .copied()
        .flatten()?;
    Some(env.objc.borrow::<eagl::EAGLContextHostObject>(context).api)
}
fn guest_error(env: &mut crate::Environment, error: Option<u32>) -> u32 {
    let Some(context) = env
        .framework_state
        .opengles
        .current_ctxs
        .get(&env.current_thread)
        .copied()
        .flatten()
    else {
        return 0;
    };
    let state = env.objc.borrow_mut::<eagl::EAGLContextHostObject>(context);
    if let Some(error) = error {
        if state.guest_error == 0 {
            state.guest_error = error;
        }
        0
    } else {
        std::mem::take(&mut state.guest_error)
    }
}
impl State {
    #[cfg(feature = "diagnostic-live-cycles")]
    pub(crate) fn resource_profile(&self) -> (usize, u64, u64, u64) {
        (
            self.current_ctxs.len(),
            self.generated_textures,
            self.deleted_textures,
            self.skipped_texture_deletes,
        )
    }
    fn current_ctx_for_thread(&self, thread: crate::ThreadId) -> Option<crate::objc::id> {
        self.current_ctxs.get(&thread).copied().flatten()
    }
}

#[cfg(feature = "diagnostic-live-cycles")]
pub(crate) fn resource_profile(env: &crate::Environment) -> (usize, u64, u64, u64) {
    env.framework_state.opengles.resource_profile()
}

pub(crate) fn active_context_for_probe(
    env: &mut crate::Environment,
) -> Box<dyn crate::gles::GLES + '_> {
    sync_context(
        &mut env.framework_state.opengles,
        &mut env.objc,
        env.window.as_mut().unwrap(),
        env.current_thread,
    )
}

fn sync_context<'objc, 'win: 'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    window: &'win mut crate::window::Window,
    current_thread: crate::ThreadId,
) -> Box<dyn crate::gles::GLES + 'objc> {
    let gles_ctx = get_thread_context(state, objc, current_thread);
    gles_ctx.make_current(window)
}

fn get_thread_context<'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    current_thread: crate::ThreadId,
) -> &'objc mut dyn crate::gles::GLESContext {
    let current_ctx = state.current_ctx_for_thread(current_thread);
    let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(current_ctx.unwrap());
    let gles_ctx = host_obj.gles_ctx.as_deref_mut().unwrap();
    gles_ctx
}

#[cfg(test)]
mod context_lookup_tests {
    use super::*;

    #[test]
    fn no_context_queries_do_not_retain_finished_thread_entries() {
        let mut state = State::default();
        let context = crate::objc::id::from_bits(0x1234);
        state.current_ctxs.insert(7, Some(context));
        for tid in 0..20_000 {
            assert_eq!(
                state.current_ctx_for_thread(tid),
                (tid == 7).then_some(context)
            );
        }
        assert_eq!(state.current_ctxs.len(), 1);
        assert_eq!(state.current_ctxs.remove(&7), Some(Some(context)));
        assert_eq!(state.current_ctx_for_thread(7), None);
        assert!(state.current_ctxs.is_empty());
    }
}
