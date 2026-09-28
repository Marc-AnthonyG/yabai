use regex::Regex;

use crate::support::arithmetic::is_within_range_including_both_bounds;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::strings::{are_both_strings_present_and_equal, copy_into_owned_string};
use crate::window::manager::WindowManager;

pub(crate) const RULE_PROPERTY_UNSET: i32 = 0;
pub(crate) const RULE_PROPERTY_ON: i32 = 1;
pub(crate) const RULE_PROPERTY_OFF: i32 = 2;

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) struct RuleFlag: u16 {
        const APPLICATION_PATTERN_IS_VALID = 0x001;
        const TITLE_PATTERN_IS_VALID = 0x002;
        const ROLE_PATTERN_IS_VALID = 0x004;
        const SUBROLE_PATTERN_IS_VALID = 0x008;
        const APPLICATION_PATTERN_IS_NEGATED = 0x010;
        const TITLE_PATTERN_IS_NEGATED = 0x020;
        const ROLE_PATTERN_IS_NEGATED = 0x040;
        const SUBROLE_PATTERN_IS_NEGATED = 0x080;
        const ONE_SHOT = 0x100;
        const ONE_SHOT_DUE_FOR_REMOVAL = 0x200;
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) struct RuleEffectsFlag: u16 {
        const FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE = 0x01;
        const OPACITY_IS_SET = 0x02;
        const LAYER_IS_SET = 0x04;
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
    pub(crate) flags: RuleEffectsFlag,
}

#[derive(Default)]
pub(crate) struct Rule {
    pub(crate) label: Option<String>,
    pub(crate) app: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) role: Option<String>,
    pub(crate) subrole: Option<String>,
    pub(crate) app_regex: Option<Regex>,
    pub(crate) title_regex: Option<Regex>,
    pub(crate) role_regex: Option<Regex>,
    pub(crate) subrole_regex: Option<Regex>,
    pub(crate) effects: RuleEffects,
    pub(crate) flags: RuleFlag,
}

pub(crate) fn combine_rule_effects_into_accumulated_effects(
    effects: &RuleEffects,
    result: &mut RuleEffects,
) {
    let focus_follows_window_to_its_space = effects
        .flags
        .contains(RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE);

    if effects.display_id.0 != 0 {
        result.display_id = effects.display_id;
        result.flags.set(
            RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE,
            focus_follows_window_to_its_space,
        );
    }

    if effects.space_id.0 != 0 {
        result.space_id = effects.space_id;
        result.flags.set(
            RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE,
            focus_follows_window_to_its_space,
        );
    }

    if effects.flags.contains(RuleEffectsFlag::OPACITY_IS_SET)
        && is_within_range_including_both_bounds(effects.opacity, 0.0f32, 1.0f32)
    {
        result.opacity = effects.opacity;
        result.flags.insert(RuleEffectsFlag::OPACITY_IS_SET);
    }

    if effects.flags.contains(RuleEffectsFlag::LAYER_IS_SET) {
        result.layer = effects.layer;
        result.flags.insert(RuleEffectsFlag::LAYER_IS_SET);
    }

    if let Some(scratchpad) = effects.scratchpad.as_deref() {
        result.scratchpad = Some(copy_into_owned_string(scratchpad));
    }

    if effects.manage != RULE_PROPERTY_UNSET {
        result.manage = effects.manage;
    }
    if effects.sticky != RULE_PROPERTY_UNSET {
        result.sticky = effects.sticky;
    }
    if effects.mff != RULE_PROPERTY_UNSET {
        result.mff = effects.mff;
    }
    if effects.fullscreen != RULE_PROPERTY_UNSET {
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

pub(crate) fn add_rule_replacing_any_with_the_same_label(
    rule: Rule,
    window_manager: &mut WindowManager,
) {
    if let Some(label) = rule.label.as_deref() {
        remove_rule_with_label(label.as_bytes(), window_manager);
    }
    window_manager.rules.push(rule);
}

pub(crate) fn remove_rule_at_index(index: i32, window_manager: &mut WindowManager) -> bool {
    for rule_index in 0..window_manager.rules.len() {
        if rule_index as i32 == index {
            window_manager.rules.swap_remove(rule_index);
            return true;
        }
    }

    false
}

pub(crate) fn remove_rule_with_label(label: &[u8], window_manager: &mut WindowManager) -> bool {
    let label = String::from_utf8_lossy(label);

    for rule_index in 0..window_manager.rules.len() {
        if are_both_strings_present_and_equal(
            window_manager.rules[rule_index].label.as_deref(),
            Some(&*label),
        ) {
            window_manager.rules.swap_remove(rule_index);
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{RuleEffects, RuleEffectsFlag, combine_rule_effects_into_accumulated_effects};
    use crate::support::handles::SpaceId;

    fn effects_sending_to_space(space_id: u64, focus_follows: bool) -> RuleEffects {
        let mut effects = RuleEffects {
            space_id: SpaceId(space_id),
            ..RuleEffects::default()
        };
        effects.flags.set(
            RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE,
            focus_follows,
        );
        effects
    }

    #[test]
    fn a_later_space_without_focus_following_clears_the_earlier_rules_focus_following() {
        let mut accumulated = RuleEffects::default();

        combine_rule_effects_into_accumulated_effects(
            &effects_sending_to_space(3, true),
            &mut accumulated,
        );
        combine_rule_effects_into_accumulated_effects(
            &effects_sending_to_space(5, false),
            &mut accumulated,
        );

        assert_eq!(accumulated.space_id.0, 5);
        assert!(
            !accumulated
                .flags
                .contains(RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE)
        );
    }

    #[test]
    fn a_rule_without_a_display_or_space_keeps_the_accumulated_focus_following() {
        let mut accumulated = RuleEffects::default();
        combine_rule_effects_into_accumulated_effects(
            &effects_sending_to_space(3, true),
            &mut accumulated,
        );

        combine_rule_effects_into_accumulated_effects(&RuleEffects::default(), &mut accumulated);

        assert!(
            accumulated
                .flags
                .contains(RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE)
        );
    }

    #[test]
    fn an_opacity_outside_zero_to_one_is_not_taken() {
        let mut accumulated = RuleEffects::default();
        let mut effects = RuleEffects {
            opacity: 1.5,
            ..RuleEffects::default()
        };
        effects.flags.insert(RuleEffectsFlag::OPACITY_IS_SET);

        combine_rule_effects_into_accumulated_effects(&effects, &mut accumulated);

        assert!(!accumulated.flags.contains(RuleEffectsFlag::OPACITY_IS_SET));
        assert_eq!(accumulated.opacity, 0.0);
    }
}
