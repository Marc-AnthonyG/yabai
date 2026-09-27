use crate::display::manager::DisplayManager;
use crate::message::domain::config::handle_domain_config;
use crate::message::domain::display::handle_domain_display;
use crate::message::domain::query::handle_domain_query;
use crate::message::domain::rule::handle_domain_rule;
use crate::message::domain::signal::handle_domain_signal;
use crate::message::domain::space::handle_domain_space;
use crate::message::domain::window::handle_domain_window;
use crate::message::token::{MessageCursor, token_equals};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal};
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::response::{FailurePiece, Response};
use crate::window::manager::WindowManager;

pub(crate) const DOMAIN_CONFIG: &str = "config";
pub(crate) const DOMAIN_DISPLAY: &str = "display";
pub(crate) const DOMAIN_SPACE: &str = "space";
pub(crate) const DOMAIN_WINDOW: &str = "window";
pub(crate) const DOMAIN_QUERY: &str = "query";
pub(crate) const DOMAIN_RULE: &str = "rule";
pub(crate) const DOMAIN_SIGNAL: &str = "signal";

pub(crate) fn handle_message(
    response: &mut Response,
    message: &mut [u8],
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut message_cursor = MessageCursor::new(message);
    let domain = message_cursor.get_token();
    if token_equals(domain, message_cursor.bytes(), DOMAIN_CONFIG) {
        handle_domain_config(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_DISPLAY) {
        handle_domain_display(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_SPACE) {
        handle_domain_space(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_WINDOW) {
        handle_domain_window(
            response,
            domain,
            &mut message_cursor,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_QUERY) {
        handle_domain_query(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_RULE) {
        handle_domain_rule(
            response,
            domain,
            &mut message_cursor,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_SIGNAL) {
        handle_domain_signal(response, domain, &mut message_cursor, signal_event);
    } else {
        response.fail_pieces(&[
            FailurePiece::Text("unknown domain '"),
            FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
            FailurePiece::Text("'\n"),
        ]);
    }
}
