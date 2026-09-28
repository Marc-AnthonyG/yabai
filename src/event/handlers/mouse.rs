#![allow(deprecated)]

use std::sync::atomic::Ordering;

use objc2_core_graphics::CGMouseButton;

use crate::debug;
use crate::display::bounds::query_bounds_of_display_left_for_windows;
use crate::display::focus::{focus_window_under_point, move_active_menu_bar_to_display};
use crate::display::identity::query_display_at_point;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::ffi::accessibility::{kAXDrawerRole, kAXSheetRole};
use crate::ffi::carbon_core::{read_system_clock_in_nanoseconds, system_clock_ticks_per_second};
use crate::ffi::core_foundation::{
    CFArrayGetCount, CFEqual, CFIndex, CFNumber, CFRetainedAssumedSendAndSync, CGPoint, as_cftype,
    cfarray_borrow_value_at_index, cfnumber_read_i32, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventGetIntegerValueField, CGEventGetLocation, CGRectContainsRect,
    CGRectGetMidX, CGRectGetMidY,
};
use crate::ffi::skylight::SLSCopyAssociatedWindows;
use crate::layout::insertion::{destroy_insert_feedback_of_node, show_insert_feedback_of_node};
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit, leaf_holding_window};
use crate::mouse::drag::{
    DraggedWindowFrameDelta, MouseDragState, measure_dragged_window_frame_delta,
};
use crate::mouse::drop::{
    MouseDropAction, adjust_split_ratios_to_mouse_moved_window_or_restore_its_frame,
    determine_drop_action_for_dragged_window, retile_window_dropped_on_no_target_window,
    stack_dropped_window_onto_destination_window, swap_dropped_window_with_destination_window,
    warp_dropped_window_beside_destination_window,
};
use crate::mouse::tap::{MOUSE_TAP_STATE, MouseMode, MouseModifier};
use crate::scripting_addition::client::move_window_through_scripting_addition;
use crate::space::managed_space::query_windows_on_space;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::state::process_wide::{
    DOCK_SWIPE_GESTURE_IS_IN_PROGRESS, LAST_DOCK_SWIPE_GESTURE_END_TIME, SKYLIGHT_CONNECTION_ID,
};
use crate::support::direction::{
    DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_STACK_INSTEAD_OF_SPLIT,
    DIRECTION_WEST,
};
use crate::support::geometry::is_point_inside_rectangle_including_its_edges;
use crate::support::handles::WindowId;
use crate::support::resize_handle::ResizeHandle;
use crate::window::focus::{
    focus_and_raise_window_of_process, focus_window_of_process_without_raising_it,
};
use crate::window::frame::{
    move_window_through_accessibility, resize_floating_window_by_dragging_edges,
};
use crate::window::manager::{
    FocusFollowsMouseMode, WindowManager, is_window_eligible_for_management, space_managing_window,
    tracked_window_with_id,
};
use crate::window::model::{
    WindowFlag, is_window_flag_set, query_window_level_from_window_server,
    query_window_sub_level_from_window_server, window_role,
};
use crate::window::screen_lookup::{
    query_tracked_window_at_point, query_tracked_window_at_point_skipping_window,
};

pub(crate) fn handle_mouse_down_event(
    event: CFRetainedAssumedSendAndSync<CGEvent>,
    event_modifier: MouseModifier,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if is_mission_control_active(mission_control_mode) {
        return;
    }
    if mouse_drag_state.current_action != MouseMode::None {
        return;
    }

    let point = CGEventGetLocation(Some(event.as_ref()));
    debug!(
        "{}: {:.2}, {:.2}\n",
        "handle_mouse_down_event", point.x as f64, point.y as f64
    );

    let window = query_tracked_window_at_point(window_manager, point);
    let Some(window) = window else {
        return;
    };
    let Some(window_record) = window_manager.window.find(&window) else {
        return;
    };
    if is_window_flag_set(window_record, WindowFlag::IN_NATIVE_FULLSCREEN) {
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

pub(crate) fn handle_mouse_up_event(
    event: CFRetainedAssumedSendAndSync<CGEvent>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    'set_current_action_to_none: {
        if is_mission_control_active(mission_control_mode) {
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
                    "handle_mouse_up_event", mouse_window.0 as i32
                );
                break 'clear_mouse_state_window;
            }

            let is_fullscreen = window_manager
                .window
                .find(&mouse_window)
                .is_some_and(|window| is_window_flag_set(window, WindowFlag::IN_NATIVE_FULLSCREEN));
            if is_fullscreen {
                debug!(
                    "{}: {} is transitioning into native-fullscreen mode, ignoring event..\n",
                    "handle_mouse_up_event", mouse_window.0 as i32
                );
                break 'clear_mouse_state_window;
            }

            let point = CGEventGetLocation(Some(event.as_ref()));
            debug!(
                "{}: {:.2}, {:.2}\n",
                "handle_mouse_up_event", point.x as f64, point.y as f64
            );

            let source_view = space_managing_window(window_manager, mouse_window);
            let Some(source_view) = source_view else {
                break 'clear_mouse_state_window;
            };

            let mut info = DraggedWindowFrameDelta::default();
            measure_dragged_window_frame_delta(mouse_drag_state, &mut info, window_manager);

            if info.changed_position && !info.changed_size {
                let cursor_space_id = query_current_space_of_display(query_display_at_point(point));
                let destination_view = find_or_create_view_for_space(
                    space_manager,
                    cursor_space_id,
                    display_manager,
                    window_manager,
                );

                let mut window = query_tracked_window_at_point_skipping_window(
                    window_manager,
                    point,
                    mouse_window,
                );
                if window.is_none() {
                    window = query_tracked_window_at_point(window_manager, point);
                }
                if window == Some(mouse_window) {
                    window = None;
                }

                let a_node = leaf_holding_window(space_manager, source_view, mouse_window);
                let b_node = match window {
                    Some(window) => leaf_holding_window(space_manager, destination_view, window),
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
                        destroy_insert_feedback_of_node(
                            feedback_space_id,
                            feedback_node_id,
                            window_manager,
                            space_manager,
                        );
                        mouse_drag_state.feedback_node = None;
                    }

                    let drop_action = determine_drop_action_for_dragged_window(
                        source_view,
                        a_node,
                        window,
                        point,
                        window_manager,
                        space_manager,
                    );
                    match drop_action {
                        MouseDropAction::Stack => {
                            stack_dropped_window_onto_destination_window(
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
                            swap_dropped_window_with_destination_window(
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
                            warp_dropped_window_beside_destination_window(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::Horizontal,
                                WindowNodeChild::First,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::WarpRight => {
                            warp_dropped_window_beside_destination_window(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::Vertical,
                                WindowNodeChild::Second,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::WarpBottom => {
                            warp_dropped_window_beside_destination_window(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::Horizontal,
                                WindowNodeChild::Second,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::WarpLeft => {
                            warp_dropped_window_beside_destination_window(
                                window_manager,
                                space_manager,
                                source_view,
                                a_node,
                                mouse_window,
                                destination_view,
                                b_node,
                                window,
                                WindowNodeSplit::Vertical,
                                WindowNodeChild::First,
                                display_manager,
                                mouse_drag_state,
                            );
                        }
                        MouseDropAction::None => { /* silence compiler warning.. */ }
                    }
                } else if let Some(a_node) = a_node {
                    retile_window_dropped_on_no_target_window(
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
                adjust_split_ratios_to_mouse_moved_window_or_restore_its_frame(
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

pub(crate) fn handle_mouse_dragged_event(
    event: CFRetainedAssumedSendAndSync<CGEvent>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if is_mission_control_active(mission_control_mode) {
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
            "handle_mouse_dragged_event", mouse_window.0 as i32
        );
        mouse_drag_state.window_id = None;
        mouse_drag_state.current_action = MouseMode::None;
        drop(event);
        return;
    }

    let point = CGEventGetLocation(Some(event.as_ref()));
    debug!(
        "{}: {:.2}, {:.2}\n",
        "handle_mouse_dragged_event", point.x as f64, point.y as f64
    );

    if mouse_drag_state.current_action == MouseMode::Move {
        let mut new_point = CGPoint {
            x: mouse_drag_state.window_frame.origin.x
                + (point.x - mouse_drag_state.down_location.x),
            y: mouse_drag_state.window_frame.origin.y
                + (point.y - mouse_drag_state.down_location.y),
        };

        let display_id = query_display_at_point(new_point);
        if display_id.0 != 0 {
            let bounds =
                query_bounds_of_display_left_for_windows(display_id, false, display_manager);
            if new_point.y < bounds.origin.y {
                new_point.y = bounds.origin.y;
            }
        }

        if !move_window_through_scripting_addition(
            mouse_window,
            new_point.x as i32,
            new_point.y as i32,
        ) {
            move_window_through_accessibility(
                mouse_window,
                new_point.x as f32,
                new_point.y as f32,
                window_manager,
            );
        }
    } else if mouse_drag_state.current_action == MouseMode::Resize {
        let event_time = read_system_clock_in_nanoseconds();
        let delta_time = (event_time as f32 - mouse_drag_state.last_moved_time as f32)
            * (1000.0f32 / system_clock_ticks_per_second() as f32);
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
        resize_floating_window_by_dragging_edges(
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

    let source_view = space_managing_window(window_manager, mouse_window);
    let Some(source_view) = source_view else {
        return;
    };

    let mut info = DraggedWindowFrameDelta::default();
    measure_dragged_window_frame_delta(mouse_drag_state, &mut info, window_manager);

    if info.changed_position && !info.changed_size {
        let cursor_space_id = query_current_space_of_display(query_display_at_point(point));
        let destination_view = find_or_create_view_for_space(
            space_manager,
            cursor_space_id,
            display_manager,
            window_manager,
        );

        let mut window =
            query_tracked_window_at_point_skipping_window(window_manager, point, mouse_window);
        if window.is_none() {
            window = query_tracked_window_at_point(window_manager, point);
        }
        if window == Some(mouse_window) {
            window = None;
        }

        let a_node = leaf_holding_window(space_manager, source_view, mouse_window);
        let b_node = match window {
            Some(window) => leaf_holding_window(space_manager, destination_view, window),
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
                destroy_insert_feedback_of_node(
                    feedback_space_id,
                    feedback_node_id,
                    window_manager,
                    space_manager,
                );
            }

            let mut insert_direction = 0;
            let drop_action = determine_drop_action_for_dragged_window(
                source_view,
                a_node,
                window,
                point,
                window_manager,
                space_manager,
            );
            match drop_action {
                MouseDropAction::Stack => {
                    insert_direction = DIRECTION_STACK_INSTEAD_OF_SPLIT;
                }
                MouseDropAction::Swap => {
                    insert_direction = DIRECTION_STACK_INSTEAD_OF_SPLIT;
                }
                MouseDropAction::WarpTop => {
                    insert_direction = DIRECTION_NORTH;
                }
                MouseDropAction::WarpRight => {
                    insert_direction = DIRECTION_EAST;
                }
                MouseDropAction::WarpBottom => {
                    insert_direction = DIRECTION_SOUTH;
                }
                MouseDropAction::WarpLeft => {
                    insert_direction = DIRECTION_WEST;
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
                if insert_direction == 0 {
                    destroy_insert_feedback_of_node(
                        destination_view,
                        b_node,
                        window_manager,
                        space_manager,
                    );
                    if mouse_drag_state.feedback_node == Some((destination_view, b_node)) {
                        mouse_drag_state.feedback_node = None;
                    }
                } else {
                    show_insert_feedback_of_node(
                        destination_view,
                        b_node,
                        window_manager,
                        space_manager,
                    );
                    mouse_drag_state.feedback_node = Some((destination_view, b_node));
                }
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
                destroy_insert_feedback_of_node(
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

pub(crate) fn handle_mouse_moved_event(
    event: CFRetainedAssumedSendAndSync<CGEvent>,
    _event_modifier: MouseModifier,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if window_manager.focus_follows_mouse_mode == FocusFollowsMouseMode::Disabled {
        return;
    }
    if is_mission_control_active(mission_control_mode) {
        return;
    }
    if mouse_drag_state.ffm_window_id.0 != 0 {
        return;
    }

    if DOCK_SWIPE_GESTURE_IS_IN_PROGRESS.load(Ordering::Relaxed) {
        return;
    }
    let last_gesture_time = LAST_DOCK_SWIPE_GESTURE_END_TIME.load(Ordering::Relaxed);
    let delta_time = (read_system_clock_in_nanoseconds() as f32 - last_gesture_time as f32)
        * (1000.0f32 / system_clock_ticks_per_second() as f32);
    if delta_time < 1250.0f32 {
        return;
    }

    let point = CGEventGetLocation(Some(event.as_ref()));
    let window = query_tracked_window_at_point(window_manager, point);

    if let Some(mut window) = window {
        if window == window_manager.focused_window_id {
            return;
        }
        if !is_window_eligible_for_management(window, window_manager) {
            return;
        }

        if window_manager.focus_follows_mouse_mode == FocusFollowsMouseMode::Autofocus {
            //
            // NOTE(asmvik): Look for a window with role AXSheet or AXDrawer
            // and forward focus to it because we are not allowed to focus the main
            // window in these cases.
            //

            let window_list = unsafe {
                take_create_rule_result(SLSCopyAssociatedWindows(
                    *SKYLIGHT_CONNECTION_ID.get().unwrap(),
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
                    let child = tracked_window_with_id(window_manager, child_window_id);
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
            focus_window_of_process_without_raising_it(
                &window_process_serial_number,
                window,
                window_manager,
            );
            mouse_drag_state.ffm_window_id = window;
        } else if window_manager.focus_follows_mouse_mode == FocusFollowsMouseMode::Autoraise {
            //
            // NOTE(asmvik): If any **floating** window would be fully occluded by
            // autoraising the window below the cursor we do not actually perform the
            // focus change, as it is likely that the user is trying to reach for the
            // smaller window that sits on top of the window we would otherwise raise.
            //

            let mut occludes_window = false;

            let window_list =
                query_windows_on_space(space_manager.current_space_id, false, window_manager);

            if let Some(window_list) = window_list {
                let window_count = window_list.len() as i32;
                for index in 0..window_count {
                    let window_id = window_list[index as usize];
                    if window_id == window {
                        break;
                    }

                    let sub_window = tracked_window_with_id(window_manager, window_id);
                    let Some(sub_window) = sub_window else {
                        continue;
                    };
                    let Some(sub_window_record) = window_manager.window.find(&sub_window) else {
                        continue;
                    };

                    if !is_window_flag_set(sub_window_record, WindowFlag::FLOATING) {
                        continue;
                    }
                    if query_window_level_from_window_server(window)
                        != query_window_level_from_window_server(sub_window)
                    {
                        continue;
                    }
                    if query_window_sub_level_from_window_server(window)
                        != query_window_sub_level_from_window_server(sub_window)
                    {
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
                focus_and_raise_window_of_process(
                    &window_process_serial_number,
                    window,
                    window_element_ref,
                );
                mouse_drag_state.ffm_window_id = window;
            }
        }
    } else {
        let cursor_display_id = query_display_at_point(point);
        if display_manager.current_display_id == cursor_display_id {
            return;
        }

        let bounds =
            query_bounds_of_display_left_for_windows(cursor_display_id, false, display_manager);
        if !is_point_inside_rectangle_including_its_edges(bounds, point) {
            return;
        }

        let window_id = focus_window_under_point(point, window_manager);
        if window_id.0 == 0 {
            move_active_menu_bar_to_display(cursor_display_id);
        }
        mouse_drag_state.ffm_window_id = window_id;
    }
}
