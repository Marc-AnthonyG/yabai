use std::sync::atomic::Ordering;

use crate::command::config::SettingsForEverySpace;
use crate::command::values::{ExternalBarPadding, PackedArgbColor};
use crate::config_file::change_watcher::start_watching_the_config_file_to_reload_it_on_change_unless_already_watching;
use crate::display::manager::DisplayManager;
use crate::ffi::core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use crate::layout::group_header::refresh_the_group_headers_of_every_view;
use crate::layout::group_header_style::GroupHeaderStyle;
use crate::layout::view::move_view_windows_into_their_areas_or_defer_until_space_is_visible;
use crate::mouse::tap::MOUSE_TAP_STATE;
use crate::scripting_addition::installer::is_system_integrity_protection_relaxed_enough_for_scripting_addition;
use crate::space::manager::{
    SpaceManager, recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date,
};
use crate::state::process_wide::{
    RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED, VERBOSE_DEBUG_OUTPUT_ENABLED,
};
use crate::support::color::rgba_color_from_packed_argb;
use crate::window::focus::set_focus_follows_mouse_mode;
use crate::window::manager::WindowManager;
use crate::window::opacity::{
    set_active_window_opacity_applying_it_to_the_focused_window, set_menu_bar_opacity,
    set_normal_window_opacity_applying_it_to_every_unfocused_window,
    set_window_opacity_enabled_for_every_eligible_window,
};
use crate::window::shadow::set_shadow_removal_mode_for_every_eligible_window;

pub(crate) fn change_the_settings_for_every_space(
    settings: &SettingsForEverySpace,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Vec<String> {
    let mut failures = Vec::new();

    if let Some(debug_output) = settings.debug_output {
        VERBOSE_DEBUG_OUTPUT_ENABLED.store(debug_output.is_on(), Ordering::Relaxed);
    }
    if let Some(reload_config_file_on_change) = settings.reload_config_file_on_change {
        RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED
            .store(reload_config_file_on_change.is_on(), Ordering::Relaxed);
        if reload_config_file_on_change.is_on() {
            start_watching_the_config_file_to_reload_it_on_change_unless_already_watching();
        }
    }
    if let Some(external_bar) = settings.external_bar {
        keep_room_for_an_external_bar(external_bar, display_manager, window_manager, space_manager);
    }
    if let Some(menubar_opacity) = settings.menubar_opacity {
        set_menu_bar_opacity(window_manager, menubar_opacity);
    }
    if let Some(mouse_follows_focus) = settings.mouse_follows_focus {
        window_manager.enable_mff = mouse_follows_focus.is_on();
    }
    if let Some(focus_follows_mouse) = settings.focus_follows_mouse {
        set_focus_follows_mouse_mode(window_manager, focus_follows_mouse);
    }
    if let Some(display_arrangement_order) = settings.display_arrangement_order {
        display_manager.order = display_arrangement_order;
    }
    if let Some(window_origin_display) = settings.window_origin_display {
        window_manager.window_origin_display_mode = window_origin_display;
    }
    if let Some(window_placement) = settings.window_placement {
        space_manager.window_placement = window_placement;
    }
    if let Some(window_insertion_point) = settings.window_insertion_point {
        space_manager.window_insertion_point = window_insertion_point;
    }
    if let Some(window_zoom_persist) = settings.window_zoom_persist {
        space_manager.window_zoom_persist = window_zoom_persist.is_on();
    }
    if let Some(skip_window_focus_animation) = settings.skip_window_focus_animation {
        space_manager.skip_window_focus_animation = skip_window_focus_animation.is_on();
    }
    if let Some(window_shadow) = settings.window_shadow {
        set_shadow_removal_mode_for_every_eligible_window(window_manager, window_shadow);
    }
    if let Some(window_opacity) = settings.window_opacity {
        set_window_opacity_enabled_for_every_eligible_window(
            window_manager,
            window_opacity.is_on(),
        );
    }
    if let Some(window_opacity_duration) = settings.window_opacity_duration {
        window_manager.window_opacity_duration = window_opacity_duration;
    }
    if let Some(active_window_opacity) = settings.active_window_opacity {
        set_active_window_opacity_applying_it_to_the_focused_window(
            window_manager,
            active_window_opacity,
        );
    }
    if let Some(normal_window_opacity) = settings.normal_window_opacity {
        set_normal_window_opacity_applying_it_to_every_unfocused_window(
            window_manager,
            normal_window_opacity,
        );
    }
    if let Some(window_animation_duration) = settings.window_animation_duration {
        if let Err(failure) = set_window_animation_duration_when_the_system_allows_it(
            window_manager,
            window_animation_duration,
        ) {
            failures.push(failure);
        }
    }
    if let Some(window_animation_easing) = settings.window_animation_easing {
        window_manager.window_animation_easing = window_animation_easing;
    }
    if let Some(insert_feedback_color) = settings.insert_feedback_color {
        window_manager.insert_feedback_color = rgba_color_from_packed_argb(insert_feedback_color.0);
        window_manager.insert_feedback_color_follows_the_system_accent_color = false;
    }
    if let Some(group_header_height) = settings.group_header_height {
        window_manager.group_header_style.height = group_header_height as f32;
        move_the_windows_of_every_view_into_their_areas(space_manager, window_manager);
    }
    if change_the_look_of_group_headers(settings, &mut window_manager.group_header_style) {
        refresh_the_group_headers_of_every_view(space_manager, window_manager);
    }
    if let Some(split_ratio) = settings.split_ratio {
        space_manager.split_ratio = split_ratio;
    }
    if let Some(mouse_modifier) = settings.mouse_modifier {
        MOUSE_TAP_STATE
            .modifier
            .store(mouse_modifier.mouse_modifier().0, Ordering::Relaxed);
    }
    if let Some(mouse_action1) = settings.mouse_action1 {
        MOUSE_TAP_STATE
            .action1
            .store(mouse_action1.mouse_mode() as u8, Ordering::Relaxed);
    }
    if let Some(mouse_action2) = settings.mouse_action2 {
        MOUSE_TAP_STATE
            .action2
            .store(mouse_action2.mouse_mode() as u8, Ordering::Relaxed);
    }
    if let Some(mouse_drop_action) = settings.mouse_drop_action {
        MOUSE_TAP_STATE
            .drop_action
            .store(mouse_drop_action.mouse_mode() as u8, Ordering::Relaxed);
    }

    failures
}

fn keep_room_for_an_external_bar(
    external_bar: ExternalBarPadding,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    display_manager.mode = external_bar.mode;
    display_manager.top_padding = i32::try_from(external_bar.top_padding).unwrap_or(i32::MAX);
    display_manager.bottom_padding = i32::try_from(external_bar.bottom_padding).unwrap_or(i32::MAX);
    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
        space_manager,
        display_manager,
        window_manager,
    );
}

fn set_window_animation_duration_when_the_system_allows_it(
    window_manager: &mut WindowManager,
    window_animation_duration: f32,
) -> Result<(), String> {
    if window_animation_duration == 0.0 {
        window_manager.window_animation_duration = window_animation_duration;
        return Ok(());
    }
    if !is_system_integrity_protection_relaxed_enough_for_scripting_addition() {
        return Err(String::from(
            "--window-animation-duration requires System Integrity Protection to be partially disabled! ignoring it..",
        ));
    }
    if !CGPreflightScreenCaptureAccess() {
        CGRequestScreenCaptureAccess();
        return Err(String::from(
            "--window-animation-duration requires Screen Recording permissions! ignoring it..",
        ));
    }
    window_manager.window_animation_duration = window_animation_duration;
    Ok(())
}

fn change_the_look_of_group_headers(
    settings: &SettingsForEverySpace,
    group_header_style: &mut GroupHeaderStyle,
) -> bool {
    let colours_with_their_settings = [
        (
            &mut group_header_style.background_color,
            settings.group_header_background_color,
        ),
        (
            &mut group_header_style.active_color,
            settings.group_header_active_color,
        ),
        (
            &mut group_header_style.inactive_color,
            settings.group_header_inactive_color,
        ),
        (
            &mut group_header_style.active_text_color,
            settings.group_header_active_text_color,
        ),
        (
            &mut group_header_style.inactive_text_color,
            settings.group_header_inactive_text_color,
        ),
    ];
    let mut did_change_the_look = false;
    for (colour, setting) in colours_with_their_settings {
        if let Some(PackedArgbColor(packed_colour)) = setting {
            *colour = rgba_color_from_packed_argb(packed_colour);
            did_change_the_look = true;
        }
    }
    if let Some(font_family) = &settings.group_header_font_family {
        group_header_style.font_family = font_family.clone();
        did_change_the_look = true;
    }
    if let Some(font_style) = &settings.group_header_font_style {
        group_header_style.font_style = font_style.clone();
        did_change_the_look = true;
    }
    if let Some(font_size) = settings.group_header_font_size {
        group_header_style.font_size = font_size;
        did_change_the_look = true;
    }
    did_change_the_look
}

fn move_the_windows_of_every_view_into_their_areas(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) {
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
            space_manager,
            space_id,
            window_manager,
        );
    }
}
