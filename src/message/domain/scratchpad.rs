use crate::command::scratchpad::ScratchpadCommand;
use crate::display::manager::DisplayManager;
use crate::message::selector_resolution::resolve_window_selector_or_the_focused_window;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::window::manager::WindowManager;
use crate::window::scratchpad::{
    assign_window_to_scratchpad_making_it_float,
    recover_hidden_scratchpad_windows_by_ordering_every_window_in,
    remove_window_from_its_scratchpad, scratchpad_window_with_label, toggle_scratchpad_window,
};

const TOGGLE_BETWEEN_SHOWN_AND_HIDDEN: i32 = 0;

pub(crate) fn run_scratchpad_command(
    command: ScratchpadCommand,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Result<String, String> {
    match command {
        ScratchpadCommand::Assign { name, window } => {
            let window_id = resolve_window_selector_or_the_focused_window(
                window.as_ref(),
                display_manager,
                window_manager,
                space_manager,
            )?;
            if !assign_window_to_scratchpad_making_it_float(
                window_manager,
                window_id,
                name,
                process_manager,
                display_manager,
                space_manager,
                mouse_drag_state,
            ) {
                return Err(String::from(
                    "the given scratchpad is already assigned to a different window.",
                ));
            }
        }
        ScratchpadCommand::Unassign { window } => {
            let window_id = resolve_window_selector_or_the_focused_window(
                window.as_ref(),
                display_manager,
                window_manager,
                space_manager,
            )?;
            if !remove_window_from_its_scratchpad(
                window_manager,
                window_id,
                true,
                process_manager,
                display_manager,
                space_manager,
                mouse_drag_state,
            ) {
                return Err(String::from(
                    "the selected window was not assigned to a scratchpad.",
                ));
            }
        }
        ScratchpadCommand::Toggle { name } => {
            let window_id = scratchpad_window_with_label(window_manager, name.as_bytes())
                .ok_or_else(|| format!("no window is the scratchpad '{name}'."))?;
            if !toggle_scratchpad_window(
                window_manager,
                window_id,
                TOGGLE_BETWEEN_SHOWN_AND_HIDDEN,
                process_manager,
            ) {
                return Err(String::from("could not locate the focused space."));
            }
        }
        ScratchpadCommand::Recover => {
            recover_hidden_scratchpad_windows_by_ordering_every_window_in(
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            )
        }
    }
    Ok(String::new())
}
