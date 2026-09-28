use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::space::lookup::query_mission_control_index_of_space;
use crate::support::json::{
    escape_string_for_json_when_it_needs_escaping, json_literal_for_boolean,
    json_literal_for_optional_boolean,
};
use crate::support::layer::LAYER_NAMES;
use crate::support::printf_float_format::format_float_with_decimals_as_printf_does;
use crate::support::response::Response;
use crate::window::manager::WindowManager;
use crate::window::rule::{Rule, RuleEffectsFlag, RuleFlag};

pub(crate) fn write_rule_as_json_object(
    response: &mut Response,
    rule: &Rule,
    index: i32,
    display_manager: &mut DisplayManager,
) {
    let app = rule.app.as_deref();
    let title = rule.title.as_deref();
    let role = rule.role.as_deref();
    let subrole = rule.subrole.as_deref();

    let escaped_app = app.and_then(escape_string_for_json_when_it_needs_escaping);
    let escaped_title = title.and_then(escape_string_for_json_when_it_needs_escaping);
    let escaped_role = role.and_then(escape_string_for_json_when_it_needs_escaping);
    let escaped_subrole = subrole.and_then(escape_string_for_json_when_it_needs_escaping);

    let mut flags = rule.flags;
    if rule.app_regex.is_some() {
        flags.insert(RuleFlag::APPLICATION_PATTERN_IS_VALID);
    }
    if rule.title_regex.is_some() {
        flags.insert(RuleFlag::TITLE_PATTERN_IS_VALID);
    }
    if rule.role_regex.is_some() {
        flags.insert(RuleFlag::ROLE_PATTERN_IS_VALID);
    }
    if rule.subrole_regex.is_some() {
        flags.insert(RuleFlag::SUBROLE_PATTERN_IS_VALID);
    }

    let effects_flags = rule.effects.flags;

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
         \t\"opacity\":{},\n\
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
            query_arrangement_index_of_display(rule.effects.display_id, display_manager)
        } else {
            0
        },
        if rule.effects.space_id.0 != 0 {
            query_mission_control_index_of_space(rule.effects.space_id)
        } else {
            0
        },
        json_literal_for_boolean(
            effects_flags.contains(RuleEffectsFlag::FOCUS_FOLLOWS_WINDOW_TO_ITS_SPACE)
        ),
        format_float_with_decimals_as_printf_does(rule.effects.opacity as f64, 4),
        json_literal_for_optional_boolean(rule.effects.manage),
        json_literal_for_optional_boolean(rule.effects.sticky),
        json_literal_for_optional_boolean(rule.effects.mff),
        if effects_flags.contains(RuleEffectsFlag::LAYER_IS_SET) {
            LAYER_NAMES
                .get(rule.effects.layer as usize)
                .copied()
                .flatten()
                .unwrap_or("(null)")
        } else {
            ""
        },
        json_literal_for_optional_boolean(rule.effects.fullscreen),
        rule.effects.grid[0] as i32,
        rule.effects.grid[1] as i32,
        rule.effects.grid[2] as i32,
        rule.effects.grid[3] as i32,
        rule.effects.grid[4] as i32,
        rule.effects.grid[5] as i32,
        rule.effects.scratchpad.as_deref().unwrap_or(""),
        json_literal_for_boolean(flags.contains(RuleFlag::ONE_SHOT)),
        ((rule.effects.flags.bits() as u32) << 16) | (flags.bits() as u32),
    ));
}

pub(crate) fn write_every_rule_as_json_array(
    response: &mut Response,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    response.write(format_args!("["));
    for index in 0..window_manager.rules.len() as i32 {
        let rule = &window_manager.rules[index as usize];
        write_rule_as_json_object(response, rule, index, display_manager);
        if index < window_manager.rules.len() as i32 - 1 {
            response.write(format_args!(","));
        }
    }
    response.write(format_args!("]\n"));
}
