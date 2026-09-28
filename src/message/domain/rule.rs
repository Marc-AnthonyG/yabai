use std::ffi::CString;

use regex::Regex;

use crate::daemon_fail;
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::manager::DisplayManager;
use crate::message::common_arguments::{ARGUMENT_COMMON_VALUE_OFF, ARGUMENT_COMMON_VALUE_ON};
use crate::message::common_failures::{
    daemon_fail_with_invalid_regex_pattern_for_key, daemon_fail_with_invalid_value_for_key,
    daemon_fail_with_unknown_command_for_domain,
};
use crate::message::domain::window::{
    ARGUMENT_WINDOW_LAYER_ABOVE, ARGUMENT_WINDOW_LAYER_AUTO, ARGUMENT_WINDOW_LAYER_BELOW,
    ARGUMENT_WINDOW_LAYER_NORMAL,
};
use crate::message::labels::RESERVED_WINDOW_IDENTIFIERS;
use crate::message::selectors::{parse_display_selector, parse_space_selector};
use crate::message::token::{
    MessageCursor, Token, TokenValueType, is_token_equal_to, null_terminated_bytes_starting_at,
    parse_token_into_typed_value, split_token_into_key_value_pair_in_place,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::serialise::rule::write_every_rule_as_json_array;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::arithmetic::is_within_range_including_both_bounds;
use crate::support::layer::{LAYER_ABOVE, LAYER_AUTO, LAYER_BELOW, LAYER_NORMAL};
use crate::support::response::{FailurePiece, Response};
use crate::window::manager::WindowManager;
use crate::window::rule::{
    RULE_PROPERTY_OFF, RULE_PROPERTY_ON, Rule, RuleEffectsFlag, RuleFlag,
    add_rule_replacing_any_with_the_same_label, remove_rule_at_index, remove_rule_with_label,
};
use crate::window::rule_application::{
    apply_rule_to_every_matching_root_window,
    reapply_every_rule_except_one_shot_rules_to_every_root_window,
    reapply_rule_at_index_to_every_root_window, reapply_rule_with_label_to_every_root_window,
};

/* --------------------------------DOMAIN RULE---------------------------------- */
pub(crate) const COMMAND_RULE_ADD: &str = "--add";
pub(crate) const COMMAND_RULE_REMOVE: &str = "--remove";
pub(crate) const COMMAND_RULE_APPLY: &str = "--apply";
pub(crate) const COMMAND_RULE_LIST: &str = "--list";

pub(crate) const ARGUMENT_RULE_ONE_SHOT: &str = "--one-shot";
pub(crate) const ARGUMENT_RULE_KEY_APPLICATION: &str = "app";
pub(crate) const ARGUMENT_RULE_KEY_TITLE: &str = "title";
pub(crate) const ARGUMENT_RULE_KEY_ROLE: &str = "role";
pub(crate) const ARGUMENT_RULE_KEY_SUBROLE: &str = "subrole";
pub(crate) const ARGUMENT_RULE_KEY_DISPLAY: &str = "display";
pub(crate) const ARGUMENT_RULE_KEY_SPACE: &str = "space";
pub(crate) const ARGUMENT_RULE_KEY_OPACITY: &str = "opacity";
pub(crate) const ARGUMENT_RULE_KEY_MANAGE: &str = "manage";
pub(crate) const ARGUMENT_RULE_KEY_STICKY: &str = "sticky";
pub(crate) const ARGUMENT_RULE_KEY_MOUSE_FOLLOWS_FOCUS: &str = "mouse_follows_focus";
pub(crate) const ARGUMENT_RULE_KEY_SUB_LAYER: &str = "sub-layer";
pub(crate) const ARGUMENT_RULE_KEY_NATIVE_FULLSCREEN: &str = "native-fullscreen";
pub(crate) const ARGUMENT_RULE_KEY_GRID: &str = "grid";
pub(crate) const ARGUMENT_RULE_KEY_LABEL: &str = "label";
pub(crate) const ARGUMENT_RULE_KEY_SCRATCHPAD: &str = "scratchpad";

pub(crate) const ARGUMENT_RULE_VALUE_SPACE: u8 = b'^';
pub(crate) const ARGUMENT_RULE_VALUE_GRID: &std::ffi::CStr = c"%d:%d:%d:%d:%d:%d";
/* ----------------------------------------------------------------------------- */

pub(crate) const ONE_FLOAT_SCANF_FORMAT: &std::ffi::CStr = c"%f";

pub(crate) fn parse_rule(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
    rule: &mut Rule,
    token: Token,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let mut unsupported_exclusion: Option<usize> = None;
    let mut did_parse = true;
    let mut has_filter = false;

    let mut token = token;
    while token.is_not_empty() {
        'iteration: {
            let Some(pair) =
                split_token_into_key_value_pair_in_place(message_cursor.bytes_mut(), token.start)
            else {
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

            let key = null_terminated_bytes_starting_at(message_cursor.bytes(), pair.key).to_vec();
            let value =
                null_terminated_bytes_starting_at(message_cursor.bytes(), pair.value).to_vec();

            if key == ARGUMENT_RULE_KEY_LABEL.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }
                rule.label = Some(String::from_utf8_lossy(&value).into_owned());
            } else if key == ARGUMENT_RULE_KEY_SCRATCHPAD.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let mut valid = true;
                for index in 0..RESERVED_WINDOW_IDENTIFIERS.len() {
                    if value == RESERVED_WINDOW_IDENTIFIERS[index].as_bytes() {
                        valid = false;
                        break;
                    }
                }

                if valid {
                    rule.effects.scratchpad = Some(String::from_utf8_lossy(&value).into_owned());
                    rule.effects.manage = RULE_PROPERTY_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_APPLICATION.as_bytes() {
                has_filter = true;
                rule.app = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::APPLICATION_PATTERN_IS_NEGATED;
                }
                rule.app_regex = Regex::new(&String::from_utf8_lossy(&value)).ok();
                if rule.app_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_TITLE.as_bytes() {
                has_filter = true;
                rule.title = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::TITLE_PATTERN_IS_NEGATED;
                }
                rule.title_regex = Regex::new(&String::from_utf8_lossy(&value)).ok();
                if rule.title_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_ROLE.as_bytes() {
                has_filter = true;
                rule.role = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::ROLE_PATTERN_IS_NEGATED;
                }
                rule.role_regex = Regex::new(&String::from_utf8_lossy(&value)).ok();
                if rule.role_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_SUBROLE.as_bytes() {
                has_filter = true;
                rule.subrole = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::SUBROLE_PATTERN_IS_NEGATED;
                }
                rule.subrole_regex = Regex::new(&String::from_utf8_lossy(&value)).ok();
                if rule.subrole_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_DISPLAY.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let mut value_start = pair.value;
                if message_cursor.bytes()[value_start] == ARGUMENT_RULE_VALUE_SPACE {
                    value_start += 1;
                    rule.effects.flags |= RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE;
                }

                let acting_display_id = query_display_showing_the_active_menu_bar();
                let mut value_cursor = message_cursor.cursor_at(value_start);
                let selector = parse_display_selector(
                    response,
                    &mut value_cursor,
                    acting_display_id,
                    false,
                    display_manager,
                );
                if let Some(selector_display_id) = selector.resolved_target() {
                    rule.effects.display_id = selector_display_id;
                } else {
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_SPACE.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let mut value_start = pair.value;
                if message_cursor.bytes()[value_start] == ARGUMENT_RULE_VALUE_SPACE {
                    value_start += 1;
                    rule.effects.flags |= RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE;
                }

                let acting_space_id = query_current_space_of_the_focused_display(window_manager);
                let mut value_cursor = message_cursor.cursor_at(value_start);
                let selector = parse_space_selector(
                    response,
                    &mut value_cursor,
                    acting_space_id,
                    false,
                    space_manager,
                );
                if let Some(selector_space_id) = selector.resolved_target() {
                    rule.effects.space_id = selector_space_id;
                } else {
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_GRID.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let subject = CString::new(value.clone()).unwrap_or_default();
                let grid = rule.effects.grid.as_mut_ptr() as *mut libc::c_int;
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ARGUMENT_RULE_VALUE_GRID.as_ptr(),
                        grid,
                        grid.add(1),
                        grid.add(2),
                        grid.add(3),
                        grid.add(4),
                        grid.add(5),
                    )
                };
                if converted != 6 {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_OPACITY.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let subject = CString::new(value.clone()).unwrap_or_default();
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ONE_FLOAT_SCANF_FORMAT.as_ptr(),
                        &mut rule.effects.opacity as *mut libc::c_float,
                    )
                };
                if converted == 1
                    && is_within_range_including_both_bounds(rule.effects.opacity, 0.0f32, 1.0f32)
                {
                    rule.effects.flags |= RuleEffectsFlag::OPACITY_IS_SET;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_MANAGE.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VALUE_ON.as_bytes() {
                    rule.effects.manage = RULE_PROPERTY_ON;
                } else if value == ARGUMENT_COMMON_VALUE_OFF.as_bytes() {
                    rule.effects.manage = RULE_PROPERTY_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_STICKY.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VALUE_ON.as_bytes() {
                    rule.effects.sticky = RULE_PROPERTY_ON;
                } else if value == ARGUMENT_COMMON_VALUE_OFF.as_bytes() {
                    rule.effects.sticky = RULE_PROPERTY_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_MOUSE_FOLLOWS_FOCUS.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VALUE_ON.as_bytes() {
                    rule.effects.mff = RULE_PROPERTY_ON;
                } else if value == ARGUMENT_COMMON_VALUE_OFF.as_bytes() {
                    rule.effects.mff = RULE_PROPERTY_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_SUB_LAYER.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_WINDOW_LAYER_BELOW.as_bytes() {
                    rule.effects.layer = LAYER_BELOW;
                    rule.effects.flags |= RuleEffectsFlag::LAYER_IS_SET;
                } else if value == ARGUMENT_WINDOW_LAYER_NORMAL.as_bytes() {
                    rule.effects.layer = LAYER_NORMAL;
                    rule.effects.flags |= RuleEffectsFlag::LAYER_IS_SET;
                } else if value == ARGUMENT_WINDOW_LAYER_ABOVE.as_bytes() {
                    rule.effects.layer = LAYER_ABOVE;
                    rule.effects.flags |= RuleEffectsFlag::LAYER_IS_SET;
                } else if value == ARGUMENT_WINDOW_LAYER_AUTO.as_bytes() {
                    rule.effects.layer = LAYER_AUTO;
                    rule.effects.flags |= RuleEffectsFlag::LAYER_IS_SET;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_NATIVE_FULLSCREEN.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VALUE_ON.as_bytes() {
                    rule.effects.fullscreen = RULE_PROPERTY_ON;
                } else if value == ARGUMENT_COMMON_VALUE_OFF.as_bytes() {
                    rule.effects.fullscreen = RULE_PROPERTY_OFF;
                } else {
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

    if !has_filter {
        daemon_fail!(
            response,
            "missing required key-value pair 'app[!]=..' or 'title[!]=..'\n"
        );
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

    did_parse
}

pub(crate) fn run_rule_command(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let command = message_cursor.take_next_token();
    if is_token_equal_to(command, message_cursor.bytes(), COMMAND_RULE_ADD) {
        let mut rule = Rule::default();

        let mut token = message_cursor.take_next_token();
        if is_token_equal_to(token, message_cursor.bytes(), ARGUMENT_RULE_ONE_SHOT) {
            rule.flags |= RuleFlag::ONE_SHOT;
            token = message_cursor.take_next_token();
        }

        if parse_rule(
            response,
            message_cursor,
            &mut rule,
            token,
            display_manager,
            window_manager,
            space_manager,
        ) {
            add_rule_replacing_any_with_the_same_label(rule, window_manager);
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_RULE_APPLY) {
        let value =
            parse_token_into_typed_value(message_cursor.take_next_token(), message_cursor.bytes());
        if let TokenValueType::Integer(int_value) = value.type_of_value {
            if !reapply_rule_at_index_to_every_root_window(
                int_value,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            ) {
                daemon_fail!(response, "rule with index '{}' not found.\n", int_value);
            }
        } else if let TokenValueType::String = value.type_of_value {
            if !reapply_rule_with_label_to_every_root_window(
                null_terminated_bytes_starting_at(message_cursor.bytes(), value.token.start),
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            ) {
                let mut rule = Rule::default();
                if parse_rule(
                    response,
                    message_cursor,
                    &mut rule,
                    value.token,
                    display_manager,
                    window_manager,
                    space_manager,
                ) {
                    apply_rule_to_every_matching_root_window(
                        &rule,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    );
                }
            }
        } else if let TokenValueType::Invalid = value.type_of_value {
            reapply_every_rule_except_one_shot_rules_to_every_root_window(
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            );
        } else {
            response.write_failure_pieces_unless_silent(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(value.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for RULE_SEL\n"),
            ]);
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_RULE_REMOVE) {
        let value =
            parse_token_into_typed_value(message_cursor.take_next_token(), message_cursor.bytes());
        if let TokenValueType::Integer(int_value) = value.type_of_value {
            if !remove_rule_at_index(int_value, window_manager) {
                daemon_fail!(response, "rule with index '{}' not found.\n", int_value);
            }
        } else if let TokenValueType::String = value.type_of_value {
            if !remove_rule_with_label(
                null_terminated_bytes_starting_at(message_cursor.bytes(), value.token.start),
                window_manager,
            ) {
                response.write_failure_pieces_unless_silent(&[
                    FailurePiece::Text("rule with label '"),
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
                FailurePiece::Text("' is not a valid option for RULE_SEL\n"),
            ]);
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_RULE_LIST) {
        write_every_rule_as_json_array(response, display_manager, window_manager);
    } else {
        daemon_fail_with_unknown_command_for_domain(
            response,
            message_cursor.bytes(),
            command,
            domain,
        );
    }
}
