use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::application::model::application_is_frontmost;
use crate::debug;
use crate::display::manager::DisplayManager;
use crate::event::queue::{Event, event_loop_post};
use crate::ffi::accessibility::{AXUIElement, AXUIElementRef, ax_window_id, ax_window_pid};
use crate::ffi::core_foundation::{CFRetained, SendCFRetained};
use crate::ffi::core_graphics::{CGPointEqualToPoint, CGRectEqualToRect};
use crate::ffi::skylight::{SLSOrderWindow, SLSSpaceSetFrontPSN};
use crate::layout::area::ax_diff;
use crate::layout::settings::ViewFlag;
use crate::layout::tree::{view_find_window_node, window_node_flush};
use crate::mouse::drag::MouseDragState;
use crate::mouse::tap::MouseMode;
use crate::notifications::window::{update_window_notifications, window_unobserve};
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::signal::queue::{PendingSignal, SignalContext, event_signal_push};
use crate::space::focus::{space_manager_active_space, space_manager_focus_space_using_gesture};
use crate::space::lookup::space_manager_cursor_space;
use crate::space::managed_space::{
    space_display_id, space_is_visible, space_manager_is_window_on_space,
};
use crate::space::manager::SpaceManager;
use crate::space::tiling::{
    space_manager_tile_window_on_space, space_manager_tile_window_on_space_with_insertion_point,
    space_manager_untile_window,
};
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::{CONNECTION, PENDING_WINDOW_FOCUS};
use crate::support::handles::{ProcessId, WindowId};
use crate::support::log::or_null;
use crate::support::macos_version::{workspace_is_macos_sequoia, workspace_is_macos_tahoe};
use crate::window::discovery::window_manager_create_and_add_window;
use crate::window::focus::window_did_receive_focus;
use crate::window::fullscreen::window_manager_wait_for_native_fullscreen_transition;
use crate::window::manager::{
    WindowManager, WindowOriginMode, window_manager_add_lost_focused_event,
    window_manager_add_managed_window, window_manager_find_application,
    window_manager_find_lost_focused_event, window_manager_find_managed_window,
    window_manager_find_window, window_manager_is_window_eligible,
    window_manager_remove_lost_focused_event, window_manager_remove_managed_window,
    window_manager_remove_window, window_manager_should_manage_window,
};
use crate::window::model::{
    WindowFlag, window_ax_can_move, window_ax_can_resize, window_ax_frame, window_ax_origin,
    window_ax_role, window_ax_subrole, window_check_flag, window_clear_flag, window_destroy,
    window_is_fullscreen, window_set_flag, window_space, window_title,
};
use crate::window::rule::RuleFlag;
use crate::window::scratchpad::window_manager_remove_scratchpad_for_window;
use crate::window::shadow::window_manager_purify_window;

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
        window_manager_wait_for_native_fullscreen_transition(window);

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
