use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::labels::DisplayLabel;
use crate::ffi::core_graphics::{CGDisplayRegisterReconfigurationCallback, kCGErrorSuccess};
use crate::notifications::display::handle_display_reconfiguration_callback;
use crate::support::handles::DisplayId;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub(crate) enum DisplayArrangementOrder {
    #[default]
    Default = 0,
    Horizontal = 1,
    Vertical = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum ExternalBarMode {
    #[default]
    Off = 0,
    MainDisplayOnly = 1,
    EveryDisplay = 2,
}

#[derive(Default)]
pub(crate) struct DisplayManager {
    pub(crate) current_display_id: DisplayId,
    pub(crate) last_display_id: DisplayId,
    pub(crate) top_padding: i32,
    pub(crate) bottom_padding: i32,
    pub(crate) order: DisplayArrangementOrder,
    pub(crate) mode: ExternalBarMode,
    pub(crate) labels: Vec<DisplayLabel>,
}

pub(crate) static DISPLAY_ARRANGEMENT_ORDER_NAMES: [&str; 3] =
    ["default", "horizontal", "vertical"];

pub(crate) static EXTERNAL_BAR_MODE_NAMES: [&str; 3] = ["off", "main", "all"];

pub(crate) fn start_display_manager_observing_display_reconfiguration(
    display_manager: &mut DisplayManager,
) -> bool {
    display_manager.current_display_id = query_display_showing_the_active_menu_bar();
    display_manager.last_display_id = display_manager.current_display_id;
    display_manager.order = DisplayArrangementOrder::Default;
    display_manager.mode = ExternalBarMode::Off;
    display_manager.top_padding = 0;
    display_manager.bottom_padding = 0;
    let registration_result = unsafe {
        CGDisplayRegisterReconfigurationCallback(
            Some(handle_display_reconfiguration_callback),
            core::ptr::null_mut(),
        )
    };
    registration_result == kCGErrorSuccess
}
