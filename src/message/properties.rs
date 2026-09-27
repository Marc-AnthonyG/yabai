use crate::message::token::{Token, c_string_at, token_prefix};
use crate::support::response::{FailurePiece, Response};

pub(crate) struct Properties {
    pub(crate) token: Token,
    pub(crate) did_parse: bool,
    pub(crate) did_error: bool,
    pub(crate) flags: u64,
}

pub(crate) fn parse_property(
    properties: &mut Properties,
    property: &[u8],
    property_values: &[u64],
    property_strings: &[&str],
) -> bool {
    for index in 0..property_strings.len() {
        if property == property_strings[index].as_bytes() {
            properties.flags |= property_values[index];
            return true;
        }
    }

    false
}

pub(crate) fn parse_properties(
    response: &mut Response,
    message_bytes: &mut [u8],
    token: Token,
    property_values: &[u64],
    property_strings: &[&str],
) -> Properties {
    let mut result = Properties {
        token,
        did_parse: false,
        did_error: false,
        flags: 0,
    };

    result.did_parse = token.is_valid() && !token_prefix(token, message_bytes, "--");
    if !result.did_parse {
        return result;
    }

    let mut cursor = 0;
    for index in 0..token.length {
        if index + 1 == token.length {
            let property = c_string_at(message_bytes, token.start + cursor);
            if !parse_property(&mut result, property, property_values, property_strings) {
                let reported = &message_bytes[token.start + cursor..token.start + index + 1];
                response.fail_pieces(&[
                    FailurePiece::Text("'"),
                    FailurePiece::BytesStoppingAtFirstNull(reported),
                    FailurePiece::Text("' is not a valid property.\n"),
                ]);
                result.did_error = true;
            }
        } else if message_bytes[token.start + index] == b',' {
            message_bytes[token.start + index] = 0;

            let property = c_string_at(message_bytes, token.start + cursor);
            if !parse_property(&mut result, property, property_values, property_strings) {
                let reported = &message_bytes[token.start + cursor..token.start + index + 1];
                response.fail_pieces(&[
                    FailurePiece::Text("'"),
                    FailurePiece::BytesStoppingAtFirstNull(reported),
                    FailurePiece::Text("' is not a valid property.\n"),
                ]);
                result.did_error = true;
            }

            cursor = index + 1;
        }
    }

    result
}
