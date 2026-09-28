use crate::daemon_fail;
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::manager::DisplayManager;
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_option_given_to_command_for_domain,
};
use crate::message::properties::parse_comma_separated_properties;
use crate::message::selectors::{
    parse_display_selector, parse_space_selector, parse_window_selector,
};
use crate::message::token::{MessageCursor, Token, is_token_equal_to};
use crate::mouse::drag::MouseDragState;
use crate::query::displays::write_every_display_as_json_array;
use crate::query::spaces::{
    write_space_as_json_object_followed_by_newline, write_spaces_of_display_as_json_array,
    write_spaces_of_every_display_as_json_array, write_spaces_of_window_as_json_array,
};
use crate::query::windows::{
    write_windows_on_display_as_json_array, write_windows_on_every_display_as_json_array,
    write_windows_on_spaces_as_json_array,
};
use crate::serialise::display::{
    DISPLAY_PROPERTY_NAMES, DISPLAY_PROPERTY_SELECTION_BITS, write_display_as_json_object,
};
use crate::serialise::space::{SPACE_PROPERTY_NAMES, SPACE_PROPERTY_SELECTION_BITS};
use crate::serialise::window::{
    WINDOW_PROPERTY_NAMES, WINDOW_PROPERTY_SELECTION_BITS, write_tracked_window_as_json_object,
};
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::managed_space::query_display_holding_space;
use crate::space::manager::SpaceManager;
use crate::support::response::Response;
use crate::window::focus::query_focused_tracked_window;
use crate::window::manager::WindowManager;
use crate::window::model::query_display_holding_window;

/* --------------------------------DOMAIN QUERY--------------------------------- */
pub(crate) const COMMAND_QUERY_DISPLAYS: &str = "--displays";
pub(crate) const COMMAND_QUERY_SPACES: &str = "--spaces";
pub(crate) const COMMAND_QUERY_WINDOWS: &str = "--windows";

pub(crate) const ARGUMENT_QUERY_DISPLAY: &str = "--display";
pub(crate) const ARGUMENT_QUERY_SPACE: &str = "--space";
pub(crate) const ARGUMENT_QUERY_WINDOW: &str = "--window";
/* ----------------------------------------------------------------------------- */

pub(crate) fn run_query_command(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let command = message_cursor.take_next_token();
    if is_token_equal_to(command, message_cursor.bytes(), COMMAND_QUERY_DISPLAYS) {
        let token = message_cursor.take_next_token();
        let properties = parse_comma_separated_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &DISPLAY_PROPERTY_SELECTION_BITS,
            &DISPLAY_PROPERTY_NAMES,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.take_next_token()
        } else {
            properties.token
        };
        if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = query_display_showing_the_active_menu_bar();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_display_id) = selector.resolved_target() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            write_display_as_json_object(
                response,
                acting_display_id,
                properties.flags,
                display_manager,
            );
            response.write(format_args!("\n"));
        } else if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = query_current_space_of_the_focused_display(window_manager);
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

            write_display_as_json_object(
                response,
                query_display_holding_space(acting_space_id),
                properties.flags,
                display_manager,
            );
            response.write(format_args!("\n"));
        } else if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = query_focused_tracked_window(window_manager);
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_window_id) = selector.resolved_target() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                write_display_as_json_object(
                    response,
                    query_display_holding_window(acting_window),
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
        } else if option.is_not_empty() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else {
            write_every_display_as_json_array(response, properties.flags, display_manager);
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_QUERY_SPACES) {
        let token = message_cursor.take_next_token();
        let properties = parse_comma_separated_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &SPACE_PROPERTY_SELECTION_BITS,
            &SPACE_PROPERTY_NAMES,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.take_next_token()
        } else {
            properties.token
        };
        if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = query_display_showing_the_active_menu_bar();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_display_id) = selector.resolved_target() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            if !write_spaces_of_display_as_json_array(
                response,
                acting_display_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
            ) {
                daemon_fail!(response, "could not retrieve spaces for display.\n");
            }
        } else if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = query_current_space_of_the_focused_display(window_manager);
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

            if !write_space_as_json_object_followed_by_newline(
                response,
                acting_space_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
            ) {
                daemon_fail!(response, "could not retrieve space details.\n");
            }
        } else if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = query_focused_tracked_window(window_manager);
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_window_id) = selector.resolved_target() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                write_spaces_of_window_as_json_array(
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
        } else if option.is_not_empty() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else if !write_spaces_of_every_display_as_json_array(
            response,
            properties.flags,
            display_manager,
            window_manager,
            space_manager,
        ) {
            daemon_fail!(response, "could not retrieve spaces for displays.\n");
        }
    } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_QUERY_WINDOWS) {
        let token = message_cursor.take_next_token();
        let properties = parse_comma_separated_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &WINDOW_PROPERTY_SELECTION_BITS,
            &WINDOW_PROPERTY_NAMES,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.take_next_token()
        } else {
            properties.token
        };
        if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = query_display_showing_the_active_menu_bar();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_display_id) = selector.resolved_target() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            write_windows_on_display_as_json_array(
                response,
                acting_display_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = query_current_space_of_the_focused_display(window_manager);
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

            write_windows_on_spaces_as_json_array(
                response,
                &[acting_space_id],
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else if is_token_equal_to(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = query_focused_tracked_window(window_manager);
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_window_id) = selector.resolved_target() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                write_tracked_window_as_json_object(
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
        } else if option.is_not_empty() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else {
            write_windows_on_every_display_as_json_array(
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
