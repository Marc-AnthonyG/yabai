use crate::scripting_addition::client::set_window_shadow_through_scripting_addition;
use crate::support::handles::WindowId;
use crate::window::manager::{
    ShadowRemovalMode, WindowManager, is_window_eligible_for_management, space_managing_window,
};
use crate::window::model::{WindowFlag, clear_window_flag, is_window_flag_set, set_window_flag};

pub(crate) fn set_shadow_removal_mode_for_every_eligible_window(
    window_manager: &mut WindowManager,
    mode: ShadowRemovalMode,
) {
    window_manager.shadow_removal_mode = mode;
    for window_id in window_manager.window.keys().copied().collect::<Vec<_>>() {
        if is_window_eligible_for_management(window_id, window_manager) {
            apply_shadow_removal_mode_to_window(window_manager, window_id);
        }
    }
}

pub(crate) fn apply_shadow_removal_mode_to_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    let value: i32;

    if window_manager.shadow_removal_mode == ShadowRemovalMode::Never {
        value = 1;
    } else if window_manager.shadow_removal_mode == ShadowRemovalMode::FromManagedWindows {
        value = if space_managing_window(window_manager, window_id).is_some() {
            0
        } else {
            1
        };
    } else
    /*if (wm->purify_mode == PURIFY_ALWAYS) */
    {
        value = 0;
    }

    if set_window_shadow_through_scripting_addition(window_id, value != 0) {
        let Some(window) = window_manager.window.get_mut(&window_id) else {
            return;
        };
        if value != 0 {
            set_window_flag(window, WindowFlag::HAS_SHADOW);
        } else {
            clear_window_flag(window, WindowFlag::HAS_SHADOW);
        }
    }
}

pub(crate) fn toggle_window_shadow(window_id: WindowId, window_manager: &mut WindowManager) {
    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };

    let shadow = !is_window_flag_set(window, WindowFlag::HAS_SHADOW);
    if set_window_shadow_through_scripting_addition(window_id, shadow) {
        let Some(window) = window_manager.window.get_mut(&window_id) else {
            return;
        };
        if shadow {
            set_window_flag(window, WindowFlag::HAS_SHADOW);
        } else {
            clear_window_flag(window, WindowFlag::HAS_SHADOW);
        }
    }
}
