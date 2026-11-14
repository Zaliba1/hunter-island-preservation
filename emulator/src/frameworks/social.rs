/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Offline Social.framework availability. No accounts or posting are provided.
//! Apple's isAvailableForServiceType: requires an accessible configured account.
use crate::dyld::{ConstantExports, HostConstant, HostDylib};
use crate::objc::{id, objc_classes, ClassExports};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/Social.framework/Social",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};

pub const CONSTANTS: ConstantExports = &[
    (
        "_SLServiceTypeFacebook",
        HostConstant::NSString("com.apple.social.facebook"),
    ),
    (
        "_SLServiceTypeTwitter",
        HostConstant::NSString("com.apple.social.twitter"),
    ),
];

const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);
    @implementation SLComposeViewController: UIViewController
    + (bool)isAvailableForServiceType:(id)_service_type {
        false
    }
    @end
};

#[cfg(test)]
mod tests {
    use crate::frameworks::foundation::ns_string;
    use crate::objc::{id, msg_class, nil};
    #[test]
    fn social_services_are_unavailable_without_configured_accounts() {
        let mut env = crate::Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut env;
        for service in [
            nil,
            ns_string::get_static_str(env, "com.apple.social.facebook"),
            ns_string::get_static_str(env, "com.apple.social.twitter"),
            ns_string::get_static_str(env, "unknown"),
        ] {
            let available: bool =
                msg_class![env; SLComposeViewController isAvailableForServiceType:service];
            assert!(!available);
        }
    }
}
