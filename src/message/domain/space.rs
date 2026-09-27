use crate::daemon_fail;
use crate::display::identity::display_manager_active_display_id;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_id;
use crate::layout::settings::ViewType;
use crate::layout::tree::WindowNodeSplit;
use crate::message::argument_prefixes::parse_value_type;
use crate::message::common_arguments::{ARGUMENT_COMMON_VAL_AXIS_X, ARGUMENT_COMMON_VAL_AXIS_Y};
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_value_given_to_command_for_domain,
};
use crate::message::labels::{LabelType, parse_label};
use crate::message::selectors::{parse_display_selector, parse_space_selector};
use crate::message::token::{MessageCursor, Token, token_equals};
use crate::mouse::drag::MouseDragState;
use crate::space::focus::{
    space_manager_active_space, space_manager_focus_space, space_manager_switch_space,
    space_manager_toggle_mission_control, space_manager_toggle_show_desktop,
};
use crate::space::labels::{
    space_manager_remove_label_for_space, space_manager_set_label_for_space,
};
use crate::space::managed_space::space_is_user;
use crate::space::manager::SpaceManager;
use crate::space::operations::{
    SpaceOpError, space_manager_add_space, space_manager_destroy_space,
    space_manager_move_space_to_display, space_manager_move_space_to_space,
    space_manager_swap_space_with_space,
};
use crate::space::tiling::{
    space_manager_balance_space, space_manager_equalize_space, space_manager_mirror_space,
    space_manager_rotate_space,
};
use crate::space::view_settings::{
    space_manager_set_gap_for_space, space_manager_set_layout_for_space,
    space_manager_set_padding_for_space, space_manager_toggle_gap_for_space,
    space_manager_toggle_padding_for_space,
};
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::SpaceId;
use crate::support::response::Response;
use crate::support::strings::MAXLEN;
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
pub(crate) const ARGUMENT_SPACE_TGL_PADDING: &str = "padding";
pub(crate) const ARGUMENT_SPACE_TGL_GAP: &str = "gap";
pub(crate) const ARGUMENT_SPACE_TGL_MC: &str = "mission-control";
pub(crate) const ARGUMENT_SPACE_TGL_SD: &str = "show-desktop";
pub(crate) const ARGUMENT_SPACE_LAYOUT_BSP: &str = "bsp";
pub(crate) const ARGUMENT_SPACE_LAYOUT_STACK: &str = "stack";
pub(crate) const ARGUMENT_SPACE_LAYOUT_FLT: &str = "float";
/* ----------------------------------------------------------------------------- */

pub(crate) fn handle_domain_space(
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
    let mut acting_space_id = space_manager_active_space(window_manager);
    let selector = parse_space_selector(
        &mut Response::silent(),
        message_cursor,
        acting_space_id,
        true,
        space_manager,
    );

    if selector.did_parse() {
        acting_space_id = selector.resolved().unwrap_or(SpaceId(0));
        command = message_cursor.get_token();
    } else {
        command = selector.token;
    }

    if acting_space_id == SpaceId(0) {
        daemon_fail!(response, "could not locate the space to act on!\n");
        return;
    }

    while command.is_valid() {
        if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_FOCUS) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result =
                    space_manager_focus_space(selector_space_id, window_manager, mission_control_mode);
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot focus an already focused space.\n");
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot focus space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot focus space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot focus space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_SWITCH) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result = space_manager_switch_space(
                    selector_space_id,
                    display_manager,
                    window_manager,
                    space_manager,
                    mission_control_mode,
                    mouse_drag_state,
                );
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot focus an already focused space.\n");
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot focus space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot focus space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot focus space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_MOVE) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result = space_manager_move_space_to_space(
                    acting_space_id,
                    selector_space_id,
                    window_manager,
                    mission_control_mode,
                );
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot move space to itself.\n");
                } else if result == SpaceOpError::SameDisplay {
                    daemon_fail!(
                        response,
                        "cannot move space across display boundaries. use --display instead.\n"
                    );
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot move space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot move space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot move space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_SWAP) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result = space_manager_swap_space_with_space(
                    acting_space_id,
                    selector_space_id,
                    display_manager,
                    window_manager,
                    space_manager,
                    mission_control_mode,
                    mouse_drag_state,
                );
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot swap space with itself.\n");
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot swap space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot swap space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot swap space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_DISPLAY) {
            let selector = parse_display_selector(
                response,
                message_cursor,
                display_manager_active_display_id(),
                false,
                display_manager,
            );
            if let Some(selector_display_id) = selector.resolved() {
                let result = space_manager_move_space_to_display(
                    space_manager,
                    acting_space_id,
                    selector_display_id,
                    display_manager,
                    window_manager,
                    mission_control_mode,
                );
                if result == SpaceOpError::MissingSrc {
                    daemon_fail!(response, "could not locate the space to act on.\n");
                } else if result == SpaceOpError::MissingDst {
                    daemon_fail!(
                        response,
                        "could not locate the active space of the given display.\n"
                    );
                } else if result == SpaceOpError::InvalidSrc {
                    daemon_fail!(
                        response,
                        "acting space is the last user-space on the source display and cannot be moved.\n"
                    );
                } else if result == SpaceOpError::InvalidDst {
                    daemon_fail!(
                        response,
                        "acting space is already located on the given display.\n"
                    );
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot send space to display because it is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot send space to display because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot send space to display due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_CREATE) {
            let selector = parse_display_selector(
                response,
                message_cursor,
                display_manager_active_display_id(),
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_space_id = display_space_id(selector_display_id);
                } else {
                    return;
                }
            }

            let result = space_manager_add_space(acting_space_id, mission_control_mode);
            if result == SpaceOpError::MissingSrc {
                daemon_fail!(response, "could not locate the space to act on.\n");
            } else if result == SpaceOpError::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot create space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOpError::InMissionControl {
                daemon_fail!(
                    response,
                    "cannot create space because mission-control is active.\n"
                );
            } else if result == SpaceOpError::ScriptingAddition {
                daemon_fail!(
                    response,
                    "cannot create space due to an error with the scripting-addition.\n"
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_DESTROY) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                true,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_space_id) = selector.resolved() {
                    acting_space_id = selector_space_id;
                } else {
                    return;
                }
            }

            let result = space_manager_destroy_space(
                acting_space_id,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            );
            if result == SpaceOpError::MissingSrc {
                daemon_fail!(response, "could not locate the space to act on.\n");
            } else if result == SpaceOpError::InvalidSrc {
                daemon_fail!(
                    response,
                    "acting space is the last user-space on the source display and cannot be destroyed.\n"
                );
            } else if result == SpaceOpError::InvalidType {
                daemon_fail!(response, "cannot destroy a macOS fullscreen space.\n");
            } else if result == SpaceOpError::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot destroy space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOpError::InMissionControl {
                daemon_fail!(
                    response,
                    "cannot destroy space because mission-control is active.\n"
                );
            } else if result == SpaceOpError::ScriptingAddition {
                daemon_fail!(
                    response,
                    "cannot destroy space due to an error with the scripting-addition.\n"
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_EQUALIZE) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                if !space_manager_equalize_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                if !space_manager_equalize_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                if !space_manager_equalize_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Y as u32,
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_BALANCE) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                if !space_manager_balance_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                if !space_manager_balance_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                if !space_manager_balance_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Y as u32,
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_MIRROR) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                if !space_manager_mirror_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot mirror a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                if !space_manager_mirror_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Y,
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_ROTATE) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_90) {
                if !space_manager_rotate_space(
                    space_manager,
                    acting_space_id,
                    90,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_180) {
                if !space_manager_rotate_space(
                    space_manager,
                    acting_space_id,
                    180,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_270) {
                if !space_manager_rotate_space(
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_PADDING) {
            let mut top: libc::c_int = 0;
            let mut bottom: libc::c_int = 0;
            let mut left: libc::c_int = 0;
            let mut right: libc::c_int = 0;
            let mut type_of_change = [0 as libc::c_char; MAXLEN];
            let value = message_cursor.get_token();
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
                if !space_manager_set_padding_for_space(
                    space_manager,
                    acting_space_id,
                    parse_value_type(&type_of_change) as i32,
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_GAP) {
            let mut gap: libc::c_int = 0;
            let mut type_of_change = [0 as libc::c_char; MAXLEN];
            let value = message_cursor.get_token();
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
                if !space_manager_set_gap_for_space(
                    space_manager,
                    acting_space_id,
                    parse_value_type(&type_of_change) as i32,
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_TOGGLE) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_PADDING) {
                if !space_manager_toggle_padding_for_space(
                    space_manager,
                    acting_space_id,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot toggle padding for a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_GAP) {
                if !space_manager_toggle_gap_for_space(
                    space_manager,
                    acting_space_id,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot toggle gap for a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_MC) {
                space_manager_toggle_mission_control(
                    acting_space_id,
                    window_manager,
                    mission_control_mode,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_SD) {
                space_manager_toggle_show_desktop(
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_LAYOUT) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_BSP) {
                if space_is_user(acting_space_id) {
                    space_manager_set_layout_for_space(
                        space_manager,
                        acting_space_id,
                        ViewType::Bsp,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                } else {
                    daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_STACK) {
                if space_is_user(acting_space_id) {
                    space_manager_set_layout_for_space(
                        space_manager,
                        acting_space_id,
                        ViewType::Stack,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                } else {
                    daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_FLT) {
                if space_is_user(acting_space_id) {
                    space_manager_set_layout_for_space(
                        space_manager,
                        acting_space_id,
                        ViewType::Float,
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_LABEL) {
            let mut label = None;
            let token = message_cursor.get_token();
            if parse_label(
                response,
                message_cursor.bytes(),
                token,
                LabelType::Space,
                &mut label,
            ) {
                if let Some(label) = label {
                    space_manager_set_label_for_space(space_manager, acting_space_id, label);
                } else if !space_manager_remove_label_for_space(space_manager, acting_space_id) {
                    daemon_fail!(response, "the selected space was not associated with a label!\n");
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

        command = message_cursor.get_token();
    }
}
