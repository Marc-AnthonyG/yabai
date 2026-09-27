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
