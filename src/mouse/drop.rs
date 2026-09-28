use std::sync::atomic::Ordering;

use crate::display::manager::DisplayManager;
use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
use crate::ffi::core_graphics::CGRectContainsPoint;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::{
    MOST_WINDOWS_A_NODE_CAN_HOLD, WindowNodeChild, WindowNodeSplit,
    add_window_to_view_tree_preferring_insertion_point,
    collect_windows_below_node_with_their_target_areas, is_window_in_node, leaf_holding_window,
    move_windows_below_node_into_their_areas, remove_window_from_view_tree,
    stack_window_in_node_as_its_front_window, swap_windows_between_nodes_clearing_their_zoom,
};
use crate::mouse::drag::{DraggedWindowFrameDelta, MouseDragState};
use crate::mouse::tap::{MOUSE_TAP_STATE, MouseMode};
use crate::scripting_addition::client::order_window_relative_to_other_window_through_scripting_addition;
use crate::space::manager::SpaceManager;
use crate::space::tiling::{tile_window_on_space, untile_window_from_view_of_space};
use crate::support::geometry::is_point_strictly_inside_triangle;
use crate::support::handles::{NodeId, SpaceId, WindowId};
use crate::support::layer::LAYER_BELOW;
use crate::support::resize_handle::ResizeHandle;
use crate::window::animation::{
    WindowWithTargetFrame, move_window_to_its_target_frame_animating_if_enabled,
    move_windows_to_their_target_frames_animating_if_enabled,
};
use crate::window::frame::resize_window_by_dragging_edges_or_to_absolute_size;
use crate::window::layer::set_window_layer_unless_explicitly_set;
use crate::window::manager::{
    WindowManager, WindowOperationOutcome, forget_managed_window,
    record_managed_window_on_space_updating_its_shadow, tracked_window_with_id,
};
use crate::window::shadow::apply_shadow_removal_mode_to_window;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum MouseDropAction {
    None = 0,
    Stack = 1,
    Swap = 2,
    WarpTop = 3,
    WarpRight = 4,
    WarpBottom = 5,
    WarpLeft = 6,
}

pub(crate) fn determine_drop_action_for_dragged_window(
    source_space_id: SpaceId,
    source_node_id: NodeId,
    destination_window_id: WindowId,
    point: CGPoint,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> MouseDropAction {
    let Some(destination_window_frame) = window_manager
        .window
        .find(&destination_window_id)
        .map(|window| window.frame)
    else {
        return MouseDropAction::None;
    };
    let source_node_window_count = space_manager
        .view
        .find(&source_space_id)
        .and_then(|view| view.find_node(source_node_id))
        .map(|node| node.window_count);
    let drop_action_setting =
        MouseMode::from_discriminant(MOUSE_TAP_STATE.drop_action.load(Ordering::Relaxed));

    drop_action_for_point_over_window_frame(
        destination_window_frame,
        point,
        source_node_window_count,
        drop_action_setting,
    )
}

fn drop_action_for_point_over_window_frame(
    destination_window_frame: CGRect,
    point: CGPoint,
    source_node_window_count: Option<i32>,
    drop_action_setting: MouseMode,
) -> MouseDropAction {
    let point_relative_to_frame_origin = CGPoint {
        x: point.x - destination_window_frame.origin.x,
        y: point.y - destination_window_frame.origin.y,
    };
    let center_rect = CGRect {
        origin: CGPoint {
            x: 0.25f32 as f64 * destination_window_frame.size.width,
            y: 0.25f32 as f64 * destination_window_frame.size.height,
        },
        size: CGSize {
            width: 0.50f32 as f64 * destination_window_frame.size.width,
            height: 0.50f32 as f64 * destination_window_frame.size.height,
        },
    };
    let top_triangle: [CGPoint; 3] = [
        CGPoint {
            x: 0.0f32 as f64,
            y: 0.0f32 as f64,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: destination_window_frame.size.width,
            y: 0.0f32 as f64,
        },
    ];
    let right_triangle: [CGPoint; 3] = [
        CGPoint {
            x: destination_window_frame.size.width,
            y: 0.0f32 as f64,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: destination_window_frame.size.width,
            y: destination_window_frame.size.height,
        },
    ];
    let bottom_triangle: [CGPoint; 3] = [
        CGPoint {
            x: destination_window_frame.size.width,
            y: destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.0f32 as f64,
            y: destination_window_frame.size.height,
        },
    ];
    let left_triangle: [CGPoint; 3] = [
        CGPoint {
            x: 0.0f32 as f64,
            y: destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.0f32 as f64,
            y: 0.0f32 as f64,
        },
    ];

    if (CGRectContainsPoint(center_rect, point_relative_to_frame_origin))
        && (source_node_window_count == Some(1))
    {
        return if drop_action_setting == MouseMode::Stack {
            MouseDropAction::Stack
        } else {
            MouseDropAction::Swap
        };
    } else if is_point_strictly_inside_triangle(&top_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpTop;
    } else if is_point_strictly_inside_triangle(&right_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpRight;
    } else if is_point_strictly_inside_triangle(&bottom_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpBottom;
    } else if is_point_strictly_inside_triangle(&left_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpLeft;
    }

    MouseDropAction::None
}

pub(crate) fn stack_dropped_window_onto_destination_window(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_window_id: WindowId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    untile_window_from_view_of_space(
        space_manager,
        source_space_id,
        source_window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
    forget_managed_window(window_manager, source_window_id);

    let destination_node =
        leaf_holding_window(space_manager, destination_space_id, destination_window_id);
    let Some(destination_node) = destination_node else {
        return;
    };
    let Some(destination_node_window_count) = space_manager
        .view
        .find(&destination_space_id)
        .and_then(|view| view.find_node(destination_node))
        .map(|node| node.window_count)
    else {
        return;
    };
    if destination_node_window_count + 1 < MOST_WINDOWS_A_NODE_CAN_HOLD as i32 {
        stack_window_in_node_as_its_front_window(
            destination_space_id,
            destination_node,
            source_window_id,
            space_manager,
        );
        record_managed_window_on_space_updating_its_shadow(
            window_manager,
            source_window_id,
            space_manager,
            destination_space_id,
        );
        set_window_layer_unless_explicitly_set(source_window_id, LAYER_BELOW, window_manager);

        let Some(view) = space_manager.view.find(&destination_space_id) else {
            return;
        };
        let Some(node) = view.find_node(destination_node) else {
            return;
        };
        order_window_relative_to_other_window_through_scripting_addition(
            source_window_id,
            1,
            node.window_order[1],
        );

        let area = match node.zoom.and_then(|zoom| view.find_node(zoom)) {
            Some(zoom) => zoom.area,
            None => node.area,
        };
        move_window_to_its_target_frame_animating_if_enabled(
            WindowWithTargetFrame {
                window_id: source_window_id,
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height,
            },
            window_manager,
        );
    }
}

pub(crate) fn swap_dropped_window_with_destination_window(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_node_id: NodeId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_node_id: NodeId,
    destination_window_id: WindowId,
) {
    let source_view_insertion_point = space_manager
        .view
        .find(&source_space_id)
        .map_or(WindowId(0), |view| view.insertion_point);
    let destination_view_insertion_point = space_manager
        .view
        .find(&destination_space_id)
        .map_or(WindowId(0), |view| view.insertion_point);
    if is_window_in_node(
        source_space_id,
        source_node_id,
        source_view_insertion_point,
        space_manager,
    ) {
        if let Some(source_view) = space_manager.view.find_mut(&source_space_id) {
            source_view.insertion_point = destination_window_id;
        }
    } else if is_window_in_node(
        destination_space_id,
        destination_node_id,
        destination_view_insertion_point,
        space_manager,
    ) {
        if let Some(destination_view) = space_manager.view.find_mut(&destination_space_id) {
            destination_view.insertion_point = source_window_id;
        }
    }

    swap_windows_between_nodes_clearing_their_zoom(
        source_space_id,
        source_node_id,
        destination_space_id,
        destination_node_id,
        space_manager,
    );

    if source_space_id != destination_space_id {
        let source_node_window_list: Vec<WindowId> = space_manager
            .view
            .find(&source_space_id)
            .and_then(|view| view.find_node(source_node_id))
            .map_or(Vec::new(), |node| {
                node.window_list[..node.window_count as usize].to_vec()
            });
        for index in 0..source_node_window_list.len() {
            forget_managed_window(window_manager, source_node_window_list[index]);
            if let Some(window) =
                tracked_window_with_id(window_manager, source_node_window_list[index])
            {
                record_managed_window_on_space_updating_its_shadow(
                    window_manager,
                    window,
                    space_manager,
                    source_space_id,
                );
            }
        }

        let destination_node_window_list: Vec<WindowId> = space_manager
            .view
            .find(&destination_space_id)
            .and_then(|view| view.find_node(destination_node_id))
            .map_or(Vec::new(), |node| {
                node.window_list[..node.window_count as usize].to_vec()
            });
        for index in 0..destination_node_window_list.len() {
            forget_managed_window(window_manager, destination_node_window_list[index]);
            if let Some(window) =
                tracked_window_with_id(window_manager, destination_node_window_list[index])
            {
                record_managed_window_on_space_updating_its_shadow(
                    window_manager,
                    window,
                    space_manager,
                    destination_space_id,
                );
            }
        }
    }

    let mut window_list: Vec<WindowWithTargetFrame> = Vec::new();
    collect_windows_below_node_with_their_target_areas(
        source_space_id,
        source_node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    collect_windows_below_node_with_their_target_areas(
        destination_space_id,
        destination_node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    move_windows_to_their_target_frames_animating_if_enabled(&window_list, window_manager);
}

pub(crate) fn warp_dropped_window_beside_destination_window(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_node_id: NodeId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_node_id: NodeId,
    destination_window_id: WindowId,
    split: WindowNodeSplit,
    child: WindowNodeChild,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let source_node = space_manager
        .view
        .find(&source_space_id)
        .and_then(|view| view.find_node(source_node_id))
        .map(|node| (node.parent, node.window_count));
    let destination_node_parent = space_manager
        .view
        .find(&destination_space_id)
        .and_then(|view| view.find_node(destination_node_id))
        .and_then(|node| node.parent);
    let Some((source_node_parent, source_node_window_count)) = source_node else {
        return;
    };

    if (source_node_parent.is_some() && destination_node_parent.is_some())
        && ((source_space_id, source_node_parent)
            == (destination_space_id, destination_node_parent))
        && (source_node_window_count == 1)
    {
        let Some(destination_node_parent) = destination_node_parent else {
            return;
        };
        let Some(destination_node_parent) = space_manager
            .view
            .find_mut(&destination_space_id)
            .and_then(|view| view.find_node_mut(destination_node_parent))
        else {
            return;
        };
        if destination_node_parent.split == split {
            swap_dropped_window_with_destination_window(
                window_manager,
                space_manager,
                source_space_id,
                source_node_id,
                source_window_id,
                destination_space_id,
                destination_node_id,
                destination_window_id,
            );
            return;
        } else {
            destination_node_parent.split = split;
            destination_node_parent.child = child;
        }
    } else if let Some(destination_node) = space_manager
        .view
        .find_mut(&destination_space_id)
        .and_then(|view| view.find_node_mut(destination_node_id))
    {
        destination_node.split = split;
        destination_node.child = child;
    }

    let source_node_remove = remove_window_from_view_tree(
        space_manager,
        source_space_id,
        source_window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
    forget_managed_window(window_manager, source_window_id);
    apply_shadow_removal_mode_to_window(window_manager, source_window_id);

    let source_node_add = add_window_to_view_tree_preferring_insertion_point(
        space_manager,
        destination_space_id,
        source_window_id,
        destination_window_id,
        display_manager,
        window_manager,
    );
    record_managed_window_on_space_updating_its_shadow(
        window_manager,
        source_window_id,
        space_manager,
        destination_space_id,
    );

    let mut window_list: Vec<WindowWithTargetFrame> = Vec::new();

    if let Some(source_node_remove) = source_node_remove {
        collect_windows_below_node_with_their_target_areas(
            source_space_id,
            source_node_remove,
            &mut window_list,
            window_manager,
            space_manager,
        );
    }

    if let Some(source_node_add) = source_node_add {
        let source_node_add_parent = space_manager
            .view
            .find(&destination_space_id)
            .and_then(|view| view.find_node(source_node_add))
            .and_then(|node| node.parent);
        let source_node_remove = source_node_remove.map(|node_id| (source_space_id, node_id));
        if source_node_remove != Some((destination_space_id, source_node_add))
            && source_node_remove
                != source_node_add_parent.map(|node_id| (destination_space_id, node_id))
        {
            collect_windows_below_node_with_their_target_areas(
                destination_space_id,
                source_node_add,
                &mut window_list,
                window_manager,
                space_manager,
            );
        }
    }

    move_windows_to_their_target_frames_animating_if_enabled(&window_list, window_manager);
}

pub(crate) fn retile_window_dropped_on_no_target_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    window_id: WindowId,
    node_id: NodeId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if source_space_id == destination_space_id {
        move_windows_below_node_into_their_areas(
            source_space_id,
            node_id,
            window_manager,
            space_manager,
        );
    } else {
        untile_window_from_view_of_space(
            space_manager,
            source_space_id,
            window_id,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        forget_managed_window(window_manager, window_id);
        apply_shadow_removal_mode_to_window(window_manager, window_id);

        let view = tile_window_on_space(
            space_manager,
            window_id,
            destination_space_id,
            display_manager,
            window_manager,
        );
        record_managed_window_on_space_updating_its_shadow(
            window_manager,
            window_id,
            space_manager,
            view,
        );
    }
}

pub(crate) fn adjust_split_ratios_to_mouse_moved_window_or_restore_its_frame(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    info: &DraggedWindowFrameDelta,
    display_manager: &mut DisplayManager,
) {
    let mut success = true;

    'end: {
        let view_layout = space_manager.view.find(&space_id).map(|view| view.layout);
        if view_layout != Some(ViewLayout::BinarySpacePartitioning) {
            success = false;
            break 'end;
        }

        if info.changed_position {
            let mut direction: u8 = 0;
            if info.changed_x {
                direction |= ResizeHandle::LEFT.0;
            }
            if info.changed_y {
                direction |= ResizeHandle::TOP.0;
            }
            if resize_window_by_dragging_edges_or_to_absolute_size(
                window_manager,
                window_id,
                direction as i32,
                info.delta_x,
                info.delta_y,
                true,
                display_manager,
                space_manager,
            ) == WindowOperationOutcome::InvalidDestinationNode
            {
                success = false;
            }
        }

        if info.changed_size {
            let mut direction: u8 = 0;
            if info.changed_width && !info.changed_x {
                direction |= ResizeHandle::RIGHT.0;
            }
            if info.changed_height && !info.changed_y {
                direction |= ResizeHandle::BOTTOM.0;
            }
            if resize_window_by_dragging_edges_or_to_absolute_size(
                window_manager,
                window_id,
                direction as i32,
                info.delta_width,
                info.delta_height,
                true,
                display_manager,
                space_manager,
            ) == WindowOperationOutcome::InvalidDestinationNode
            {
                success = false;
            }
        }
    }

    if !success {
        let node = leaf_holding_window(space_manager, space_id, window_id);
        if let Some(node) = node {
            move_windows_below_node_into_their_areas(space_id, node, window_manager, space_manager);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MouseDropAction, drop_action_for_point_over_window_frame};
    use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
    use crate::mouse::tap::MouseMode;

    fn name_of_drop_action(action: MouseDropAction) -> &'static str {
        match action {
            MouseDropAction::None => "none",
            MouseDropAction::Stack => "stack",
            MouseDropAction::Swap => "swap",
            MouseDropAction::WarpTop => "warp top",
            MouseDropAction::WarpRight => "warp right",
            MouseDropAction::WarpBottom => "warp bottom",
            MouseDropAction::WarpLeft => "warp left",
        }
    }

    fn drop_action_over_an_800_by_600_window_at_100_200(
        x: f64,
        y: f64,
        source_node_window_count: Option<i32>,
        drop_action_setting: MouseMode,
    ) -> &'static str {
        let destination_window_frame = CGRect {
            origin: CGPoint { x: 100.0, y: 200.0 },
            size: CGSize {
                width: 800.0,
                height: 600.0,
            },
        };
        name_of_drop_action(drop_action_for_point_over_window_frame(
            destination_window_frame,
            CGPoint { x, y },
            source_node_window_count,
            drop_action_setting,
        ))
    }

    fn assert_each_point_yields(
        points_and_expected_actions: &[(f64, f64, &str)],
        source_node_window_count: Option<i32>,
        drop_action_setting: MouseMode,
    ) {
        for (x, y, expected_action) in points_and_expected_actions {
            assert_eq!(
                drop_action_over_an_800_by_600_window_at_100_200(
                    *x,
                    *y,
                    source_node_window_count,
                    drop_action_setting
                ),
                *expected_action,
                "point ({x}, {y}) with a dragged node of {source_node_window_count:?} windows"
            );
        }
    }

    #[test]
    fn a_point_in_each_triangle_outside_the_centre_warps_toward_that_edge() {
        let points_and_expected_actions = [
            (500.0, 250.0, "warp top"),
            (850.0, 500.0, "warp right"),
            (500.0, 750.0, "warp bottom"),
            (150.0, 500.0, "warp left"),
        ];

        for source_node_window_count in [Some(1), Some(2)] {
            for drop_action_setting in [MouseMode::Swap, MouseMode::Stack] {
                assert_each_point_yields(
                    &points_and_expected_actions,
                    source_node_window_count,
                    drop_action_setting,
                );
            }
        }
    }

    #[test]
    fn a_point_in_the_centre_swaps_a_single_window_node_when_the_drop_action_is_swap() {
        assert_each_point_yields(
            &[
                (500.0, 500.0, "swap"),
                (300.0, 350.0, "swap"),
                (699.5, 649.5, "swap"),
                (500.0, 400.0, "swap"),
            ],
            Some(1),
            MouseMode::Swap,
        );
    }

    #[test]
    fn a_point_in_the_centre_stacks_a_single_window_node_when_the_drop_action_is_stack() {
        assert_each_point_yields(
            &[
                (500.0, 500.0, "stack"),
                (300.0, 350.0, "stack"),
                (699.5, 649.5, "stack"),
            ],
            Some(1),
            MouseMode::Stack,
        );
    }

    #[test]
    fn a_point_in_the_centre_swaps_for_every_drop_action_setting_other_than_stack() {
        for drop_action_setting in [MouseMode::None, MouseMode::Move, MouseMode::Resize] {
            assert_each_point_yields(&[(500.0, 500.0, "swap")], Some(1), drop_action_setting);
        }
    }

    #[test]
    fn a_stacked_dragged_node_over_the_centre_falls_through_to_the_triangles() {
        assert_each_point_yields(&[(500.0, 400.0, "warp top")], Some(2), MouseMode::Swap);
        assert_each_point_yields(&[(500.0, 500.0, "none")], Some(2), MouseMode::Swap);
        assert_each_point_yields(&[(500.0, 500.0, "none")], Some(3), MouseMode::Stack);
        assert_each_point_yields(&[(500.0, 500.0, "none")], Some(0), MouseMode::Swap);
    }

    #[test]
    fn a_dragged_node_that_cannot_be_found_falls_through_to_the_triangles() {
        assert_each_point_yields(
            &[(500.0, 400.0, "warp top"), (500.0, 500.0, "none")],
            None,
            MouseMode::Stack,
        );
    }

    #[test]
    fn the_centre_rectangle_includes_its_minimum_edges_and_excludes_its_maximum_edges() {
        assert_each_point_yields(
            &[
                (300.0, 350.0, "swap"),
                (700.0, 500.0, "warp right"),
                (500.0, 650.0, "warp bottom"),
            ],
            Some(1),
            MouseMode::Swap,
        );
    }

    #[test]
    fn points_on_a_diagonal_between_two_triangles_yield_no_action() {
        assert_each_point_yields(
            &[
                (200.0, 275.0, "none"),
                (800.0, 275.0, "none"),
                (200.0, 725.0, "none"),
                (800.0, 725.0, "none"),
            ],
            Some(1),
            MouseMode::Swap,
        );
    }

    #[test]
    fn points_on_the_window_edges_or_outside_the_window_yield_no_action() {
        assert_each_point_yields(
            &[
                (100.0, 500.0, "none"),
                (900.0, 500.0, "none"),
                (500.0, 200.0, "none"),
                (500.0, 800.0, "none"),
                (100.0, 200.0, "none"),
                (900.0, 800.0, "none"),
                (99.0, 500.0, "none"),
                (500.0, 801.0, "none"),
            ],
            Some(1),
            MouseMode::Swap,
        );
    }
}
