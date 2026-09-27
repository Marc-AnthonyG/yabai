use crate::support::resize_handle::ResizeHandle;
use crate::support::strings::MAXLEN;
use crate::support::type_of_change::{TYPE_ABS, TYPE_REL};

pub(crate) fn c_string_in_buffer_equals(buffer: &[libc::c_char; MAXLEN], candidate: &str) -> bool {
    let end = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    buffer[..end]
        .iter()
        .map(|character| *character as u8)
        .eq(candidate.bytes())
}

pub(crate) fn parse_value_type(type_of_change: &[libc::c_char; MAXLEN]) -> u8 {
    if c_string_in_buffer_equals(type_of_change, "abs") {
        TYPE_ABS as u8
    } else if c_string_in_buffer_equals(type_of_change, "rel") {
        TYPE_REL as u8
    } else {
        0
    }
}

pub(crate) fn parse_resize_handle(handle: &[libc::c_char; MAXLEN]) -> u8 {
    if c_string_in_buffer_equals(handle, "top") {
        ResizeHandle::TOP.0
    } else if c_string_in_buffer_equals(handle, "bottom") {
        ResizeHandle::BOTTOM.0
    } else if c_string_in_buffer_equals(handle, "left") {
        ResizeHandle::LEFT.0
    } else if c_string_in_buffer_equals(handle, "right") {
        ResizeHandle::RIGHT.0
    } else if c_string_in_buffer_equals(handle, "top_left") {
        ResizeHandle::TOP.0 | ResizeHandle::LEFT.0
    } else if c_string_in_buffer_equals(handle, "top_right") {
        ResizeHandle::TOP.0 | ResizeHandle::RIGHT.0
    } else if c_string_in_buffer_equals(handle, "bottom_left") {
        ResizeHandle::BOTTOM.0 | ResizeHandle::LEFT.0
    } else if c_string_in_buffer_equals(handle, "bottom_right") {
        ResizeHandle::BOTTOM.0 | ResizeHandle::RIGHT.0
    } else if c_string_in_buffer_equals(handle, "abs") {
        ResizeHandle::ABS.0
    } else {
        0
    }
}
