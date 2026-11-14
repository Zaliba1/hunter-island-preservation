/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Offline-only matchmaker lifetime and cancellation of empty requests.
//! No authentication, matchmaking, invitations or successful matches are supplied.
use crate::libc::blocks::{_Block_copy, _Block_release};
use crate::mem::MutVoidPtr;
use crate::objc::{id, objc_classes, ClassExports, HostObject, NSZonePtr};

#[derive(Default)]
struct Matchmaker {
    invite_handler: MutVoidPtr,
}
impl HostObject for Matchmaker {}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation GKMatchmaker: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::<Matchmaker>::default(), &mut env.mem)
}
+ (id)sharedMatchmaker {
    if let Some(matchmaker) = env.framework_state.game_kit.matchmaker {
        matchmaker
    } else {
        // The framework keeps the initial ownership reference for this session.
        let matchmaker = env.objc.alloc_object(this, Box::<Matchmaker>::default(), &mut env.mem);
        env.framework_state.game_kit.matchmaker = Some(matchmaker);
        log!("Created offline-only matchmaker; Game Center authentication and matches unavailable");
        matchmaker
    }
}
- (MutVoidPtr)inviteHandler {
    env.objc.borrow::<Matchmaker>(this).invite_handler
}
- (())setInviteHandler:(MutVoidPtr)handler {
    let copied = _Block_copy(env, handler);
    let old = std::mem::replace(&mut env.objc.borrow_mut::<Matchmaker>(this).invite_handler, copied);
    _Block_release(env, old);
    log!("Offline matchmaker: invitation handler copied; no online invitations are delivered");
}
- (())dealloc {
    let old = std::mem::take(&mut env.objc.borrow_mut::<Matchmaker>(this).invite_handler);
    _Block_release(env, old);
    env.objc.dealloc_object(this, &mut env.mem);
}
- (())cancel {
    // There can be no pending request: request-creation APIs remain unsupported.
    log!("Offline matchmaker: no pending matchmaking request to cancel");
}
- (())cancelInviteToPlayer:(id)_player {
    // Likewise, this implementation has never issued an invitation.
    log!("Offline matchmaker: no pending invitation to cancel");
}
@end
};

#[cfg(test)]
mod tests {
    use crate::objc::{id, msg, msg_class, nil, release, retain};
    #[test]
    fn offline_singleton_and_empty_cancellation() {
        let mut environment = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut environment;
        let first: id = msg_class![env; GKMatchmaker sharedMatchmaker];
        assert_ne!(first, nil);
        retain(env, first);
        release(env, first);
        let again: id = msg_class![env; GKMatchmaker sharedMatchmaker];
        assert_eq!(first, again);
        () = msg![env; first cancel];
        () = msg![env; first cancelInviteToPlayer:nil];
        let player: id = msg_class![env; GKLocalPlayer localPlayer];
        assert_eq!(
            player, nil,
            "Offline support must not fabricate a signed-in player"
        );
        let descriptor = env.mem.alloc(8).cast::<u32>();
        env.mem.write(descriptor, 0);
        env.mem.write(descriptor + 1, 24);
        let literal = env.mem.alloc(24);
        let isa = env
            .objc
            .get_known_class("_NSConcreteStackBlock", &mut env.mem);
        env.mem.write(
            literal.cast(),
            crate::libc::blocks::Header {
                isa,
                flags: 0,
                reserved: 0,
                invoke: crate::abi::GuestFunction::from_addr_with_thumb_bit(1),
                descriptor,
            },
        );
        env.mem.write(literal.cast::<u32>() + 5, 0x12345678);
        () = msg![env; first setInviteHandler:literal];
        let copied: crate::mem::MutVoidPtr = msg![env; first inviteHandler];
        assert_ne!(copied, literal);
        env.mem.free(literal);
        assert_eq!(env.mem.read(copied.cast::<u32>() + 5), 0x12345678);
        () = msg![env; first setInviteHandler:copied];
        let header: crate::libc::blocks::Header = env.mem.read(copied.cast());
        assert_eq!(header.flags & 0xffff, 1);
        super::_Block_copy(env, copied);
        () = msg![env; first setInviteHandler:(crate::mem::MutVoidPtr::null())];
        let current: crate::mem::MutVoidPtr = msg![env; first inviteHandler];
        assert!(current.is_null());
        let other: id = msg_class![env; GKMatchmaker new];
        () = msg![env; other setInviteHandler:copied];
        release(env, other);
        let header: crate::libc::blocks::Header = env.mem.read(copied.cast());
        assert_eq!(
            header.flags & 0xffff,
            1,
            "Handler cleared/released on replacement and destruction"
        );
        super::_Block_release(env, copied);
        env.mem.free(descriptor.cast());
    }
}
