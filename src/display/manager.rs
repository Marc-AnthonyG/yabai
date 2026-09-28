use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::labels::DisplayLabel;
use crate::ffi::core_graphics::{CGDisplayRegisterReconfigurationCallback, kCGErrorSuccess};
use crate::notifications::display::handle_display_reconfiguration_callback;
use crate::support::handles::DisplayId;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(usize)]
pub(crate) enum DisplayArrangementOrder {
    #[default]
    Default = 0,
    Horizontal = 1,
    Vertical = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(i32)]
pub(crate) enum ExternalBarMode {
    #[default]
    Off = 0,
    #[value(name = "main")]
    #[serde(rename = "main")]
    MainDisplayOnly = 1,
    #[value(name = "all")]
    #[serde(rename = "all")]
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
