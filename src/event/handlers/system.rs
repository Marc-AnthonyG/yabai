use crate::debug;
use crate::display::manager::DisplayManager;
use crate::notifications::mission_control::{
    start_observing_mission_control_through_the_dock,
    stop_observing_mission_control_through_the_dock,
};
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{
    PendingSignal, SignalContext, queue_pending_signal_for_its_subscribers,
};
use crate::space::manager::{
    SpaceManager, recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date,
};
use crate::support::color::RgbaColor;
use crate::support::macos_version::{
    is_running_on_macos_monterey, is_running_on_macos_sequoia, is_running_on_macos_sonoma,
    is_running_on_macos_tahoe, is_running_on_macos_ventura,
};
use crate::window::focus::warp_cursor_to_window_center_if_mouse_follows_focus;
use crate::window::manager::{WindowManager, tracked_window_with_id};
use crate::window::opacity::set_window_opacity_unless_disabled_or_fixed_by_rule;

pub(crate) fn handle_dock_did_restart_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "handle_dock_did_restart_event");

    if is_running_on_macos_monterey()
        || is_running_on_macos_ventura()
        || is_running_on_macos_sonoma()
        || is_running_on_macos_sequoia()
        || is_running_on_macos_tahoe()
    {
        stop_observing_mission_control_through_the_dock();
        start_observing_mission_control_through_the_dock();
    }

    queue_pending_signal_for_its_subscribers(
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

pub(crate) fn handle_menu_bar_hidden_changed_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "handle_menu_bar_hidden_changed_event");
    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
        space_manager,
        display_manager,
        window_manager,
    );
    queue_pending_signal_for_its_subscribers(
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

pub(crate) fn handle_dock_did_change_preferences_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "handle_dock_did_change_preferences_event");
    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
        space_manager,
        display_manager,
        window_manager,
    );
    queue_pending_signal_for_its_subscribers(
        SignalType::DockDidChangePreferences,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_system_woke_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!("{}:\n", "handle_system_woke_event");

    let focused_window = tracked_window_with_id(window_manager, window_manager.focused_window_id);
    if let Some(focused_window) = focused_window {
        let active_window_opacity = window_manager.active_window_opacity;
        set_window_opacity_unless_disabled_or_fixed_by_rule(
            window_manager,
            focused_window,
            active_window_opacity,
        );
        warp_cursor_to_window_center_if_mouse_follows_focus(window_manager, focused_window);
    }

    queue_pending_signal_for_its_subscribers(
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

pub(crate) fn handle_system_accent_color_changed_event(
    accent_color: RgbaColor,
    window_manager: &mut WindowManager,
) {
    debug!(
        "{}: 0x{:x}\n",
        "EVENT_HANDLER_SYSTEM_ACCENT_COLOR_CHANGED", accent_color.packed
    );

    if window_manager.insert_feedback_color_follows_the_system_accent_color {
        window_manager.insert_feedback_color = accent_color;
    }
}
