use crate::debug;
use crate::display::manager::DisplayManager;
use crate::notifications::mission_control::{mission_control_observe, mission_control_unobserve};
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{PendingSignal, SignalContext, event_signal_push};
use crate::space::manager::{SpaceManager, space_manager_mark_spaces_invalid};
use crate::support::macos_version::{
    workspace_is_macos_monterey, workspace_is_macos_sequoia, workspace_is_macos_sonoma,
    workspace_is_macos_tahoe, workspace_is_macos_ventura,
};
use crate::window::focus::window_manager_center_mouse;
use crate::window::manager::{WindowManager, window_manager_find_window};
use crate::window::opacity::window_manager_set_window_opacity;

pub(crate) fn event_handler_dock_did_restart(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "EVENT_HANDLER_DOCK_DID_RESTART");

    if workspace_is_macos_monterey()
        || workspace_is_macos_ventura()
        || workspace_is_macos_sonoma()
        || workspace_is_macos_sequoia()
        || workspace_is_macos_tahoe()
    {
        mission_control_unobserve();
        mission_control_observe();
    }

    event_signal_push(
        SignalType::DockDidRestart,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_menu_bar_hidden_changed(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "EVENT_HANDLER_MENU_BAR_HIDDEN_CHANGED");
    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
    event_signal_push(
        SignalType::MenuBarHiddenChanged,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_dock_did_change_pref(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "EVENT_HANDLER_DOCK_DID_CHANGE_PREF");
    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
    event_signal_push(
        SignalType::DockDidChangePref,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_system_woke(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "EVENT_HANDLER_SYSTEM_WOKE");

    let focused_window =
        window_manager_find_window(window_manager, window_manager.focused_window_id);
    if let Some(focused_window) = focused_window {
        let active_window_opacity = window_manager.active_window_opacity;
        window_manager_set_window_opacity(window_manager, focused_window, active_window_opacity);
        window_manager_center_mouse(window_manager, focused_window);
    }

    event_signal_push(
        SignalType::SystemWoke,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}
