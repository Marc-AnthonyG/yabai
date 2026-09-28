#![allow(deprecated)]

use std::sync::atomic::Ordering;

use objc2_core_graphics::CGMouseButton;

use crate::debug;
use crate::display::bounds::query_bounds_of_display_left_for_windows;
use crate::display::identity::query_display_at_point;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::ffi::carbon_core::{read_system_clock_in_nanoseconds, system_clock_ticks_per_second};
use crate::ffi::core_foundation::{CFRetainedAssumedSendAndSync, CGPoint};
use crate::ffi::core_graphics::{
    CGEvent, CGEventCreate, CGEventField, CGEventGetIntegerValueField, CGEventGetLocation,
    CGRectGetMidX, CGRectGetMidY,
};
use crate::layout::group_header::{
    front_window_of_the_group_whose_header_holds_point, window_whose_group_header_tab_holds_point,
};
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
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::state::process_wide::{
    DOCK_SWIPE_GESTURE_IS_IN_PROGRESS, LAST_DOCK_SWIPE_GESTURE_END_TIME,
};
use crate::support::direction::{
    DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_STACK_INSTEAD_OF_SPLIT,
    DIRECTION_WEST,
};
use crate::support::resize_handle::ResizeHandle;
use crate::window::focus::focus_and_raise_tracked_window;
use crate::window::focus_follows_mouse::focus_the_window_at_point_the_way_focus_follows_mouse_does;
use crate::window::frame::{
    move_window_through_accessibility, resize_floating_window_by_dragging_edges,
};
use crate::window::manager::{FocusFollowsMouseMode, WindowManager, space_managing_window};
use crate::window::model::{WindowFlag, is_window_flag_set};
use crate::window::screen_lookup::{
    query_tracked_window_at_point, query_tracked_window_at_point_skipping_window,
};

pub(crate) fn handle_mouse_down_event(
    event: CFRetainedAssumedSendAndSync<CGEvent>,
    event_modifier: MouseModifier,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
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

    let button =
        CGEventGetIntegerValueField(Some(event.as_ref()), CGEventField::MouseEventButtonNumber);
    let is_a_plain_left_click = button == CGMouseButton::Left.0 as i64
        && MOUSE_TAP_STATE.modifier.load(Ordering::Relaxed) != event_modifier.0;
    if is_a_plain_left_click
        && let Some(window_of_the_clicked_tab) =
            window_whose_group_header_tab_holds_point(point, space_manager)
    {
        focus_and_raise_tracked_window(window_manager, window_of_the_clicked_tab);
        return;
    }

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
                let front_window_of_the_group_whose_header_is_under_the_cursor =
                    front_window_of_the_group_whose_header_holds_point(point, space_manager);
                if front_window_of_the_group_whose_header_is_under_the_cursor.is_some() {
                    window = front_window_of_the_group_whose_header_is_under_the_cursor;
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

                    let drop_action =
                        if front_window_of_the_group_whose_header_is_under_the_cursor.is_some() {
                            MouseDropAction::Stack
                        } else {
                            determine_drop_action_for_dragged_window(
                                source_view,
                                a_node,
                                destination_view,
                                b_node,
                                window,
                                point,
                                window_manager,
                                space_manager,
                            )
                        };
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
        let front_window_of_the_group_whose_header_is_under_the_cursor =
            front_window_of_the_group_whose_header_holds_point(point, space_manager);
        if front_window_of_the_group_whose_header_is_under_the_cursor.is_some() {
            window = front_window_of_the_group_whose_header_is_under_the_cursor;
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
            let drop_action =
                if front_window_of_the_group_whose_header_is_under_the_cursor.is_some() {
                    MouseDropAction::Stack
                } else {
                    determine_drop_action_for_dragged_window(
                        source_view,
                        a_node,
                        destination_view,
                        b_node,
                        window,
                        point,
                        window_manager,
                        space_manager,
                    )
                };
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

    focus_the_window_at_point_the_way_focus_follows_mouse_does(
        CGEventGetLocation(Some(event.as_ref())),
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}

pub(crate) fn handle_focus_follows_mouse_under_the_still_cursor_event(
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
    if mouse_drag_state.ffm_window_id.0 != 0 || mouse_drag_state.window_id.is_some() {
        return;
    }
    if DOCK_SWIPE_GESTURE_IS_IN_PROGRESS.load(Ordering::Relaxed) {
        return;
    }
    let Some(event_carrying_the_cursor_location) = CGEventCreate(None) else {
        return;
    };

    focus_the_window_at_point_the_way_focus_follows_mouse_does(
        CGEventGetLocation(Some(&event_carrying_the_cursor_location)),
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}
