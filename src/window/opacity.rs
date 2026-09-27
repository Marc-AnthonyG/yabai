use crate::ffi::skylight::SLSSetMenuBarInsetAndAlpha;
use crate::globals::CONNECTION;
use crate::handles::WindowId;
use crate::scripting_addition::client::scripting_addition_set_opacity;
use crate::window::focus::window_manager_focused_window;
use crate::window::manager::{WindowManager, window_manager_is_window_eligible};

pub(crate) fn window_manager_set_window_opacity_enabled(
    window_manager: &mut WindowManager,
    enabled: bool,
) {
    window_manager.enable_window_opacity = enabled;
    for window_id in window_manager.window.keys_in_bucket_order() {
        if window_manager_is_window_eligible(window_id, window_manager) {
            let opacity = if enabled {
                match window_manager.window.find(&window_id) {
                    Some(window) => window.opacity,
                    None => continue,
                }
            } else {
                1.0f32
            };
            window_manager_set_opacity(window_manager, window_id, opacity);
        }
    }
}

pub(crate) fn window_manager_set_opacity(
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

    scripting_addition_set_opacity(window_id, opacity, window_manager.window_opacity_duration)
}

pub(crate) fn window_manager_set_window_opacity(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    opacity: f32,
) {
    if !window_manager.enable_window_opacity {
        return;
    }
    if !window_manager_is_window_eligible(window_id, window_manager) {
        return;
    }
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    if window.opacity != 0.0f32 {
        return;
    }

    window_manager_set_opacity(window_manager, window_id, opacity);
}

pub(crate) fn window_manager_set_menubar_opacity(window_manager: &mut WindowManager, opacity: f32) {
    window_manager.menubar_opacity = opacity;
    unsafe { SLSSetMenuBarInsetAndAlpha(*CONNECTION.get().unwrap(), 0.0, 1.0, opacity) };
}

pub(crate) fn window_manager_set_active_window_opacity(
    window_manager: &mut WindowManager,
    opacity: f32,
) {
    window_manager.active_window_opacity = opacity;
    let window = window_manager_focused_window(window_manager);
    if let Some(window_id) = window {
        window_manager_set_window_opacity(
            window_manager,
            window_id,
            window_manager.active_window_opacity,
        );
    }
}

pub(crate) fn window_manager_set_normal_window_opacity(
    window_manager: &mut WindowManager,
    opacity: f32,
) {
    window_manager.normal_window_opacity = opacity;
    for window_id in window_manager.window.keys_in_bucket_order() {
        if window_id == window_manager.focused_window_id {
            continue;
        }
        if window_manager_is_window_eligible(window_id, window_manager) {
            window_manager_set_window_opacity(
                window_manager,
                window_id,
                window_manager.normal_window_opacity,
            );
        }
    }
}
