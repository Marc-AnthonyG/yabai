#![allow(deprecated)]

use core::ptr::NonNull;
use std::sync::Arc;

use crate::application::model::{application_create, application_destroy, application_window_list};
use crate::display::identity::display_manager_active_display_list;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_list;
use crate::event::queue::{Event, event_loop_post};
use crate::ffi::accessibility::{
    _AXUIElementCreateWithRemoteToken, AXUIElement, AXUIElementCopyAttributeValue, AXUIElementRef,
    ax_window_id, kAXRoleAttribute, kAXWindowRole,
};
use crate::ffi::core_foundation::{
    CFData, CFDataCreateMutable, CFDataGetMutableBytePtr, CFDataIncreaseLength, CFEqual, CFIndex,
    CFMutableData, CFRetained, CFType, as_cftype, cfarray_borrow_value_at_index, cfarray_count,
    take_create_rule_result,
};
use crate::mouse::drag::MouseDragState;
use crate::notifications::application::{application_observe, application_unobserve};
use crate::notifications::window::{window_observe, window_unobserve};
use crate::notifications::workspace::{
    WORKSPACE_CONTEXT, workspace_application_observe_activation_policy,
};
use crate::process::manager::{PROCESS_TABLE, ProcessManager};
use crate::process::model::Process;
use crate::process::running_application::workspace_application_is_observable;
use crate::serialise::window::window_serialize;
use crate::space::managed_space::space_window_list_for_connection;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::handles::{ProcessId, SpaceId, WindowId};
use crate::support::log::{g_verbose, or_null};
use crate::support::response::Response;
use crate::window::focus::window_manager_focused_window;
use crate::window::manager::{
    WindowManager, window_manager_add_application, window_manager_add_window,
    window_manager_find_lost_focused_event, window_manager_find_window,
    window_manager_is_window_eligible, window_manager_remove_lost_focused_event,
};
use crate::window::model::{
    WindowFlag, WindowRuleFlag, window_can_move, window_can_resize, window_check_flag,
    window_check_rule_flag, window_clear_rule_flag, window_create, window_destroy,
    window_is_standard, window_is_sticky, window_is_undersized, window_is_unknown,
    window_level_is_standard, window_role_ts, window_set_flag, window_subrole_ts, window_title_ts,
};
use crate::window::opacity::window_manager_set_window_opacity;
use crate::window::rule::RuleFlag;
use crate::window::rule_application::{
    window_manager_apply_manage_rules_to_window, window_manager_apply_rules_to_window,
};
use crate::window::shadow::window_manager_purify_window;

pub(crate) fn window_manager_create_and_add_window(
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
    let mut window = window_create(process_id, window_ref, window_id, window_manager);

    let window_title = window_title_ts(&window);
    let window_role = window_role_ts(&window);
    let window_subrole = window_subrole_ts(&window);
    let application_name = window
        .application
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
        .map(|application| Arc::clone(&application.name));
    crate::debug!(
        "{}:{} {} - {} ({}:{}:{})\n",
        "window_manager_create_and_add_window",
        window.id.0 as i32,
        or_null(application_name.as_deref()),
        window_title,
        window_role,
        window_subrole,
        window.is_root as i32
    );

    if window_is_unknown(&window) {
        crate::debug!(
            "{}: ignoring AXUnknown window {} {}\n",
            "window_manager_create_and_add_window",
            or_null(application_name.as_deref()),
            window.id.0 as i32
        );
        window_manager_remove_lost_focused_event(window_manager, window.id);
        window_destroy(window);
        return None;
    }

    //
    // NOTE(asmvik): Attempt to track **all** windows.
    //

    if !window_observe(&mut window, window_manager) {
        crate::debug!(
            "{}: could not observe {} {}\n",
            "window_manager_create_and_add_window",
            or_null(application_name.as_deref()),
            window.id.0 as i32
        );
        window_manager_remove_lost_focused_event(window_manager, window.id);
        window_unobserve(&mut window, window_manager);
        window_destroy(window);
        return None;
    }

    if window_manager_find_lost_focused_event(window_manager, window.id) {
        event_loop_post(Event::WindowFocused(window.id));
        window_manager_remove_lost_focused_event(window_manager, window.id);
    }

    let window_id = window.id;
    let window_is_root = window.is_root;
    window_manager_add_window(window_manager, window);

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

        window_manager_apply_manage_rules_to_window(
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

        if window_manager_is_window_eligible(window_id, window_manager) {
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window.is_eligible = true;
            }
            window_manager_apply_rules_to_window(
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
            window_manager_purify_window(window_manager, window_id);
            window_manager_set_window_opacity(
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
            if window_check_flag(window, WindowFlag::MINIMIZE) {
                return Some(window_id);
            }
            if window_check_flag(window, WindowFlag::FULLSCREEN) {
                return Some(window_id);
            }
            if window_check_rule_flag(window, WindowRuleFlag::MANAGED) {
                return Some(window_id);
            }

            if window_check_rule_flag(window, WindowRuleFlag::FULLSCREEN) {
                window_clear_rule_flag(window, WindowRuleFlag::FULLSCREEN);
                return Some(window_id);
            }

            if window_is_sticky(window.id)
                || !window_can_move(window)
                || !window_is_standard(window)
                || !window_level_is_standard(window)
                || (!window_can_resize(window) && window_is_undersized(window))
            {
                window_set_flag(window, WindowFlag::FLOAT);
            }
        } else {
            crate::debug!(
                "{} ignoring incorrectly marked window {} {}\n",
                "window_manager_create_and_add_window",
                or_null(application_name.as_deref()),
                window_id.0 as i32
            );
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window_set_flag(window, WindowFlag::FLOAT);
            }

            //
            // NOTE(asmvik): Print window information when debug_output is enabled.
            // Useful for identifying and creating rules if this window should in fact be managed.
            //

            if g_verbose() {
                let mut response = Response::to_standard_output();
                response.write(format_args!("window info: \n"));
                window_serialize(
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
            "window_manager_create_and_add_window",
            or_null(application_name.as_deref()),
            window_id.0 as i32
        );
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_flag(window, WindowFlag::FLOAT);
        }

        //
        // NOTE(asmvik): Print window information when debug_output is enabled.
        //

        if g_verbose() {
            let mut response = Response::to_standard_output();
            response.write(format_args!("window info: \n"));
            window_serialize(
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

pub(crate) fn window_manager_add_application_windows(
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
    let Some(window_list) = application_window_list(application) else {
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

        let window_id = ax_window_id(window_ref);
        if window_id == 0
            || window_manager_find_window(window_manager, WindowId(window_id)).is_some()
        {
            continue;
        }

        let retained_window_ref =
            CFRetained::into_raw(unsafe { CFRetained::retain(NonNull::from(window_ref)) })
                .as_ptr()
                .cast_const();
        let window = window_manager_create_and_add_window(
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
        if RuleFlag(window_manager.rules[index as usize].flags).contains(RuleFlag::ONE_SHOT_REMOVE)
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

pub(crate) fn window_manager_existing_application_window_list(
    process_id: Option<ProcessId>,
    window_manager: &mut WindowManager,
) -> Option<Vec<WindowId>> {
    let display_list = display_manager_active_display_list();

    let mut space_list: Option<Vec<SpaceId>> = None;

    for index in 0..display_list.len() {
        let Some(list) = display_space_list(display_list[index]) else {
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
    space_window_list_for_connection(&space_list, connection_id, true, window_manager)
}

pub(crate) fn window_manager_add_existing_application_windows(
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
        window_manager_existing_application_window_list(Some(process_id), window_manager)
    else {
        return result;
    };
    let global_window_count = global_window_list.len() as i32;

    let Some(application) = window_manager.application.find(&process_id) else {
        return result;
    };
    let application_name = Arc::clone(&application.name);
    let application_process_id = application.process_id;
    let window_list_ref = application_window_list(application);
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
            let window_id = window_ref.map_or(0, |window_ref| ax_window_id(window_ref));

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

            if window_manager_find_window(window_manager, WindowId(window_id)).is_none()
                && let Some(window_ref) = window_ref
            {
                let retained_window_ref =
                    CFRetained::into_raw(unsafe { CFRetained::retain(NonNull::from(window_ref)) })
                        .as_ptr()
                        .cast_const();
                window_manager_create_and_add_window(
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
                let window = window_manager_find_window(window_manager, global_window_list[index]);
                if window.is_none() {
                    missing_window = true;
                    application_window_list.push(global_window_list[index]);
                }
            }

            if missing_window {
                crate::debug!(
                    "{}: {} has {} windows that are not yet resolved, attempting workaround\n",
                    "window_manager_add_existing_application_windows",
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
                                let element_window_id = element_ref
                                    .as_deref()
                                    .map_or(0, |element_ref| ax_window_id(element_ref));
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
                                    window_manager_create_and_add_window(
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
                    "window_manager_add_existing_application_windows",
                    application_name
                );
                window_manager.applications_to_refresh.push(process_id);
            } else {
                crate::debug!(
                    "{}: workaround resolved all windows for {}\n",
                    "window_manager_add_existing_application_windows",
                    application_name
                );
            }
        } else {
            let mut missing_window = false;

            for index in 0..global_window_count as usize {
                let window = window_manager_find_window(window_manager, global_window_list[index]);
                if window.is_none() {
                    missing_window = true;
                    break;
                }
            }

            if !missing_window {
                crate::debug!(
                    "{}: all windows for {} are now resolved\n",
                    "window_manager_add_existing_application_windows",
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
            "window_manager_add_existing_application_windows",
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

pub(crate) fn space_manager_refresh_application_windows(
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
            "space_manager_refresh_application_windows",
            application_name.as_deref().unwrap_or("(null)")
        );
        let result = window_manager_add_existing_application_windows(
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

pub(crate) fn window_manager_begin(
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
            if workspace_application_is_observable(&process) {
                let mut application = application_create(&process);

                if application_observe(&mut application) {
                    let application_process_id = application.process_id;
                    window_manager_add_application(window_manager, application);
                    window_manager_add_existing_application_windows(
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
                    application_unobserve(&mut application);
                    application_destroy(application);
                }
            } else {
                crate::debug!(
                    "{}: {} ({}) is not observable, subscribing to activationPolicy changes\n",
                    "window_manager_begin",
                    process.name,
                    process.process_id.0
                );
                workspace_application_observe_activation_policy(
                    WORKSPACE_CONTEXT.get().unwrap(),
                    &process,
                );
            }
        }
    });

    let window = window_manager_focused_window(window_manager);
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
        window_manager_set_window_opacity(
            window_manager,
            window_id,
            window_manager.active_window_opacity,
        );
    }
}
