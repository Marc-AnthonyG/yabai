#[derive(Clone, Copy)]
pub(crate) struct Token {
    pub(crate) start: usize,
    pub(crate) length: usize,
}

impl Token {
    pub(crate) fn bytes(self, message_bytes: &[u8]) -> &[u8] {
        &message_bytes[self.start..self.start + self.length]
    }

    pub(crate) fn as_c_string_pointer(self, message_bytes: &[u8]) -> *const libc::c_char {
        message_bytes[self.start..].as_ptr() as *const libc::c_char
    }
}

pub(crate) struct MessageCursor<'message> {
    pub(crate) bytes: &'message mut [u8],
    pub(crate) at: usize,
}

impl<'message> MessageCursor<'message> {
    pub(crate) fn new(bytes: &'message mut [u8]) -> MessageCursor<'message> {
        MessageCursor { bytes, at: 0 }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &*self.bytes
    }

    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut *self.bytes
    }

    pub(crate) fn cursor_at(&mut self, at: usize) -> MessageCursor<'_> {
        MessageCursor {
            bytes: &mut *self.bytes,
            at,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TokenType {
    Invalid,
    Int(i32),
    Float(f32),
    U32(u32),
    String,
}

#[derive(Clone, Copy)]
pub(crate) struct TokenValue {
    pub(crate) token: Token,
    pub(crate) type_of_value: TokenType,
}

pub(crate) struct KeyValuePair {
    pub(crate) key: usize,
    pub(crate) value: usize,
    pub(crate) exclusion: bool,
}

impl MessageCursor<'_> {
    pub(crate) fn get_token(&mut self) -> Token {
        let start = self.at;
        while self.at < self.bytes.len() && self.bytes[self.at] != 0 {
            self.at += 1;
        }
        let length = self.at - start;

        let stopped_on_a_null = self.at < self.bytes.len() && self.bytes[self.at] == 0;
        let byte_after_the_null_is_not_null =
            self.at + 1 < self.bytes.len() && self.bytes[self.at + 1] != 0;

        if stopped_on_a_null && byte_after_the_null_is_not_null {
            self.at += 1;
        } else {
            // NOTE(asmvik): don't go past the null-terminator
        }

        Token { start, length }
    }
}

pub(crate) fn token_prefix(token: Token, message_bytes: &[u8], candidate: &str) -> bool {
    let token_bytes = token.bytes(message_bytes);
    let candidate_bytes = candidate.as_bytes();

    for index in 0..token_bytes.len() {
        if index == candidate_bytes.len() {
            return true;
        }
        if token_bytes[index] != candidate_bytes[index] {
            return false;
        }
    }

    token_bytes.len() == candidate_bytes.len()
}

pub(crate) fn token_equals(token: Token, message_bytes: &[u8], candidate: &str) -> bool {
    token.bytes(message_bytes) == candidate.as_bytes()
}

impl Token {
    pub(crate) fn is_valid(self) -> bool {
        self.length > 0
    }
}

pub(crate) fn token_is_positive_integer(token: Token, message_bytes: &[u8]) -> Option<i32> {
    let mut value: i32 = 0;

    for character in token.bytes(message_bytes) {
        if !(*character >= b'0' && *character <= b'9') {
            return None;
        }
        value = value
            .wrapping_mul(10)
            .wrapping_add((*character - b'0') as i32);
    }

    Some(value)
}

pub(crate) fn token_is_hexadecimal(token: Token, message_bytes: &[u8]) -> Option<u32> {
    if token.length <= 2 {
        return None;
    }

    let token_bytes = token.bytes(message_bytes);
    if !(token_bytes[0] == b'0' && (token_bytes[1] == b'x' || token_bytes[1] == b'X')) {
        return None;
    }

    let mut value: u32 = 0;
    for character in &token_bytes[2..] {
        let digit = match *character {
            b'0'..=b'9' => *character - b'0',
            b'a'..=b'f' => *character - b'a' + 0xA,
            b'A'..=b'F' => *character - b'A' + 0xA,
            _ => return None,
        };
        value = value.wrapping_mul(16).wrapping_add(digit as u32);
    }

    Some(value)
}

pub(crate) fn token_is_float(token: Token, message_bytes: &[u8]) -> Option<f32> {
    let mut end: *mut libc::c_char = std::ptr::null_mut();
    let value = unsafe { libc::strtof(token.as_c_string_pointer(message_bytes), &mut end) };

    if end.is_null() || unsafe { *end } != 0 {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn token_to_value(token: Token, message_bytes: &[u8]) -> TokenValue {
    let type_of_value = if !token.is_valid() {
        TokenType::Invalid
    } else if let Some(value) = token_is_positive_integer(token, message_bytes) {
        TokenType::Int(value)
    } else if let Some(value) = token_is_hexadecimal(token, message_bytes) {
        TokenType::U32(value)
    } else if let Some(value) = token_is_float(token, message_bytes) {
        TokenType::Float(value)
    } else {
        TokenType::String
    };

    TokenValue {
        token,
        type_of_value,
    }
}

pub(crate) fn c_string_at(message_bytes: &[u8], start: usize) -> &[u8] {
    let end = message_bytes[start..]
        .iter()
        .position(|byte| *byte == 0)
        .map_or(message_bytes.len(), |offset| start + offset);
    &message_bytes[start..end]
}

pub(crate) fn parse_key_value_pair(
    message_bytes: &mut [u8],
    token_start: usize,
) -> Option<KeyValuePair> {
    let mut at = token_start;

    while at < message_bytes.len() && message_bytes[at] != 0 {
        let first_character = message_bytes[at];
        let second_character = if at + 1 < message_bytes.len() {
            message_bytes[at + 1]
        } else {
            0
        };

        if first_character == b'!' && second_character == b'=' {
            break;
        } else if first_character == b'=' {
            break;
        }

        at += 1;
    }

    let first_character = if at < message_bytes.len() {
        message_bytes[at]
    } else {
        0
    };
    let second_character = if at + 1 < message_bytes.len() {
        message_bytes[at + 1]
    } else {
        0
    };

    let index = if first_character == b'!' && second_character == b'=' {
        2
    } else {
        1
    };
    let check = if index == 2 { b'!' } else { b'=' };

    if first_character != check {
        return None;
    }

    let value = at + index;
    if value < message_bytes.len() && message_bytes[value] != 0 {
        message_bytes[at] = 0;
        Some(KeyValuePair {
            key: token_start,
            value,
            exclusion: index == 2,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MessageCursor, Token, TokenType, c_string_at, parse_key_value_pair, token_equals,
        token_prefix, token_to_value,
    };

    #[derive(Debug, PartialEq)]
    enum Classification {
        Invalid,
        Int(i32),
        U32(u32),
        FloatBits(u32),
        String,
    }

    fn message_from_arguments(arguments: &[&str]) -> Vec<u8> {
        let mut message = Vec::new();
        for argument in arguments {
            message.extend_from_slice(argument.as_bytes());
            message.push(0);
        }
        message.push(0);
        message
    }

    fn start_and_length_of_each_token(
        message: &mut [u8],
        token_count: usize,
    ) -> Vec<(usize, usize)> {
        let mut cursor = MessageCursor::new(message);
        (0..token_count)
            .map(|_| {
                let token = cursor.get_token();
                (token.start, token.length)
            })
            .collect()
    }

    fn classify_the_first_argument(argument: &str) -> Classification {
        let mut message = message_from_arguments(&[argument]);
        let mut cursor = MessageCursor::new(&mut message);
        let token = cursor.get_token();
        match token_to_value(token, cursor.bytes()).type_of_value {
            TokenType::Invalid => Classification::Invalid,
            TokenType::Int(value) => Classification::Int(value),
            TokenType::U32(value) => Classification::U32(value),
            TokenType::Float(value) => Classification::FloatBits(value.to_bits()),
            TokenType::String => Classification::String,
        }
    }

    fn assert_each_argument_classifies_as(expected_classifications: &[(&str, Classification)]) {
        for (argument, expected_classification) in expected_classifications {
            assert_eq!(
                &classify_the_first_argument(argument),
                expected_classification,
                "classifying {argument:?}"
            );
        }
    }

    fn first_token_of(message: &mut [u8]) -> Token {
        MessageCursor::new(message).get_token()
    }

    #[test]
    fn get_token_returns_each_argument_in_turn_and_steps_over_its_terminator() {
        let mut message = message_from_arguments(&["window", "--focus", "west"]);

        let tokens = start_and_length_of_each_token(&mut message, 3);

        assert_eq!(tokens, vec![(0, 6), (7, 7), (15, 4)]);
    }

    #[test]
    fn get_token_parks_on_the_terminator_of_the_last_argument() {
        let mut message = message_from_arguments(&["window", "--focus", "west"]);
        let mut cursor = MessageCursor::new(&mut message);

        for _ in 0..3 {
            cursor.get_token();
        }

        assert_eq!(cursor.at, 19);
    }

    #[test]
    fn get_token_keeps_returning_empty_tokens_after_the_double_nul() {
        let mut message = message_from_arguments(&["window", "--focus", "west"]);

        let tokens = start_and_length_of_each_token(&mut message, 6);

        assert_eq!(tokens[3..], [(19, 0), (19, 0), (19, 0)]);
    }

    #[test]
    fn get_token_after_a_single_argument_returns_empty_tokens_at_its_terminator() {
        let mut message = message_from_arguments(&["query"]);

        let tokens = start_and_length_of_each_token(&mut message, 3);

        assert_eq!(tokens, vec![(0, 5), (5, 0), (5, 0)]);
    }

    #[test]
    fn get_token_on_an_empty_message_returns_empty_tokens_at_the_start_forever() {
        let mut message = message_from_arguments(&[]);
        let mut cursor = MessageCursor::new(&mut message);

        for _ in 0..3 {
            let token = cursor.get_token();
            assert_eq!((token.start, token.length), (0, 0));
        }
        assert_eq!(cursor.at, 0);
    }

    #[test]
    fn get_token_treats_an_empty_argument_as_the_end_of_the_message() {
        let mut message = message_from_arguments(&["a", "", "b"]);

        let tokens = start_and_length_of_each_token(&mut message, 4);

        assert_eq!(tokens, vec![(0, 1), (1, 0), (1, 0), (1, 0)]);
    }

    #[test]
    fn token_is_valid_only_when_it_is_not_empty() {
        let mut message = message_from_arguments(&["x"]);
        let mut cursor = MessageCursor::new(&mut message);

        assert!(cursor.get_token().is_valid());
        assert!(!cursor.get_token().is_valid());
    }

    #[test]
    fn token_equals_matches_only_the_whole_candidate() {
        let mut message = message_from_arguments(&["window"]);
        let token = first_token_of(&mut message);

        assert!(token_equals(token, &message, "window"));
        assert!(!token_equals(token, &message, "win"));
        assert!(!token_equals(token, &message, "windows"));
        assert!(!token_equals(token, &message, ""));
    }

    #[test]
    fn an_empty_token_equals_only_the_empty_candidate() {
        let mut message = message_from_arguments(&[]);
        let token = first_token_of(&mut message);

        assert!(token_equals(token, &message, ""));
        assert!(!token_equals(token, &message, "a"));
    }

    #[test]
    fn token_prefix_is_true_when_the_candidate_is_a_prefix_of_the_token() {
        let mut message = message_from_arguments(&["window"]);
        let token = first_token_of(&mut message);

        assert!(token_prefix(token, &message, "window"));
        assert!(token_prefix(token, &message, "win"));
        assert!(token_prefix(token, &message, "w"));
        assert!(token_prefix(token, &message, ""));
        assert!(!token_prefix(token, &message, "windows"));
        assert!(!token_prefix(token, &message, "x"));
    }

    #[test]
    fn an_empty_token_has_only_the_empty_prefix() {
        let mut message = message_from_arguments(&[]);
        let token = first_token_of(&mut message);

        assert!(token_prefix(token, &message, ""));
        assert!(!token_prefix(token, &message, "a"));
    }

    #[test]
    fn token_to_value_classifies_an_empty_token_as_invalid() {
        assert_eq!(classify_the_first_argument(""), Classification::Invalid);
    }

    #[test]
    fn token_to_value_classifies_digit_only_tokens_as_integers() {
        assert_each_argument_classifies_as(&[
            ("0", Classification::Int(0)),
            ("7", Classification::Int(7)),
            ("42", Classification::Int(42)),
            ("007", Classification::Int(7)),
            ("2147483647", Classification::Int(2147483647)),
        ]);
    }

    #[test]
    fn token_to_value_wraps_decimal_integers_past_i32_max_as_the_c_build_does() {
        assert_each_argument_classifies_as(&[
            ("2147483648", Classification::Int(-2147483648)),
            ("4294967296", Classification::Int(0)),
            ("99999999999", Classification::Int(1215752191)),
        ]);
    }

    #[test]
    fn token_to_value_classifies_0x_prefixed_hexadecimal_digits_as_u32() {
        assert_each_argument_classifies_as(&[
            ("0x0", Classification::U32(0)),
            ("0xff", Classification::U32(255)),
            ("0XFF", Classification::U32(255)),
            ("0xDeadBeef", Classification::U32(3735928559)),
            ("0xFFFFFFFF", Classification::U32(4294967295)),
        ]);
    }

    #[test]
    fn token_to_value_wraps_hexadecimal_values_past_u32_max() {
        assert_eq!(
            classify_the_first_argument("0x100000000"),
            Classification::U32(0)
        );
    }

    #[test]
    fn token_to_value_treats_a_bare_0x_or_a_non_hexadecimal_digit_as_a_string() {
        assert_each_argument_classifies_as(&[
            ("0x", Classification::String),
            ("0X", Classification::String),
            ("0x1G", Classification::String),
            ("0xg", Classification::String),
        ]);
    }

    #[test]
    fn token_to_value_classifies_hexadecimal_floats_that_strtof_accepts_as_floats() {
        assert_each_argument_classifies_as(&[
            ("0x1p3", Classification::FloatBits(0x41000000)),
            ("0x1.8p1", Classification::FloatBits(0x40400000)),
            ("0X1P-2", Classification::FloatBits(0x3e800000)),
        ]);
    }

    #[test]
    fn token_to_value_classifies_signed_and_decimal_numbers_as_floats() {
        assert_each_argument_classifies_as(&[
            ("-5", Classification::FloatBits(0xc0a00000)),
            ("+5", Classification::FloatBits(0x40a00000)),
            ("1.5", Classification::FloatBits(0x3fc00000)),
            ("-0.25", Classification::FloatBits(0xbe800000)),
            (".5", Classification::FloatBits(0x3f000000)),
            ("5.", Classification::FloatBits(0x40a00000)),
            ("0.1", Classification::FloatBits(0x3dcccccd)),
            ("1e3", Classification::FloatBits(0x447a0000)),
            ("1E-2", Classification::FloatBits(0x3c23d70a)),
        ]);
    }

    #[test]
    fn token_to_value_accepts_leading_whitespace_as_strtof_does_but_not_trailing_whitespace() {
        assert_each_argument_classifies_as(&[
            (" 1.5", Classification::FloatBits(0x3fc00000)),
            ("\t2", Classification::FloatBits(0x40000000)),
            ("\n3", Classification::FloatBits(0x40400000)),
            ("1.5 ", Classification::String),
            ("  ", Classification::String),
        ]);
    }

    #[test]
    fn token_to_value_accepts_the_infinity_and_nan_spellings_of_strtof() {
        assert_each_argument_classifies_as(&[
            ("inf", Classification::FloatBits(0x7f800000)),
            ("-inf", Classification::FloatBits(0xff800000)),
            ("INF", Classification::FloatBits(0x7f800000)),
            ("infinity", Classification::FloatBits(0x7f800000)),
            ("nan", Classification::FloatBits(0x7fc00000)),
            ("NaN", Classification::FloatBits(0x7fc00000)),
            ("-nan", Classification::FloatBits(0xffc00000)),
            ("nan(1)", Classification::FloatBits(0x7fc00001)),
        ]);
    }

    #[test]
    fn token_to_value_saturates_out_of_range_floats_as_strtof_does() {
        assert_each_argument_classifies_as(&[
            ("1e40", Classification::FloatBits(0x7f800000)),
            ("-1e40", Classification::FloatBits(0xff800000)),
            ("3.4028236e38", Classification::FloatBits(0x7f800000)),
            ("1e-50", Classification::FloatBits(0x00000000)),
        ]);
    }

    #[test]
    fn token_to_value_classifies_what_strtof_does_not_consume_entirely_as_a_string() {
        assert_each_argument_classifies_as(&[
            ("1e", Classification::String),
            ("1,5", Classification::String),
            ("1.5.2", Classification::String),
            ("abc", Classification::String),
            ("west", Classification::String),
            ("--focus", Classification::String),
            ("-", Classification::String),
            ("+", Classification::String),
            ("e5", Classification::String),
        ]);
    }

    #[test]
    fn c_string_at_stops_at_the_next_terminator() {
        let message = message_from_arguments(&["window", "--focus"]);

        assert_eq!(c_string_at(&message, 0), b"window");
        assert_eq!(c_string_at(&message, 3), b"dow");
        assert_eq!(c_string_at(&message, 7), b"--focus");
        assert_eq!(c_string_at(&message, 15), b"");
    }

    struct ExpectedKeyValuePair {
        argument: &'static str,
        value_offset: usize,
        key: &'static str,
        value: &'static str,
        message_after_parsing: &'static [u8],
    }

    fn assert_parses_as_key_value_pair(
        expected_pair: &ExpectedKeyValuePair,
        expected_exclusion: bool,
    ) {
        let argument = expected_pair.argument;
        let mut message = message_from_arguments(&[argument]);

        let pair = parse_key_value_pair(&mut message, 0)
            .unwrap_or_else(|| panic!("{argument:?} should parse as a key-value pair"));

        assert_eq!(
            (pair.key, pair.value, pair.exclusion),
            (0, expected_pair.value_offset, expected_exclusion),
            "offsets and exclusion of {argument:?}"
        );
        assert_eq!(
            c_string_at(&message, pair.key),
            expected_pair.key.as_bytes(),
            "key of {argument:?}"
        );
        assert_eq!(
            c_string_at(&message, pair.value),
            expected_pair.value.as_bytes(),
            "value of {argument:?}"
        );
        assert_eq!(
            message, expected_pair.message_after_parsing,
            "message after parsing {argument:?}"
        );
    }

    #[test]
    fn parse_key_value_pair_splits_at_the_first_equals_sign_in_place() {
        let expected_pairs = [
            ExpectedKeyValuePair {
                argument: "app=Safari",
                value_offset: 4,
                key: "app",
                value: "Safari",
                message_after_parsing: b"app\0Safari\0\0",
            },
            ExpectedKeyValuePair {
                argument: "title=a=b",
                value_offset: 6,
                key: "title",
                value: "a=b",
                message_after_parsing: b"title\0a=b\0\0",
            },
            ExpectedKeyValuePair {
                argument: "a!b=c",
                value_offset: 4,
                key: "a!b",
                value: "c",
                message_after_parsing: b"a!b\0c\0\0",
            },
            ExpectedKeyValuePair {
                argument: "=value",
                value_offset: 1,
                key: "",
                value: "value",
                message_after_parsing: b"\0value\0\0",
            },
            ExpectedKeyValuePair {
                argument: "app=!=x",
                value_offset: 4,
                key: "app",
                value: "!=x",
                message_after_parsing: b"app\0!=x\0\0",
            },
        ];

        for expected_pair in &expected_pairs {
            assert_parses_as_key_value_pair(expected_pair, false);
        }
    }

    #[test]
    fn parse_key_value_pair_treats_not_equals_as_an_exclusion() {
        let expected_pairs = [
            ExpectedKeyValuePair {
                argument: "app!=Safari",
                value_offset: 5,
                key: "app",
                value: "Safari",
                message_after_parsing: b"app\0=Safari\0\0",
            },
            ExpectedKeyValuePair {
                argument: "title!=a=b",
                value_offset: 7,
                key: "title",
                value: "a=b",
                message_after_parsing: b"title\0=a=b\0\0",
            },
            ExpectedKeyValuePair {
                argument: "!=value",
                value_offset: 2,
                key: "",
                value: "value",
                message_after_parsing: b"\0=value\0\0",
            },
        ];

        for expected_pair in &expected_pairs {
            assert_parses_as_key_value_pair(expected_pair, true);
        }
    }

    #[test]
    fn parse_key_value_pair_reports_offsets_from_the_start_of_the_message() {
        let mut message = message_from_arguments(&["--add", "app!=Safari"]);

        let pair = parse_key_value_pair(&mut message, 6).expect("app!=Safari is a key-value pair");

        assert_eq!((pair.key, pair.value, pair.exclusion), (6, 11, true));
        assert_eq!(c_string_at(&message, pair.key), b"app");
        assert_eq!(c_string_at(&message, pair.value), b"Safari");
    }

    #[test]
    fn parse_key_value_pair_rejects_a_missing_value_or_operator_and_leaves_the_message_intact() {
        for argument in ["app=", "app!=", "app", "app!", ""] {
            let mut message = message_from_arguments(&[argument]);
            let message_before = message.clone();

            assert!(
                parse_key_value_pair(&mut message, 0).is_none(),
                "{argument:?} should not parse as a key-value pair"
            );
            assert_eq!(
                message, message_before,
                "message after rejecting {argument:?}"
            );
        }
    }
}
