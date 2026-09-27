use std::ffi::CString;

use crate::daemon_fail;
use crate::message::common_failures::{
    daemon_fail_with_invalid_regex_pattern_for_key, daemon_fail_with_invalid_value_for_key,
    daemon_fail_with_unknown_command_for_domain,
};
use crate::message::token::{
    MessageCursor, Token, TokenType, c_string_at, parse_key_value_pair, token_equals,
    token_to_value,
};
use crate::serialise::signal::event_signal_list;
use crate::signal::definition::{
    SIGNAL_TYPE_COUNT, Signal, SignalProp, SignalType, event_signal_add, event_signal_remove,
    event_signal_remove_by_index, signal_type_from_string,
};
use crate::support::regex::PosixRegex;
use crate::support::response::{FailurePiece, Response};

/* --------------------------------DOMAIN SIGNAL-------------------------------- */
pub(crate) const COMMAND_SIGNAL_ADD: &str = "--add";
pub(crate) const COMMAND_SIGNAL_REM: &str = "--remove";
pub(crate) const COMMAND_SIGNAL_LS: &str = "--list";

pub(crate) const ARGUMENT_SIGNAL_KEY_APP: &str = "app";
pub(crate) const ARGUMENT_SIGNAL_KEY_TITLE: &str = "title";
pub(crate) const ARGUMENT_SIGNAL_KEY_ACTIVE: &str = "active";
pub(crate) const ARGUMENT_SIGNAL_KEY_EVENT: &str = "event";
pub(crate) const ARGUMENT_SIGNAL_KEY_ACTION: &str = "action";
pub(crate) const ARGUMENT_SIGNAL_KEY_LABEL: &str = "label";

pub(crate) const ARGUMENT_SIGNAL_VALUE_YES: &str = "yes";
pub(crate) const ARGUMENT_SIGNAL_VALUE_NO: &str = "no";
/* ----------------------------------------------------------------------------- */

pub(crate) fn handle_domain_signal(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    let command = message_cursor.get_token();
    if token_equals(command, message_cursor.bytes(), COMMAND_SIGNAL_ADD) {
        let mut unsupported_exclusion: Option<usize> = None;
        let mut did_parse = true;
        let mut has_command = false;
        let mut has_signal_type = false;
        let mut signal_type = SignalType::Unknown;
        let mut signal = Signal {
            app: None,
            title: None,
            app_regex_exclude: false,
            title_regex_exclude: false,
            app_regex: None,
            title_regex: None,
            active: SignalProp::Undefined,
            command: None,
            label: None,
        };

        let mut token = message_cursor.get_token();
        while token.is_valid() {
            'iteration: {
                let Some(pair) = parse_key_value_pair(message_cursor.bytes_mut(), token.start)
                else {
                    response.fail_pieces(&[
                        FailurePiece::Text("invalid key-value pair '"),
                        FailurePiece::Bytes(c_string_at(message_cursor.bytes(), token.start)),
                        FailurePiece::Text("'\n"),
                    ]);
                    did_parse = false;
                    break 'iteration;
                };

                let key = c_string_at(message_cursor.bytes(), pair.key).to_vec();
                let value = c_string_at(message_cursor.bytes(), pair.value).to_vec();

                if key == ARGUMENT_SIGNAL_KEY_LABEL.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }
                    signal.label = Some(String::from_utf8_lossy(&value).into_owned());
                } else if key == ARGUMENT_SIGNAL_KEY_APP.as_bytes() {
                    signal.app = Some(String::from_utf8_lossy(&value).into_owned());
                    signal.app_regex_exclude = pair.exclusion;
                    signal.app_regex =
                        PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                    if signal.app_regex.is_none() {
                        daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_TITLE.as_bytes() {
                    signal.title = Some(String::from_utf8_lossy(&value).into_owned());
                    signal.title_regex_exclude = pair.exclusion;
                    signal.title_regex =
                        PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                    if signal.title_regex.is_none() {
                        daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_ACTIVE.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }

                    if value == ARGUMENT_SIGNAL_VALUE_YES.as_bytes() {
                        signal.active = SignalProp::Yes;
                    } else if value == ARGUMENT_SIGNAL_VALUE_NO.as_bytes() {
                        signal.active = SignalProp::No;
                    } else {
                        daemon_fail_with_invalid_value_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_ACTION.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }

                    has_command = true;
                    signal.command = Some(String::from_utf8_lossy(&value).into_owned());
                } else if key == ARGUMENT_SIGNAL_KEY_EVENT.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }

                    has_signal_type = true;
                    signal_type = signal_type_from_string(&value);
                    if signal_type == SignalType::Unknown {
                        daemon_fail_with_invalid_value_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else {
                    response.fail_pieces(&[
                        FailurePiece::Text("unknown key '"),
                        FailurePiece::Bytes(&key),
                        FailurePiece::Text("'\n"),
                    ]);
                    did_parse = false;
                }
            }

            token = message_cursor.get_token();
        }

        if !has_signal_type {
            daemon_fail!(response, "missing required key-value pair 'event=..'\n");
            did_parse = false;
        }

        if !has_command {
            daemon_fail!(response, "missing required key-value pair 'action=..'\n");
            did_parse = false;
        }

        if let Some(unsupported_exclusion) = unsupported_exclusion {
            response.fail_pieces(&[
                FailurePiece::Text("unsupported token '!' (exclusion) given for key '"),
                FailurePiece::Bytes(c_string_at(message_cursor.bytes(), unsupported_exclusion)),
                FailurePiece::Text("'\n"),
            ]);
            did_parse = false;
        }

        if did_parse {
            event_signal_add(signal_type, signal, signal_event);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_SIGNAL_REM) {
        let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
        if let TokenType::Int(int_value) = value.type_of_value {
            if !event_signal_remove_by_index(int_value, signal_event) {
                daemon_fail!(response, "signal with index '{}' not found.\n", int_value);
            }
        } else if let TokenType::String = value.type_of_value {
            if !event_signal_remove(
                c_string_at(message_cursor.bytes(), value.token.start),
                signal_event,
            ) {
                response.fail_pieces(&[
                    FailurePiece::Text("signal with label '"),
                    FailurePiece::Bytes(c_string_at(message_cursor.bytes(), value.token.start)),
                    FailurePiece::Text("' not found.\n"),
                ]);
            }
        } else {
            response.fail_pieces(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(value.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for SIGNAL_SEL\n"),
            ]);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_SIGNAL_LS) {
        event_signal_list(response, signal_event);
    } else {
        daemon_fail_with_unknown_command_for_domain(
            response,
            message_cursor.bytes(),
            command,
            domain,
        );
    }
}
