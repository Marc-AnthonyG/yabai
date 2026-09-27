use crate::message::common_arguments::{
    ARGUMENT_COMMON_SEL_EAST, ARGUMENT_COMMON_SEL_FIRST, ARGUMENT_COMMON_SEL_LAST,
    ARGUMENT_COMMON_SEL_MOUSE, ARGUMENT_COMMON_SEL_NEXT, ARGUMENT_COMMON_SEL_NORTH,
    ARGUMENT_COMMON_SEL_PREV, ARGUMENT_COMMON_SEL_RECENT, ARGUMENT_COMMON_SEL_SOUTH,
    ARGUMENT_COMMON_SEL_WEST,
};
use crate::message::domain::window::{
    ARGUMENT_WINDOW_SCRATCHPAD_RECOVER, ARGUMENT_WINDOW_TOGGLE_EXPOSE,
    ARGUMENT_WINDOW_TOGGLE_FLOAT, ARGUMENT_WINDOW_TOGGLE_FULLSC, ARGUMENT_WINDOW_TOGGLE_NATIVE,
    ARGUMENT_WINDOW_TOGGLE_PARENT, ARGUMENT_WINDOW_TOGGLE_PIP, ARGUMENT_WINDOW_TOGGLE_SHADOW,
    ARGUMENT_WINDOW_TOGGLE_SPLIT, ARGUMENT_WINDOW_TOGGLE_STICKY, ARGUMENT_WINDOW_TOGGLE_WINDOWED,
};
use crate::message::token::{Token, TokenType, token_equals, token_to_value};
use crate::support::response::{FailurePiece, Response};

#[derive(Clone, Copy)]
pub(crate) enum LabelType {
    Display,
    Space,
    Window,
}

pub(crate) const RESERVED_DISPLAY_IDENTIFIERS: [&str; 10] = [
    ARGUMENT_COMMON_SEL_NORTH,
    ARGUMENT_COMMON_SEL_EAST,
    ARGUMENT_COMMON_SEL_SOUTH,
    ARGUMENT_COMMON_SEL_WEST,
    ARGUMENT_COMMON_SEL_PREV,
    ARGUMENT_COMMON_SEL_NEXT,
    ARGUMENT_COMMON_SEL_FIRST,
    ARGUMENT_COMMON_SEL_LAST,
    ARGUMENT_COMMON_SEL_RECENT,
    ARGUMENT_COMMON_SEL_MOUSE,
];

pub(crate) const RESERVED_SPACE_IDENTIFIERS: [&str; 6] = [
    ARGUMENT_COMMON_SEL_PREV,
    ARGUMENT_COMMON_SEL_NEXT,
    ARGUMENT_COMMON_SEL_FIRST,
    ARGUMENT_COMMON_SEL_LAST,
    ARGUMENT_COMMON_SEL_RECENT,
    ARGUMENT_COMMON_SEL_MOUSE,
];

pub(crate) const RESERVED_WINDOW_IDENTIFIERS: [&str; 11] = [
    ARGUMENT_WINDOW_TOGGLE_FLOAT,
    ARGUMENT_WINDOW_TOGGLE_STICKY,
    ARGUMENT_WINDOW_TOGGLE_SHADOW,
    ARGUMENT_WINDOW_TOGGLE_SPLIT,
    ARGUMENT_WINDOW_TOGGLE_PARENT,
    ARGUMENT_WINDOW_TOGGLE_FULLSC,
    ARGUMENT_WINDOW_TOGGLE_WINDOWED,
    ARGUMENT_WINDOW_TOGGLE_NATIVE,
    ARGUMENT_WINDOW_TOGGLE_EXPOSE,
    ARGUMENT_WINDOW_TOGGLE_PIP,
    ARGUMENT_WINDOW_SCRATCHPAD_RECOVER,
];

pub(crate) fn parse_label(
    response: &mut Response,
    message_bytes: &[u8],
    token: Token,
    label_type: LabelType,
    label: &mut Option<String>,
) -> bool {
    let value = token_to_value(token, message_bytes);

    if matches!(value.type_of_value, TokenType::Invalid) {
        *label = None;
        return true;
    }

    if !matches!(value.type_of_value, TokenType::String) {
        response.fail_pieces(&[
            FailurePiece::Text("'"),
            FailurePiece::Bytes(token.bytes(message_bytes)),
            FailurePiece::Text("' cannot be used as a label.\n"),
        ]);
        return false;
    }

    match label_type {
        LabelType::Display => {
            for index in 0..RESERVED_DISPLAY_IDENTIFIERS.len() {
                if token_equals(token, message_bytes, RESERVED_DISPLAY_IDENTIFIERS[index]) {
                    response.fail_pieces(&[
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
                if token_equals(token, message_bytes, RESERVED_SPACE_IDENTIFIERS[index]) {
                    response.fail_pieces(&[
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
                if token_equals(token, message_bytes, RESERVED_WINDOW_IDENTIFIERS[index]) {
                    response.fail_pieces(&[
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
