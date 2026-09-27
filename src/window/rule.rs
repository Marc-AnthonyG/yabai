use crate::support::arithmetic::in_range_ii;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::regex::PosixRegex;
use crate::support::strings::{string_copy, string_equals};
use crate::window::manager::WindowManager;

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
