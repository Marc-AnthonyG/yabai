use crate::debug;
use crate::display::identity::{query_display_showing_the_active_menu_bar, query_main_display};
use crate::display::labels::remove_label_of_display;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::ffi::skylight::SLSSetMenuBarInsetAndAlpha;
use crate::layout::settings::ViewFlag;
use crate::layout::tree::move_windows_below_node_into_their_areas;
use crate::layout::view::{
    has_view_out_of_date_areas, has_view_windows_awaiting_their_areas,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{
    PendingSignal, SignalContext, queue_pending_signal_for_its_subscribers,
};
use crate::space::managed_space::{
    is_native_fullscreen_space, is_user_space, query_display_holding_space,
};
use crate::space::manager::{
    SpaceManager, find_or_create_view_for_space, reattach_views_to_spaces_of_added_display_by_uuid,
    recompute_current_view_of_display_and_mark_its_other_views_out_of_date,
    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date,
};
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{DisplayId, ROOT_NODE_ID};
use crate::window::discovery::retry_tracking_windows_of_applications_with_unresolved_windows;
use crate::window::focus::{query_focused_tracked_window, respond_to_window_receiving_focus};
use crate::window::manager::{
    WindowManager, forget_focused_event_that_arrived_before_window_was_tracked,
    has_focused_event_arrived_before_window_was_tracked,
};
use crate::window::space_reconciliation::{
    reconcile_space_view_with_windows_on_space, reconcile_views_after_display_added_or_removed,
};

pub(crate) fn handle_display_changed_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let new_display_id = query_display_showing_the_active_menu_bar();
    if display_manager.current_display_id == new_display_id {
        debug!(
            "{}: newly activated display {} was already active ({})! ignoring event..\n",
            "handle_display_changed_event",
            display_manager.current_display_id.0 as i32,
            new_display_id.0 as i32
        );
        return;
    }

    display_manager.last_display_id = display_manager.current_display_id;
    display_manager.current_display_id = new_display_id;

    space_manager.last_space_id = space_manager.current_space_id;
    space_manager.current_space_id =
        query_current_space_of_display(display_manager.current_display_id);

    let expected_display_id = query_display_holding_space(space_manager.current_space_id);
    if display_manager.current_display_id != expected_display_id {
        debug!(
            "{}: {} {} did not match {}! ignoring event..\n",
            "handle_display_changed_event",
            display_manager.current_display_id.0 as i32,
            space_manager.current_space_id.0 as i64,
            expected_display_id.0 as i32
        );
        return;
    }

    if window_manager.menubar_opacity != 1.0f32 {
        let alpha = if is_native_fullscreen_space(space_manager.current_space_id) {
            1.0f32
        } else {
            window_manager.menubar_opacity
        };
        unsafe {
            SLSSetMenuBarInsetAndAlpha(
                *SKYLIGHT_CONNECTION_ID.get().unwrap(),
                0 as f64,
                1 as f64,
                alpha,
            )
        };
    }

    debug!(
        "{}: {} {}\n",
        "handle_display_changed_event",
        display_manager.current_display_id.0 as i32,
        space_manager.current_space_id.0 as i64
    );
    let view = find_or_create_view_for_space(
        space_manager,
        space_manager.current_space_id,
        display_manager,
        window_manager,
    );

    if retry_tracking_windows_of_applications_with_unresolved_windows(
        space_manager,
        process_manager,
        display_manager,
        window_manager,
        mouse_drag_state,
        mission_control_mode,
    ) {
        let focused_window = query_focused_tracked_window(window_manager);
        if let Some(focused_window) = focused_window
            && has_focused_event_arrived_before_window_was_tracked(window_manager, focused_window)
        {
            respond_to_window_receiving_focus(
                window_manager,
                mouse_drag_state,
                focused_window,
                space_manager,
            );
            forget_focused_event_that_arrived_before_window_was_tracked(
                window_manager,
                focused_window,
            );
        }
    }

    if !is_mission_control_active(mission_control_mode)
        && is_user_space(space_manager.current_space_id)
    {
        reconcile_space_view_with_windows_on_space(
            space_manager,
            window_manager,
            space_manager.current_space_id,
            display_manager,
            mouse_drag_state,
        );

        if has_view_out_of_date_areas(space_manager, view) {
            recompute_view_areas_from_display_bounds_and_padding(
                space_manager,
                view,
                display_manager,
                window_manager,
            );
        }

        if has_view_windows_awaiting_their_areas(space_manager, view) {
            move_windows_below_node_into_their_areas(
                view,
                ROOT_NODE_ID,
                window_manager,
                space_manager,
            );
            if let Some(view) = space_manager.view.find_mut(&view) {
                view.clear_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
            }
        }
    }

    queue_pending_signal_for_its_subscribers(
        SignalType::DisplayChanged,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_display_added_event(
    display_id: DisplayId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    debug!(
        "{}: {}\n",
        "handle_display_added_event", display_id.0 as i32
    );
    reattach_views_to_spaces_of_added_display_by_uuid(
        space_manager,
        display_id,
        window_manager,
        mouse_drag_state,
    );
    reconcile_views_after_display_added_or_removed(
        space_manager,
        window_manager,
        display_id,
        display_manager,
        mouse_drag_state,
    );
    queue_pending_signal_for_its_subscribers(
        SignalType::DisplayAdded,
        SignalContext::Display(display_id),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_display_removed_event(
    display_id: DisplayId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    debug!(
        "{}: {}\n",
        "handle_display_removed_event", display_id.0 as i32
    );
    remove_label_of_display(display_manager, display_id);
    reconcile_views_after_display_added_or_removed(
        space_manager,
        window_manager,
        query_main_display(),
        display_manager,
        mouse_drag_state,
    );
    queue_pending_signal_for_its_subscribers(
        SignalType::DisplayRemoved,
        SignalContext::Display(display_id),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_display_moved_event(
    display_id: DisplayId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!(
        "{}: {}\n",
        "handle_display_moved_event", display_id.0 as i32
    );
    recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
        space_manager,
        display_manager,
        window_manager,
    );
    queue_pending_signal_for_its_subscribers(
        SignalType::DisplayMoved,
        SignalContext::Display(display_id),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_display_resized_event(
    display_id: DisplayId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    debug!(
        "{}: {}\n",
        "handle_display_resized_event", display_id.0 as i32
    );
    recompute_current_view_of_display_and_mark_its_other_views_out_of_date(
        space_manager,
        display_id,
        display_manager,
        window_manager,
    );
    queue_pending_signal_for_its_subscribers(
        SignalType::DisplayResized,
        SignalContext::Display(display_id),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}
