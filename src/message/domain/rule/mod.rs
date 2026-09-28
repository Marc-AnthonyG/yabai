pub mod building_a_rule;

use crate::command::rule::{RuleCommand, RuleDefinition};
use crate::command::selectors::IndexOrLabelSelector;
use crate::display::manager::DisplayManager;
use crate::message::domain::rule::building_a_rule::{
    does_the_definition_set_anything, rule_of_definition,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::serialise::rule::every_rule_as_pretty_json;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::window::manager::WindowManager;
use crate::window::rule::{
    add_rule_replacing_any_with_the_same_label, remove_rule_at_index, remove_rule_with_label,
};
use crate::window::rule_application::{
    apply_rule_to_every_matching_root_window,
    reapply_every_rule_except_one_shot_rules_to_every_root_window,
    reapply_rule_at_index_to_every_root_window, reapply_rule_with_label_to_every_root_window,
};

pub(crate) fn run_rule_command(
    command: RuleCommand,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Result<String, String> {
    match command {
        RuleCommand::Add {
            one_shot,
            definition,
        } => {
            let rule = rule_of_definition(
                &definition,
                one_shot,
                display_manager,
                window_manager,
                space_manager,
            )?;
            add_rule_replacing_any_with_the_same_label(rule, window_manager);
        }
        RuleCommand::Apply { rule, definition } => apply_a_rule_to_the_open_windows(
            rule.as_ref(),
            &definition,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        )?,
        RuleCommand::Remove { rule } => {
            let was_removed = match &rule {
                IndexOrLabelSelector::Index(index) => {
                    remove_rule_at_index(*index as usize, window_manager)
                }
                IndexOrLabelSelector::Label(label) => remove_rule_with_label(label, window_manager),
            };
            if !was_removed {
                return Err(failure_locating_the_rule(&rule));
            }
        }
        RuleCommand::List => {
            return Ok(every_rule_as_pretty_json(window_manager, display_manager) + "\n");
        }
    }
    Ok(String::new())
}

fn apply_a_rule_to_the_open_windows(
    rule: Option<&IndexOrLabelSelector>,
    definition: &RuleDefinition,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Result<(), String> {
    match rule {
        Some(rule) => {
            let was_found = match rule {
                IndexOrLabelSelector::Index(index) => reapply_rule_at_index_to_every_root_window(
                    *index as usize,
                    process_manager,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                    mission_control_mode,
                ),
                IndexOrLabelSelector::Label(label) => reapply_rule_with_label_to_every_root_window(
                    label,
                    process_manager,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                    mission_control_mode,
                ),
            };
            if !was_found {
                return Err(failure_locating_the_rule(rule));
            }
        }
        None if does_the_definition_set_anything(definition) => {
            let rule_applied_once = rule_of_definition(
                definition,
                false,
                display_manager,
                window_manager,
                space_manager,
            )?;
            apply_rule_to_every_matching_root_window(
                &rule_applied_once,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            );
        }
        None => reapply_every_rule_except_one_shot_rules_to_every_root_window(
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        ),
    }
    Ok(())
}

fn failure_locating_the_rule(rule: &IndexOrLabelSelector) -> String {
    match rule {
        IndexOrLabelSelector::Index(index) => format!("rule with index '{index}' not found."),
        IndexOrLabelSelector::Label(label) => format!("rule with label '{label}' not found."),
    }
}
