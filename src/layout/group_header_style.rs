use crate::support::color::{RgbaColor, rgba_color_from_packed_argb};

pub(crate) struct GroupHeaderStyle {
    pub(crate) height: f32,
    pub(crate) background_color: RgbaColor,
    pub(crate) active_color: RgbaColor,
    pub(crate) inactive_color: RgbaColor,
    pub(crate) active_text_color: RgbaColor,
    pub(crate) inactive_text_color: RgbaColor,
    pub(crate) font_family: String,
    pub(crate) font_style: String,
    pub(crate) font_size: f32,
}

pub(crate) fn group_header_style_with_its_initial_settings() -> GroupHeaderStyle {
    GroupHeaderStyle {
        height: 24.0,
        background_color: rgba_color_from_packed_argb(0x00000000),
        active_color: rgba_color_from_packed_argb(0xff3d59a1),
        inactive_color: rgba_color_from_packed_argb(0xff292e42),
        active_text_color: rgba_color_from_packed_argb(0xffc0caf5),
        inactive_text_color: rgba_color_from_packed_argb(0xff737aa2),
        font_family: String::from("Helvetica Neue"),
        font_style: String::new(),
        font_size: 12.0,
    }
}
