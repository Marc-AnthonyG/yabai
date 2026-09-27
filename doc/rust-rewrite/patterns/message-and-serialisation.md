# Pattern: message parsing and serialisation

Phase-1 elaboration of `DECISIONS.md` **3**, **26**, **27**, **28**, **29** and **32** for the
command surface: `src/message.c` (3045 lines), `src/message.h`, the `struct rule` / `struct signal`
halves of `src/rule.c` and `src/event_signal.c`, every JSON writer the `query` domain reaches, the
`YABAI_*` environment block, and the client half of the wire protocol in `src/yabai.c`.

This document is prescriptive. Where an inventory in `files/` or `sweeps/` offers a choice, the
choice is made here and the rejected option is not mentioned again. A translator following this
document never has to decide anything about tokenising, response writing or JSON bytes.

All `path:line` references are against commit `dd84572`.

Names in the Rust sketches follow `DECISIONS.md` 37 (C names kept, nothing abbreviated), 38 (the
only comments are the ones that exist in the C source, carried over verbatim) and 39 (`unsafe` only
around FFI).

---

## 1. What is fixed and what is free

Fixed by `DECISIONS.md` 3, byte for byte, and therefore not negotiable anywhere below:

* the request framing (4-byte host-endian length, NUL-separated arguments, trailing double NUL)
* the response framing (raw bytes until close, one `0x07` prefix byte per failure message)
* every error message string in `src/message.c`, including its punctuation and trailing newline
* every query JSON byte, including the `[`/`]` asymmetries of §9.8 and the `", "` vs `","` split
* the twelve `YABAI_*` variable names and their `printf` conversions
* the client's exit code rule

Free: the internal shape of the parser, as long as the above holds.

`TIME_FUNCTION` (`src/misc/timer.h:136`) is **not** translated at any of its eleven sites in
`src/message.c` (`:667`, `:792`, `:873`, `:1154`, `:1702`, `:1761`, `:2049`, `:2421`, `:2598`,
`:2808`, `:2866`) nor at `src/rule.c:6`, `src/event_signal.c:402`, `:433`. `DECISIONS.md` 5 removes
the `PROFILE` machinery; each removal is one line in `DEVIATIONS.md`.

---

## 2. The message buffer, `Token`, and the `get_token` cursor

### 2.1 Where the buffer comes from

`EVENT_HANDLER(DAEMON_MESSAGE)` (`src/event_loop.c:1613-1644`) reads a 4-byte count, allocates
exactly that many bytes, reads the payload into them, and hands the block to
`handle_message(rsp, message)` (`src/event_loop.c:1634`). The block is *not* NUL-terminated by the
reader; it is NUL-terminated because the client appends two NUL bytes (`src/yabai.c:80-81`).

In Rust the block is a `Vec<u8>` owned by the handler and passed as `&mut [u8]`. It is **bytes, not
text**: `DECISIONS.md` 28 puts `from_utf8_lossy` at the point a value is *stored*, which for this
subsystem means only where a label, a regex source, a scratchpad name or a signal command is copied
out of the buffer into an owned `String` (§10). Nothing in the parser and nothing in an error
message goes through UTF-8 validation, so a window title or label containing invalid UTF-8 comes
back out of an error message unchanged.

### 2.2 `Token`

```rust
#[derive(Clone, Copy)]
struct Token {
    start: usize,
    length: usize,
}
```

This is `DECISIONS.md` 27 in full: a `(start, length)` range over the mutable message buffer, never
a `&[u8]` and never a `&str`. Index pairs are mandatory, not a preference — `parse_key_value_pair`
and `parse_properties` write NUL bytes into the same buffer while a token is alive (§2.6), which a
borrowed slice cannot express.

`struct token` in C (`src/message.c:254`) is `{ char *text; int length; }`. `text` is never NULL in
the daemon: every token is produced by `get_token`, which sets `token.text = *message`. So
`token_is_valid` (`src/message.c:338`, `text && length > 0`) becomes exactly:

```rust
impl Token {
    fn is_valid(self) -> bool {
        self.length > 0
    }
}
```

Two derived accessors, used everywhere below:

```rust
impl Token {
    fn bytes(self, message_bytes: &[u8]) -> &[u8] {
        &message_bytes[self.start..self.start + self.length]
    }

    fn as_c_string_pointer(self, message_bytes: &[u8]) -> *const libc::c_char {
        message_bytes[self.start..].as_ptr() as *const libc::c_char
    }
}
```

`as_c_string_pointer` is sound for every token the cursor produces, because the byte at
`start + length` is the NUL that terminated the scan. It stays sound after the `stack.` prefix
strip of §2.7, which moves `start` forward and shortens `length` without touching the NUL.

### 2.3 `MessageCursor`

```rust
struct MessageCursor<'message> {
    bytes: &'message mut [u8],
    at: usize,
}

impl<'message> MessageCursor<'message> {
    fn new(bytes: &'message mut [u8]) -> MessageCursor<'message> {
        MessageCursor { bytes, at: 0 }
    }

    fn bytes(&self) -> &[u8] {
        self.bytes
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes
    }

    fn cursor_at(&mut self, at: usize) -> MessageCursor<'_> {
        MessageCursor { bytes: self.bytes, at }
    }
}
```

`cursor_at` is the translation of C's `parse_display_selector(rsp, &value, ...)` at
`src/message.c:2685` and `parse_space_selector(rsp, &value, ...)` at `:2699`, where `value` is a
`char *` pointing *inside* the message buffer and the selector parser advances that local pointer
instead of the outer one. The reborrow freezes the outer cursor for the duration of the inner
parse and leaves its `at` untouched, which is exactly the C behaviour. Token indices produced by
the inner cursor remain valid after it is dropped, because they index the same buffer.

### 2.4 `get_token`, reproduced exactly

The C, `src/message.c:298-315`:

```c
static struct token get_token(char **message)
{
    struct token token;

    token.text = *message;
    while (**message) {
        ++(*message);
    }
    token.length = *message - token.text;

    if ((*message)[0] == '\0' && (*message)[1] != '\0') {
        ++(*message);
    } else {
        // NOTE(asmvik): don't go past the null-terminator
    }

    return token;
}
```

The Rust:

```rust
impl MessageCursor<'_> {
    fn get_token(&mut self) -> Token {
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
```

Four things this pins down, all of them load-bearing:

1. **The cursor parks at the double NUL.** With a well-formed message the last argument's NUL sits
   at `len - 2` and the terminator at `len - 1`. When the scan stops on the NUL at `len - 2`, the
   byte after it is NUL, so the cursor does not advance. Every subsequent `get_token` starts at
   `len - 2`, scans zero bytes, and returns `Token { start: len - 2, length: 0 }` — an invalid
   token, for ever. The five dispatch loops (`src/message.c:1169`, `:1779`, `:2062`, `:2604`,
   `:2877`) all terminate on `token_is_valid` being false, so this is the loop termination
   condition. A `split` iterator over NUL does not reproduce it and must not be used.
2. **The empty `else` branch carries the only explanatory comment in `src/message.c`.** Keep it
   verbatim, attached to the branch that implements it, per `DECISIONS.md` 38.
3. **The one-byte over-read at `src/message.c:308` is made safe by the length checks.** C reads
   `(*message)[1]` unconditionally; for a malformed client message that omits the second NUL this
   reads one byte past the end of the allocation. `byte_after_the_null_is_not_null` bounds-checks
   instead and treats an out-of-range index as "the byte is NUL", i.e. do not advance, i.e. park.
   This is `DECISIONS.md` 4: one line in `DEVIATIONS.md` reading
   *`src/message.c:308` — C read one byte past the message allocation when the trailing NUL was
   missing; Rust bounds-checks and treats the end of the buffer as the terminator.*
4. **`stopped_on_a_null` is only false at the end of the buffer.** In C the loop can only exit on a
   NUL, so the first half of the condition is always true there; it is written out in Rust because
   a truncated message can end without one. Same outcome: park.

### 2.5 `token_prefix`, `token_equals`

```rust
fn token_prefix(token: Token, message_bytes: &[u8], candidate: &str) -> bool {
    let token_bytes = token.bytes(message_bytes);
    let candidate_bytes = candidate.as_bytes();
    if token_bytes.len() < candidate_bytes.len() {
        return token_bytes == &candidate_bytes[..token_bytes.len()];
    }
    &token_bytes[..candidate_bytes.len()] == candidate_bytes
}

fn token_equals(token: Token, message_bytes: &[u8], candidate: &str) -> bool {
    token.bytes(message_bytes) == candidate.as_bytes()
}
```

`token_equals` (`src/message.c:327-336`) is exact byte equality including length, so the slice
comparison is a literal transcription. A zero-length token equals `""` in both. The comparison is
on bytes, never on `str`, so no UTF-8 validation and no locale enters the dispatch.

`token_prefix` (`src/message.c:317-325`) returns `true` when the *candidate* runs out first (the
token starts with it) **and** when the token runs out first at a point where the candidate also
ends. It is used twice: `token_prefix(token, "--")` at `src/message.c:629` and
`token_prefix(result.token, "stack.")` at `:1060`.

### 2.6 The two in-place NUL writers

`DECISIONS.md` 27 says the buffer is mutable and the tokens are ranges over it. Both writers keep
the C's in-place splitting; neither is rewritten to return sub-slices, because the NUL they write
is what makes the resulting `char *` a C string for `string_equals`, `sscanf` and `regcomp`
downstream.

#### `parse_key_value_pair` — `src/message.c:440-470`

C writes a NUL over the `=` (or over the `!` of `!=`) and hands back two `char *` into the buffer,
plus a `bool *exclusion` it leaves **untouched** on failure. Callers pre-initialise `exclusion` to
`false` (`src/message.c:2607`, `:2880`).

```rust
struct KeyValuePair {
    key: usize,
    value: usize,
    exclusion: bool,
}

fn parse_key_value_pair(message_bytes: &mut [u8], token_start: usize) -> Option<KeyValuePair> {
    let mut at = token_start;
    while at < message_bytes.len() && message_bytes[at] != 0 {
        let first = message_bytes[at];
        let second = if at + 1 < message_bytes.len() { message_bytes[at + 1] } else { 0 };

        if first == b'!' && second == b'=' {
            break;
        } else if first == b'=' {
            break;
        }

        at += 1;
    }

    let stopped_on = if at < message_bytes.len() { message_bytes[at] } else { 0 };
    let second = if at + 1 < message_bytes.len() { message_bytes[at + 1] } else { 0 };

    let index = if stopped_on == b'!' && second == b'=' { 2 } else { 1 };
    let check = if index == 2 { b'!' } else { b'=' };

    if stopped_on != check {
        return None;
    }

    let value = at + index;
    if value < message_bytes.len() && message_bytes[value] != 0 {
        message_bytes[at] = 0;
        Some(KeyValuePair { key: token_start, value, exclusion: index == 2 })
    } else {
        None
    }
}
```

`Option` is `DECISIONS.md` 32 ("nullable pointers become `Option`"). It collapses the two C failure
shapes — `key = NULL, value = NULL` and `key = set, value = NULL` — into one `None`, which is
exact: both callers test `!key || !value` and both print `token.text`, the *original, unmodified*
token, never `key`:

```
daemon_fail(rsp, "invalid key-value pair '%s'\n", token.text);
```

(`src/message.c:2611`, `:2884`). Because `None` is only returned before the NUL is written, the
buffer is unmodified in that case too, so `token.text` still prints the whole `k!=` or `k` text.

`key` and `value` are indices of NUL-terminated C strings inside the buffer. Read them with:

```rust
fn c_string_at(message_bytes: &[u8], start: usize) -> &[u8] {
    let end = message_bytes[start..]
        .iter()
        .position(|byte| *byte == 0)
        .map_or(message_bytes.len(), |offset| start + offset);
    &message_bytes[start..end]
}
```

`string_equals(key, ARGUMENT_RULE_KEY_APP)` becomes
`c_string_at(message_bytes, pair.key) == ARGUMENT_RULE_KEY_APP.as_bytes()`. C's `string_equals`
(`src/misc/helpers.h:254`) is `a && b && strcmp(a, b) == 0`; neither side is ever NULL here, so the
slice comparison is exact.

#### `parse_properties` — `src/message.c:625-650`

Splits a comma list in place, one NUL per comma, OR-ing each matched property into a `u64` flag
word.

```rust
struct Properties {
    token: Token,
    did_parse: bool,
    did_error: bool,
    flags: u64,
}

fn parse_properties(
    response: &mut Response,
    message_bytes: &mut [u8],
    token: Token,
    property_val: &[u64],
    property_str: &[&str],
) -> Properties {
    let mut result = Properties { token, did_parse: false, did_error: false, flags: 0 };

    result.did_parse = token.is_valid() && !token_prefix(token, message_bytes, "--");
    if !result.did_parse {
        return result;
    }

    let mut cursor = 0;
    for i in 0..token.length {
        if i + 1 == token.length {
            let property = c_string_at(message_bytes, token.start + cursor);
            if !parse_property(&mut result, property, property_val, property_str) {
                let reported = &message_bytes[token.start + cursor..token.start + i + 1];
                response.fail_pieces(&[
                    FailurePiece::Text("'"),
                    FailurePiece::BytesStoppingAtFirstNull(reported),
                    FailurePiece::Text("' is not a valid property.\n"),
                ]);
                result.did_error = true;
            }
        } else if message_bytes[token.start + i] == b',' {
            message_bytes[token.start + i] = 0;

            let property = c_string_at(message_bytes, token.start + cursor);
            if !parse_property(&mut result, property, property_val, property_str) {
                let reported = &message_bytes[token.start + cursor..token.start + i + 1];
                response.fail_pieces(&[
                    FailurePiece::Text("'"),
                    FailurePiece::BytesStoppingAtFirstNull(reported),
                    FailurePiece::Text("' is not a valid property.\n"),
                ]);
                result.did_error = true;
            }

            cursor = i + 1;
        }
    }

    result
}
```

Three details that are easy to lose:

* The **last-byte branch is tested first**, so `--windows id,` is one property named `id,` and
  fails; it is not "an empty trailing field".
* The reported length is `i - cursor + 1` in C, one byte *past* the segment when a comma was hit —
  and `%.*s` stops at the NUL that was just written, so the comma is not printed. The Rust piece
  `FailurePiece::BytesStoppingAtFirstNull` (§7.4) reproduces that truncation. This is the only
  `%.*s` in the whole file whose precision runs past a NUL; everywhere else the NUL sits exactly at
  `start + length`.
* `did_parse == false` (the token was invalid, or began with `--`) means the *caller* re-uses
  `properties.token` as the next token instead of pulling a fresh one
  (`src/message.c:2428`, `:2484`, `:2541`). `did_error` is independent and aborts the query with a
  bare `return`.

`parse_property` (`src/message.c:613-623`) is a linear scan, and stays one:

```rust
fn parse_property(
    properties: &mut Properties,
    property: &[u8],
    property_val: &[u64],
    property_str: &[&str],
) -> bool {
    for i in 0..property_str.len() {
        if property == property_str[i].as_bytes() {
            properties.flags |= property_val[i];
            return true;
        }
    }
    false
}
```

The three property tables (`DISPLAY_PROPERTY_LIST` `src/display.h:7-35`, `SPACE_PROPERTY_LIST`
`src/view.h:21-40`, `WINDOW_PROPERTY_LIST` `src/window.h:66-85`) become one `macro_rules!` each per
`DECISIONS.md` 31, generating the flag constants and the `&[&str]` name table side by side. The flag
word stays `u64` because `WINDOW_PROPERTY_IS_GRABBED` is `0x100000000` (`src/window.h:64`).

### 2.7 The `stack.` prefix strip — `src/message.c:1060-1064`

```c
result.token.text   += strlen(ARGUMENT_COMMON_SEL_STACK_PREFIX);
result.token.length -= strlen(ARGUMENT_COMMON_SEL_STACK_PREFIX);
```

In the index model:

```rust
result.token.start += ARGUMENT_COMMON_SEL_STACK_PREFIX.len();
result.token.length -= ARGUMENT_COMMON_SEL_STACK_PREFIX.len();
```

The strip mutates the token that the failure message at `src/message.c:1110` then prints, with the
prefix manually put back by the format string:

```c
daemon_fail(rsp, "value '%s%.*s' is not a valid option for WINDOW_SEL\n",
            ARGUMENT_COMMON_SEL_STACK_PREFIX, result.token.length, result.token.text);
```

Keep both halves. For the bare token `stack.` the stripped length is 0, the arm falls through to
that message, and the printed text is `stack.` — the prefix alone.

---

## 3. Classifiers and `token_to_value`

`DECISIONS.md` 27 sends `strtof` to libc; the integer and hexadecimal classifiers stay hand-written
because they are not libc calls in C either. `DECISIONS.md` 32 turns the `bool` + out-parameter
pairs into `Option`, which is exact: every caller reads the out-parameter only when the function
returned `true`.

### 3.1 `token_is_positive_integer` — `src/message.c:343-356`

Digits only, decimal, accumulated into a **signed** `int` that C lets wrap.
`DECISIONS.md` 8 sets `overflow-checks = false`, and the accumulation is written with explicit
`wrapping_*` anyway so debug and release agree with C:

```rust
fn token_is_positive_integer(token: Token, message_bytes: &[u8]) -> Option<i32> {
    let mut value: i32 = 0;

    for byte in token.bytes(message_bytes) {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.wrapping_mul(10).wrapping_add((byte - b'0') as i32);
    }

    Some(value)
}
```

`token_char_int_table` (`src/message.c:283-296`) is a sparse designated-initializer array reached
only after an explicit ASCII range check. The subtraction above is the same function on the same
domain, so the table is not translated; that is one line in `DEVIATIONS.md`.

Note that an empty token returns `Some(0)` here, mirroring C's `return true` with `*value == 0` for
`length == 0` — but `token_to_value` guards with `token_is_valid`, and the other caller
(`src/message.c:1101`) guards with `token_is_valid(result.token) && ... && index > 0`, so the empty
case is unreachable in both. Keep the guards in the same places rather than adding a length check
here.

### 3.2 `token_is_hexadecimal` — `src/message.c:358-381`

```rust
fn token_is_hexadecimal(token: Token, message_bytes: &[u8]) -> Option<u32> {
    if token.length <= 2 {
        return None;
    }

    let token_bytes = token.bytes(message_bytes);
    if !(token_bytes[0] == b'0' && (token_bytes[1] == b'x' || token_bytes[1] == b'X')) {
        return None;
    }

    let mut value: u32 = 0;
    for byte in &token_bytes[2..] {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return None,
        };
        value = value.wrapping_mul(16).wrapping_add(digit as u32);
    }

    Some(value)
}
```

`length <= 2` rejects `0x`; the exponent letter of a hex float literal (`0x1p3`) is rejected by the
digit match, which is why such a token falls through to `token_is_float` (§3.3) and parses as
`8.0`. That reachability is the whole reason `strtof` cannot be replaced.

### 3.3 `token_is_float` — `src/message.c:383-395`, via `libc::strtof`

`DECISIONS.md` 27: call libc. `str::parse::<f32>()` rejects leading whitespace and hex float
literals that `strtof` accepts, and `strtof` additionally accepts `nan(n-char-seq)`. All three
differences are reachable from the socket.

```rust
fn token_is_float(token: Token, message_bytes: &[u8]) -> Option<f32> {
    let mut end: *mut libc::c_char = std::ptr::null_mut();
    let value = unsafe { libc::strtof(token.as_c_string_pointer(message_bytes), &mut end) };

    if end.is_null() || unsafe { *end } != 0 {
        None
    } else {
        Some(value)
    }
}
```

The `end.is_null()` half of the test never fires (POSIX `strtof` sets `*endptr` to `nptr` when no
conversion happens) and is kept because it costs nothing and keeps the expression the same shape as
the C. Success requires the **whole** C string to have been consumed, which is why the terminating
NUL of §2.2 matters: `strtof` reads past `token.length` and stops at it.

### 3.4 `token_to_value` — `src/message.c:397-416`

The C union is replaced by a real enum per `DECISIONS.md` 31; the ordering of the type chain is
observable and is preserved literally.

```rust
#[derive(Clone, Copy)]
enum TokenType {
    Invalid,
    Unknown,
    Int(i32),
    Float(f32),
    U32(u32),
    String,
}

#[derive(Clone, Copy)]
struct TokenValue {
    token: Token,
    type_of_value: TokenType,
}

fn token_to_value(token: Token, message_bytes: &[u8]) -> TokenValue {
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

    TokenValue { token, type_of_value }
}
```

`TOKEN_TYPE_UNKNOWN` is unreachable in the daemon (`src/message.c:408` can only fail if
`token.text` were NULL, which `token_is_valid` already excluded). The variant is kept because
`DECISIONS.md` 31 keeps enums whose values are observable, and because keeping it makes the Rust
readable against the C; nothing constructs it.

`string_value` in C aliases the numeric union arms and is literally `token.text`. In Rust the
`String` variant carries nothing; every consumer that wants the text uses `value.token` and reads
the bytes from the buffer. The three sites that pass `value.string_value` to a function taking a
`char *` — `display_manager_get_display_for_label` (`src/message.c:772`),
`space_manager_get_space_for_label` (`:853`), `rule_remove_by_label` /
`event_signal_remove` (`:2849`, `:2963`) — become
`c_string_at(message_bytes, value.token.start)`.

**Consequence to keep in mind everywhere below:** because the integer branch is tried first and
accepts digits only, `5` is `Int` and `-5` is `Float`. Negative integers arrive as floats
throughout the command surface, which is why `window --move rel:-100:0` goes through the `%f`
`sscanf` formats and not through an integer path.

---

## 4. The nine `sscanf` sites

`DECISIONS.md` 27: these call libc directly. A hand-rolled splitter gets four things wrong that are
all reachable — trailing junk after the last conversion is ignored by `%d`/`%f`, a `%N[^:]` scanset
must match at least one character, `%5[^:]` truncates, and only the full conversion count is
accepted.

### 4.1 Call shape

The format strings stay exactly where they are, as C string literals with their C names
(`DECISIONS.md` 37):

```rust
const ARGUMENT_CONFIG_EXTERNAL_BAR: &std::ffi::CStr = c"%5[^:]:%d:%d";
const ARGUMENT_SPACE_PADDING: &std::ffi::CStr = c"%255[^:]:%d:%d:%d:%d";
const ARGUMENT_SPACE_GAP: &std::ffi::CStr = c"%255[^:]:%d";
const ARGUMENT_WINDOW_GRID: &std::ffi::CStr = c"%d:%d:%d:%d:%d:%d";
const ARGUMENT_WINDOW_MOVE: &std::ffi::CStr = c"%255[^:]:%f:%f";
const ARGUMENT_WINDOW_RESIZE: &std::ffi::CStr = c"%255[^:]:%f:%f";
const ARGUMENT_WINDOW_RATIO: &std::ffi::CStr = c"%255[^:]:%f";
const ARGUMENT_RULE_VALUE_GRID: &std::ffi::CStr = c"%d:%d:%d:%d:%d:%d";
const SCAN_ONE_FLOAT: &std::ffi::CStr = c"%f";
```

`SCAN_ONE_FLOAT` is the one format that is an inline literal in C (`src/message.c:2718`); it gets a
name so the set is greppable.

The subject pointer is `value.as_c_string_pointer(message_bytes)` for the seven sites that scan a
token, and `c_string_at(...).as_ptr()` for the two inside `parse_rule` that scan a `key=value`
right-hand side. Both are NUL-terminated (§2.2, §2.6).

### 4.2 The nine sites, in source order

| # | C | format | out-parameters | accepted when |
| --- | --- | --- | --- | --- |
| 1 | `src/message.c:1672` | `ARGUMENT_CONFIG_EXTERNAL_BAR` | `[c_char; 6]`, `c_int`, `c_int` | `== 3` |
| 2 | `src/message.c:1972` | `ARGUMENT_SPACE_PADDING` | `[c_char; 512]`, 4 × `c_int` | `== 5` |
| 3 | `src/message.c:1983` | `ARGUMENT_SPACE_GAP` | `[c_char; 512]`, `c_int` | `== 2` |
| 4 | `src/message.c:2220` | `ARGUMENT_WINDOW_GRID` | 6 × `c_int` **into `unsigned` lvalues** | `== 6` |
| 5 | `src/message.c:2232` | `ARGUMENT_WINDOW_MOVE` | `[c_char; 512]`, 2 × `c_float` | `== 3` |
| 6 | `src/message.c:2244` | `ARGUMENT_WINDOW_RESIZE` | `[c_char; 512]`, 2 × `c_float` | `== 3` |
| 7 | `src/message.c:2260` | `ARGUMENT_WINDOW_RATIO` | `[c_char; 512]`, `c_float` | `== 2` |
| 8 | `src/message.c:2708` | `ARGUMENT_RULE_VALUE_GRID` | 6 × `c_int` **into `unsigned` lvalues** | `== 6` |
| 9 | `src/message.c:2718` | `SCAN_ONE_FLOAT` | 1 × `c_float` **into a `float` field** | `== 1` and in `0.0..=1.0` |

Buffer sizes are the C's, not the format's: `char mode[6]` at `src/message.c:1670` and
`char type[MAXLEN]` / `char handle[MAXLEN]` with `MAXLEN == 512` (`src/misc/macros.h:20`) at
`:1971`, `:1982`, `:2231`, `:2243`, `:2259`. Do not shrink them to 256; `%255[^:]` writes at most
256 bytes but the C array is 512 and a translator who trims it has changed a stack layout for no
reason.

### 4.3 The canonical site — one scanset plus two floats

`src/message.c:2230-2241`, `window --move`:

```rust
let mut type_of_move = [0 as libc::c_char; MAXLEN];
let mut x: libc::c_float = 0.0;
let mut y: libc::c_float = 0.0;
let value = message_cursor.get_token();

let converted = unsafe {
    libc::sscanf(
        value.as_c_string_pointer(message_cursor.bytes()),
        ARGUMENT_WINDOW_MOVE.as_ptr(),
        type_of_move.as_mut_ptr(),
        &mut x as *mut libc::c_float,
        &mut y as *mut libc::c_float,
    )
};

if converted == 3 {
    let result = window_manager_move_window_relative(
        window_manager,
        acting_window,
        parse_value_type(&type_of_move),
        x,
        y,
    );
    if result == WindowOpError::InvalidSrcView {
        daemon_fail!(response, "cannot move a managed window.\n");
    }
} else {
    // the unknown-value message of §7.4
}
```

`libc::sscanf` is variadic; every out-pointer is passed as an explicit raw pointer so the argument
types are visible at the call. The `unsafe` block wraps the call only, per `DECISIONS.md` 39.

`parse_value_type` (`src/message.c:472-481`) and `parse_resize_handle` (`:483-506`) take the
scanset buffer, which is a NUL-terminated C string in a fixed array. Give them
`&[libc::c_char; MAXLEN]` and compare against `"abs"` / `"rel"` / the nine handle names by reading
up to the NUL. They return `0` for anything unrecognised, and that `0` is a meaningful value
downstream (no `TYPE_ABS`, no `TYPE_REL`), so `DECISIONS.md` 32 keeps them returning `u8`, not
`Option`.

### 4.4 The empty-value path

When the value token is invalid the cursor has parked and `value.as_c_string_pointer` points at a
NUL. `sscanf("")` returns `EOF` (`-1`), which fails every `== N` test, so:

* `external_bar` with no value falls to `fprintf(rsp, "%s:%d:%d\n", ...)` at `src/message.c:1691`
  and **prints the current setting** — it is not an error;
* the other eight fall to their `daemon_fail` branch.

Reproduce by keeping the `if converted == N { ... } else { ... }` shape and never special-casing an
empty subject.

### 4.5 The two `unsigned`-lvalue sites

`src/message.c:2220` declares `unsigned r, c, x, y, w, h` and passes `&r` to a `%d` conversion;
`src/message.c:2708` passes `&rule->effects.grid[0..5]`, also `unsigned`. The widths match, so the
`int` that `sscanf` stores is reinterpreted as `unsigned`.

In Rust: declare six `libc::c_int` locals, scan into those, and cast `as u32` when handing them on.
A negative component therefore keeps its two's-complement value instead of failing, exactly as
today.

```rust
let mut rows: libc::c_int = 0;
let mut columns: libc::c_int = 0;
let mut x: libc::c_int = 0;
let mut y: libc::c_int = 0;
let mut width: libc::c_int = 0;
let mut height: libc::c_int = 0;

let converted = unsafe {
    libc::sscanf(
        value.as_c_string_pointer(message_cursor.bytes()),
        ARGUMENT_WINDOW_GRID.as_ptr(),
        &mut rows as *mut libc::c_int,
        &mut columns as *mut libc::c_int,
        &mut x as *mut libc::c_int,
        &mut y as *mut libc::c_int,
        &mut width as *mut libc::c_int,
        &mut height as *mut libc::c_int,
    )
};

if converted == 6 {
    let result = window_manager_apply_grid(
        space_manager,
        window_manager,
        acting_window,
        rows as u32,
        columns as u32,
        x as u32,
        y as u32,
        width as u32,
        height as u32,
    );
    if result == WindowOpError::InvalidSrcView {
        daemon_fail!(response, "cannot apply grid layout to a managed window.\n");
    }
}
```

At site 8 the C scans straight into `rule->effects.grid[i]`, so a partial conversion leaves partly
written fields behind. Scan into six locals and copy them into `rule.effects.grid` only when
`converted == 6`. That is observationally identical: a short conversion sets `did_parse = false`,
and a rule with `did_parse == false` is always dropped without ever being serialised
(`src/message.c:2823`, `:2837`).

Site 9 is the same shape: scan into a `libc::c_float` local, and assign
`rule.effects.opacity` plus `RULE_OPACITY` only when `converted == 1 && (0.0..=1.0).contains(&opacity)`,
matching `in_range_ii` (`src/misc/macros.h:12`).

---

## 5. The tri-state selector

`struct selector` (`src/message.c:652-663`) is a token plus a `bool did_parse` plus a union whose
tag is implicit in which of the four producers built it. The union is zeroed by the designated
initializer `{ .token = get_token(message), .did_parse = true }`, so a selector that *parsed* but
failed to *resolve* keeps `did_parse == true` with a zero arm. Three states, and all three are
distinguished by real control flow.

### 5.1 The type

```rust
enum SelectorOutcome<Target> {
    NotASelector,
    ParsedButUnresolved,
    Resolved(Target),
}

struct Selector<Target> {
    token: Token,
    outcome: SelectorOutcome<Target>,
}

impl<Target: Copy> Selector<Target> {
    fn did_parse(&self) -> bool {
        !matches!(self.outcome, SelectorOutcome::NotASelector)
    }

    fn resolved(&self) -> Option<Target> {
        match self.outcome {
            SelectorOutcome::Resolved(value) => Some(value),
            _ => None,
        }
    }
}
```

The token lives outside the enum because every one of the three states needs it: `NotASelector`
hands it back as the next command token, `ParsedButUnresolved` and `Resolved` are still tested with
`token_is_valid` at the retargeting sites of §5.5.

Four instantiations, matching the four producers:

| producer | C | `Target` |
| --- | --- | --- |
| `parse_display_selector` | `src/message.c:665` | `u32` display id |
| `parse_space_selector` | `src/message.c:790` | `u64` space id |
| `parse_window_selector` | `src/message.c:871` | `u32` window id |
| `parse_insert_selector` | `src/message.c:1130` | `i32` direction |

`parse_window_selector` resolves to a **window id**, not a reference, per `DECISIONS.md` 14: the
selector outlives calls that mutate the window manager (`src/message.c:2078`, `:2094`, `:2112`), so
it must be a handle looked up at each use.

`parse_insert_selector` never produces `ParsedButUnresolved`: its five arms
(`north`/`east`/`south`/`west`/`stack` → `360`/`90`/`180`/`270`/`111`,
`src/misc/macros.h:26-30`) either resolve or set `did_parse = false`.

### 5.2 The construction rule

Every producer starts with

```rust
let mut result = Selector { token: message_cursor.get_token(), outcome: SelectorOutcome::ParsedButUnresolved };
```

which is the C initializer exactly: `did_parse = true`, arm zeroed. From there:

* **`Resolved` is constructed only with a non-zero value.** Every `result.did = did;` in C is
  guarded by `if (did)` at the same place — except `ARGUMENT_COMMON_SEL_RECENT` at
  `src/message.c:761` (`result.did = g_display_manager.last_display_id;`) and `:842`
  (`result.sid = g_space_manager.last_space_id;`), which assign unconditionally. Those two arms
  therefore get an explicit non-zero test in Rust so a zero `last_*_id` lands in
  `ParsedButUnresolved`, which is what the call sites' `did_parse && arm` test already meant.
* **`NotASelector` is set in exactly four places per producer**: the `TOKEN_TYPE_STRING` fallthrough
  (label lookup failed), the `TOKEN_TYPE_INVALID` arm, the catch-all arm for `Float`/`U32` values,
  and — in `parse_window_selector` only — the `stack.` sub-arm fallthrough at
  `src/message.c:1110`.
* **Failing to resolve does not clear the token and does not stop the parse.** The failure message
  is emitted inside the producer; the caller sees `ParsedButUnresolved` and takes its own branch.

The `optional` parameter suppresses the message for the `TOKEN_TYPE_INVALID` case **only**
(`src/message.c:778`, `:864`, `:1120`); every other failure reports regardless. Keep it a plain
`bool` parameter named `optional`.

### 5.3 Shape A — the probing head

`src/message.c:1705-1713` (display), `:1764-1772` (space), `:2052-2060` (window). The first
selector of a domain is parsed with a **silent** response (§7.2) so that a leading command token is
not reported as a bad selector:

```rust
let mut acting_display_id = display_manager_active_display_id(display_manager);
let selector = {
    let mut silent_response = Response::silent();
    parse_display_selector(&mut silent_response, &mut message_cursor, acting_display_id, true, display_manager)
};

let command;
if selector.did_parse() {
    acting_display_id = selector.resolved().unwrap_or(0);
    command = message_cursor.get_token();
} else {
    command = selector.token;
}
```

`unwrap_or(0)` is the whole point of the tri-state: `ParsedButUnresolved` **clears** the acting
target rather than leaving the active one in place, and the next statement
(`if (!acting_did)` at `src/message.c:1715`, `if (!acting_sid)` at `:1775`) turns that into
`could not locate the display to act on!` / `could not locate the space to act on!` followed by a
bare return. For windows the acting target is `Option<u32>` and the equivalent is
`acting_window_id = selector.resolved();` with no up-front check — `handle_domain_window` checks
per iteration instead (§6.2).

### 5.4 Shape B — a mandatory selector

The common arm, e.g. `src/message.c:1781-1783`:

```rust
let selector = parse_space_selector(response, &mut message_cursor, acting_space_id, false, space_manager);
if let Some(space_id) = selector.resolved() {
    ...
}
```

`NotASelector` and `ParsedButUnresolved` collapse here, and that is correct: the producer already
wrote the failure message for both, and the C does nothing in either case (there is no `else`).

### 5.5 Shape C — an optional selector that retargets or abandons the message

```rust
let selector = parse_window_selector(response, &mut message_cursor, acting_window_id, true, window_manager);

if selector.token.is_valid() {
    match selector.resolved() {
        Some(window_id) => acting_window_id = Some(window_id),
        None => return,
    }
}
```

This is the shape that needs all three states: `token.is_valid()` asks "was there a token at all",
and only then does `Resolved` vs the other two decide between retargeting and abandoning the rest
of the message. Sites:

| C | domain | effect of `Resolved` |
| --- | --- | --- |
| `src/message.c:1859-1866` | `space --create` | `acting_sid = display_space_id(selector.did)` |
| `src/message.c:1881-1888` | `space --destroy` | `acting_sid = selector.sid` |
| `src/message.c:2075-2082` | `window --focus` | `acting_window = selector.window` |
| `src/message.c:2091-2098` | `window --close` | `acting_window = selector.window` |
| `src/message.c:2109-2116` | `window --minimize` | `acting_window = selector.window` |
| `src/message.c:2368-2377` | `window --raise` | `selector_wid = selector.window->id` |
| `src/message.c:2383-2392` | `window --lower` | `selector_wid = selector.window->id` |
| `src/message.c:2431-2437`, `:2446-2451`, `:2459-2465` | `query --displays` narrowing | narrows the query target |
| `src/message.c:2488-2493`, `:2502-2508`, `:2517-2523` | `query --spaces` narrowing | narrows the query target |
| `src/message.c:2545-2550`, `:2557-2563`, `:2570-2576` | `query --windows` narrowing | narrows the query target |

`window --raise` / `--lower` differ from the rest: they do **not** retarget; they capture
`selector.window->id` into a separate `selector_wid` that defaults to `0` and is passed to
`scripting_addition_order_window` as the reference window. In Rust,
`let mut selector_window_id: u32 = 0;` and `Some(window_id) => selector_window_id = window_id`.
Keep the `0` — it is the "no reference window" sentinel the scripting addition reads
(`DECISIONS.md` 32).

---

## 6. Domain-handler control flow

### 6.1 The dispatch loops

Five loops, all of them `for (; token_is_valid(command); command = get_token(&message))`:
`src/message.c:1169` (config, 33 arms), `:1779` (space, 16 arms), `:2062` (window, 20 arms),
`:2604` (`parse_rule`), `:2877` (`signal --add`). In Rust:

```rust
let mut command = ...;
while command.is_valid() {
    ...
    command = message_cursor.get_token();
}
```

A `while let Some(..)` over an iterator is wrong here, because the loop body re-enters the cursor
(each arm pulls its own value tokens) and because several arms `return` out of the whole handler.
Write the loop as above, with the re-assignment at the bottom and every `continue` in
`parse_rule` (`src/message.c:2613`) reaching it.

The arms themselves are `if token_equals(...) { } else if ... { } else { daemon_fail }` chains, not
a `match`, because the order is observable in two places:

* `parse_window_selector`: the `stack.` **prefix** test at `src/message.c:1060` comes after every
  exact-match arm, so a window labelled `stack.foo` cannot shadow it.
* `window --toggle`: the final arm at `src/message.c:2332` is a fallthrough that treats any
  unrecognised value as a scratchpad label and only fails if
  `window_manager_toggle_scratchpad_window_by_label` returns `false`.

A byte-slice `match` with a `_` arm reproduces both, and is the preferred form wherever the arms
are all exact matches; use an `if`/`else if` chain where a prefix test or a fallthrough call sits in
the chain.

### 6.2 The per-iteration acting-window guard

`src/message.c:2063-2071`, at the top of every iteration of the window loop:

```rust
if acting_window_id.is_none()
    && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_FOCUS)
    && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_CLOSE)
    && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_MINIMIZE)
    && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_DEMINIMIZE)
    && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_TOGGLE)
{
    daemon_fail!(response, "could not locate the window to act on!\n");
    return;
}
```

It is re-evaluated every iteration precisely because an earlier arm can have cleared or changed
`acting_window_id`.

### 6.3 Mid-loop retargeting

The acting target is a loop-carried variable, not a constant. Rebinding sites:

* `acting_sid`: `src/message.c:1768` (head), `:1863` (`--create`), `:1884` (`--destroy`)
* `acting_window`: `src/message.c:2056` (head), `:2078` (`--focus`), `:2094` (`--close`),
  `:2112` (`--minimize`)

So `yabai -m window --focus next --close` closes the window that `--focus next` selected, not the
originally focused one, and `yabai -m space --create --destroy` destroys the space that `--create`
left the cursor on. Both are user-visible and both are regression tests waiting to be written.

### 6.4 Early returns that abandon the rest of the message

Nine `return` statements inside the two loops abandon every remaining command **silently**, with no
message of their own (the selector parser already wrote one, or the selector was simply absent):
`src/message.c:1865`, `:1886`, `:2080`, `:2096`, `:2114`, `:2375`, `:2390`, plus the two guard
returns at `:1776` and `:2070`. `handle_domain_query` adds twelve more (`:2426`, `:2437`, `:2451`,
`:2465`, `:2482`, `:2493`, `:2508`, `:2523`, `:2539`, `:2550`, `:2563`, `:2576`), three of which
(`:2426`, `:2482`, `:2539`) are the `properties.did_error` abort.

Translate them as `return`, never as `break`. A `break` would run any code after the loop; there is
none today, but the distinction is what makes the function's shape reviewable against the C.

### 6.5 `handle_domain_display` has no loop

`src/message.c:1700-1757` parses at most one command (`--focus`, `--space`, `--label`) and then
returns. `yabai -m display --focus 1 --label main` silently ignores `--label main`. Do not
"fix" it into a loop.

### 6.6 `handle_message` — `src/message.c:2979-3000`

```rust
pub fn handle_message(
    response: &mut Response,
    message: &mut [u8],
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) {
    let mut message_cursor = MessageCursor::new(message);
    let domain = message_cursor.get_token();

    if token_equals(domain, message_cursor.bytes(), DOMAIN_CONFIG) {
        handle_domain_config(response, domain, &mut message_cursor, display_manager, space_manager, window_manager);
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_DISPLAY) {
        handle_domain_display(response, domain, &mut message_cursor, display_manager, space_manager);
    } else if ...
    } else {
        response.fail_pieces(&[
            FailurePiece::Text("unknown domain '"),
            FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
            FailurePiece::Text("'\n"),
        ]);
    }
}
```

Per `DECISIONS.md` 13 the managers are explicit `&mut` parameters in the C parameter order with the
ones C reached through a global appended; the exact per-function sets come from the precomputed
call graph, and `handle_message`'s set is the union of the seven handlers'. No function in this
subsystem reaches a manager through a global. `g_verbose` (`src/message.c:1173-1177`) and the
event-tap half of `g_mouse_state` (`:1623-1664`) are the two exceptions, and they are statics of
atomics by `DECISIONS.md` 18 and 23, written with `Ordering::Relaxed`.

---

## 7. The `Response` type

`DECISIONS.md` 28: *"Responses are written as bytes through one `Response` type that owns the
failure prefix byte and the 'no response wanted' case."* One type replaces both `FILE *rsp` and the
`NULL` that `src/message.c:1706`, `:1765` and `:2053` pass to a probing selector parse.

### 7.1 The type in full

```rust
pub struct Response {
    stream: Option<std::io::BufWriter<std::os::unix::net::UnixStream>>,
}

impl Response {
    pub fn to_client(stream: std::os::unix::net::UnixStream) -> Response {
        Response { stream: Some(std::io::BufWriter::new(stream)) }
    }

    pub fn silent() -> Response {
        Response { stream: None }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        if let Some(stream) = self.stream.as_mut() {
            let _ = std::io::Write::write_all(stream, bytes);
        }
    }

    pub fn write_bytes_stopping_at_first_null(&mut self, bytes: &[u8]) {
        let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
        self.write_bytes(&bytes[..end]);
    }

    pub fn write(&mut self, arguments: std::fmt::Arguments) {
        if let Some(stream) = self.stream.as_mut() {
            let _ = std::io::Write::write_fmt(stream, arguments);
        }
    }

    pub fn begin_failure(&mut self) {
        self.write_bytes(FAILURE_MESSAGE);
    }

    pub fn fail(&mut self, arguments: std::fmt::Arguments) {
        if self.stream.is_none() {
            return;
        }
        self.begin_failure();
        self.write(arguments);
    }

    pub fn fail_pieces(&mut self, pieces: &[FailurePiece]) {
        if self.stream.is_none() {
            return;
        }
        self.begin_failure();
        for piece in pieces {
            match piece {
                FailurePiece::Text(text) => self.write_bytes(text.as_bytes()),
                FailurePiece::Bytes(bytes) => self.write_bytes(bytes),
                FailurePiece::BytesStoppingAtFirstNull(bytes) => {
                    self.write_bytes_stopping_at_first_null(bytes)
                }
            }
        }
    }
}

pub enum FailurePiece<'message> {
    Text(&'message str),
    Bytes(&'message [u8]),
    BytesStoppingAtFirstNull(&'message [u8]),
}

const FAILURE_MESSAGE: &[u8] = b"\x07";

impl Drop for Response {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.as_mut() {
            let _ = std::io::Write::flush(stream);
        }
    }
}

macro_rules! daemon_fail {
    ($response:expr, $($argument:tt)*) => {
        $response.fail(format_args!($($argument)*))
    };
}
```

### 7.2 The silent case

`daemon_fail` early-returns on `rsp == NULL` (`src/message.c:420`). That is how the three probing
parses at `src/message.c:1706`, `:1765` and `:2053` keep a leading command token from being
reported as a bad selector. `Response::silent()` reproduces it, and `Response` is the only type any
parse function takes, so no call site can accidentally lose the suppression.

Checked: nothing reachable from a probing parse writes to the response except through
`daemon_fail`. `parse_display_selector`, `parse_space_selector` and `parse_window_selector` contain
no `fprintf` at all, so a silent `Response` is never asked to do anything else.

### 7.3 The failure prefix, exactly once

`FAILURE_MESSAGE` is the single BEL byte `"\x07"` (`src/misc/macros.h:18`), written *before* the
text by `fprintf(rsp, FAILURE_MESSAGE)` at `src/message.c:423`, once per `daemon_fail` call. A
message that trips several `daemon_fail` calls emits several BELs mid-stream — `parse_rule` does
exactly that, because it `continue`s after each error (`src/message.c:2613`) and reports the
missing filter and the unsupported exclusion at the end (`:2793`, `:2798`).

Do **not** hoist the prefix into the stream, do not emit it once per response, and do not emit it
from `write`. `begin_failure` is called from exactly two places, both of them inside `Response`.

The client tests only `rsp[0]` of each `read()` chunk and strips exactly one byte
(`src/yabai.c:111-114`), so a second BEL in the middle of a chunk reaches the user's terminal as a
literal bell character. That is current behaviour.

### 7.4 The `%.*s` messages

Roughly sixty `daemon_fail` calls interpolate token text with `%.*s`. Those must be written as raw
bytes, never through `str` or `from_utf8_lossy`, because a window title or label with invalid UTF-8
would gain replacement characters in the error text. That is what `fail_pieces` is for. The single
most repeated message in the file, present in every domain:

```rust
response.fail_pieces(&[
    FailurePiece::Text("unknown value '"),
    FailurePiece::Bytes(value.bytes(message_cursor.bytes())),
    FailurePiece::Text("' given to command '"),
    FailurePiece::Bytes(command.bytes(message_cursor.bytes())),
    FailurePiece::Text("' for domain '"),
    FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
    FailurePiece::Text("'\n"),
]);
```

`BytesStoppingAtFirstNull` exists for one caller, `parse_properties` (§2.6), whose reported length
deliberately runs one byte past the segment onto the NUL it just wrote. Every other `%.*s` has its
NUL exactly at `start + length`, so `Bytes` is correct for them.

Messages with no token interpolation use the macro: `daemon_fail!(response, "cannot focus an
already focused space.\n")`. Messages that interpolate only numbers also use the macro, e.g.
`daemon_fail!(response, "could not close window with id '{}'.\n", acting_window_id)` for
`src/message.c:2104`, where the C `%d` prints a `uint32_t` and the Rust therefore writes
`acting_window_id as i32` (`DECISIONS.md` 29).

### 7.5 Construction, flush and close

`EVENT_HANDLER(DAEMON_MESSAGE)` (`src/event_loop.c:1613-1644`):

```c
if ((bytes_read == bytes_to_read) && (rsp = fdopen(param1, "w"))) {
    debug_message(__FUNCTION__, message);
    handle_message(rsp, message);

    fflush(rsp);
    fclose(rsp);

    return;
}

socket_close(param1);
```

In Rust the accepted fd arrives as an owned `UnixStream` in the event payload
(`DECISIONS.md` 19), so:

* **Success path** — build `Response::to_client(stream)`, call `handle_message`, drop it. `Drop`
  flushes and then closes by dropping the `BufWriter<UnixStream>`. There is **no** `shutdown()`
  call: `fclose` does not shut a socket down, it only closes, and the close is what ends the
  client's read loop.
* **Failure path** — the header read short, or the payload read short. C calls `socket_close`
  (`src/misc/helpers.h:198`), which is `shutdown(SHUT_RDWR)` then `close`. In Rust:
  `let _ = stream.shutdown(std::net::Shutdown::Both);` then drop. No `Response` is built.

`fdopen` failing has no Rust counterpart; the fd is already owned.

The buffer: C's `fdopen` stream is stdio-buffered and flushed once at `fclose` for any response
smaller than the buffer. `BufWriter::new` (8 KiB) is at least as large, so every response that can
contain a BEL — the config and command chains, all well under a kilobyte — still reaches the client
as a single write, exactly as today. The only responses that exceed the buffer are query JSON,
which never contains a BEL. Do not use an unbuffered writer: that would split a failure message
across reads and change which bytes the client routes to stderr.

### 7.6 Errors are swallowed

`DECISIONS.md` 32: *"`Result` appears only at `io::Write`."* Every C `fprintf` return is ignored, so
every Rust write is `let _ = ...`. Never `unwrap()`: a client that hangs up mid-response would take
the daemon down through the panic hook of `DECISIONS.md` 7.

### 7.7 `daemon_deprecated` is not translated

`src/message.c:429-438` is `__unused` and has no caller. `DECISIONS.md` 5 removes dead code; this is
named there. One line in `DEVIATIONS.md`; no `#[allow(dead_code)]` stub.

---

## 8. The `COMMAND_` / `ARGUMENT_` string constants

`src/message.c:14-251` is the CLI grammar, 150-odd `#define`s in eight banner-delimited groups. They
keep their C names (`DECISIONS.md` 37) and stay in one place so the grammar is greppable.

```rust
const DOMAIN_CONFIG: &str = "config";
const DOMAIN_DISPLAY: &str = "display";
const DOMAIN_SPACE: &str = "space";
const DOMAIN_WINDOW: &str = "window";
const DOMAIN_QUERY: &str = "query";
const DOMAIN_RULE: &str = "rule";
const DOMAIN_SIGNAL: &str = "signal";

const COMMAND_WINDOW_FOCUS: &str = "--focus";
const ARGUMENT_COMMON_SEL_STACK_PREFIX: &str = "stack.";
```

Rules for the whole table:

* **`&'static str` for every keyword.** Comparison against token bytes goes through
  `candidate.as_bytes()`, which is byte-exact — no UTF-8 validation of the incoming token, no
  locale. Every keyword is ASCII.
* **Three exceptions, and only three.** `ARGUMENT_RULE_VALUE_SPACE` is a character
  (`src/message.c:215`) and becomes `const ARGUMENT_RULE_VALUE_SPACE: u8 = b'^';`. The nine
  `sscanf` formats become `&CStr` (§4.1) — they are `ARGUMENT_CONFIG_EXTERNAL_BAR`,
  `ARGUMENT_SPACE_PADDING`, `ARGUMENT_SPACE_GAP`, `ARGUMENT_WINDOW_GRID`, `ARGUMENT_WINDOW_MOVE`,
  `ARGUMENT_WINDOW_RESIZE`, `ARGUMENT_WINDOW_RATIO`, `ARGUMENT_RULE_VALUE_GRID`. `MAXLEN` stays
  `const MAXLEN: usize = 512;`.
* **The three reserved-identifier tables** (`src/message.c:515-552`) become
  `const RESERVED_DISPLAY_IDENTIFIERS: [&str; 10]`, `[&str; 6]`, `[&str; 11]`, built out of the
  same `ARGUMENT_*` constants so a rename cannot desynchronise them. They are read two different
  ways in C — `token_equals` against a token at `src/message.c:571`, `:579`, `:587`, and
  `string_equals` against a `char *` value at `:2623` — and both become a byte-slice comparison.
  Compare the **value** slice at the `parse_rule` site, not the whole `key=value` token.
* **The banner comments** (`src/message.c:22`, `:90`, `:92`, `:96`, `:98`, `:128`, `:130`, `:180`,
  `:182`, `:190`, `:192`, `:217`, `:219`, `:233`, `:235`, `:252`) are comments present in the C
  source, so `DECISIONS.md` 38 carries them over verbatim while the constants stay in one file.
* **Enum-to-string tables** read when a config value is omitted (`bool_str`
  `src/misc/helpers.h:173`, `ffm_mode_str`, `display_arrangement_order_str`,
  `window_origin_mode_str`, `window_node_child_str`, `window_insertion_point_str`,
  `animation_easing_type_str`, `purify_mode_str`, `view_type_str`, `window_node_split_str`,
  `auto_balance_str`, `mouse_mod_str`, `mouse_mode_str`, `external_bar_mode_str`) become a
  `fn as_str(self) -> &'static str` on the enum, except `layer_str`
  (`src/misc/helpers.h:175-181`), which stays an indexed array — see §9.6.

---

## 9. JSON serialisation

`DECISIONS.md` 29: *"JSON is written with hand-rolled format strings, never a serialiser. Floats
are cast to `f64` before formatting, `%d` of a `u32` prints `as i32`, `%lld` of a `u64` prints
`as i64`, a NULL `%s` prints `(null)`."*

Every format string below is a literal transcription of the C. Braces double in Rust
(`"{{\n"` for `"{\n"`). `%.4f` and `{:.4}` agree for every finite value, and no field here can be
infinite or NaN.

### 9.1 Primitives

```rust
fn json_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn json_optional_bool(value: i32) -> &'static str {
    if value == 0 { return "null"; }
    if value == 1 { return "true"; }
    "false"
}
```

(`src/misc/helpers.h:225-236`.) Both emit **unquoted** tokens into fields whose format strings carry
no quotes. Getting a quote wrong here breaks every downstream `jq` script.

`ts_string_escape` (`src/misc/helpers.h:259-319`) returns `NULL` when nothing needed escaping, and
every caller falls back with `escaped ? escaped : raw ? raw : ""`. In Rust:

```rust
fn json_escape(text: &str) -> std::borrow::Cow<'_, str>
```

`Cow::Borrowed` is the C `NULL` return plus the caller's fallback, collapsed. Operate on **bytes**,
not `char`s:

* `"` `\` `\x08` `\x0c` `\n` `\r` `\t` become the two-character forms `\"` `\\` `\b` `\f` `\n`
  `\r` `\t`;
* every other byte `<= 0x1f` becomes `\u%04x` with **lowercase** hex, six bytes;
* every byte `>= 0x20` passes through unchanged, including all of UTF-8's continuation bytes. The C
  condition `*cursor >= 0x00 && *cursor <= 0x1f` on a **signed** `char` makes `0x80..=0xff`
  negative and therefore unescaped; over `u8` the `>= 0x00` half is vacuous and `c <= 0x1f` alone
  gives the identical result. A `char`-based implementation would escape different things.

Escaping is applied to: window `app` and `title` (`src/window.c:475-489`); rule `app`, `title`,
`role`, `subrole` (`src/rule.c:13-16`); signal `app`, `title`, `action` (`src/event_signal.c:408-410`).
It is **not** applied to `label`, `scratchpad`, `uuid`, or to `role`/`subrole` in
`window_serialize` (`src/window.c:507-519` prints `window_role_ts` raw). That inconsistency is the
contract; do not regularise it.

### 9.2 The property-masked object shape

`window_serialize`, `view_serialize` and `display_serialize` share one shape:

```rust
let flags = if flags.is_empty() { WindowPropertyFlags::ALL } else { flags };

let mut did_output = false;
response.write(format_args!("{{\n"));

if flags.contains(WindowPropertyFlags::ID) {
    response.write(format_args!("\t\"id\":{}", window.id as i32));
    did_output = true;
}

if flags.contains(WindowPropertyFlags::PID) {
    if did_output { response.write(format_args!(",\n")); }
    response.write(format_args!("\t\"pid\":{}", window.application.process_id));
    did_output = true;
}

...

response.write(format_args!("\n}}"));
```

`flags == 0x0 → flags |= ~flags` (`src/window.c:413`, `src/view.c:864`, `src/display.c:24`) means
"no mask selects everything"; `WindowPropertyFlags::ALL` is `u64::MAX`, not the OR of the defined
bits. The property flag words are newtypes over `u64` with associated constants
(`DECISIONS.md` 11 and 31); the separator is `",\n"` emitted **before** each key after the first,
never after.

Note that `\n}` closes the object with **no** trailing newline. The newline is added by the caller
(`fprintf(rsp, "\n")` at `src/message.c:2442`, `:2456`, `:2471`, `:2582`, `src/space_manager.c:21`) or is
part of the array wrapper (§9.8).

### 9.3 Window object — `window_serialize`, `src/window.c:409-712`

Keys in `WINDOW_PROPERTY_LIST` order (`src/window.h:31-64`), each preceded by `",\n"` when
`did_output`:

| key | C fragment | Rust `format!` | value |
| --- | --- | --- | --- |
| `id` | `\t"id":%d` | `"\t\"id\":{}"` | `window.id as i32` (`u32`) |
| `pid` | `\t"pid":%d` | `"\t\"pid\":{}"` | `window.application.process_id` (`i32`) |
| `app` | `\t"app":"%s"` | `"\t\"app\":\"{}\""` | `json_escape(&application.name)` |
| `title` | `\t"title":"%s"` | `"\t\"title\":\"{}\""` | `json_escape(&window_title(window))` |
| `scratchpad` | `\t"scratchpad":"%s"` | `"\t\"scratchpad\":\"{}\""` | `window.scratchpad.as_deref().unwrap_or("")` |
| `frame` | `\t"frame":{\n\t\t"x":%.4f,\n\t\t"y":%.4f,\n\t\t"w":%.4f,\n\t\t"h":%.4f\n\t}` | `"\t\"frame\":{{\n\t\t\"x\":{:.4},\n\t\t\"y\":{:.4},\n\t\t\"w\":{:.4},\n\t\t\"h\":{:.4}\n\t}}"` | `window.frame` is `CGRect`, i.e. four `f64` already |
| `role` | `\t"role":"%s"` | `"\t\"role\":\"{}\""` | `window_role(window)` **unescaped** |
| `subrole` | `\t"subrole":"%s"` | `"\t\"subrole\":\"{}\""` | `window_subrole(window)` **unescaped** |
| `root-window` | `\t"root-window":%s` | `"\t\"root-window\":{}"` | `json_bool(window.is_root)` |
| `display` | `\t"display":%d` | `"\t\"display\":{}"` | `i32` |
| `space` | `\t"space":%d` | `"\t\"space\":{}"` | `i32` |
| `level` | `\t"level":%d` | `"\t\"level\":{}"` | `i32` |
| `sub-level` | `\t"sub-level":%d` | `"\t\"sub-level\":{}"` | `i32` |
| `layer` | `\t"layer":"%s"` | `"\t\"layer\":\"{}\""` | `window_layer(level)` |
| `sub-layer` | `\t"sub-layer":"%s"` | `"\t\"sub-layer\":\"{}\""` | `window_layer(sub_level)` |
| `opacity` | `\t"opacity":%.4f` | `"\t\"opacity\":{:.4}"` | `window_opacity(window.id) as f64` (`f32`) |
| `split-type` | `\t"split-type":"%s"` | `"\t\"split-type\":\"{}\""` | `window_node_split_str[node && node.parent ? parent.split : 0]` |
| `split-child` | `\t"split-child":"%s"` | `"\t\"split-child\":\"{}\""` | `window_node_child_str[...]` |
| `stack-index` | `\t"stack-index":%d` | `"\t\"stack-index\":{}"` | `i32` |
| `can-move` | `\t"can-move":%s` | | `json_bool(...)` |
| `can-resize` | `\t"can-resize":%s` | | `json_bool(...)` |
| `has-focus` | `\t"has-focus":%s` | | `json_bool(window.id == window_manager.focused_window_id)` |
| `has-shadow` | `\t"has-shadow":%s` | | `json_bool(...)` |
| `has-parent-zoom` | `\t"has-parent-zoom":%s` | | `json_bool(...)` |
| `has-fullscreen-zoom` | `\t"has-fullscreen-zoom":%s` | | `json_bool(...)` |
| `has-ax-reference` | `\t"has-ax-reference":%s` | | `json_bool(true)` — always `true` |
| `is-native-fullscreen` | `\t"is-native-fullscreen":%s` | | `json_bool(...)` |
| `is-visible` | `\t"is-visible":%s` | | `json_bool(...)` |
| `is-minimized` | `\t"is-minimized":%s` | | `json_bool(...)` |
| `is-hidden` | `\t"is-hidden":%s` | | `json_bool(...)` |
| `is-floating` | `\t"is-floating":%s` | | `json_bool(...)` |
| `is-sticky` | `\t"is-sticky":%s` | | `json_bool(...)` |
| `is-grabbed` | `\t"is-grabbed":%s` | | `json_bool(...)` — **does not set `did_output`** |

`is-grabbed` is the last key, so its omission of `did_output = true` (`src/window.c:703-708`) is not
observable today — but write it the same way, because a new key appended after it would be.

`window_layer` (`src/window.c:113-119`) compares against the three process-wide window levels and
returns the string `"unknown"` for anything else. Those levels are `OnceLock` statics
(`DECISIONS.md` 18), so the function is a free function, not a method.

`window_nonax_serialize` (`src/window.c:121-406`) emits **the same keys in the same order** with
placeholders for what it cannot know: `"scratchpad":""`, `"role":""`, `"subrole":""`,
`"split-type"` = `"none"`, `"split-child"` = `"none"`, `"stack-index":0`, and `false` for
`can-move`, `can-resize`, `has-focus`, `has-parent-zoom`, `has-fullscreen-zoom`,
`has-ax-reference`, `is-visible`, `is-minimized`, `is-hidden`, `is-floating`, `is-grabbed`. It is
reached from `window_manager_query_windows_for_spaces` (`src/window_manager.c:48`) for every window
id the daemon has no `struct window` for. Keep the two functions adjacent so the key lists stay in
step.

### 9.4 Space object — `view_serialize`, `src/view.c:860-964`

| key | C fragment | value |
| --- | --- | --- |
| `id` | `\t"id":%lld` | `view.space_id as i64` (`u64`) |
| `uuid` | `\t"uuid":"%s"` | `uuid.unwrap_or("<unknown>")`, **unescaped** |
| `index` | `\t"index":%d` | `i32` |
| `label` | `\t"label":"%s"` | `space_label.unwrap_or("")`, **unescaped** |
| `type` | `\t"type":"%s"` | `view_type_str[view.layout]` |
| `display` | `\t"display":%d` | `i32` |
| `windows` | `\t"windows":[` then `%d, ` for all but the last and `%d` for the last, then `]` | `window_id as i32` |
| `first-window` | `\t"first-window":%d` | `first_leaf.window_order[0] as i32`, or `0` |
| `last-window` | `\t"last-window":%d` | same |
| `has-focus` | `\t"has-focus":%s` | `json_bool(...)` |
| `is-visible` | `\t"is-visible":%s` | `json_bool(...)` |
| `is-native-fullscreen` | `\t"is-native-fullscreen":%s` | `json_bool(...)` — **does not set `did_output`** |

**The `windows` separator is `", "` — comma *and space*** (`src/view.c:920`). An empty window list
prints `[]`. The loop is:

```rust
response.write(format_args!("\t\"windows\":["));
for i in 0..window_count {
    if i < window_count - 1 {
        response.write(format_args!("{}, ", window_list[i] as i32));
    } else {
        response.write(format_args!("{}", window_list[i] as i32));
    }
}
response.write(format_args!("]"));
```

### 9.5 Display object — `display_serialize`, `src/display.c:20-99`

| key | C fragment | value |
| --- | --- | --- |
| `id` | `\t"id":%d` | `display_id as i32` (`u32`) |
| `uuid` | `\t"uuid":"%s"` | `uuid.unwrap_or("<unknown>")`, **unescaped** |
| `index` | `\t"index":%d` | `i32` |
| `label` | `\t"label":"%s"` | `display_label.unwrap_or("")`, **unescaped** |
| `frame` | same nested object as the window frame | `CGDisplayBounds(display_id)`, four `f64` |
| `spaces` | `\t"spaces":[` then `%d, ` / `%d`, then `]` | `first_mission_control_index + i` (`i32`) |
| `has-focus` | `\t"has-focus":%s` | `json_bool(...)` — **does not set `did_output`** |

`spaces` is `first_mci + i`, the mission-control index of the display's *first* space plus the loop
counter — it assumes contiguous indices and does not re-query per space (`src/display.c:77-88`).
Keep the arithmetic. When `display_space_list` returns nothing, the array prints `[]` because the
brackets are written outside the `if (space_list)` guard. The separator is again `", "`.

### 9.6 Rule object — `rule_serialize`, `src/rule.c:4-61`

Unlike the three above, this is **not** property-masked: one `fprintf` with all nineteen keys and
the separators baked into the format string. The Rust is one `write!`:

```rust
response.write(format_args!(
    "{{\n\
     \t\"index\":{},\n\
     \t\"label\":\"{}\",\n\
     \t\"app\":\"{}\",\n\
     \t\"title\":\"{}\",\n\
     \t\"role\":\"{}\",\n\
     \t\"subrole\":\"{}\",\n\
     \t\"display\":{},\n\
     \t\"space\":{},\n\
     \t\"follow_space\":{},\n\
     \t\"opacity\":{:.4},\n\
     \t\"manage\":{},\n\
     \t\"sticky\":{},\n\
     \t\"mouse_follows_focus\":{},\n\
     \t\"sub-layer\":\"{}\",\n\
     \t\"native-fullscreen\":{},\n\
     \t\"grid\":\"{}:{}:{}:{}:{}:{}\",\n\
     \t\"scratchpad\":\"{}\",\n\
     \t\"one-shot\":{},\n\
     \t\"flags\":\"0x{:08x}\"\n\
     }}",
    index,
    rule.label.as_deref().unwrap_or(""),
    ...
));
```

Value rules, all of them traps:

* `index` is the `Vec` position, an `i32`.
* `display` is `if rule.effects.display_id != 0 { display_manager_display_id_arrangement(...) } else { 0 }`;
  `space` is the same shape with `space_manager_mission_control_index`. Both are `i32`.
* `follow_space` and `one-shot` are `json_bool`; `manage`, `sticky`, `mouse_follows_focus` and
  `native-fullscreen` are `json_optional_bool` — **unquoted** `null`/`true`/`false`.
* `opacity` is `rule.effects.opacity as f64` with `{:.4}`, never the `f32`.
* `sub-layer` is `if rule.effects.flags.contains(RULE_LAYER) { layer_str[rule.effects.layer] } else { "" }`.
  `layer_str` (`src/misc/helpers.h:175-181`) is a six-slot designated-initializer array with
  **holes at indices 1 and 2**, and `src/rule.c:53` indexes it with an unvalidated `int`. On macOS
  a NULL `%s` prints `(null)`. `DECISIONS.md` 29 fixes that: keep `layer_str` an array of
  `Option<&'static str>` indexed by the raw `i32`, and print `"(null)"` for a hole and for an
  out-of-range index. The message parser only ever sets 0, 3, 4 or 5 (`src/message.c:2757-2775`),
  so the branch is unreachable today, but it must be written explicitly rather than left to panic.
* `grid` is six `unsigned` printed with `%d`: `rule.effects.grid[i] as i32` each.
* `flags` is `(rule.effects.flags as u32) << 16 | rule.flags as u32`, zero-padded to eight hex
  digits: `"0x{:08x}"`, **not** `{:#010x}` (which would print the `0x` twice). The C shifts a
  `uint16_t` promoted to `int`; widening to `u32` before the shift removes the signed overflow that
  a future `0x8000` effects bit would cause, and changes nothing today.
* The four `*_VALID` bits are derived from `regex.is_some()` when serialising, because
  `Option<PosixRegex>` replaces the flag-plus-`regex_t` pair (§10.3) and the flags word is
  still published verbatim.

### 9.7 Signal object — `event_signal_serialize`, `src/event_signal.c:400-429`

```
{\n
\t"index":%d,\n
\t"label":"%s",\n
\t"app":"%s",\n
\t"title":"%s",\n
\t"active":%s,\n
\t"event":"%s",\n
\t"action":"%s"\n
}
```

`label` is unescaped with an `""` fallback; `app`, `title` and `action` go through `json_escape`
with the same fallback; `active` is `json_optional_bool(signal.active)`, unquoted; `event` is
`signal_type_str[type]`.

### 9.8 Top-level array wrappers and the `[` / `]` asymmetries

These are the quirks `DECISIONS.md` 3 names explicitly. There are two families and they behave
differently on an empty result.

**Family A — `%c` terminator.** The closing bracket is emitted *as the separator of the last
element*, so no element means no bracket.

| function | C | shape |
| --- | --- | --- |
| `display_manager_query_displays` | `src/display_manager.c:5-21` | `"["` + per item `display_serialize` + `"%c"` (`,` for all but the last, `]` for the last) + `"\n"` |
| `space_manager_query_spaces_for_window` | `src/space_manager.c:26-44` | same |
| `space_manager_query_spaces_for_display` | `src/space_manager.c:46-65` | same |
| `space_manager_query_spaces_for_displays` | `src/space_manager.c:67-93` | nested: the inner loop separates with `","`, the outer with `"%c"` |

```rust
response.write(format_args!("["));
for i in 0..count {
    display_serialize(response, display_list[i], flags, ...);
    response.write(format_args!("{}", if i < count - 1 { ',' } else { ']' }));
}
response.write(format_args!("\n"));
```

Two consequences to preserve exactly:

* **`count == 0` prints `"[\n"` — an unterminated array.**
* In the three space queries the loop `continue`s when `space_manager_query_view` returns nothing
  (`src/space_manager.c:36`, `:57`, `:83`), which **skips the separator too**. If the skipped
  element was the last one, the `]` is never written and the array is unterminated even though
  earlier elements printed. Do not restructure these loops into `filter().enumerate()`; keep the
  `continue` inside the index loop so the `i < count - 1` test still sees the original count.

**Family B — unconditional `"]\n"`.**

| function | C | empty result |
| --- | --- | --- |
| `window_manager_query_windows_for_spaces` | `src/window_manager.c:38-52` | `"[]\n"` |
| `window_manager_query_window_rules` | `src/window_manager.c:26-35` | `"[]\n"` |
| `event_signal_list` | `src/event_signal.c:431-454` | `"[]\n"` |

```rust
response.write(format_args!("["));
for i in 0..window_count {
    ...
    if i < window_count - 1 { response.write(format_args!(",")); }
}
response.write(format_args!("]\n"));
```

`window_manager_query_windows_for_display` and `_for_displays` (`src/window_manager.c:54-89`) both
funnel into `_for_spaces`, so they share family B.

`event_signal_list` has a third comma rule on top: `event_did_output` guards the separator *between
signal types* (`src/event_signal.c:441-443`), `j < buf_len(...) - 1` guards it *within* a type
(`:447`), and `event_did_output` is only ever set to `true` (`:451`). Transcribe all three.

**Single-object queries** are the object plus `"\n"`: `space_manager_query_space`
(`src/space_manager.c:14-24`), `display_serialize` + `fprintf(rsp, "\n")` at `src/message.c:2442`,
`:2456`, `:2471`, and `window_serialize` + `fprintf(rsp, "\n")` at `:2582`.

### 9.9 Scalar config responses

`handle_domain_config` prints the current value when no value token follows. The conversion is
per-setting and observable; `yabai -m config window_animation_duration` returns `0.000000` today
and shell scripts parse it.

| setting | C | Rust |
| --- | --- | --- |
| `debug_output`, `mouse_follows_focus`, `window_zoom_persist`, `skip_window_focus_animation`, `window_opacity` | `"%s\n"` from `bool_str` (`src/message.c:1173`, `:1184`, `:1258`, `:1269`, `:1280`) | `"{}\n"` with `bool_str[value as usize]` |
| `window_opacity_duration`, `window_animation_duration` | `"%f\n"` — **six decimals** (`:1291`, `:1300`) | `"{:.6}\n"` on `value as f64` |
| `menubar_opacity`, `active_window_opacity`, `normal_window_opacity`, `split_ratio` | `"%.4f\n"` (`:1346`, `:1355`, `:1364`, `:1545`) | `"{:.4}\n"` on `value as f64` |
| `insert_feedback_color` | `"0x%x\n"` — **no zero padding, lowercase** (`:1373`) | `"0x{:x}\n"` on `window_manager.insert_feedback_color.p` (`u32`) |
| paddings, `window_gap` | `"%d\n"` (`:1384`-`:1487`) | `"{}\n"` on `i32` |
| every enum setting | `"%s\n"` from the matching `*_str` table | `"{}\n"` with `as_str()` |
| `external_bar` | `"%s:%d:%d\n"` (`:1691`) | `"{}:{}:{}\n"` |

`insert_feedback_color` is written back with `rgba_color_from_hex(value.u32_value)`
(`src/misc/helpers.h:178-187`), which stores the original `u32` in `.p` alongside the four derived
`f32` channels — so the printed value is exactly the hex the user typed, with no padding and with
`0x` written literally. `{:#x}` would also print `0x`, but `"0x{:x}"` is written out so the form is
unmistakable, and `{:#010x}` is wrong (it pads and is the rule form, §9.6).

Note the setter guard: `value.type == TOKEN_TYPE_U32 && value.u32_value` (`src/message.c:1374`), so
`insert_feedback_color 0x00000000` is rejected as an unknown value rather than setting transparent
black. Keep the non-zero test.

---

## 10. Rules and signals: owned structures and boxed POSIX regexes

### 10.1 `PosixRegex`

`DECISIONS.md` 26: *"POSIX regex through `libc::regcomp`/`regexec`/`regfree` behind a `Drop` wrapper
that boxes the `regex_t`."* The patterns come out of users' `yabairc` files and are POSIX **extended**
regular expressions; they are not, and must not become, anything else.

The type is named `PosixRegex` and is defined once, in `src/misc/regex.rs`, alongside `RegexMatch`
and the free function `regex_match`. Four modules name it and none declares its own wrapper:
`rule.rs` and `event_signal.rs` hold it in their fields (`src/rule.h:51-54`,
`src/event_signal.h:112-113`), `message.rs` builds it at the six `regcomp` sites
(`src/message.c:2641`, `:2651`, `:2661`, `:2671`, `:2895`, `:2903`), and `window_manager.rs` passes
it to `regex_match` (`src/window_manager.c:94`, `:97`, `:100`, `:103`). That spelling is the one
`TRANSLATION_PLAN.md` uses, and `DECISIONS.md` 37 fixes it — no other spelling of it may appear in
any document or in the Rust.

```rust
pub struct PosixRegex {
    regex: Box<libc::regex_t>,
}

impl PosixRegex {
    pub fn compile(pattern: &std::ffi::CStr) -> Option<PosixRegex> {
        let mut regex: Box<libc::regex_t> = Box::new(unsafe { std::mem::zeroed() });
        let status = unsafe { libc::regcomp(regex.as_mut(), pattern.as_ptr(), libc::REG_EXTENDED) };
        if status == 0 {
            Some(PosixRegex { regex })
        } else {
            None
        }
    }

    pub fn matches(&self, subject: &std::ffi::CStr) -> bool {
        let status = unsafe {
            libc::regexec(self.regex.as_ref(), subject.as_ptr(), 0, std::ptr::null_mut(), 0)
        };
        status == 0
    }
}

impl Drop for PosixRegex {
    fn drop(&mut self) {
        unsafe { libc::regfree(self.regex.as_mut()) };
    }
}
```

* **The `Box` is the point.** `rule_add` and `event_signal_add` bitwise-move the whole struct into a
  buffer (`src/rule.c:178`, `src/event_signal.c:355`), so the `regex_t` moves with it. Boxing gives
  it a stable address and removes the question entirely.
* **`regcomp` is called without `REG_NOSUB`**, and `regexec` is always
  `regexec(re, subject, 0, NULL, 0)` — a boolean test with no capture groups. Keep both.
* **A failed `regcomp` must not be `regfree`d.** `compile` returns `None` before the wrapper exists,
  so the plain `Box<regex_t>` is dropped without `Drop` running. That is the only correct order.
* The subject must be NUL-terminated. `regexec` takes `*const c_char`, so every match site builds a
  `CString` (or holds one already, §10.5).

### 10.2 The three-valued `RegexMatch`

`DECISIONS.md` 26: *"The three-valued match result stays three-valued."* Collapsing it to a `bool`
silently changes which events fire.

```rust
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegexMatch {
    Undefined = 0,
    Yes = 1,
    No = 2,
}

pub fn regex_match(regex: Option<&PosixRegex>, subject: &std::ffi::CStr) -> RegexMatch {
    match regex {
        None => RegexMatch::Undefined,
        Some(regex) => {
            if regex.matches(subject) { RegexMatch::Yes } else { RegexMatch::No }
        }
    }
}
```

Discriminants are explicit and match `src/misc/macros.h:22-24`.

**The expected-value comparison** is the idiom that needs the third state
(`src/event_signal.c:17-18`, `src/window_manager.c:93-104`):

```rust
let regex_match_app = if signal.app_regex_exclude { RegexMatch::Yes } else { RegexMatch::No };
let app_no_match = regex_match(signal.app_regex.as_ref(), app) == regex_match_app;
```

The code builds the value that means *reject* — `Yes` when the filter is an exclusion, `No` when it
is an inclusion — and rejects on equality. `Undefined` equals neither, so **a subscription with no
regex compiled never rejects**. Write it exactly this way; a `match` on the three variants that
tries to be clearer will get the exclusion case backwards.

Ten call sites: `src/event_signal.c:18`, `:24`, `:35`, `:38`, `:47`, `:50` (signal filtering) and
`src/window_manager.c:94`, `:97`, `:100`, `:103` (rule matching). All ten follow the same two-line
shape.

`window_manager_rule_matches_window` (`src/window_manager.c:91-106`) is four of these in a row with
an early `return false` after each — keep the early returns rather than OR-ing the four together,
so the `regexec` calls short-circuit as they do today.

**NULL subjects.** `ts_cfstring_copy` (`src/misc/helpers.h:361-370`) can return NULL, so
`es->title` can be NULL and `regexec(re, NULL, ...)` would dereference it. `DECISIONS.md` 4
forbids reproducing that: the title is `Option<CString>` and `None` is treated as `c""`, which is
what the surrounding code already assumes. One line in `DEVIATIONS.md`.

### 10.3 `Rule`

```rust
pub struct Rule {
    pub label: Option<String>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub app_regex: Option<PosixRegex>,
    pub title_regex: Option<PosixRegex>,
    pub role_regex: Option<PosixRegex>,
    pub subrole_regex: Option<PosixRegex>,
    pub effects: RuleEffects,
    pub flags: RuleFlags,
}

pub struct RuleEffects {
    pub display_id: u32,
    pub space_id: u64,
    pub opacity: f32,
    pub manage: RuleProp,
    pub sticky: RuleProp,
    pub mouse_follows_focus: RuleProp,
    pub layer: i32,
    pub fullscreen: RuleProp,
    pub grid: [u32; 6],
    pub scratchpad: Option<String>,
    pub flags: RuleEffectsFlags,
}

#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleProp {
    #[default]
    Undefined = 0,
    On = 1,
    Off = 2,
}
```

Mapping from `src/rule.h:29-56`:

* The four `*_VALID` flag bits and the four bare `regex_t` fields collapse into four
  `Option<PosixRegex>`. `rule_serialize` still publishes the bits, derived from
  `is_some()` (§9.6). The four `*_EXCLUDE` bits stay in `flags`, because they are independent of
  whether a regex compiled.
* `RuleFlags` and `RuleEffectsFlags` are newtypes over `u16` with associated constants
  (`DECISIONS.md` 11 forbids `bitflags`, 31 mandates the newtype). The discriminants are published
  over the socket by `"flags":"0x%08x"` and are therefore fixed: `RULE_APP_VALID 0x001` through
  `RULE_ONE_SHOT_REMOVE 0x200`, and `RULE_FOLLOW_SPACE 0x01`, `RULE_OPACITY 0x02`,
  `RULE_LAYER 0x04`. `rule_check_flag` / `rule_set_flag` / `rule_clear_flag` and their
  `rule_effects_*` counterparts (`src/rule.h:59-65`) become `contains` / `insert` / `remove`
  methods on the newtypes.
* `layer` stays a plain `i32`, because `src/rule.c:53` indexes `layer_str` with it unvalidated
  (§9.6).
* `grid` stays `[u32; 6]` so the `%d`-into-`unsigned` behaviour of §4.5 is preserved.
* Text fields are `Option<String>`, built with `String::from_utf8_lossy(...).into_owned()` at the
  point they are stored (`DECISIONS.md` 28). The regexes are compiled from the **raw bytes** of the
  same value, through a `CString`, so `regcomp` sees exactly what C saw.
* `rule_destroy` (`src/rule.c:207-221`) disappears into `Drop`. It is not a public function in
  Rust. The C version neither clears the flags nor NULLs the pointers, so calling it twice is a
  double free; `Drop` makes that unrepresentable.

### 10.4 `Signal`

```rust
pub struct Signal {
    pub app: Option<String>,
    pub title: Option<String>,
    pub app_regex_exclude: bool,
    pub title_regex_exclude: bool,
    pub app_regex: Option<PosixRegex>,
    pub title_regex: Option<PosixRegex>,
    pub active: SignalProp,
    pub command: Option<String>,
    pub label: Option<String>,
}

#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SignalProp {
    #[default]
    Undefined = 0,
    Yes = 1,
    No = 2,
}
```

From `src/event_signal.h:104-116`. Same collapse of `*_regex_valid` + `regex_t` into `Option`.
`event_signal_destroy` disappears into `Drop` for the same reason.

The `active` test at `src/event_signal.c:26-27` and `:52-53` is:

```c
bool active = signal->active == SIGNAL_PROP_UD;
if (!active) active = es->active == (signal->active == SIGNAL_PROP_YES);
```

`es->active` is an `int` holding 0 or 1, assigned from a comparison. In Rust keep it an `i32` on
the queued record and write:

```rust
let mut active = signal.active == SignalProp::Undefined;
if !active {
    active = event_signal.active == i32::from(signal.active == SignalProp::Yes);
}
```

### 10.5 Move, not copy

`DECISIONS.md` 17 replaces `buf_*` with `Vec`. Both adders are a label-scoped remove followed by a
move:

```c
void rule_add(struct rule *rule)
{
    if (rule->label) rule_remove_by_label(rule->label);
    buf_push(g_window_manager.rules, *rule);
}
```

(`src/rule.c:175-179`; `event_signal_add` at `src/event_signal.c:352-356` is identical in shape.)

```rust
pub fn rule_add(window_manager: &mut WindowManager, rule: Rule) {
    if let Some(label) = rule.label.as_deref() {
        rule_remove_by_label(window_manager, label);
    }
    window_manager.rules.push(rule);
}
```

Three hard rules:

1. **`Rule` and `Signal` are never `Copy` and never `Clone`.** The C is a bitwise move of four
   `regex_t`s and six owned pointers; a `Clone` would make the double-ownership hazard
   representable again.
2. **The success path does not destroy.** `src/message.c:2820-2824`:
   `if (parse_rule(...)) rule_add(&rule); else rule_destroy(&rule);`. In Rust that is
   `if did_parse { rule_add(window_manager, rule) } else { drop(rule) }` — and the `else` may be
   written as nothing at all, since the value falls out of scope. Do **not** add a `rule_destroy`
   equivalent after `rule_add`.
3. **`buf_del` is a swap-remove** (`src/misc/sbuffer.h:19` copies the last element over slot `x`).
   `Vec::swap_remove`, never `Vec::remove`. The index order that `rule --list` and `signal --list`
   print is the `Vec` order, so using `remove` would silently change which rule a later
   `rule --remove 2` deletes.

`buf_del`'s value is the **pre-decrement** length, used as a truthy "did we delete" at
`src/event_loop.c:572` and `src/window_manager.c:1570`. It is always non-zero there, because the
loop is indexing a live element. The one-shot sweeps become:

```rust
let mut rule_index = 0;
let mut rule_length = window_manager.rules.len();
while rule_index < rule_length {
    if window_manager.rules[rule_index].flags.contains(RuleFlags::ONE_SHOT_REMOVE) {
        window_manager.rules.swap_remove(rule_index);
        rule_length -= 1;
    } else {
        rule_index += 1;
    }
}
```

The C writes this as `--i; --rule_len;` inside a `for`; the `while` above visits exactly the same
elements in the same order. `rule_destroy` is gone: `swap_remove` returns the `Rule`, which drops.

### 10.6 Two-phase parse with error accumulation

`parse_rule` (`src/message.c:2596-2804`) and the `signal --add` loop (`:2877-2937`) keep parsing
after an error: each failure sets `did_parse = false` and `continue`s, so one message can report
several errors, and the end-of-parse checks add more (`has_filter` at `:2793`, `has_signal_type`
and `has_command` at `:2939` / `:2944`, `unsupported_exclusion` at `:2798` / `:2949`).

Use a `did_parse: bool` accumulator, never `?`. A `?`-based early return would emit only the first
error and change the response text.

`unsupported_exclusion` records the **last** offending key, not the first, and is reported once
(`src/message.c:2600`, `:2870`, thirteen assignment sites). In Rust,
`let mut unsupported_exclusion: Option<&[u8]> = None;` holding the key slice, reported with
`FailurePiece::Bytes`. So `yabai -m rule --add app=X display!=1 space!=2` reports `space` only.
Preserve that.

Repeated keys (`app=A app=B`) overwrite the field and the regex. In C that leaks the previous
`string_copy` and the previously compiled `regex_t`; assigning to an `Option<String>` /
`Option<PosixRegex>` drops the old value. One line in `DEVIATIONS.md`.

### 10.7 The `^` prefix and the nested selector parse

`src/message.c:2680-2687` (`display=`) and `:2694-2701` (`space=`):

```c
if (value[0] == ARGUMENT_RULE_VALUE_SPACE) {
    ++value;
    rule_effects_set_flag(&rule->effects, RULE_FOLLOW_SPACE);
}

struct selector selector = parse_display_selector(rsp, &value, display_manager_active_display_id(), false);
```

`ARGUMENT_RULE_VALUE_SPACE` is the character `'^'`. The flag is set **before** the selector parse,
and the parse runs on a second cursor over the value sub-slice (§2.3):

```rust
let mut value_start = pair.value;
if message_cursor.bytes()[value_start] == ARGUMENT_RULE_VALUE_SPACE {
    value_start += 1;
    rule.effects.flags.insert(RuleEffectsFlags::FOLLOW_SPACE);
}

let selector = {
    let mut value_cursor = message_cursor.cursor_at(value_start);
    parse_display_selector(response, &mut value_cursor, display_manager_active_display_id(display_manager), false, display_manager)
};

match selector.resolved() {
    Some(display_id) => rule.effects.display_id = display_id,
    None => did_parse = false,
}
```

The outer cursor is untouched by the inner parse, which is what lets the `key=value` loop continue
from the next token.

Note `rule_combine_effects` (`src/rule.c:63-111`): the `did` and `sid` arms both **overwrite**
`RULE_FOLLOW_SPACE` on the accumulator — setting it when this rule had it and *clearing* it
otherwise — so a later `display=` without `^` cancels an earlier `space=^`. That is the C
behaviour and it stays.

---

## 11. The twelve `YABAI_*` environment variables

`DECISIONS.md` 3 fixes these byte for byte: they go straight into the user's shell. They are
written by `event_signal_push` (`src/event_signal.c:99-341`) into at most four name/value slots per
queued event, and exported with `setenv(name, value, 1)` in the forked grandchild
(`src/event_signal.c:86-89`).

### 11.1 The complete table

| variable | conversion | source type | emitted for | C |
| --- | --- | --- | --- | --- |
| `YABAI_PROCESS_ID` | `%d` | `pid_t` (`i32`) | `application_launched`, `_activated`, `_deactivated`, `_visible`, `_terminated`, `_hidden`, `_front_switched` | `:132-133`, `:143-144`, `:169-170`, `:180-181` |
| `YABAI_RECENT_PROCESS_ID` | `%d` | `pid_t` (`i32`) | `application_front_switched` | `:171-172` |
| `YABAI_WINDOW_ID` | `%d` | **`uint32_t`** | `window_created`, `_focused`, `_deminimized`, `_destroyed`, `_moved`, `_resized`, `_minimized`, `_title_changed` | `:194-195`, `:206-207`, `:221-222` |
| `YABAI_SPACE_ID` | `%lld` | **`uint64_t`** | `space_created`, `space_destroyed`, `space_changed` | `:237-238`, `:248-249`, `:268-269` |
| `YABAI_RECENT_SPACE_ID` | `%lld` | **`uint64_t`** | `space_changed` | `:270-271` |
| `YABAI_SPACE_INDEX` | `%d` | `int` | `space_created`, `space_changed` | `:239-240`, `:273-274` |
| `YABAI_RECENT_SPACE_INDEX` | `%d` | `int` | `space_changed` | `:275-276` |
| `YABAI_DISPLAY_ID` | `%d` | **`uint32_t`** | `display_added`, `_moved`, `_resized`, `_removed`, `_changed` | `:289-290`, `:300-301`, `:320-321` |
| `YABAI_RECENT_DISPLAY_ID` | `%d` | **`uint32_t`** | `display_changed` | `:322-323` |
| `YABAI_DISPLAY_INDEX` | `%d` | `int` | `display_added`, `_moved`, `_resized`, `_changed` | `:291-292`, `:325-326` |
| `YABAI_RECENT_DISPLAY_INDEX` | `%d` | `int` | `display_changed` | `:327-328` |
| `YABAI_MISSION_CONTROL_MODE` | `%s` | `mission_control_mode_str[mode]` | `mission_control_enter`, `mission_control_exit` | `:337-338` |

The three bolded rows are `DECISIONS.md` 29 in action:

```rust
format!("{}", window.id as i32)      // YABAI_WINDOW_ID,  %d on a uint32_t
format!("{}", space_id as i64)       // YABAI_SPACE_ID,   %lld on a uint64_t
format!("{}", display_id as i32)     // YABAI_DISPLAY_ID, %d on a uint32_t
```

Printing the unsigned value instead would change the text for any id above `INT32_MAX`.
`mission_control_mode_str` is `src/mission_control.c:38-44`: `inactive`, `show`,
`show-all-windows`, `show-front-windows`, `show-desktop`.

### 11.2 Slot order matters

The four slots are filled in the order listed above per event type, and the grandchild `setenv`s
them in slot order (`src/event_signal.c:86-89`). `space_changed` fills all four:
`YABAI_SPACE_ID`, `YABAI_RECENT_SPACE_ID`, `YABAI_SPACE_INDEX`, `YABAI_RECENT_SPACE_INDEX`;
`display_changed` likewise. Nothing observes the order today, but the record shape is
`[Option<(CString, CString)>; 4]` and the indices are the C's.

### 11.3 Building them before the fork

`DECISIONS.md` 25: *"Everything the child needs (argv, environment, the regex filter verdict) is
computed in the parent before the fork; the child only calls async-signal-safe functions and leaves
through `_exit`."* For this subsystem that means:

* the queued record owns `CString` name/value pairs and an owned command `CString`, built in
  `event_signal_push` on the event-loop thread;
* the `argv` array is `[*const c_char; 5]` matching `src/event_signal.c:91` exactly —
  `"/usr/bin/env"`, `"sh"`, `"-c"`, the command, then null. Note `exec[0]` is `"/usr/bin/env"` and
  it is also `argv[0]`; `execvp(exec[0], exec)` passes the same pointer twice;
* nothing between `fork` and `execvp` allocates.

`arg_size` is 128 and `snprintf` truncates silently, but no value in the table above can exceed
about twenty bytes and no name exceeds twenty-six, so the cap is unreachable. The Rust formats
without a cap; one line in `DEVIATIONS.md`.

`DECISIONS.md` 25 keeps `libc::fork` for both forks and keeps the double fork. The parent branch is
taken when `pid != 0`, **including `-1`**, so a failed fork silently drops every queued signal
(`src/event_signal.c:65-68`); the intermediate child `continue`s on `pid != 0`
(`src/event_signal.c:84`) so it keeps spawning the remaining commands; and both children leave
through `exit`, not `_exit`, so the stdio buffers inherited from the parent are flushed a second
time (`src/event_signal.c:92`, `:96`). `signal(SIGCHLD, SIG_IGN)` at `src/yabai.c:151` is what
reaps them.

---

## 12. Client-mode framing — `src/yabai.c:54-121`

The client half of the wire protocol. It is a different process from the daemon but the same
binary, and `DECISIONS.md` 3 fixes the framing, so the two halves must be changed together or not
at all.

### 12.1 Request

`client_send_message(argc - 1, argv + 1)` is called from `parse_arguments`
(`src/yabai.c:212`), so inside the function `argv[0]` is `-m` / `--message` and the message words
are `argv[1..argc]`.

```
message_length = argc                       // src/yabai.c:66
for i in 1..argc:
    message_length += strlen(argv[i])       // src/yabai.c:68-71
```

so `message_length` is the sum of the argument lengths plus one NUL per argument plus one extra
NUL. The buffer is `sizeof(int) + message_length` bytes:

* bytes 0..4 — `message_length` as a host-endian `int`, **excluding** these four bytes
  (`src/yabai.c:75`);
* then each argument's bytes followed by `'\0'` (`src/yabai.c:76-80`);
* then one more `'\0'` (`src/yabai.c:81`) — the double NUL that `get_token` parks on (§2.4).

`send(sockfd, message, sizeof(int) + message_length, 0)` writes the whole thing in one call
(`src/yabai.c:94`), then `shutdown(sockfd, SHUT_WR)` (`src/yabai.c:100`) half-closes so the
daemon's read loop at `src/event_loop.c:1624-1629` terminates.

```rust
let mut message: Vec<u8> = Vec::new();
message.extend_from_slice(&0_i32.to_ne_bytes());
for argument in &arguments {
    message.extend_from_slice(argument.as_bytes());
    message.push(0);
}
message.push(0);

let message_length = (message.len() - std::mem::size_of::<i32>()) as i32;
message[..std::mem::size_of::<i32>()].copy_from_slice(&message_length.to_ne_bytes());

stream.write_all(&message)?;
stream.shutdown(std::net::Shutdown::Write)?;
```

`to_ne_bytes` is deliberate: the header is host-endian, written with `memcpy` of an `int`, and read
with `read(param1, &bytes_to_read, sizeof(int))`. Client and daemon are always the same machine.

The socket path is `"/tmp/yabai_%s.socket"` with `$USER` (`src/yabai.c:2`, formatted at `:86`);
`env USER` unset is a hard error (`src/yabai.c:60-62`), as is `argc <= 1` (`:56-58`).

### 12.2 Response

```c
while ((bytes_read = read(sockfd, rsp, sizeof(rsp)-1)) > 0) {
    rsp[bytes_read] = '\0';

    if (rsp[0] == FAILURE_MESSAGE[0]) {
        result = EXIT_FAILURE;
        output = stderr;
        fprintf(output, "%s", rsp + 1);
        fflush(output);
    } else {
        fprintf(output, "%s", rsp);
        fflush(output);
    }
}
```

Four things a rewrite must keep:

1. **Only the first byte of each chunk is tested**, and exactly one byte is stripped
   (`rsp + 1`). A second BEL later in the same chunk reaches the terminal as a bell.
2. **`output` is loop-carried.** Once a chunk begins with BEL, `output` becomes `stderr` and
   **stays** `stderr` for every later chunk of the same response. A response whose first chunk is a
   failure sends all subsequent output to stderr too.
3. **`result` is sticky**: any BEL chunk makes the exit code `EXIT_FAILURE`; otherwise
   `EXIT_SUCCESS` (`src/yabai.c:103`, `:112`, `:123`).
4. **`fprintf("%s", rsp)` stops at the first NUL in the chunk.** The daemon never writes a NUL, so
   this is invisible; keep writing the chunk up to `bytes_read` in Rust, which is the same bytes
   for every response the daemon can produce.

```rust
let mut result = EXIT_SUCCESS;
let mut write_to_standard_error = false;
let mut chunk = [0_u8; BUFSIZ - 1];

while let Ok(bytes_read) = stream.read(&mut chunk) {
    if bytes_read == 0 { break; }

    let (payload, is_failure) = if chunk[0] == FAILURE_MESSAGE[0] {
        (&chunk[1..bytes_read], true)
    } else {
        (&chunk[..bytes_read], false)
    };

    if is_failure {
        result = EXIT_FAILURE;
        write_to_standard_error = true;
    }

    if write_to_standard_error {
        let mut standard_error = std::io::stderr();
        let _ = standard_error.write_all(payload);
        let _ = standard_error.flush();
    } else {
        let mut standard_output = std::io::stdout();
        let _ = standard_output.write_all(payload);
        let _ = standard_output.flush();
    }
}
```

The chunk size is `BUFSIZ - 1` because C reads `sizeof(rsp) - 1` to leave room for the NUL it
writes; keeping the same size keeps the same chunking. `BUFSIZ` is 1024 on macOS.

Finally `socket_close(sockfd)` — `shutdown(SHUT_RDWR)` then `close`
(`src/misc/helpers.h:198-202`) — and `return result`, which `parse_arguments` passes to `exit`
(`src/yabai.c:212`).

---

## 13. `DEVIATIONS.md` lines this document creates

One line each, in the format `DECISIONS.md` 4 asks for (C location, what C did, what Rust does):

* `src/message.c:308` — read one byte past the message allocation when the client's trailing NUL
  was missing; Rust bounds-checks and treats the end of the buffer as the terminator (§2.4).
* `src/message.c:283-296` — `token_char_int_table`, a sparse lookup reached only after an explicit
  ASCII range check; Rust computes the digit arithmetically (§3.1).
* `src/message.c:429-438` — `daemon_deprecated`, `__unused` with no caller; not translated
  (`DECISIONS.md` 5, §7.7).
* `src/message.c:667` and ten more sites, `src/rule.c:6`, `src/event_signal.c:402`, `:433` —
  `TIME_FUNCTION`; not translated (`DECISIONS.md` 5, §1).
* `src/message.c:2404` — leaked the `malloc`-ed scratchpad label when
  `window_manager_set_scratchpad_for_window` returned `false`; Rust drops the `String`.
* `src/message.c:2637-2676` — a repeated rule key leaked the previous `string_copy` and the
  previously compiled `regex_t`; Rust drops the old `Option` contents (§10.6).
* `src/rule.c:93-96` — `rule_combine_effects` replaced the accumulator's `scratchpad` with a fresh
  `string_copy` into a caller-local that was never destroyed, leaking on every application;
  `Option<String>` drops it.
* `src/rule.c:60` — `(uint32_t)(rule->effects.flags << 16)` promotes a `uint16_t` to `int` before
  the shift, which would be signed overflow once an effects bit reached `0x8000`; Rust widens to
  `u32` first (§9.6).
* `src/rule.c:53` — `layer_str[rule->effects.layer]` indexes a holed array with an unvalidated
  `int`, printing `(null)` on macOS for indices 1 and 2 and reading out of bounds past index 5;
  Rust returns `"(null)"` for a hole and for an out-of-range index (§9.6).
* `src/event_signal.c:18`, `:24`, `:35`, `:38`, `:47`, `:50` — `regexec` could be handed a NULL
  subject when `ts_cfstring_copy` failed; Rust treats a missing subject as `""` (§10.2).
* `src/event_signal.c:104-108` — the 128-byte `snprintf` cap on environment names and values,
  unreachable for every value the table in §11.1 can produce; Rust formats without a cap.
* `src/message.c:2895`, `:2903`, `:2641`, `:2651`, `:2661`, `:2671` — a rule or signal pattern
  containing invalid UTF-8 is stored lossily for `rule --list` / `signal --list` while the regex is
  still compiled from the raw bytes (`DECISIONS.md` 28, §10.3).

The three socket-setup deviations in `message_loop_begin` (`src/message.c:3018` uninitialised
`sockaddr_un`, `:3020` silent `sun_path` truncation, `:3027-3037` leaked `sockfd`) belong to the
entry-and-event-loop document, not this one.

---

## 14. Glossary for this subsystem

Fixed spellings, per `DECISIONS.md` 37. Anything not on this list keeps its C name unabbreviated.

| C | Rust |
| --- | --- |
| `struct token` | `Token { start, length }` |
| `char **message` cursor | `MessageCursor { bytes, at }` |
| `struct token_value` | `TokenValue { token, type_of_value }` |
| `enum token_type` | `TokenType` |
| `struct selector` | `Selector<Target> { token, outcome }` + `SelectorOutcome` |
| `struct properties` | `Properties { token, did_parse, did_error, flags }` |
| `FILE *rsp` | `Response` |
| `daemon_fail` | `Response::fail` / `daemon_fail!` |
| `FAILURE_MESSAGE` | `FAILURE_MESSAGE: &[u8]` |
| `regex_t` | `PosixRegex` |
| `REGEX_MATCH_UD/YES/NO` | `RegexMatch::Undefined/Yes/No` |
| `RULE_PROP_UD/ON/OFF` | `RuleProp::Undefined/On/Off` |
| `SIGNAL_PROP_UD/YES/NO` | `SignalProp::Undefined/Yes/No` |
| `enum rule_flag` | `RuleFlags` (newtype over `u16`) |
| `enum rule_effects_flag` | `RuleEffectsFlags` (newtype over `u16`) |
| `acting_did` | `acting_display_id` |
| `acting_sid` | `acting_space_id` |
| `acting_window` | `acting_window_id: Option<u32>` |
| `selector_wid` | `selector_window_id: u32` |
| `es` (`struct event_signal *`) | `event_signal` |
| `arg_name` / `arg_value` | `[Option<(CString, CString)>; 4]` |
