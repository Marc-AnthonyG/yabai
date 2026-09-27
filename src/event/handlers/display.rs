use crate::debug;
use crate::display::identity::{
    display_manager_active_display_id, display_manager_main_display_id,
};
use crate::display::labels::display_manager_remove_label_for_display;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_id;
use crate::ffi::skylight::SLSSetMenuBarInsetAndAlpha;
use crate::layout::settings::ViewFlag;
use crate::layout::tree::window_node_flush;
use crate::layout::view::{view_is_dirty, view_is_invalid, view_update};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{PendingSignal, SignalContext, event_signal_push};
use crate::space::managed_space::{space_display_id, space_is_fullscreen, space_is_user};
use crate::space::manager::{
    SpaceManager, space_manager_find_view, space_manager_handle_display_add,
    space_manager_mark_spaces_invalid, space_manager_mark_spaces_invalid_for_display,
};
use crate::state::mission_control_mode::{MissionControlMode, mission_control_is_active};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{DisplayId, ROOT_NODE_ID};
use crate::window::discovery::space_manager_refresh_application_windows;
use crate::window::focus::{window_did_receive_focus, window_manager_focused_window};
use crate::window::manager::{
    WindowManager, window_manager_find_lost_focused_event, window_manager_remove_lost_focused_event,
};
use crate::window::space_reconciliation::{
    window_manager_handle_display_add_and_remove,
    window_manager_validate_and_check_for_windows_on_space,
};

pub(crate) fn event_handler_display_changed(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let new_display_id = display_manager_active_display_id();
    if display_manager.current_display_id == new_display_id {
        debug!(
            "{}: newly activated display {} was already active ({})! ignoring event..\n",
            "EVENT_HANDLER_DISPLAY_CHANGED",
            display_manager.current_display_id.0 as i32,
            new_display_id.0 as i32
        );
        return;
    }

    display_manager.last_display_id = display_manager.current_display_id;
    display_manager.current_display_id = new_display_id;

    space_manager.last_space_id = space_manager.current_space_id;
    space_manager.current_space_id = display_space_id(display_manager.current_display_id);

    let expected_display_id = space_display_id(space_manager.current_space_id);
    if display_manager.current_display_id != expected_display_id {
        debug!(
            "{}: {} {} did not match {}! ignoring event..\n",
            "EVENT_HANDLER_DISPLAY_CHANGED",
            display_manager.current_display_id.0 as i32,
            space_manager.current_space_id.0 as i64,
            expected_display_id.0 as i32
        );
        return;
    }

    if window_manager.menubar_opacity != 1.0f32 {
        let alpha = if space_is_fullscreen(space_manager.current_space_id) {
            1.0f32
        } else {
            window_manager.menubar_opacity
        };
        unsafe {
            SLSSetMenuBarInsetAndAlpha(*CONNECTION.get().unwrap(), 0 as f64, 1 as f64, alpha)
        };
    }

    debug!(
        "{}: {} {}\n",
        "EVENT_HANDLER_DISPLAY_CHANGED",
        display_manager.current_display_id.0 as i32,
        space_manager.current_space_id.0 as i64
    );
    let view = space_manager_find_view(
        space_manager,
        space_manager.current_space_id,
        display_manager,
        window_manager,
    );

    if space_manager_refresh_application_windows(
        space_manager,
        process_manager,
        display_manager,
        window_manager,
        mouse_drag_state,
        mission_control_mode,
    ) {
        let focused_window = window_manager_focused_window(window_manager);
        if let Some(focused_window) = focused_window
            && window_manager_find_lost_focused_event(window_manager, focused_window)
        {
            window_did_receive_focus(
                window_manager,
                mouse_drag_state,
                focused_window,
                space_manager,
            );
            window_manager_remove_lost_focused_event(window_manager, focused_window);
        }
    }

    if !mission_control_is_active(mission_control_mode)
        && space_is_user(space_manager.current_space_id)
    {
        window_manager_validate_and_check_for_windows_on_space(
            space_manager,
            window_manager,
            space_manager.current_space_id,
            display_manager,
            mouse_drag_state,
        );

        if view_is_invalid(space_manager, view) {
            view_update(space_manager, view, display_manager, window_manager);
        }

        if view_is_dirty(space_manager, view) {
            window_node_flush(view, ROOT_NODE_ID, window_manager, space_manager);
            if let Some(view) = space_manager.view.find_mut(&view) {
                view.clear_flag(ViewFlag::IS_DIRTY);
            }
        }
    }

    event_signal_push(
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

pub(crate) fn event_handler_display_added(
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
        "EVENT_HANDLER_DISPLAY_ADDED", display_id.0 as i32
    );
    space_manager_handle_display_add(
        space_manager,
        display_id,
        window_manager,
        mouse_drag_state,
    );
    window_manager_handle_display_add_and_remove(
        space_manager,
        window_manager,
        display_id,
        display_manager,
        mouse_drag_state,
    );
    event_signal_push(
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

pub(crate) fn event_handler_display_removed(
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
        "EVENT_HANDLER_DISPLAY_REMOVED", display_id.0 as i32
    );
    display_manager_remove_label_for_display(display_manager, display_id);
    window_manager_handle_display_add_and_remove(
        space_manager,
        window_manager,
        display_manager_main_display_id(),
        display_manager,
        mouse_drag_state,
    );
    event_signal_push(
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

pub(crate) fn event_handler_display_moved(
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
        "EVENT_HANDLER_DISPLAY_MOVED", display_id.0 as i32
    );
    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
    event_signal_push(
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

pub(crate) fn event_handler_display_resized(
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
        "EVENT_HANDLER_DISPLAY_RESIZED", display_id.0 as i32
    );
    space_manager_mark_spaces_invalid_for_display(
        space_manager,
        display_id,
        display_manager,
        window_manager,
    );
    event_signal_push(
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
