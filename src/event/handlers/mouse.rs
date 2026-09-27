#![allow(deprecated)]

use std::sync::atomic::Ordering;

use objc2_core_graphics::CGMouseButton;

use crate::debug;
use crate::display::bounds::display_bounds_constrained;
use crate::display::focus::{
    display_manager_focus_display_with_window_at_point, display_manager_set_active_display_id,
};
use crate::display::identity::display_manager_point_display_id;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_id;
use crate::ffi::accessibility::{kAXDrawerRole, kAXSheetRole};
use crate::ffi::carbon_core::{read_os_freq, read_os_timer};
use crate::ffi::core_foundation::{
    CFArrayGetCount, CFEqual, CFIndex, CFNumber, CGPoint, SendCFRetained, as_cftype,
    cfarray_borrow_value_at_index, cfnumber_read_i32, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventGetIntegerValueField, CGEventGetLocation, CGRectContainsRect,
    CGRectGetMidX, CGRectGetMidY,
};
use crate::ffi::skylight::SLSCopyAssociatedWindows;
use crate::layout::insertion::{insert_feedback_destroy, insert_feedback_show};
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit, view_find_window_node};
use crate::mouse::drag::{MouseDragState, MouseWindowInfo, mouse_window_info_populate};
use crate::mouse::drop::{
    MouseDropAction, mouse_determine_drop_action, mouse_drop_action_stack, mouse_drop_action_swap,
    mouse_drop_action_warp, mouse_drop_no_target, mouse_drop_try_adjust_bsp_grid,
};
use crate::mouse::tap::{MOUSE_TAP_STATE, MouseMod, MouseMode};
use crate::scripting_addition::client::scripting_addition_move_window;
use crate::space::managed_space::space_window_list;
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::state::mission_control_mode::{MissionControlMode, mission_control_is_active};
use crate::state::process_wide::{CONNECTION, LAST_GESTURE_TIME, PENDING_GESTURE};
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, STACK};
use crate::support::geometry::cgrect_contains_point;
use crate::support::handles::WindowId;
use crate::support::resize_handle::ResizeHandle;
use crate::window::focus::{
    window_manager_focus_window_with_raise, window_manager_focus_window_without_raise,
};
use crate::window::frame::{
    window_manager_move_window, window_manager_resize_window_relative_internal,
};
use crate::window::manager::{
    FfmMode, WindowManager, window_manager_find_managed_window, window_manager_find_window,
    window_manager_is_window_eligible,
};
use crate::window::model::{
    WindowFlag, window_check_flag, window_level, window_role, window_sub_level,
};
use crate::window::screen_lookup::{
    window_manager_find_window_at_point, window_manager_find_window_at_point_filtering_window,
};

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
