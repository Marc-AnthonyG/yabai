use crate::support::resize_handle::ResizeHandle;
use crate::support::strings::FIXED_STRING_BUFFER_LENGTH;
use crate::support::type_of_change::{CHANGE_TYPE_ABSOLUTE, CHANGE_TYPE_RELATIVE};

pub(crate) fn is_null_terminated_buffer_equal_to(
    buffer: &[libc::c_char; FIXED_STRING_BUFFER_LENGTH],
    candidate: &str,
) -> bool {
    let end = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    buffer[..end]
        .iter()
        .map(|character| *character as u8)
        .eq(candidate.bytes())
}

pub(crate) fn parse_absolute_or_relative_change_type(
    type_of_change: &[libc::c_char; FIXED_STRING_BUFFER_LENGTH],
) -> u8 {
    if is_null_terminated_buffer_equal_to(type_of_change, "abs") {
        CHANGE_TYPE_ABSOLUTE as u8
    } else if is_null_terminated_buffer_equal_to(type_of_change, "rel") {
        CHANGE_TYPE_RELATIVE as u8
    } else {
        0
    }
}

pub(crate) fn parse_resize_handle(handle: &[libc::c_char; FIXED_STRING_BUFFER_LENGTH]) -> u8 {
    if is_null_terminated_buffer_equal_to(handle, "top") {
        ResizeHandle::TOP.0
    } else if is_null_terminated_buffer_equal_to(handle, "bottom") {
        ResizeHandle::BOTTOM.0
    } else if is_null_terminated_buffer_equal_to(handle, "left") {
        ResizeHandle::LEFT.0
    } else if is_null_terminated_buffer_equal_to(handle, "right") {
        ResizeHandle::RIGHT.0
    } else if is_null_terminated_buffer_equal_to(handle, "top_left") {
        ResizeHandle::TOP.0 | ResizeHandle::LEFT.0
    } else if is_null_terminated_buffer_equal_to(handle, "top_right") {
        ResizeHandle::TOP.0 | ResizeHandle::RIGHT.0
    } else if is_null_terminated_buffer_equal_to(handle, "bottom_left") {
        ResizeHandle::BOTTOM.0 | ResizeHandle::LEFT.0
    } else if is_null_terminated_buffer_equal_to(handle, "bottom_right") {
        ResizeHandle::BOTTOM.0 | ResizeHandle::RIGHT.0
    } else if is_null_terminated_buffer_equal_to(handle, "abs") {
        ResizeHandle::ABSOLUTE.0
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_null_terminated_buffer_equal_to, parse_absolute_or_relative_change_type,
        parse_resize_handle,
    };
    use crate::support::resize_handle::ResizeHandle;
    use crate::support::strings::FIXED_STRING_BUFFER_LENGTH;
    use crate::support::type_of_change::{CHANGE_TYPE_ABSOLUTE, CHANGE_TYPE_RELATIVE};

    fn buffer_as_sscanf_leaves_it(bytes: &[u8]) -> [libc::c_char; FIXED_STRING_BUFFER_LENGTH] {
        let mut buffer = [0 as libc::c_char; FIXED_STRING_BUFFER_LENGTH];
        for (index, byte) in bytes.iter().enumerate() {
            buffer[index] = *byte as libc::c_char;
        }
        buffer
    }

    #[test]
    fn parse_absolute_or_relative_change_type_accepts_abs_and_rel_with_the_c_values() {
        assert_eq!(
            parse_absolute_or_relative_change_type(&buffer_as_sscanf_leaves_it(b"abs")),
            CHANGE_TYPE_ABSOLUTE as u8
        );
        assert_eq!(
            parse_absolute_or_relative_change_type(&buffer_as_sscanf_leaves_it(b"rel")),
            CHANGE_TYPE_RELATIVE as u8
        );
        assert_eq!(
            parse_absolute_or_relative_change_type(&buffer_as_sscanf_leaves_it(b"abs")),
            0x1
        );
        assert_eq!(
            parse_absolute_or_relative_change_type(&buffer_as_sscanf_leaves_it(b"rel")),
            0x2
        );
    }

    #[test]
    fn parse_absolute_or_relative_change_type_rejects_every_other_spelling_with_zero() {
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
                parse_absolute_or_relative_change_type(&buffer_as_sscanf_leaves_it(rejected)),
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
            ("abs", ResizeHandle::ABSOLUTE.0),
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
    fn is_null_terminated_buffer_equal_to_ignores_everything_after_the_first_terminator() {
        let buffer = buffer_as_sscanf_leaves_it(b"abs\0olete");

        assert!(is_null_terminated_buffer_equal_to(&buffer, "abs"));
        assert!(!is_null_terminated_buffer_equal_to(&buffer, "absolete"));
    }

    #[test]
    fn is_null_terminated_buffer_equal_to_compares_a_buffer_without_terminator_in_full() {
        let buffer = [b'a' as libc::c_char; FIXED_STRING_BUFFER_LENGTH];

        assert!(is_null_terminated_buffer_equal_to(
            &buffer,
            &"a".repeat(FIXED_STRING_BUFFER_LENGTH)
        ));
        assert!(!is_null_terminated_buffer_equal_to(
            &buffer,
            &"a".repeat(FIXED_STRING_BUFFER_LENGTH - 1)
        ));
    }
}
