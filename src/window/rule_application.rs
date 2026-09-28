#![allow(deprecated)]

use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::ffi::accessibility::{AXUIElementSetAttributeValue, kAXFullscreenAttribute};
use crate::ffi::core_foundation::CFBoolean;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::space::focus::focus_space_through_the_scripting_addition_or_dock_swipes;
use crate::space::managed_space::is_native_fullscreen_space;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::WindowId;
use crate::support::regex::is_subject_rejected_by_optional_pattern;
use crate::window::floating_and_sticky::{set_whether_window_floats, set_whether_window_is_sticky};
use crate::window::grid::place_floating_window_on_display_grid;
use crate::window::layer::set_window_layer_for_it_and_its_child_windows;
use crate::window::manager::{WindowManager, is_window_eligible_for_management};
use crate::window::model::{
    WindowRuleFlag, is_window_in_native_fullscreen_according_to_accessibility,
    query_space_holding_window, window_role_as_string, window_subrole_as_string,
    window_title_as_string,
};
use crate::window::opacity::apply_opacity_to_window_through_scripting_addition;
use crate::window::rule::{
    RULE_PROPERTY_OFF, RULE_PROPERTY_ON, Rule, RuleEffects, RuleEffectsFlag, RuleFlag,
    combine_rule_effects_into_accumulated_effects,
};
use crate::window::scratchpad::assign_window_to_scratchpad_making_it_float;
use crate::window::send_to_space::send_window_to_space;

pub(crate) fn is_window_matched_by_rule(
    rule: &Rule,
    window_id: WindowId,
    window_title: &str,
    window_role: &str,
    window_subrole: &str,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(application_process_id) = window_manager
        .window
        .get(&window_id)
        .and_then(|window| window.application)
    else {
        return false;
    };
    let Some(application) = window_manager.application.get(&application_process_id) else {
        return false;
    };
    !(is_subject_rejected_by_optional_pattern(
        rule.app_regex.as_ref(),
        rule.flags
            .contains(RuleFlag::APPLICATION_PATTERN_IS_NEGATED),
        &application.name,
    ) || is_subject_rejected_by_optional_pattern(
        rule.title_regex.as_ref(),
        rule.flags.contains(RuleFlag::TITLE_PATTERN_IS_NEGATED),
        window_title,
    ) || is_subject_rejected_by_optional_pattern(
        rule.role_regex.as_ref(),
        rule.flags.contains(RuleFlag::ROLE_PATTERN_IS_NEGATED),
        window_role,
    ) || is_subject_rejected_by_optional_pattern(
        rule.subrole_regex.as_ref(),
        rule.flags.contains(RuleFlag::SUBROLE_PATTERN_IS_NEGATED),
        window_subrole,
    ))
}

pub(crate) fn apply_manage_effect_of_rule_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    effects: &RuleEffects,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if effects.manage == RULE_PROPERTY_ON {
        if let Some(window) = window_manager.window.get_mut(&window_id) {
            window.rule_flags.insert(WindowRuleFlag::MANAGE_FORCED_ON);
        }
        set_whether_window_floats(
            space_manager,
            window_manager,
            window_id,
            false,
            true,
            display_manager,
            mouse_drag_state,
        );
    } else if effects.manage == RULE_PROPERTY_OFF {
        if let Some(window) = window_manager.window.get_mut(&window_id) {
            window.rule_flags.remove(WindowRuleFlag::MANAGE_FORCED_ON);
        }
        set_whether_window_floats(
            space_manager,
            window_manager,
            window_id,
            true,
            true,
            display_manager,
            mouse_drag_state,
        );
    }
}

pub(crate) fn apply_effects_other_than_manage_of_rule_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    effects: &RuleEffects,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if effects.space_id.0 != 0 || effects.display_id.0 != 0 {
        let window_is_in_native_fullscreen = window_manager
            .window
            .get(&window_id)
            .is_some_and(is_window_in_native_fullscreen_according_to_accessibility);

        if !window_is_in_native_fullscreen
            && !is_native_fullscreen_space(query_space_holding_window(window_id))
        {
            let space_id = if effects.space_id.0 != 0 {
                effects.space_id
            } else {
                query_current_space_of_display(effects.display_id)
            };
            send_window_to_space(
                space_manager,
                window_manager,
                window_id,
                space_id,
                true,
                process_manager,
                display_manager,
                mouse_drag_state,
            );
            if effects
                .flags
                .contains(RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE)
                || effects.fullscreen == RULE_PROPERTY_ON
            {
                focus_space_through_the_scripting_addition_or_dock_swipes(
                    space_id,
                    window_manager,
                    mission_control_mode,
                );
            }
        }
    }

    if effects.sticky == RULE_PROPERTY_ON {
        set_whether_window_is_sticky(
            space_manager,
            window_manager,
            window_id,
            true,
            display_manager,
            mouse_drag_state,
        );
    } else if effects.sticky == RULE_PROPERTY_OFF {
        set_whether_window_is_sticky(
            space_manager,
            window_manager,
            window_id,
            false,
            display_manager,
            mouse_drag_state,
        );
    }

    if effects.mff == RULE_PROPERTY_ON {
        if let Some(window) = window_manager.window.get_mut(&window_id) {
            window
                .rule_flags
                .insert(WindowRuleFlag::OVERRIDES_MOUSE_FOLLOWS_FOCUS);
            window
                .rule_flags
                .insert(WindowRuleFlag::MOUSE_FOLLOWS_FOCUS_OVERRIDE_IS_ON);
        }
    } else if effects.mff == RULE_PROPERTY_OFF {
        if let Some(window) = window_manager.window.get_mut(&window_id) {
            window
                .rule_flags
                .insert(WindowRuleFlag::OVERRIDES_MOUSE_FOLLOWS_FOCUS);
            window
                .rule_flags
                .remove(WindowRuleFlag::MOUSE_FOLLOWS_FOCUS_OVERRIDE_IS_ON);
        }
    }

    if effects.flags.contains(RuleEffectsFlag::LAYER_IS_SET) {
        set_window_layer_for_it_and_its_child_windows(window_id, effects.layer, window_manager);
    }

    if effects.flags.contains(RuleEffectsFlag::OPACITY_IS_SET)
        && (0.0..=1.0).contains(&effects.opacity)
    {
        if let Some(window) = window_manager.window.get_mut(&window_id) {
            window.opacity = effects.opacity;
        }
        apply_opacity_to_window_through_scripting_addition(
            window_manager,
            window_id,
            effects.opacity,
        );
    }

    if effects.fullscreen == RULE_PROPERTY_ON {
        if let Some(window) = window_manager.window.get_mut(&window_id) {
            let window_element_ref = window.element_ref;
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXFullscreenAttribute(),
                    CFBoolean::new(true),
                )
            };
            window
                .rule_flags
                .insert(WindowRuleFlag::NATIVE_FULLSCREEN_REQUESTED);
        }
    }

    if let Some(effects_scratchpad) = effects.scratchpad.as_deref() {
        let scratchpad = effects_scratchpad.to_owned();
        assign_window_to_scratchpad_making_it_float(
            window_manager,
            window_id,
            scratchpad,
            process_manager,
            display_manager,
            space_manager,
            mouse_drag_state,
        );
    }

    if effects.grid[0] != 0 && effects.grid[1] != 0 {
        place_floating_window_on_display_grid(
            space_manager,
            window_manager,
            window_id,
            effects.grid[0],
            effects.grid[1],
            effects.grid[2],
            effects.grid[3],
            effects.grid[4],
            effects.grid[5],
            display_manager,
        );
    }
}

pub(crate) fn apply_manage_effect_of_matching_rules_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    window_title: &str,
    window_role: &str,
    window_subrole: &str,
    one_shot_rules: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut matched = false;
    let mut effects = RuleEffects::default();

    let mut rules = std::mem::take(&mut window_manager.rules);

    for index in 0..rules.len() {
        if one_shot_rules || !rules[index].flags.contains(RuleFlag::ONE_SHOT) {
            if is_window_matched_by_rule(
                &rules[index],
                window_id,
                window_title,
                window_role,
                window_subrole,
                window_manager,
            ) {
                if rules[index].effects.manage == RULE_PROPERTY_ON {
                    if rules[index].role_regex.is_none() && window_role != "AXWindow" {
                        continue;
                    }
                    if rules[index].subrole_regex.is_none() && window_subrole != "AXStandardWindow"
                    {
                        continue;
                    }
                }

                matched = true;
                combine_rule_effects_into_accumulated_effects(&rules[index].effects, &mut effects);

                if rules[index].flags.contains(RuleFlag::ONE_SHOT) {
                    rules[index]
                        .flags
                        .insert(RuleFlag::ONE_SHOT_DUE_FOR_REMOVAL);
                }
            }
        }
    }

    window_manager.rules = rules;

    if matched {
        apply_manage_effect_of_rule_to_window(
            space_manager,
            window_manager,
            window_id,
            &effects,
            display_manager,
            mouse_drag_state,
        );
    }
}

pub(crate) fn apply_effects_other_than_manage_of_matching_rules_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    window_title: &str,
    window_role: &str,
    window_subrole: &str,
    one_shot_rules: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut matched = false;
    let mut effects = RuleEffects::default();

    let mut rules = std::mem::take(&mut window_manager.rules);

    for index in 0..rules.len() {
        if one_shot_rules || !rules[index].flags.contains(RuleFlag::ONE_SHOT) {
            if is_window_matched_by_rule(
                &rules[index],
                window_id,
                window_title,
                window_role,
                window_subrole,
                window_manager,
            ) {
                let window_is_managed_by_rule =
                    window_manager.window.get(&window_id).is_some_and(|window| {
                        window.rule_flags.contains(WindowRuleFlag::MANAGE_FORCED_ON)
                    });
                if !window_is_managed_by_rule {
                    if rules[index].role_regex.is_none() && window_role != "AXWindow" {
                        continue;
                    }
                    if rules[index].subrole_regex.is_none() && window_subrole != "AXStandardWindow"
                    {
                        continue;
                    }
                }

                matched = true;
                combine_rule_effects_into_accumulated_effects(&rules[index].effects, &mut effects);

                if rules[index].flags.contains(RuleFlag::ONE_SHOT) {
                    rules[index]
                        .flags
                        .insert(RuleFlag::ONE_SHOT_DUE_FOR_REMOVAL);
                }
            }
        }
    }

    window_manager.rules = rules;

    if matched {
        apply_effects_other_than_manage_of_rule_to_window(
            space_manager,
            window_manager,
            window_id,
            &effects,
            process_manager,
            display_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    }
}

pub(crate) fn reapply_every_rule_except_one_shot_rules_to_every_root_window(
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    for window_id in window_manager.window.keys().copied().collect::<Vec<_>>() {
        let Some(window) = window_manager.window.get(&window_id) else {
            continue;
        };

        if window.is_root {
            let window_title = window_title_as_string(window);
            let window_role = window_role_as_string(window);
            let window_subrole = window_subrole_as_string(window);

            apply_manage_effect_of_matching_rules_to_window(
                space_manager,
                window_manager,
                window_id,
                &window_title,
                &window_role,
                &window_subrole,
                false,
                display_manager,
                mouse_drag_state,
            );

            if is_window_eligible_for_management(window_id, window_manager) {
                if let Some(window) = window_manager.window.get_mut(&window_id) {
                    window.is_eligible = true;
                }
                apply_effects_other_than_manage_of_matching_rules_to_window(
                    space_manager,
                    window_manager,
                    window_id,
                    &window_title,
                    &window_role,
                    &window_subrole,
                    false,
                    process_manager,
                    display_manager,
                    mouse_drag_state,
                    mission_control_mode,
                );
            }
        }
    }
}

pub(crate) fn reapply_rule_at_index_to_every_root_window(
    index: i32,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> bool {
    for rule_index in 0..window_manager.rules.len() {
        if rule_index as i32 == index {
            if !window_manager.rules[rule_index]
                .flags
                .contains(RuleFlag::ONE_SHOT)
            {
                let rule = std::mem::take(&mut window_manager.rules[rule_index]);
                apply_rule_to_every_matching_root_window(
                    &rule,
                    process_manager,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                    mission_control_mode,
                );
                window_manager.rules[rule_index] = rule;
            }
            return true;
        }
    }

    false
}

pub(crate) fn reapply_rule_with_label_to_every_root_window(
    label: &[u8],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> bool {
    let label = String::from_utf8_lossy(label);

    for rule_index in 0..window_manager.rules.len() {
        if window_manager.rules[rule_index].label.as_deref() == Some(&*label) {
            if !window_manager.rules[rule_index]
                .flags
                .contains(RuleFlag::ONE_SHOT)
            {
                let rule = std::mem::take(&mut window_manager.rules[rule_index]);
                apply_rule_to_every_matching_root_window(
                    &rule,
                    process_manager,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                    mission_control_mode,
                );
                window_manager.rules[rule_index] = rule;
            }
            return true;
        }
    }

    false
}

pub(crate) fn apply_rule_to_every_matching_root_window(
    rule: &Rule,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    for window_id in window_manager.window.keys().copied().collect::<Vec<_>>() {
        let Some(window) = window_manager.window.get(&window_id) else {
            continue;
        };

        if window.is_root {
            let window_title = window_title_as_string(window);
            let window_role = window_role_as_string(window);
            let window_subrole = window_subrole_as_string(window);

            if is_window_matched_by_rule(
                rule,
                window_id,
                &window_title,
                &window_role,
                &window_subrole,
                window_manager,
            ) {
                apply_manage_effect_of_rule_to_window(
                    space_manager,
                    window_manager,
                    window_id,
                    &rule.effects,
                    display_manager,
                    mouse_drag_state,
                );

                if is_window_eligible_for_management(window_id, window_manager) {
                    if let Some(window) = window_manager.window.get_mut(&window_id) {
                        window.is_eligible = true;
                    }
                    apply_effects_other_than_manage_of_rule_to_window(
                        space_manager,
                        window_manager,
                        window_id,
                        &rule.effects,
                        process_manager,
                        display_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    );
                }
            }
        }
    }
}
