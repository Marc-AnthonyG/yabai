use serde::Serialize;

use crate::command::values::{GridPlacement, OnOrOff};
use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::space::lookup::query_mission_control_index_of_space;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::layer::WindowStackingSubLayer;
use crate::window::manager::WindowManager;
use crate::window::rule::{
    RULE_PROPERTY_OFF, RULE_PROPERTY_ON, Rule, RuleEffects, RuleEffectsFlag, RuleFlag,
};

#[derive(Serialize, Debug, Default)]
pub(crate) struct RuleSnapshot {
    pub(crate) index: usize,
    pub(crate) label: Option<String>,
    pub(crate) app: Option<String>,
    pub(crate) app_not: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) title_not: Option<String>,
    pub(crate) role: Option<String>,
    pub(crate) role_not: Option<String>,
    pub(crate) subrole: Option<String>,
    pub(crate) subrole_not: Option<String>,
    pub(crate) display: Option<i32>,
    pub(crate) space: Option<i32>,
    pub(crate) follow: bool,
    pub(crate) manage: Option<OnOrOff>,
    pub(crate) sticky: Option<OnOrOff>,
    pub(crate) mouse_follows_focus: Option<OnOrOff>,
    pub(crate) sub_layer: Option<WindowStackingSubLayer>,
    pub(crate) opacity: Option<f32>,
    pub(crate) native_fullscreen: Option<OnOrOff>,
    pub(crate) grid: Option<GridPlacement>,
    pub(crate) scratchpad: Option<String>,
    pub(crate) one_shot: bool,
}

pub(crate) fn every_rule_as_pretty_json(
    window_manager: &WindowManager,
    display_manager: &mut DisplayManager,
) -> String {
    let snapshots: Vec<RuleSnapshot> = window_manager
        .rules
        .iter()
        .enumerate()
        .map(|(index, rule)| snapshot_of_rule(index, rule, display_manager))
        .collect();
    serde_json::to_string_pretty(&snapshots).unwrap_or_default()
}

fn snapshot_of_rule(
    index: usize,
    rule: &Rule,
    display_manager: &mut DisplayManager,
) -> RuleSnapshot {
    let (app, app_not) = pattern_or_its_negation(
        &rule.app,
        rule.flags,
        RuleFlag::APPLICATION_PATTERN_IS_NEGATED,
    );
    let (title, title_not) =
        pattern_or_its_negation(&rule.title, rule.flags, RuleFlag::TITLE_PATTERN_IS_NEGATED);
    let (role, role_not) =
        pattern_or_its_negation(&rule.role, rule.flags, RuleFlag::ROLE_PATTERN_IS_NEGATED);
    let (subrole, subrole_not) = pattern_or_its_negation(
        &rule.subrole,
        rule.flags,
        RuleFlag::SUBROLE_PATTERN_IS_NEGATED,
    );
    let effects = &rule.effects;

    RuleSnapshot {
        index,
        label: rule.label.clone(),
        app,
        app_not,
        title,
        title_not,
        role,
        role_not,
        subrole,
        subrole_not,
        display: (effects.display_id != DisplayId(0))
            .then(|| query_arrangement_index_of_display(effects.display_id, display_manager)),
        space: (effects.space_id != SpaceId(0))
            .then(|| query_mission_control_index_of_space(effects.space_id)),
        follow: effects
            .flags
            .contains(RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE),
        manage: switch_of_rule_property(effects.manage),
        sticky: switch_of_rule_property(effects.sticky),
        mouse_follows_focus: switch_of_rule_property(effects.mff),
        sub_layer: sub_layer_of_effects(effects),
        opacity: effects
            .flags
            .contains(RuleEffectsFlag::OPACITY_IS_SET)
            .then_some(effects.opacity),
        native_fullscreen: switch_of_rule_property(effects.fullscreen),
        grid: grid_placement_of_effects(effects),
        scratchpad: effects.scratchpad.clone(),
        one_shot: rule.flags.contains(RuleFlag::ONE_SHOT),
    }
}

fn pattern_or_its_negation(
    pattern: &Option<String>,
    flags: RuleFlag,
    negation_flag: RuleFlag,
) -> (Option<String>, Option<String>) {
    if flags.contains(negation_flag) {
        (None, pattern.clone())
    } else {
        (pattern.clone(), None)
    }
}

fn switch_of_rule_property(rule_property: i32) -> Option<OnOrOff> {
    match rule_property {
        RULE_PROPERTY_ON => Some(OnOrOff::On),
        RULE_PROPERTY_OFF => Some(OnOrOff::Off),
        _ => None,
    }
}

fn sub_layer_of_effects(effects: &RuleEffects) -> Option<WindowStackingSubLayer> {
    if !effects.flags.contains(RuleEffectsFlag::LAYER_IS_SET) {
        return None;
    }
    WindowStackingSubLayer::of_layer(effects.layer)
}

fn grid_placement_of_effects(effects: &RuleEffects) -> Option<GridPlacement> {
    let [rows, columns, column, row, width, height] = effects.grid;
    (rows != 0 && columns != 0).then_some(GridPlacement {
        rows,
        columns,
        column,
        row,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::{RuleSnapshot, snapshot_of_rule};
    use crate::command::CommandLine;
    use crate::display::manager::DisplayManager;
    use crate::window::rule::{RULE_PROPERTY_OFF, Rule, RuleFlag};

    #[test]
    fn a_rule_prints_the_flags_of_rule_add_as_snake_case_keys() {
        let command = CommandLine::command();
        let rule_add = command
            .find_subcommand("rule")
            .and_then(|rule| rule.find_subcommand("add"))
            .unwrap();
        let flags_as_keys: Vec<String> = rule_add
            .get_arguments()
            .filter_map(|argument| argument.get_long())
            .filter(|flag| !["help", "version"].contains(flag))
            .map(|flag| flag.replace('-', "_"))
            .collect();

        let keys: Vec<String> = serde_json::to_value(RuleSnapshot::default())
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();

        for flag in &flags_as_keys {
            assert!(keys.contains(flag), "{flag}");
        }
        assert_eq!(keys.len(), flags_as_keys.len() + 1);
    }

    #[test]
    fn a_negated_pattern_prints_under_its_not_key_and_unset_effects_print_null() {
        let mut rule = Rule {
            app: Some(String::from("^Finder$")),
            ..Rule::default()
        };
        rule.flags.insert(RuleFlag::APPLICATION_PATTERN_IS_NEGATED);
        rule.effects.manage = RULE_PROPERTY_OFF;

        let snapshot =
            serde_json::to_value(snapshot_of_rule(2, &rule, &mut DisplayManager::default()))
                .unwrap();

        assert_eq!(snapshot["index"], 2);
        assert_eq!(snapshot["app"], serde_json::Value::Null);
        assert_eq!(snapshot["app_not"], "^Finder$");
        assert_eq!(snapshot["manage"], "off");
        assert_eq!(snapshot["sticky"], serde_json::Value::Null);
        assert_eq!(snapshot["display"], serde_json::Value::Null);
        assert_eq!(snapshot["grid"], serde_json::Value::Null);
        assert_eq!(snapshot["one_shot"], false);
    }
}
