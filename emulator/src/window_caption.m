// SPDX-License-Identifier: MPL-2.0
#import <AppKit/AppKit.h>
#include <stdint.h>
uint16_t hunter_caption_height(void) {
    NSRect content = NSMakeRect(0, 0, 200, 200);
    NSRect frame = [NSWindow frameRectForContentRect:content styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable];
    return (uint16_t)ceil(NSHeight(frame) - NSHeight(content));
}

#include <stdbool.h>
bool hunter_probe_window_fits(void) {
    for (NSWindow *window in NSApp.windows) {
        if ([window.title isEqualToString:@"Hunter Island work-area test"] && window.isVisible) {
            return (window.styleMask & NSWindowStyleMaskTitled) &&
                   [window standardWindowButton:NSWindowCloseButton].enabled &&
                   NSContainsRect(window.screen.visibleFrame, window.frame);
        }
    }
    return false;
}
