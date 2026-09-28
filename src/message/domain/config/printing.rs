use crate::command::config::ConfigurationSettingName;
use crate::command::selectors::SpaceSelector;
use crate::display::manager::DisplayManager;
use crate::message::selector_resolution::resolve_space_selector;
use crate::serialise::configuration::{
    EffectiveConfiguration, effective_configuration_as_the_view_sees_it,
    effective_global_configuration, every_setting_as_pretty_json,
    value_of_one_setting_as_bare_text,
};
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::window::manager::WindowManager;

pub(crate) fn print_the_settings_as_the_global_or_one_space_sees_them(
    space: Option<&SpaceSelector>,
    setting: Option<ConfigurationSettingName>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<String, String> {
    let configuration = match space {
        Some(space_selector) => effective_configuration_as_one_space_sees_it(
            space_selector,
            display_manager,
            window_manager,
            space_manager,
        )?,
        None => effective_global_configuration(display_manager, window_manager, space_manager),
    };

    let text = match setting {
        Some(setting) => value_of_one_setting_as_bare_text(&configuration, setting),
        None => every_setting_as_pretty_json(&configuration),
    };
    Ok(text + "\n")
}

fn effective_configuration_as_one_space_sees_it(
    space_selector: &SpaceSelector,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<EffectiveConfiguration, String> {
    let space_id = resolve_space_selector(
        space_selector,
        query_current_space_of_the_focused_display(window_manager),
        space_manager,
    )?;
    let view_space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let global_configuration =
        effective_global_configuration(display_manager, window_manager, space_manager);

    Ok(match space_manager.view.get(&view_space_id) {
        Some(view) => effective_configuration_as_the_view_sees_it(view, global_configuration),
        None => global_configuration,
    })
}
