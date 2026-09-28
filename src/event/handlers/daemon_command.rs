use std::os::unix::net::UnixStream;

use crate::command::DaemonCommand;
use crate::display::manager::DisplayManager;
use crate::message::dispatch::run_daemon_command;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::protocol::reply::write_reply;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal};
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::window::manager::WindowManager;

pub(crate) fn handle_daemon_command_event(
    command: DaemonCommand,
    reply_to: UnixStream,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    crate::debug!(
        "handle_daemon_command_event: {}\n",
        serde_json::to_string(&command).unwrap_or_default()
    );

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

    let _ = write_reply(&reply_to, &reply);
}
