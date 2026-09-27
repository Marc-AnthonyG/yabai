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

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::os::unix::net::UnixStream;

    use super::parse_properties;
    use crate::message::token::MessageCursor;
    use crate::support::response::Response;

    const PROPERTY_VALUES: [u64; 4] = [0x1, 0x2, 0x4, 0x8];
    const PROPERTY_STRINGS: [&str; 4] = ["id", "pid", "app", "title"];

    struct ParsedProperties {
        did_parse: bool,
        did_error: bool,
        flags: u64,
        response_bytes: Vec<u8>,
        message_after_parsing: Vec<u8>,
    }

    fn parse_the_properties_argument(argument: &str) -> ParsedProperties {
        let mut message = argument.as_bytes().to_vec();
        message.extend_from_slice(b"\0\0");
        let token = MessageCursor::new(&mut message).get_token();
        let (daemon_side, mut client_side) = UnixStream::pair().expect("a connected socket pair");

        let properties = {
            let mut response = Response::to_client(daemon_side);
            parse_properties(
                &mut response,
                &mut message,
                token,
                &PROPERTY_VALUES,
                &PROPERTY_STRINGS,
            )
        };
        let mut response_bytes = Vec::new();
        client_side
            .read_to_end(&mut response_bytes)
            .expect("the response to be readable");

        ParsedProperties {
            did_parse: properties.did_parse,
            did_error: properties.did_error,
            flags: properties.flags,
            response_bytes,
            message_after_parsing: message,
        }
    }

    #[test]
    fn parse_properties_ors_the_value_of_each_comma_separated_property() {
        let expected_flags: [(&str, u64, &[u8]); 3] = [
            ("id", 0x1, b"id\0\0"),
            ("id,app,title", 0xd, b"id\0app\0title\0\0"),
            ("title,id", 0x9, b"title\0id\0\0"),
        ];

        for (argument, expected_flags, expected_message) in expected_flags {
            let parsed = parse_the_properties_argument(argument);

            assert!(parsed.did_parse, "did_parse for {argument:?}");
            assert!(!parsed.did_error, "did_error for {argument:?}");
            assert_eq!(parsed.flags, expected_flags, "flags for {argument:?}");
            assert_eq!(parsed.response_bytes, b"", "response for {argument:?}");
            assert_eq!(
                parsed.message_after_parsing, expected_message,
                "message for {argument:?}"
            );
        }
    }

    #[test]
    fn parse_properties_reports_an_unknown_property_and_keeps_the_known_ones() {
        let parsed = parse_the_properties_argument("id,bogus,app");

        assert!(parsed.did_parse);
        assert!(parsed.did_error);
        assert_eq!(parsed.flags, 0x5);
        assert_eq!(
            parsed.response_bytes,
            b"\x07'bogus' is not a valid property.\n"
        );
        assert_eq!(parsed.message_after_parsing, b"id\0bogus\0app\0\0");
    }

    #[test]
    fn parse_properties_keeps_a_trailing_comma_on_the_last_property_and_rejects_it() {
        let parsed = parse_the_properties_argument("id,title,");

        assert!(parsed.did_error);
        assert_eq!(parsed.flags, 0x1);
        assert_eq!(
            parsed.response_bytes,
            b"\x07'title,' is not a valid property.\n"
        );
        assert_eq!(parsed.message_after_parsing, b"id\0title,\0\0");
    }

    #[test]
    fn parse_properties_rejects_an_empty_property_between_two_commas() {
        let parsed = parse_the_properties_argument("id,,app");

        assert!(parsed.did_error);
        assert_eq!(parsed.flags, 0x5);
        assert_eq!(parsed.response_bytes, b"\x07'' is not a valid property.\n");
        assert_eq!(parsed.message_after_parsing, b"id\0\0app\0\0");
    }

    #[test]
    fn parse_properties_rejects_a_prefix_or_an_extension_of_a_property_name() {
        for (argument, expected_response) in [
            ("i", &b"\x07'i' is not a valid property.\n"[..]),
            ("idx", b"\x07'idx' is not a valid property.\n"),
        ] {
            let parsed = parse_the_properties_argument(argument);

            assert!(parsed.did_parse, "did_parse for {argument:?}");
            assert!(parsed.did_error, "did_error for {argument:?}");
            assert_eq!(parsed.flags, 0, "flags for {argument:?}");
            assert_eq!(
                parsed.response_bytes, expected_response,
                "response for {argument:?}"
            );
        }
    }

    #[test]
    fn parse_properties_does_not_parse_an_empty_token_or_an_option() {
        for argument in ["", "--space"] {
            let parsed = parse_the_properties_argument(argument);

            assert!(!parsed.did_parse, "did_parse for {argument:?}");
            assert!(!parsed.did_error, "did_error for {argument:?}");
            assert_eq!(parsed.flags, 0, "flags for {argument:?}");
            assert_eq!(parsed.response_bytes, b"", "response for {argument:?}");
        }
    }

    #[test]
    fn parse_properties_with_a_silent_response_still_reports_the_error() {
        let mut message = b"bogus\0\0".to_vec();
        let token = MessageCursor::new(&mut message).get_token();

        let properties = parse_properties(
            &mut Response::silent(),
            &mut message,
            token,
            &PROPERTY_VALUES,
            &PROPERTY_STRINGS,
        );

        assert!(properties.did_parse);
        assert!(properties.did_error);
        assert_eq!(properties.flags, 0);
    }
}
