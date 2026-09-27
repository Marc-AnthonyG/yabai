#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use objc2::msg_send;

use crate::application::model::{
    Application, application_create, application_destroy, application_focused_window,
};
use crate::debug;
use crate::display::manager::DisplayManager;
use crate::event::queue::{Event, event_loop_post};
use crate::ffi::appkit::NSRunningApplication;
use crate::ffi::carbon_core::{read_os_freq, read_os_timer};
use crate::ffi::carbon_events::GetCurrentEventTime;
use crate::ffi::core_foundation::{CFType, k_fence, take_create_rule_result};
use crate::ffi::dispatch::{NSEC_PER_SEC, dispatch_after_on_main_queue};
use crate::ffi::foundation::NSString;
use crate::ffi::skylight::SLSSpaceSetFrontPSN;
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::{
    view_add_window_node_with_insertion_point, view_remove_window_node, window_node_flush,
};
use crate::layout::view::view_is_dirty;
use crate::mouse::drag::MouseDragState;
use crate::notifications::application::{application_observe, application_unobserve};
use crate::notifications::window::{update_window_notifications, window_unobserve};
use crate::notifications::workspace::{
    WORKSPACE_CONTEXT, release_kvo_refcon_on_main_queue, remove_observer_swallowing_exception,
    workspace_application_observe_activation_policy,
    workspace_application_observe_finished_launching,
};
use crate::process::active_space::process_manager_active_space_for_psn;
use crate::process::manager::{ProcessManager, process_manager_find_process};
use crate::process::model::{Process, process_destroy};
use crate::process::running_application::{
    workspace_application_create_running_ns_application,
    workspace_application_is_finished_launching, workspace_application_is_observable,
};
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{PendingSignal, SignalContext, event_signal_push};
use crate::space::focus::space_manager_focus_space_using_gesture;
use crate::space::lookup::space_manager_cursor_space;
use crate::space::managed_space::{space_display_id, space_is_visible};
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::{CONNECTION, LAST_CMD_TAB_TIME, PENDING_WINDOW_FOCUS};
use crate::support::handles::{ProcessId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::support::layer::{LAYER_BELOW, LAYER_NORMAL};
use crate::support::log::or_null;
use crate::support::macos_version::{workspace_is_macos_sequoia, workspace_is_macos_tahoe};
use crate::window::discovery::{
    window_manager_add_application_windows, window_manager_add_existing_application_windows,
};
use crate::window::focus::window_did_receive_focus;
use crate::window::layer::window_manager_adjust_layer;
use crate::window::manager::{
    WindowManager, WindowOriginMode, window_manager_add_application,
    window_manager_add_lost_focused_event, window_manager_add_lost_front_switched_event,
    window_manager_add_managed_window, window_manager_find_application,
    window_manager_find_application_windows, window_manager_find_lost_front_switched_event,
    window_manager_find_managed_window, window_manager_find_window,
    window_manager_is_window_eligible, window_manager_remove_application,
    window_manager_remove_lost_front_switched_event, window_manager_remove_managed_window,
    window_manager_remove_window, window_manager_should_manage_window,
};
use crate::window::model::{window_destroy, window_space};
use crate::window::opacity::window_manager_set_window_opacity;
use crate::window::scratchpad::window_manager_remove_scratchpad_for_window;
use crate::window::shadow::window_manager_purify_window;

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

    if process.ns_application.load(Ordering::Acquire).is_null() {
        debug!(
            "{}: {} ({}) missing ns_application. fetching..\n",
            "EVENT_HANDLER_APPLICATION_LAUNCHED", process.name, process.process_id.0
        );
        process.ns_application.store(
            workspace_application_create_running_ns_application(&process),
            Ordering::Release,
        );

        if process.ns_application.load(Ordering::Acquire).is_null() {
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
            let ns_application = process.ns_application.load(Ordering::Acquire);
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
            let ns_application = process.ns_application.load(Ordering::Acquire);
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
