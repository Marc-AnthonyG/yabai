use crate::message::common_arguments::{
    ARGUMENT_COMMON_SELECTOR_EAST, ARGUMENT_COMMON_SELECTOR_FIRST, ARGUMENT_COMMON_SELECTOR_LAST,
    ARGUMENT_COMMON_SELECTOR_MOUSE, ARGUMENT_COMMON_SELECTOR_NEXT, ARGUMENT_COMMON_SELECTOR_NORTH,
    ARGUMENT_COMMON_SELECTOR_PREVIOUS, ARGUMENT_COMMON_SELECTOR_RECENT,
    ARGUMENT_COMMON_SELECTOR_SOUTH, ARGUMENT_COMMON_SELECTOR_WEST,
};
use crate::message::domain::window::{
    ARGUMENT_WINDOW_SCRATCHPAD_RECOVER, ARGUMENT_WINDOW_TOGGLE_EXPOSE,
    ARGUMENT_WINDOW_TOGGLE_FLOAT, ARGUMENT_WINDOW_TOGGLE_NATIVE, ARGUMENT_WINDOW_TOGGLE_PARENT,
    ARGUMENT_WINDOW_TOGGLE_PICTURE_IN_PICTURE, ARGUMENT_WINDOW_TOGGLE_SHADOW,
    ARGUMENT_WINDOW_TOGGLE_SPLIT, ARGUMENT_WINDOW_TOGGLE_STICKY, ARGUMENT_WINDOW_TOGGLE_WINDOWED,
    ARGUMENT_WINDOW_TOGGLE_ZOOM_FULLSCREEN,
};
use crate::message::token::{
    Token, TokenValueType, is_token_equal_to, parse_token_into_typed_value,
};
use crate::support::response::{FailurePiece, Response};

#[derive(Clone, Copy)]
pub(crate) enum LabelType {
    Display,
    Space,
    Window,
}

pub(crate) const RESERVED_DISPLAY_IDENTIFIERS: [&str; 10] = [
    ARGUMENT_COMMON_SELECTOR_NORTH,
    ARGUMENT_COMMON_SELECTOR_EAST,
    ARGUMENT_COMMON_SELECTOR_SOUTH,
    ARGUMENT_COMMON_SELECTOR_WEST,
    ARGUMENT_COMMON_SELECTOR_PREVIOUS,
    ARGUMENT_COMMON_SELECTOR_NEXT,
    ARGUMENT_COMMON_SELECTOR_FIRST,
    ARGUMENT_COMMON_SELECTOR_LAST,
    ARGUMENT_COMMON_SELECTOR_RECENT,
    ARGUMENT_COMMON_SELECTOR_MOUSE,
];

pub(crate) const RESERVED_SPACE_IDENTIFIERS: [&str; 6] = [
    ARGUMENT_COMMON_SELECTOR_PREVIOUS,
    ARGUMENT_COMMON_SELECTOR_NEXT,
    ARGUMENT_COMMON_SELECTOR_FIRST,
    ARGUMENT_COMMON_SELECTOR_LAST,
    ARGUMENT_COMMON_SELECTOR_RECENT,
    ARGUMENT_COMMON_SELECTOR_MOUSE,
];

pub(crate) const RESERVED_WINDOW_IDENTIFIERS: [&str; 11] = [
    ARGUMENT_WINDOW_TOGGLE_FLOAT,
    ARGUMENT_WINDOW_TOGGLE_STICKY,
    ARGUMENT_WINDOW_TOGGLE_SHADOW,
    ARGUMENT_WINDOW_TOGGLE_SPLIT,
    ARGUMENT_WINDOW_TOGGLE_PARENT,
    ARGUMENT_WINDOW_TOGGLE_ZOOM_FULLSCREEN,
    ARGUMENT_WINDOW_TOGGLE_WINDOWED,
    ARGUMENT_WINDOW_TOGGLE_NATIVE,
    ARGUMENT_WINDOW_TOGGLE_EXPOSE,
    ARGUMENT_WINDOW_TOGGLE_PICTURE_IN_PICTURE,
    ARGUMENT_WINDOW_SCRATCHPAD_RECOVER,
];

pub(crate) fn parse_label_refusing_numbers_and_reserved_words(
    response: &mut Response,
    message_bytes: &[u8],
    token: Token,
    label_type: LabelType,
    label: &mut Option<String>,
) -> bool {
    let value = parse_token_into_typed_value(token, message_bytes);

    if matches!(value.type_of_value, TokenValueType::Invalid) {
        *label = None;
        return true;
    }

    if !matches!(value.type_of_value, TokenValueType::String) {
        response.write_failure_pieces_unless_silent(&[
            FailurePiece::Text("'"),
            FailurePiece::Bytes(token.bytes(message_bytes)),
            FailurePiece::Text("' cannot be used as a label.\n"),
        ]);
        return false;
    }

    match label_type {
        LabelType::Display => {
            for index in 0..RESERVED_DISPLAY_IDENTIFIERS.len() {
                if is_token_equal_to(token, message_bytes, RESERVED_DISPLAY_IDENTIFIERS[index]) {
                    response.write_failure_pieces_unless_silent(&[
                        FailurePiece::Text("'"),
                        FailurePiece::Bytes(token.bytes(message_bytes)),
                        FailurePiece::Text(
                            "' is a reserved keyword and cannot be used as a label.\n",
                        ),
                    ]);
                    return false;
                }
            }
        }
        LabelType::Space => {
            for index in 0..RESERVED_SPACE_IDENTIFIERS.len() {
                if is_token_equal_to(token, message_bytes, RESERVED_SPACE_IDENTIFIERS[index]) {
                    response.write_failure_pieces_unless_silent(&[
                        FailurePiece::Text("'"),
                        FailurePiece::Bytes(token.bytes(message_bytes)),
                        FailurePiece::Text(
                            "' is a reserved keyword and cannot be used as a label.\n",
                        ),
                    ]);
                    return false;
                }
            }
        }
        LabelType::Window => {
            for index in 0..RESERVED_WINDOW_IDENTIFIERS.len() {
                if is_token_equal_to(token, message_bytes, RESERVED_WINDOW_IDENTIFIERS[index]) {
                    response.write_failure_pieces_unless_silent(&[
                        FailurePiece::Text("'"),
                        FailurePiece::Bytes(token.bytes(message_bytes)),
                        FailurePiece::Text(
                            "' is a reserved keyword and cannot be used as a scratchpad.\n",
                        ),
                    ]);
                    return false;
                }
            }
        }
    }

    *label = Some(String::from_utf8_lossy(token.bytes(message_bytes)).into_owned());

    true
}

#[cfg(test)]
mod tests {
    use super::{LabelType, parse_label_refusing_numbers_and_reserved_words};
    use crate::message::token::MessageCursor;
    use crate::support::response::Response;

    struct ParsedLabel {
        accepted: bool,
        label: Option<String>,
        response_text: String,
    }

    fn parse_the_label_argument(argument: &str, label_type: LabelType) -> ParsedLabel {
        let mut message = argument.as_bytes().to_vec();
        message.extend_from_slice(b"\0\0");
        let token = MessageCursor::new(&mut message).take_next_token();
        let mut label = Some("old".to_string());
        let mut response = Response::collecting();

        let accepted = parse_label_refusing_numbers_and_reserved_words(
            &mut response,
            &message,
            token,
            label_type,
            &mut label,
        );
        let (standard_output, failures) = response.into_standard_output_and_one_failure_per_line();
        let response_text = failures
            .iter()
            .fold(standard_output, |text, failure| text + failure + "\n");

        ParsedLabel {
            accepted,
            label,
            response_text,
        }
    }

    fn assert_accepted_as(argument: &str, label_type: LabelType, expected_label: Option<&str>) {
        let parsed = parse_the_label_argument(argument, label_type);

        assert!(parsed.accepted, "{argument:?} should be accepted");
        assert_eq!(
            parsed.label.as_deref(),
            expected_label,
            "label for {argument:?}"
        );
        assert_eq!(parsed.response_text, "", "response for {argument:?}");
    }

    fn assert_rejected_with(argument: &str, label_type: LabelType, expected_response: &str) {
        let parsed = parse_the_label_argument(argument, label_type);

        assert!(!parsed.accepted, "{argument:?} should be rejected");
        assert_eq!(
            parsed.label.as_deref(),
            Some("old"),
            "label after rejecting {argument:?}"
        );
        assert_eq!(
            parsed.response_text, expected_response,
            "response for {argument:?}"
        );
    }

    const EVERY_LABEL_TYPE: [LabelType; 3] =
        [LabelType::Display, LabelType::Space, LabelType::Window];

    #[test]
    fn an_empty_label_clears_the_label_and_is_accepted() {
        for label_type in EVERY_LABEL_TYPE {
            assert_accepted_as("", label_type, None);
        }
    }

    #[test]
    fn a_word_that_is_not_reserved_becomes_the_label() {
        for label_type in EVERY_LABEL_TYPE {
            assert_accepted_as("main", label_type, Some("main"));
            assert_accepted_as("Main", label_type, Some("Main"));
            assert_accepted_as("stack", label_type, Some("stack"));
        }
    }

    #[test]
    fn a_number_cannot_be_used_as_a_label() {
        for label_type in EVERY_LABEL_TYPE {
            for argument in ["5", "0x1f", "1.5", " 2"] {
                assert_rejected_with(
                    argument,
                    label_type,
                    &format!("'{argument}' cannot be used as a label.\n"),
                );
            }
        }
    }

    #[test]
    fn a_display_label_cannot_be_a_display_selector_word() {
        for argument in [
            "north", "east", "south", "west", "prev", "next", "first", "last", "recent", "mouse",
        ] {
            assert_rejected_with(
                argument,
                LabelType::Display,
                &format!("'{argument}' is a reserved keyword and cannot be used as a label.\n"),
            );
        }
    }

    #[test]
    fn a_space_label_cannot_be_a_space_selector_word_but_may_be_a_direction() {
        for argument in ["prev", "next", "first", "last", "recent", "mouse"] {
            assert_rejected_with(
                argument,
                LabelType::Space,
                &format!("'{argument}' is a reserved keyword and cannot be used as a label.\n"),
            );
        }
        assert_accepted_as("north", LabelType::Space, Some("north"));
    }

    #[test]
    fn a_scratchpad_cannot_be_a_window_toggle_word_but_may_be_a_selector_word() {
        for argument in [
            "float",
            "sticky",
            "shadow",
            "split",
            "zoom-parent",
            "zoom-fullscreen",
            "windowed-fullscreen",
            "native-fullscreen",
            "expose",
            "pip",
            "recover",
        ] {
            assert_rejected_with(
                argument,
                LabelType::Window,
                &format!(
                    "'{argument}' is a reserved keyword and cannot be used as a scratchpad.\n"
                ),
            );
        }
        for argument in ["north", "prev", "mouse"] {
            assert_accepted_as(argument, LabelType::Window, Some(argument));
        }
    }

    #[test]
    fn window_toggle_words_are_accepted_as_display_and_space_labels() {
        for label_type in [LabelType::Display, LabelType::Space] {
            for argument in ["float", "recover", "zoom-parent"] {
                assert_accepted_as(argument, label_type, Some(argument));
            }
        }
    }
}
