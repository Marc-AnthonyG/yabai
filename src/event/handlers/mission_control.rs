#![allow(deprecated)]

use crate::debug;
use crate::display::manager::DisplayManager;
use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::core_foundation::{
    CFArrayGetCount, CFDictionary, CFEqual, CFIndex, CFNumber, CFString, as_cftype,
    cfarray_borrow_value_at_index, cfdictionary_borrow_value, cfnumber_read_u64_widening,
    dock_window_owner_name,
};
use crate::ffi::core_graphics::{
    CGWindowListCopyWindowInfo, kCGWindowLayer, kCGWindowListOptionOnScreenOnly, kCGWindowName,
    kCGWindowOwnerName,
};
use crate::ffi::dispatch::{NSEC_PER_SEC, dispatch_after_on_main_queue};
use crate::ffi::skylight::SLSSetMenuBarInsetAndAlpha;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{
    PendingSignal, SignalContext, queue_pending_signal_for_its_subscribers,
};
use crate::space::managed_space::is_native_fullscreen_space;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::window::manager::WindowManager;
use crate::window::space_reconciliation::reconcile_every_view_after_mission_control_changes;

pub(crate) fn handle_mission_control_show_all_windows_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "handle_mission_control_show_all_windows_event");
    *mission_control_mode = MissionControlMode::ShowAllWindows;
    queue_pending_signal_for_its_subscribers(
        SignalType::MissionControlEnter,
        SignalContext::MissionControl(*mission_control_mode),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_mission_control_show_front_windows_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "handle_mission_control_show_front_windows_event");
    *mission_control_mode = MissionControlMode::ShowFrontWindows;
    queue_pending_signal_for_its_subscribers(
        SignalType::MissionControlEnter,
        SignalContext::MissionControl(*mission_control_mode),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_mission_control_show_desktop_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "handle_mission_control_show_desktop_event");
    *mission_control_mode = MissionControlMode::ShowDesktop;
    queue_pending_signal_for_its_subscribers(
        SignalType::MissionControlEnter,
        SignalContext::MissionControl(*mission_control_mode),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_mission_control_enter_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "handle_mission_control_enter_event");
    *mission_control_mode = MissionControlMode::Show;

    dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, || {
        post_event_to_event_loop(Event::MissionControlCheckForExit);
    });

    queue_pending_signal_for_its_subscribers(
        SignalType::MissionControlEnter,
        SignalContext::MissionControl(*mission_control_mode),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn handle_mission_control_check_for_exit_event(
    mission_control_mode: &mut MissionControlMode,
) {
    if !is_mission_control_active(mission_control_mode) {
        return;
    }

    let window_list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, 0);
    let window_count = window_list
        .as_deref()
        .map_or(0, |window_list| CFArrayGetCount(window_list) as i32);
    let mut found = false;

    for index in 0..window_count {
        let dictionary = window_list.as_deref().and_then(|window_list| unsafe {
            cfarray_borrow_value_at_index::<CFDictionary>(window_list, index as CFIndex)
        });
        let Some(dictionary) = dictionary else {
            continue;
        };

        let name = unsafe { cfdictionary_borrow_value::<CFString>(dictionary, kCGWindowName) };
        if name.is_some() {
            continue;
        }

        let owner =
            unsafe { cfdictionary_borrow_value::<CFString>(dictionary, kCGWindowOwnerName) };
        let Some(owner) = owner else {
            continue;
        };

        let layer_ref =
            unsafe { cfdictionary_borrow_value::<CFNumber>(dictionary, kCGWindowLayer) };
        let Some(layer_ref) = layer_ref else {
            continue;
        };

        let layer: u64 = cfnumber_read_u64_widening(layer_ref);
        if layer != 18 {
            continue;
        }

        if CFEqual(
            Some(as_cftype(dock_window_owner_name())),
            Some(as_cftype(owner)),
        ) {
            found = true;
            break;
        }
    }

    if found {
        dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, || {
            post_event_to_event_loop(Event::MissionControlCheckForExit);
        });
    } else {
        dispatch_after_on_main_queue(0.0f32 as i64, || {
            post_event_to_event_loop(Event::MissionControlExit);
        });
    }

    drop(window_list);
}

pub(crate) fn handle_mission_control_exit_event(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "handle_mission_control_exit_event");

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

    if *mission_control_mode == MissionControlMode::Show
        || *mission_control_mode == MissionControlMode::ShowAllWindows
    {
        reconcile_every_view_after_mission_control_changes(
            space_manager,
            window_manager,
            display_manager,
            mouse_drag_state,
        );
    }

    queue_pending_signal_for_its_subscribers(
        SignalType::MissionControlExit,
        SignalContext::MissionControl(*mission_control_mode),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
    *mission_control_mode = MissionControlMode::Inactive;
}
