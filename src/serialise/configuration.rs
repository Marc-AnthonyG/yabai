use std::sync::atomic::Ordering;

use serde::Serialize;

use crate::command::config::{
    AutoBalanceAxes, ConfigurationSettingName, MouseButtonAction, MouseDropAction, MouseModifierKey,
};
use crate::command::values::{ExternalBarPadding, OnOrOff, PackedArgbColor};
use crate::display::manager::{DisplayArrangementOrder, DisplayManager};
use crate::layout::insertion::WindowInsertionPoint;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::layout::view::View;
use crate::mouse::tap::{MOUSE_TAP_STATE, MouseMode, MouseModifier};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::{
    RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED, VERBOSE_DEBUG_OUTPUT_ENABLED,
};
use crate::support::easing::AnimationEasingType;
use crate::window::manager::{
    FocusFollowsMouseMode, ShadowRemovalMode, WindowManager, WindowOriginDisplayMode,
};

#[derive(Serialize, Debug)]
pub(crate) struct EffectiveConfiguration {
    pub(crate) layout: ViewLayout,
    pub(crate) split_type: WindowNodeSplit,
    pub(crate) top_padding: i32,
    pub(crate) bottom_padding: i32,
    pub(crate) left_padding: i32,
    pub(crate) right_padding: i32,
    pub(crate) window_gap: i32,
    pub(crate) auto_balance: AutoBalanceAxes,
    pub(crate) debug_output: OnOrOff,
    pub(crate) reload_config_file_on_change: OnOrOff,
    pub(crate) external_bar: ExternalBarPadding,
    pub(crate) menubar_opacity: f32,
    pub(crate) mouse_follows_focus: OnOrOff,
    pub(crate) focus_follows_mouse: FocusFollowsMouseMode,
    pub(crate) display_arrangement_order: DisplayArrangementOrder,
    pub(crate) window_origin_display: WindowOriginDisplayMode,
    pub(crate) window_placement: WindowNodeChild,
    pub(crate) window_insertion_point: WindowInsertionPoint,
    pub(crate) window_zoom_persist: OnOrOff,
    pub(crate) skip_window_focus_animation: OnOrOff,
    pub(crate) window_shadow: ShadowRemovalMode,
    pub(crate) window_opacity: OnOrOff,
    pub(crate) window_opacity_duration: f32,
    pub(crate) active_window_opacity: f32,
    pub(crate) normal_window_opacity: f32,
    pub(crate) window_animation_duration: f32,
    pub(crate) window_animation_easing: AnimationEasingType,
    pub(crate) insert_feedback_color: PackedArgbColor,
    pub(crate) group_header_height: u32,
    pub(crate) group_header_background_color: PackedArgbColor,
    pub(crate) group_header_active_color: PackedArgbColor,
    pub(crate) group_header_inactive_color: PackedArgbColor,
    pub(crate) group_header_active_text_color: PackedArgbColor,
    pub(crate) group_header_inactive_text_color: PackedArgbColor,
    pub(crate) group_header_font_family: String,
    pub(crate) group_header_font_style: String,
    pub(crate) group_header_font_size: f32,
    pub(crate) split_ratio: f32,
    pub(crate) mouse_modifier: Option<MouseModifierKey>,
    pub(crate) mouse_action1: Option<MouseButtonAction>,
    pub(crate) mouse_action2: Option<MouseButtonAction>,
    pub(crate) mouse_drop_action: Option<MouseDropAction>,
}

pub(crate) fn effective_global_configuration(
    display_manager: &DisplayManager,
    window_manager: &WindowManager,
    space_manager: &SpaceManager,
) -> EffectiveConfiguration {
    let group_header_style = &window_manager.group_header_style;
    EffectiveConfiguration {
        layout: space_manager.layout,
        split_type: space_manager.split_type,
        top_padding: space_manager.top_padding,
        bottom_padding: space_manager.bottom_padding,
        left_padding: space_manager.left_padding,
        right_padding: space_manager.right_padding,
        window_gap: space_manager.window_gap,
        auto_balance: AutoBalanceAxes::balancing_split_axes(space_manager.auto_balance),
        debug_output: OnOrOff::of_switch(VERBOSE_DEBUG_OUTPUT_ENABLED.load(Ordering::Relaxed)),
        reload_config_file_on_change: OnOrOff::of_switch(
            RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED.load(Ordering::Relaxed),
        ),
        external_bar: ExternalBarPadding {
            mode: display_manager.mode,
            top_padding: display_manager.top_padding.max(0) as u32,
            bottom_padding: display_manager.bottom_padding.max(0) as u32,
        },
        menubar_opacity: window_manager.menubar_opacity,
        mouse_follows_focus: OnOrOff::of_switch(window_manager.enable_mff),
        focus_follows_mouse: window_manager.focus_follows_mouse_mode,
        display_arrangement_order: display_manager.order,
        window_origin_display: window_manager.window_origin_display_mode,
        window_placement: space_manager.window_placement,
        window_insertion_point: space_manager.window_insertion_point,
        window_zoom_persist: OnOrOff::of_switch(space_manager.window_zoom_persist),
        skip_window_focus_animation: OnOrOff::of_switch(space_manager.skip_window_focus_animation),
        window_shadow: window_manager.shadow_removal_mode,
        window_opacity: OnOrOff::of_switch(window_manager.enable_window_opacity),
        window_opacity_duration: window_manager.window_opacity_duration,
        active_window_opacity: window_manager.active_window_opacity,
        normal_window_opacity: window_manager.normal_window_opacity,
        window_animation_duration: window_manager.window_animation_duration,
        window_animation_easing: window_manager.window_animation_easing,
        insert_feedback_color: PackedArgbColor(window_manager.insert_feedback_color.packed),
        group_header_height: group_header_style.height as u32,
        group_header_background_color: PackedArgbColor(group_header_style.background_color.packed),
        group_header_active_color: PackedArgbColor(group_header_style.active_color.packed),
        group_header_inactive_color: PackedArgbColor(group_header_style.inactive_color.packed),
        group_header_active_text_color: PackedArgbColor(
            group_header_style.active_text_color.packed,
        ),
        group_header_inactive_text_color: PackedArgbColor(
            group_header_style.inactive_text_color.packed,
        ),
        group_header_font_family: group_header_style.font_family.clone(),
        group_header_font_style: group_header_style.font_style.clone(),
        group_header_font_size: group_header_style.font_size,
        split_ratio: space_manager.split_ratio,
        mouse_modifier: MouseModifierKey::of_mouse_modifier(MouseModifier(
            MOUSE_TAP_STATE.modifier.load(Ordering::Relaxed),
        )),
        mouse_action1: MouseButtonAction::of_mouse_mode(MouseMode::from_discriminant(
            MOUSE_TAP_STATE.action1.load(Ordering::Relaxed),
        )),
        mouse_action2: MouseButtonAction::of_mouse_mode(MouseMode::from_discriminant(
            MOUSE_TAP_STATE.action2.load(Ordering::Relaxed),
        )),
        mouse_drop_action: MouseDropAction::of_mouse_mode(MouseMode::from_discriminant(
            MOUSE_TAP_STATE.drop_action.load(Ordering::Relaxed),
        )),
    }
}

pub(crate) fn effective_configuration_as_the_view_sees_it(
    view: &View,
    global_configuration: EffectiveConfiguration,
) -> EffectiveConfiguration {
    EffectiveConfiguration {
        layout: view.layout,
        split_type: view.split_type,
        top_padding: view.top_padding,
        bottom_padding: view.bottom_padding,
        left_padding: view.left_padding,
        right_padding: view.right_padding,
        window_gap: view.window_gap,
        auto_balance: AutoBalanceAxes::balancing_split_axes(view.auto_balance),
        ..global_configuration
    }
}

pub(crate) fn every_setting_as_pretty_json(configuration: &EffectiveConfiguration) -> String {
    serde_json::to_string_pretty(configuration).unwrap_or_default()
}

pub(crate) fn value_of_one_setting_as_bare_text(
    configuration: &EffectiveConfiguration,
    setting: ConfigurationSettingName,
) -> String {
    let every_setting = serde_json::to_value(configuration).unwrap_or_default();
    let key_of_the_setting = serde_json::to_value(setting).unwrap_or_default();
    let value_of_the_setting = key_of_the_setting
        .as_str()
        .and_then(|key| every_setting.get(key))
        .cloned()
        .unwrap_or_default();

    match value_of_the_setting {
        serde_json::Value::String(text) => text,
        other_value => other_value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::{
        effective_global_configuration, every_setting_as_pretty_json,
        value_of_one_setting_as_bare_text,
    };
    use crate::command::config::ConfigurationSettingName;
    use crate::display::manager::DisplayManager;
    use crate::space::manager::create_space_manager_without_any_view_with_its_initial_settings;
    use crate::window::manager::create_window_manager_tracking_nothing_with_its_initial_settings;

    #[test]
    fn every_setting_name_is_a_key_of_the_effective_configuration_and_nothing_else_is() {
        let configuration = effective_global_configuration(
            &DisplayManager::default(),
            &create_window_manager_tracking_nothing_with_its_initial_settings(),
            &create_space_manager_without_any_view_with_its_initial_settings(),
        );
        let every_setting = serde_json::to_value(&configuration).unwrap();
        let keys: Vec<&String> = every_setting.as_object().unwrap().keys().collect();

        let keys_of_the_setting_names: Vec<String> = ConfigurationSettingName::value_variants()
            .iter()
            .map(|setting| {
                serde_json::to_value(setting)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();

        assert_eq!(keys, keys_of_the_setting_names.iter().collect::<Vec<_>>());
    }

    #[test]
    fn one_setting_prints_bare_in_the_spelling_config_set_takes() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();
        window_manager.group_header_style.font_family = String::from("JetBrainsMono Nerd Font");
        let configuration = effective_global_configuration(
            &DisplayManager::default(),
            &window_manager,
            &create_space_manager_without_any_view_with_its_initial_settings(),
        );

        for (setting, expected_text) in [
            (
                ConfigurationSettingName::GroupHeaderFontFamily,
                "JetBrainsMono Nerd Font",
            ),
            (
                ConfigurationSettingName::GroupHeaderActiveColor,
                "0xff3d59a1",
            ),
            (
                ConfigurationSettingName::GroupHeaderBackgroundColor,
                "0x00000000",
            ),
            (ConfigurationSettingName::GroupHeaderHeight, "24"),
            (ConfigurationSettingName::GroupHeaderFontSize, "12.0"),
            (ConfigurationSettingName::ExternalBar, "off:0:0"),
            (ConfigurationSettingName::WindowPlacement, "second-child"),
            (ConfigurationSettingName::FocusFollowsMouse, "off"),
            (ConfigurationSettingName::WindowShadow, "on"),
        ] {
            assert_eq!(
                value_of_one_setting_as_bare_text(&configuration, setting),
                expected_text,
                "{setting:?}"
            );
        }
    }

    #[test]
    fn every_setting_prints_as_one_json_object_with_snake_case_keys() {
        let configuration = effective_global_configuration(
            &DisplayManager::default(),
            &create_window_manager_tracking_nothing_with_its_initial_settings(),
            &create_space_manager_without_any_view_with_its_initial_settings(),
        );

        let every_setting = every_setting_as_pretty_json(&configuration);
        let keys_of_every_setting: Vec<String> =
            serde_json::from_str::<serde_json::Value>(&every_setting)
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();

        assert!(every_setting.starts_with("{\n  \"layout\": "));
        assert!(keys_of_every_setting.contains(&String::from("window_animation_easing")));
        assert!(keys_of_every_setting.iter().all(|key| !key.contains('-')));
    }
}
