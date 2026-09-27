use crate::debug;
use crate::support::handles::WindowId;
use crate::window::manager::{FfmMode, WindowManager};

pub(crate) fn event_handler_menu_opened(
    _window_id: WindowId,
    window_manager: &mut WindowManager,
    focus_follows_mouse_suspended_value: &mut FfmMode,
    is_menu_open: &mut i32,
) {
    debug!("{}\n", "EVENT_HANDLER_MENU_OPENED");
    *is_menu_open += 1;

    if *is_menu_open == 1 {
        *focus_follows_mouse_suspended_value = window_manager.ffm_mode;
        window_manager.ffm_mode = FfmMode::Disabled;
    }
}

pub(crate) fn event_handler_menu_closed(
    window_manager: &mut WindowManager,
    focus_follows_mouse_suspended_value: &mut FfmMode,
    is_menu_open: &mut i32,
) {
    debug!("{}\n", "EVENT_HANDLER_MENU_CLOSED");
    *is_menu_open -= 1;

    if *is_menu_open == 0 {
        window_manager.ffm_mode = *focus_follows_mouse_suspended_value;
    } else if *is_menu_open < 0 {
        *is_menu_open = 0;
    }
}
