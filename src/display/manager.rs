use crate::display::identity::display_manager_active_display_id;
use crate::display::labels::DisplayLabel;
use crate::ffi::core_graphics::{CGDisplayRegisterReconfigurationCallback, kCGErrorSuccess};
use crate::notifications::display::display_handler;
use crate::support::handles::DisplayId;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub(crate) enum DisplayArrangementOrder {
    #[default]
    Default = 0,
    X = 1,
    Y = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum ExternalBarMode {
    #[default]
    Off = 0,
    Main = 1,
    All = 2,
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

pub(crate) static DISPLAY_ARRANGEMENT_ORDER_STR: [&str; 3] = ["default", "horizontal", "vertical"];

pub(crate) static EXTERNAL_BAR_MODE_STR: [&str; 3] = ["off", "main", "all"];

pub(crate) fn display_manager_begin(display_manager: &mut DisplayManager) -> bool {
    display_manager.current_display_id = display_manager_active_display_id();
    display_manager.last_display_id = display_manager.current_display_id;
    display_manager.order = DisplayArrangementOrder::Default;
    display_manager.mode = ExternalBarMode::Off;
    display_manager.top_padding = 0;
    display_manager.bottom_padding = 0;
    let registration_result = unsafe {
        CGDisplayRegisterReconfigurationCallback(Some(display_handler), core::ptr::null_mut())
    };
    registration_result == kCGErrorSuccess
}
