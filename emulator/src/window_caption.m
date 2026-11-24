// SPDX-License-Identifier: MPL-2.0
#import <AppKit/AppKit.h>
#include <stdint.h>
uint16_t hunter_caption_height(void) {
    NSRect content = NSMakeRect(0, 0, 200, 200);
    NSRect frame = [NSWindow frameRectForContentRect:content styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable];
    return (uint16_t)ceil(NSHeight(frame) - NSHeight(content));
}
