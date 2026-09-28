use crate::command::DaemonCommand;
use crate::display::manager::DisplayManager;
use crate::message::domain::config::run_config_command;
use crate::message::domain::display::run_display_command;
use crate::message::domain::query::run_query_command;
use crate::message::domain::rule::run_rule_command;
use crate::message::domain::scratchpad::run_scratchpad_command;
use crate::message::domain::signal::run_signal_command;
use crate::message::domain::space::run_space_command;
use crate::message::domain::window::run_window_command;
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
        DaemonCommand::Display(display_command) => {
            DaemonReply::printing_or_failing_with(run_display_command(
                display_command,
                display_manager,
                window_manager,
                space_manager,
                mission_control_mode,
            ))
        }
        DaemonCommand::Space(space_command) => {
            DaemonReply::printing_or_failing_with(run_space_command(
                space_command,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
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
        DaemonCommand::Rule(rule_command) => {
            DaemonReply::printing_or_failing_with(run_rule_command(
                rule_command,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            ))
        }
        DaemonCommand::Signal(signal_command) => {
            DaemonReply::printing_or_failing_with(run_signal_command(signal_command, signal_event))
        }
        DaemonCommand::Window(window_command) => {
            DaemonReply::printing_or_failing_with(run_window_command(
                window_command,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            ))
        }
        DaemonCommand::Scratchpad(scratchpad_command) => {
            DaemonReply::printing_or_failing_with(run_scratchpad_command(
                scratchpad_command,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            ))
        }
    }
}
