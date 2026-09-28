#![allow(deprecated)]

use core::ptr::NonNull;
use std::sync::Arc;

use crate::application::model::{
    copy_accessibility_windows_of_application, create_application_for_process,
    destroy_application_releasing_its_accessibility_element,
};
use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_spaces_of_display;
use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::accessibility::{
    _AXUIElementCreateWithRemoteToken, AXUIElement, AXUIElementCopyAttributeValue, AXUIElementRef,
    kAXRoleAttribute, kAXWindowRole, read_window_id_of_accessibility_element,
};
use crate::ffi::core_foundation::{
    CFData, CFDataCreateMutable, CFDataGetMutableBytePtr, CFDataIncreaseLength, CFEqual, CFIndex,
    CFMutableData, CFRetained, CFType, as_cftype, cfarray_borrow_value_at_index, cfarray_count,
    take_create_rule_result,
};
use crate::mouse::drag::MouseDragState;
use crate::notifications::application::{
    start_observing_application_notifications_reporting_whether_all_registered,
    stop_observing_application_notifications,
};
use crate::notifications::window::{
    start_observing_window_notifications_reporting_whether_all_registered,
    stop_observing_window_notifications,
};
use crate::notifications::workspace::{
    WORKSPACE_CONTEXT, start_observing_application_activation_policy,
};
use crate::process::manager::{PROCESS_TABLE, ProcessManager};
use crate::process::model::Process;
use crate::process::running_application::is_process_observable_refreshing_its_activation_policy;
use crate::serialise::window::write_tracked_window_as_json_object;
use crate::space::managed_space::query_windows_on_spaces_owned_by_connection;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::{ProcessId, SpaceId, WindowId};
use crate::support::log::{is_verbose_debug_output_enabled, text_or_printf_null_placeholder};
use crate::support::response::Response;
use crate::window::focus::query_focused_tracked_window;
use crate::window::manager::{
    WindowManager, forget_focused_event_that_arrived_before_window_was_tracked,
    has_focused_event_arrived_before_window_was_tracked, is_window_eligible_for_management,
    start_tracking_application, start_tracking_window, tracked_window_with_id,
};
use crate::window::model::{
    WindowFlag, WindowRuleFlag, clear_window_rule_flag, create_window_from_accessibility_element,
    destroy_window_releasing_its_accessibility_element, is_window_a_standard_window,
    is_window_at_most_500_points_wide_or_tall, is_window_at_normal_window_level,
    is_window_flag_set, is_window_movable, is_window_on_more_than_one_space, is_window_resizable,
    is_window_rule_flag_set, is_window_subrole_unknown, set_window_flag, window_role_as_string,
    window_subrole_as_string, window_title_as_string,
};
use crate::window::opacity::set_window_opacity_unless_disabled_or_fixed_by_rule;
use crate::window::rule::RuleFlag;
use crate::window::rule_application::{
    apply_effects_other_than_manage_of_matching_rules_to_window,
    apply_manage_effect_of_matching_rules_to_window,
};
use crate::window::shadow::apply_shadow_removal_mode_to_window;

pub(crate) fn track_newly_discovered_window_applying_its_rules(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_id: ProcessId,
    window_ref: AXUIElementRef,
    window_id: WindowId,
    one_shot_rules: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Option<WindowId> {
    let mut window =
        create_window_from_accessibility_element(process_id, window_ref, window_id, window_manager);

    let window_title = window_title_as_string(&window);
    let window_role = window_role_as_string(&window);
    let window_subrole = window_subrole_as_string(&window);
    let application_name = window
        .application
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
        .map(|application| Arc::clone(&application.name));
    crate::debug!(
        "{}:{} {} - {} ({}:{}:{})\n",
        "track_newly_discovered_window_applying_its_rules",
        window.id.0 as i32,
        text_or_printf_null_placeholder(application_name.as_deref()),
        window_title,
        window_role,
        window_subrole,
        window.is_root as i32
    );

    if is_window_subrole_unknown(&window) {
        crate::debug!(
            "{}: ignoring AXUnknown window {} {}\n",
            "track_newly_discovered_window_applying_its_rules",
            text_or_printf_null_placeholder(application_name.as_deref()),
            window.id.0 as i32
        );
        forget_focused_event_that_arrived_before_window_was_tracked(window_manager, window.id);
        destroy_window_releasing_its_accessibility_element(window);
        return None;
    }

    //
    // NOTE(asmvik): Attempt to track **all** windows.
    //

    if !start_observing_window_notifications_reporting_whether_all_registered(
        &mut window,
        window_manager,
    ) {
        crate::debug!(
            "{}: could not observe {} {}\n",
            "track_newly_discovered_window_applying_its_rules",
            text_or_printf_null_placeholder(application_name.as_deref()),
            window.id.0 as i32
        );
        forget_focused_event_that_arrived_before_window_was_tracked(window_manager, window.id);
        stop_observing_window_notifications(&mut window, window_manager);
        destroy_window_releasing_its_accessibility_element(window);
        return None;
    }

    if has_focused_event_arrived_before_window_was_tracked(window_manager, window.id) {
        post_event_to_event_loop(Event::WindowFocused(window.id));
        forget_focused_event_that_arrived_before_window_was_tracked(window_manager, window.id);
    }

    let window_id = window.id;
    let window_is_root = window.is_root;
    start_tracking_window(window_manager, window);

    //
    // NOTE(asmvik): However, only **root windows** are eligible for management.
    //

    if window_is_root {
        //
        // NOTE(asmvik): A lot of windows misreport their accessibility role, so we allow the user
        // to specify rules to make sure that we do in fact manage these windows properly.
        //
        // This part of the rule must be applied at this stage (prior to other rule properties), and if
        // no such rule matches this window, it will be ignored if it does not have a role of kAXWindowRole.
        //

        apply_manage_effect_of_matching_rules_to_window(
            space_manager,
            window_manager,
            window_id,
            &window_title,
            &window_role,
            &window_subrole,
            one_shot_rules,
            display_manager,
            mouse_drag_state,
        );

        if is_window_eligible_for_management(window_id, window_manager) {
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window.is_eligible = true;
            }
            apply_effects_other_than_manage_of_matching_rules_to_window(
                space_manager,
                window_manager,
                window_id,
                &window_title,
                &window_role,
                &window_subrole,
                one_shot_rules,
                process_manager,
                display_manager,
                mouse_drag_state,
                mission_control_mode,
            );
            apply_shadow_removal_mode_to_window(window_manager, window_id);
            set_window_opacity_unless_disabled_or_fixed_by_rule(
                window_manager,
                window_id,
                window_manager.normal_window_opacity,
            );

            let application_is_hidden = window_manager
                .application
                .find(&process_id)
                .is_some_and(|application| application.is_hidden);
            if application_is_hidden {
                return Some(window_id);
            }

            let Some(window) = window_manager.window.find_mut(&window_id) else {
                return Some(window_id);
            };
            if is_window_flag_set(window, WindowFlag::MINIMIZED) {
                return Some(window_id);
            }
            if is_window_flag_set(window, WindowFlag::IN_NATIVE_FULLSCREEN) {
                return Some(window_id);
            }
            if is_window_rule_flag_set(window, WindowRuleFlag::MANAGE_FORCED_ON) {
                return Some(window_id);
            }

            if is_window_rule_flag_set(window, WindowRuleFlag::NATIVE_FULLSCREEN_REQUESTED) {
                clear_window_rule_flag(window, WindowRuleFlag::NATIVE_FULLSCREEN_REQUESTED);
                return Some(window_id);
            }

            if is_window_on_more_than_one_space(window.id)
                || !is_window_movable(window)
                || !is_window_a_standard_window(window)
                || !is_window_at_normal_window_level(window)
                || (!is_window_resizable(window)
                    && is_window_at_most_500_points_wide_or_tall(window))
            {
                set_window_flag(window, WindowFlag::FLOATING);
            }
        } else {
            crate::debug!(
                "{} ignoring incorrectly marked window {} {}\n",
                "track_newly_discovered_window_applying_its_rules",
                text_or_printf_null_placeholder(application_name.as_deref()),
                window_id.0 as i32
            );
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                set_window_flag(window, WindowFlag::FLOATING);
            }

            //
            // NOTE(asmvik): Print window information when debug_output is enabled.
            // Useful for identifying and creating rules if this window should in fact be managed.
            //

            if is_verbose_debug_output_enabled() {
                let mut response = Response::to_standard_output();
                response.write(format_args!("window info: \n"));
                write_tracked_window_as_json_object(
                    &mut response,
                    window_id,
                    0,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                );
                response.write(format_args!("\n"));
            }
        }
    } else {
        crate::debug!(
            "{} ignoring child window {} {}\n",
            "track_newly_discovered_window_applying_its_rules",
            text_or_printf_null_placeholder(application_name.as_deref()),
            window_id.0 as i32
        );
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            set_window_flag(window, WindowFlag::FLOATING);
        }

        //
        // NOTE(asmvik): Print window information when debug_output is enabled.
        //

        if is_verbose_debug_output_enabled() {
            let mut response = Response::to_standard_output();
            response.write(format_args!("window info: \n"));
            write_tracked_window_as_json_object(
                &mut response,
                window_id,
                0,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
            response.write(format_args!("\n"));
        }
    }

    Some(window_id)
}

pub(crate) fn track_untracked_windows_of_application_applying_one_shot_rules(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_id: ProcessId,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Vec<WindowId> {
    let Some(application) = window_manager.application.find(&process_id) else {
        return Vec::new();
    };
    let Some(window_list) = copy_accessibility_windows_of_application(application) else {
        return Vec::new();
    };

    let window_count = cfarray_count(&window_list) as i32;
    let mut list: Vec<WindowId> = Vec::with_capacity(window_count as usize);

    for index in 0..window_count {
        let Some(window_ref) = (unsafe {
            cfarray_borrow_value_at_index::<AXUIElement>(&window_list, index as CFIndex)
        }) else {
            continue;
        };

        let window_id = read_window_id_of_accessibility_element(window_ref);
        if window_id == 0 || tracked_window_with_id(window_manager, WindowId(window_id)).is_some() {
            continue;
        }

        let retained_window_ref =
            CFRetained::into_raw(unsafe { CFRetained::retain(NonNull::from(window_ref)) })
                .as_ptr()
                .cast_const();
        let window = track_newly_discovered_window_applying_its_rules(
            space_manager,
            window_manager,
            process_id,
            retained_window_ref,
            WindowId(window_id),
            true,
            process_manager,
            display_manager,
            mouse_drag_state,
            mission_control_mode,
        );
        if let Some(window) = window {
            list.push(window);
        }
    }

    let mut rule_length = window_manager.rules.len() as i32;
    let mut index: i32 = 0;
    while index < rule_length {
        if RuleFlag(window_manager.rules[index as usize].flags)
            .contains(RuleFlag::ONE_SHOT_DUE_FOR_REMOVAL)
        {
            window_manager.rules.swap_remove(index as usize);
            index -= 1;
            rule_length -= 1;
        }
        index += 1;
    }

    drop(window_list);
    list
}

pub(crate) fn query_application_windows_on_every_space(
    process_id: Option<ProcessId>,
    window_manager: &mut WindowManager,
) -> Option<Vec<WindowId>> {
    let display_list = query_displays_active_for_drawing();

    let mut space_list: Option<Vec<SpaceId>> = None;

    for index in 0..display_list.len() {
        let Some(list) = query_spaces_of_display(display_list[index]) else {
            continue;
        };

        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of windows that have been allocated.
        //

        space_list.get_or_insert_with(Vec::new).extend(list);
    }

    let Some(space_list) = space_list else {
        return None;
    };

    let connection_id = match process_id {
        Some(process_id) => match window_manager.application.find(&process_id) {
            Some(application) => application.connection,
            None => return None,
        },
        None => 0,
    };
    query_windows_on_spaces_owned_by_connection(&space_list, connection_id, true, window_manager)
}

pub(crate) fn track_existing_windows_of_application_including_those_on_inactive_spaces(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_id: ProcessId,
    refresh_index: i32,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> bool {
    let mut result = false;

    let Some(global_window_list) =
        query_application_windows_on_every_space(Some(process_id), window_manager)
    else {
        return result;
    };
    let global_window_count = global_window_list.len() as i32;

    let Some(application) = window_manager.application.find(&process_id) else {
        return result;
    };
    let application_name = Arc::clone(&application.name);
    let application_process_id = application.process_id;
    let window_list_ref = copy_accessibility_windows_of_application(application);
    let window_count = match &window_list_ref {
        Some(window_list_ref) => cfarray_count(window_list_ref) as i32,
        None => 0,
    };

    let mut empty_count = 0;
    if let Some(window_list_ref) = &window_list_ref {
        for index in 0..window_count {
            let window_ref = unsafe {
                cfarray_borrow_value_at_index::<AXUIElement>(window_list_ref, index as CFIndex)
            };
            let window_id = window_ref.map_or(0, |window_ref| {
                read_window_id_of_accessibility_element(window_ref)
            });

            //
            // @cleanup
            //
            // :Workaround
            //
            // NOTE(asmvik): The AX API appears to always include a single element for Finder that returns an empty window id.
            // This is likely the desktop window. Other similar cases should be handled the same way; simply ignore the window when
            // we attempt to do an equality check to see if we have correctly discovered the number of windows to track.
            //

            if window_id == 0 {
                empty_count += 1;
                continue;
            }

            if tracked_window_with_id(window_manager, WindowId(window_id)).is_none()
                && let Some(window_ref) = window_ref
            {
                let retained_window_ref =
                    CFRetained::into_raw(unsafe { CFRetained::retain(NonNull::from(window_ref)) })
                        .as_ptr()
                        .cast_const();
                track_newly_discovered_window_applying_its_rules(
                    space_manager,
                    window_manager,
                    process_id,
                    retained_window_ref,
                    WindowId(window_id),
                    false,
                    process_manager,
                    display_manager,
                    mouse_drag_state,
                    mission_control_mode,
                );
            }
        }
    }

    if global_window_count != window_count - empty_count {
        if refresh_index == -1 {
            let mut missing_window = false;
            let mut application_window_list: Vec<WindowId> = Vec::new();

            for index in 0..global_window_count as usize {
                let window = tracked_window_with_id(window_manager, global_window_list[index]);
                if window.is_none() {
                    missing_window = true;
                    application_window_list.push(global_window_list[index]);
                }
            }

            if missing_window {
                crate::debug!(
                    "{}: {} has {} windows that are not yet resolved, attempting workaround\n",
                    "track_existing_windows_of_application_including_those_on_inactive_spaces",
                    application_name,
                    application_window_list.len() as i32
                );

                //
                // NOTE(asmvik): MacOS API does not return AXUIElementRef of windows on inactive spaces.
                // However, we can just brute-force the element_id and create the AXUIElementRef ourselves.
                //
                // :Attribution
                // https://github.com/decodism
                // https://github.com/lwouis/alt-tab-macos/issues/1324#issuecomment-2631035482
                //

                if let Some(data_ref) = CFDataCreateMutable(None, 0x14) {
                    CFDataIncreaseLength(Some(&*data_ref), 0x14);

                    let data = CFDataGetMutableBytePtr(Some(&*data_ref));
                    unsafe {
                        core::ptr::write_unaligned(
                            data.add(0x0).cast::<u32>(),
                            application_process_id.0 as u32,
                        );
                        core::ptr::write_unaligned(data.add(0x8).cast::<u32>(), 0x636f636f);
                    }

                    for element_id in 0u64..0x7fff {
                        let application_window_list_length = application_window_list.len() as i32;
                        if application_window_list_length == 0 {
                            break;
                        }

                        unsafe {
                            core::ptr::write_unaligned(data.add(0xc).cast::<u64>(), element_id)
                        };
                        let element_ref = unsafe {
                            take_create_rule_result(_AXUIElementCreateWithRemoteToken(
                                (&*data_ref as *const CFMutableData).cast::<CFData>(),
                            ))
                        };

                        let mut role: *const CFType = core::ptr::null();
                        if let Some(element_ref) = &element_ref {
                            unsafe {
                                AXUIElementCopyAttributeValue(
                                    element_ref,
                                    kAXRoleAttribute(),
                                    NonNull::from(&mut role),
                                )
                            };
                        }
                        let role = unsafe { take_create_rule_result(role) };

                        if let Some(role) = role {
                            if CFEqual(Some(&*role), Some(as_cftype(kAXWindowRole()))) {
                                let element_window_id =
                                    element_ref.as_deref().map_or(0, |element_ref| {
                                        read_window_id_of_accessibility_element(element_ref)
                                    });
                                let mut matched = false;

                                if element_window_id != 0 {
                                    for inner_index in 0..application_window_list_length as usize {
                                        if application_window_list[inner_index]
                                            == WindowId(element_window_id)
                                        {
                                            matched = true;
                                            application_window_list.swap_remove(inner_index);
                                            break;
                                        }
                                    }
                                }

                                if matched && let Some(element_ref) = element_ref {
                                    track_newly_discovered_window_applying_its_rules(
                                        space_manager,
                                        window_manager,
                                        process_id,
                                        CFRetained::into_raw(element_ref).as_ptr().cast_const(),
                                        WindowId(element_window_id),
                                        false,
                                        process_manager,
                                        display_manager,
                                        mouse_drag_state,
                                        mission_control_mode,
                                    );
                                } else {
                                    drop(element_ref);
                                }
                            }

                            drop(role);
                        }
                    }

                    drop(data_ref);
                }
            }

            if application_window_list.len() > 0 {
                crate::debug!(
                    "{}: workaround failed to resolve all windows for {}\n",
                    "track_existing_windows_of_application_including_those_on_inactive_spaces",
                    application_name
                );
                window_manager.applications_to_refresh.push(process_id);
            } else {
                crate::debug!(
                    "{}: workaround resolved all windows for {}\n",
                    "track_existing_windows_of_application_including_those_on_inactive_spaces",
                    application_name
                );
            }
        } else {
            let mut missing_window = false;

            for index in 0..global_window_count as usize {
                let window = tracked_window_with_id(window_manager, global_window_list[index]);
                if window.is_none() {
                    missing_window = true;
                    break;
                }
            }

            if !missing_window {
                crate::debug!(
                    "{}: all windows for {} are now resolved\n",
                    "track_existing_windows_of_application_including_those_on_inactive_spaces",
                    application_name
                );
                window_manager
                    .applications_to_refresh
                    .swap_remove(refresh_index as usize);
                result = true;
            }
        }
    } else if refresh_index != -1 {
        crate::debug!(
            "{}: all windows for {} are now resolved\n",
            "track_existing_windows_of_application_including_those_on_inactive_spaces",
            application_name
        );
        window_manager
            .applications_to_refresh
            .swap_remove(refresh_index as usize);
        result = true;
    }

    drop(window_list_ref);

    result
}

pub(crate) fn retry_tracking_windows_of_applications_with_unresolved_windows(
    space_manager: &mut SpaceManager,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> bool {
    let mut refresh_count = window_manager.applications_to_refresh.len() as i32;
    if refresh_count == 0 {
        return false;
    }
    let window_count = window_manager.window.len();
    let mut index: i32 = 0;
    while index < refresh_count {
        let process_id = window_manager.applications_to_refresh[index as usize];
        let application_name = window_manager
            .application
            .find(&process_id)
            .map(|application| Arc::clone(&application.name));
        crate::debug!(
            "{}: {} has windows that are not yet resolved\n",
            "retry_tracking_windows_of_applications_with_unresolved_windows",
            application_name.as_deref().unwrap_or("(null)")
        );
        let result = track_existing_windows_of_application_including_those_on_inactive_spaces(
            space_manager,
            window_manager,
            process_id,
            index,
            process_manager,
            display_manager,
            mouse_drag_state,
            mission_control_mode,
        );
        if result {
            refresh_count -= 1;
            index -= 1;
        }
        index += 1;
    }
    window_count != window_manager.window.len()
}

pub(crate) fn start_tracking_running_applications_and_their_windows(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    objc2::rc::autoreleasepool(|_pool| {
        let process_list: Vec<Arc<Process>> = PROCESS_TABLE
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .values()
            .map(Arc::clone)
            .collect();

        for process in process_list {
            if is_process_observable_refreshing_its_activation_policy(&process) {
                let mut application = create_application_for_process(&process);

                if start_observing_application_notifications_reporting_whether_all_registered(
                    &mut application,
                ) {
                    let application_process_id = application.process_id;
                    start_tracking_application(window_manager, application);
                    track_existing_windows_of_application_including_those_on_inactive_spaces(
                        space_manager,
                        window_manager,
                        application_process_id,
                        -1,
                        process_manager,
                        display_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    );
                } else {
                    stop_observing_application_notifications(&mut application);
                    destroy_application_releasing_its_accessibility_element(application);
                }
            } else {
                crate::debug!(
                    "{}: {} ({}) is not observable, subscribing to activationPolicy changes\n",
                    "start_tracking_running_applications_and_their_windows",
                    process.name,
                    process.process_id.0
                );
                start_observing_application_activation_policy(
                    WORKSPACE_CONTEXT.get().unwrap(),
                    &process,
                );
            }
        }
    });

    let window = query_focused_tracked_window(window_manager);
    if let Some(window_id) = window {
        window_manager.last_window_id = window_id;
        window_manager.focused_window_id = window_id;
        if let Some(application) = window_manager
            .window
            .find(&window_id)
            .and_then(|window| window.application)
            .and_then(|application_process_id| {
                window_manager.application.find(&application_process_id)
            })
        {
            window_manager.focused_window_process_serial_number = application.process_serial_number;
        }
        set_window_opacity_unless_disabled_or_fixed_by_rule(
            window_manager,
            window_id,
            window_manager.active_window_opacity,
        );
    }
}
