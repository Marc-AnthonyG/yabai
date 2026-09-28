use objc2_core_foundation::{CFAttributedString, CFDictionary, CFRetained, CFString, CFType};
pub use objc2_core_text::{CTFont, CTLine, CTLineTruncationType};
use objc2_core_text::{
    CTFontDescriptor, kCTFontAttributeName, kCTFontFamilyNameAttribute, kCTFontStyleNameAttribute,
    kCTForegroundColorFromContextAttributeName,
};

use crate::ffi::core_foundation::CFBoolean;

pub struct TypographicBoundsOfLine {
    pub ascent: f64,
    pub descent: f64,
}

pub fn create_font_of_family_and_style(
    family_name: &str,
    style_name: &str,
    size: f64,
) -> CFRetained<CTFont> {
    let family_name = CFString::from_str(family_name);
    let style_name = CFString::from_str(style_name);
    let mut keys: Vec<&CFString> = vec![unsafe { kCTFontFamilyNameAttribute }];
    let mut values: Vec<&CFString> = vec![&family_name];
    if style_name.length() > 0 {
        keys.push(unsafe { kCTFontStyleNameAttribute });
        values.push(&style_name);
    }
    let attributes = CFDictionary::from_slices(&keys, &values);
    let descriptor = unsafe { CTFontDescriptor::with_attributes(attributes.as_opaque()) };
    unsafe { CTFont::with_font_descriptor(&descriptor, size, std::ptr::null()) }
}

pub fn create_line_of_text_coloured_by_the_context_fill(
    text: &str,
    font: &CTFont,
) -> Option<CFRetained<CTLine>> {
    let text = CFString::from_str(text);
    let keys: [&CFString; 2] = unsafe {
        [
            kCTFontAttributeName,
            kCTForegroundColorFromContextAttributeName,
        ]
    };
    let values: [&CFType; 2] = [font, CFBoolean::new(true)];
    let attributes = CFDictionary::from_slices(&keys, &values);
    let attributed_text =
        unsafe { CFAttributedString::new(None, Some(&text), Some(attributes.as_opaque())) }?;
    Some(unsafe { CTLine::with_attributed_string(&attributed_text) })
}

pub fn typographic_bounds_of_line(line: &CTLine) -> TypographicBoundsOfLine {
    let mut ascent = 0.0;
    let mut descent = 0.0;
    unsafe { line.typographic_bounds(&mut ascent, &mut descent, std::ptr::null_mut()) };
    TypographicBoundsOfLine { ascent, descent }
}
