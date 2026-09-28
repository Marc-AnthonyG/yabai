use crate::daemon_fail;
use crate::display::focus::{
    focus_display_through_its_front_window_or_a_click_at_its_center,
    focus_space_if_it_is_on_display,
};
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::labels::{
    remove_label_of_display, set_label_of_display_removing_it_from_any_other_display,
};
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::message::common_failures::daemon_fail_with_unknown_command_for_domain;
use crate::message::labels::{LabelType, parse_label_refusing_numbers_and_reserved_words};
use crate::message::selectors::{parse_display_selector, parse_space_selector};
use crate::message::token::{MessageCursor, Token, is_token_equal_to};
use crate::space::manager::SpaceManager;
use crate::space::operations::SpaceOperationOutcome;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::DisplayId;
use crate::support::response::Response;
use crate::window::manager::WindowManager;

/* --------------------------------DOMAIN DISPLAY------------------------------- */
pub(crate) const COMMAND_DISPLAY_FOCUS: &str = "--focus";
pub(crate) const COMMAND_DISPLAY_SPACE: &str = "--space";
pub(crate) const COMMAND_DISPLAY_LABEL: &str = "--label";
/* ----------------------------------------------------------------------------- */

pub(crate) fn run_display_command(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
) {
    let command;
    let mut acting_display_id = query_display_showing_the_active_menu_bar();
    let selector = parse_display_selector(
        &mut Response::silent(),
        message_cursor,
        acting_display_id,
        true,
        display_manager,
    );

    if selector.is_recognised_selector() {
        acting_display_id = selector.resolved_target().unwrap_or(DisplayId(0));
        command = message_cursor.take_next_token();
    } else {
        command = selector.token;
    }

    if acting_display_id == DisplayId(0) {
        daemon_fail!(response, "could not locate the display to act on!\n");
        return;
    }

    if is_token_equal_to(command, message_cursor.bytes(), COMMAND_DISPLAY_FOCUS) {
        let selector = parse_display_selector(
            response,
            message_cursor,
            acting_display_id,
            false,
            display_manager,
        );
        if let Some(selector_display_id) = selector.resolved_target() {
            if acting_display_id != selector_display_id {
                focus_display_through_its_front_window_or_a_click_at_its_center(
                    selector_display_id,
                    query_current_space_of_display(selector_display_id),
                    window_manager,
                );
            } else {
                daemon_fail!(response, "cannot focus an already focused display.\n");
            }
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_DISPLAY_SPACE) {
        let selector = parse_space_selector(
            response,
            message_cursor,
            query_current_space_of_display(acting_display_id),
            false,
            space_manager,
        );
        if let Some(selector_space_id) = selector.resolved_target() {
            let result = focus_space_if_it_is_on_display(
                acting_display_id,
                selector_space_id,
                mission_control_mode,
            );
            if result == SpaceOperationOutcome::NotOnTheSameDisplay {
                daemon_fail!(
                    response,
                    "acting display does not contain the given space.\n"
                );
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
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_DISPLAY_LABEL) {
        let mut label = None;
        let token = message_cursor.take_next_token();
        if parse_label_refusing_numbers_and_reserved_words(
            response,
            message_cursor.bytes(),
            token,
            LabelType::Display,
            &mut label,
        ) {
            if let Some(label) = label {
                set_label_of_display_removing_it_from_any_other_display(
                    display_manager,
                    acting_display_id,
                    label,
                );
            } else if !remove_label_of_display(display_manager, acting_display_id) {
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
