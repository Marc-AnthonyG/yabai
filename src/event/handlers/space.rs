use crate::debug;
use crate::display::manager::DisplayManager;
use crate::ffi::skylight::{SLSSetMenuBarInsetAndAlpha, SLSSpaceGetType};
use crate::layout::settings::ViewFlag;
use crate::layout::tree::window_node_flush;
use crate::layout::view::{view_destroy, view_is_dirty, view_is_invalid, view_update};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{PendingSignal, SignalContext, event_signal_push};
use crate::space::focus::space_manager_active_space;
use crate::space::labels::space_manager_remove_label_for_space;
use crate::space::managed_space::{space_is_fullscreen, space_is_user};
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::state::mission_control_mode::{MissionControlMode, mission_control_is_active};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{ROOT_NODE_ID, SpaceId};
use crate::window::discovery::space_manager_refresh_application_windows;
use crate::window::focus::{window_did_receive_focus, window_manager_focused_window};
use crate::window::manager::{
    WindowManager, window_manager_find_lost_focused_event, window_manager_remove_lost_focused_event,
};
use crate::window::space_reconciliation::window_manager_validate_and_check_for_windows_on_space;

pub(crate) fn event_handler_sls_space_created(
    space_id: SpaceId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let space_type = unsafe { SLSSpaceGetType(*CONNECTION.get().unwrap(), space_id.0) };

    if space_type == 0 || space_type == 4 {
        debug!(
            "{}: {}, {}\n",
            "EVENT_HANDLER_SLS_SPACE_CREATED", space_id.0 as i64, space_type
        );
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
        event_signal_push(
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

pub(crate) fn event_handler_sls_space_destroyed(
    space_id: SpaceId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    if space_manager.view.find(&space_id).is_some() {
        debug!(
            "{}: {}\n",
            "EVENT_HANDLER_SLS_SPACE_DESTROYED", space_id.0 as i64
        );
        space_manager_remove_label_for_space(space_manager, space_id);
        view_destroy(space_manager, space_id, window_manager, mouse_drag_state);
        drop(space_manager.view.remove(&space_id));
        event_signal_push(
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

pub(crate) fn event_handler_space_changed(
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
    space_manager.current_space_id = space_manager_active_space(window_manager);

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
        "{}: {}\n",
        "EVENT_HANDLER_SPACE_CHANGED", space_manager.current_space_id.0 as i64
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
