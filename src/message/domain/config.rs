use std::sync::atomic::Ordering;

use crate::daemon_fail;
use crate::display::manager::{
    DISPLAY_ARRANGEMENT_ORDER_STR, DisplayArrangementOrder, DisplayManager, EXTERNAL_BAR_MODE_STR,
    ExternalBarMode,
};
use crate::ffi::core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use crate::layout::insertion::{WINDOW_INSERTION_POINT_STR, WindowInsertionPoint};
use crate::layout::settings::{AUTO_BALANCE_STR, VIEW_TYPE_STR, ViewFlag, ViewType};
use crate::layout::tree::{
    WINDOW_NODE_CHILD_STR, WINDOW_NODE_SPLIT_STR, WindowNodeChild, WindowNodeSplit,
};
use crate::layout::view::{view_clear, view_flush, view_update};
use crate::message::common_arguments::{
    ARGUMENT_COMMON_VAL_AXIS_X, ARGUMENT_COMMON_VAL_AXIS_Y, ARGUMENT_COMMON_VAL_OFF,
    ARGUMENT_COMMON_VAL_ON,
};
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_value_given_to_command_for_domain,
};
use crate::message::selectors::parse_space_selector;
use crate::message::token::{
    MessageCursor, Token, TokenType, c_string_at, token_equals, token_to_value,
};
use crate::mouse::drag::MouseDragState;
use crate::mouse::tap::{MOUSE_MOD_STR, MOUSE_MODE_STR, MOUSE_TAP_STATE, MouseMod, MouseMode};
use crate::scripting_addition::installer::scripting_addition_is_sip_friendly;
use crate::space::managed_space::space_is_user;
use crate::space::manager::{
    SpaceManager, space_manager_find_view, space_manager_mark_spaces_invalid,
};
use crate::space::view_settings::{
    space_manager_set_auto_balance_for_all_spaces, space_manager_set_bottom_padding_for_all_spaces,
    space_manager_set_layout_for_all_spaces, space_manager_set_left_padding_for_all_spaces,
    space_manager_set_right_padding_for_all_spaces, space_manager_set_split_type_for_all_spaces,
    space_manager_set_top_padding_for_all_spaces, space_manager_set_window_gap_for_all_spaces,
};
use crate::state::process_wide::VERBOSE;
use crate::support::arithmetic::{in_range_ei, in_range_ii};
use crate::support::color::rgba_color_from_hex;
use crate::support::easing::{ANIMATION_EASING_TYPE_STR, AnimationEasingType, EASING_TYPE_COUNT};
use crate::support::handles::SpaceId;
use crate::support::printf_float_format::format_float_with_decimals_as_printf_does;
use crate::support::response::{FailurePiece, Response};
use crate::support::strings::BOOL_STR;
use crate::window::focus::window_manager_set_focus_follows_mouse;
use crate::window::manager::{
    FFM_MODE_STR, FfmMode, PURIFY_MODE_STR, PurifyMode, WINDOW_ORIGIN_MODE_STR, WindowManager,
    WindowOriginMode,
};
use crate::window::opacity::{
    window_manager_set_active_window_opacity, window_manager_set_menubar_opacity,
    window_manager_set_normal_window_opacity, window_manager_set_window_opacity_enabled,
};
use crate::window::shadow::window_manager_set_purify_mode;
use crate::window::space_reconciliation::window_manager_validate_and_check_for_windows_on_space;

/* --------------------------------DOMAIN CONFIG-------------------------------- */
pub(crate) const COMMAND_CONFIG_DEBUG_OUTPUT: &str = "debug_output";
pub(crate) const COMMAND_CONFIG_MFF: &str = "mouse_follows_focus";
pub(crate) const COMMAND_CONFIG_FFM: &str = "focus_follows_mouse";
pub(crate) const COMMAND_CONFIG_DISPLAY_ORDER: &str = "display_arrangement_order";
pub(crate) const COMMAND_CONFIG_WINDOW_ORIGIN: &str = "window_origin_display";
pub(crate) const COMMAND_CONFIG_WINDOW_PLACEMENT: &str = "window_placement";
pub(crate) const COMMAND_CONFIG_WINDOW_INSERT_POINT: &str = "window_insertion_point";
pub(crate) const COMMAND_CONFIG_WINDOW_ZOOM_PERSIST: &str = "window_zoom_persist";
pub(crate) const COMMAND_CONFIG_OPACITY: &str = "window_opacity";
pub(crate) const COMMAND_CONFIG_OPACITY_DURATION: &str = "window_opacity_duration";
pub(crate) const COMMAND_CONFIG_ANIMATION_DURATION: &str = "window_animation_duration";
pub(crate) const COMMAND_CONFIG_ANIMATION_EASING: &str = "window_animation_easing";
pub(crate) const COMMAND_CONFIG_SHADOW: &str = "window_shadow";
pub(crate) const COMMAND_CONFIG_MENUBAR_OPACITY: &str = "menubar_opacity";
pub(crate) const COMMAND_CONFIG_ACTIVE_WINDOW_OPACITY: &str = "active_window_opacity";
pub(crate) const COMMAND_CONFIG_NORMAL_WINDOW_OPACITY: &str = "normal_window_opacity";
pub(crate) const COMMAND_CONFIG_INSERT_FEEDBACK_COLOR: &str = "insert_feedback_color";
pub(crate) const COMMAND_CONFIG_TOP_PADDING: &str = "top_padding";
pub(crate) const COMMAND_CONFIG_BOTTOM_PADDING: &str = "bottom_padding";
pub(crate) const COMMAND_CONFIG_LEFT_PADDING: &str = "left_padding";
pub(crate) const COMMAND_CONFIG_RIGHT_PADDING: &str = "right_padding";
pub(crate) const COMMAND_CONFIG_LAYOUT: &str = "layout";
pub(crate) const COMMAND_CONFIG_WINDOW_GAP: &str = "window_gap";
pub(crate) const COMMAND_CONFIG_SPLIT_RATIO: &str = "split_ratio";
pub(crate) const COMMAND_CONFIG_SPLIT_TYPE: &str = "split_type";
pub(crate) const COMMAND_CONFIG_AUTO_BALANCE: &str = "auto_balance";
pub(crate) const COMMAND_CONFIG_MOUSE_MOD: &str = "mouse_modifier";
pub(crate) const COMMAND_CONFIG_MOUSE_ACTION1: &str = "mouse_action1";
pub(crate) const COMMAND_CONFIG_MOUSE_ACTION2: &str = "mouse_action2";
pub(crate) const COMMAND_CONFIG_MOUSE_DROP_ACTION: &str = "mouse_drop_action";
pub(crate) const COMMAND_CONFIG_EXTERNAL_BAR: &str = "external_bar";
pub(crate) const COMMAND_CONFIG_SKIP_SPACE_ANIMATION: &str = "skip_window_focus_animation";

pub(crate) const SELECTOR_CONFIG_SPACE: &str = "--space";

pub(crate) const ARGUMENT_CONFIG_FFM_AUTOFOCUS: &str = "autofocus";
pub(crate) const ARGUMENT_CONFIG_FFM_AUTORAISE: &str = "autoraise";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_DEFAULT: &str = "default";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_X: &str = "horizontal";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_Y: &str = "vertical";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_DEFAULT: &str = "default";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_FOCUSED: &str = "focused";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_CURSOR: &str = "cursor";
pub(crate) const ARGUMENT_CONFIG_WINDOW_PLACEMENT_FST: &str = "first_child";
pub(crate) const ARGUMENT_CONFIG_WINDOW_PLACEMENT_SND: &str = "second_child";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_FOCUSED: &str = "focused";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_FIRST: &str = "first";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_LAST: &str = "last";
pub(crate) const ARGUMENT_CONFIG_SHADOW_FLT: &str = "float";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_BSP: &str = "bsp";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_STACK: &str = "stack";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_FLOAT: &str = "float";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_Y: &str = "vertical";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_X: &str = "horizontal";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_AUTO: &str = "auto";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_ALT: &str = "alt";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_SHIFT: &str = "shift";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_CMD: &str = "cmd";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_CTRL: &str = "ctrl";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_FN: &str = "fn";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_MOVE: &str = "move";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_RESIZE: &str = "resize";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_SWAP: &str = "swap";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_STACK: &str = "stack";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR_MAIN: &str = "main";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR_ALL: &str = "all";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR: &std::ffi::CStr = c"%5[^:]:%d:%d";
/* ----------------------------------------------------------------------------- */

pub(crate) fn handle_domain_config(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut selector_space_id = SpaceId(0);
    let selector = message_cursor.get_token();
    let mut command = selector;

    let found_selector = token_equals(selector, message_cursor.bytes(), SELECTOR_CONFIG_SPACE);
    if found_selector {
        let space_selector =
            parse_space_selector(response, message_cursor, SpaceId(0), false, space_manager);
        let Some(space_id) = space_selector.resolved() else {
            return;
        };

        selector_space_id = space_id;
        command = message_cursor.get_token();
    }

    while command.is_valid() {
        if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_DEBUG_OUTPUT) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[VERBOSE.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                VERBOSE.store(false, Ordering::Relaxed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                VERBOSE.store(true, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MFF) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[window_manager.enable_mff as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager.enable_mff = false;
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                window_manager.enable_mff = true;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_FFM) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    FFM_MODE_STR[window_manager.ffm_mode as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager_set_focus_follows_mouse(window_manager, FfmMode::Disabled);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_FFM_AUTOFOCUS,
            ) {
                window_manager_set_focus_follows_mouse(window_manager, FfmMode::Autofocus);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_FFM_AUTORAISE,
            ) {
                window_manager_set_focus_follows_mouse(window_manager, FfmMode::Autoraise);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_DISPLAY_ORDER) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    DISPLAY_ARRANGEMENT_ORDER_STR[display_manager.order as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_DEFAULT,
            ) {
                display_manager.order = DisplayArrangementOrder::Default;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_X,
            ) {
                display_manager.order = DisplayArrangementOrder::X;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_Y,
            ) {
                display_manager.order = DisplayArrangementOrder::Y;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_WINDOW_ORIGIN) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_ORIGIN_MODE_STR[window_manager.window_origin_mode as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_DEFAULT,
            ) {
                window_manager.window_origin_mode = WindowOriginMode::Default;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_FOCUSED,
            ) {
                window_manager.window_origin_mode = WindowOriginMode::Focused;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_CURSOR,
            ) {
                window_manager.window_origin_mode = WindowOriginMode::Cursor;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_PLACEMENT,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_NODE_CHILD_STR[space_manager.window_placement as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_PLACEMENT_FST,
            ) {
                space_manager.window_placement = WindowNodeChild::First;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_PLACEMENT_SND,
            ) {
                space_manager.window_placement = WindowNodeChild::Second;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_INSERT_POINT,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_INSERTION_POINT_STR[space_manager.window_insertion_point as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_FOCUSED,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::Focused;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_FIRST,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::First;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_LAST,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::Last;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_ZOOM_PERSIST,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[space_manager.window_zoom_persist as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                space_manager.window_zoom_persist = false;
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                space_manager.window_zoom_persist = true;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_SKIP_SPACE_ANIMATION,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[space_manager.skip_window_focus_animation as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                space_manager.skip_window_focus_animation = false;
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                space_manager.skip_window_focus_animation = true;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_OPACITY) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[window_manager.enable_window_opacity as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager_set_window_opacity_enabled(window_manager, false);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                window_manager_set_window_opacity_enabled(window_manager, true);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_OPACITY_DURATION,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.window_opacity_duration as f64,
                        6
                    )
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value {
                window_manager.window_opacity_duration = float_value;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ANIMATION_DURATION,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.window_animation_duration as f64,
                        6
                    )
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && float_value.is_finite()
                && float_value >= 0.0f32
            {
                if float_value == 0.0f32 {
                    window_manager.window_animation_duration = float_value;
                } else if !scripting_addition_is_sip_friendly() {
                    response.fail_pieces(&[
                        FailurePiece::Text("command '"),
                        FailurePiece::Bytes(command.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' for domain '"),
                        FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
                        FailurePiece::Text(
                            "' requires System Integrity Protection to be partially disabled! ignoring request..\n",
                        ),
                    ]);
                } else if CGPreflightScreenCaptureAccess() {
                    window_manager.window_animation_duration = float_value;
                } else {
                    response.fail_pieces(&[
                        FailurePiece::Text("command '"),
                        FailurePiece::Bytes(command.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' for domain '"),
                        FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
                        FailurePiece::Text(
                            "' requires Screen Recording permissions! ignoring request..\n",
                        ),
                    ]);
                    CGRequestScreenCaptureAccess();
                }
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ANIMATION_EASING,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    ANIMATION_EASING_TYPE_STR[window_manager.window_animation_easing as usize]
                ));
            } else {
                let mut found_match = false;
                for index in 0..EASING_TYPE_COUNT {
                    if token_equals(value, message_cursor.bytes(), ANIMATION_EASING_TYPE_STR[index])
                    {
                        if let Some(easing) = AnimationEasingType::from_index(index) {
                            window_manager.window_animation_easing = easing;
                        }
                        found_match = true;
                        break;
                    }
                }
                if !found_match {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_SHADOW) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    PURIFY_MODE_STR[window_manager.purify_mode as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager_set_purify_mode(window_manager, PurifyMode::Always);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_SHADOW_FLT) {
                window_manager_set_purify_mode(window_manager, PurifyMode::Managed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                window_manager_set_purify_mode(window_manager, PurifyMode::Disabled);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MENUBAR_OPACITY,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.menubar_opacity as f64,
                        4
                    )
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ii(float_value, 0.0f32, 1.0f32)
            {
                window_manager_set_menubar_opacity(window_manager, float_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ACTIVE_WINDOW_OPACITY,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.active_window_opacity as f64,
                        4
                    )
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ei(float_value, 0.0f32, 1.0f32)
            {
                window_manager_set_active_window_opacity(window_manager, float_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_NORMAL_WINDOW_OPACITY,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.normal_window_opacity as f64,
                        4
                    )
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ei(float_value, 0.0f32, 1.0f32)
            {
                window_manager_set_normal_window_opacity(window_manager, float_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_INSERT_FEEDBACK_COLOR,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "0x{:x}\n",
                    window_manager.insert_feedback_color.packed
                ));
            } else if let TokenType::U32(u32_value) = value.type_of_value
                && u32_value != 0
            {
                window_manager.insert_feedback_color = rgba_color_from_hex(u32_value);
                window_manager.insert_feedback_color_follows_the_system_accent_color = false;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_TOP_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.top_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::TOP_PADDING);
                        view.top_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
                    } else {
                        daemon_fail_with_unknown_value_given_to_command_for_domain(
                            response,
                            message_cursor.bytes(),
                            value.token,
                            command,
                            domain,
                        );
                    }
                }
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.top_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_top_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_BOTTOM_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.bottom_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::BOTTOM_PADDING);
                        view.bottom_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
                    } else {
                        daemon_fail_with_unknown_value_given_to_command_for_domain(
                            response,
                            message_cursor.bytes(),
                            value.token,
                            command,
                            domain,
                        );
                    }
                }
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.bottom_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_bottom_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_LEFT_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.left_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::LEFT_PADDING);
                        view.left_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
                    } else {
                        daemon_fail_with_unknown_value_given_to_command_for_domain(
                            response,
                            message_cursor.bytes(),
                            value.token,
                            command,
                            domain,
                        );
                    }
                }
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.left_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_left_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_RIGHT_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.right_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::RIGHT_PADDING);
                        view.right_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
                    } else {
                        daemon_fail_with_unknown_value_given_to_command_for_domain(
                            response,
                            message_cursor.bytes(),
                            value.token,
                            command,
                            domain,
                        );
                    }
                }
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.right_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_right_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_WINDOW_GAP) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.window_gap));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::WINDOW_GAP);
                        view.window_gap = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
                    } else {
                        daemon_fail_with_unknown_value_given_to_command_for_domain(
                            response,
                            message_cursor.bytes(),
                            value.token,
                            command,
                            domain,
                        );
                    }
                }
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.window_gap));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_window_gap_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_LAYOUT) {
            let value = message_cursor.get_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if !value.is_valid() {
                    if let Some(view) = space_manager.view.find(&view_space_id) {
                        response.write(format_args!("{}\n", VIEW_TYPE_STR[view.layout as usize]));
                    }
                } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_BSP) {
                    if space_is_user(selector_space_id) {
                        if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                            view.set_flag(ViewFlag::LAYOUT);
                            view.layout = ViewType::Bsp;
                        }
                        view_clear(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                        window_manager_validate_and_check_for_windows_on_space(
                            space_manager,
                            window_manager,
                            selector_space_id,
                            display_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                    }
                } else if token_equals(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_CONFIG_LAYOUT_STACK,
                ) {
                    if space_is_user(selector_space_id) {
                        if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                            view.set_flag(ViewFlag::LAYOUT);
                            view.layout = ViewType::Stack;
                        }
                        view_clear(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                        window_manager_validate_and_check_for_windows_on_space(
                            space_manager,
                            window_manager,
                            selector_space_id,
                            display_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                    }
                } else if token_equals(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_CONFIG_LAYOUT_FLOAT,
                ) {
                    if space_is_user(selector_space_id) {
                        if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                            view.set_flag(ViewFlag::LAYOUT);
                            view.layout = ViewType::Float;
                        }
                        view_clear(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                    }
                } else {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            } else if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    VIEW_TYPE_STR[space_manager.layout as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_BSP) {
                space_manager_set_layout_for_all_spaces(
                    space_manager,
                    ViewType::Bsp,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_STACK) {
                space_manager_set_layout_for_all_spaces(
                    space_manager,
                    ViewType::Stack,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_FLOAT) {
                space_manager_set_layout_for_all_spaces(
                    space_manager,
                    ViewType::Float,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_SPLIT_RATIO) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(space_manager.split_ratio as f64, 4)
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ii(float_value, 0.1f32, 0.9f32)
            {
                space_manager.split_ratio = float_value;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_SPLIT_TYPE) {
            let value = message_cursor.get_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if !value.is_valid() {
                        response.write(format_args!(
                            "{}\n",
                            WINDOW_NODE_SPLIT_STR[view.split_type as usize]
                        ));
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_Y,
                    ) {
                        view.set_flag(ViewFlag::SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::Y;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_X,
                    ) {
                        view.set_flag(ViewFlag::SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::X;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_AUTO,
                    ) {
                        view.set_flag(ViewFlag::SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::Auto;
                    } else {
                        daemon_fail_with_unknown_value_given_to_command_for_domain(
                            response,
                            message_cursor.bytes(),
                            value,
                            command,
                            domain,
                        );
                    }
                }
            } else if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_NODE_SPLIT_STR[space_manager.split_type as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_SPLIT_TYPE_Y) {
                space_manager_set_split_type_for_all_spaces(space_manager, WindowNodeSplit::Y);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_SPLIT_TYPE_X) {
                space_manager_set_split_type_for_all_spaces(space_manager, WindowNodeSplit::X);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_SPLIT_TYPE_AUTO,
            ) {
                space_manager_set_split_type_for_all_spaces(space_manager, WindowNodeSplit::Auto);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_AUTO_BALANCE) {
            let value = message_cursor.get_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if !value.is_valid() {
                        response.write(format_args!(
                            "{}\n",
                            AUTO_BALANCE_STR[view.auto_balance as usize]
                        ));
                    } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::None as u32;
                    } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VAL_AXIS_X,
                    ) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::X as u32;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VAL_AXIS_Y,
                    ) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::Y as u32;
                    } else {
                        daemon_fail_with_unknown_value_given_to_command_for_domain(
                            response,
                            message_cursor.bytes(),
                            value,
                            command,
                            domain,
                        );
                    }
                }
            } else if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    AUTO_BALANCE_STR[space_manager.auto_balance as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::None as u32,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::X as u32,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::Y as u32,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MOUSE_MOD) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MOD_STR[MOUSE_TAP_STATE.modifier.load(Ordering::Relaxed) as usize]
                        .unwrap_or("(null)")
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_MOUSE_MOD_ALT) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::ALT.0, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MOD_SHIFT,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::SHIFT.0, Ordering::Relaxed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_MOUSE_MOD_CMD) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::CMD.0, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MOD_CTRL,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::CTRL.0, Ordering::Relaxed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_MOUSE_MOD_FN) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::FN.0, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MOUSE_ACTION1) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_STR[MOUSE_TAP_STATE.action1.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_MOVE,
            ) {
                MOUSE_TAP_STATE
                    .action1
                    .store(MouseMode::Move as u8, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_RESIZE,
            ) {
                MOUSE_TAP_STATE
                    .action1
                    .store(MouseMode::Resize as u8, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MOUSE_ACTION2) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_STR[MOUSE_TAP_STATE.action2.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_MOVE,
            ) {
                MOUSE_TAP_STATE
                    .action2
                    .store(MouseMode::Move as u8, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_RESIZE,
            ) {
                MOUSE_TAP_STATE
                    .action2
                    .store(MouseMode::Resize as u8, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MOUSE_DROP_ACTION,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_STR[MOUSE_TAP_STATE.drop_action.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_SWAP,
            ) {
                MOUSE_TAP_STATE
                    .drop_action
                    .store(MouseMode::Swap as u8, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_STACK,
            ) {
                MOUSE_TAP_STATE
                    .drop_action
                    .store(MouseMode::Stack as u8, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_EXTERNAL_BAR) {
            let mut top: libc::c_int = 0;
            let mut bottom: libc::c_int = 0;
            let mut mode = [0 as libc::c_char; 6];
            let value = message_cursor.get_token();
            let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                .unwrap_or_default();
            let converted = unsafe {
                libc::sscanf(
                    subject.as_ptr(),
                    ARGUMENT_CONFIG_EXTERNAL_BAR.as_ptr(),
                    mode.as_mut_ptr(),
                    &mut top as *mut libc::c_int,
                    &mut bottom as *mut libc::c_int,
                )
            };
            if converted == 3 {
                let mode = mode.map(|character| character as u8);
                let mode = c_string_at(&mode, 0);
                if mode == ARGUMENT_CONFIG_EXTERNAL_BAR_MAIN.as_bytes() {
                    display_manager.mode = ExternalBarMode::Main;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
                } else if mode == ARGUMENT_CONFIG_EXTERNAL_BAR_ALL.as_bytes() {
                    display_manager.mode = ExternalBarMode::All;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
                } else if mode == ARGUMENT_COMMON_VAL_OFF.as_bytes() {
                    display_manager.mode = ExternalBarMode::Off;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
                } else {
                    response.fail_pieces(&[
                        FailurePiece::Text("unknown mode '"),
                        FailurePiece::Bytes(mode),
                        FailurePiece::Text("' specified in value '"),
                        FailurePiece::Bytes(value.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' given to command '"),
                        FailurePiece::Bytes(command.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' for domain '"),
                        FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
                        FailurePiece::Text("'\n"),
                    ]);
                }
            } else {
                response.write(format_args!(
                    "{}:{}:{}\n",
                    EXTERNAL_BAR_MODE_STR[display_manager.mode as usize],
                    display_manager.top_padding,
                    display_manager.bottom_padding
                ));
            }
        } else {
            daemon_fail_with_unknown_command_for_domain(
                response,
                message_cursor.bytes(),
                command,
                domain,
            );
        }

        command = message_cursor.get_token();
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::os::unix::net::UnixStream;

    use super::handle_domain_config;
    use crate::display::manager::DisplayManager;
    use crate::event::handlers::system::event_handler_system_accent_color_changed;
    use crate::message::token::MessageCursor;
    use crate::mouse::drag::mouse_drag_state_without_a_drag;
    use crate::space::manager::space_manager_without_any_view_with_its_initial_settings;
    use crate::support::color::rgba_color_from_hex;
    use crate::support::response::Response;
    use crate::window::manager::{
        WindowManager, window_manager_tracking_nothing_with_its_initial_settings,
    };

    fn config_message(arguments: &[&str]) -> Vec<u8> {
        let mut message = Vec::new();
        for argument in std::iter::once(&"config").chain(arguments) {
            message.extend_from_slice(argument.as_bytes());
            message.push(0);
        }
        message.push(0);
        message
    }

    fn handle_config_message_and_read_the_response(
        arguments: &[&str],
        window_manager: &mut WindowManager,
    ) -> String {
        let mut message = config_message(arguments);
        let (mut client_end, daemon_end) = UnixStream::pair().unwrap();
        {
            let mut response = Response::to_client(daemon_end);
            let mut message_cursor = MessageCursor::new(&mut message);
            let domain = message_cursor.get_token();
            handle_domain_config(
                &mut response,
                domain,
                &mut message_cursor,
                &mut DisplayManager::default(),
                window_manager,
                &mut space_manager_without_any_view_with_its_initial_settings(),
                &mut mouse_drag_state_without_a_drag(),
            );
        }

        let mut response_text = String::new();
        client_end.read_to_string(&mut response_text).unwrap();
        response_text
    }

    #[test]
    fn querying_insert_feedback_color_prints_the_packed_colour_as_lowercase_hexadecimal() {
        let mut window_manager = window_manager_tracking_nothing_with_its_initial_settings();
        window_manager.insert_feedback_color = rgba_color_from_hex(0xff0a7aff);

        let response_text = handle_config_message_and_read_the_response(
            &["insert_feedback_color"],
            &mut window_manager,
        );

        assert_eq!(response_text, "0xff0a7aff\n");
    }

    #[test]
    fn setting_insert_feedback_color_stores_it_and_the_accent_colour_no_longer_replaces_it() {
        let mut window_manager = window_manager_tracking_nothing_with_its_initial_settings();

        let response_text = handle_config_message_and_read_the_response(
            &["insert_feedback_color", "0xAA336699"],
            &mut window_manager,
        );
        event_handler_system_accent_color_changed(
            rgba_color_from_hex(0xff007aff),
            &mut window_manager,
        );

        assert_eq!(response_text, "");
        assert_eq!(window_manager.insert_feedback_color.packed, 0xaa336699);
        assert_eq!(
            window_manager.insert_feedback_color.red,
            rgba_color_from_hex(0xaa336699).red
        );
        assert!(!window_manager.insert_feedback_color_follows_the_system_accent_color);
    }

    #[test]
    fn until_a_client_sets_insert_feedback_color_the_accent_colour_replaces_it() {
        let mut window_manager = window_manager_tracking_nothing_with_its_initial_settings();

        event_handler_system_accent_color_changed(
            rgba_color_from_hex(0xff007aff),
            &mut window_manager,
        );

        assert_eq!(window_manager.insert_feedback_color.packed, 0xff007aff);
        assert!(window_manager.insert_feedback_color_follows_the_system_accent_color);
    }

    #[test]
    fn an_insert_feedback_color_of_zero_or_in_decimal_is_refused_and_changes_nothing() {
        for refused_value in ["0x0", "0x00000000", "4278190335", "red"] {
            let mut window_manager = window_manager_tracking_nothing_with_its_initial_settings();
            let packed_colour_before = window_manager.insert_feedback_color.packed;

            let response_text = handle_config_message_and_read_the_response(
                &["insert_feedback_color", refused_value],
                &mut window_manager,
            );

            assert_eq!(
                response_text,
                format!(
                    "\x07unknown value '{refused_value}' given to command 'insert_feedback_color' for domain 'config'\n"
                )
            );
            assert_eq!(
                window_manager.insert_feedback_color.packed,
                packed_colour_before
            );
            assert!(window_manager.insert_feedback_color_follows_the_system_accent_color);
        }
    }
}
