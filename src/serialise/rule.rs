use crate::display::arrangement::display_manager_display_id_arrangement;
use crate::display::manager::DisplayManager;
use crate::space::lookup::space_manager_mission_control_index;
use crate::support::json::{json_bool, json_optional_bool, ts_string_escape};
use crate::support::layer::LAYER_STR;
use crate::support::response::Response;
use crate::window::manager::WindowManager;
use crate::window::rule::{Rule, RuleEffectsFlag, RuleFlag};

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

pub(crate) fn window_manager_query_window_rules(
    response: &mut Response,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    response.write(format_args!("["));
    for index in 0..window_manager.rules.len() as i32 {
        let rule = &window_manager.rules[index as usize];
        rule_serialize(response, rule, index, display_manager);
        if index < window_manager.rules.len() as i32 - 1 {
            response.write(format_args!(","));
        }
    }
    response.write(format_args!("]\n"));
}
