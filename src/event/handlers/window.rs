use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::application::is_application_frontmost;
use crate::debug;
use crate::display::manager::DisplayManager;
use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::accessibility::{
    AXUIElement, AXUIElementRef, read_process_id_from_accessibility_element_memory,
    read_window_id_of_accessibility_element,
};
use crate::ffi::core_foundation::{CFRetained, CFRetainedAssumedSendAndSync};
use crate::ffi::core_graphics::{CGPointEqualToPoint, CGRectEqualToRect};
use crate::ffi::skylight::{SLSOrderWindow, SLSSpaceSetFrontPSN};
use crate::layout::area::is_difference_beyond_accessibility_rounding;
use crate::layout::group_area::area_given_to_the_windows_of_tile;
use crate::layout::group_header::{
    keep_the_group_header_right_above_the_front_window_of_the_group_holding,
    refresh_the_group_headers_of_the_view_managing_window,
};
use crate::layout::settings::ViewFlag;
use crate::layout::tree::{leaf_holding_window, move_windows_below_node_into_their_areas};
use crate::mouse::drag::MouseDragState;
use crate::mouse::tap::MouseMode;
use crate::notifications::window::{
    request_skylight_notifications_for_windows_that_need_them, stop_observing_window_notifications,
};
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{
    PendingSignal, SignalContext, queue_pending_signal_for_its_subscribers,
};
use crate::space::focus::{
    focus_space_with_synthesized_dock_swipes, query_current_space_of_the_focused_display,
};
use crate::space::lookup::query_current_space_of_display_under_the_cursor;
use crate::space::managed_space::{
    is_space_visible_on_its_display, is_window_on_space, query_display_holding_space,
};
use crate::space::manager::SpaceManager;
use crate::space::tiling::{
    tile_window_on_space, tile_window_on_space_preferring_insertion_point,
    untile_window_from_view_of_space,
};
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::{SKYLIGHT_CONNECTION_ID, WINDOW_FOCUS_NOTIFICATION_IS_PENDING};
use crate::support::handles::{ProcessId, WindowId};
use crate::support::log::text_or_printf_null_placeholder;
use crate::support::macos_version::{is_running_on_macos_sequoia, is_running_on_macos_tahoe};
use crate::window::discovery::track_newly_discovered_window_applying_its_rules;
use crate::window::focus::respond_to_window_receiving_focus;
use crate::window::focus_follows_mouse::schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles;
use crate::window::fullscreen::wait_until_native_fullscreen_transition_finishes;
use crate::window::manager::{
    WindowManager, WindowOriginDisplayMode,
    forget_focused_event_that_arrived_before_window_was_tracked, forget_managed_window,
    has_focused_event_arrived_before_window_was_tracked, is_window_eligible_for_management,
    record_focused_event_that_arrived_before_window_was_tracked,
    record_managed_window_on_space_updating_its_shadow, should_window_be_managed,
    space_managing_window, stop_tracking_window, tracked_application_with_process_id,
    tracked_window_with_id,
};
use crate::window::model::{
    WindowFlag, can_window_be_moved_through_accessibility,
    can_window_be_resized_through_accessibility, clear_window_flag,
    copy_window_role_through_accessibility, copy_window_subrole_through_accessibility,
    copy_window_title_through_accessibility, destroy_window_releasing_its_accessibility_element,
    is_window_flag_set, is_window_in_native_fullscreen_according_to_accessibility,
    query_space_holding_window, read_window_frame_through_accessibility,
    read_window_origin_through_accessibility, set_window_flag,
};
use crate::window::rule::RuleFlag;
use crate::window::scratchpad::remove_window_from_its_scratchpad;
use crate::window::shadow::apply_shadow_removal_mode_to_window;

pub(crate) fn handle_window_created_event(
    element_ref: CFRetainedAssumedSendAndSync<AXUIElement>,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let window_id = WindowId(read_window_id_of_accessibility_element(
        element_ref.as_ref(),
    ));
    if window_id.0 == 0 {
        return;
    }

    let existing_window = tracked_window_with_id(window_manager, window_id);
    if existing_window.is_some() {
        return;
    }

    let window_process_id = ProcessId(unsafe {
        read_process_id_from_accessibility_element_memory(element_ref.as_ref() as *const AXUIElement)
    });
    if window_process_id.0 == 0 {
        return;
    }

    let application = tracked_application_with_process_id(window_manager, window_process_id);
    let Some(application) = application else {
        return;
    };

    let window_ref: AXUIElementRef = CFRetained::into_raw(element_ref.0).as_ptr();
    let window = track_newly_discovered_window_applying_its_rules(
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
        if RuleFlag(window_manager.rules[index as usize].flags)
            .contains(RuleFlag::ONE_SHOT_DUE_FOR_REMOVAL)
        {
            window_manager.rules.swap_remove(index as usize);
            index -= 1;
            rule_len -= 1;
        }
        index += 1;
    }

    if should_window_be_managed(window, window_manager)
        && space_managing_window(window_manager, window).is_none()
    {
        let space_id;

        if window_manager.window_origin_display_mode
            == WindowOriginDisplayMode::DisplayTheWindowOpenedOn
        {
            space_id = query_space_holding_window(window);
        } else if window_manager.window_origin_display_mode
            == WindowOriginDisplayMode::FocusedDisplay
        {
            space_id = space_manager.current_space_id;
        } else
        /* if (g_window_manager.window_origin_mode == WINDOW_ORIGIN_CURSOR) */
        {
            space_id = query_current_space_of_display_under_the_cursor();
        }

        let view = tile_window_on_space(
            space_manager,
            window,
            space_id,
            display_manager,
            window_manager,
        );
        record_managed_window_on_space_updating_its_shadow(
            window_manager,
            window,
            space_manager,
            view,
        );
    }

    if is_window_eligible_for_management(window, window_manager) {
        queue_pending_signal_for_its_subscribers(
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

    if is_running_on_macos_sequoia() || is_running_on_macos_tahoe() {
        request_skylight_notifications_for_windows_that_need_them(window_manager, space_manager);
    }
}

pub(crate) fn handle_window_destroyed_event(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = match tracked_window_with_id(window_manager, window_id) {
        Some(window) if window.0 != 0 => window,
        _ => {
            debug!(
                "{}: window has already been destroyed, ignoring event..\n",
                "handle_window_destroyed_event"
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
        "handle_window_destroyed_event",
        application_name.as_deref().unwrap_or("<unknown>"),
        window.0 as i32
    );

    let view = space_managing_window(window_manager, window);
    if let Some(view) = view {
        untile_window_from_view_of_space(
            space_manager,
            view,
            window,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        forget_managed_window(window_manager, window);
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
        queue_pending_signal_for_its_subscribers(
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

    remove_window_from_its_scratchpad(
        window_manager,
        window,
        false,
        process_manager,
        display_manager,
        space_manager,
        mouse_drag_state,
    );
    let window = stop_tracking_window(window_manager, window);
    if let Some(mut window) = window {
        stop_observing_window_notifications(&mut window, window_manager);
        destroy_window_releasing_its_accessibility_element(window);
    }

    if is_running_on_macos_sequoia() || is_running_on_macos_tahoe() {
        request_skylight_notifications_for_windows_that_need_them(window_manager, space_manager);
    }
    schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles(window_manager);
}

pub(crate) fn handle_window_focused_event(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    WINDOW_FOCUS_NOTIFICATION_IS_PENDING.store(false, Ordering::Release);

    let window = tracked_window_with_id(window_manager, window_id);
    let Some(window) = window else {
        record_focused_event_that_arrived_before_window_was_tracked(window_manager, window_id);
        return;
    };

    let is_still_alive = window_manager
        .window
        .find(&window)
        .is_some_and(|window| window.liveness.is_still_alive());
    if !is_still_alive {
        debug!(
            "{}: {} has been marked invalid by the system, ignoring event..\n",
            "handle_window_focused_event", window_id.0 as i32
        );
        return;
    }

    let is_minimized = window_manager
        .window
        .find(&window)
        .is_some_and(|window| is_window_flag_set(window, WindowFlag::MINIMIZED));
    if is_minimized {
        record_focused_event_that_arrived_before_window_was_tracked(window_manager, window);
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
    if !is_application_frontmost(application) {
        return;
    }
    let application_name = Arc::clone(&application.name);
    let application_process_serial_number = application.process_serial_number;

    debug!(
        "{}: {} {}\n",
        "handle_window_focused_event", application_name, window.0 as i32
    );

    if space_manager.skip_window_focus_animation {
        let space_id = query_space_holding_window(window);
        if space_id.0 != 0 && !is_space_visible_on_its_display(space_id) {
            unsafe {
                SLSSpaceSetFrontPSN(
                    *SKYLIGHT_CONNECTION_ID.get().unwrap(),
                    space_id.0,
                    application_process_serial_number,
                )
            };
            focus_space_with_synthesized_dock_swipes(
                query_display_holding_space(space_id),
                space_id,
                window_manager,
            );
        }
    }

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
}

pub(crate) fn handle_window_moved_event(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = tracked_window_with_id(window_manager, window_id);
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
            "handle_window_moved_event", window_id.0 as i32
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
            "handle_window_moved_event", window_id.0 as i32
        );
        return;
    }

    let Some(window_record) = window_manager.window.find(&window) else {
        return;
    };
    let new_origin = read_window_origin_through_accessibility(window_record);
    if CGPointEqualToPoint(new_origin, window_record.frame.origin) {
        debug!(
            "{}:DEBOUNCED {} {}\n",
            "handle_window_moved_event",
            text_or_printf_null_placeholder(application_name.as_deref()),
            window.0 as i32
        );
        return;
    }

    debug!(
        "{}: {} {}\n",
        "handle_window_moved_event",
        text_or_printf_null_placeholder(application_name.as_deref()),
        window.0 as i32
    );
    queue_pending_signal_for_its_subscribers(
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
        clear_window_flag(window_record, WindowFlag::IN_WINDOWED_FULLSCREEN);

        if mouse_drag_state.window_id.is_none() || mouse_drag_state.window_id != Some(window) {
            let view = space_managing_window(window_manager, window);
            if let Some(view) = view {
                let node = leaf_holding_window(space_manager, view, window);
                if let Some(node) = node
                    && space_manager.view.find(&view).is_some_and(|view| {
                        view.find_node(node).is_some_and(|window_node| {
                            let node_window_area = area_given_to_the_windows_of_tile(
                                window_node.area,
                                view,
                                window_node,
                                window_manager,
                            );
                            (is_difference_beyond_accessibility_rounding(
                                node_window_area.x as f64,
                                new_origin.x,
                            ) || is_difference_beyond_accessibility_rounding(
                                node_window_area.y as f64,
                                new_origin.y,
                            )) && window_node.zoom.is_none_or(|zoom| {
                                view.find_node(zoom).is_some_and(|zoom| {
                                    let zoom_window_area = area_given_to_the_windows_of_tile(
                                        zoom.area,
                                        view,
                                        window_node,
                                        window_manager,
                                    );
                                    is_difference_beyond_accessibility_rounding(
                                        zoom_window_area.x as f64,
                                        new_origin.x,
                                    ) || is_difference_beyond_accessibility_rounding(
                                        zoom_window_area.y as f64,
                                        new_origin.y,
                                    )
                                })
                            })
                        })
                    })
                {
                    if is_space_visible_on_its_display(view) {
                        move_windows_below_node_into_their_areas(
                            view,
                            node,
                            window_manager,
                            space_manager,
                        );
                    } else if let Some(view) = space_manager.view.find_mut(&view) {
                        view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
                    }
                }
            }
        }
    }
}

pub(crate) fn handle_window_resized_event(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = tracked_window_with_id(window_manager, window_id);
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
            "handle_window_resized_event", window_id.0 as i32
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
            "handle_window_resized_event", window_id.0 as i32
        );
        return;
    }

    let Some(window_record) = window_manager.window.find(&window) else {
        return;
    };
    let new_frame = read_window_frame_through_accessibility(window_record);
    if CGRectEqualToRect(new_frame, window_record.frame) {
        debug!(
            "{}:DEBOUNCED {} {}\n",
            "handle_window_resized_event",
            text_or_printf_null_placeholder(application_name.as_deref()),
            window.0 as i32
        );
        return;
    }

    debug!(
        "{}: {} {}\n",
        "handle_window_resized_event",
        text_or_printf_null_placeholder(application_name.as_deref()),
        window.0 as i32
    );
    queue_pending_signal_for_its_subscribers(
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
    let was_fullscreen = is_window_flag_set(window_record, WindowFlag::IN_NATIVE_FULLSCREEN);

    let is_fullscreen = is_window_in_native_fullscreen_according_to_accessibility(window_record);
    if is_fullscreen {
        set_window_flag(window_record, WindowFlag::IN_NATIVE_FULLSCREEN);
    } else {
        clear_window_flag(window_record, WindowFlag::IN_NATIVE_FULLSCREEN);
    }

    if was_fullscreen != is_fullscreen {
        if can_window_be_moved_through_accessibility(window_record) {
            set_window_flag(window_record, WindowFlag::MOVABLE);
        } else {
            clear_window_flag(window_record, WindowFlag::MOVABLE);
        }

        if can_window_be_resized_through_accessibility(window_record) {
            set_window_flag(window_record, WindowFlag::RESIZABLE);
        } else {
            clear_window_flag(window_record, WindowFlag::RESIZABLE);
        }

        drop(window_record.role.take());
        let role = copy_window_role_through_accessibility(window_record);
        window_record.role = role;

        drop(window_record.subrole.take());
        let subrole = copy_window_subrole_through_accessibility(window_record);
        window_record.subrole = subrole;
    }

    let windowed_fullscreen = CGRectEqualToRect(window_record.windowed_frame, window_record.frame);
    window_record.frame = new_frame;

    if !was_fullscreen && is_fullscreen {
        let view = space_managing_window(window_manager, window);
        if let Some(view) = view {
            untile_window_from_view_of_space(
                space_manager,
                view,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            forget_managed_window(window_manager, window);
            apply_shadow_removal_mode_to_window(window_manager, window);
        }
    } else if was_fullscreen && !is_fullscreen {
        wait_until_native_fullscreen_transition_finishes(window);

        if should_window_be_managed(window, window_manager)
            && space_managing_window(window_manager, window).is_none()
        {
            let view = tile_window_on_space(
                space_manager,
                window,
                query_space_holding_window(window),
                display_manager,
                window_manager,
            );
            record_managed_window_on_space_updating_its_shadow(
                window_manager,
                window,
                space_manager,
                view,
            );
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
                clear_window_flag(window_record, WindowFlag::IN_WINDOWED_FULLSCREEN);
            }

            if mouse_drag_state.window_id.is_none() || mouse_drag_state.window_id != Some(window) {
                let view = space_managing_window(window_manager, window);
                if let Some(view) = view {
                    let node = leaf_holding_window(space_manager, view, window);
                    if let Some(node) = node
                        && space_manager.view.find(&view).is_some_and(|view| {
                            view.find_node(node).is_some_and(|window_node| {
                                let node_window_area = area_given_to_the_windows_of_tile(
                                    window_node.area,
                                    view,
                                    window_node,
                                    window_manager,
                                );
                                (is_difference_beyond_accessibility_rounding(
                                    node_window_area.x as f64,
                                    new_frame.origin.x,
                                ) || is_difference_beyond_accessibility_rounding(
                                    node_window_area.y as f64,
                                    new_frame.origin.y,
                                ) || is_difference_beyond_accessibility_rounding(
                                    node_window_area.width as f64,
                                    new_frame.size.width,
                                ) || is_difference_beyond_accessibility_rounding(
                                    node_window_area.height as f64,
                                    new_frame.size.height,
                                )) && window_node.zoom.is_none_or(|zoom| {
                                    view.find_node(zoom).is_some_and(|zoom| {
                                        let zoom_window_area = area_given_to_the_windows_of_tile(
                                            zoom.area,
                                            view,
                                            window_node,
                                            window_manager,
                                        );
                                        is_difference_beyond_accessibility_rounding(
                                            zoom_window_area.x as f64,
                                            new_frame.origin.x,
                                        ) || is_difference_beyond_accessibility_rounding(
                                            zoom_window_area.y as f64,
                                            new_frame.origin.y,
                                        ) || is_difference_beyond_accessibility_rounding(
                                            zoom_window_area.width as f64,
                                            new_frame.size.width,
                                        ) || is_difference_beyond_accessibility_rounding(
                                            zoom_window_area.height as f64,
                                            new_frame.size.height,
                                        )
                                    })
                                })
                            })
                        })
                    {
                        if is_space_visible_on_its_display(view) {
                            move_windows_below_node_into_their_areas(
                                view,
                                node,
                                window_manager,
                                space_manager,
                            );
                        } else if let Some(view) = space_manager.view.find_mut(&view) {
                            view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn handle_window_minimized_event(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = tracked_window_with_id(window_manager, window_id);
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
            "handle_window_minimized_event", window.0 as i32
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
        "handle_window_minimized_event",
        text_or_printf_null_placeholder(application_name.as_deref()),
        window.0 as i32
    );
    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };
    set_window_flag(window_record, WindowFlag::MINIMIZED);

    if can_window_be_moved_through_accessibility(window_record) {
        set_window_flag(window_record, WindowFlag::MOVABLE);
    } else {
        clear_window_flag(window_record, WindowFlag::MOVABLE);
    }

    if can_window_be_resized_through_accessibility(window_record) {
        set_window_flag(window_record, WindowFlag::RESIZABLE);
    } else {
        clear_window_flag(window_record, WindowFlag::RESIZABLE);
    }

    drop(window_record.role.take());
    let role = copy_window_role_through_accessibility(window_record);
    window_record.role = role;

    drop(window_record.subrole.take());
    let subrole = copy_window_subrole_through_accessibility(window_record);
    window_record.subrole = subrole;

    if window == window_manager.last_window_id {
        window_manager.last_window_id = window_manager.focused_window_id;
    }

    let view = space_managing_window(window_manager, window);
    if let Some(view) = view {
        untile_window_from_view_of_space(
            space_manager,
            view,
            window,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        forget_managed_window(window_manager, window);
        apply_shadow_removal_mode_to_window(window_manager, window);
    }

    queue_pending_signal_for_its_subscribers(
        SignalType::WindowMinimized,
        SignalContext::Window(window),
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
    );
    schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles(window_manager);
}

pub(crate) fn handle_window_deminimized_event(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let window = tracked_window_with_id(window_manager, window_id);
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
            "handle_window_deminimized_event", window.0 as i32
        );
        forget_focused_event_that_arrived_before_window_was_tracked(window_manager, window);
        return;
    }

    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };
    clear_window_flag(window_record, WindowFlag::MINIMIZED);

    if can_window_be_moved_through_accessibility(window_record) {
        set_window_flag(window_record, WindowFlag::MOVABLE);
    } else {
        clear_window_flag(window_record, WindowFlag::MOVABLE);
    }

    if can_window_be_resized_through_accessibility(window_record) {
        set_window_flag(window_record, WindowFlag::RESIZABLE);
    } else {
        clear_window_flag(window_record, WindowFlag::RESIZABLE);
    }

    drop(window_record.role.take());
    let role = copy_window_role_through_accessibility(window_record);
    window_record.role = role;

    drop(window_record.subrole.take());
    let subrole = copy_window_subrole_through_accessibility(window_record);
    window_record.subrole = subrole;

    let window_application = window_record.application;
    let application_name = window_application
        .and_then(|application| window_manager.application.find(&application))
        .map(|application| Arc::clone(&application.name));

    let space_id = query_current_space_of_the_focused_display(window_manager);
    if is_window_on_space(space_id, window) {
        debug!(
            "{}: window {} {} is deminimized on active space\n",
            "handle_window_deminimized_event",
            text_or_printf_null_placeholder(application_name.as_deref()),
            window.0 as i32
        );
        if should_window_be_managed(window, window_manager)
            && space_managing_window(window_manager, window).is_none()
        {
            let last_window = tracked_window_with_id(window_manager, window_manager.last_window_id);
            let last_window_application = last_window
                .and_then(|last_window| window_manager.window.find(&last_window))
                .and_then(|last_window| last_window.application);
            let insertion_point = match last_window {
                Some(last_window) if last_window_application != window_application => last_window,
                _ => WindowId(0),
            };
            let view = tile_window_on_space_preferring_insertion_point(
                space_manager,
                window,
                space_id,
                insertion_point,
                display_manager,
                window_manager,
            );
            record_managed_window_on_space_updating_its_shadow(
                window_manager,
                window,
                space_manager,
                view,
            );
        }
    } else {
        debug!(
            "{}: window {} {} is deminimized on inactive space\n",
            "handle_window_deminimized_event",
            text_or_printf_null_placeholder(application_name.as_deref()),
            window.0 as i32
        );
    }

    if has_focused_event_arrived_before_window_was_tracked(window_manager, window) {
        post_event_to_event_loop(Event::WindowFocused(window));
        forget_focused_event_that_arrived_before_window_was_tracked(window_manager, window);
    }

    queue_pending_signal_for_its_subscribers(
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

pub(crate) fn handle_window_title_changed_event(
    window_id: WindowId,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let window = tracked_window_with_id(window_manager, window_id);
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
            "handle_window_title_changed_event", window_id.0 as i32
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
        "handle_window_title_changed_event",
        text_or_printf_null_placeholder(application_name.as_deref()),
        window.0 as i32
    );

    let Some(window_record) = window_manager.window.find_mut(&window) else {
        return;
    };

    drop(window_record.title.take());

    let title = copy_window_title_through_accessibility(window_record);
    window_record.title = title;
    refresh_the_group_headers_of_the_view_managing_window(window, space_manager, window_manager);

    queue_pending_signal_for_its_subscribers(
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

pub(crate) fn handle_skylight_window_ordered_event(
    window_id: WindowId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    debug!(
        "{}: {}\n",
        "handle_skylight_window_ordered_event", window_id.0 as i32
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
                *SKYLIGHT_CONNECTION_ID.get().unwrap(),
                feedback_window_id,
                1,
                relative_window_id,
            )
        };
    }
    keep_the_group_header_right_above_the_front_window_of_the_group_holding(
        window_id,
        space_manager,
        window_manager,
    );
}

pub(crate) fn handle_skylight_window_destroyed_event(
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
        "handle_skylight_window_destroyed_event", window_id.0 as i32
    );

    let window = tracked_window_with_id(window_manager, window_id);
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
            "handle_skylight_window_destroyed_event", window_id.0 as i32
        );
        return;
    }

    handle_window_destroyed_event(
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
