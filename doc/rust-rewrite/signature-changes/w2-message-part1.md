# Signature changes — `w2-message-part1` (part of W2-message)

No signature in `src/message.rs` was changed, and no other module's signature was adjusted in the
isolated copy. Every function this part translated kept the frozen signature.

One function with no C original was **added** to `src/message.rs`, because `parse_value_type`
(`src/message.c:472`) and `parse_resize_handle` (`:483`) take the `sscanf` scanset buffer as
`&[libc::c_char; MAXLEN]` and C compares it with `string_equals`, which
`crate::misc::helpers::string_equals` cannot express for that type:

`src/message.c:474` | new helper, no C original | `pub(crate) fn c_string_in_buffer_equals(buffer: &[libc::c_char; MAXLEN], candidate: &str) -> bool` — the NUL-terminated prefix of the buffer compared byte for byte against the candidate, which is `string_equals` specialised to the scanset buffer
