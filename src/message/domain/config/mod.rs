pub mod printing;
pub mod settings_a_space_can_override;
pub mod settings_for_every_space;

use crate::command::config::{ConfigCommand, ConfigSetArguments};
use crate::display::manager::DisplayManager;
use crate::message::domain::config::printing::print_the_settings_as_the_global_or_one_space_sees_them;
use crate::message::domain::config::settings_a_space_can_override::{
    change_the_global_settings_a_space_can_override, override_the_settings_of_space,
};
use crate::message::domain::config::settings_for_every_space::change_the_settings_for_every_space;
use crate::message::selector_resolution::resolve_space_selector;
use crate::mouse::drag::MouseDragState;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::SpaceManager;
use crate::window::manager::WindowManager;

pub(crate) fn run_config_command(
    command: ConfigCommand,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> Result<String, Vec<String>> {
    match command {
        ConfigCommand::Set(arguments) => change_the_settings(
            &arguments,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        )
        .map(|()| String::new()),
        ConfigCommand::Get { space, setting } => {
            print_the_settings_as_the_global_or_one_space_sees_them(
                space.as_ref(),
                setting,
                display_manager,
                window_manager,
                space_manager,
            )
            .map_err(|failure| vec![failure])
        }
    }
}

fn change_the_settings(
    arguments: &ConfigSetArguments,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> Result<(), Vec<String>> {
    match &arguments.space {
        Some(space_selector) => {
            let space_id = resolve_space_selector(
                space_selector,
                query_current_space_of_the_focused_display(window_manager),
                space_manager,
            )
            .map_err(|failure| vec![failure])?;
            override_the_settings_of_space(
                space_id,
                &arguments.settings_a_space_can_override,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            )
            .map_err(|failure| vec![failure])?;
        }
        None => change_the_global_settings_a_space_can_override(
            &arguments.settings_a_space_can_override,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        ),
    }

    let failures = change_the_settings_for_every_space(
        &arguments.settings_for_every_space,
        display_manager,
        window_manager,
        space_manager,
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    use super::run_config_command;
    use crate::command::config::{
        ConfigCommand, ConfigSetArguments, ConfigurationSettingName, SettingsForEverySpace,
    };
    use crate::command::values::{OnOrOff, PackedArgbColor};
    use crate::display::manager::DisplayManager;
    use crate::event::handlers::system::handle_system_accent_color_changed_event;
    use crate::mouse::drag::mouse_drag_state_without_a_drag;
    use crate::space::manager::create_space_manager_without_any_view_with_its_initial_settings;
    use crate::support::color::rgba_color_from_packed_argb;
    use crate::window::manager::{
        WindowManager, create_window_manager_tracking_nothing_with_its_initial_settings,
    };

    fn run_config_command_on(
        command: ConfigCommand,
        window_manager: &mut WindowManager,
    ) -> Result<String, Vec<String>> {
        run_config_command(
            command,
            &mut DisplayManager::default(),
            window_manager,
            &mut create_space_manager_without_any_view_with_its_initial_settings(),
            &mut mouse_drag_state_without_a_drag(),
        )
    }

    fn config_set_of_settings_for_every_space(
        settings_for_every_space: SettingsForEverySpace,
    ) -> ConfigCommand {
        ConfigCommand::Set(ConfigSetArguments {
            space: None,
            settings_a_space_can_override: Default::default(),
            settings_for_every_space,
        })
    }

    fn config_get_of(setting: ConfigurationSettingName) -> ConfigCommand {
        ConfigCommand::Get {
            space: None,
            setting: Some(setting),
        }
    }

    #[test]
    fn setting_the_insert_feedback_color_stores_it_and_the_accent_colour_no_longer_replaces_it() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let outcome = run_config_command_on(
            config_set_of_settings_for_every_space(SettingsForEverySpace {
                insert_feedback_color: Some(PackedArgbColor(0xaa336699)),
                ..Default::default()
            }),
            &mut window_manager,
        );
        handle_system_accent_color_changed_event(
            rgba_color_from_packed_argb(0xff007aff),
            &mut window_manager,
        );

        assert_eq!(outcome, Ok(String::new()));
        assert_eq!(window_manager.insert_feedback_color.packed, 0xaa336699);
        assert!(!window_manager.insert_feedback_color_follows_the_system_accent_color);
    }

    #[test]
    fn until_a_client_sets_the_insert_feedback_color_the_accent_colour_replaces_it() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        handle_system_accent_color_changed_event(
            rgba_color_from_packed_argb(0xff007aff),
            &mut window_manager,
        );

        assert_eq!(window_manager.insert_feedback_color.packed, 0xff007aff);
        assert!(window_manager.insert_feedback_color_follows_the_system_accent_color);
    }

    #[test]
    fn after_reload_config_file_on_change_is_turned_off_getting_it_prints_off() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let outcome_of_turning_it_off = run_config_command_on(
            config_set_of_settings_for_every_space(SettingsForEverySpace {
                reload_config_file_on_change: Some(OnOrOff::Off),
                ..Default::default()
            }),
            &mut window_manager,
        );
        let outcome_of_getting_it = run_config_command_on(
            config_get_of(ConfigurationSettingName::ReloadConfigFileOnChange),
            &mut window_manager,
        );

        assert_eq!(outcome_of_turning_it_off, Ok(String::new()));
        assert_eq!(outcome_of_getting_it, Ok(String::from("off\n")));
    }

    #[test]
    fn setting_group_header_colours_and_font_stores_them_in_one_call() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let outcome = run_config_command_on(
            config_set_of_settings_for_every_space(SettingsForEverySpace {
                group_header_height: Some(30),
                group_header_background_color: Some(PackedArgbColor(0)),
                group_header_inactive_color: Some(PackedArgbColor(0xff292e42)),
                group_header_font_family: Some(String::from("JetBrainsMono Nerd Font")),
                group_header_font_style: Some(String::new()),
                group_header_font_size: Some(13.5),
                ..Default::default()
            }),
            &mut window_manager,
        );

        assert_eq!(outcome, Ok(String::new()));
        let style = &window_manager.group_header_style;
        assert_eq!(style.height, 30.0);
        assert_eq!(style.background_color.packed, 0);
        assert_eq!(style.inactive_color.packed, 0xff292e42);
        assert_eq!(style.font_family, "JetBrainsMono Nerd Font");
        assert_eq!(style.font_style, "");
        assert_eq!(style.font_size, 13.5);
    }

    #[test]
    fn getting_every_setting_prints_one_json_object_ending_with_a_newline() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();

        let every_setting = run_config_command_on(
            ConfigCommand::Get {
                space: None,
                setting: None,
            },
            &mut window_manager,
        )
        .unwrap();

        assert!(every_setting.ends_with("}\n"));
        let parsed: serde_json::Value = serde_json::from_str(&every_setting).unwrap();
        assert_eq!(parsed["group_header_font_family"], "Helvetica Neue");
    }
}
