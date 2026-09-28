use std::sync::atomic::Ordering;

use crate::config_file::change_watcher::start_watching_the_config_file_to_reload_it_on_change_unless_already_watching;
use crate::daemon_fail;
use crate::display::manager::{
    DISPLAY_ARRANGEMENT_ORDER_NAMES, DisplayArrangementOrder, DisplayManager,
    EXTERNAL_BAR_MODE_NAMES, ExternalBarMode,
};
use crate::ffi::core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use crate::layout::group_header::refresh_the_group_headers_of_every_view;
use crate::layout::insertion::{WINDOW_INSERTION_POINT_NAMES, WindowInsertionPoint};
use crate::layout::settings::{AUTO_BALANCE_NAMES, VIEW_LAYOUT_NAMES, ViewFlag, ViewLayout};
use crate::layout::tree::{
    WINDOW_NODE_CHILD_NAMES, WINDOW_NODE_SPLIT_NAMES, WindowNodeChild, WindowNodeSplit,
};
use crate::layout::view::{
    clear_view_tree_unmanaging_every_window,
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::message::common_arguments::{
    ARGUMENT_COMMON_VALUE_AXIS_X, ARGUMENT_COMMON_VALUE_AXIS_Y, ARGUMENT_COMMON_VALUE_OFF,
    ARGUMENT_COMMON_VALUE_ON,
};
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_value_given_to_command_for_domain,
};
use crate::message::selectors::parse_space_selector;
use crate::message::token::{
    MessageCursor, Token, TokenValueType, is_token_equal_to, null_terminated_bytes_starting_at,
    parse_token_into_typed_value,
};
use crate::mouse::drag::MouseDragState;
use crate::mouse::tap::{
    MOUSE_MODE_NAMES, MOUSE_MODIFIER_NAMES, MOUSE_TAP_STATE, MouseMode, MouseModifier,
};
use crate::scripting_addition::installer::is_system_integrity_protection_relaxed_enough_for_scripting_addition;
use crate::space::managed_space::is_user_space;
use crate::space::manager::{
    SpaceManager, find_or_create_view_for_space,
    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date,
};
use crate::space::view_settings::{
    set_global_auto_balance_applying_it_to_views_without_their_own,
    set_global_bottom_padding_applying_it_to_views_without_their_own,
    set_global_layout_applying_it_to_views_without_their_own,
    set_global_left_padding_applying_it_to_views_without_their_own,
    set_global_right_padding_applying_it_to_views_without_their_own,
    set_global_split_type_applying_it_to_views_without_their_own,
    set_global_top_padding_applying_it_to_views_without_their_own,
    set_global_window_gap_applying_it_to_views_without_their_own,
};
use crate::state::process_wide::{
    RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED, VERBOSE_DEBUG_OUTPUT_ENABLED,
};
use crate::support::arithmetic::{
    is_within_range_excluding_low_including_high, is_within_range_including_both_bounds,
};
use crate::support::color::{RgbaColor, rgba_color_from_packed_argb};
use crate::support::easing::{
    ANIMATION_EASING_TYPE_COUNT, ANIMATION_EASING_TYPE_NAMES, AnimationEasingType,
};
use crate::support::handles::SpaceId;
use crate::support::printf_float_format::format_float_with_decimals_as_printf_does;
use crate::support::response::{FailurePiece, Response};
use crate::support::strings::BOOLEAN_NAMES;
use crate::window::focus::set_focus_follows_mouse_mode;
use crate::window::manager::{
    FOCUS_FOLLOWS_MOUSE_MODE_NAMES, FocusFollowsMouseMode, SHADOW_REMOVAL_MODE_NAMES,
    ShadowRemovalMode, WINDOW_ORIGIN_DISPLAY_MODE_NAMES, WindowManager, WindowOriginDisplayMode,
};
use crate::window::opacity::{
    set_active_window_opacity_applying_it_to_the_focused_window, set_menu_bar_opacity,
    set_normal_window_opacity_applying_it_to_every_unfocused_window,
    set_window_opacity_enabled_for_every_eligible_window,
};
use crate::window::shadow::set_shadow_removal_mode_for_every_eligible_window;
use crate::window::space_reconciliation::reconcile_space_view_with_windows_on_space;

/* --------------------------------DOMAIN CONFIG-------------------------------- */
pub(crate) const COMMAND_CONFIG_DEBUG_OUTPUT: &str = "debug_output";
pub(crate) const COMMAND_CONFIG_RELOAD_CONFIG_FILE_ON_CHANGE: &str = "reload_config_file_on_change";
pub(crate) const COMMAND_CONFIG_MOUSE_FOLLOWS_FOCUS: &str = "mouse_follows_focus";
pub(crate) const COMMAND_CONFIG_FOCUS_FOLLOWS_MOUSE: &str = "focus_follows_mouse";
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
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_HEIGHT: &str = "group_header_height";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_BACKGROUND_COLOR: &str =
    "group_header_background_color";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_ACTIVE_COLOR: &str = "group_header_active_color";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_INACTIVE_COLOR: &str = "group_header_inactive_color";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_ACTIVE_TEXT_COLOR: &str =
    "group_header_active_text_color";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_INACTIVE_TEXT_COLOR: &str =
    "group_header_inactive_text_color";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_FONT_FAMILY: &str = "group_header_font_family";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_FONT_STYLE: &str = "group_header_font_style";
pub(crate) const COMMAND_CONFIG_GROUP_HEADER_FONT_SIZE: &str = "group_header_font_size";
pub(crate) const COMMAND_CONFIG_TOP_PADDING: &str = "top_padding";
pub(crate) const COMMAND_CONFIG_BOTTOM_PADDING: &str = "bottom_padding";
pub(crate) const COMMAND_CONFIG_LEFT_PADDING: &str = "left_padding";
pub(crate) const COMMAND_CONFIG_RIGHT_PADDING: &str = "right_padding";
pub(crate) const COMMAND_CONFIG_LAYOUT: &str = "layout";
pub(crate) const COMMAND_CONFIG_WINDOW_GAP: &str = "window_gap";
pub(crate) const COMMAND_CONFIG_SPLIT_RATIO: &str = "split_ratio";
pub(crate) const COMMAND_CONFIG_SPLIT_TYPE: &str = "split_type";
pub(crate) const COMMAND_CONFIG_AUTO_BALANCE: &str = "auto_balance";
pub(crate) const COMMAND_CONFIG_MOUSE_MODIFIER: &str = "mouse_modifier";
pub(crate) const COMMAND_CONFIG_MOUSE_ACTION1: &str = "mouse_action1";
pub(crate) const COMMAND_CONFIG_MOUSE_ACTION2: &str = "mouse_action2";
pub(crate) const COMMAND_CONFIG_MOUSE_DROP_ACTION: &str = "mouse_drop_action";
pub(crate) const COMMAND_CONFIG_EXTERNAL_BAR: &str = "external_bar";
pub(crate) const COMMAND_CONFIG_SKIP_WINDOW_FOCUS_ANIMATION: &str = "skip_window_focus_animation";

pub(crate) const SELECTOR_CONFIG_SPACE: &str = "--space";

pub(crate) const ARGUMENT_CONFIG_FOCUS_FOLLOWS_MOUSE_AUTOFOCUS: &str = "autofocus";
pub(crate) const ARGUMENT_CONFIG_FOCUS_FOLLOWS_MOUSE_AUTORAISE: &str = "autoraise";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_DEFAULT: &str = "default";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_X: &str = "horizontal";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_Y: &str = "vertical";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_DEFAULT: &str = "default";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_FOCUSED: &str = "focused";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_CURSOR: &str = "cursor";
pub(crate) const ARGUMENT_CONFIG_WINDOW_PLACEMENT_FIRST_CHILD: &str = "first_child";
pub(crate) const ARGUMENT_CONFIG_WINDOW_PLACEMENT_SECOND_CHILD: &str = "second_child";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_FOCUSED: &str = "focused";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_FIRST: &str = "first";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_LAST: &str = "last";
pub(crate) const ARGUMENT_CONFIG_SHADOW_FLOAT: &str = "float";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_BINARY_SPACE_PARTITIONING: &str = "bsp";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_STACK: &str = "stack";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_FLOAT: &str = "float";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_Y: &str = "vertical";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_X: &str = "horizontal";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_AUTO: &str = "auto";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MODIFIER_ALT: &str = "alt";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MODIFIER_SHIFT: &str = "shift";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MODIFIER_COMMAND: &str = "cmd";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MODIFIER_CONTROL: &str = "ctrl";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MODIFIER_FUNCTION: &str = "fn";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_MOVE: &str = "move";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_RESIZE: &str = "resize";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_SWAP: &str = "swap";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_STACK: &str = "stack";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR_MAIN: &str = "main";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR_ALL: &str = "all";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR: &std::ffi::CStr = c"%5[^:]:%d:%d";
/* ----------------------------------------------------------------------------- */

pub(crate) fn run_config_command(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut selector_space_id = SpaceId(0);
    let selector = message_cursor.take_next_token();
    let mut command = selector;

    let found_selector = is_token_equal_to(selector, message_cursor.bytes(), SELECTOR_CONFIG_SPACE);
    if found_selector {
        let space_selector =
            parse_space_selector(response, message_cursor, SpaceId(0), false, space_manager);
        let Some(space_id) = space_selector.resolved_target() else {
            return;
        };

        selector_space_id = space_id;
        command = message_cursor.take_next_token();
    }

    while command.is_not_empty() {
        if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_DEBUG_OUTPUT) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    BOOLEAN_NAMES[VERBOSE_DEBUG_OUTPUT_ENABLED.load(Ordering::Relaxed) as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                VERBOSE_DEBUG_OUTPUT_ENABLED.store(false, Ordering::Relaxed);
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
                VERBOSE_DEBUG_OUTPUT_ENABLED.store(true, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_RELOAD_CONFIG_FILE_ON_CHANGE,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    BOOLEAN_NAMES
                        [RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED.load(Ordering::Relaxed) as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED.store(false, Ordering::Relaxed);
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
                RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED.store(true, Ordering::Relaxed);
                start_watching_the_config_file_to_reload_it_on_change_unless_already_watching();
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MOUSE_FOLLOWS_FOCUS,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    BOOLEAN_NAMES[window_manager.enable_mff as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                window_manager.enable_mff = false;
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_FOCUS_FOLLOWS_MOUSE,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    FOCUS_FOLLOWS_MOUSE_MODE_NAMES
                        [window_manager.focus_follows_mouse_mode as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                set_focus_follows_mouse_mode(window_manager, FocusFollowsMouseMode::Disabled);
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_FOCUS_FOLLOWS_MOUSE_AUTOFOCUS,
            ) {
                set_focus_follows_mouse_mode(window_manager, FocusFollowsMouseMode::Autofocus);
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_FOCUS_FOLLOWS_MOUSE_AUTORAISE,
            ) {
                set_focus_follows_mouse_mode(window_manager, FocusFollowsMouseMode::Autoraise);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_DISPLAY_ORDER,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    DISPLAY_ARRANGEMENT_ORDER_NAMES[display_manager.order as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_DEFAULT,
            ) {
                display_manager.order = DisplayArrangementOrder::Default;
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_X,
            ) {
                display_manager.order = DisplayArrangementOrder::Horizontal;
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_Y,
            ) {
                display_manager.order = DisplayArrangementOrder::Vertical;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_ORIGIN,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_ORIGIN_DISPLAY_MODE_NAMES
                        [window_manager.window_origin_display_mode as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_DEFAULT,
            ) {
                window_manager.window_origin_display_mode =
                    WindowOriginDisplayMode::DisplayTheWindowOpenedOn;
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_FOCUSED,
            ) {
                window_manager.window_origin_display_mode = WindowOriginDisplayMode::FocusedDisplay;
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_CURSOR,
            ) {
                window_manager.window_origin_display_mode =
                    WindowOriginDisplayMode::DisplayUnderTheCursor;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_PLACEMENT,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_NODE_CHILD_NAMES[space_manager.window_placement as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_PLACEMENT_FIRST_CHILD,
            ) {
                space_manager.window_placement = WindowNodeChild::First;
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_PLACEMENT_SECOND_CHILD,
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_INSERT_POINT,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_INSERTION_POINT_NAMES[space_manager.window_insertion_point as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_FOCUSED,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::Focused;
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_FIRST,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::First;
            } else if is_token_equal_to(
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_ZOOM_PERSIST,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    BOOLEAN_NAMES[space_manager.window_zoom_persist as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                space_manager.window_zoom_persist = false;
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_SKIP_WINDOW_FOCUS_ANIMATION,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    BOOLEAN_NAMES[space_manager.skip_window_focus_animation as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                space_manager.skip_window_focus_animation = false;
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_OPACITY) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    BOOLEAN_NAMES[window_manager.enable_window_opacity as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                set_window_opacity_enabled_for_every_eligible_window(window_manager, false);
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
                set_window_opacity_enabled_for_every_eligible_window(window_manager, true);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_OPACITY_DURATION,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.window_opacity_duration as f64,
                        6
                    )
                ));
            } else if let TokenValueType::Float(float_value) = value.type_of_value {
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ANIMATION_DURATION,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.window_animation_duration as f64,
                        6
                    )
                ));
            } else if let TokenValueType::Float(float_value) = value.type_of_value
                && float_value.is_finite()
                && float_value >= 0.0f32
            {
                if float_value == 0.0f32 {
                    window_manager.window_animation_duration = float_value;
                } else if !is_system_integrity_protection_relaxed_enough_for_scripting_addition() {
                    response.write_failure_pieces_unless_silent(&[
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
                    response.write_failure_pieces_unless_silent(&[
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ANIMATION_EASING,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    ANIMATION_EASING_TYPE_NAMES[window_manager.window_animation_easing as usize]
                ));
            } else {
                let mut found_match = false;
                for index in 0..ANIMATION_EASING_TYPE_COUNT {
                    if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ANIMATION_EASING_TYPE_NAMES[index],
                    ) {
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_SHADOW) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    SHADOW_REMOVAL_MODE_NAMES[window_manager.shadow_removal_mode as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                set_shadow_removal_mode_for_every_eligible_window(
                    window_manager,
                    ShadowRemovalMode::FromEveryWindow,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_CONFIG_SHADOW_FLOAT)
            {
                set_shadow_removal_mode_for_every_eligible_window(
                    window_manager,
                    ShadowRemovalMode::FromManagedWindows,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
                set_shadow_removal_mode_for_every_eligible_window(
                    window_manager,
                    ShadowRemovalMode::Never,
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MENUBAR_OPACITY,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.menubar_opacity as f64,
                        4
                    )
                ));
            } else if let TokenValueType::Float(float_value) = value.type_of_value
                && is_within_range_including_both_bounds(float_value, 0.0f32, 1.0f32)
            {
                set_menu_bar_opacity(window_manager, float_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ACTIVE_WINDOW_OPACITY,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.active_window_opacity as f64,
                        4
                    )
                ));
            } else if let TokenValueType::Float(float_value) = value.type_of_value
                && is_within_range_excluding_low_including_high(float_value, 0.0f32, 1.0f32)
            {
                set_active_window_opacity_applying_it_to_the_focused_window(
                    window_manager,
                    float_value,
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_NORMAL_WINDOW_OPACITY,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(
                        window_manager.normal_window_opacity as f64,
                        4
                    )
                ));
            } else if let TokenValueType::Float(float_value) = value.type_of_value
                && is_within_range_excluding_low_including_high(float_value, 0.0f32, 1.0f32)
            {
                set_normal_window_opacity_applying_it_to_every_unfocused_window(
                    window_manager,
                    float_value,
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_INSERT_FEEDBACK_COLOR,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "0x{:x}\n",
                    window_manager.insert_feedback_color.packed
                ));
            } else if let TokenValueType::Hexadecimal(u32_value) = value.type_of_value
                && u32_value != 0
            {
                window_manager.insert_feedback_color = rgba_color_from_packed_argb(u32_value);
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_HEIGHT,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    window_manager.group_header_style.height as i32
                ));
            } else if let TokenValueType::Integer(height) = value.type_of_value
                && height >= 0
            {
                window_manager.group_header_style.height = height as f32;
                move_the_windows_of_every_view_into_their_areas(space_manager, window_manager);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_BACKGROUND_COLOR,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "0x{:x}\n",
                    window_manager.group_header_style.background_color.packed
                ));
            } else if let TokenValueType::Hexadecimal(packed_color) = value.type_of_value {
                window_manager.group_header_style.background_color =
                    rgba_color_from_packed_argb(packed_color);
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_ACTIVE_COLOR,
        ) {
            if query_or_set_group_header_color(
                response,
                message_cursor,
                command,
                domain,
                &mut window_manager.group_header_style.active_color,
            ) {
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_INACTIVE_COLOR,
        ) {
            if query_or_set_group_header_color(
                response,
                message_cursor,
                command,
                domain,
                &mut window_manager.group_header_style.inactive_color,
            ) {
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_ACTIVE_TEXT_COLOR,
        ) {
            if query_or_set_group_header_color(
                response,
                message_cursor,
                command,
                domain,
                &mut window_manager.group_header_style.active_text_color,
            ) {
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_INACTIVE_TEXT_COLOR,
        ) {
            if query_or_set_group_header_color(
                response,
                message_cursor,
                command,
                domain,
                &mut window_manager.group_header_style.inactive_text_color,
            ) {
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_FONT_FAMILY,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    window_manager.group_header_style.font_family
                ));
            } else {
                window_manager.group_header_style.font_family =
                    String::from_utf8_lossy(value.bytes(message_cursor.bytes())).into_owned();
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_FONT_STYLE,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    window_manager.group_header_style.font_style
                ));
            } else {
                window_manager.group_header_style.font_style =
                    String::from_utf8_lossy(value.bytes(message_cursor.bytes())).into_owned();
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_GROUP_HEADER_FONT_SIZE,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            let font_size = match value.type_of_value {
                TokenValueType::Integer(font_size) => Some(font_size as f32),
                TokenValueType::Float(font_size) => Some(font_size),
                _ => None,
            };
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    window_manager.group_header_style.font_size
                ));
            } else if let Some(font_size) = font_size
                && font_size > 0.0
            {
                window_manager.group_header_style.font_size = font_size;
                refresh_the_group_headers_of_every_view(space_manager, window_manager);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_TOP_PADDING) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                    if let TokenValueType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.top_padding));
                    } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_TOP_PADDING);
                        view.top_padding = int_value;
                        recompute_view_areas_from_display_bounds_and_padding(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                        );
                        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                            space_manager,
                            view_space_id,
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
                }
            } else if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.top_padding));
            } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                set_global_top_padding_applying_it_to_views_without_their_own(
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_BOTTOM_PADDING,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                    if let TokenValueType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.bottom_padding));
                    } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_BOTTOM_PADDING);
                        view.bottom_padding = int_value;
                        recompute_view_areas_from_display_bounds_and_padding(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                        );
                        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                            space_manager,
                            view_space_id,
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
                }
            } else if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.bottom_padding));
            } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                set_global_bottom_padding_applying_it_to_views_without_their_own(
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_LEFT_PADDING) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                    if let TokenValueType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.left_padding));
                    } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_LEFT_PADDING);
                        view.left_padding = int_value;
                        recompute_view_areas_from_display_bounds_and_padding(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                        );
                        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                            space_manager,
                            view_space_id,
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
                }
            } else if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.left_padding));
            } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                set_global_left_padding_applying_it_to_views_without_their_own(
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_RIGHT_PADDING,
        ) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                    if let TokenValueType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.right_padding));
                    } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_RIGHT_PADDING);
                        view.right_padding = int_value;
                        recompute_view_areas_from_display_bounds_and_padding(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                        );
                        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                            space_manager,
                            view_space_id,
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
                }
            } else if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.right_padding));
            } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                set_global_right_padding_applying_it_to_views_without_their_own(
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_WINDOW_GAP) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                    if let TokenValueType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.window_gap));
                    } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_WINDOW_GAP);
                        view.window_gap = int_value;
                        recompute_view_areas_from_display_bounds_and_padding(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                        );
                        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                            space_manager,
                            view_space_id,
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
                }
            } else if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.window_gap));
            } else if let TokenValueType::Integer(int_value) = value.type_of_value {
                set_global_window_gap_applying_it_to_views_without_their_own(
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_LAYOUT) {
            let value = message_cursor.take_next_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if !value.is_not_empty() {
                    if let Some(view) = space_manager.view.get(&view_space_id) {
                        response.write(format_args!(
                            "{}\n",
                            VIEW_LAYOUT_NAMES[view.layout as usize]
                        ));
                    }
                } else if is_token_equal_to(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_CONFIG_LAYOUT_BINARY_SPACE_PARTITIONING,
                ) {
                    if is_user_space(selector_space_id) {
                        if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                            view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_LAYOUT);
                            view.layout = ViewLayout::BinarySpacePartitioning;
                        }
                        clear_view_tree_unmanaging_every_window(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                        reconcile_space_view_with_windows_on_space(
                            space_manager,
                            window_manager,
                            selector_space_id,
                            display_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(
                            response,
                            "cannot set layout for a macOS fullscreen space!\n"
                        );
                    }
                } else if is_token_equal_to(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_CONFIG_LAYOUT_STACK,
                ) {
                    if is_user_space(selector_space_id) {
                        if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                            view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_LAYOUT);
                            view.layout = ViewLayout::Stack;
                        }
                        clear_view_tree_unmanaging_every_window(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                        reconcile_space_view_with_windows_on_space(
                            space_manager,
                            window_manager,
                            selector_space_id,
                            display_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(
                            response,
                            "cannot set layout for a macOS fullscreen space!\n"
                        );
                    }
                } else if is_token_equal_to(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_CONFIG_LAYOUT_FLOAT,
                ) {
                    if is_user_space(selector_space_id) {
                        if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                            view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_LAYOUT);
                            view.layout = ViewLayout::Float;
                        }
                        clear_view_tree_unmanaging_every_window(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(
                            response,
                            "cannot set layout for a macOS fullscreen space!\n"
                        );
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
            } else if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    VIEW_LAYOUT_NAMES[space_manager.layout as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_LAYOUT_BINARY_SPACE_PARTITIONING,
            ) {
                set_global_layout_applying_it_to_views_without_their_own(
                    space_manager,
                    ViewLayout::BinarySpacePartitioning,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_STACK)
            {
                set_global_layout_applying_it_to_views_without_their_own(
                    space_manager,
                    ViewLayout::Stack,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_FLOAT)
            {
                set_global_layout_applying_it_to_views_without_their_own(
                    space_manager,
                    ViewLayout::Float,
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_SPLIT_RATIO) {
            let value = parse_token_into_typed_value(
                message_cursor.take_next_token(),
                message_cursor.bytes(),
            );
            if let TokenValueType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_decimals_as_printf_does(space_manager.split_ratio as f64, 4)
                ));
            } else if let TokenValueType::Float(float_value) = value.type_of_value
                && is_within_range_including_both_bounds(float_value, 0.1f32, 0.9f32)
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_SPLIT_TYPE) {
            let value = message_cursor.take_next_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                    if !value.is_not_empty() {
                        response.write(format_args!(
                            "{}\n",
                            WINDOW_NODE_SPLIT_NAMES[view.split_type as usize]
                        ));
                    } else if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_Y,
                    ) {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::Vertical;
                    } else if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_X,
                    ) {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::Horizontal;
                    } else if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_AUTO,
                    ) {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_SPLIT_TYPE);
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
            } else if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_NODE_SPLIT_NAMES[space_manager.split_type as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_CONFIG_SPLIT_TYPE_Y)
            {
                set_global_split_type_applying_it_to_views_without_their_own(
                    space_manager,
                    WindowNodeSplit::Vertical,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_CONFIG_SPLIT_TYPE_X)
            {
                set_global_split_type_applying_it_to_views_without_their_own(
                    space_manager,
                    WindowNodeSplit::Horizontal,
                );
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_SPLIT_TYPE_AUTO,
            ) {
                set_global_split_type_applying_it_to_views_without_their_own(
                    space_manager,
                    WindowNodeSplit::Auto,
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_AUTO_BALANCE) {
            let value = message_cursor.take_next_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = find_or_create_view_for_space(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view_space_id) {
                    if !value.is_not_empty() {
                        response.write(format_args!(
                            "{}\n",
                            AUTO_BALANCE_NAMES[view.auto_balance as usize]
                        ));
                    } else if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VALUE_OFF,
                    ) {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::None as u32;
                    } else if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VALUE_ON,
                    ) {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_AUTO_BALANCE);
                        view.auto_balance =
                            WindowNodeSplit::Horizontal as u32 | WindowNodeSplit::Vertical as u32;
                    } else if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VALUE_AXIS_X,
                    ) {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::Horizontal as u32;
                    } else if is_token_equal_to(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VALUE_AXIS_Y,
                    ) {
                        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::Vertical as u32;
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
            } else if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    AUTO_BALANCE_NAMES[space_manager.auto_balance as usize]
                ));
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_OFF) {
                set_global_auto_balance_applying_it_to_views_without_their_own(
                    space_manager,
                    WindowNodeSplit::None as u32,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_ON) {
                set_global_auto_balance_applying_it_to_views_without_their_own(
                    space_manager,
                    WindowNodeSplit::Horizontal as u32 | WindowNodeSplit::Vertical as u32,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_X)
            {
                set_global_auto_balance_applying_it_to_views_without_their_own(
                    space_manager,
                    WindowNodeSplit::Horizontal as u32,
                );
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_Y)
            {
                set_global_auto_balance_applying_it_to_views_without_their_own(
                    space_manager,
                    WindowNodeSplit::Vertical as u32,
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MOUSE_MODIFIER,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODIFIER_NAMES[MOUSE_TAP_STATE.modifier.load(Ordering::Relaxed) as usize]
                        .unwrap_or("(null)")
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MODIFIER_ALT,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseModifier::ALT.0, Ordering::Relaxed);
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MODIFIER_SHIFT,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseModifier::SHIFT.0, Ordering::Relaxed);
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MODIFIER_COMMAND,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseModifier::COMMAND.0, Ordering::Relaxed);
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MODIFIER_CONTROL,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseModifier::CONTROL.0, Ordering::Relaxed);
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MODIFIER_FUNCTION,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseModifier::FUNCTION.0, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MOUSE_ACTION1,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_NAMES[MOUSE_TAP_STATE.action1.load(Ordering::Relaxed) as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_MOVE,
            ) {
                MOUSE_TAP_STATE
                    .action1
                    .store(MouseMode::Move as u8, Ordering::Relaxed);
            } else if is_token_equal_to(
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MOUSE_ACTION2,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_NAMES[MOUSE_TAP_STATE.action2.load(Ordering::Relaxed) as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_MOVE,
            ) {
                MOUSE_TAP_STATE
                    .action2
                    .store(MouseMode::Move as u8, Ordering::Relaxed);
            } else if is_token_equal_to(
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
        } else if is_token_equal_to(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MOUSE_DROP_ACTION,
        ) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_NAMES[MOUSE_TAP_STATE.drop_action.load(Ordering::Relaxed) as usize]
                ));
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_SWAP,
            ) {
                MOUSE_TAP_STATE
                    .drop_action
                    .store(MouseMode::Swap as u8, Ordering::Relaxed);
            } else if is_token_equal_to(
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_CONFIG_EXTERNAL_BAR) {
            let mut top: libc::c_int = 0;
            let mut bottom: libc::c_int = 0;
            let mut mode = [0 as libc::c_char; 6];
            let value = message_cursor.take_next_token();
            let subject =
                std::ffi::CString::new(value.bytes(message_cursor.bytes())).unwrap_or_default();
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
                let mode = null_terminated_bytes_starting_at(&mode, 0);
                if mode == ARGUMENT_CONFIG_EXTERNAL_BAR_MAIN.as_bytes() {
                    display_manager.mode = ExternalBarMode::MainDisplayOnly;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
                        space_manager,
                        display_manager,
                        window_manager,
                    );
                } else if mode == ARGUMENT_CONFIG_EXTERNAL_BAR_ALL.as_bytes() {
                    display_manager.mode = ExternalBarMode::EveryDisplay;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
                        space_manager,
                        display_manager,
                        window_manager,
                    );
                } else if mode == ARGUMENT_COMMON_VALUE_OFF.as_bytes() {
                    display_manager.mode = ExternalBarMode::Off;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
                        space_manager,
                        display_manager,
                        window_manager,
                    );
                } else {
                    response.write_failure_pieces_unless_silent(&[
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
                    EXTERNAL_BAR_MODE_NAMES[display_manager.mode as usize],
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

        command = message_cursor.take_next_token();
    }
}

fn query_or_set_group_header_color(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
    command: Token,
    domain: Token,
    color: &mut RgbaColor,
) -> bool {
    let value =
        parse_token_into_typed_value(message_cursor.take_next_token(), message_cursor.bytes());
    if let TokenValueType::Invalid = value.type_of_value {
        response.write(format_args!("0x{:x}\n", color.packed));
        false
    } else if let TokenValueType::Hexadecimal(packed_color) = value.type_of_value
        && packed_color != 0
    {
        *color = rgba_color_from_packed_argb(packed_color);
        true
    } else {
        daemon_fail_with_unknown_value_given_to_command_for_domain(
            response,
            message_cursor.bytes(),
            value.token,
            command,
            domain,
        );
        false
    }
}

fn move_the_windows_of_every_view_into_their_areas(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) {
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
            space_manager,
            space_id,
            window_manager,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::run_config_command;
    use crate::display::manager::DisplayManager;
    use crate::event::handlers::system::handle_system_accent_color_changed_event;
    use crate::message::token::MessageCursor;
    use crate::mouse::drag::mouse_drag_state_without_a_drag;
    use crate::space::manager::create_space_manager_without_any_view_with_its_initial_settings;
    use crate::support::color::rgba_color_from_packed_argb;
    use crate::support::response::Response;
    use crate::window::manager::{
        WindowManager, create_window_manager_tracking_nothing_with_its_initial_settings,
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
        let mut response = Response::collecting();
        let mut message_cursor = MessageCursor::new(&mut message);
        let domain = message_cursor.take_next_token();
        run_config_command(
            &mut response,
            domain,
            &mut message_cursor,
            &mut DisplayManager::default(),
            window_manager,
            &mut create_space_manager_without_any_view_with_its_initial_settings(),
            &mut mouse_drag_state_without_a_drag(),
        );

        let (standard_output, failures) = response.into_standard_output_and_one_failure_per_line();
        failures
            .iter()
            .fold(standard_output, |text, failure| text + failure + "\n")
    }

    #[test]
    fn querying_insert_feedback_color_prints_the_packed_colour_as_lowercase_hexadecimal() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();
        window_manager.insert_feedback_color = rgba_color_from_packed_argb(0xff0a7aff);

        let response_text = handle_config_message_and_read_the_response(
            &["insert_feedback_color"],
            &mut window_manager,
        );

        assert_eq!(response_text, "0xff0a7aff\n");
    }

    #[test]
    fn setting_insert_feedback_color_stores_it_and_the_accent_colour_no_longer_replaces_it() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let response_text = handle_config_message_and_read_the_response(
            &["insert_feedback_color", "0xAA336699"],
            &mut window_manager,
        );
        handle_system_accent_color_changed_event(
            rgba_color_from_packed_argb(0xff007aff),
            &mut window_manager,
        );

        assert_eq!(response_text, "");
        assert_eq!(window_manager.insert_feedback_color.packed, 0xaa336699);
        assert_eq!(
            window_manager.insert_feedback_color.red,
            rgba_color_from_packed_argb(0xaa336699).red
        );
        assert!(!window_manager.insert_feedback_color_follows_the_system_accent_color);
    }

    #[test]
    fn until_a_client_sets_insert_feedback_color_the_accent_colour_replaces_it() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        handle_system_accent_color_changed_event(
            rgba_color_from_packed_argb(0xff007aff),
            &mut window_manager,
        );

        assert_eq!(window_manager.insert_feedback_color.packed, 0xff007aff);
        assert!(window_manager.insert_feedback_color_follows_the_system_accent_color);
    }

    #[test]
    fn an_insert_feedback_color_of_zero_or_in_decimal_is_refused_and_changes_nothing() {
        for refused_value in ["0x0", "0x00000000", "4278190335", "red"] {
            let mut window_manager =
                create_window_manager_tracking_nothing_with_its_initial_settings();
            let packed_colour_before = window_manager.insert_feedback_color.packed;

            let response_text = handle_config_message_and_read_the_response(
                &["insert_feedback_color", refused_value],
                &mut window_manager,
            );

            assert_eq!(
                response_text,
                format!(
                    "unknown value '{refused_value}' given to command 'insert_feedback_color' for domain 'config'\n"
                )
            );
            assert_eq!(
                window_manager.insert_feedback_color.packed,
                packed_colour_before
            );
            assert!(window_manager.insert_feedback_color_follows_the_system_accent_color);
        }
    }

    #[test]
    fn after_reload_config_file_on_change_is_turned_off_the_query_prints_off() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let response_to_turning_it_off = handle_config_message_and_read_the_response(
            &["reload_config_file_on_change", "off"],
            &mut window_manager,
        );
        let response_to_the_query = handle_config_message_and_read_the_response(
            &["reload_config_file_on_change"],
            &mut window_manager,
        );

        assert_eq!(response_to_turning_it_off, "");
        assert_eq!(response_to_the_query, "off\n");
    }

    #[test]
    fn reload_config_file_on_change_refuses_a_value_other_than_on_or_off() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let response_text = handle_config_message_and_read_the_response(
            &["reload_config_file_on_change", "yes"],
            &mut window_manager,
        );

        assert_eq!(
            response_text,
            "unknown value 'yes' given to command 'reload_config_file_on_change' for domain 'config'\n"
        );
    }

    #[test]
    fn group_header_settings_print_their_initial_values() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        for (setting, expected_response) in [
            ("group_header_height", "24\n"),
            ("group_header_active_color", "0xff3d59a1\n"),
            ("group_header_inactive_text_color", "0xff737aa2\n"),
            ("group_header_font_family", "Helvetica Neue\n"),
            ("group_header_font_size", "12\n"),
        ] {
            assert_eq!(
                handle_config_message_and_read_the_response(&[setting], &mut window_manager),
                expected_response,
                "{setting}"
            );
        }
    }

    #[test]
    fn setting_group_header_colours_and_font_stores_them() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let responses = [
            handle_config_message_and_read_the_response(
                &["group_header_inactive_color", "0xff292e42"],
                &mut window_manager,
            ),
            handle_config_message_and_read_the_response(
                &["group_header_font_family", "JetBrainsMono Nerd Font"],
                &mut window_manager,
            ),
            handle_config_message_and_read_the_response(
                &["group_header_font_size", "13.5"],
                &mut window_manager,
            ),
            handle_config_message_and_read_the_response(
                &["group_header_height", "30"],
                &mut window_manager,
            ),
        ];

        assert!(responses.iter().all(String::is_empty));
        let style = &window_manager.group_header_style;
        assert_eq!(style.inactive_color.packed, 0xff292e42);
        assert_eq!(style.font_family, "JetBrainsMono Nerd Font");
        assert_eq!(style.font_size, 13.5);
        assert_eq!(style.height, 30.0);
    }

    #[test]
    fn group_header_settings_refuse_a_zero_colour_a_negative_height_and_a_zero_font_size() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        for (setting, refused_value) in [
            ("group_header_active_color", "0x0"),
            ("group_header_active_color", "4278190335"),
            ("group_header_height", "-1"),
            ("group_header_font_size", "0"),
        ] {
            let response_text = handle_config_message_and_read_the_response(
                &[setting, refused_value],
                &mut window_manager,
            );

            assert_eq!(
                response_text,
                format!(
                    "unknown value '{refused_value}' given to command '{setting}' for domain 'config'\n"
                )
            );
        }
        let style = &window_manager.group_header_style;
        assert_eq!(style.active_color.packed, 0xff3d59a1);
        assert_eq!(style.height, 24.0);
        assert_eq!(style.font_size, 12.0);
    }

    #[test]
    fn group_header_background_accepts_a_fully_transparent_colour_and_a_font_style_is_stored() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let responses = [
            handle_config_message_and_read_the_response(
                &["group_header_background_color", "0xcc292e42"],
                &mut window_manager,
            ),
            handle_config_message_and_read_the_response(
                &["group_header_font_style", "Bold"],
                &mut window_manager,
            ),
        ];
        let background_after_it_was_set = window_manager.group_header_style.background_color.packed;
        let response_to_clearing_the_background = handle_config_message_and_read_the_response(
            &["group_header_background_color", "0x0"],
            &mut window_manager,
        );

        assert!(responses.iter().all(String::is_empty));
        assert_eq!(response_to_clearing_the_background, "");
        assert_eq!(background_after_it_was_set, 0xcc292e42);
        assert_eq!(window_manager.group_header_style.background_color.packed, 0);
        assert_eq!(window_manager.group_header_style.font_style, "Bold");
    }
}
