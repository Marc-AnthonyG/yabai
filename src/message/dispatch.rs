use crate::display::manager::DisplayManager;
use crate::message::domain::config::run_config_command;
use crate::message::domain::display::run_display_command;
use crate::message::domain::query::run_query_command;
use crate::message::domain::rule::run_rule_command;
use crate::message::domain::signal::run_signal_command;
use crate::message::domain::space::run_space_command;
use crate::message::domain::window::run_window_command;
use crate::message::token::{MessageCursor, is_token_equal_to};
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

pub(crate) fn dispatch_message_to_its_domain(
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
    let domain = message_cursor.take_next_token();
    if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_CONFIG) {
        run_config_command(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    } else if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_DISPLAY) {
        run_display_command(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mission_control_mode,
        );
    } else if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_SPACE) {
        run_space_command(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_WINDOW) {
        run_window_command(
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
    } else if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_QUERY) {
        run_query_command(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    } else if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_RULE) {
        run_rule_command(
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
    } else if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_SIGNAL) {
        run_signal_command(response, domain, &mut message_cursor, signal_event);
    } else {
        response.write_failure_pieces_unless_silent(&[
            FailurePiece::Text("unknown domain '"),
            FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
            FailurePiece::Text("'\n"),
        ]);
    }
}
