use regex::Regex;

use crate::daemon_fail;
use crate::message::common_failures::{
    daemon_fail_with_invalid_regex_pattern_for_key, daemon_fail_with_invalid_value_for_key,
    daemon_fail_with_unknown_command_for_domain,
};
use crate::message::token::{
    MessageCursor, Token, TokenValueType, is_token_equal_to, null_terminated_bytes_starting_at,
    parse_token_into_typed_value, split_token_into_key_value_pair_in_place,
};
use crate::serialise::signal::write_every_signal_as_json_array;
use crate::signal::definition::{
    SIGNAL_TYPE_COUNT, Signal, SignalPropertyRequirement, SignalType,
    add_signal_replacing_any_with_the_same_label, remove_signal_at_listing_index,
    remove_signal_with_label, signal_type_for_event_name,
};
use crate::support::response::{FailurePiece, Response};

/* --------------------------------DOMAIN SIGNAL-------------------------------- */
pub(crate) const COMMAND_SIGNAL_ADD: &str = "--add";
pub(crate) const COMMAND_SIGNAL_REMOVE: &str = "--remove";
pub(crate) const COMMAND_SIGNAL_LIST: &str = "--list";

pub(crate) const ARGUMENT_SIGNAL_KEY_APPLICATION: &str = "app";
pub(crate) const ARGUMENT_SIGNAL_KEY_TITLE: &str = "title";
pub(crate) const ARGUMENT_SIGNAL_KEY_ACTIVE: &str = "active";
pub(crate) const ARGUMENT_SIGNAL_KEY_EVENT: &str = "event";
pub(crate) const ARGUMENT_SIGNAL_KEY_ACTION: &str = "action";
pub(crate) const ARGUMENT_SIGNAL_KEY_LABEL: &str = "label";

pub(crate) const ARGUMENT_SIGNAL_VALUE_YES: &str = "yes";
pub(crate) const ARGUMENT_SIGNAL_VALUE_NO: &str = "no";
/* ----------------------------------------------------------------------------- */

pub(crate) fn run_signal_command(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    let command = message_cursor.take_next_token();
    if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SIGNAL_ADD) {
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
            active: SignalPropertyRequirement::Undefined,
            command: None,
            label: None,
        };

        let mut token = message_cursor.take_next_token();
        while token.is_not_empty() {
            'iteration: {
                let Some(pair) = split_token_into_key_value_pair_in_place(
                    message_cursor.bytes_mut(),
                    token.start,
                ) else {
                    response.write_failure_pieces_unless_silent(&[
                        FailurePiece::Text("invalid key-value pair '"),
                        FailurePiece::Bytes(null_terminated_bytes_starting_at(
                            message_cursor.bytes(),
                            token.start,
                        )),
                        FailurePiece::Text("'\n"),
                    ]);
                    did_parse = false;
                    break 'iteration;
                };

                let key =
                    null_terminated_bytes_starting_at(message_cursor.bytes(), pair.key).to_vec();
                let value =
                    null_terminated_bytes_starting_at(message_cursor.bytes(), pair.value).to_vec();

                if key == ARGUMENT_SIGNAL_KEY_LABEL.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }
                    signal.label = Some(String::from_utf8_lossy(&value).into_owned());
                } else if key == ARGUMENT_SIGNAL_KEY_APPLICATION.as_bytes() {
                    signal.app = Some(String::from_utf8_lossy(&value).into_owned());
                    signal.app_regex_exclude = pair.exclusion;
                    signal.app_regex = Regex::new(&String::from_utf8_lossy(&value)).ok();
                    if signal.app_regex.is_none() {
                        daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_TITLE.as_bytes() {
                    signal.title = Some(String::from_utf8_lossy(&value).into_owned());
                    signal.title_regex_exclude = pair.exclusion;
                    signal.title_regex = Regex::new(&String::from_utf8_lossy(&value)).ok();
                    if signal.title_regex.is_none() {
                        daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_ACTIVE.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }

                    if value == ARGUMENT_SIGNAL_VALUE_YES.as_bytes() {
                        signal.active = SignalPropertyRequirement::Yes;
                    } else if value == ARGUMENT_SIGNAL_VALUE_NO.as_bytes() {
                        signal.active = SignalPropertyRequirement::No;
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
                    signal_type = signal_type_for_event_name(&value);
                    if signal_type == SignalType::Unknown {
                        daemon_fail_with_invalid_value_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else {
                    response.write_failure_pieces_unless_silent(&[
                        FailurePiece::Text("unknown key '"),
                        FailurePiece::Bytes(&key),
                        FailurePiece::Text("'\n"),
                    ]);
                    did_parse = false;
                }
            }

            token = message_cursor.take_next_token();
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
            response.write_failure_pieces_unless_silent(&[
                FailurePiece::Text("unsupported token '!' (exclusion) given for key '"),
                FailurePiece::Bytes(null_terminated_bytes_starting_at(
                    message_cursor.bytes(),
                    unsupported_exclusion,
                )),
                FailurePiece::Text("'\n"),
            ]);
            did_parse = false;
        }

        if did_parse {
            add_signal_replacing_any_with_the_same_label(signal_type, signal, signal_event);
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SIGNAL_REMOVE) {
        let value =
            parse_token_into_typed_value(message_cursor.take_next_token(), message_cursor.bytes());
        if let TokenValueType::Integer(int_value) = value.type_of_value {
            if !remove_signal_at_listing_index(int_value, signal_event) {
                daemon_fail!(response, "signal with index '{}' not found.\n", int_value);
            }
        } else if let TokenValueType::String = value.type_of_value {
            if !remove_signal_with_label(
                null_terminated_bytes_starting_at(message_cursor.bytes(), value.token.start),
                signal_event,
            ) {
                response.write_failure_pieces_unless_silent(&[
                    FailurePiece::Text("signal with label '"),
                    FailurePiece::Bytes(null_terminated_bytes_starting_at(
                        message_cursor.bytes(),
                        value.token.start,
                    )),
                    FailurePiece::Text("' not found.\n"),
                ]);
            }
        } else {
            response.write_failure_pieces_unless_silent(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(value.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for SIGNAL_SEL\n"),
            ]);
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SIGNAL_LIST) {
        write_every_signal_as_json_array(response, signal_event);
    } else {
        daemon_fail_with_unknown_command_for_domain(
            response,
            message_cursor.bytes(),
            command,
            domain,
        );
    }
}
