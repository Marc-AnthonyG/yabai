#![allow(deprecated)]

use core::ffi::{c_int, c_void};
use core::ptr::NonNull;
use std::io::Read;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

use objc2::msg_send;
use objc2::rc::autoreleasepool;
use objc2_core_graphics::CGMouseButton;

use crate::application::{
    Application, application_create, application_destroy, application_focused_window,
    application_is_frontmost, application_observe, application_unobserve,
};
use crate::debug;
use crate::display::{display_bounds_constrained, display_space_id};
use crate::display_manager::{
    DisplayManager, display_manager_active_display_id,
    display_manager_focus_display_with_window_at_point, display_manager_main_display_id,
    display_manager_point_display_id, display_manager_remove_label_for_display,
    display_manager_set_active_display_id,
};
use crate::ffi::accessibility::{
    AXUIElement, AXUIElementRef, ax_window_id, ax_window_pid, kAXDrawerRole, kAXSheetRole,
};
use crate::ffi::appkit::NSRunningApplication;
use crate::ffi::carbon_core::{read_os_freq, read_os_timer};
use crate::ffi::carbon_events::GetCurrentEventTime;
use crate::ffi::core_foundation::{
    CFArrayGetCount, CFDictionary, CFEqual, CFIndex, CFNumber, CFRetained, CFString, CFType,
    CGPoint, SendCFRetained, as_cftype, cfarray_borrow_value_at_index, cfdictionary_borrow_value,
    cfnumber_read_i32, cfnumber_read_u64_widening, k_dock, k_fence, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventGetIntegerValueField, CGEventGetLocation, CGPointEqualToPoint,
    CGRectContainsRect, CGRectEqualToRect, CGRectGetMidX, CGRectGetMidY,
    CGWindowListCopyWindowInfo, kCGWindowLayer, kCGWindowListOptionOnScreenOnly, kCGWindowName,
    kCGWindowOwnerName,
};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::foundation::NSString;
use crate::ffi::skylight::{
    SLSCopyAssociatedWindows, SLSOrderWindow, SLSRequestNotificationsForWindows,
    SLSSetMenuBarInsetAndAlpha, SLSSpaceGetType, SLSSpaceSetFrontPSN,
};
use crate::globals::{
    CONNECTION, LAST_CMD_TAB_TIME, LAST_GESTURE_TIME, PENDING_GESTURE, PENDING_WINDOW_FOCUS,
};
use crate::handles::{DisplayId, ProcessId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::layout::area::ax_diff;
use crate::layout::insertion::{insert_feedback_destroy, insert_feedback_show};
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::{
    WindowNodeChild, WindowNodeSplit, view_add_window_node_with_insertion_point,
    view_find_window_node, view_remove_window_node, window_node_flush,
};
use crate::layout::view::{view_destroy, view_is_dirty, view_is_invalid, view_update};
use crate::message::handle_message;
use crate::mission_control::{
    MissionControlMode, mission_control_is_active, mission_control_observe,
    mission_control_unobserve,
};
use crate::mouse::drag::{MouseDragState, MouseWindowInfo, mouse_window_info_populate};
use crate::mouse::drop::{
    MouseDropAction, mouse_determine_drop_action, mouse_drop_action_stack, mouse_drop_action_swap,
    mouse_drop_action_warp, mouse_drop_no_target, mouse_drop_try_adjust_bsp_grid,
};
use crate::mouse::tap::{MOUSE_TAP_STATE, MouseMod, MouseMode};
use crate::process_manager::{
    Process, ProcessManager, process_destroy, process_manager_active_space_for_psn,
    process_manager_find_process,
};
use crate::rule::RuleFlag;
use crate::scripting_addition::client::scripting_addition_move_window;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::exec::event_signal_flush;
use crate::signal::queue::{PendingSignal, SignalContext, event_signal_push};
use crate::space::{
    space_display_id, space_is_fullscreen, space_is_user, space_is_visible, space_window_list,
};
use crate::space_manager::{
    SpaceManager, space_manager_active_space, space_manager_cursor_space, space_manager_find_view,
    space_manager_focus_space_using_gesture, space_manager_handle_display_add,
    space_manager_is_window_on_space, space_manager_mark_spaces_invalid,
    space_manager_mark_spaces_invalid_for_display, space_manager_refresh_application_windows,
    space_manager_remove_label_for_space, space_manager_tile_window_on_space,
    space_manager_tile_window_on_space_with_insertion_point, space_manager_untile_window,
};
use crate::state::EventLoopOwnedState;
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, STACK};
use crate::support::geometry::cgrect_contains_point;
use crate::support::layer::{LAYER_BELOW, LAYER_NORMAL};
use crate::support::log::{debug_message, or_null};
use crate::support::resize_handle::ResizeHandle;
use crate::support::response::Response;
use crate::support::sockets::socket_close;
use crate::window::model::{
    WindowFlag, window_ax_can_move, window_ax_can_resize, window_ax_frame, window_ax_origin,
    window_ax_role, window_ax_subrole, window_check_flag, window_clear_flag, window_destroy,
    window_is_fullscreen, window_level, window_role, window_set_flag, window_space,
    window_sub_level, window_title,
};
use crate::window::notifications::window_unobserve;
use crate::window_manager::{
    FfmMode, WindowManager, WindowOriginMode, window_manager_add_application,
    window_manager_add_application_windows, window_manager_add_existing_application_windows,
    window_manager_add_lost_focused_event, window_manager_add_lost_front_switched_event,
    window_manager_add_managed_window, window_manager_adjust_layer, window_manager_center_mouse,
    window_manager_correct_for_mission_control_changes, window_manager_create_and_add_window,
    window_manager_find_application, window_manager_find_application_windows,
    window_manager_find_lost_focused_event, window_manager_find_lost_front_switched_event,
    window_manager_find_managed_window, window_manager_find_window,
    window_manager_find_window_at_point, window_manager_find_window_at_point_filtering_window,
    window_manager_focus_window_with_raise, window_manager_focus_window_without_raise,
    window_manager_focused_window, window_manager_handle_display_add_and_remove,
    window_manager_is_window_eligible, window_manager_move_window, window_manager_purify_window,
    window_manager_remove_application, window_manager_remove_lost_focused_event,
    window_manager_remove_lost_front_switched_event, window_manager_remove_managed_window,
    window_manager_remove_scratchpad_for_window, window_manager_remove_window,
    window_manager_resize_window_relative_internal, window_manager_set_window_opacity,
    window_manager_should_manage_window, window_manager_validate_and_check_for_windows_on_space,
    window_manager_wait_for_native_fullscreen_transition,
};
use crate::workspace::{
    WORKSPACE_CONTEXT, release_kvo_refcon_on_main_queue, remove_observer_swallowing_exception,
    workspace_application_create_running_ns_application,
    workspace_application_is_finished_launching, workspace_application_is_observable,
    workspace_application_observe_activation_policy,
    workspace_application_observe_finished_launching, workspace_is_macos_monterey,
    workspace_is_macos_sequoia, workspace_is_macos_sonoma, workspace_is_macos_tahoe,
    workspace_is_macos_ventura,
};

pub(crate) enum Event {
    ApplicationLaunched(Arc<Process>),
    ApplicationTerminated(Arc<Process>),
    ApplicationFrontSwitched(Arc<Process>),
    ApplicationVisible(ProcessId),
    ApplicationHidden(ProcessId),
    WindowCreated(SendCFRetained<AXUIElement>),
    WindowDestroyed(WindowId),
    WindowFocused(WindowId),
    WindowMoved(WindowId),
    WindowResized(WindowId),
    WindowMinimized(WindowId),
    WindowDeminimized(WindowId),
    WindowTitleChanged(WindowId),
    SlsWindowOrdered(WindowId),
    SlsWindowDestroyed(WindowId),
    SlsSpaceCreated(SpaceId),
    SlsSpaceDestroyed(SpaceId),
    SpaceChanged,
    DisplayAdded(DisplayId),
    DisplayRemoved(DisplayId),
    DisplayMoved(DisplayId),
    DisplayResized(DisplayId),
    DisplayChanged,
    MouseDown {
        event: SendCFRetained<CGEvent>,
        event_modifier: MouseMod,
    },
    MouseUp {
        event: SendCFRetained<CGEvent>,
    },
    MouseDragged {
        event: SendCFRetained<CGEvent>,
    },
    MouseMoved {
        event: SendCFRetained<CGEvent>,
        event_modifier: MouseMod,
    },
    MissionControlShowAllWindows,
    MissionControlShowFrontWindows,
    MissionControlShowDesktop,
    MissionControlEnter,
    MissionControlCheckForExit,
    MissionControlExit,
    DockDidRestart,
    MenuOpened(WindowId),
    MenuClosed,
    MenuBarHiddenChanged,
    DockDidChangePref,
    SystemWoke,
    DaemonMessage(UnixStream),
}

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

pub(crate) const NSEC_PER_SEC: u64 = 1000000000;

pub(crate) fn update_window_notifications(
    window_manager: &mut WindowManager,
    space_manager: &SpaceManager,
) {
    let mut window_list: Vec<u32> = Vec::new();

    if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
        // NOTE(asmvik): Subscribe to all windows because of window_destroyed (and ordered) notifications
        for window in window_manager.window.values() {
            window_list.push(window.id.0);
        }
    } else {
        // NOTE(asmvik): Subscribe to windows that have a feedback_border because of window_ordered notifications
        for (space_id, node_id) in window_manager.insert_feedback.values() {
            let Some(node) = space_manager
                .view
                .find(space_id)
                .and_then(|view| view.find_node(*node_id))
            else {
                continue;
            };
            window_list.push(node.window_order[0].0);
        }
    }

    let window_count = window_list.len() as c_int;
    unsafe {
        SLSRequestNotificationsForWindows(
            *CONNECTION.get().unwrap(),
            window_list.as_mut_ptr(),
            window_count,
        );
    }
}

pub(crate) fn window_did_receive_focus(
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let focused_window =
        window_manager_find_window(window_manager, window_manager.focused_window_id);
    if let Some(focused_window) = focused_window {
        if focused_window != window_id && window_space(focused_window) == window_space(window_id) {
            let normal_window_opacity = window_manager.normal_window_opacity;
            window_manager_set_window_opacity(
                window_manager,
                focused_window,
                normal_window_opacity,
            );
        }
    }

    let active_window_opacity = window_manager.active_window_opacity;
    window_manager_set_window_opacity(window_manager, window_id, active_window_opacity);

    if window_manager.focused_window_id != window_id {
        if mouse_drag_state.ffm_window_id != window_id {
            window_manager_center_mouse(window_manager, window_id);
        }

        window_manager.last_window_id = window_manager.focused_window_id;
    }

    window_manager.focused_window_id = window_id;
    let application_process_serial_number = window_manager
        .window
        .find(&window_id)
        .and_then(|window| window.application)
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
        .map(|application| application.process_serial_number);
    if let Some(application_process_serial_number) = application_process_serial_number {
        window_manager.focused_window_process_serial_number = application_process_serial_number;
    }
    mouse_drag_state.ffm_window_id = WindowId(0);

    let Some(view) = window_manager_find_managed_window(window_manager, window_id) else {
        return;
    };

    let Some(node_id) = view_find_window_node(space_manager, view, window_id) else {
        return;
    };
    let Some(view) = space_manager.view.find_mut(&view) else {
        return;
    };
    let node = view.node_mut(node_id);
    if node.window_count <= 1 {
        return;
    }

    for index in 0..node.window_count {
        if node.window_order[index as usize] != window_id {
            continue;
        }

        node.window_order.copy_within(0..index as usize, 1);
        node.window_order[0] = window_id;

        break;
    }
}

pub(crate) fn event_handler_application_launched(
    process: Arc<Process>,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if process.terminated.load(Ordering::Relaxed) {
        debug!(
            "{}: {} ({}) terminated during launch\n",
            "EVENT_HANDLER_APPLICATION_LAUNCHED", process.name, process.process_id.0
        );
        window_manager_remove_lost_front_switched_event(window_manager, process.process_id);
        return;
    }

    if process.ns_application.load(Ordering::Relaxed).is_null() {
        debug!(
            "{}: {} ({}) missing ns_application. fetching..\n",
            "EVENT_HANDLER_APPLICATION_LAUNCHED", process.name, process.process_id.0
        );
        process.ns_application.store(
            workspace_application_create_running_ns_application(&process),
            Ordering::Release,
        );

        if process.ns_application.load(Ordering::Relaxed).is_null() {
            debug!(
                "{}: {} ({}) unable to fetch ns_application..\n",
                "EVENT_HANDLER_APPLICATION_LAUNCHED", process.name, process.process_id.0
            );

            let process_serial_number = process.process_serial_number;
            dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, move || {
                let process = process_manager_find_process(&process_serial_number);
                if let Some(process) = process {
                    event_loop_post(Event::ApplicationLaunched(process));
                }
            });

            return;
        }
    }

    if !workspace_application_is_finished_launching(&process) {
        debug!(
            "{}: {} ({}) is not finished launching, subscribing to finishedLaunching changes\n",
            "EVENT_HANDLER_APPLICATION_LAUNCHED", process.name, process.process_id.0
        );
        workspace_application_observe_finished_launching(
            WORKSPACE_CONTEXT.get().unwrap(),
            &process,
        );

        //
        // NOTE(asmvik): Do this again in case of race-conditions between the previous check and key-value observation subscription.
        // Not actually sure if this can happen in practice..
        //

        if workspace_application_is_finished_launching(&process) {
            let ns_application = process.ns_application.load(Ordering::Relaxed);
            let application = unsafe { ns_application.cast::<NSRunningApplication>().as_ref() };
            if let Some(application) = application {
                let observation_info: *mut c_void =
                    unsafe { msg_send![application, observationInfo] };
                if !observation_info.is_null() {
                    let key_path = NSString::from_str("finishedLaunching");
                    if remove_observer_swallowing_exception(
                        application,
                        WORKSPACE_CONTEXT.get().unwrap(),
                        &key_path,
                        &process,
                    ) {
                        release_kvo_refcon_on_main_queue(&process);
                    }
                }
            }
        } else {
            return;
        }
    }

    if !workspace_application_is_observable(&process) {
        debug!(
            "{}: {} ({}) is not observable, subscribing to activationPolicy changes\n",
            "EVENT_HANDLER_APPLICATION_LAUNCHED", process.name, process.process_id.0
        );
        workspace_application_observe_activation_policy(WORKSPACE_CONTEXT.get().unwrap(), &process);

        //
        // NOTE(asmvik): Do this again in case of race-conditions between the previous check and key-value observation subscription.
        // Not actually sure if this can happen in practice..
        //

        if workspace_application_is_observable(&process) {
            let ns_application = process.ns_application.load(Ordering::Relaxed);
            let application = unsafe { ns_application.cast::<NSRunningApplication>().as_ref() };
            if let Some(application) = application {
                let observation_info: *mut c_void =
                    unsafe { msg_send![application, observationInfo] };
                if !observation_info.is_null() {
                    let key_path = NSString::from_str("activationPolicy");
                    if remove_observer_swallowing_exception(
                        application,
                        WORKSPACE_CONTEXT.get().unwrap(),
                        &key_path,
                        &process,
                    ) {
                        release_kvo_refcon_on_main_queue(&process);
                    }
                }
            }
        } else {
            return;
        }
    }

    //
    // NOTE(asmvik): If we somehow receive a duplicate launched event due to the subscription-timing-mess above,
    // simply ignore the event..
    //

    let application = window_manager_find_application(window_manager, process.process_id);
    let mut application: Application = match application {
        Some(_) => return,
        None => application_create(&process),
    };

    if !application_observe(&mut application) {
        let ax_retry = application.ax_retry;

        application_unobserve(&mut application);
        application_destroy(application);
        debug!(
            "{}: could not observe notifications for {} ({}) ({})\n",
            "EVENT_HANDLER_APPLICATION_LAUNCHED",
            process.name,
            process.process_id.0,
            ax_retry as i32
        );

        if ax_retry {
            let process_serial_number = process.process_serial_number;
            dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, move || {
                let process = process_manager_find_process(&process_serial_number);
                if let Some(process) = process {
                    event_loop_post(Event::ApplicationLaunched(process));
                }
            });
        }

        return;
    }

    if window_manager_find_lost_front_switched_event(window_manager, process.process_id) {
        event_loop_post(Event::ApplicationFrontSwitched(Arc::clone(&process)));
        window_manager_remove_lost_front_switched_event(window_manager, process.process_id);
    }

    debug!(
        "{}: {} ({})\n",
        "EVENT_HANDLER_APPLICATION_LAUNCHED", process.name, process.process_id.0
    );
    window_manager_add_application(window_manager, application);
    event_signal_push(
        SignalType::ApplicationLaunched,
        SignalContext::Application(process.process_id),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );

    let window_list = window_manager_add_application_windows(
        space_manager,
        window_manager,
        process.process_id,
        process_manager,
        display_manager,
        mouse_drag_state,
        mission_control_mode,
    );
    let mut prev_window_id = window_manager.focused_window_id;

    let mut space_id = SpaceId(0);
    let default_origin = window_manager.window_origin_mode == WindowOriginMode::Default;

    if !default_origin {
        if window_manager.window_origin_mode == WindowOriginMode::Focused {
            space_id = space_manager.current_space_id;
        } else
        /* if (g_window_manager.window_origin_mode == WINDOW_ORIGIN_CURSOR) */
        {
            space_id = space_manager_cursor_space();
        }
    }

    let mut view_list: Vec<SpaceId> = Vec::new();

    for index in 0..window_list.len() {
        let window_id = window_list[index];

        if window_manager_should_manage_window(window_id, window_manager)
            && window_manager_find_managed_window(window_manager, window_id).is_none()
        {
            if default_origin {
                space_id = window_space(window_id);
            }

            let view =
                space_manager_find_view(space_manager, space_id, display_manager, window_manager);
            let view_layout = space_manager.view.find(&view).map(|view| view.layout);
            if view_layout.is_some_and(|view_layout| view_layout != ViewType::Float) {
                //
                // @cleanup
                //
                // :AXBatching
                //
                // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
                // making sure that each window is only moved and resized a single time, when the final layout has been computed.
                // This is necessary to make sure that we do not call the AX API for each modification to the tree.
                //

                window_manager_adjust_layer(window_id, LAYER_BELOW, window_manager);
                view_add_window_node_with_insertion_point(
                    space_manager,
                    view,
                    window_id,
                    prev_window_id,
                    display_manager,
                    window_manager,
                );
                window_manager_add_managed_window(window_manager, window_id, space_manager, view);

                if let Some(view) = space_manager.view.find_mut(&view) {
                    view.set_flag(ViewFlag::IS_DIRTY);
                }
                view_list.push(view);

                prev_window_id = window_id;
            }
        }

        if window_manager_is_window_eligible(window_id, window_manager) {
            event_signal_push(
                SignalType::WindowCreated,
                SignalContext::Window(window_id),
                signal_event,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                signal_storage,
            );
        }
    }

    //
    // @cleanup
    //
    // :AXBatching
    //
    // NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.
    // This is necessary to make sure that we do not call the AX API for each modification to the tree.
    //

    for index in 0..view_list.len() {
        let view = view_list[index];
        if !space_is_visible(view) {
            continue;
        }
        if !view_is_dirty(space_manager, view) {
            continue;
        }

        window_node_flush(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&view) {
            view.clear_flag(ViewFlag::IS_DIRTY);
        }
    }

    if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
        update_window_notifications(window_manager, space_manager);
    }
}

pub(crate) fn event_handler_application_terminated(
    process: Arc<Process>,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let application = window_manager_find_application(window_manager, process.process_id);

    match application {
        None => {
            debug!(
                "{}: {} ({}) (not observed)\n",
                "EVENT_HANDLER_APPLICATION_TERMINATED", process.name, process.process_id.0
            );
        }
        Some(application) => {
            debug!(
                "{}: {} ({})\n",
                "EVENT_HANDLER_APPLICATION_TERMINATED", process.name, process.process_id.0
            );
            event_signal_push(
                SignalType::ApplicationTerminated,
                SignalContext::Application(application),
                signal_event,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                signal_storage,
            );
            for index in 0..window_manager.applications_to_refresh.len() {
                if application == window_manager.applications_to_refresh[index] {
                    window_manager.applications_to_refresh.swap_remove(index);
                    break;
                }
            }

            let window_list = window_manager_find_application_windows(window_manager, application);

            let mut view_list: Vec<SpaceId> = Vec::new();

            for index in 0..window_list.len() {
                let window_id = window_list[index];

                let claimed_for_destruction = window_manager
                    .window
                    .find(&window_id)
                    .is_some_and(|window| window.liveness.claim_for_destruction());
                if !claimed_for_destruction {
                    if let Some(window) = window_manager.window.find_mut(&window_id) {
                        window.application = None;
                    }
                    continue;
                }

                let view = window_manager_find_managed_window(window_manager, window_id);
                if let Some(view) = view {
                    //
                    // @cleanup
                    //
                    // :AXBatching
                    //
                    // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
                    // making sure that each window is only moved and resized a single time, when the final layout has been computed.
                    // This is necessary to make sure that we do not call the AX API for each modification to the tree.
                    //

                    view_remove_window_node(
                        space_manager,
                        view,
                        window_id,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                    window_manager_remove_managed_window(window_manager, window_id);

                    if let Some(view) = space_manager.view.find_mut(&view) {
                        view.set_flag(ViewFlag::IS_DIRTY);
                    }
                    view_list.push(view);
                }

                if mouse_drag_state.window_id == Some(window_id) {
                    mouse_drag_state.window_id = None;
                }
                if mouse_drag_state.ffm_window_id == window_id {
                    mouse_drag_state.ffm_window_id = WindowId(0);
                }

                let is_eligible = window_manager
                    .window
                    .find(&window_id)
                    .is_some_and(|window| window.is_eligible);
                if is_eligible {
                    event_signal_push(
                        SignalType::WindowDestroyed,
                        SignalContext::Window(window_id),
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    );
                }

                window_manager_remove_scratchpad_for_window(
                    window_manager,
                    window_id,
                    false,
                    process_manager,
                    display_manager,
                    space_manager,
                    mouse_drag_state,
                );
                let window = window_manager_remove_window(window_manager, window_id);
                if let Some(mut window) = window {
                    window_unobserve(&mut window, window_manager);
                    window_destroy(window);
                }
            }

            let application_record = window_manager_remove_application(window_manager, application);
            if let Some(mut application_record) = application_record {
                application_unobserve(&mut application_record);
                application_destroy(application_record);
            }

            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            for index in 0..view_list.len() {
                let view = view_list[index];
                if !space_is_visible(view) {
                    continue;
                }
                if !view_is_dirty(space_manager, view) {
                    continue;
                }

                window_node_flush(view, ROOT_NODE_ID, window_manager, space_manager);
                if let Some(view) = space_manager.view.find_mut(&view) {
                    view.clear_flag(ViewFlag::IS_DIRTY);
                }
            }

            if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
                update_window_notifications(window_manager, space_manager);
            }
        }
    }

    process_destroy(process);
}

pub(crate) fn event_handler_application_front_switched(
    process: Arc<Process>,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let application = window_manager_find_application(window_manager, process.process_id);

    let Some(application) = application else {
        window_manager_add_lost_front_switched_event(window_manager, process.process_id);
        return;
    };

    if space_manager.skip_window_focus_animation {
        let application_connection = window_manager
            .application
            .find(&application)
            .map_or(0, |application| application.connection);
        let psn_space_id = process_manager_active_space_for_psn(application_connection);

        let last_cmd_tab_time = LAST_CMD_TAB_TIME.load(Ordering::Relaxed);
        let delta_time = (read_os_timer() as f32 - last_cmd_tab_time as f32)
            * (1000.0f32 / read_os_freq() as f32);
        if delta_time > 1500.0f32 {
            let application_element_ref = window_manager
                .application
                .find(&application)
                .map(|application| application.element_ref);
            if let Some(application_element_ref) = application_element_ref {
                let mut dummy: *const CFType = core::ptr::null();
                unsafe {
                    crate::ffi::accessibility::AXUIElementCopyAttributeValue(
                        &*application_element_ref,
                        k_fence(),
                        NonNull::from(&mut dummy),
                    )
                };
                drop(unsafe { take_create_rule_result(dummy) });
            }
        }

        if PENDING_WINDOW_FOCUS.load(Ordering::Relaxed) == false {
            if psn_space_id.0 != 0 && !space_is_visible(psn_space_id) {
                unsafe {
                    SLSSpaceSetFrontPSN(
                        *CONNECTION.get().unwrap(),
                        psn_space_id.0,
                        process.process_serial_number,
                    )
                };
                space_manager_focus_space_using_gesture(
                    space_display_id(psn_space_id),
                    psn_space_id,
                    window_manager,
                );
            }
        }
    }

    let deactivated_application =
        window_manager_find_application(window_manager, process_manager.front_process_id);
    if let Some(deactivated_application) = deactivated_application {
        event_signal_push(
            SignalType::ApplicationDeactivated,
            SignalContext::Application(deactivated_application),
            signal_event,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            signal_storage,
        );
    }

    debug!(
        "{}: {} ({})\n",
        "EVENT_HANDLER_APPLICATION_FRONT_SWITCHED", process.name, process.process_id.0
    );
    event_signal_push(
        SignalType::ApplicationActivated,
        SignalContext::Application(application),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
    process_manager.switch_event_time = unsafe { GetCurrentEventTime() };
    process_manager.last_front_process_id = process_manager.front_process_id;
    process_manager.front_process_id = process.process_id;
    event_signal_push(
        SignalType::ApplicationFrontSwitched,
        SignalContext::None,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );

    for index in 0..window_manager.applications_to_refresh.len() {
        if application == window_manager.applications_to_refresh[index] {
            let application_name = window_manager
                .application
                .find(&application)
                .map(|application| Arc::clone(&application.name));
            debug!(
                "{}: {} has windows that are not yet resolved\n",
                "EVENT_HANDLER_APPLICATION_FRONT_SWITCHED",
                or_null(application_name.as_deref())
            );
            window_manager_add_existing_application_windows(
                space_manager,
                window_manager,
                application,
                index as i32,
                process_manager,
                display_manager,
                mouse_drag_state,
                mission_control_mode,
            );
            break;
        }
    }

    let application_focused_window_id = window_manager
        .application
        .find(&application)
        .map_or(WindowId(0), application_focused_window);
    if application_focused_window_id.0 == 0 {
        let focused_window =
            window_manager_find_window(window_manager, window_manager.focused_window_id);
        if let Some(focused_window) = focused_window {
            let normal_window_opacity = window_manager.normal_window_opacity;
            window_manager_set_window_opacity(
                window_manager,
                focused_window,
                normal_window_opacity,
            );
        }

        window_manager.last_window_id = window_manager.focused_window_id;
        window_manager.focused_window_id = WindowId(0);
        let application_process_serial_number = window_manager
            .application
            .find(&application)
            .map(|application| application.process_serial_number);
        if let Some(application_process_serial_number) = application_process_serial_number {
            window_manager.focused_window_process_serial_number = application_process_serial_number;
        }
        mouse_drag_state.ffm_window_id = WindowId(0);
        return;
    }

    let window = window_manager_find_window(window_manager, application_focused_window_id);
    let Some(window) = window else {
        let focused_window =
            window_manager_find_window(window_manager, window_manager.focused_window_id);
        if let Some(focused_window) = focused_window {
            let normal_window_opacity = window_manager.normal_window_opacity;
            window_manager_set_window_opacity(
                window_manager,
                focused_window,
                normal_window_opacity,
            );
        }

        window_manager_add_lost_focused_event(window_manager, application_focused_window_id);
        return;
    };

    window_did_receive_focus(window_manager, mouse_drag_state, window, space_manager);
    event_signal_push(
        SignalType::WindowFocused,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
    PENDING_WINDOW_FOCUS.store(false, Ordering::Release);
}

pub(crate) fn event_handler_application_visible(
    process_id: ProcessId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let application = window_manager_find_application(window_manager, process_id);
    let Some(application) = application else {
        return;
    };

    let application_name = window_manager
        .application
        .find(&application)
        .map(|application| Arc::clone(&application.name));
    debug!(
        "{}: {}\n",
        "EVENT_HANDLER_APPLICATION_VISIBLE",
        or_null(application_name.as_deref())
    );
    if let Some(application) = window_manager.application.find_mut(&application) {
        application.is_hidden = false;
    }

    let window_list = window_manager_find_application_windows(window_manager, application);
    let mut prev_window_id = window_manager.last_window_id;

    let mut view_list: Vec<SpaceId> = Vec::new();

    for index in 0..window_list.len() {
        let window_id = window_list[index];

        if window_manager_should_manage_window(window_id, window_manager)
            && window_manager_find_managed_window(window_manager, window_id).is_none()
        {
            let view = space_manager_find_view(
                space_manager,
                window_space(window_id),
                display_manager,
                window_manager,
            );
            let Some(view_layout) = space_manager.view.find(&view).map(|view| view.layout) else {
                continue;
            };
            if view_layout == ViewType::Float {
                continue;
            }

            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            window_manager_adjust_layer(window_id, LAYER_BELOW, window_manager);
            view_add_window_node_with_insertion_point(
                space_manager,
                view,
                window_id,
                prev_window_id,
                display_manager,
                window_manager,
            );
            window_manager_add_managed_window(window_manager, window_id, space_manager, view);

            if let Some(view) = space_manager.view.find_mut(&view) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
            view_list.push(view);

            prev_window_id = window_id;
        }
    }

    //
    // @cleanup
    //
    // :AXBatching
    //
    // NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.
    // This is necessary to make sure that we do not call the AX API for each modification to the tree.
    //

    for index in 0..view_list.len() {
        let view = view_list[index];
        if !space_is_visible(view) {
            continue;
        }
        if !view_is_dirty(space_manager, view) {
            continue;
        }

        window_node_flush(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&view) {
            view.clear_flag(ViewFlag::IS_DIRTY);
        }
    }

    event_signal_push(
        SignalType::ApplicationVisible,
        SignalContext::Application(application),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_application_hidden(
    process_id: ProcessId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let application = window_manager_find_application(window_manager, process_id);
    let Some(application) = application else {
        return;
    };

    let application_name = window_manager
        .application
        .find(&application)
        .map(|application| Arc::clone(&application.name));
    debug!(
        "{}: {}\n",
        "EVENT_HANDLER_APPLICATION_HIDDEN",
        or_null(application_name.as_deref())
    );
    if let Some(application) = window_manager.application.find_mut(&application) {
        application.is_hidden = true;
    }

    let window_list = window_manager_find_application_windows(window_manager, application);

    let mut view_list: Vec<SpaceId> = Vec::new();

    for index in 0..window_list.len() {
        let window_id = window_list[index];

        let view = window_manager_find_managed_window(window_manager, window_id);
        if let Some(view) = view {
            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            window_manager_adjust_layer(window_id, LAYER_NORMAL, window_manager);
            view_remove_window_node(
                space_manager,
                view,
                window_id,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, window_id);
            window_manager_purify_window(window_manager, window_id);

            if let Some(view) = space_manager.view.find_mut(&view) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
            view_list.push(view);
        }
    }

    //
    // @cleanup
    //
    // :AXBatching
    //
    // NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.
    // This is necessary to make sure that we do not call the AX API for each modification to the tree.
    //

    for index in 0..view_list.len() {
        let view = view_list[index];
        if !space_is_visible(view) {
            continue;
        }
        if !view_is_dirty(space_manager, view) {
            continue;
        }

        window_node_flush(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&view) {
            view.clear_flag(ViewFlag::IS_DIRTY);
        }
    }

    event_signal_push(
        SignalType::ApplicationHidden,
        SignalContext::Application(application),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_window_created(
    element_ref: SendCFRetained<AXUIElement>,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let window_id = WindowId(ax_window_id(element_ref.as_ref()));
    if window_id.0 == 0 {
        return;
    }

    let existing_window = window_manager_find_window(window_manager, window_id);
    if existing_window.is_some() {
        return;
    }

    let window_process_id =
        ProcessId(unsafe { ax_window_pid(element_ref.as_ref() as *const AXUIElement) });
    if window_process_id.0 == 0 {
        return;
    }

    let application = window_manager_find_application(window_manager, window_process_id);
    let Some(application) = application else {
        return;
    };

    let window_ref: AXUIElementRef = CFRetained::into_raw(element_ref.0).as_ptr();
    let window = window_manager_create_and_add_window(
        space_manager,
        window_manager,
        application,
        window_ref,
        window_id,
        true,
        process_manager,
        display_manager,
        mouse_drag_state,
        mission_control_mode,
    );
    let Some(window) = window else {
        return;
    };

    let mut rule_len = window_manager.rules.len() as i32;
    let mut index: i32 = 0;
    while index < rule_len {
        if RuleFlag(window_manager.rules[index as usize].flags).contains(RuleFlag::ONE_SHOT_REMOVE)
        {
            window_manager.rules.swap_remove(index as usize);
            index -= 1;
            rule_len -= 1;
        }
        index += 1;
    }

    if window_manager_should_manage_window(window, window_manager)
        && window_manager_find_managed_window(window_manager, window).is_none()
    {
        let space_id;

        if window_manager.window_origin_mode == WindowOriginMode::Default {
            space_id = window_space(window);
        } else if window_manager.window_origin_mode == WindowOriginMode::Focused {
            space_id = space_manager.current_space_id;
        } else
        /* if (g_window_manager.window_origin_mode == WINDOW_ORIGIN_CURSOR) */
        {
            space_id = space_manager_cursor_space();
        }

        let view = space_manager_tile_window_on_space(
            space_manager,
            window,
            space_id,
            display_manager,
            window_manager,
        );
        window_manager_add_managed_window(window_manager, window, space_manager, view);
    }

    if window_manager_is_window_eligible(window, window_manager) {
        event_signal_push(
            SignalType::WindowCreated,
            SignalContext::Window(window),
            signal_event,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            signal_storage,
        );
    }

    if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
        update_window_notifications(window_manager, space_manager);
    }
}

pub(crate) fn event_handler_window_destroyed(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = match window_manager_find_window(window_manager, window_id) {
        Some(window) if window.0 != 0 => window,
        _ => {
            debug!(
                "{}: window has already been destroyed, ignoring event..\n",
                "EVENT_HANDLER_WINDOW_DESTROYED"
            );
            return;
        }
    };

    let application_name = window_manager
        .window
        .find(&window)
        .and_then(|window| window.application)
        .and_then(|application| window_manager.application.find(&application))
        .map(|application| Arc::clone(&application.name));
    debug!(
        "{}: {} {}\n",
        "EVENT_HANDLER_WINDOW_DESTROYED",
        application_name.as_deref().unwrap_or("<unknown>"),
        window.0 as i32
    );

    let view = window_manager_find_managed_window(window_manager, window);
    if let Some(view) = view {
        space_manager_untile_window(
            space_manager,
            view,
            window,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, window);
    }

    if mouse_drag_state.window_id == Some(window) {
        mouse_drag_state.window_id = None;
    }
    if mouse_drag_state.ffm_window_id == window {
        mouse_drag_state.ffm_window_id = WindowId(0);
    }

    let is_eligible = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.is_eligible);
    if is_eligible {
        event_signal_push(
            SignalType::WindowDestroyed,
            SignalContext::Window(window),
            signal_event,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            signal_storage,
        );
    }

    window_manager_remove_scratchpad_for_window(
        window_manager,
        window,
        false,
        process_manager,
        display_manager,
        space_manager,
        mouse_drag_state,
    );
    let window = window_manager_remove_window(window_manager, window);
    if let Some(mut window) = window {
        window_unobserve(&mut window, window_manager);
        window_destroy(window);
    }

    if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
        update_window_notifications(window_manager, space_manager);
    }
}

pub(crate) fn event_handler_window_focused(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    PENDING_WINDOW_FOCUS.store(false, Ordering::Release);

    let window = window_manager_find_window(window_manager, window_id);
    let Some(window) = window else {
        window_manager_add_lost_focused_event(window_manager, window_id);
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_FOCUSED", window_id.0 as i32
        );
        return;
    }

    let is_minimized = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window_check_flag(window, WindowFlag::MINIMIZE));
    if is_minimized {
        window_manager_add_lost_focused_event(window_manager, window);
        return;
    }

    let application = window_manager
        .window
        .find(&window)
        .and_then(|window| window.application)
        .and_then(|application| window_manager.application.find(&application));
    let Some(application) = application else {
        return;
    };
    if !application_is_frontmost(application) {
        return;
    }
    let application_name = Arc::clone(&application.name);
    let application_process_serial_number = application.process_serial_number;

    debug!(
        "{}: {} {}\n",
        "EVENT_HANDLER_WINDOW_FOCUSED", application_name, window.0 as i32
    );

    if space_manager.skip_window_focus_animation {
        let space_id = window_space(window);
        if space_id.0 != 0 && !space_is_visible(space_id) {
            unsafe {
                SLSSpaceSetFrontPSN(
                    *CONNECTION.get().unwrap(),
                    space_id.0,
                    application_process_serial_number,
                )
            };
            space_manager_focus_space_using_gesture(
                space_display_id(space_id),
                space_id,
                window_manager,
            );
        }
    }

    window_did_receive_focus(window_manager, mouse_drag_state, window, space_manager);
    event_signal_push(
        SignalType::WindowFocused,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_window_moved(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = window_manager_find_window(window_manager, window_id);
    let Some(window) = window else {
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_MOVED", window_id.0 as i32
        );
        return;
    }

    let application = window_manager
        .window
        .find(&window)
        .and_then(|window| window.application)
        .and_then(|application| window_manager.application.find(&application));
    let application_is_hidden = application.is_some_and(|application| application.is_hidden);
    let application_name = application.map(|application| Arc::clone(&application.name));
    if application_is_hidden {
        debug!(
            "{}: {} was moved while the application is hidden, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_MOVED", window_id.0 as i32
        );
        return;
    }

    let Some(window_record) = window_manager.window.find(&window) else {
        return;
    };
    let new_origin = window_ax_origin(window_record);
    if CGPointEqualToPoint(new_origin, window_record.frame.origin) {
        debug!(
            "{}:DEBOUNCED {} {}\n",
            "EVENT_HANDLER_WINDOW_MOVED",
            or_null(application_name.as_deref()),
            window.0 as i32
        );
        return;
    }

    debug!(
        "{}: {} {}\n",
        "EVENT_HANDLER_WINDOW_MOVED",
        or_null(application_name.as_deref()),
        window.0 as i32
    );
    event_signal_push(
        SignalType::WindowMoved,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };
    let windowed_fullscreen = CGRectEqualToRect(window_record.windowed_frame, window_record.frame);
    window_record.frame.origin = new_origin;

    if !windowed_fullscreen {
        window_clear_flag(window_record, WindowFlag::WINDOWED);

        if mouse_drag_state.window_id.is_none() || mouse_drag_state.window_id != Some(window) {
            let view = window_manager_find_managed_window(window_manager, window);
            if let Some(view) = view {
                let node = view_find_window_node(space_manager, view, window);
                if let Some(node) = node
                    && space_manager.view.find(&view).is_some_and(|view| {
                        view.find_node(node).is_some_and(|window_node| {
                            (ax_diff(window_node.area.x as f64, new_origin.x)
                                || ax_diff(window_node.area.y as f64, new_origin.y))
                                && window_node.zoom.is_none_or(|zoom| {
                                    view.find_node(zoom).is_some_and(|zoom| {
                                        ax_diff(zoom.area.x as f64, new_origin.x)
                                            || ax_diff(zoom.area.y as f64, new_origin.y)
                                    })
                                })
                        })
                    })
                {
                    if space_is_visible(view) {
                        window_node_flush(view, node, window_manager, space_manager);
                    } else if let Some(view) = space_manager.view.find_mut(&view) {
                        view.set_flag(ViewFlag::IS_DIRTY);
                    }
                }
            }
        }
    }
}

pub(crate) fn event_handler_window_resized(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = window_manager_find_window(window_manager, window_id);
    let Some(window) = window else {
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_RESIZED", window_id.0 as i32
        );
        return;
    }

    let application = window_manager
        .window
        .find(&window)
        .and_then(|window| window.application)
        .and_then(|application| window_manager.application.find(&application));
    let application_is_hidden = application.is_some_and(|application| application.is_hidden);
    let application_name = application.map(|application| Arc::clone(&application.name));
    if application_is_hidden {
        debug!(
            "{}: {} was resized while the application is hidden, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_RESIZED", window_id.0 as i32
        );
        return;
    }

    let Some(window_record) = window_manager.window.find(&window) else {
        return;
    };
    let new_frame = window_ax_frame(window_record);
    if CGRectEqualToRect(new_frame, window_record.frame) {
        debug!(
            "{}:DEBOUNCED {} {}\n",
            "EVENT_HANDLER_WINDOW_RESIZED",
            or_null(application_name.as_deref()),
            window.0 as i32
        );
        return;
    }

    debug!(
        "{}: {} {}\n",
        "EVENT_HANDLER_WINDOW_RESIZED",
        or_null(application_name.as_deref()),
        window.0 as i32
    );
    event_signal_push(
        SignalType::WindowResized,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );

    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };
    let was_fullscreen = window_check_flag(window_record, WindowFlag::FULLSCREEN);

    let is_fullscreen = window_is_fullscreen(window_record);
    if is_fullscreen {
        window_set_flag(window_record, WindowFlag::FULLSCREEN);
    } else {
        window_clear_flag(window_record, WindowFlag::FULLSCREEN);
    }

    if was_fullscreen != is_fullscreen {
        if window_ax_can_move(window_record) {
            window_set_flag(window_record, WindowFlag::MOVABLE);
        } else {
            window_clear_flag(window_record, WindowFlag::MOVABLE);
        }

        if window_ax_can_resize(window_record) {
            window_set_flag(window_record, WindowFlag::RESIZABLE);
        } else {
            window_clear_flag(window_record, WindowFlag::RESIZABLE);
        }

        drop(window_record.role.take());
        let role = window_ax_role(window_record);
        window_record.role = role;

        drop(window_record.subrole.take());
        let subrole = window_ax_subrole(window_record);
        window_record.subrole = subrole;
    }

    let windowed_fullscreen = CGRectEqualToRect(window_record.windowed_frame, window_record.frame);
    window_record.frame = new_frame;

    if !was_fullscreen && is_fullscreen {
        let view = window_manager_find_managed_window(window_manager, window);
        if let Some(view) = view {
            space_manager_untile_window(
                space_manager,
                view,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, window);
            window_manager_purify_window(window_manager, window);
        }
    } else if was_fullscreen && !is_fullscreen {
        window_manager_wait_for_native_fullscreen_transition(window, window_manager);

        if window_manager_should_manage_window(window, window_manager)
            && window_manager_find_managed_window(window_manager, window).is_none()
        {
            let view = space_manager_tile_window_on_space(
                space_manager,
                window,
                window_space(window),
                display_manager,
                window_manager,
            );
            window_manager_add_managed_window(window_manager, window, space_manager, view);
        }
    } else if !was_fullscreen == !is_fullscreen {
        if mouse_drag_state.current_action == MouseMode::Move
            && mouse_drag_state.window_id == Some(window)
        {
            if let Some(mouse_window) = window_manager.window.find(&window) {
                mouse_drag_state.window_frame.size = mouse_window.frame.size;
            }
        }

        if !windowed_fullscreen {
            if let Some(window_record) = window_manager.window.find_mut(&window) {
                window_clear_flag(window_record, WindowFlag::WINDOWED);
            }

            if mouse_drag_state.window_id.is_none() || mouse_drag_state.window_id != Some(window) {
                let view = window_manager_find_managed_window(window_manager, window);
                if let Some(view) = view {
                    let node = view_find_window_node(space_manager, view, window);
                    if let Some(node) = node
                        && space_manager.view.find(&view).is_some_and(|view| {
                            view.find_node(node).is_some_and(|window_node| {
                                (ax_diff(window_node.area.x as f64, new_frame.origin.x)
                                    || ax_diff(window_node.area.y as f64, new_frame.origin.y)
                                    || ax_diff(window_node.area.width as f64, new_frame.size.width)
                                    || ax_diff(
                                        window_node.area.height as f64,
                                        new_frame.size.height,
                                    ))
                                    && window_node.zoom.is_none_or(|zoom| {
                                        view.find_node(zoom).is_some_and(|zoom| {
                                            ax_diff(zoom.area.x as f64, new_frame.origin.x)
                                                || ax_diff(zoom.area.y as f64, new_frame.origin.y)
                                                || ax_diff(
                                                    zoom.area.width as f64,
                                                    new_frame.size.width,
                                                )
                                                || ax_diff(
                                                    zoom.area.height as f64,
                                                    new_frame.size.height,
                                                )
                                        })
                                    })
                            })
                        })
                    {
                        if space_is_visible(view) {
                            window_node_flush(view, node, window_manager, space_manager);
                        } else if let Some(view) = space_manager.view.find_mut(&view) {
                            view.set_flag(ViewFlag::IS_DIRTY);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn event_handler_window_minimized(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = window_manager_find_window(window_manager, window_id);
    let Some(window) = window else {
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_MINIMIZED", window.0 as i32
        );
        return;
    }

    let application_name = window_manager
        .window
        .find(&window)
        .and_then(|window| window.application)
        .and_then(|application| window_manager.application.find(&application))
        .map(|application| Arc::clone(&application.name));
    debug!(
        "{}: {} {}\n",
        "EVENT_HANDLER_WINDOW_MINIMIZED",
        or_null(application_name.as_deref()),
        window.0 as i32
    );
    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };
    window_set_flag(window_record, WindowFlag::MINIMIZE);

    if window_ax_can_move(window_record) {
        window_set_flag(window_record, WindowFlag::MOVABLE);
    } else {
        window_clear_flag(window_record, WindowFlag::MOVABLE);
    }

    if window_ax_can_resize(window_record) {
        window_set_flag(window_record, WindowFlag::RESIZABLE);
    } else {
        window_clear_flag(window_record, WindowFlag::RESIZABLE);
    }

    drop(window_record.role.take());
    let role = window_ax_role(window_record);
    window_record.role = role;

    drop(window_record.subrole.take());
    let subrole = window_ax_subrole(window_record);
    window_record.subrole = subrole;

    if window == window_manager.last_window_id {
        window_manager.last_window_id = window_manager.focused_window_id;
    }

    let view = window_manager_find_managed_window(window_manager, window);
    if let Some(view) = view {
        space_manager_untile_window(
            space_manager,
            view,
            window,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, window);
        window_manager_purify_window(window_manager, window);
    }

    event_signal_push(
        SignalType::WindowMinimized,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_window_deminimized(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let window = window_manager_find_window(window_manager, window_id);
    let Some(window) = window else {
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_DEMINIMIZED", window.0 as i32
        );
        window_manager_remove_lost_focused_event(window_manager, window);
        return;
    }

    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };
    window_clear_flag(window_record, WindowFlag::MINIMIZE);

    if window_ax_can_move(window_record) {
        window_set_flag(window_record, WindowFlag::MOVABLE);
    } else {
        window_clear_flag(window_record, WindowFlag::MOVABLE);
    }

    if window_ax_can_resize(window_record) {
        window_set_flag(window_record, WindowFlag::RESIZABLE);
    } else {
        window_clear_flag(window_record, WindowFlag::RESIZABLE);
    }

    drop(window_record.role.take());
    let role = window_ax_role(window_record);
    window_record.role = role;

    drop(window_record.subrole.take());
    let subrole = window_ax_subrole(window_record);
    window_record.subrole = subrole;

    let window_application = window_record.application;
    let application_name = window_application
        .and_then(|application| window_manager.application.find(&application))
        .map(|application| Arc::clone(&application.name));

    let space_id = space_manager_active_space(window_manager);
    if space_manager_is_window_on_space(space_id, window) {
        debug!(
            "{}: window {} {} is deminimized on active space\n",
            "EVENT_HANDLER_WINDOW_DEMINIMIZED",
            or_null(application_name.as_deref()),
            window.0 as i32
        );
        if window_manager_should_manage_window(window, window_manager)
            && window_manager_find_managed_window(window_manager, window).is_none()
        {
            let last_window =
                window_manager_find_window(window_manager, window_manager.last_window_id);
            let last_window_application = last_window
                .and_then(|last_window| window_manager.window.find(&last_window))
                .and_then(|last_window| last_window.application);
            let insertion_point = match last_window {
                Some(last_window) if last_window_application != window_application => last_window,
                _ => WindowId(0),
            };
            let view = space_manager_tile_window_on_space_with_insertion_point(
                space_manager,
                window,
                space_id,
                insertion_point,
                display_manager,
                window_manager,
            );
            window_manager_add_managed_window(window_manager, window, space_manager, view);
        }
    } else {
        debug!(
            "{}: window {} {} is deminimized on inactive space\n",
            "EVENT_HANDLER_WINDOW_DEMINIMIZED",
            or_null(application_name.as_deref()),
            window.0 as i32
        );
    }

    if window_manager_find_lost_focused_event(window_manager, window) {
        event_loop_post(Event::WindowFocused(window));
        window_manager_remove_lost_focused_event(window_manager, window);
    }

    event_signal_push(
        SignalType::WindowDeminimized,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_window_title_changed(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let window = window_manager_find_window(window_manager, window_id);
    let Some(window) = window else {
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_WINDOW_TITLE_CHANGED", window_id.0 as i32
        );
        return;
    }

    let application_name = window_manager
        .window
        .find(&window)
        .and_then(|window| window.application)
        .and_then(|application| window_manager.application.find(&application))
        .map(|application| Arc::clone(&application.name));
    debug!(
        "{}: {} {}\n",
        "EVENT_HANDLER_WINDOW_TITLE_CHANGED",
        or_null(application_name.as_deref()),
        window.0 as i32
    );

    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };

    drop(window_record.title.take());

    let title = window_title(window_record);
    window_record.title = title;

    event_signal_push(
        SignalType::WindowTitleChanged,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
}

pub(crate) fn event_handler_sls_window_ordered(
    window_id: WindowId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    debug!(
        "{}: {}\n",
        "EVENT_HANDLER_SLS_WINDOW_ORDERED", window_id.0 as i32
    );
    let node = window_manager.insert_feedback.find(&window_id).copied();
    let feedback_window_order = node.and_then(|(space_id, node_id)| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .map(|node| {
                (
                    node.feedback_window
                        .as_ref()
                        .map_or(0, |feedback_window| feedback_window.id.0),
                    node.window_order[0].0,
                )
            })
    });
    if let Some((feedback_window_id, relative_window_id)) = feedback_window_order {
        unsafe {
            SLSOrderWindow(
                *CONNECTION.get().unwrap(),
                feedback_window_id,
                1,
                relative_window_id,
            )
        };
    }
}

pub(crate) fn event_handler_sls_window_destroyed(
    window_id: WindowId,
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
        "EVENT_HANDLER_SLS_WINDOW_DESTROYED", window_id.0 as i32
    );

    let window = window_manager_find_window(window_manager, window_id);
    let Some(window) = window else {
        return;
    };

    let claimed_for_destruction = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.claim_for_destruction());
    if !claimed_for_destruction {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_SLS_WINDOW_DESTROYED", window_id.0 as i32
        );
        return;
    }

    event_handler_window_destroyed(
        window,
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
        mouse_drag_state,
    );
}

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

pub(crate) fn event_handler_mouse_down(
    event: SendCFRetained<CGEvent>,
    event_modifier: MouseMod,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if mission_control_is_active(mission_control_mode) {
        return;
    }
    if mouse_drag_state.current_action != MouseMode::None {
        return;
    }

    let point = CGEventGetLocation(Some(event.as_ref()));
    debug!(
        "{}: {:.2}, {:.2}\n",
        "EVENT_HANDLER_MOUSE_DOWN", point.x as f64, point.y as f64
    );

    let window = window_manager_find_window_at_point(window_manager, point);
    let Some(window) = window else {
        return;
    };
    let Some(window_record) = window_manager.window.find(&window) else {
        return;
    };
    if window_check_flag(window_record, WindowFlag::FULLSCREEN) {
        return;
    }

    mouse_drag_state.window_id = Some(window);
    mouse_drag_state.window_frame = window_record.frame;
    mouse_drag_state.down_location = point;
    mouse_drag_state.direction = 0;

    let button =
        CGEventGetIntegerValueField(Some(event.as_ref()), CGEventField::MouseEventButtonNumber);

    if button == CGMouseButton::Left.0 as i64
        && MOUSE_TAP_STATE.modifier.load(Ordering::Relaxed) == event_modifier.0
    {
        mouse_drag_state.current_action =
            MouseMode::from_discriminant(MOUSE_TAP_STATE.action1.load(Ordering::Relaxed));
    } else if button == CGMouseButton::Right.0 as i64
        && MOUSE_TAP_STATE.modifier.load(Ordering::Relaxed) == event_modifier.0
    {
        mouse_drag_state.current_action =
            MouseMode::from_discriminant(MOUSE_TAP_STATE.action2.load(Ordering::Relaxed));
    }

    if mouse_drag_state.current_action == MouseMode::Resize {
        let frame_mid = CGPoint {
            x: CGRectGetMidX(mouse_drag_state.window_frame),
            y: CGRectGetMidY(mouse_drag_state.window_frame),
        };
        if point.x < frame_mid.x {
            mouse_drag_state.direction |= ResizeHandle::LEFT.0;
        }
        if point.y < frame_mid.y {
            mouse_drag_state.direction |= ResizeHandle::TOP.0;
        }
        if point.x > frame_mid.x {
            mouse_drag_state.direction |= ResizeHandle::RIGHT.0;
        }
        if point.y > frame_mid.y {
            mouse_drag_state.direction |= ResizeHandle::BOTTOM.0;
        }
    }
}

pub(crate) fn event_handler_mouse_up(
    event: SendCFRetained<CGEvent>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    'set_current_action_to_none: {
        if mission_control_is_active(mission_control_mode) {
            return;
        }

        'clear_mouse_state_window: {
            let Some(mouse_window) = mouse_drag_state.window_id else {
                break 'set_current_action_to_none;
            };

            let is_still_alive = window_manager
                .window
                .find(&mouse_window)
                .is_some_and(|window| window.liveness.is_still_alive());
            if !is_still_alive {
                debug!(
                    "{}: {} has been marked invalid by the system, ignoring event..\n",
                    "EVENT_HANDLER_MOUSE_UP", mouse_window.0 as i32
                );
                break 'clear_mouse_state_window;
            }

            let is_fullscreen = window_manager
                .window
                .find(&mouse_window)
                .is_some_and(|window| window_check_flag(window, WindowFlag::FULLSCREEN));
            if is_fullscreen {
                debug!(
                    "{}: {} is transitioning into native-fullscreen mode, ignoring event..\n",
                    "EVENT_HANDLER_MOUSE_UP", mouse_window.0 as i32
                );
                break 'clear_mouse_state_window;
            }

            let point = CGEventGetLocation(Some(event.as_ref()));
            debug!(
                "{}: {:.2}, {:.2}\n",
                "EVENT_HANDLER_MOUSE_UP", point.x as f64, point.y as f64
            );

            let source_view = window_manager_find_managed_window(window_manager, mouse_window);
            let Some(source_view) = source_view else {
                break 'clear_mouse_state_window;
            };

            let mut info = MouseWindowInfo::default();
            mouse_window_info_populate(mouse_drag_state, &mut info, window_manager);

            if info.changed_position && !info.changed_size {
                let cursor_space_id = display_space_id(display_manager_point_display_id(point));
                let destination_view = space_manager_find_view(
                    space_manager,
                    cursor_space_id,
                    display_manager,
                    window_manager,
                );

                let mut window = window_manager_find_window_at_point_filtering_window(
                    window_manager,
                    point,
                    mouse_window,
                );
                if window.is_none() {
                    window = window_manager_find_window_at_point(window_manager, point);
                }
                if window == Some(mouse_window) {
                    window = None;
                }

                let a_node = view_find_window_node(space_manager, source_view, mouse_window);
                let b_node = match window {
                    Some(window) => view_find_window_node(space_manager, destination_view, window),
                    None => None,
                };

                if let Some(a_node) = a_node
                    && let Some(b_node) = b_node
                    && let Some(window) = window
                    && (source_view, a_node) != (destination_view, b_node)
                {
                    if let Some((feedback_space_id, feedback_node_id)) =
                        mouse_drag_state.feedback_node
                    {
                        if let Some(feedback_node) = space_manager
                            .view
                            .find_mut(&feedback_space_id)
                            .and_then(|view| view.find_node_mut(feedback_node_id))
                        {
                            feedback_node.insert_direction = 0;
                        }
                        insert_feedback_destroy(
                            feedback_space_id,
                            feedback_node_id,
                            window_manager,
                            space_manager,
                        );
                        mouse_drag_state.feedback_node = None;
                    }

                    let drop_action = mouse_determine_drop_action(
                        source_view,
                        a_node,
                        window,
                        point,
                        window_manager,
                        space_manager,
                    );
                    match drop_action {
                        MouseDropAction::Stack => {
                            mouse_drop_action_stack(
                                window_manager,
                                space_manager,
                                source_view,
                                mouse_window,
                                destination_view,
                                window,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::Swap => {
                            mouse_drop_action_swap(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                            );
                        }
                        MouseDropAction::WarpTop => {
                            mouse_drop_action_warp(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::X,
                                WindowNodeChild::First,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::WarpRight => {
                            mouse_drop_action_warp(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::Y,
                                WindowNodeChild::Second,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::WarpBottom => {
                            mouse_drop_action_warp(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::X,
                                WindowNodeChild::Second,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::WarpLeft => {
                            mouse_drop_action_warp(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::Y,
                                WindowNodeChild::First,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::None => { /* silence compiler warning.. */ }
                    }
                } else if let Some(a_node) = a_node {
                    mouse_drop_no_target(
                        space_manager,
                        window_manager,
                        source_view,
                        destination_view,
                        mouse_window,
                        a_node,
                        display_manager,
                        mouse_drag_state,
                    );
                }
            } else if info.changed_position || info.changed_size {
                mouse_drop_try_adjust_bsp_grid(
                    window_manager,
                    space_manager,
                    source_view,
                    mouse_window,
                    &info,
                    display_manager,
                );
            }
        }

        mouse_drag_state.window_id = None;
    }

    mouse_drag_state.current_action = MouseMode::None;
}

pub(crate) fn event_handler_mouse_dragged(
    event: SendCFRetained<CGEvent>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if mission_control_is_active(mission_control_mode) {
        return;
    }
    let Some(mouse_window) = mouse_drag_state.window_id else {
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&mouse_window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "EVENT_HANDLER_MOUSE_DRAGGED", mouse_window.0 as i32
        );
        mouse_drag_state.window_id = None;
        mouse_drag_state.current_action = MouseMode::None;
        drop(event);
        return;
    }

    let point = CGEventGetLocation(Some(event.as_ref()));
    debug!(
        "{}: {:.2}, {:.2}\n",
        "EVENT_HANDLER_MOUSE_DRAGGED", point.x as f64, point.y as f64
    );

    if mouse_drag_state.current_action == MouseMode::Move {
        let mut new_point = CGPoint {
            x: mouse_drag_state.window_frame.origin.x
                + (point.x - mouse_drag_state.down_location.x),
            y: mouse_drag_state.window_frame.origin.y
                + (point.y - mouse_drag_state.down_location.y),
        };

        let display_id = display_manager_point_display_id(new_point);
        if display_id.0 != 0 {
            let bounds = display_bounds_constrained(display_id, false, display_manager);
            if new_point.y < bounds.origin.y {
                new_point.y = bounds.origin.y;
            }
        }

        if !scripting_addition_move_window(mouse_window, new_point.x as i32, new_point.y as i32) {
            window_manager_move_window(
                mouse_window,
                new_point.x as f32,
                new_point.y as f32,
                window_manager,
            );
        }
    } else if mouse_drag_state.current_action == MouseMode::Resize {
        let event_time = read_os_timer();
        let delta_time = (event_time as f32 - mouse_drag_state.last_moved_time as f32)
            * (1000.0f32 / read_os_freq() as f32);
        if delta_time < 67.67f32 {
            return;
        }

        let delta_x = (point.x - mouse_drag_state.down_location.x) as i32;
        let delta_y = (point.y - mouse_drag_state.down_location.y) as i32;

        let Some(mouse_window_frame) = window_manager
            .window
            .find(&mouse_window)
            .map(|window| window.frame)
        else {
            return;
        };
        window_manager_resize_window_relative_internal(
            mouse_window,
            mouse_window_frame,
            mouse_drag_state.direction as i32,
            delta_x as f32,
            delta_y as f32,
            false,
            window_manager,
        );

        mouse_drag_state.last_moved_time = event_time;
        mouse_drag_state.down_location = point;
    }

    let source_view = window_manager_find_managed_window(window_manager, mouse_window);
    let Some(source_view) = source_view else {
        return;
    };

    let mut info = MouseWindowInfo::default();
    mouse_window_info_populate(mouse_drag_state, &mut info, window_manager);

    if info.changed_position && !info.changed_size {
        let cursor_space_id = display_space_id(display_manager_point_display_id(point));
        let destination_view = space_manager_find_view(
            space_manager,
            cursor_space_id,
            display_manager,
            window_manager,
        );

        let mut window = window_manager_find_window_at_point_filtering_window(
            window_manager,
            point,
            mouse_window,
        );
        if window.is_none() {
            window = window_manager_find_window_at_point(window_manager, point);
        }
        if window == Some(mouse_window) {
            window = None;
        }

        let a_node = view_find_window_node(space_manager, source_view, mouse_window);
        let b_node = match window {
            Some(window) => view_find_window_node(space_manager, destination_view, window),
            None => None,
        };

        if let Some(a_node) = a_node
            && let Some(b_node) = b_node
            && let Some(window) = window
            && (source_view, a_node) != (destination_view, b_node)
        {
            if let Some((feedback_space_id, feedback_node_id)) = mouse_drag_state.feedback_node
                && (feedback_space_id, feedback_node_id) != (destination_view, b_node)
            {
                if let Some(feedback_node) = space_manager
                    .view
                    .find_mut(&feedback_space_id)
                    .and_then(|view| view.find_node_mut(feedback_node_id))
                {
                    feedback_node.insert_direction = 0;
                }
                insert_feedback_destroy(
                    feedback_space_id,
                    feedback_node_id,
                    window_manager,
                    space_manager,
                );
            }

            let mut insert_direction = 0;
            let drop_action = mouse_determine_drop_action(
                source_view,
                a_node,
                window,
                point,
                window_manager,
                space_manager,
            );
            match drop_action {
                MouseDropAction::Stack => {
                    insert_direction = STACK;
                }
                MouseDropAction::Swap => {
                    insert_direction = STACK;
                }
                MouseDropAction::WarpTop => {
                    insert_direction = DIR_NORTH;
                }
                MouseDropAction::WarpRight => {
                    insert_direction = DIR_EAST;
                }
                MouseDropAction::WarpBottom => {
                    insert_direction = DIR_SOUTH;
                }
                MouseDropAction::WarpLeft => {
                    insert_direction = DIR_WEST;
                }
                MouseDropAction::None => { /* silence compiler warning.. */ }
            }

            let b_node_insert_direction = space_manager
                .view
                .find(&destination_view)
                .and_then(|view| view.find_node(b_node))
                .map(|node| node.insert_direction);
            if let Some(b_node_insert_direction) = b_node_insert_direction
                && b_node_insert_direction != insert_direction
            {
                if let Some(node) = space_manager
                    .view
                    .find_mut(&destination_view)
                    .and_then(|view| view.find_node_mut(b_node))
                {
                    node.insert_direction = insert_direction;
                }
                insert_feedback_show(destination_view, b_node, window_manager, space_manager);
                mouse_drag_state.feedback_node = Some((destination_view, b_node));
            }
        } else if b_node.is_none() {
            if let Some((feedback_space_id, feedback_node_id)) = mouse_drag_state.feedback_node {
                if let Some(feedback_node) = space_manager
                    .view
                    .find_mut(&feedback_space_id)
                    .and_then(|view| view.find_node_mut(feedback_node_id))
                {
                    feedback_node.insert_direction = 0;
                }
                insert_feedback_destroy(
                    feedback_space_id,
                    feedback_node_id,
                    window_manager,
                    space_manager,
                );
                mouse_drag_state.feedback_node = None;
            }
        }
    }
}

pub(crate) fn event_handler_mouse_moved(
    event: SendCFRetained<CGEvent>,
    _event_modifier: MouseMod,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if window_manager.ffm_mode == FfmMode::Disabled {
        return;
    }
    if mission_control_is_active(mission_control_mode) {
        return;
    }
    if mouse_drag_state.ffm_window_id.0 != 0 {
        return;
    }

    if PENDING_GESTURE.load(Ordering::Relaxed) {
        return;
    }
    let last_gesture_time = LAST_GESTURE_TIME.load(Ordering::Relaxed);
    let delta_time =
        (read_os_timer() as f32 - last_gesture_time as f32) * (1000.0f32 / read_os_freq() as f32);
    if delta_time < 1250.0f32 {
        return;
    }

    let point = CGEventGetLocation(Some(event.as_ref()));
    let window = window_manager_find_window_at_point(window_manager, point);

    if let Some(mut window) = window {
        if window == window_manager.focused_window_id {
            return;
        }
        if !window_manager_is_window_eligible(window, window_manager) {
            return;
        }

        if window_manager.ffm_mode == FfmMode::Autofocus {
            //
            // NOTE(asmvik): Look for a window with role AXSheet or AXDrawer
            // and forward focus to it because we are not allowed to focus the main
            // window in these cases.
            //

            let window_list = unsafe {
                take_create_rule_result(SLSCopyAssociatedWindows(
                    *CONNECTION.get().unwrap(),
                    window.0,
                ))
            };
            if let Some(window_list) = window_list {
                let window_count = CFArrayGetCount(&window_list) as i32;

                for index in 0..window_count {
                    let child_number = unsafe {
                        cfarray_borrow_value_at_index::<CFNumber>(&window_list, index as CFIndex)
                    };
                    let child_window_id =
                        WindowId(child_number.map_or(0, cfnumber_read_i32) as u32);
                    let child = window_manager_find_window(window_manager, child_window_id);
                    let Some(child) = child else {
                        continue;
                    };

                    let Some(child_record) = window_manager.window.find(&child) else {
                        continue;
                    };
                    let role = window_role(child_record);
                    let Some(role) = role else {
                        continue;
                    };

                    let valid = CFEqual(Some(as_cftype(role)), Some(as_cftype(kAXSheetRole())))
                        || CFEqual(Some(as_cftype(role)), Some(as_cftype(kAXDrawerRole())));

                    if valid {
                        window = child;
                        break;
                    }
                }

                drop(window_list);
            }

            let window_process_serial_number = window_manager
                .window
                .find(&window)
                .and_then(|window| window.application)
                .and_then(|application| window_manager.application.find(&application))
                .map(|application| application.process_serial_number);
            let Some(window_process_serial_number) = window_process_serial_number else {
                return;
            };
            window_manager_focus_window_without_raise(
                &window_process_serial_number,
                window,
                window_manager,
            );
            mouse_drag_state.ffm_window_id = window;
        } else if window_manager.ffm_mode == FfmMode::Autoraise {
            //
            // NOTE(asmvik): If any **floating** window would be fully occluded by
            // autoraising the window below the cursor we do not actually perform the
            // focus change, as it is likely that the user is trying to reach for the
            // smaller window that sits on top of the window we would otherwise raise.
            //

            let mut occludes_window = false;

            let window_list =
                space_window_list(space_manager.current_space_id, false, window_manager);

            if let Some(window_list) = window_list {
                let window_count = window_list.len() as i32;
                for index in 0..window_count {
                    let window_id = window_list[index as usize];
                    if window_id == window {
                        break;
                    }

                    let sub_window = window_manager_find_window(window_manager, window_id);
                    let Some(sub_window) = sub_window else {
                        continue;
                    };
                    let Some(sub_window_record) = window_manager.window.find(&sub_window) else {
                        continue;
                    };

                    if !window_check_flag(sub_window_record, WindowFlag::FLOAT) {
                        continue;
                    }
                    if window_level(window) != window_level(sub_window) {
                        continue;
                    }
                    if window_sub_level(window) != window_sub_level(sub_window) {
                        continue;
                    }

                    let window_frame = window_manager
                        .window
                        .find(&window)
                        .map(|window| window.frame);
                    if let Some(window_frame) = window_frame
                        && CGRectContainsRect(window_frame, sub_window_record.frame)
                    {
                        occludes_window = true;
                        break;
                    }
                }
            }

            if !occludes_window {
                let Some(window_record) = window_manager.window.find(&window) else {
                    return;
                };
                let window_element_ref = window_record.element_ref;
                let window_process_serial_number = window_record
                    .application
                    .and_then(|application| window_manager.application.find(&application))
                    .map(|application| application.process_serial_number);
                let Some(window_process_serial_number) = window_process_serial_number else {
                    return;
                };
                window_manager_focus_window_with_raise(
                    &window_process_serial_number,
                    window,
                    window_element_ref,
                );
                mouse_drag_state.ffm_window_id = window;
            }
        }
    } else {
        let cursor_display_id = display_manager_point_display_id(point);
        if display_manager.current_display_id == cursor_display_id {
            return;
        }

        let bounds = display_bounds_constrained(cursor_display_id, false, display_manager);
        if !cgrect_contains_point(bounds, point) {
            return;
        }

        let window_id = display_manager_focus_display_with_window_at_point(point, window_manager);
        if window_id.0 == 0 {
            display_manager_set_active_display_id(cursor_display_id);
        }
        mouse_drag_state.ffm_window_id = window_id;
    }
}

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

pub(crate) fn event_handler_menu_opened(
    _window_id: WindowId,
    window_manager: &mut WindowManager,
    focus_follows_mouse_suspended_value: &mut FfmMode,
    is_menu_open: &mut i32,
) {
    debug!("{}\n", "EVENT_HANDLER_MENU_OPENED");
    *is_menu_open += 1;

    if *is_menu_open == 1 {
        *focus_follows_mouse_suspended_value = window_manager.ffm_mode;
        window_manager.ffm_mode = FfmMode::Disabled;
    }
}

pub(crate) fn event_handler_menu_closed(
    window_manager: &mut WindowManager,
    focus_follows_mouse_suspended_value: &mut FfmMode,
    is_menu_open: &mut i32,
) {
    debug!("{}\n", "EVENT_HANDLER_MENU_CLOSED");
    *is_menu_open -= 1;

    if *is_menu_open == 0 {
        window_manager.ffm_mode = *focus_follows_mouse_suspended_value;
    } else if *is_menu_open < 0 {
        *is_menu_open = 0;
    }
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

pub(crate) fn event_handler_daemon_message(
    stream: UnixStream,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut stream = stream;
    let mut bytes_read: i32 = 0;
    let mut bytes_to_read_bytes = [0u8; size_of::<c_int>()];

    let prefix_read = stream.read(&mut bytes_to_read_bytes);
    if prefix_read.is_ok_and(|count| count == size_of::<c_int>()) {
        let bytes_to_read = c_int::from_ne_bytes(bytes_to_read_bytes);

        if bytes_to_read > 0 {
            let mut message = vec![0u8; bytes_to_read as usize + 2];

            loop {
                let current_read =
                    match stream.read(&mut message[bytes_read as usize..bytes_to_read as usize]) {
                        Ok(count) => count as i32,
                        Err(_) => -1,
                    };
                if current_read <= 0 {
                    break;
                }

                bytes_read += current_read;
                if !(bytes_read < bytes_to_read) {
                    break;
                }
            }

            if bytes_read == bytes_to_read {
                let mut response = Response::to_client(stream);
                debug_message(
                    "EVENT_HANDLER_DAEMON_MESSAGE",
                    &String::from_utf8_lossy(&message),
                );
                handle_message(
                    &mut response,
                    &mut message,
                    signal_event,
                    process_manager,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                    mission_control_mode,
                );

                drop(response);

                return;
            }
        }
    }

    socket_close(stream.into_raw_fd());
}

pub(crate) fn event_loop_run(
    event_receiver: Receiver<Event>,
    mut event_loop_owned_state: EventLoopOwnedState,
) {
    let EventLoopOwnedState {
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
        mouse_drag_state,
        mission_control_mode,
        focus_follows_mouse_suspended_value,
        is_menu_open,
    } = &mut event_loop_owned_state;

    while let Ok(first_event_of_batch) = event_receiver.recv() {
        autoreleasepool(|_| {
            let mut next = first_event_of_batch;

            loop {
                match next {
                    Event::ApplicationLaunched(process) => event_handler_application_launched(
                        process,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::ApplicationTerminated(process) => event_handler_application_terminated(
                        process,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::ApplicationFrontSwitched(process) => {
                        event_handler_application_front_switched(
                            process,
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mouse_drag_state,
                            mission_control_mode,
                        )
                    }
                    Event::ApplicationVisible(process_id) => event_handler_application_visible(
                        process_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::ApplicationHidden(process_id) => event_handler_application_hidden(
                        process_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowCreated(element_ref) => event_handler_window_created(
                        element_ref,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::WindowDestroyed(window_id) => event_handler_window_destroyed(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowFocused(window_id) => event_handler_window_focused(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowMoved(window_id) => event_handler_window_moved(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowResized(window_id) => event_handler_window_resized(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowMinimized(window_id) => event_handler_window_minimized(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowDeminimized(window_id) => event_handler_window_deminimized(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::WindowTitleChanged(window_id) => event_handler_window_title_changed(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SlsWindowOrdered(window_id) => {
                        event_handler_sls_window_ordered(window_id, window_manager, space_manager)
                    }
                    Event::SlsWindowDestroyed(window_id) => event_handler_sls_window_destroyed(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::SlsSpaceCreated(space_id) => event_handler_sls_space_created(
                        space_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SlsSpaceDestroyed(space_id) => event_handler_sls_space_destroyed(
                        space_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::SpaceChanged => event_handler_space_changed(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::DisplayAdded(display_id) => event_handler_display_added(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::DisplayRemoved(display_id) => event_handler_display_removed(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::DisplayMoved(display_id) => event_handler_display_moved(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DisplayResized(display_id) => event_handler_display_resized(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DisplayChanged => event_handler_display_changed(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseDown {
                        event,
                        event_modifier,
                    } => event_handler_mouse_down(
                        event,
                        event_modifier,
                        window_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseUp { event } => event_handler_mouse_up(
                        event,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseDragged { event } => event_handler_mouse_dragged(
                        event,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseMoved {
                        event,
                        event_modifier,
                    } => event_handler_mouse_moved(
                        event,
                        event_modifier,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MissionControlShowAllWindows => {
                        event_handler_mission_control_show_all_windows(
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mission_control_mode,
                        )
                    }
                    Event::MissionControlShowFrontWindows => {
                        event_handler_mission_control_show_front_windows(
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mission_control_mode,
                        )
                    }
                    Event::MissionControlShowDesktop => event_handler_mission_control_show_desktop(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mission_control_mode,
                    ),
                    Event::MissionControlEnter => event_handler_mission_control_enter(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mission_control_mode,
                    ),
                    Event::MissionControlCheckForExit => {
                        event_handler_mission_control_check_for_exit(mission_control_mode)
                    }
                    Event::MissionControlExit => event_handler_mission_control_exit(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::DockDidRestart => event_handler_dock_did_restart(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::MenuOpened(window_id) => event_handler_menu_opened(
                        window_id,
                        window_manager,
                        focus_follows_mouse_suspended_value,
                        is_menu_open,
                    ),
                    Event::MenuClosed => event_handler_menu_closed(
                        window_manager,
                        focus_follows_mouse_suspended_value,
                        is_menu_open,
                    ),
                    Event::MenuBarHiddenChanged => event_handler_menu_bar_hidden_changed(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DockDidChangePref => event_handler_dock_did_change_pref(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SystemWoke => event_handler_system_woke(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DaemonMessage(stream) => event_handler_daemon_message(
                        stream,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                }

                event_signal_flush(signal_event, signal_storage);

                match event_receiver.try_recv() {
                    Ok(next_event) => next = next_event,
                    Err(_) => break,
                }
            }
        });
    }
}

pub(crate) fn event_loop_post(event: Event) {
    if let Some(event_sender) = EVENT_SENDER.get() {
        let _ = event_sender.send(event);
    }
}

pub(crate) fn event_loop_begin() -> Receiver<Event> {
    let (event_sender, event_receiver) = channel::<Event>();
    let _ = EVENT_SENDER.set(event_sender);

    event_receiver
}
