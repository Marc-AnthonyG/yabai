# Signature changes — `w2b-message` (part 2 of W2-message)

No frozen signature in `src/message.rs` was changed, and no other module's signature was adjusted in
the isolated copy: every callee was called through the signature on disk (`DECISIONS.md` 42). The
one request already recorded for this file (`w2-message-part1.md`, the added
`c_string_in_buffer_equals`) is in place and needed nothing further.

Five functions with no C original were **added** to `src/message.rs`, `pub(crate)` per
`DECISIONS.md` 48. Each is one `daemon_fail` format string that the C repeats verbatim at many call
sites, written once as `Response::fail_pieces` with `FailurePiece::Bytes` for every `%.*s` / `%s`;
the bytes written are identical to the inline form.

`src/message.c:1179`, 54 sites in `:1152-2414` | `daemon_fail(rsp, "unknown value '%.*s' given to command '%.*s' for domain '%.*s'\n", …)` | `pub(crate) fn daemon_fail_with_unknown_value_given_to_command_for_domain(response: &mut Response, message_bytes: &[u8], value: Token, command: Token, domain: Token)` | new helper, no C original
`src/message.c:2476`, `:2533`, `:2587` | `daemon_fail(rsp, "unknown option '%.*s' given to command '%.*s' for domain '%.*s'\n", …)` | `pub(crate) fn daemon_fail_with_unknown_option_given_to_command_for_domain(response: &mut Response, message_bytes: &[u8], option: Token, command: Token, domain: Token)` | new helper, no C original
`src/message.c:1695`, `:1755`, `:2042`, `:2414`, `:2592`, `:2860`, `:2975` | `daemon_fail(rsp, "unknown command '%.*s' for domain '%.*s'\n", …)` | `pub(crate) fn daemon_fail_with_unknown_command_for_domain(response: &mut Response, message_bytes: &[u8], command: Token, domain: Token)` | new helper, no C original
`src/message.c:2634`, 10 sites in `:2596-2934` | `daemon_fail(rsp, "invalid value '%s' for key '%s'\n", value, key)` | `pub(crate) fn daemon_fail_with_invalid_value_for_key(response: &mut Response, value: &[u8], key: &[u8])` | new helper, no C original
`src/message.c:2644`, `:2654`, `:2664`, `:2674`, `:2897`, `:2905` | `daemon_fail(rsp, "invalid regex pattern '%s' for key '%s'\n", value, key)` | `pub(crate) fn daemon_fail_with_invalid_regex_pattern_for_key(response: &mut Response, value: &[u8], key: &[u8])` | new helper, no C original
