#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use objc2::msg_send;

use crate::application::{
    Application, create_application_for_process,
    destroy_application_releasing_its_accessibility_element, read_focused_window_of_application,
};
use crate::debug;
use crate::display::manager::DisplayManager;
use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::appkit::NSRunningApplication;
use crate::ffi::carbon_core::{read_system_clock_in_nanoseconds, system_clock_ticks_per_second};
use crate::ffi::carbon_events::GetCurrentEventTime;
use crate::ffi::core_foundation::{
    CFType, accessibility_fence_attribute_name, take_create_rule_result,
};
use crate::ffi::dispatch::{NSEC_PER_SEC, dispatch_after_on_main_queue};
use crate::ffi::foundation::NSString;
use crate::ffi::skylight::SLSSpaceSetFrontPSN;
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::tree::{
    add_window_to_view_tree_preferring_insertion_point, move_windows_below_node_into_their_areas,
    remove_window_from_view_tree,
};
use crate::layout::view::has_view_windows_awaiting_their_areas;
use crate::mouse::drag::MouseDragState;
use crate::notifications::application::{
    start_observing_application_notifications_reporting_whether_all_registered,
    stop_observing_application_notifications,
};
use crate::notifications::window::{
    request_skylight_notifications_for_windows_that_need_them, stop_observing_window_notifications,
};
use crate::notifications::workspace::{
    WORKSPACE_CONTEXT, release_key_value_observation_process_reference_on_main_queue,
    remove_observer_swallowing_exception, start_observing_application_activation_policy,
    start_observing_application_finished_launching,
};
use crate::process::active_space::query_space_of_first_window_owned_by_connection;
use crate::process::manager::{ProcessManager, process_with_process_serial_number};
use crate::process::model::{Process, destroy_process_releasing_its_running_application};
use crate::process::running_application::{
    copy_running_application_of_process, has_process_finished_launching,
    is_process_observable_refreshing_its_activation_policy,
};
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{
    PendingSignal, SignalContext, queue_pending_signal_for_its_subscribers,
};
use crate::space::focus::focus_space_with_synthesized_dock_swipes;
use crate::space::lookup::query_current_space_of_display_under_the_cursor;
use crate::space::managed_space::{is_space_visible_on_its_display, query_display_holding_space};
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::{
    LAST_COMMAND_TAB_TIME, SKYLIGHT_CONNECTION_ID, WINDOW_FOCUS_NOTIFICATION_IS_PENDING,
};
use crate::support::handles::{ProcessId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::support::layer::{LAYER_BELOW, LAYER_NORMAL};
use crate::support::log::text_or_printf_null_placeholder;
use crate::support::macos_version::{is_running_on_macos_sequoia, is_running_on_macos_tahoe};
use crate::window::discovery::{
    track_existing_windows_of_application_including_those_on_inactive_spaces,
    track_untracked_windows_of_application_applying_one_shot_rules,
};
use crate::window::focus::respond_to_window_receiving_focus;
use crate::window::layer::set_window_layer_unless_explicitly_set;
use crate::window::manager::{
    WindowManager, WindowOriginDisplayMode,
    forget_front_switched_event_that_arrived_before_application_was_tracked, forget_managed_window,
    has_front_switched_event_arrived_before_application_was_tracked,
    is_window_eligible_for_management, record_focused_event_that_arrived_before_window_was_tracked,
    record_front_switched_event_that_arrived_before_application_was_tracked,
    record_managed_window_on_space_updating_its_shadow, should_window_be_managed,
    space_managing_window, start_tracking_application, stop_tracking_application,
    stop_tracking_window, tracked_application_with_process_id, tracked_window_with_id,
    tracked_windows_of_application,
};
use crate::window::model::{
    destroy_window_releasing_its_accessibility_element, query_space_holding_window,
};
use crate::window::opacity::set_window_opacity_unless_disabled_or_fixed_by_rule;
use crate::window::scratchpad::remove_window_from_its_scratchpad;
use crate::window::shadow::apply_shadow_removal_mode_to_window;

pub(crate) fn handle_application_launched_event(
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
            "handle_application_launched_event", process.name, process.process_id.0
        );
        forget_front_switched_event_that_arrived_before_application_was_tracked(
            window_manager,
            process.process_id,
        );
        return;
    }

    if process.ns_application.load(Ordering::Acquire).is_null() {
        debug!(
            "{}: {} ({}) missing ns_application. fetching..\n",
            "handle_application_launched_event", process.name, process.process_id.0
        );
        process.ns_application.store(
            copy_running_application_of_process(&process),
            Ordering::Release,
        );

        if process.ns_application.load(Ordering::Acquire).is_null() {
            debug!(
                "{}: {} ({}) unable to fetch ns_application..\n",
                "handle_application_launched_event", process.name, process.process_id.0
            );

            let process_serial_number = process.process_serial_number;
            dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, move || {
                let process = process_with_process_serial_number(&process_serial_number);
                if let Some(process) = process {
                    post_event_to_event_loop(Event::ApplicationLaunched(process));
                }
            });

            return;
        }
    }

    if !has_process_finished_launching(&process) {
        debug!(
            "{}: {} ({}) is not finished launching, subscribing to finishedLaunching changes\n",
            "handle_application_launched_event", process.name, process.process_id.0
        );
        start_observing_application_finished_launching(WORKSPACE_CONTEXT.get().unwrap(), &process);

        //
        // NOTE(asmvik): Do this again in case of race-conditions between the previous check and key-value observation subscription.
        // Not actually sure if this can happen in practice..
        //

        if has_process_finished_launching(&process) {
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
                        release_key_value_observation_process_reference_on_main_queue(&process);
                    }
                }
            }
        } else {
            return;
        }
    }

    if !is_process_observable_refreshing_its_activation_policy(&process) {
        debug!(
            "{}: {} ({}) is not observable, subscribing to activationPolicy changes\n",
            "handle_application_launched_event", process.name, process.process_id.0
        );
        start_observing_application_activation_policy(WORKSPACE_CONTEXT.get().unwrap(), &process);

        //
        // NOTE(asmvik): Do this again in case of race-conditions between the previous check and key-value observation subscription.
        // Not actually sure if this can happen in practice..
        //

        if is_process_observable_refreshing_its_activation_policy(&process) {
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
                        release_key_value_observation_process_reference_on_main_queue(&process);
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

    let application = tracked_application_with_process_id(window_manager, process.process_id);
    let mut application: Application = match application {
        Some(_) => return,
        None => create_application_for_process(&process),
    };

    if !start_observing_application_notifications_reporting_whether_all_registered(&mut application)
    {
        let ax_retry = application.ax_retry;

        stop_observing_application_notifications(&mut application);
        destroy_application_releasing_its_accessibility_element(application);
        debug!(
            "{}: could not observe notifications for {} ({}) ({})\n",
            "handle_application_launched_event",
            process.name,
            process.process_id.0,
            ax_retry as i32
        );

        if ax_retry {
            let process_serial_number = process.process_serial_number;
            dispatch_after_on_main_queue((0.1f32 * NSEC_PER_SEC as f32) as i64, move || {
                let process = process_with_process_serial_number(&process_serial_number);
                if let Some(process) = process {
                    post_event_to_event_loop(Event::ApplicationLaunched(process));
                }
            });
        }

        return;
    }

    if has_front_switched_event_arrived_before_application_was_tracked(
        window_manager,
        process.process_id,
    ) {
        post_event_to_event_loop(Event::ApplicationFrontSwitched(Arc::clone(&process)));
        forget_front_switched_event_that_arrived_before_application_was_tracked(
            window_manager,
            process.process_id,
        );
    }

    debug!(
        "{}: {} ({})\n",
        "handle_application_launched_event", process.name, process.process_id.0
    );
    start_tracking_application(window_manager, application);
    queue_pending_signal_for_its_subscribers(
        SignalType::ApplicationLaunched,
        SignalContext::Application(process.process_id),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );

    let window_list = track_untracked_windows_of_application_applying_one_shot_rules(
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
    let default_origin = window_manager.window_origin_display_mode
        == WindowOriginDisplayMode::DisplayTheWindowOpenedOn;

    if !default_origin {
        if window_manager.window_origin_display_mode == WindowOriginDisplayMode::FocusedDisplay {
            space_id = space_manager.current_space_id;
        } else
        /* if (g_window_manager.window_origin_mode == WINDOW_ORIGIN_CURSOR) */
        {
            space_id = query_current_space_of_display_under_the_cursor();
        }
    }

    let mut view_list: Vec<SpaceId> = Vec::new();

    for index in 0..window_list.len() {
        let window_id = window_list[index];

        if should_window_be_managed(window_id, window_manager)
            && space_managing_window(window_manager, window_id).is_none()
        {
            if default_origin {
                space_id = query_space_holding_window(window_id);
            }

            let view = find_or_create_view_for_space(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
            let view_layout = space_manager.view.get(&view).map(|view| view.layout);
            if view_layout.is_some_and(|view_layout| view_layout != ViewLayout::Float) {
                //
                // @cleanup
                //
                // :AXBatching
                //
                // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
                // making sure that each window is only moved and resized a single time, when the final layout has been computed.
                // This is necessary to make sure that we do not call the AX API for each modification to the tree.
                //

                set_window_layer_unless_explicitly_set(window_id, LAYER_BELOW, window_manager);
                add_window_to_view_tree_preferring_insertion_point(
                    space_manager,
                    view,
                    window_id,
                    prev_window_id,
                    display_manager,
                    window_manager,
                );
                record_managed_window_on_space_updating_its_shadow(
                    window_manager,
                    window_id,
                    space_manager,
                    view,
                );

                if let Some(view) = space_manager.view.get_mut(&view) {
                    view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
                }
                view_list.push(view);

                prev_window_id = window_id;
            }
        }

        if is_window_eligible_for_management(window_id, window_manager) {
            queue_pending_signal_for_its_subscribers(
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
        if !is_space_visible_on_its_display(view) {
            continue;
        }
        if !has_view_windows_awaiting_their_areas(space_manager, view) {
            continue;
        }

        move_windows_below_node_into_their_areas(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.get_mut(&view) {
            view.flags.remove(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    }

    if is_running_on_macos_sequoia() || is_running_on_macos_tahoe() {
        request_skylight_notifications_for_windows_that_need_them(window_manager, space_manager);
    }
}

pub(crate) fn handle_application_terminated_event(
    process: Arc<Process>,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let application = tracked_application_with_process_id(window_manager, process.process_id);

    match application {
        None => {
            debug!(
                "{}: {} ({}) (not observed)\n",
                "handle_application_terminated_event", process.name, process.process_id.0
            );
        }
        Some(application) => {
            debug!(
                "{}: {} ({})\n",
                "handle_application_terminated_event", process.name, process.process_id.0
            );
            queue_pending_signal_for_its_subscribers(
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

            let window_list = tracked_windows_of_application(window_manager, application);

            let mut view_list: Vec<SpaceId> = Vec::new();

            for index in 0..window_list.len() {
                let window_id = window_list[index];

                let claimed_for_destruction = window_manager
                    .window
                    .get(&window_id)
                    .is_some_and(|window| window.liveness.claim_for_destruction());
                if !claimed_for_destruction {
                    if let Some(window) = window_manager.window.get_mut(&window_id) {
                        window.application = None;
                    }
                    continue;
                }

                let view = space_managing_window(window_manager, window_id);
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

                    remove_window_from_view_tree(
                        space_manager,
                        view,
                        window_id,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                    forget_managed_window(window_manager, window_id);

                    if let Some(view) = space_manager.view.get_mut(&view) {
                        view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
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
                    .get(&window_id)
                    .is_some_and(|window| window.is_eligible);
                if is_eligible {
                    queue_pending_signal_for_its_subscribers(
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

                remove_window_from_its_scratchpad(
                    window_manager,
                    window_id,
                    false,
                    process_manager,
                    display_manager,
                    space_manager,
                    mouse_drag_state,
                );
                let window = stop_tracking_window(window_manager, window_id);
                if let Some(mut window) = window {
                    stop_observing_window_notifications(&mut window, window_manager);
                    destroy_window_releasing_its_accessibility_element(window);
                }
            }

            let application_record = stop_tracking_application(window_manager, application);
            if let Some(mut application_record) = application_record {
                stop_observing_application_notifications(&mut application_record);
                destroy_application_releasing_its_accessibility_element(application_record);
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
                if !is_space_visible_on_its_display(view) {
                    continue;
                }
                if !has_view_windows_awaiting_their_areas(space_manager, view) {
                    continue;
                }

                move_windows_below_node_into_their_areas(
                    view,
                    ROOT_NODE_ID,
                    window_manager,
                    space_manager,
                );
                if let Some(view) = space_manager.view.get_mut(&view) {
                    view.flags.remove(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
                }
            }

            if is_running_on_macos_sequoia() || is_running_on_macos_tahoe() {
                request_skylight_notifications_for_windows_that_need_them(
                    window_manager,
                    space_manager,
                );
            }
        }
    }

    destroy_process_releasing_its_running_application(process);
}

pub(crate) fn handle_application_front_switched_event(
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
    let application = tracked_application_with_process_id(window_manager, process.process_id);

    let Some(application) = application else {
        record_front_switched_event_that_arrived_before_application_was_tracked(
            window_manager,
            process.process_id,
        );
        return;
    };

    if space_manager.skip_window_focus_animation {
        let application_connection = window_manager
            .application
            .get(&application)
            .map_or(0, |application| application.connection);
        let psn_space_id = query_space_of_first_window_owned_by_connection(application_connection);

        let last_cmd_tab_time = LAST_COMMAND_TAB_TIME.load(Ordering::Relaxed);
        let delta_time = (read_system_clock_in_nanoseconds() as f32 - last_cmd_tab_time as f32)
            * (1000.0f32 / system_clock_ticks_per_second() as f32);
        if delta_time > 1500.0f32 {
            let application_element_ref = window_manager
                .application
                .get(&application)
                .map(|application| application.element_ref);
            if let Some(application_element_ref) = application_element_ref {
                let mut dummy: *const CFType = core::ptr::null();
                unsafe {
                    crate::ffi::accessibility::AXUIElementCopyAttributeValue(
                        &*application_element_ref,
                        accessibility_fence_attribute_name(),
                        NonNull::from(&mut dummy),
                    )
                };
                drop(unsafe { take_create_rule_result(dummy) });
            }
        }

        if WINDOW_FOCUS_NOTIFICATION_IS_PENDING.load(Ordering::Relaxed) == false {
            if psn_space_id.0 != 0 && !is_space_visible_on_its_display(psn_space_id) {
                unsafe {
                    SLSSpaceSetFrontPSN(
                        *SKYLIGHT_CONNECTION_ID.get().unwrap(),
                        psn_space_id.0,
                        process.process_serial_number,
                    )
                };
                focus_space_with_synthesized_dock_swipes(
                    query_display_holding_space(psn_space_id),
                    psn_space_id,
                    window_manager,
                );
            }
        }
    }

    let deactivated_application =
        tracked_application_with_process_id(window_manager, process_manager.front_process_id);
    if let Some(deactivated_application) = deactivated_application {
        queue_pending_signal_for_its_subscribers(
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
        "handle_application_front_switched_event", process.name, process.process_id.0
    );
    queue_pending_signal_for_its_subscribers(
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
    queue_pending_signal_for_its_subscribers(
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
                .get(&application)
                .map(|application| Arc::clone(&application.name));
            debug!(
                "{}: {} has windows that are not yet resolved\n",
                "handle_application_front_switched_event",
                text_or_printf_null_placeholder(application_name.as_deref())
            );
            track_existing_windows_of_application_including_those_on_inactive_spaces(
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
        .get(&application)
        .map_or(WindowId(0), read_focused_window_of_application);
    if application_focused_window_id.0 == 0 {
        let focused_window =
            tracked_window_with_id(window_manager, window_manager.focused_window_id);
        if let Some(focused_window) = focused_window {
            let normal_window_opacity = window_manager.normal_window_opacity;
            set_window_opacity_unless_disabled_or_fixed_by_rule(
                window_manager,
                focused_window,
                normal_window_opacity,
            );
        }

        window_manager.last_window_id = window_manager.focused_window_id;
        window_manager.focused_window_id = WindowId(0);
        let application_process_serial_number = window_manager
            .application
            .get(&application)
            .map(|application| application.process_serial_number);
        if let Some(application_process_serial_number) = application_process_serial_number {
            window_manager.focused_window_process_serial_number = application_process_serial_number;
        }
        mouse_drag_state.ffm_window_id = WindowId(0);
        return;
    }

    let window = tracked_window_with_id(window_manager, application_focused_window_id);
    let Some(window) = window else {
        let focused_window =
            tracked_window_with_id(window_manager, window_manager.focused_window_id);
        if let Some(focused_window) = focused_window {
            let normal_window_opacity = window_manager.normal_window_opacity;
            set_window_opacity_unless_disabled_or_fixed_by_rule(
                window_manager,
                focused_window,
                normal_window_opacity,
            );
        }

        record_focused_event_that_arrived_before_window_was_tracked(
            window_manager,
            application_focused_window_id,
        );
        return;
    };

    respond_to_window_receiving_focus(window_manager, mouse_drag_state, window, space_manager);
    queue_pending_signal_for_its_subscribers(
        SignalType::WindowFocused,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
    WINDOW_FOCUS_NOTIFICATION_IS_PENDING.store(false, Ordering::Release);
}

pub(crate) fn handle_application_visible_event(
    process_id: ProcessId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let application = tracked_application_with_process_id(window_manager, process_id);
    let Some(application) = application else {
        return;
    };

    let application_name = window_manager
        .application
        .get(&application)
        .map(|application| Arc::clone(&application.name));
    debug!(
        "{}: {}\n",
        "handle_application_visible_event",
        text_or_printf_null_placeholder(application_name.as_deref())
    );
    if let Some(application) = window_manager.application.get_mut(&application) {
        application.is_hidden = false;
    }

    let window_list = tracked_windows_of_application(window_manager, application);
    let mut prev_window_id = window_manager.last_window_id;

    let mut view_list: Vec<SpaceId> = Vec::new();

    for index in 0..window_list.len() {
        let window_id = window_list[index];

        if should_window_be_managed(window_id, window_manager)
            && space_managing_window(window_manager, window_id).is_none()
        {
            let view = find_or_create_view_for_space(
                space_manager,
                query_space_holding_window(window_id),
                display_manager,
                window_manager,
            );
            let Some(view_layout) = space_manager.view.get(&view).map(|view| view.layout) else {
                continue;
            };
            if view_layout == ViewLayout::Float {
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

            set_window_layer_unless_explicitly_set(window_id, LAYER_BELOW, window_manager);
            add_window_to_view_tree_preferring_insertion_point(
                space_manager,
                view,
                window_id,
                prev_window_id,
                display_manager,
                window_manager,
            );
            record_managed_window_on_space_updating_its_shadow(
                window_manager,
                window_id,
                space_manager,
                view,
            );

            if let Some(view) = space_manager.view.get_mut(&view) {
                view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
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
        if !is_space_visible_on_its_display(view) {
            continue;
        }
        if !has_view_windows_awaiting_their_areas(space_manager, view) {
            continue;
        }

        move_windows_below_node_into_their_areas(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.get_mut(&view) {
            view.flags.remove(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    }

    queue_pending_signal_for_its_subscribers(
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

pub(crate) fn handle_application_hidden_event(
    process_id: ProcessId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let application = tracked_application_with_process_id(window_manager, process_id);
    let Some(application) = application else {
        return;
    };

    let application_name = window_manager
        .application
        .get(&application)
        .map(|application| Arc::clone(&application.name));
    debug!(
        "{}: {}\n",
        "handle_application_hidden_event",
        text_or_printf_null_placeholder(application_name.as_deref())
    );
    if let Some(application) = window_manager.application.get_mut(&application) {
        application.is_hidden = true;
    }

    let window_list = tracked_windows_of_application(window_manager, application);

    let mut view_list: Vec<SpaceId> = Vec::new();

    for index in 0..window_list.len() {
        let window_id = window_list[index];

        let view = space_managing_window(window_manager, window_id);
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

            set_window_layer_unless_explicitly_set(window_id, LAYER_NORMAL, window_manager);
            remove_window_from_view_tree(
                space_manager,
                view,
                window_id,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            forget_managed_window(window_manager, window_id);
            apply_shadow_removal_mode_to_window(window_manager, window_id);

            if let Some(view) = space_manager.view.get_mut(&view) {
                view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
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
        if !is_space_visible_on_its_display(view) {
            continue;
        }
        if !has_view_windows_awaiting_their_areas(space_manager, view) {
            continue;
        }

        move_windows_below_node_into_their_areas(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.get_mut(&view) {
            view.flags.remove(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    }

    queue_pending_signal_for_its_subscribers(
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
