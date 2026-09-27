use crate::display_manager::{DisplayManager, display_manager_display_id_arrangement};
use crate::handles::{DisplayId, SpaceId};
use crate::misc::helpers::{
    LAYER_STR, json_bool, json_optional_bool, string_copy, string_equals, ts_string_escape,
};
use crate::misc::macros::in_range_ii;
use crate::misc::regex::PosixRegex;
use crate::misc::response::Response;
use crate::mission_control::MissionControlMode;
use crate::process_manager::ProcessManager;
use crate::space_manager::{SpaceManager, space_manager_mission_control_index};
use crate::state::MouseDragState;
use crate::window::{window_role_ts, window_subrole_ts, window_title_ts};
use crate::window_manager::{
    WindowManager, window_manager_apply_manage_rule_effects_to_window,
    window_manager_apply_manage_rules_to_window, window_manager_apply_rule_effects_to_window,
    window_manager_apply_rules_to_window, window_manager_is_window_eligible,
    window_manager_rule_matches_window,
};

pub(crate) const RULE_PROP_UD: i32 = 0;
pub(crate) const RULE_PROP_ON: i32 = 1;
pub(crate) const RULE_PROP_OFF: i32 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuleFlag(pub u16);

impl RuleFlag {
    pub(crate) const APP_VALID: RuleFlag = RuleFlag(0x001);
    pub(crate) const TITLE_VALID: RuleFlag = RuleFlag(0x002);
    pub(crate) const ROLE_VALID: RuleFlag = RuleFlag(0x004);
    pub(crate) const SUBROLE_VALID: RuleFlag = RuleFlag(0x008);
    pub(crate) const APP_EXCLUDE: RuleFlag = RuleFlag(0x010);
    pub(crate) const TITLE_EXCLUDE: RuleFlag = RuleFlag(0x020);
    pub(crate) const ROLE_EXCLUDE: RuleFlag = RuleFlag(0x040);
    pub(crate) const SUBROLE_EXCLUDE: RuleFlag = RuleFlag(0x080);
    pub(crate) const ONE_SHOT: RuleFlag = RuleFlag(0x100);
    pub(crate) const ONE_SHOT_REMOVE: RuleFlag = RuleFlag(0x200);

    pub(crate) fn contains(self, flag: RuleFlag) -> bool {
        (self.0 & flag.0) != 0
    }

    pub(crate) fn insert(&mut self, flag: RuleFlag) {
        self.0 |= flag.0;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuleEffectsFlag(pub u16);

impl RuleEffectsFlag {
    pub(crate) const FOLLOW_SPACE: RuleEffectsFlag = RuleEffectsFlag(0x01);
    pub(crate) const OPACITY: RuleEffectsFlag = RuleEffectsFlag(0x02);
    pub(crate) const LAYER: RuleEffectsFlag = RuleEffectsFlag(0x04);

    pub(crate) fn contains(self, flag: RuleEffectsFlag) -> bool {
        (self.0 & flag.0) != 0
    }

    pub(crate) fn remove(&mut self, flag: RuleEffectsFlag) {
        self.0 &= !flag.0;
    }

    pub(crate) fn insert(&mut self, flag: RuleEffectsFlag) {
        self.0 |= flag.0;
    }
}

#[derive(Default)]
pub(crate) struct RuleEffects {
    pub(crate) display_id: DisplayId,
    pub(crate) space_id: SpaceId,
    pub(crate) opacity: f32,
    pub(crate) manage: i32,
    pub(crate) sticky: i32,
    pub(crate) mff: i32,
    pub(crate) layer: i32,
    pub(crate) fullscreen: i32,
    pub(crate) grid: [u32; 6],
    pub(crate) scratchpad: Option<String>,
    pub(crate) flags: u16,
}

#[derive(Default)]
pub(crate) struct Rule {
    pub(crate) label: Option<String>,
    pub(crate) app: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) role: Option<String>,
    pub(crate) subrole: Option<String>,
    pub(crate) app_regex: Option<PosixRegex>,
    pub(crate) title_regex: Option<PosixRegex>,
    pub(crate) role_regex: Option<PosixRegex>,
    pub(crate) subrole_regex: Option<PosixRegex>,
    pub(crate) effects: RuleEffects,
    pub(crate) flags: u16,
}

pub(crate) fn rule_serialize(
    response: &mut Response,
    rule: &Rule,
    index: i32,
    display_manager: &mut DisplayManager,
) {
    let app = rule.app.as_deref();
    let title = rule.title.as_deref();
    let role = rule.role.as_deref();
    let subrole = rule.subrole.as_deref();

    let escaped_app = app.and_then(ts_string_escape);
    let escaped_title = title.and_then(ts_string_escape);
    let escaped_role = role.and_then(ts_string_escape);
    let escaped_subrole = subrole.and_then(ts_string_escape);

    let mut flags = RuleFlag(rule.flags);
    if rule.app_regex.is_some() {
        flags.insert(RuleFlag::APP_VALID);
    }
    if rule.title_regex.is_some() {
        flags.insert(RuleFlag::TITLE_VALID);
    }
    if rule.role_regex.is_some() {
        flags.insert(RuleFlag::ROLE_VALID);
    }
    if rule.subrole_regex.is_some() {
        flags.insert(RuleFlag::SUBROLE_VALID);
    }

    let effects_flags = RuleEffectsFlag(rule.effects.flags);

    response.write(format_args!(
        "{{\n\
         \t\"index\":{},\n\
         \t\"label\":\"{}\",\n\
         \t\"app\":\"{}\",\n\
         \t\"title\":\"{}\",\n\
         \t\"role\":\"{}\",\n\
         \t\"subrole\":\"{}\",\n\
         \t\"display\":{},\n\
         \t\"space\":{},\n\
         \t\"follow_space\":{},\n\
         \t\"opacity\":{:.4},\n\
         \t\"manage\":{},\n\
         \t\"sticky\":{},\n\
         \t\"mouse_follows_focus\":{},\n\
         \t\"sub-layer\":\"{}\",\n\
         \t\"native-fullscreen\":{},\n\
         \t\"grid\":\"{}:{}:{}:{}:{}:{}\",\n\
         \t\"scratchpad\":\"{}\",\n\
         \t\"one-shot\":{},\n\
         \t\"flags\":\"0x{:08x}\"\n\
         }}",
        index,
        rule.label.as_deref().unwrap_or(""),
        escaped_app.as_deref().or(app).unwrap_or(""),
        escaped_title.as_deref().or(title).unwrap_or(""),
        escaped_role.as_deref().or(role).unwrap_or(""),
        escaped_subrole.as_deref().or(subrole).unwrap_or(""),
        if rule.effects.display_id.0 != 0 {
            display_manager_display_id_arrangement(rule.effects.display_id, display_manager)
        } else {
            0
        },
        if rule.effects.space_id.0 != 0 {
            space_manager_mission_control_index(rule.effects.space_id)
        } else {
            0
        },
        json_bool(effects_flags.contains(RuleEffectsFlag::FOLLOW_SPACE)),
        rule.effects.opacity as f64,
        json_optional_bool(rule.effects.manage),
        json_optional_bool(rule.effects.sticky),
        json_optional_bool(rule.effects.mff),
        if effects_flags.contains(RuleEffectsFlag::LAYER) {
            LAYER_STR
                .get(rule.effects.layer as usize)
                .copied()
                .flatten()
                .unwrap_or("(null)")
        } else {
            ""
        },
        json_optional_bool(rule.effects.fullscreen),
        rule.effects.grid[0] as i32,
        rule.effects.grid[1] as i32,
        rule.effects.grid[2] as i32,
        rule.effects.grid[3] as i32,
        rule.effects.grid[4] as i32,
        rule.effects.grid[5] as i32,
        rule.effects.scratchpad.as_deref().unwrap_or(""),
        json_bool(flags.contains(RuleFlag::ONE_SHOT)),
        ((rule.effects.flags as u32) << 16) | (flags.0 as u32),
    ));
}

pub(crate) fn rule_combine_effects(effects: &RuleEffects, result: &mut RuleEffects) {
    let effects_flags = RuleEffectsFlag(effects.flags);
    let mut result_flags = RuleEffectsFlag(result.flags);

    if effects.display_id.0 != 0 {
        result.display_id = effects.display_id;
        if effects_flags.contains(RuleEffectsFlag::FOLLOW_SPACE) {
            result_flags.insert(RuleEffectsFlag::FOLLOW_SPACE);
        } else {
            result_flags.remove(RuleEffectsFlag::FOLLOW_SPACE);
        }
    }

    if effects.space_id.0 != 0 {
        result.space_id = effects.space_id;
        if effects_flags.contains(RuleEffectsFlag::FOLLOW_SPACE) {
            result_flags.insert(RuleEffectsFlag::FOLLOW_SPACE);
        } else {
            result_flags.remove(RuleEffectsFlag::FOLLOW_SPACE);
        }
    }

    if effects_flags.contains(RuleEffectsFlag::OPACITY)
        && in_range_ii(effects.opacity, 0.0f32, 1.0f32)
    {
        result.opacity = effects.opacity;
        result_flags.insert(RuleEffectsFlag::OPACITY);
    }

    if effects_flags.contains(RuleEffectsFlag::LAYER) {
        result.layer = effects.layer;
        result_flags.insert(RuleEffectsFlag::LAYER);
    }

    result.flags = result_flags.0;

    if let Some(scratchpad) = effects.scratchpad.as_deref() {
        result.scratchpad = Some(string_copy(scratchpad));
    }

    if effects.manage != RULE_PROP_UD {
        result.manage = effects.manage;
    }
    if effects.sticky != RULE_PROP_UD {
        result.sticky = effects.sticky;
    }
    if effects.mff != RULE_PROP_UD {
        result.mff = effects.mff;
    }
    if effects.fullscreen != RULE_PROP_UD {
        result.fullscreen = effects.fullscreen;
    }

    if effects.grid[0] != 0 && effects.grid[1] != 0 {
        result.grid[0] = effects.grid[0];
        result.grid[1] = effects.grid[1];
        result.grid[2] = effects.grid[2];
        result.grid[3] = effects.grid[3];
        result.grid[4] = effects.grid[4];
        result.grid[5] = effects.grid[5];
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

pub(crate) fn rule_add(rule: Rule, window_manager: &mut WindowManager) {
    if let Some(label) = rule.label.as_deref() {
        rule_remove_by_label(label.as_bytes(), window_manager);
    }
    window_manager.rules.push(rule);
}

pub(crate) fn rule_remove_by_index(index: i32, window_manager: &mut WindowManager) -> bool {
    for rule_index in 0..window_manager.rules.len() {
        if rule_index as i32 == index {
            window_manager.rules.swap_remove(rule_index);
            return true;
        }
    }

    false
}

pub(crate) fn rule_remove_by_label(label: &[u8], window_manager: &mut WindowManager) -> bool {
    let label = String::from_utf8_lossy(label);

    for rule_index in 0..window_manager.rules.len() {
        if string_equals(
            window_manager.rules[rule_index].label.as_deref(),
            Some(&*label),
        ) {
            window_manager.rules.swap_remove(rule_index);
            return true;
        }
    }

    false
}

impl Drop for Rule {
    fn drop(&mut self) {
        drop(self.app_regex.take());
        drop(self.title_regex.take());
        drop(self.role_regex.take());
        drop(self.subrole_regex.take());

        drop(self.label.take());
        drop(self.app.take());
        drop(self.title.take());
        drop(self.role.take());
        drop(self.subrole.take());

        drop(self.effects.scratchpad.take());
    }
}
