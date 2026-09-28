use crate::command::space::{SpaceAction, SpaceCommand, SpaceToggleableSetting, TreeAxis};
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::layout::tree::WindowNodeSplit;
use crate::message::selector_resolution::{
    resolve_display_selector, resolve_space_selector, resolve_space_selector_or_the_focused_space,
};
use crate::mouse::drag::MouseDragState;
use crate::space::focus::{
    focus_space_then_toggle_mission_control, focus_space_then_toggle_show_desktop,
    focus_space_through_the_scripting_addition_or_dock_swipes,
    switch_to_space_bringing_it_to_the_current_display,
};
use crate::space::labels::{
    remove_label_of_space, set_label_of_space_removing_it_from_any_other_space,
};
use crate::space::managed_space::is_user_space;
use crate::space::manager::SpaceManager;
use crate::space::operations::{
    SpaceOperationOutcome, add_space_on_display_of_space,
    destroy_user_space_unless_it_is_the_last_of_its_display, move_space_to_position_of_space,
    send_space_to_display, swap_space_with_space,
};
use crate::space::tiling::{
    balance_split_ratios_in_view_of_space, mirror_view_of_space_along_axis,
    reset_split_ratios_in_view_of_space_to_the_global_ratio, rotate_view_of_space_by_degrees,
};
use crate::space::view_settings::{
    set_layout_of_space_retiling_its_windows, set_padding_of_space, set_window_gap_of_space,
    toggle_padding_of_space, toggle_window_gap_of_space,
};
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::SpaceId;
use crate::window::manager::WindowManager;

pub(crate) fn run_space_command(
    command: SpaceCommand,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Result<String, String> {
    let acting_space_id = resolve_space_selector_or_the_focused_space(
        command.space.as_ref(),
        window_manager,
        space_manager,
    )?;

    match command.action {
        SpaceAction::Focus { target } => {
            let target_space_id = resolve_space_selector(&target, acting_space_id, space_manager)?;
            fail_unless_the_space_was_focused(
                focus_space_through_the_scripting_addition_or_dock_swipes(
                    target_space_id,
                    window_manager,
                    mission_control_mode,
                ),
            )?;
        }
        SpaceAction::Switch { target } => {
            let target_space_id = resolve_space_selector(&target, acting_space_id, space_manager)?;
            fail_unless_the_space_was_focused(switch_to_space_bringing_it_to_the_current_display(
                target_space_id,
                display_manager,
                window_manager,
                space_manager,
                mission_control_mode,
                mouse_drag_state,
            ))?;
        }
        SpaceAction::Create { display } => {
            let space_on_the_display = match display {
                Some(display) => query_current_space_of_display(resolve_display_selector(
                    &display,
                    query_display_showing_the_active_menu_bar(),
                    display_manager,
                )?),
                None => acting_space_id,
            };
            match add_space_on_display_of_space(space_on_the_display, mission_control_mode) {
                SpaceOperationOutcome::MissingSource => {
                    return Err(String::from("could not locate the space to act on."));
                }
                outcome => fail_when_the_display_is_busy_or_the_scripting_addition_fails(
                    outcome,
                    "create space",
                )?,
            }
        }
        SpaceAction::Destroy => {
            match destroy_user_space_unless_it_is_the_last_of_its_display(
                acting_space_id,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            ) {
                SpaceOperationOutcome::MissingSource => {
                    return Err(String::from("could not locate the space to act on."));
                }
                SpaceOperationOutcome::InvalidSource => {
                    return Err(String::from(
                        "acting space is the last user-space on the source display and cannot be destroyed.",
                    ));
                }
                SpaceOperationOutcome::NotAUserSpace => {
                    return Err(String::from("cannot destroy a macOS fullscreen space."));
                }
                outcome => fail_when_the_display_is_busy_or_the_scripting_addition_fails(
                    outcome,
                    "destroy space",
                )?,
            }
        }
        SpaceAction::Move { target } => {
            let target_space_id = resolve_space_selector(&target, acting_space_id, space_manager)?;
            match move_space_to_position_of_space(
                acting_space_id,
                target_space_id,
                window_manager,
                mission_control_mode,
            ) {
                SpaceOperationOutcome::SameSpace => {
                    return Err(String::from("cannot move space to itself."));
                }
                SpaceOperationOutcome::NotOnTheSameDisplay => {
                    return Err(String::from(
                        "cannot move space across display boundaries; use send-to-display instead.",
                    ));
                }
                outcome => fail_when_the_display_is_busy_or_the_scripting_addition_fails(
                    outcome,
                    "move space",
                )?,
            }
        }
        SpaceAction::Swap { target } => {
            let target_space_id = resolve_space_selector(&target, acting_space_id, space_manager)?;
            match swap_space_with_space(
                acting_space_id,
                target_space_id,
                display_manager,
                window_manager,
                space_manager,
                mission_control_mode,
                mouse_drag_state,
            ) {
                SpaceOperationOutcome::SameSpace => {
                    return Err(String::from("cannot swap space with itself."));
                }
                outcome => fail_when_the_display_is_busy_or_the_scripting_addition_fails(
                    outcome,
                    "swap space",
                )?,
            }
        }
        SpaceAction::SendToDisplay { display } => {
            let display_id = resolve_display_selector(
                &display,
                query_display_showing_the_active_menu_bar(),
                display_manager,
            )?;
            fail_unless_the_space_was_sent_to_the_display(send_space_to_display(
                space_manager,
                acting_space_id,
                display_id,
                display_manager,
                window_manager,
                mission_control_mode,
            ))?;
        }
        SpaceAction::Equalize { axis } => {
            if !reset_split_ratios_in_view_of_space_to_the_global_ratio(
                space_manager,
                acting_space_id,
                splits_along_the_axis_or_both(axis),
                display_manager,
                window_manager,
            ) {
                return Err(String::from("cannot equalize a non-managed space."));
            }
        }
        SpaceAction::Balance { axis } => {
            if !balance_split_ratios_in_view_of_space(
                space_manager,
                acting_space_id,
                splits_along_the_axis_or_both(axis),
                display_manager,
                window_manager,
            ) {
                return Err(String::from("cannot balance a non-managed space."));
            }
        }
        SpaceAction::Mirror { axis } => {
            if !mirror_view_of_space_along_axis(
                space_manager,
                acting_space_id,
                axis.split_of_the_tree_along_this_axis(),
                display_manager,
                window_manager,
            ) {
                return Err(String::from("cannot mirror a non-managed space."));
            }
        }
        SpaceAction::Rotate { degrees } => {
            if !rotate_view_of_space_by_degrees(
                space_manager,
                acting_space_id,
                degrees.degrees(),
                display_manager,
                window_manager,
            ) {
                return Err(String::from("cannot rotate a non-managed space."));
            }
        }
        SpaceAction::Padding {
            change,
            top,
            bottom,
            left,
            right,
        } => {
            if !set_padding_of_space(
                space_manager,
                acting_space_id,
                change,
                top,
                bottom,
                left,
                right,
                display_manager,
                window_manager,
            ) {
                return Err(String::from("cannot set padding for a non-managed space."));
            }
        }
        SpaceAction::Gap { change, gap } => {
            if !set_window_gap_of_space(
                space_manager,
                acting_space_id,
                change,
                gap,
                display_manager,
                window_manager,
            ) {
                return Err(String::from("cannot set gap for a non-managed space."));
            }
        }
        SpaceAction::Toggle { setting } => toggle_setting_of_space(
            setting,
            acting_space_id,
            display_manager,
            window_manager,
            space_manager,
            mission_control_mode,
        )?,
        SpaceAction::Layout { layout } => {
            if !is_user_space(acting_space_id) {
                return Err(String::from(
                    "cannot set layout for a macOS fullscreen space.",
                ));
            }
            set_layout_of_space_retiling_its_windows(
                space_manager,
                acting_space_id,
                layout,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
        }
        SpaceAction::Label { label: Some(label) } => {
            set_label_of_space_removing_it_from_any_other_space(
                space_manager,
                acting_space_id,
                label,
            );
        }
        SpaceAction::Label { label: None } => {
            if !remove_label_of_space(space_manager, acting_space_id) {
                return Err(String::from(
                    "the selected space was not associated with a label.",
                ));
            }
        }
    }
    Ok(String::new())
}

pub(crate) fn fail_when_the_display_is_busy_or_the_scripting_addition_fails(
    outcome: SpaceOperationOutcome,
    operation: &str,
) -> Result<(), String> {
    match outcome {
        SpaceOperationOutcome::DisplayIsAnimating => Err(format!(
            "cannot {operation} because the display is in the middle of an animation."
        )),
        SpaceOperationOutcome::MissionControlIsActive => Err(format!(
            "cannot {operation} because mission-control is active."
        )),
        SpaceOperationOutcome::ScriptingAdditionFailed => Err(format!(
            "cannot {operation} due to an error with the scripting-addition."
        )),
        _ => Ok(()),
    }
}

fn fail_unless_the_space_was_focused(outcome: SpaceOperationOutcome) -> Result<(), String> {
    match outcome {
        SpaceOperationOutcome::SameSpace => {
            Err(String::from("cannot focus an already focused space."))
        }
        outcome => {
            fail_when_the_display_is_busy_or_the_scripting_addition_fails(outcome, "focus space")
        }
    }
}

fn fail_unless_the_space_was_sent_to_the_display(
    outcome: SpaceOperationOutcome,
) -> Result<(), String> {
    match outcome {
        SpaceOperationOutcome::MissingSource => {
            Err(String::from("could not locate the space to act on."))
        }
        SpaceOperationOutcome::MissingDestination => Err(String::from(
            "could not locate the active space of the given display.",
        )),
        SpaceOperationOutcome::InvalidSource => Err(String::from(
            "acting space is the last user-space on the source display and cannot be moved.",
        )),
        SpaceOperationOutcome::InvalidDestination => Err(String::from(
            "acting space is already located on the given display.",
        )),
        outcome => fail_when_the_display_is_busy_or_the_scripting_addition_fails(
            outcome,
            "send space to display",
        ),
    }
}

fn splits_along_the_axis_or_both(axis: Option<TreeAxis>) -> u32 {
    match axis {
        Some(axis) => axis.split_of_the_tree_along_this_axis() as u32,
        None => WindowNodeSplit::Horizontal as u32 | WindowNodeSplit::Vertical as u32,
    }
}

fn toggle_setting_of_space(
    setting: SpaceToggleableSetting,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
) -> Result<(), String> {
    match setting {
        SpaceToggleableSetting::Padding => {
            if !toggle_padding_of_space(space_manager, space_id, display_manager, window_manager) {
                return Err(String::from(
                    "cannot toggle padding for a non-managed space.",
                ));
            }
        }
        SpaceToggleableSetting::Gap => {
            if !toggle_window_gap_of_space(space_manager, space_id, display_manager, window_manager)
            {
                return Err(String::from("cannot toggle gap for a non-managed space."));
            }
        }
        SpaceToggleableSetting::MissionControl => {
            focus_space_then_toggle_mission_control(space_id, window_manager, mission_control_mode);
        }
        SpaceToggleableSetting::ShowDesktop => {
            focus_space_then_toggle_show_desktop(space_id, window_manager, mission_control_mode);
        }
    }
    Ok(())
}
