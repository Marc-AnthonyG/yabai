use regex::Regex;

use crate::command::rule::RuleDefinition;
use crate::command::values::OnOrOff;
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::manager::DisplayManager;
use crate::message::selector_resolution::{resolve_display_selector, resolve_space_selector};
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::SpaceManager;
use crate::window::manager::WindowManager;
use crate::window::rule::{RULE_PROPERTY_OFF, RULE_PROPERTY_ON, Rule, RuleEffectsFlag, RuleFlag};

pub(crate) fn does_the_definition_set_anything(definition: &RuleDefinition) -> bool {
    has_a_matcher(definition)
        || definition.label.is_some()
        || definition.display.is_some()
        || definition.space.is_some()
        || definition.follow
        || definition.manage.is_some()
        || definition.sticky.is_some()
        || definition.mouse_follows_focus.is_some()
        || definition.sub_layer.is_some()
        || definition.opacity.is_some()
        || definition.native_fullscreen.is_some()
        || definition.grid.is_some()
        || definition.scratchpad.is_some()
}

pub(crate) fn has_a_matcher(definition: &RuleDefinition) -> bool {
    [
        &definition.app,
        &definition.app_not,
        &definition.title,
        &definition.title_not,
        &definition.role,
        &definition.role_not,
        &definition.subrole,
        &definition.subrole_not,
    ]
    .iter()
    .any(|matcher| matcher.is_some())
}

pub(crate) fn rule_of_definition(
    definition: &RuleDefinition,
    is_one_shot: bool,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<Rule, String> {
    if !has_a_matcher(definition) {
        return Err(String::from(
            "a rule needs at least one of --app, --title, --role and --subrole, or their --…-not form",
        ));
    }

    let mut rule = Rule {
        label: definition.label.clone(),
        ..Rule::default()
    };
    rule.flags.set(RuleFlag::ONE_SHOT, is_one_shot);
    match_windows_by_their_patterns(&mut rule, definition)?;
    send_the_window_to_its_display_or_space(
        &mut rule,
        definition,
        display_manager,
        window_manager,
        space_manager,
    )?;
    give_the_window_its_other_effects(&mut rule, definition);
    Ok(rule)
}

fn match_windows_by_their_patterns(
    rule: &mut Rule,
    definition: &RuleDefinition,
) -> Result<(), String> {
    if let Some((pattern, is_negated)) =
        pattern_and_whether_it_is_negated(&definition.app, &definition.app_not)
    {
        rule.app_regex = Some(compile_pattern(&pattern, "--app")?);
        rule.app = Some(pattern);
        rule.flags
            .set(RuleFlag::APPLICATION_PATTERN_IS_NEGATED, is_negated);
    }
    if let Some((pattern, is_negated)) =
        pattern_and_whether_it_is_negated(&definition.title, &definition.title_not)
    {
        rule.title_regex = Some(compile_pattern(&pattern, "--title")?);
        rule.title = Some(pattern);
        rule.flags
            .set(RuleFlag::TITLE_PATTERN_IS_NEGATED, is_negated);
    }
    if let Some((pattern, is_negated)) =
        pattern_and_whether_it_is_negated(&definition.role, &definition.role_not)
    {
        rule.role_regex = Some(compile_pattern(&pattern, "--role")?);
        rule.role = Some(pattern);
        rule.flags
            .set(RuleFlag::ROLE_PATTERN_IS_NEGATED, is_negated);
    }
    if let Some((pattern, is_negated)) =
        pattern_and_whether_it_is_negated(&definition.subrole, &definition.subrole_not)
    {
        rule.subrole_regex = Some(compile_pattern(&pattern, "--subrole")?);
        rule.subrole = Some(pattern);
        rule.flags
            .set(RuleFlag::SUBROLE_PATTERN_IS_NEGATED, is_negated);
    }
    Ok(())
}

fn pattern_and_whether_it_is_negated(
    pattern: &Option<String>,
    negated_pattern: &Option<String>,
) -> Option<(String, bool)> {
    match (pattern, negated_pattern) {
        (Some(pattern), _) => Some((pattern.clone(), false)),
        (None, Some(negated_pattern)) => Some((negated_pattern.clone(), true)),
        (None, None) => None,
    }
}

fn compile_pattern(pattern: &str, flag: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|_| format!("invalid regex pattern '{pattern}' for {flag}"))
}

fn send_the_window_to_its_display_or_space(
    rule: &mut Rule,
    definition: &RuleDefinition,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<(), String> {
    if let Some(display_selector) = &definition.display {
        rule.effects.display_id = resolve_display_selector(
            display_selector,
            query_display_showing_the_active_menu_bar(),
            display_manager,
        )?;
    }
    if let Some(space_selector) = &definition.space {
        rule.effects.space_id = resolve_space_selector(
            space_selector,
            query_current_space_of_the_focused_display(window_manager),
            space_manager,
        )?;
    }
    rule.effects.flags.set(
        RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE,
        definition.follow,
    );
    Ok(())
}

fn give_the_window_its_other_effects(rule: &mut Rule, definition: &RuleDefinition) {
    let effects = &mut rule.effects;
    if let Some(manage) = definition.manage {
        effects.manage = rule_property_of(manage);
    }
    if let Some(sticky) = definition.sticky {
        effects.sticky = rule_property_of(sticky);
    }
    if let Some(mouse_follows_focus) = definition.mouse_follows_focus {
        effects.mff = rule_property_of(mouse_follows_focus);
    }
    if let Some(native_fullscreen) = definition.native_fullscreen {
        effects.fullscreen = rule_property_of(native_fullscreen);
    }
    if let Some(sub_layer) = definition.sub_layer {
        effects.layer = sub_layer.layer();
        effects.flags.insert(RuleEffectsFlag::LAYER_IS_SET);
    }
    if let Some(opacity) = definition.opacity {
        effects.opacity = opacity;
        effects.flags.insert(RuleEffectsFlag::OPACITY_IS_SET);
    }
    if let Some(grid) = definition.grid {
        effects.grid = [
            grid.rows,
            grid.columns,
            grid.column,
            grid.row,
            grid.width,
            grid.height,
        ];
    }
    if let Some(scratchpad) = &definition.scratchpad {
        effects.scratchpad = Some(scratchpad.clone());
        effects.manage = RULE_PROPERTY_OFF;
    }
}

fn rule_property_of(switch: OnOrOff) -> i32 {
    if switch.is_on() {
        RULE_PROPERTY_ON
    } else {
        RULE_PROPERTY_OFF
    }
}

#[cfg(test)]
mod tests {
    use super::{does_the_definition_set_anything, has_a_matcher, rule_of_definition};
    use crate::command::rule::RuleDefinition;
    use crate::command::values::{GridPlacement, OnOrOff};
    use crate::display::manager::DisplayManager;
    use crate::space::manager::create_space_manager_without_any_view_with_its_initial_settings;
    use crate::support::layer::{LAYER_ABOVE, WindowStackingSubLayer};
    use crate::window::manager::create_window_manager_tracking_nothing_with_its_initial_settings;
    use crate::window::rule::{
        RULE_PROPERTY_OFF, RULE_PROPERTY_ON, Rule, RuleEffectsFlag, RuleFlag,
    };

    fn rule_of(definition: &RuleDefinition, is_one_shot: bool) -> Result<Rule, String> {
        rule_of_definition(
            definition,
            is_one_shot,
            &mut DisplayManager::default(),
            &mut create_window_manager_tracking_nothing_with_its_initial_settings(),
            &mut create_space_manager_without_any_view_with_its_initial_settings(),
        )
    }

    #[test]
    fn a_rule_without_a_matcher_is_refused_whatever_else_it_sets() {
        let definition = RuleDefinition {
            label: Some(String::from("floating")),
            manage: Some(OnOrOff::Off),
            ..RuleDefinition::default()
        };

        assert!(!has_a_matcher(&definition));
        assert!(does_the_definition_set_anything(&definition));
        assert!(rule_of(&definition, false).is_err());
        assert!(!does_the_definition_set_anything(&RuleDefinition::default()));
    }

    #[test]
    fn a_negated_pattern_is_stored_as_the_pattern_with_its_negation_flag() {
        let rule = rule_of(
            &RuleDefinition {
                app: Some(String::from("^Finder$")),
                title_not: Some(String::from("Copy")),
                ..RuleDefinition::default()
            },
            true,
        )
        .unwrap();

        assert_eq!(rule.app.as_deref(), Some("^Finder$"));
        assert!(
            !rule
                .flags
                .contains(RuleFlag::APPLICATION_PATTERN_IS_NEGATED)
        );
        assert_eq!(rule.title.as_deref(), Some("Copy"));
        assert!(rule.flags.contains(RuleFlag::TITLE_PATTERN_IS_NEGATED));
        assert!(rule.title_regex.is_some());
        assert!(rule.flags.contains(RuleFlag::ONE_SHOT));
    }

    #[test]
    fn the_effects_of_a_rule_come_from_its_flags() {
        let rule = rule_of(
            &RuleDefinition {
                app: Some(String::from("Raycast")),
                sticky: Some(OnOrOff::On),
                sub_layer: Some(WindowStackingSubLayer::Above),
                opacity: Some(0.9),
                grid: Some("4:4:1:1:2:2".parse::<GridPlacement>().unwrap()),
                ..RuleDefinition::default()
            },
            false,
        )
        .unwrap();

        assert_eq!(rule.effects.sticky, RULE_PROPERTY_ON);
        assert_eq!(rule.effects.layer, LAYER_ABOVE);
        assert!(rule.effects.flags.contains(RuleEffectsFlag::LAYER_IS_SET));
        assert_eq!(rule.effects.opacity, 0.9);
        assert!(rule.effects.flags.contains(RuleEffectsFlag::OPACITY_IS_SET));
        assert_eq!(rule.effects.grid, [4, 4, 1, 1, 2, 2]);
    }

    #[test]
    fn a_scratchpad_rule_leaves_its_window_unmanaged() {
        let rule = rule_of(
            &RuleDefinition {
                app: Some(String::from("Notes")),
                manage: Some(OnOrOff::On),
                scratchpad: Some(String::from("notes")),
                ..RuleDefinition::default()
            },
            false,
        )
        .unwrap();

        assert_eq!(rule.effects.scratchpad.as_deref(), Some("notes"));
        assert_eq!(rule.effects.manage, RULE_PROPERTY_OFF);
    }
}
