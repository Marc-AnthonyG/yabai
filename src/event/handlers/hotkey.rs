use crate::command::{parse_daemon_command, refusal_of_clap_error_without_usage};
use crate::display::manager::DisplayManager;
use crate::hotkey::chord::Chord;
use crate::message::dispatch::run_daemon_command;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal};
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::window::manager::WindowManager;

pub(crate) fn handle_hotkey_pressed_event(
    chord: Chord,
    yabai_command_arguments: Vec<String>,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    crate::debug!(
        "handle_hotkey_pressed_event: {chord}: {}\n",
        yabai_command_arguments.join(" ")
    );

    let command = match parse_daemon_command(&yabai_command_arguments) {
        Ok(command) => command,
        Err(error) => {
            crate::warn!(
                "yabai: hotkey '{chord}': {}\n",
                refusal_of_clap_error_without_usage(&error)
            );
            return;
        }
    };

    let reply = run_daemon_command(
        command,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
        mission_control_mode,
    );

    for failure in reply.failures {
        crate::warn!("yabai: hotkey '{chord}': {failure}\n");
    }
}
