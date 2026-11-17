/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UITextView`.

use crate::frameworks::core_graphics::cg_context::CGContextSetRGBFillColor;
use crate::frameworks::core_graphics::cg_geometry::CGPointZero;
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::frameworks::foundation::{ns_string, NSInteger, NSRange, NSUInteger};
use crate::frameworks::uikit::ui_color;
use crate::frameworks::uikit::ui_font::{
    break_lines_with_font, UILineBreakModeTailTruncation, UILineBreakModeWordWrap, UITextAlignment,
    UITextAlignmentLeft,
};
use crate::frameworks::uikit::ui_graphics::UIGraphicsGetCurrentContext;
use crate::frameworks::uikit::ui_view::ui_control::ui_text_field::UIReturnKeyType;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes, release,
    retain, todo_objc_setter, ClassExports, NSZonePtr,
};
use crate::window::TextInputEvent;
use crate::Environment;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub const PROBE_CLASSES: ClassExports = tests::CLASSES;
#[cfg(not(test))]
pub const PROBE_CLASSES: ClassExports = &[];

type UIDataDetectorTypes = NSUInteger;

pub struct UITextViewHostObject {
    superclass: super::UIScrollViewHostObject,
    editable: bool,
    delegate: id, // weak UITextViewDelegate
    editing: bool,
    ending: bool,
    autocorrection: NSInteger,
    autocapitalization: NSInteger,
    /// `NSString*`
    text: id,
    /// `UIFont*`
    font: id,
    /// `UIColor*`
    text_color: id,
    text_alignment: UITextAlignment,
}
impl_HostObject_with_superclass!(UITextViewHostObject);
impl Default for UITextViewHostObject {
    fn default() -> Self {
        UITextViewHostObject {
            superclass: Default::default(),
            editable: true,
            delegate: nil,
            editing: false,
            ending: false,
            autocorrection: 0,
            autocapitalization: 1,
            font: nil,
            text: nil,
            text_color: nil,
            text_alignment: UITextAlignmentLeft,
        }
    }
}

fn responds(env: &mut Environment, delegate: id, name: &str) -> bool {
    let sel = env
        .objc
        .register_host_selector(name.to_owned(), &mut env.mem);
    delegate != nil && msg![env; delegate respondsToSelector:sel]
}

fn keyboard(env: &mut Environment, this: id, showing: bool) {
    use crate::frameworks::uikit::ui_view::ui_window::*;
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    let will = ns_string::get_static_str(
        env,
        if showing {
            UIKeyboardWillShowNotification
        } else {
            UIKeyboardWillHideNotification
        },
    );
    () = msg![env; center postNotificationName:will object:this userInfo:nil];
    if env.window.is_some() {
        env.on_parent_stack_in_coroutine(|window, _| {
            if showing {
                window.start_text_input();
            } else {
                window.stop_text_input();
            }
        });
    }
    let did = ns_string::get_static_str(
        env,
        if showing {
            UIKeyboardDidShowNotification
        } else {
            UIKeyboardDidHideNotification
        },
    );
    () = msg![env; center postNotificationName:did object:this userInfo:nil];
}

/// SDL desktop keys and Android soft-keyboard input use the same delegate path.
/// Editing currently appends at the end; selection/marked-text input is TODO.
pub fn handle_input(env: &mut Environment, this: id, event: TextInputEvent) {
    if !env.objc.borrow::<UITextViewHostObject>(this).editing {
        return;
    }
    retain(env, this);
    let current: id = msg![env; this text];
    retain(env, current);
    let mut text = to_rust_string(env, current).into_owned();
    let length = text.encode_utf16().count() as u32;
    let (range, replacement) = match event {
        TextInputEvent::Text(value) => (
            NSRange {
                location: length,
                length: 0,
            },
            value,
        ),
        TextInputEvent::Return => (
            NSRange {
                location: length,
                length: 0,
            },
            "\n".to_owned(),
        ),
        TextInputEvent::Backspace => {
            let Some(last) = text.pop() else {
                release(env, current);
                release(env, this);
                return;
            };
            let removed = last.len_utf16() as u32;
            (
                NSRange {
                    location: length - removed,
                    length: removed,
                },
                String::new(),
            )
        }
    };
    let replacement = ns_string::from_rust_string(env, replacement);
    let delegate = env.objc.borrow::<UITextViewHostObject>(this).delegate;
    retain(env, delegate);
    let allowed = !responds(
        env,
        delegate,
        "textView:shouldChangeTextInRange:replacementText:",
    ) || msg![env; delegate textView:this shouldChangeTextInRange:range replacementText:replacement];
    // A delegate can resign, remove and release the view while handling Return.
    if allowed && env.objc.borrow::<UITextViewHostObject>(this).editing {
        text.push_str(&to_rust_string(env, replacement));
        let new_text = ns_string::from_rust_string(env, text);
        () = msg![env; this setText:new_text];
        release(env, new_text);
        if responds(env, delegate, "textViewDidChange:") {
            () = msg![env; delegate textViewDidChange:this];
        }
    }
    release(env, delegate);
    release(env, replacement);
    release(env, current);
    release(env, this);
}

// Update contentOffset and contentSize when anything that potentially affects
// contentSize like font and text change.
fn update_scroll(env: &mut Environment, this: id) {
    let bounds: CGRect = msg![env; this bounds];
    let bound_size = bounds.size;
    let font: id = msg![env; this font];
    let text: id = msg![env; this text];

    // Calculate our new contentSize
    let calculated_size: CGSize = msg![env; text sizeWithFont:font constrainedToSize:bound_size];
    () = msg![env; this setContentSize:calculated_size];

    // Reset contentOffset if we have now gone out of bounds of contentSize,
    // otherwise ignore.
    let current_content_offset: CGPoint = msg![env; this contentOffset];
    if current_content_offset.x > calculated_size.width - bounds.size.width
        || current_content_offset.y > calculated_size.height - bounds.size.height
    {
        () = msg![env; this setContentOffset:(CGPoint { x: 0.0, y: 0.0 })];
    }
}
pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITextView: UIScrollView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITextViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    let this: id = msg_super![env; this initWithFrame:frame];
    // TODO: refactor to a common init with `initWithCoder:`
    // These aren't redundant, the setters fetch the real defaults.
    () = msg![env; this setFont:nil];
    () = msg![env; this setTextColor:nil];
    let empty = ns_string::get_static_str(env, "");
    () = msg![env; this setText:empty];
    this
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    // These aren't redundant, the setters fetch the real defaults.
    () = msg![env; this setFont:nil];
    () = msg![env; this setTextColor:nil];
    let empty = ns_string::get_static_str(env, "");
    () = msg![env; this setText:empty];
    this
}

- (())dealloc {
    let UITextViewHostObject {
        font,
        text,
        text_color,
        ..
    } = std::mem::take(env.objc.borrow_mut(this));

    release(env, font);
    release(env, text_color);
    release(env, text);
    msg_super![env; this dealloc]
}

- (id)text {
    env.objc.borrow::<UITextViewHostObject>(this).text
}
- (())setText:(id)new_text { // NSString*
    let new_text = if new_text == nil { ns_string::get_static_str(env, "") } else { new_text };
    let hostobj  = env.objc.borrow_mut::<UITextViewHostObject>(this);
    let old_text = std::mem::replace(&mut hostobj.text, new_text);
    retain(env, new_text);
    release(env, old_text);
    update_scroll(env,this);
    () = msg![env; this setNeedsDisplay];
}

- (id)textColor {
    env.objc.borrow::<UITextViewHostObject>(this).text_color
}
- (())setTextColor:(id)new_text_color { // UIColor*
    let new_text_color: id = if new_text_color == nil {
        msg_class![env; UIColor blackColor]
    } else {
        new_text_color
    };

    let hostobj  = env.objc.borrow_mut::<UITextViewHostObject>(this);
    let old_text_color = std::mem::replace(&mut hostobj.text_color, new_text_color);
    retain(env, new_text_color);
    release(env, old_text_color);
    () = msg![env; this setNeedsDisplay];
}

- (UITextAlignment)textAlignment {
    env.objc.borrow::<UITextViewHostObject>(this).text_alignment
}
- (())setTextAlignment:(UITextAlignment)new_text_alignment {
    env.objc.borrow_mut::<UITextViewHostObject>(this).text_alignment = new_text_alignment;
    () = msg![env; this setNeedsDisplay];
}

- (id)font {
    env.objc.borrow::<UITextViewHostObject>(this).font
}
- (())setFont:(id)new_font { // UIFont*
    let new_font: id = if new_font == nil {
        // reset to default
        let size: CGFloat = 17.0;
        msg_class![env; UIFont systemFontOfSize:size]
    } else {
        new_font
    };

    let hostobj  = env.objc.borrow_mut::<UITextViewHostObject>(this);
    let old_font = std::mem::replace(&mut hostobj.font, new_font);
    retain(env, new_font);
    release(env, old_font);
    update_scroll(env,this);
    () = msg![env; this setNeedsDisplay];
}

- (())flashScrollIndicators {
    // TODO
}

- (id)delegate { env.objc.borrow::<UITextViewHostObject>(this).delegate }
- (())setDelegate:(id)delegate { env.objc.borrow_mut::<UITextViewHostObject>(this).delegate = delegate; }
- (bool)isFirstResponder { env.framework_state.uikit.ui_responder.first_responder == this }
- (bool)canBecomeFirstResponder { env.objc.borrow::<UITextViewHostObject>(this).editable }
- (bool)becomeFirstResponder {
    if !env.objc.borrow::<UITextViewHostObject>(this).editable { return false; }
    if env.objc.borrow::<UITextViewHostObject>(this).editing { return true; }
    let delegate = env.objc.borrow::<UITextViewHostObject>(this).delegate;
    if responds(env, delegate, "textViewShouldBeginEditing:") && !msg![env; delegate textViewShouldBeginEditing:this] { return false; }
    let old = env.framework_state.uikit.ui_responder.first_responder;
    if old != nil && old != this && !msg![env; old resignFirstResponder] { return false; }
    retain(env, this);
    retain(env, delegate);
    env.objc.borrow_mut::<UITextViewHostObject>(this).editing = true;
    env.framework_state.uikit.ui_responder.first_responder = this;
    keyboard(env, this, true);
    if responds(env, delegate, "textViewDidBeginEditing:") { () = msg![env; delegate textViewDidBeginEditing:this]; }
    release(env, delegate);
    release(env, this);
    true
}
- (bool)resignFirstResponder {
    let state = env.objc.borrow_mut::<UITextViewHostObject>(this);
    if !state.editing || state.ending { return true; }
    state.ending = true;
    let delegate = state.delegate;
    retain(env, this);
    retain(env, delegate);
    if responds(env, delegate, "textViewShouldEndEditing:") && !msg![env; delegate textViewShouldEndEditing:this] {
        env.objc.borrow_mut::<UITextViewHostObject>(this).ending = false;
        release(env, delegate); release(env, this); return false;
    }
    env.objc.borrow_mut::<UITextViewHostObject>(this).editing = false;
    if env.framework_state.uikit.ui_responder.first_responder == this {
        env.framework_state.uikit.ui_responder.first_responder = nil;
        keyboard(env, this, false);
    }
    // Mark non-editing before callbacks: the game's didEnd calls resign again.
    if responds(env, delegate, "textViewDidEndEditing:") { () = msg![env; delegate textViewDidEndEditing:this]; }
    env.objc.borrow_mut::<UITextViewHostObject>(this).ending = false;
    release(env, delegate); release(env, this);
    true
}
- (())touchesBegan:(id)_touches withEvent:(id)_event {
    let _: bool = msg![env; this becomeFirstResponder];
    if env.objc.borrow::<UITextViewHostObject>(this).editing && env.window.is_some() {
        env.on_parent_stack_in_coroutine(|window,_| window.start_text_input());
    }
}
- (NSInteger)autocorrectionType { env.objc.borrow::<UITextViewHostObject>(this).autocorrection }
- (())setAutocorrectionType:(NSInteger)value { env.objc.borrow_mut::<UITextViewHostObject>(this).autocorrection = value; }
- (NSInteger)autocapitalizationType { env.objc.borrow::<UITextViewHostObject>(this).autocapitalization }
- (())setAutocapitalizationType:(NSInteger)value { env.objc.borrow_mut::<UITextViewHostObject>(this).autocapitalization = value; }
- (bool)isEditable {
    env.objc.borrow::<UITextViewHostObject>(this).editable
}
- (())setEditable:(bool)editable {
    if !editable { let _: bool = msg![env; this resignFirstResponder]; }
    env.objc.borrow_mut::<UITextViewHostObject>(this).editable = editable;
}

- (())scrollRangeToVisible:(NSRange)range {
    let &mut UITextViewHostObject {
        font,
        text,
        ..
    } = env.objc.borrow_mut(this);

    if range.location > msg![env; text length] {
        return;
    }

    let bounds: CGRect = msg![env; this bounds];
    let bound_size = bounds.size;

    let text = to_rust_string(env, text);

    let lines = break_lines_with_font(env, font, &text, Some((bound_size, UILineBreakModeWordWrap)));

    let mut line_count = 0;
    let mut current_position = 0;

    for (_, line) in lines {
        current_position += line.len();

        if let Some(offset) = text[current_position..].find(|c: char| !c.is_whitespace()) {
            current_position += offset;
        } else {
            current_position = text.len();
        }

        if range.location <= current_position as u32 {
            break;
        }

        line_count += 1;
    }

    let line_height: CGFloat = msg![env; font lineHeight];
    let leading: CGFloat = msg![env; font leading];

    let height_to_range_start = (line_count + 1) as f32 * line_height - leading;

    let content_offset: CGPoint = msg![env; this contentOffset];

    if height_to_range_start - line_height < content_offset.y {
        let new_scroll_y = CGPoint {x: 0.0, y: line_count as f32 * line_height};
        () = msg![env; this setContentOffset:new_scroll_y];
    }
    else if height_to_range_start > content_offset.y + bound_size.height {
        let new_scroll_y = CGPoint {x: 0.0, y: height_to_range_start- bound_size.height};
        () = msg![env; this setContentOffset:new_scroll_y];
    }

    update_scroll(env, this);
}

- (())setReturnKeyType:(UIReturnKeyType)type_ {
    todo_objc_setter!(this, type_);
}

- (())setDataDetectorTypes:(UIDataDetectorTypes)types {
    todo_objc_setter!(this, types);
}

- (())drawRect:(CGRect)_rect {
    let bounds: CGRect = msg![env; this bounds];
    let context = UIGraphicsGetCurrentContext(env);

    let &mut UITextViewHostObject {
        font,
        text,
        text_color,
        text_alignment,
        ..
    } = env.objc.borrow_mut(this);

    let (r, g, b, a) = ui_color::get_rgba(&env.objc, text_color);
    CGContextSetRGBFillColor(env, context, r, g, b, a);

    let content_offset: CGPoint = msg![env; this contentOffset];
    let rect = CGRect {
        origin: CGPointZero,
        // If size is not expanded by the offset,
        // the text is rendered truncated.
        size: CGSize {
            width: bounds.size.width + content_offset.x,
            height: bounds.size.height + content_offset.y,
        }
    };

    log_dbg!("UItextView text rendering in rect {:?}", rect);
    let _size: CGSize = msg![env; text drawInRect:rect
                                         withFont:font
                                    lineBreakMode:UILineBreakModeTailTruncation
                                        alignment:text_alignment];
}

@end

};
