use crate::debug;
use crate::display::manager::DisplayManager;
use crate::ffi::skylight::{SLSSetMenuBarInsetAndAlpha, SLSSpaceGetType};
use crate::layout::group_header::refresh_the_group_headers_of_view;
use crate::layout::settings::ViewFlag;
use crate::layout::tree::move_windows_below_node_into_their_areas;
use crate::layout::view::{
    free_view_tree_and_release_its_uuid, has_view_out_of_date_areas,
    has_view_windows_awaiting_their_areas, recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{
    PendingSignal, SignalContext, queue_pending_signal_for_its_subscribers,
};
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::labels::remove_label_of_space;
use crate::space::managed_space::{is_native_fullscreen_space, is_user_space};
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{ROOT_NODE_ID, SpaceId};
use crate::window::discovery::retry_tracking_windows_of_applications_with_unresolved_windows;
use crate::window::focus::{query_focused_tracked_window, respond_to_window_receiving_focus};
use crate::window::focus_follows_mouse::schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles;
use crate::window::manager::{
    WindowManager, forget_focused_event_that_arrived_before_window_was_tracked,
    has_focused_event_arrived_before_window_was_tracked,
};
use crate::window::space_reconciliation::reconcile_space_view_with_windows_on_space;

pub(crate) fn handle_skylight_space_created_event(
    space_id: SpaceId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let space_type = unsafe { SLSSpaceGetType(*SKYLIGHT_CONNECTION_ID.get().unwrap(), space_id.0) };

    if space_type == 0 || space_type == 4 {
        debug!(
            "{}: {}, {}\n",
            "handle_skylight_space_created_event", space_id.0 as i64, space_type
        );
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
        queue_pending_signal_for_its_subscribers(
            SignalType::SpaceCreated,
            SignalContext::Space(space_id),
            signal_event,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            signal_storage,
        );
    }
}

pub(crate) fn handle_skylight_space_destroyed_event(
    space_id: SpaceId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    if space_manager.view.contains_key(&space_id) {
        debug!(
            "{}: {}\n",
            "handle_skylight_space_destroyed_event", space_id.0 as i64
        );
        remove_label_of_space(space_manager, space_id);
        free_view_tree_and_release_its_uuid(
            space_manager,
            space_id,
            window_manager,
            mouse_drag_state,
        );
        drop(space_manager.view.remove(&space_id));
        queue_pending_signal_for_its_subscribers(
            SignalType::SpaceDestroyed,
            SignalContext::Space(space_id),
            signal_event,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            signal_storage,
        );
    }
}

pub(crate) fn handle_space_changed_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    space_manager.last_space_id = space_manager.current_space_id;
    space_manager.current_space_id = query_current_space_of_the_focused_display(window_manager);

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
        "{}: {}\n",
        "handle_space_changed_event", space_manager.current_space_id.0 as i64
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
            if let Some(view) = space_manager.view.get_mut(&view) {
                view.clear_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
            }
        }

        refresh_the_group_headers_of_view(view, space_manager, window_manager);
        schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles(window_manager);
    }

    queue_pending_signal_for_its_subscribers(
        SignalType::SpaceChanged,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}
