# Deviations — `w1-message` (W1-message, `src/message.h` + `src/message.c` → `src/message.rs`)

`src/message.c:429` | `daemon_deprecated` was an `__unused static inline` variadic helper with no caller anywhere in `src/` | not translated (`DECISIONS.md` 5, `state-access/message.md` "Not translated"); no `#[allow(dead_code)]` stub

`src/message.c:418` | `daemon_fail` was a `static inline` variadic function local to `src/message.c` | the `macro_rules! daemon_fail!` of `state-access/message.md` row 9 already exists on disk in `src/misc/response.rs` and is not redefined in `src/message.rs` (`DECISIONS.md` 42); its `if (!rsp) return;` guard at `:420` is `Response::silent()` instead

`src/message.c:283-296` | `token_char_int_table` was a 103-entry sparse designated-initializer array mapping hex digit characters to their values | not declared; replaced by the arithmetic it performs over the domain it is reached with (`patterns/message-and-serialisation.md` §3.1)

`src/message.c:254` | `struct token` was `{ char *text; int length; }`, a pointer plus an `int` length into the message buffer | `Token { start: usize, length: usize }`, a `(start, length)` range over the same buffer (`DECISIONS.md` 27); `token_is_valid`'s `text && length > 0` loses its NULL half, which no token produced by `get_token` can ever fail

`src/message.c:270-280` | `struct token_value` carried an anonymous union of `int`/`float`/`uint32_t`/`char *` tagged by its `enum token_type` field | the union folds into `TokenType::Int(i32)` / `Float(f32)` / `U32(u32)` payloads (`DECISIONS.md` 31); `string_value` disappears, the text being read from `token` and the buffer

`src/message.c:652-663` | `struct selector` carried an anonymous union of `int dir` / `uint32_t did` / `uint64_t sid` / `struct window *window` plus a `bool did_parse`, with "parsed but unresolved" spelled as `did_parse == true` with a zeroed arm | `Selector<Target>` with `SelectorOutcome::{NotASelector, ParsedButUnresolved, Resolved(Target)}` (`patterns/message-and-serialisation.md` §5.1); `selector::window` becomes `Selector<WindowId>`, a handle, never a reference (`DECISIONS.md` 14)

`src/message.c:440` | `parse_key_value_pair` had three out-parameters (`char **key`, `char **value`, `bool *exclusion`) and two distinct failure shapes, leaving `*exclusion` untouched on both | one `Option<KeyValuePair>` whose `None` covers both failure shapes; `KeyValuePair::exclusion` carries the flag both callers pre-initialise to `false` (`DECISIONS.md` 32, `state-access/message.md` note 11)

`src/message.c:613`, `:625` | `parse_property` and `parse_properties` took the `int property_count` that sized `property_val[]` and `property_str[]` | the count disappears into the lengths of the two slices (`state-access/message.md` step 5)

`src/message.c:515`, `:529`, `:539` | `reserved_display_identifiers`, `reserved_space_identifiers` and `reserved_window_identifiers` were lowercase `static char *[]` file-scope arrays sized by `array_count` | `const RESERVED_DISPLAY_IDENTIFIERS: [&str; 10]`, `RESERVED_SPACE_IDENTIFIERS: [&str; 6]` and `RESERVED_WINDOW_IDENTIFIERS: [&str; 11]`, the C names in the Rust constant case, each built out of the same `ARGUMENT_*` constants (`patterns/message-and-serialisation.md` §8)

`src/message.c:2718` | the `sscanf` format `"%f"` was an inline literal at the call site | named `const SCAN_ONE_FLOAT: &CStr` so the nine formats of `patterns/message-and-serialisation.md` §4.1 are greppable as one set

`src/message.c:1-5` | `g_message_loop` was an anonymous file-scope struct of `int sockfd`, `bool is_running` and `pthread_t thread` | `static MESSAGE_LOOP: OnceLock<MessageLoop>` holding a `UnixListener` and a `JoinHandle<()>` (`GLOSSARY.md` §3.29, §8.2); `is_running` disappears, being only ever set `true` and read as the accept loop's condition (`:3005`, `:3041`)

`src/message.c:3003` | `message_loop_run` took a `void *context` pthread trampoline argument that was always NULL, ignored, and suppressed with the `#pragma clang diagnostic` pair at `:3001` and `:3014` | no parameter, and no pragma; a Rust thread closure has no trampoline argument

`src/message.c:3016` | `message_loop_begin` took `char *socket_path` | `socket_path: &Path`, which `std::fs` takes directly (`state-access/message.md` judgement call 8); `SOCKET_FILE` stays a `String` and the caller passes `Path::new(..)`

`src/message.c:667`, `:792`, `:873`, `:1154`, `:1702`, `:1761`, `:2049`, `:2421`, `:2598`, `:2808`, `:2866` | `TIME_FUNCTION` instrumented eleven functions for the `PROFILE` machinery | not translated (`DECISIONS.md` 5)
