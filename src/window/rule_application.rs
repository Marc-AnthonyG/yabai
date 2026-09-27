#![allow(deprecated)]

use std::ffi::CString;

use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_id;
use crate::ffi::accessibility::{AXUIElementSetAttributeValue, kAXFullscreenAttribute};
use crate::ffi::core_foundation::{as_cftype, kCFBooleanTrue};
use crate::handles::WindowId;
use crate::mission_control::MissionControlMode;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::space::focus::space_manager_focus_space;
use crate::space::managed_space::space_is_fullscreen;
use crate::space::manager::SpaceManager;
use crate::support::arithmetic::in_range_ii;
use crate::support::regex::{RegexMatch, regex_match};
use crate::support::strings::{string_copy, string_equals};
use crate::window::floating_and_sticky::{
    window_manager_make_window_floating, window_manager_make_window_sticky,
};
use crate::window::grid::window_manager_apply_grid;
use crate::window::layer::window_manager_set_window_layer;
use crate::window::manager::{WindowManager, window_manager_is_window_eligible};
use crate::window::model::{
    WindowRuleFlag, window_check_rule_flag, window_clear_rule_flag, window_is_fullscreen,
    window_role_ts, window_set_rule_flag, window_space, window_subrole_ts, window_title_ts,
};
use crate::window::opacity::window_manager_set_opacity;
use crate::window::rule::{
    RULE_PROP_OFF, RULE_PROP_ON, Rule, RuleEffects, RuleEffectsFlag, RuleFlag, rule_combine_effects,
};
use crate::window::scratchpad::window_manager_set_scratchpad_for_window;
use crate::window::send_to_space::window_manager_send_window_to_space;

pub(crate) fn window_manager_rule_matches_window(
    rule: &Rule,
    window_id: WindowId,
    window_title: &str,
    window_role: &str,
    window_subrole: &str,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(application_process_id) = window_manager
        .window
        .find(&window_id)
        .and_then(|window| window.application)
    else {
        return false;
    };
    let Some(application) = window_manager.application.find(&application_process_id) else {
        return false;
    };
    let application_name = CString::new(application.name.as_bytes()).unwrap();

    let regex_match_app = if RuleFlag(rule.flags).contains(RuleFlag::APP_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.app_regex.as_ref(), &application_name) == regex_match_app {
        return false;
    }

    let window_title = CString::new(window_title).unwrap();
    let regex_match_title = if RuleFlag(rule.flags).contains(RuleFlag::TITLE_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.title_regex.as_ref(), &window_title) == regex_match_title {
        return false;
    }

    let window_role = CString::new(window_role).unwrap();
    let regex_match_role = if RuleFlag(rule.flags).contains(RuleFlag::ROLE_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.role_regex.as_ref(), &window_role) == regex_match_role {
        return false;
    }

    let window_subrole = CString::new(window_subrole).unwrap();
    let regex_match_subrole = if RuleFlag(rule.flags).contains(RuleFlag::SUBROLE_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.subrole_regex.as_ref(), &window_subrole) == regex_match_subrole {
        return false;
    }

    true
}

pub(crate) fn window_manager_apply_manage_rule_effects_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    effects: &RuleEffects,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if effects.manage == RULE_PROP_ON {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_rule_flag(window, WindowRuleFlag::MANAGED);
        }
        window_manager_make_window_floating(
            space_manager,
            window_manager,
            window_id,
            false,
            true,
            display_manager,
            mouse_drag_state,
        );
    } else if effects.manage == RULE_PROP_OFF {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_clear_rule_flag(window, WindowRuleFlag::MANAGED);
        }
        window_manager_make_window_floating(
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

pub(crate) fn window_manager_apply_rule_effects_to_window(
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
            .find(&window_id)
            .is_some_and(window_is_fullscreen);

        if !window_is_in_native_fullscreen && !space_is_fullscreen(window_space(window_id)) {
            let space_id = if effects.space_id.0 != 0 {
                effects.space_id
            } else {
                display_space_id(effects.display_id)
            };
            window_manager_send_window_to_space(
                space_manager,
                window_manager,
                window_id,
                space_id,
                true,
                process_manager,
                display_manager,
                mouse_drag_state,
            );
            if RuleEffectsFlag(effects.flags).contains(RuleEffectsFlag::FOLLOW_SPACE)
                || effects.fullscreen == RULE_PROP_ON
            {
                space_manager_focus_space(space_id, window_manager, mission_control_mode);
            }
        }
    }

    if effects.sticky == RULE_PROP_ON {
        window_manager_make_window_sticky(
            space_manager,
            window_manager,
            window_id,
            true,
            display_manager,
            mouse_drag_state,
        );
    } else if effects.sticky == RULE_PROP_OFF {
        window_manager_make_window_sticky(
            space_manager,
            window_manager,
            window_id,
            false,
            display_manager,
            mouse_drag_state,
        );
    }

    if effects.mff == RULE_PROP_ON {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_rule_flag(window, WindowRuleFlag::MFF);
            window_set_rule_flag(window, WindowRuleFlag::MFF_VALUE);
        }
    } else if effects.mff == RULE_PROP_OFF {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_rule_flag(window, WindowRuleFlag::MFF);
            window_clear_rule_flag(window, WindowRuleFlag::MFF_VALUE);
        }
    }

    if RuleEffectsFlag(effects.flags).contains(RuleEffectsFlag::LAYER) {
        window_manager_set_window_layer(window_id, effects.layer, window_manager);
    }

    if RuleEffectsFlag(effects.flags).contains(RuleEffectsFlag::OPACITY)
        && in_range_ii(effects.opacity, 0.0f32, 1.0f32)
    {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window.opacity = effects.opacity;
        }
        window_manager_set_opacity(window_manager, window_id, effects.opacity);
    }

    if effects.fullscreen == RULE_PROP_ON {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            let window_element_ref = window.element_ref;
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXFullscreenAttribute(),
                    as_cftype(kCFBooleanTrue()),
                )
            };
            window_set_rule_flag(window, WindowRuleFlag::FULLSCREEN);
        }
    }

    if let Some(effects_scratchpad) = effects.scratchpad.as_deref() {
        let scratchpad = string_copy(effects_scratchpad);
        window_manager_set_scratchpad_for_window(
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
        window_manager_apply_grid(
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

pub(crate) fn window_manager_apply_manage_rules_to_window(
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
        if one_shot_rules || !RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
            if window_manager_rule_matches_window(
                &rules[index],
                window_id,
                window_title,
                window_role,
                window_subrole,
                window_manager,
            ) {
                if rules[index].effects.manage == RULE_PROP_ON {
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::ROLE_VALID)
                        && !string_equals(Some(window_role), Some("AXWindow"))
                    {
                        continue;
                    }
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::SUBROLE_VALID)
                        && !string_equals(Some(window_subrole), Some("AXStandardWindow"))
                    {
                        continue;
                    }
                }

                matched = true;
                rule_combine_effects(&rules[index].effects, &mut effects);

                if RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
                    let mut rule_flags = RuleFlag(rules[index].flags);
                    rule_flags.insert(RuleFlag::ONE_SHOT_REMOVE);
                    rules[index].flags = rule_flags.0;
                }
            }
        }
    }

    window_manager.rules = rules;

    if matched {
        window_manager_apply_manage_rule_effects_to_window(
            space_manager,
            window_manager,
            window_id,
            &effects,
            display_manager,
            mouse_drag_state,
        );
    }
}

pub(crate) fn window_manager_apply_rules_to_window(
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
        if one_shot_rules || !RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
            if window_manager_rule_matches_window(
                &rules[index],
                window_id,
                window_title,
                window_role,
                window_subrole,
                window_manager,
            ) {
                let window_is_managed_by_rule = window_manager
                    .window
                    .find(&window_id)
                    .is_some_and(|window| window_check_rule_flag(window, WindowRuleFlag::MANAGED));
                if !window_is_managed_by_rule {
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::ROLE_VALID)
                        && !string_equals(Some(window_role), Some("AXWindow"))
                    {
                        continue;
                    }
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::SUBROLE_VALID)
                        && !string_equals(Some(window_subrole), Some("AXStandardWindow"))
                    {
                        continue;
                    }
                }

                matched = true;
                rule_combine_effects(&rules[index].effects, &mut effects);

                if RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
                    let mut rule_flags = RuleFlag(rules[index].flags);
                    rule_flags.insert(RuleFlag::ONE_SHOT_REMOVE);
                    rules[index].flags = rule_flags.0;
                }
            }
        }
    }

    window_manager.rules = rules;

    if matched {
        window_manager_apply_rule_effects_to_window(
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

pub(crate) fn rule_reapply_all(
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    for window_id in window_manager.window.keys_in_bucket_order() {
        let Some(window) = window_manager.window.find(&window_id) else {
            continue;
        };

        if window.is_root {
            let window_title = window_title_ts(window);
            let window_role = window_role_ts(window);
            let window_subrole = window_subrole_ts(window);

            window_manager_apply_manage_rules_to_window(
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

            if window_manager_is_window_eligible(window_id, window_manager) {
                if let Some(window) = window_manager.window.find_mut(&window_id) {
                    window.is_eligible = true;
                }
                window_manager_apply_rules_to_window(
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

pub(crate) fn rule_reapply_by_index(
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
            if !RuleFlag(window_manager.rules[rule_index].flags).contains(RuleFlag::ONE_SHOT) {
                let rule = std::mem::take(&mut window_manager.rules[rule_index]);
                rule_apply(
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

pub(crate) fn rule_reapply_by_label(
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
        if string_equals(
            window_manager.rules[rule_index].label.as_deref(),
            Some(&*label),
        ) {
            if !RuleFlag(window_manager.rules[rule_index].flags).contains(RuleFlag::ONE_SHOT) {
                let rule = std::mem::take(&mut window_manager.rules[rule_index]);
                rule_apply(
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

pub(crate) fn rule_apply(
    rule: &Rule,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    for window_id in window_manager.window.keys_in_bucket_order() {
        let Some(window) = window_manager.window.find(&window_id) else {
            continue;
        };

        if window.is_root {
            let window_title = window_title_ts(window);
            let window_role = window_role_ts(window);
            let window_subrole = window_subrole_ts(window);

            if window_manager_rule_matches_window(
                rule,
                window_id,
                &window_title,
                &window_role,
                &window_subrole,
                window_manager,
            ) {
                window_manager_apply_manage_rule_effects_to_window(
                    space_manager,
                    window_manager,
                    window_id,
                    &rule.effects,
                    display_manager,
                    mouse_drag_state,
                );

                if window_manager_is_window_eligible(window_id, window_manager) {
                    if let Some(window) = window_manager.window.find_mut(&window_id) {
                        window.is_eligible = true;
                    }
                    window_manager_apply_rule_effects_to_window(
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
