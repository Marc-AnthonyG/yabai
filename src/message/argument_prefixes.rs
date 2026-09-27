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

#[cfg(test)]
mod tests {
    use super::{c_string_in_buffer_equals, parse_resize_handle, parse_value_type};
    use crate::support::resize_handle::ResizeHandle;
    use crate::support::strings::MAXLEN;
    use crate::support::type_of_change::{TYPE_ABS, TYPE_REL};

    fn buffer_as_sscanf_leaves_it(bytes: &[u8]) -> [libc::c_char; MAXLEN] {
        let mut buffer = [0 as libc::c_char; MAXLEN];
        for (index, byte) in bytes.iter().enumerate() {
            buffer[index] = *byte as libc::c_char;
        }
        buffer
    }

    #[test]
    fn parse_value_type_accepts_abs_and_rel_with_the_c_values() {
        assert_eq!(
            parse_value_type(&buffer_as_sscanf_leaves_it(b"abs")),
            TYPE_ABS as u8
        );
        assert_eq!(
            parse_value_type(&buffer_as_sscanf_leaves_it(b"rel")),
            TYPE_REL as u8
        );
        assert_eq!(parse_value_type(&buffer_as_sscanf_leaves_it(b"abs")), 0x1);
        assert_eq!(parse_value_type(&buffer_as_sscanf_leaves_it(b"rel")), 0x2);
    }

    #[test]
    fn parse_value_type_rejects_every_other_spelling_with_zero() {
        for rejected in [
            &b""[..],
            b"ABS",
            b"Rel",
            b"absolute",
            b"ab",
            b"rel ",
            b" abs",
        ] {
            assert_eq!(
                parse_value_type(&buffer_as_sscanf_leaves_it(rejected)),
                0,
                "value type {:?}",
                String::from_utf8_lossy(rejected)
            );
        }
    }

    #[test]
    fn parse_resize_handle_accepts_each_edge_corner_and_abs() {
        let expected_handles = [
            ("top", ResizeHandle::TOP.0),
            ("bottom", ResizeHandle::BOTTOM.0),
            ("left", ResizeHandle::LEFT.0),
            ("right", ResizeHandle::RIGHT.0),
            ("top_left", ResizeHandle::TOP.0 | ResizeHandle::LEFT.0),
            ("top_right", ResizeHandle::TOP.0 | ResizeHandle::RIGHT.0),
            ("bottom_left", ResizeHandle::BOTTOM.0 | ResizeHandle::LEFT.0),
            (
                "bottom_right",
                ResizeHandle::BOTTOM.0 | ResizeHandle::RIGHT.0,
            ),
            ("abs", ResizeHandle::ABS.0),
        ];

        for (handle, expected_bits) in expected_handles {
            assert_eq!(
                parse_resize_handle(&buffer_as_sscanf_leaves_it(handle.as_bytes())),
                expected_bits,
                "resize handle {handle:?}"
            );
        }
    }

    #[test]
    fn parse_resize_handle_gives_the_corners_the_c_bit_values() {
        assert_eq!(
            parse_resize_handle(&buffer_as_sscanf_leaves_it(b"top_left")),
            0x05
        );
        assert_eq!(
            parse_resize_handle(&buffer_as_sscanf_leaves_it(b"top_right")),
            0x09
        );
        assert_eq!(
            parse_resize_handle(&buffer_as_sscanf_leaves_it(b"bottom_left")),
            0x06
        );
        assert_eq!(
            parse_resize_handle(&buffer_as_sscanf_leaves_it(b"bottom_right")),
            0x0a
        );
        assert_eq!(
            parse_resize_handle(&buffer_as_sscanf_leaves_it(b"abs")),
            0x10
        );
    }

    #[test]
    fn parse_resize_handle_rejects_every_other_spelling_with_zero() {
        for rejected in [
            &b""[..],
            b"TOP",
            b"topleft",
            b"top-left",
            b"left_top",
            b"center",
            b"rel",
            b"top ",
        ] {
            assert_eq!(
                parse_resize_handle(&buffer_as_sscanf_leaves_it(rejected)),
                0,
                "resize handle {:?}",
                String::from_utf8_lossy(rejected)
            );
        }
    }

    #[test]
    fn c_string_in_buffer_equals_ignores_everything_after_the_first_terminator() {
        let buffer = buffer_as_sscanf_leaves_it(b"abs\0olete");

        assert!(c_string_in_buffer_equals(&buffer, "abs"));
        assert!(!c_string_in_buffer_equals(&buffer, "absolete"));
    }

    #[test]
    fn c_string_in_buffer_equals_compares_a_buffer_without_terminator_in_full() {
        let buffer = [b'a' as libc::c_char; MAXLEN];

        assert!(c_string_in_buffer_equals(&buffer, &"a".repeat(MAXLEN)));
        assert!(!c_string_in_buffer_equals(&buffer, &"a".repeat(MAXLEN - 1)));
    }
}
