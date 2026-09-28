use crate::ffi::skylight::SLSSetMenuBarInsetAndAlpha;
use crate::scripting_addition::client::set_window_opacity_through_scripting_addition;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::WindowId;
use crate::window::focus::query_focused_tracked_window;
use crate::window::manager::{WindowManager, is_window_eligible_for_management};

pub(crate) fn set_window_opacity_enabled_for_every_eligible_window(
    window_manager: &mut WindowManager,
    enabled: bool,
) {
    window_manager.enable_window_opacity = enabled;
    for window_id in window_manager.window.keys().copied().collect::<Vec<_>>() {
        if is_window_eligible_for_management(window_id, window_manager) {
            let opacity = if enabled {
                match window_manager.window.get(&window_id) {
                    Some(window) => window.opacity,
                    None => continue,
                }
            } else {
                1.0f32
            };
            apply_opacity_to_window_through_scripting_addition(window_manager, window_id, opacity);
        }
    }
}

pub(crate) fn apply_opacity_to_window_through_scripting_addition(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    opacity: f32,
) -> bool {
    let mut opacity = opacity;

    if opacity == 0.0f32 {
        if window_manager.enable_window_opacity {
            opacity = if window_id == window_manager.focused_window_id {
                window_manager.active_window_opacity
            } else {
                window_manager.normal_window_opacity
            };
        } else {
            opacity = 1.0f32;
        }
    }

    set_window_opacity_through_scripting_addition(
        window_id,
        opacity,
        window_manager.window_opacity_duration,
    )
}

pub(crate) fn set_window_opacity_unless_disabled_or_fixed_by_rule(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    opacity: f32,
) {
    if !window_manager.enable_window_opacity {
        return;
    }
    if !is_window_eligible_for_management(window_id, window_manager) {
        return;
    }
    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };
    if window.opacity != 0.0f32 {
        return;
    }

    apply_opacity_to_window_through_scripting_addition(window_manager, window_id, opacity);
}

pub(crate) fn set_menu_bar_opacity(window_manager: &mut WindowManager, opacity: f32) {
    window_manager.menubar_opacity = opacity;
    unsafe {
        SLSSetMenuBarInsetAndAlpha(*SKYLIGHT_CONNECTION_ID.get().unwrap(), 0.0, 1.0, opacity)
    };
}

pub(crate) fn set_active_window_opacity_applying_it_to_the_focused_window(
    window_manager: &mut WindowManager,
    opacity: f32,
) {
    window_manager.active_window_opacity = opacity;
    let window = query_focused_tracked_window(window_manager);
    if let Some(window_id) = window {
        set_window_opacity_unless_disabled_or_fixed_by_rule(
            window_manager,
            window_id,
            window_manager.active_window_opacity,
        );
    }
}

pub(crate) fn set_normal_window_opacity_applying_it_to_every_unfocused_window(
    window_manager: &mut WindowManager,
    opacity: f32,
) {
    window_manager.normal_window_opacity = opacity;
    for window_id in window_manager.window.keys().copied().collect::<Vec<_>>() {
        if window_id == window_manager.focused_window_id {
            continue;
        }
        if is_window_eligible_for_management(window_id, window_manager) {
            set_window_opacity_unless_disabled_or_fixed_by_rule(
                window_manager,
                window_id,
                window_manager.normal_window_opacity,
            );
        }
    }
}
