use crate::display::manager::DisplayManager;
use crate::message::domain::display::run_display_command;
use crate::message::domain::space::run_space_command;
use crate::message::domain::window::run_window_command;
use crate::message::token::{MessageCursor, is_token_equal_to};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::protocol::reply::DaemonReply;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::response::{FailurePiece, Response};
use crate::window::manager::WindowManager;

const DOMAIN_DISPLAY: &str = "display";
const DOMAIN_SPACE: &str = "space";
const DOMAIN_WINDOW: &str = "window";

pub(crate) fn run_message_to_a_domain_not_yet_typed(
    arguments: &[String],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> DaemonReply {
    let mut message = every_argument_null_terminated_as_the_tokenizer_reads_them(arguments);
    let mut response = Response::collecting();
    let mut message_cursor = MessageCursor::new(&mut message);
    let domain = message_cursor.take_next_token();

    if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_DISPLAY) {
        run_display_command(
            &mut response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mission_control_mode,
        );
    } else if is_token_equal_to(domain, message_cursor.bytes(), DOMAIN_SPACE) {
        run_space_command(
            &mut response,
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
            &mut response,
            domain,
            &mut message_cursor,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else {
        response.write_failure_pieces_unless_silent(&[
            FailurePiece::Text("unknown domain '"),
            FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
            FailurePiece::Text("'\n"),
        ]);
    }

    let (standard_output, failures) = response.into_standard_output_and_one_failure_per_line();
    DaemonReply {
        standard_output,
        failures,
    }
}

const NULLS_AFTER_THE_LAST_ARGUMENT: usize = 3;

fn every_argument_null_terminated_as_the_tokenizer_reads_them(arguments: &[String]) -> Vec<u8> {
    let mut message = Vec::new();
    for argument in arguments {
        message.extend_from_slice(argument.as_bytes());
        message.push(0);
    }
    message.extend_from_slice(&[0; NULLS_AFTER_THE_LAST_ARGUMENT]);
    message
}
