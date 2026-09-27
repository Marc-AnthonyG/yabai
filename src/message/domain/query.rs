use crate::daemon_fail;
use crate::display::identity::display_manager_active_display_id;
use crate::display::manager::DisplayManager;
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_option_given_to_command_for_domain,
};
use crate::message::properties::parse_properties;
use crate::message::selectors::{
    parse_display_selector, parse_space_selector, parse_window_selector,
};
use crate::message::token::{MessageCursor, Token, token_equals};
use crate::mouse::drag::MouseDragState;
use crate::query::displays::display_manager_query_displays;
use crate::query::spaces::{
    space_manager_query_space, space_manager_query_spaces_for_display,
    space_manager_query_spaces_for_displays, space_manager_query_spaces_for_window,
};
use crate::query::windows::{
    window_manager_query_windows_for_display, window_manager_query_windows_for_displays,
    window_manager_query_windows_for_spaces,
};
use crate::serialise::display::{DISPLAY_PROPERTY_STR, DISPLAY_PROPERTY_VAL, display_serialize};
use crate::serialise::space::{SPACE_PROPERTY_STR, SPACE_PROPERTY_VAL};
use crate::serialise::window::{WINDOW_PROPERTY_STR, WINDOW_PROPERTY_VAL, window_serialize};
use crate::space::focus::space_manager_active_space;
use crate::space::managed_space::space_display_id;
use crate::space::manager::SpaceManager;
use crate::support::response::Response;
use crate::window::focus::window_manager_focused_window;
use crate::window::manager::WindowManager;
use crate::window::model::window_display_id;

/* --------------------------------DOMAIN QUERY--------------------------------- */
pub(crate) const COMMAND_QUERY_DISPLAYS: &str = "--displays";
pub(crate) const COMMAND_QUERY_SPACES: &str = "--spaces";
pub(crate) const COMMAND_QUERY_WINDOWS: &str = "--windows";

pub(crate) const ARGUMENT_QUERY_DISPLAY: &str = "--display";
pub(crate) const ARGUMENT_QUERY_SPACE: &str = "--space";
pub(crate) const ARGUMENT_QUERY_WINDOW: &str = "--window";
/* ----------------------------------------------------------------------------- */

pub(crate) fn handle_domain_query(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let command = message_cursor.get_token();
    if token_equals(command, message_cursor.bytes(), COMMAND_QUERY_DISPLAYS) {
        let token = message_cursor.get_token();
        let properties = parse_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &DISPLAY_PROPERTY_VAL,
            &DISPLAY_PROPERTY_STR,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.get_token()
        } else {
            properties.token
        };
        if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = display_manager_active_display_id();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            display_serialize(response, acting_display_id, properties.flags, display_manager);
            response.write(format_args!("\n"));
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = space_manager_active_space(window_manager);
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

            display_serialize(
                response,
                space_display_id(acting_space_id),
                properties.flags,
                display_manager,
            );
            response.write(format_args!("\n"));
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = window_manager_focused_window(window_manager);
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_window_id) = selector.resolved() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                display_serialize(
                    response,
                    window_display_id(acting_window),
                    properties.flags,
                    display_manager,
                );
                response.write(format_args!("\n"));
            } else {
                daemon_fail!(
                    response,
                    "could not find window to retrieve display details.\n"
                );
            }
        } else if option.is_valid() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else {
            display_manager_query_displays(response, properties.flags, display_manager);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_QUERY_SPACES) {
        let token = message_cursor.get_token();
        let properties = parse_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &SPACE_PROPERTY_VAL,
            &SPACE_PROPERTY_STR,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.get_token()
        } else {
            properties.token
        };
        if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = display_manager_active_display_id();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            if !space_manager_query_spaces_for_display(
                response,
                acting_display_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
            ) {
                daemon_fail!(response, "could not retrieve spaces for display.\n");
            }
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = space_manager_active_space(window_manager);
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

            if !space_manager_query_space(
                response,
                acting_space_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
            ) {
                daemon_fail!(response, "could not retrieve space details.\n");
            }
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = window_manager_focused_window(window_manager);
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_window_id) = selector.resolved() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                space_manager_query_spaces_for_window(
                    response,
                    acting_window,
                    properties.flags,
                    display_manager,
                    window_manager,
                    space_manager,
                );
            } else {
                daemon_fail!(response, "could not find window to retrieve space details.\n");
            }
        } else if option.is_valid() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else if !space_manager_query_spaces_for_displays(
            response,
            properties.flags,
            display_manager,
            window_manager,
            space_manager,
        ) {
            daemon_fail!(response, "could not retrieve spaces for displays.\n");
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_QUERY_WINDOWS) {
        let token = message_cursor.get_token();
        let properties = parse_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &WINDOW_PROPERTY_VAL,
            &WINDOW_PROPERTY_STR,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.get_token()
        } else {
            properties.token
        };
        if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = display_manager_active_display_id();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            window_manager_query_windows_for_display(
                response,
                acting_display_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = space_manager_active_space(window_manager);
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

            window_manager_query_windows_for_spaces(
                response,
                &[acting_space_id],
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = window_manager_focused_window(window_manager);
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_window_id) = selector.resolved() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                window_serialize(
                    response,
                    acting_window,
                    properties.flags,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                );
                response.write(format_args!("\n"));
            } else {
                daemon_fail!(response, "could not retrieve window details.\n");
            }
        } else if option.is_valid() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else {
            window_manager_query_windows_for_displays(
                response,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
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
