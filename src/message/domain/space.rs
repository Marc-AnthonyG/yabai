use crate::daemon_fail;
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::WindowNodeSplit;
use crate::message::argument_prefixes::parse_absolute_or_relative_change_type;
use crate::message::common_arguments::{
    ARGUMENT_COMMON_VALUE_AXIS_X, ARGUMENT_COMMON_VALUE_AXIS_Y,
};
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_value_given_to_command_for_domain,
};
use crate::message::labels::{LabelType, parse_label_refusing_numbers_and_reserved_words};
use crate::message::selectors::{parse_display_selector, parse_space_selector};
use crate::message::token::{MessageCursor, Token, is_token_equal_to};
use crate::mouse::drag::MouseDragState;
use crate::space::focus::{
    focus_space_then_toggle_mission_control, focus_space_then_toggle_show_desktop,
    focus_space_through_the_scripting_addition_or_dock_swipes,
    query_current_space_of_the_focused_display, switch_to_space_bringing_it_to_the_current_display,
};
use crate::space::labels::{
    remove_label_of_space, set_label_of_space_removing_it_from_any_other_space,
};
use crate::space::managed_space::is_user_space;
use crate::space::manager::SpaceManager;
use crate::space::operations::{
    SpaceOperationOutcome, add_space_on_display_of_space,
    destroy_user_space_unless_it_is_the_last_of_its_display, move_space_to_position_of_space,
    send_space_to_display, swap_space_with_space,
};
use crate::space::tiling::{
    balance_split_ratios_in_view_of_space, mirror_view_of_space_along_axis,
    reset_split_ratios_in_view_of_space_to_the_global_ratio, rotate_view_of_space_by_degrees,
};
use crate::space::view_settings::{
    set_layout_of_space_retiling_its_windows, set_padding_of_space, set_window_gap_of_space,
    toggle_padding_of_space, toggle_window_gap_of_space,
};
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::SpaceId;
use crate::support::response::Response;
use crate::support::strings::FIXED_STRING_BUFFER_LENGTH;
use crate::window::manager::WindowManager;

/* --------------------------------DOMAIN SPACE--------------------------------- */
pub(crate) const COMMAND_SPACE_FOCUS: &str = "--focus";
pub(crate) const COMMAND_SPACE_SWITCH: &str = "--switch";
pub(crate) const COMMAND_SPACE_CREATE: &str = "--create";
pub(crate) const COMMAND_SPACE_DESTROY: &str = "--destroy";
pub(crate) const COMMAND_SPACE_MOVE: &str = "--move";
pub(crate) const COMMAND_SPACE_SWAP: &str = "--swap";
pub(crate) const COMMAND_SPACE_DISPLAY: &str = "--display";
pub(crate) const COMMAND_SPACE_EQUALIZE: &str = "--equalize";
pub(crate) const COMMAND_SPACE_BALANCE: &str = "--balance";
pub(crate) const COMMAND_SPACE_MIRROR: &str = "--mirror";
pub(crate) const COMMAND_SPACE_ROTATE: &str = "--rotate";
pub(crate) const COMMAND_SPACE_PADDING: &str = "--padding";
pub(crate) const COMMAND_SPACE_GAP: &str = "--gap";
pub(crate) const COMMAND_SPACE_TOGGLE: &str = "--toggle";
pub(crate) const COMMAND_SPACE_LAYOUT: &str = "--layout";
pub(crate) const COMMAND_SPACE_LABEL: &str = "--label";

pub(crate) const ARGUMENT_SPACE_ROTATE_90: &str = "90";
pub(crate) const ARGUMENT_SPACE_ROTATE_180: &str = "180";
pub(crate) const ARGUMENT_SPACE_ROTATE_270: &str = "270";
pub(crate) const ARGUMENT_SPACE_PADDING: &std::ffi::CStr = c"%255[^:]:%d:%d:%d:%d";
pub(crate) const ARGUMENT_SPACE_GAP: &std::ffi::CStr = c"%255[^:]:%d";
pub(crate) const ARGUMENT_SPACE_TOGGLE_PADDING: &str = "padding";
pub(crate) const ARGUMENT_SPACE_TOGGLE_GAP: &str = "gap";
pub(crate) const ARGUMENT_SPACE_TOGGLE_MISSION_CONTROL: &str = "mission-control";
pub(crate) const ARGUMENT_SPACE_TOGGLE_SHOW_DESKTOP: &str = "show-desktop";
pub(crate) const ARGUMENT_SPACE_LAYOUT_BINARY_SPACE_PARTITIONING: &str = "bsp";
pub(crate) const ARGUMENT_SPACE_LAYOUT_STACK: &str = "stack";
pub(crate) const ARGUMENT_SPACE_LAYOUT_FLOAT: &str = "float";
/* ----------------------------------------------------------------------------- */

pub(crate) fn run_space_command(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut command;
    let mut acting_space_id = query_current_space_of_the_focused_display(window_manager);
    let selector = parse_space_selector(
        &mut Response::silent(),
        message_cursor,
        acting_space_id,
        true,
        space_manager,
    );

    if selector.is_recognised_selector() {
        acting_space_id = selector.resolved_target().unwrap_or(SpaceId(0));
        command = message_cursor.take_next_token();
    } else {
        command = selector.token;
    }

    if acting_space_id == SpaceId(0) {
        daemon_fail!(response, "could not locate the space to act on!\n");
        return;
    }

    while command.is_not_empty() {
        if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_FOCUS) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved_target() {
                let result = focus_space_through_the_scripting_addition_or_dock_swipes(
                    selector_space_id,
                    window_manager,
                    mission_control_mode,
                );
                if result == SpaceOperationOutcome::SameSpace {
                    daemon_fail!(response, "cannot focus an already focused space.\n");
                } else if result == SpaceOperationOutcome::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot focus space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOperationOutcome::MissionControlIsActive {
                    daemon_fail!(
                        response,
                        "cannot focus space because mission-control is active.\n"
                    );
                } else if result == SpaceOperationOutcome::ScriptingAdditionFailed {
                    daemon_fail!(
                        response,
                        "cannot focus space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_SWITCH) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved_target() {
                let result = switch_to_space_bringing_it_to_the_current_display(
                    selector_space_id,
                    display_manager,
                    window_manager,
                    space_manager,
                    mission_control_mode,
                    mouse_drag_state,
                );
                if result == SpaceOperationOutcome::SameSpace {
                    daemon_fail!(response, "cannot focus an already focused space.\n");
                } else if result == SpaceOperationOutcome::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot focus space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOperationOutcome::MissionControlIsActive {
                    daemon_fail!(
                        response,
                        "cannot focus space because mission-control is active.\n"
                    );
                } else if result == SpaceOperationOutcome::ScriptingAdditionFailed {
                    daemon_fail!(
                        response,
                        "cannot focus space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_MOVE) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved_target() {
                let result = move_space_to_position_of_space(
                    acting_space_id,
                    selector_space_id,
                    window_manager,
                    mission_control_mode,
                );
                if result == SpaceOperationOutcome::SameSpace {
                    daemon_fail!(response, "cannot move space to itself.\n");
                } else if result == SpaceOperationOutcome::NotOnTheSameDisplay {
                    daemon_fail!(
                        response,
                        "cannot move space across display boundaries. use --display instead.\n"
                    );
                } else if result == SpaceOperationOutcome::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot move space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOperationOutcome::MissionControlIsActive {
                    daemon_fail!(
                        response,
                        "cannot move space because mission-control is active.\n"
                    );
                } else if result == SpaceOperationOutcome::ScriptingAdditionFailed {
                    daemon_fail!(
                        response,
                        "cannot move space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_SWAP) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved_target() {
                let result = swap_space_with_space(
                    acting_space_id,
                    selector_space_id,
                    display_manager,
                    window_manager,
                    space_manager,
                    mission_control_mode,
                    mouse_drag_state,
                );
                if result == SpaceOperationOutcome::SameSpace {
                    daemon_fail!(response, "cannot swap space with itself.\n");
                } else if result == SpaceOperationOutcome::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot swap space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOperationOutcome::MissionControlIsActive {
                    daemon_fail!(
                        response,
                        "cannot swap space because mission-control is active.\n"
                    );
                } else if result == SpaceOperationOutcome::ScriptingAdditionFailed {
                    daemon_fail!(
                        response,
                        "cannot swap space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_DISPLAY) {
            let selector = parse_display_selector(
                response,
                message_cursor,
                query_display_showing_the_active_menu_bar(),
                false,
                display_manager,
            );
            if let Some(selector_display_id) = selector.resolved_target() {
                let result = send_space_to_display(
                    space_manager,
                    acting_space_id,
                    selector_display_id,
                    display_manager,
                    window_manager,
                    mission_control_mode,
                );
                if result == SpaceOperationOutcome::MissingSource {
                    daemon_fail!(response, "could not locate the space to act on.\n");
                } else if result == SpaceOperationOutcome::MissingDestination {
                    daemon_fail!(
                        response,
                        "could not locate the active space of the given display.\n"
                    );
                } else if result == SpaceOperationOutcome::InvalidSource {
                    daemon_fail!(
                        response,
                        "acting space is the last user-space on the source display and cannot be moved.\n"
                    );
                } else if result == SpaceOperationOutcome::InvalidDestination {
                    daemon_fail!(
                        response,
                        "acting space is already located on the given display.\n"
                    );
                } else if result == SpaceOperationOutcome::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot send space to display because it is in the middle of an animation.\n"
                    );
                } else if result == SpaceOperationOutcome::MissionControlIsActive {
                    daemon_fail!(
                        response,
                        "cannot send space to display because mission-control is active.\n"
                    );
                } else if result == SpaceOperationOutcome::ScriptingAdditionFailed {
                    daemon_fail!(
                        response,
                        "cannot send space to display due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_CREATE) {
            let selector = parse_display_selector(
                response,
                message_cursor,
                query_display_showing_the_active_menu_bar(),
                true,
                display_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_display_id) = selector.resolved_target() {
                    acting_space_id = query_current_space_of_display(selector_display_id);
                } else {
                    return;
                }
            }

            let result = add_space_on_display_of_space(acting_space_id, mission_control_mode);
            if result == SpaceOperationOutcome::MissingSource {
                daemon_fail!(response, "could not locate the space to act on.\n");
            } else if result == SpaceOperationOutcome::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot create space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOperationOutcome::MissionControlIsActive {
                daemon_fail!(
                    response,
                    "cannot create space because mission-control is active.\n"
                );
            } else if result == SpaceOperationOutcome::ScriptingAdditionFailed {
                daemon_fail!(
                    response,
                    "cannot create space due to an error with the scripting-addition.\n"
                );
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_DESTROY) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                true,
                space_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_space_id) = selector.resolved_target() {
                    acting_space_id = selector_space_id;
                } else {
                    return;
                }
            }

            let result = destroy_user_space_unless_it_is_the_last_of_its_display(
                acting_space_id,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            );
            if result == SpaceOperationOutcome::MissingSource {
                daemon_fail!(response, "could not locate the space to act on.\n");
            } else if result == SpaceOperationOutcome::InvalidSource {
                daemon_fail!(
                    response,
                    "acting space is the last user-space on the source display and cannot be destroyed.\n"
                );
            } else if result == SpaceOperationOutcome::NotAUserSpace {
                daemon_fail!(response, "cannot destroy a macOS fullscreen space.\n");
            } else if result == SpaceOperationOutcome::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot destroy space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOperationOutcome::MissionControlIsActive {
                daemon_fail!(
                    response,
                    "cannot destroy space because mission-control is active.\n"
                );
            } else if result == SpaceOperationOutcome::ScriptingAdditionFailed {
                daemon_fail!(
                    response,
                    "cannot destroy space due to an error with the scripting-addition.\n"
                );
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_EQUALIZE) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                if !reset_split_ratios_in_view_of_space_to_the_global_ratio(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Horizontal as u32 | WindowNodeSplit::Vertical as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_X)
            {
                if !reset_split_ratios_in_view_of_space_to_the_global_ratio(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Horizontal as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_Y)
            {
                if !reset_split_ratios_in_view_of_space_to_the_global_ratio(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Vertical as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_BALANCE) {
            let value = message_cursor.take_next_token();
            if !value.is_not_empty() {
                if !balance_split_ratios_in_view_of_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Horizontal as u32 | WindowNodeSplit::Vertical as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_X)
            {
                if !balance_split_ratios_in_view_of_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Horizontal as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_Y)
            {
                if !balance_split_ratios_in_view_of_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Vertical as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_MIRROR) {
            let value = message_cursor.take_next_token();
            if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_X) {
                if !mirror_view_of_space_along_axis(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Horizontal,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot mirror a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_COMMON_VALUE_AXIS_Y)
            {
                if !mirror_view_of_space_along_axis(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Vertical,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot mirror a non-managed space.\n");
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_ROTATE) {
            let value = message_cursor.take_next_token();
            if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_90) {
                if !rotate_view_of_space_by_degrees(
                    space_manager,
                    acting_space_id,
                    90,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_180) {
                if !rotate_view_of_space_by_degrees(
                    space_manager,
                    acting_space_id,
                    180,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_270) {
                if !rotate_view_of_space_by_degrees(
                    space_manager,
                    acting_space_id,
                    270,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_PADDING) {
            let mut top: libc::c_int = 0;
            let mut bottom: libc::c_int = 0;
            let mut left: libc::c_int = 0;
            let mut right: libc::c_int = 0;
            let mut type_of_change = [0 as libc::c_char; FIXED_STRING_BUFFER_LENGTH];
            let value = message_cursor.take_next_token();
            let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                .unwrap_or_default();
            let converted = unsafe {
                libc::sscanf(
                    subject.as_ptr(),
                    ARGUMENT_SPACE_PADDING.as_ptr(),
                    type_of_change.as_mut_ptr(),
                    &mut top as *mut libc::c_int,
                    &mut bottom as *mut libc::c_int,
                    &mut left as *mut libc::c_int,
                    &mut right as *mut libc::c_int,
                )
            };
            if converted == 5 {
                if !set_padding_of_space(
                    space_manager,
                    acting_space_id,
                    parse_absolute_or_relative_change_type(&type_of_change) as i32,
                    top,
                    bottom,
                    left,
                    right,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot set padding for a non-managed space.\n");
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_GAP) {
            let mut gap: libc::c_int = 0;
            let mut type_of_change = [0 as libc::c_char; FIXED_STRING_BUFFER_LENGTH];
            let value = message_cursor.take_next_token();
            let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                .unwrap_or_default();
            let converted = unsafe {
                libc::sscanf(
                    subject.as_ptr(),
                    ARGUMENT_SPACE_GAP.as_ptr(),
                    type_of_change.as_mut_ptr(),
                    &mut gap as *mut libc::c_int,
                )
            };
            if converted == 2 {
                if !set_window_gap_of_space(
                    space_manager,
                    acting_space_id,
                    parse_absolute_or_relative_change_type(&type_of_change) as i32,
                    gap,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot set gap for a non-managed space.\n");
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_TOGGLE) {
            let value = message_cursor.take_next_token();
            if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_SPACE_TOGGLE_PADDING) {
                if !toggle_padding_of_space(
                    space_manager,
                    acting_space_id,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot toggle padding for a non-managed space.\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_SPACE_TOGGLE_GAP) {
                if !toggle_window_gap_of_space(
                    space_manager,
                    acting_space_id,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot toggle gap for a non-managed space.\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_SPACE_TOGGLE_MISSION_CONTROL,
            ) {
                focus_space_then_toggle_mission_control(
                    acting_space_id,
                    window_manager,
                    mission_control_mode,
                );
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_SPACE_TOGGLE_SHOW_DESKTOP,
            ) {
                focus_space_then_toggle_show_desktop(
                    acting_space_id,
                    window_manager,
                    mission_control_mode,
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_LAYOUT) {
            let value = message_cursor.take_next_token();
            if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_SPACE_LAYOUT_BINARY_SPACE_PARTITIONING,
            ) {
                if is_user_space(acting_space_id) {
                    set_layout_of_space_retiling_its_windows(
                        space_manager,
                        acting_space_id,
                        ViewLayout::BinarySpacePartitioning,
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
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_STACK)
            {
                if is_user_space(acting_space_id) {
                    set_layout_of_space_retiling_its_windows(
                        space_manager,
                        acting_space_id,
                        ViewLayout::Stack,
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
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_FLOAT)
            {
                if is_user_space(acting_space_id) {
                    set_layout_of_space_retiling_its_windows(
                        space_manager,
                        acting_space_id,
                        ViewLayout::Float,
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
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_SPACE_LABEL) {
            let mut label = None;
            let token = message_cursor.take_next_token();
            if parse_label_refusing_numbers_and_reserved_words(
                response,
                message_cursor.bytes(),
                token,
                LabelType::Space,
                &mut label,
            ) {
                if let Some(label) = label {
                    set_label_of_space_removing_it_from_any_other_space(
                        space_manager,
                        acting_space_id,
                        label,
                    );
                } else if !remove_label_of_space(space_manager, acting_space_id) {
                    daemon_fail!(
                        response,
                        "the selected space was not associated with a label!\n"
                    );
                }
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
