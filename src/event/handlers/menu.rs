use crate::debug;
use crate::support::handles::WindowId;
use crate::window::manager::{FocusFollowsMouseMode, WindowManager};

pub(crate) fn handle_menu_opened_event(
    _window_id: WindowId,
    window_manager: &mut WindowManager,
    focus_follows_mouse_suspended_value: &mut FocusFollowsMouseMode,
    is_menu_open: &mut i32,
) {
    debug!("{}\n", "handle_menu_opened_event");
    *is_menu_open += 1;

    if *is_menu_open == 1 {
        *focus_follows_mouse_suspended_value = window_manager.focus_follows_mouse_mode;
        window_manager.focus_follows_mouse_mode = FocusFollowsMouseMode::Disabled;
    }
}

pub(crate) fn handle_menu_closed_event(
    window_manager: &mut WindowManager,
    focus_follows_mouse_suspended_value: &mut FocusFollowsMouseMode,
    is_menu_open: &mut i32,
) {
    debug!("{}\n", "handle_menu_closed_event");
    *is_menu_open -= 1;

    if *is_menu_open == 0 {
        window_manager.focus_follows_mouse_mode = *focus_follows_mouse_suspended_value;
    } else if *is_menu_open < 0 {
        *is_menu_open = 0;
    }
}
