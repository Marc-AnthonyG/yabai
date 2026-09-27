use crate::daemon_fail;
use crate::display::focus::{display_manager_focus_display, display_manager_focus_space};
use crate::display::identity::display_manager_active_display_id;
use crate::display::labels::{
    display_manager_remove_label_for_display, display_manager_set_label_for_display,
};
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_id;
use crate::message::common_failures::daemon_fail_with_unknown_command_for_domain;
use crate::message::labels::{LabelType, parse_label};
use crate::message::selectors::{parse_display_selector, parse_space_selector};
use crate::message::token::{MessageCursor, Token, token_equals};
use crate::space::manager::SpaceManager;
use crate::space::operations::SpaceOpError;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::DisplayId;
use crate::support::response::Response;
use crate::window::manager::WindowManager;

/* --------------------------------DOMAIN DISPLAY------------------------------- */
pub(crate) const COMMAND_DISPLAY_FOCUS: &str = "--focus";
pub(crate) const COMMAND_DISPLAY_SPACE: &str = "--space";
pub(crate) const COMMAND_DISPLAY_LABEL: &str = "--label";
/* ----------------------------------------------------------------------------- */

pub(crate) fn handle_domain_display(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
) {
    let command;
    let mut acting_display_id = display_manager_active_display_id();
    let selector = parse_display_selector(
        &mut Response::silent(),
        message_cursor,
        acting_display_id,
        true,
        display_manager,
    );

    if selector.did_parse() {
        acting_display_id = selector.resolved().unwrap_or(DisplayId(0));
        command = message_cursor.get_token();
    } else {
        command = selector.token;
    }

    if acting_display_id == DisplayId(0) {
        daemon_fail!(response, "could not locate the display to act on!\n");
        return;
    }

    if token_equals(command, message_cursor.bytes(), COMMAND_DISPLAY_FOCUS) {
        let selector = parse_display_selector(
            response,
            message_cursor,
            acting_display_id,
            false,
            display_manager,
        );
        if let Some(selector_display_id) = selector.resolved() {
            if acting_display_id != selector_display_id {
                display_manager_focus_display(
                    selector_display_id,
                    display_space_id(selector_display_id),
                    window_manager,
                );
            } else {
                daemon_fail!(response, "cannot focus an already focused display.\n");
            }
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_DISPLAY_SPACE) {
        let selector = parse_space_selector(
            response,
            message_cursor,
            display_space_id(acting_display_id),
            false,
            space_manager,
        );
        if let Some(selector_space_id) = selector.resolved() {
            let result =
                display_manager_focus_space(acting_display_id, selector_space_id, mission_control_mode);
            if result == SpaceOpError::SameDisplay {
                daemon_fail!(response, "acting display does not contain the given space.\n");
            } else if result == SpaceOpError::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot focus space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOpError::InMissionControl {
                daemon_fail!(response, "cannot focus space because mission-control is active.\n");
            } else if result == SpaceOpError::ScriptingAddition {
                daemon_fail!(
                    response,
                    "cannot focus space due to an error with the scripting-addition.\n"
                );
            }
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_DISPLAY_LABEL) {
        let mut label = None;
        let token = message_cursor.get_token();
        if parse_label(
            response,
            message_cursor.bytes(),
            token,
            LabelType::Display,
            &mut label,
        ) {
            if let Some(label) = label {
                display_manager_set_label_for_display(display_manager, acting_display_id, label);
            } else if !display_manager_remove_label_for_display(display_manager, acting_display_id)
            {
                daemon_fail!(
                    response,
                    "the selected display was not associated with a label!\n"
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
}
