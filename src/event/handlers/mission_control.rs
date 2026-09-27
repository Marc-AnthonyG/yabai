#![allow(deprecated)]

use crate::debug;
use crate::display::manager::DisplayManager;
use crate::event::queue::{Event, event_loop_post};
use crate::ffi::core_foundation::{
    CFArrayGetCount, CFDictionary, CFEqual, CFIndex, CFNumber, CFString, as_cftype,
    cfarray_borrow_value_at_index, cfdictionary_borrow_value, cfnumber_read_u64_widening, k_dock,
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
use crate::signal::queue::{PendingSignal, SignalContext, event_signal_push};
use crate::space::managed_space::space_is_fullscreen;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::{MissionControlMode, mission_control_is_active};
use crate::state::process_wide::CONNECTION;
use crate::window::manager::WindowManager;
use crate::window::space_reconciliation::window_manager_correct_for_mission_control_changes;

pub(crate) fn event_handler_mission_control_show_all_windows(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "EVENT_HANDLER_MISSION_CONTROL_SHOW_ALL_WINDOWS");
    *mission_control_mode = MissionControlMode::ShowAllWindows;
    event_signal_push(
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

pub(crate) fn event_handler_mission_control_show_front_windows(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "EVENT_HANDLER_MISSION_CONTROL_SHOW_FRONT_WINDOWS");
    *mission_control_mode = MissionControlMode::ShowFrontWindows;
    event_signal_push(
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

pub(crate) fn event_handler_mission_control_show_desktop(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "EVENT_HANDLER_MISSION_CONTROL_SHOW_DESKTOP");
    *mission_control_mode = MissionControlMode::ShowDesktop;
    event_signal_push(
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

pub(crate) fn event_handler_mission_control_enter(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "EVENT_HANDLER_MISSION_CONTROL_ENTER");
    *mission_control_mode = MissionControlMode::Show;

    dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, || {
        event_loop_post(Event::MissionControlCheckForExit);
    });

    event_signal_push(
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

pub(crate) fn event_handler_mission_control_check_for_exit(
    mission_control_mode: &mut MissionControlMode,
) {
    if !mission_control_is_active(mission_control_mode) {
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

        if CFEqual(Some(as_cftype(k_dock())), Some(as_cftype(owner))) {
            found = true;
            break;
        }
    }

    if found {
        dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, || {
            event_loop_post(Event::MissionControlCheckForExit);
        });
    } else {
        dispatch_after_on_main_queue(0.0f32 as i64, || {
            event_loop_post(Event::MissionControlExit);
        });
    }

    drop(window_list);
}

pub(crate) fn event_handler_mission_control_exit(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    debug!("{}:\n", "EVENT_HANDLER_MISSION_CONTROL_EXIT");

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

    if *mission_control_mode == MissionControlMode::Show
        || *mission_control_mode == MissionControlMode::ShowAllWindows
    {
        window_manager_correct_for_mission_control_changes(
            space_manager,
            window_manager,
            display_manager,
            mouse_drag_state,
        );
    }

    event_signal_push(
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
