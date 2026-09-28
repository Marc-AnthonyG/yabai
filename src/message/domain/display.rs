use crate::command::display::{DisplayAction, DisplayCommand};
use crate::display::focus::{
    focus_display_through_its_front_window_or_a_click_at_its_center,
    focus_space_if_it_is_on_display,
};
use crate::display::labels::{
    remove_label_of_display, set_label_of_display_removing_it_from_any_other_display,
};
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::message::domain::space::fail_when_the_display_is_busy_or_the_scripting_addition_fails;
use crate::message::selector_resolution::{
    resolve_display_selector, resolve_display_selector_or_the_display_showing_the_active_menu_bar,
    resolve_space_selector,
};
use crate::space::manager::SpaceManager;
use crate::space::operations::SpaceOperationOutcome;
use crate::state::mission_control_mode::MissionControlMode;
use crate::window::manager::WindowManager;

pub(crate) fn run_display_command(
    command: DisplayCommand,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
) -> Result<String, String> {
    let acting_display_id = resolve_display_selector_or_the_display_showing_the_active_menu_bar(
        command.display.as_ref(),
        display_manager,
    )?;

    match command.action {
        DisplayAction::Focus { target } => {
            let target_display_id =
                resolve_display_selector(&target, acting_display_id, display_manager)?;
            if target_display_id == acting_display_id {
                return Err(String::from("cannot focus an already focused display."));
            }
            focus_display_through_its_front_window_or_a_click_at_its_center(
                target_display_id,
                query_current_space_of_display(target_display_id),
                window_manager,
            );
        }
        DisplayAction::ShowSpace { space } => {
            let space_id = resolve_space_selector(
                &space,
                query_current_space_of_display(acting_display_id),
                space_manager,
            )?;
            match focus_space_if_it_is_on_display(acting_display_id, space_id, mission_control_mode)
            {
                SpaceOperationOutcome::NotOnTheSameDisplay => {
                    return Err(String::from(
                        "acting display does not contain the given space.",
                    ));
                }
                outcome => fail_when_the_display_is_busy_or_the_scripting_addition_fails(
                    outcome,
                    "focus space",
                )?,
            }
        }
        DisplayAction::Label { label: Some(label) } => {
            set_label_of_display_removing_it_from_any_other_display(
                display_manager,
                acting_display_id,
                label,
            );
        }
        DisplayAction::Label { label: None } => {
            if !remove_label_of_display(display_manager, acting_display_id) {
                return Err(String::from(
                    "the selected display was not associated with a label.",
                ));
            }
        }
    }
    Ok(String::new())
}
