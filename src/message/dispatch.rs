use crate::command::DaemonCommand;
use crate::display::manager::DisplayManager;
use crate::message::domain::config::run_config_command;
use crate::message::domain::query::run_query_command;
use crate::message::not_yet_typed_bridge::run_message_to_a_domain_not_yet_typed;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::protocol::reply::DaemonReply;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal};
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::window::manager::WindowManager;

pub(crate) fn run_daemon_command(
    command: DaemonCommand,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> DaemonReply {
    match command {
        DaemonCommand::Config(config_command) => {
            DaemonReply::printing_or_failing_with_every_failure(run_config_command(
                config_command,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            ))
        }
        DaemonCommand::Query(query_command) => {
            DaemonReply::printing_or_failing_with(run_query_command(
                query_command,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            ))
        }
        DaemonCommand::NotYetTyped { arguments } => run_message_to_a_domain_not_yet_typed(
            &arguments,
            signal_event,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        ),
    }
}
