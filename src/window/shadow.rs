use crate::scripting_addition::client::scripting_addition_set_shadow;
use crate::support::handles::WindowId;
use crate::window::manager::{
    PurifyMode, WindowManager, window_manager_find_managed_window,
    window_manager_is_window_eligible,
};
use crate::window::model::{WindowFlag, window_check_flag, window_clear_flag, window_set_flag};

pub(crate) fn window_manager_set_purify_mode(window_manager: &mut WindowManager, mode: PurifyMode) {
    window_manager.purify_mode = mode;
    for window_id in window_manager.window.keys_in_bucket_order() {
        if window_manager_is_window_eligible(window_id, window_manager) {
            window_manager_purify_window(window_manager, window_id);
        }
    }
}

pub(crate) fn window_manager_purify_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    let value: i32;

    if window_manager.purify_mode == PurifyMode::Disabled {
        value = 1;
    } else if window_manager.purify_mode == PurifyMode::Managed {
        value = if window_manager_find_managed_window(window_manager, window_id).is_some() {
            0
        } else {
            1
        };
    } else
    /*if (wm->purify_mode == PURIFY_ALWAYS) */
    {
        value = 0;
    }

    if scripting_addition_set_shadow(window_id, value != 0) {
        let Some(window) = window_manager.window.find_mut(&window_id) else {
            return;
        };
        if value != 0 {
            window_set_flag(window, WindowFlag::SHADOW);
        } else {
            window_clear_flag(window, WindowFlag::SHADOW);
        }
    }
}

pub(crate) fn window_manager_toggle_window_shadow(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };

    let shadow = !window_check_flag(window, WindowFlag::SHADOW);
    if scripting_addition_set_shadow(window_id, shadow) {
        let Some(window) = window_manager.window.find_mut(&window_id) else {
            return;
        };
        if shadow {
            window_set_flag(window, WindowFlag::SHADOW);
        } else {
            window_clear_flag(window, WindowFlag::SHADOW);
        }
    }
}
