#![allow(deprecated)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::ffi::c_void;

use objc2_core_foundation::{
    CFDictionary, CFDictionaryCreate, CFIndex, CFRetained, CFString, CFType, CGAffineTransform,
    CGFloat, kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
};
use objc2_core_graphics::CGContext;

use crate::ffi::core_foundation::{kCFBooleanTrue, take_create_rule_result};

pub const kCTLineTruncationEnd: u32 = 1;

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    pub static kCTFontAttributeName: &'static CFString;
    pub static kCTFontFamilyNameAttribute: &'static CFString;
    pub static kCTFontStyleNameAttribute: &'static CFString;
    pub static kCTForegroundColorFromContextAttributeName: &'static CFString;

    pub fn CTFontDescriptorCreateWithAttributes(attributes: &CFDictionary) -> *const CFType;
    pub fn CTFontCreateWithFontDescriptor(
        descriptor: &CFType,
        size: CGFloat,
        matrix: *const CGAffineTransform,
    ) -> *const CFType;
    pub fn CTLineCreateWithAttributedString(attributed_string: &CFType) -> *const CFType;
    pub fn CTLineCreateTruncatedLine(
        line: &CFType,
        width: f64,
        truncation_type: u32,
        truncation_token: *const CFType,
    ) -> *const CFType;
    pub fn CTLineGetTypographicBounds(
        line: &CFType,
        ascent: *mut CGFloat,
        descent: *mut CGFloat,
        leading: *mut CGFloat,
    ) -> f64;
    pub fn CTLineDraw(line: &CFType, context: &CGContext);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFAttributedStringCreate(
        allocator: *const c_void,
        string: &CFString,
        attributes: &CFDictionary,
    ) -> *const CFType;
}

pub struct TypographicBoundsOfLine {
    pub ascent: f64,
    pub descent: f64,
}

pub fn create_font_of_family_and_style(
    family_name: &str,
    style_name: &str,
    size: f64,
) -> Option<CFRetained<CFType>> {
    let family_name = CFString::from_str(family_name);
    let style_name = CFString::from_str(style_name);
    let mut keys =
        vec![(unsafe { kCTFontFamilyNameAttribute } as *const CFString).cast::<c_void>()];
    let mut values = vec![(&*family_name as *const CFString).cast::<c_void>()];
    if style_name.length() > 0 {
        keys.push((unsafe { kCTFontStyleNameAttribute } as *const CFString).cast::<c_void>());
        values.push((&*style_name as *const CFString).cast::<c_void>());
    }
    let attributes = create_cfdictionary(&keys, &values)?;
    let descriptor =
        unsafe { take_create_rule_result(CTFontDescriptorCreateWithAttributes(&attributes)) }?;
    unsafe {
        take_create_rule_result(CTFontCreateWithFontDescriptor(
            &descriptor,
            size,
            std::ptr::null(),
        ))
    }
}

pub fn create_line_of_text_coloured_by_the_context_fill(
    text: &str,
    font: &CFType,
) -> Option<CFRetained<CFType>> {
    let text = CFString::from_str(text);
    let attributes = create_cfdictionary(
        &[
            (unsafe { kCTFontAttributeName } as *const CFString).cast::<c_void>(),
            (unsafe { kCTForegroundColorFromContextAttributeName } as *const CFString)
                .cast::<c_void>(),
        ],
        &[
            (font as *const CFType).cast::<c_void>(),
            (kCFBooleanTrue() as *const _ as *const CFType).cast::<c_void>(),
        ],
    )?;
    let attributed_text = unsafe {
        take_create_rule_result(CFAttributedStringCreate(
            std::ptr::null(),
            &text,
            &attributes,
        ))
    }?;
    unsafe { take_create_rule_result(CTLineCreateWithAttributedString(&attributed_text)) }
}

pub fn truncate_line_to_width_ending_with_the_token(
    line: &CFType,
    width: f64,
    truncation_token: &CFType,
) -> Option<CFRetained<CFType>> {
    unsafe {
        take_create_rule_result(CTLineCreateTruncatedLine(
            line,
            width,
            kCTLineTruncationEnd,
            truncation_token,
        ))
    }
}

pub fn typographic_bounds_of_line(line: &CFType) -> TypographicBoundsOfLine {
    let mut ascent: CGFloat = 0.0;
    let mut descent: CGFloat = 0.0;
    unsafe { CTLineGetTypographicBounds(line, &mut ascent, &mut descent, std::ptr::null_mut()) };
    TypographicBoundsOfLine { ascent, descent }
}

pub fn draw_line_in_context(line: &CFType, context: &CGContext) {
    unsafe { CTLineDraw(line, context) };
}

fn create_cfdictionary(
    keys: &[*const c_void],
    values: &[*const c_void],
) -> Option<CFRetained<CFDictionary>> {
    let mut keys = keys.to_vec();
    let mut values = values.to_vec();
    unsafe {
        CFDictionaryCreate(
            None,
            keys.as_mut_ptr(),
            values.as_mut_ptr(),
            keys.len() as CFIndex,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        )
    }
}
