use std::sync::atomic::Ordering;

use crate::display::manager::DisplayManager;
use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
use crate::ffi::core_graphics::CGRectContainsPoint;
use crate::layout::settings::ViewType;
use crate::layout::tree::{
    NODE_MAX_WINDOW_COUNT, WindowNodeChild, WindowNodeSplit,
    view_add_window_node_with_insertion_point, view_find_window_node, view_remove_window_node,
    view_stack_window_node, window_node_capture_windows, window_node_contains_window,
    window_node_flush, window_node_swap_window_list,
};
use crate::mouse::drag::{MouseDragState, MouseWindowInfo};
use crate::mouse::tap::{MOUSE_TAP_STATE, MouseMode};
use crate::scripting_addition::client::scripting_addition_order_window;
use crate::space::manager::SpaceManager;
use crate::space::tiling::{space_manager_tile_window_on_space, space_manager_untile_window};
use crate::support::geometry::triangle_contains_point;
use crate::support::handles::{NodeId, SpaceId, WindowId};
use crate::support::layer::LAYER_BELOW;
use crate::support::resize_handle::ResizeHandle;
use crate::window::animation::{
    WindowCapture, window_manager_animate_window, window_manager_animate_window_list,
};
use crate::window::frame::window_manager_resize_window_relative;
use crate::window::layer::window_manager_adjust_layer;
use crate::window::manager::{
    WindowManager, WindowOpError, window_manager_add_managed_window, window_manager_find_window,
    window_manager_remove_managed_window,
};
use crate::window::shadow::window_manager_purify_window;

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

pub(crate) fn mouse_determine_drop_action(
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

    let source_node_window_count = space_manager
        .view
        .find(&source_space_id)
        .and_then(|view| view.find_node(source_node_id))
        .map(|node| node.window_count);

    if (CGRectContainsPoint(center_rect, point_relative_to_frame_origin))
        && (source_node_window_count == Some(1))
    {
        return if MOUSE_TAP_STATE.drop_action.load(Ordering::Relaxed) == MouseMode::Stack as u8 {
            MouseDropAction::Stack
        } else {
            MouseDropAction::Swap
        };
    } else if triangle_contains_point(&top_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpTop;
    } else if triangle_contains_point(&right_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpRight;
    } else if triangle_contains_point(&bottom_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpBottom;
    } else if triangle_contains_point(&left_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpLeft;
    }

    MouseDropAction::None
}

pub(crate) fn mouse_drop_action_stack(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_window_id: WindowId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    space_manager_untile_window(
        space_manager,
        source_space_id,
        source_window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
    window_manager_remove_managed_window(window_manager, source_window_id);

    let destination_node =
        view_find_window_node(space_manager, destination_space_id, destination_window_id);
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
    if destination_node_window_count + 1 < NODE_MAX_WINDOW_COUNT as i32 {
        view_stack_window_node(
            destination_space_id,
            destination_node,
            source_window_id,
            space_manager,
        );
        window_manager_add_managed_window(
            window_manager,
            source_window_id,
            space_manager,
            destination_space_id,
        );
        window_manager_adjust_layer(source_window_id, LAYER_BELOW, window_manager);

        let Some(view) = space_manager.view.find(&destination_space_id) else {
            return;
        };
        let Some(node) = view.find_node(destination_node) else {
            return;
        };
        scripting_addition_order_window(source_window_id, 1, node.window_order[1]);

        let area = match node.zoom.and_then(|zoom| view.find_node(zoom)) {
            Some(zoom) => zoom.area,
            None => node.area,
        };
        window_manager_animate_window(
            WindowCapture {
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

pub(crate) fn mouse_drop_action_swap(
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
    if window_node_contains_window(
        source_space_id,
        source_node_id,
        source_view_insertion_point,
        space_manager,
    ) {
        if let Some(source_view) = space_manager.view.find_mut(&source_space_id) {
            source_view.insertion_point = destination_window_id;
        }
    } else if window_node_contains_window(
        destination_space_id,
        destination_node_id,
        destination_view_insertion_point,
        space_manager,
    ) {
        if let Some(destination_view) = space_manager.view.find_mut(&destination_space_id) {
            destination_view.insertion_point = source_window_id;
        }
    }

    window_node_swap_window_list(
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
            window_manager_remove_managed_window(window_manager, source_node_window_list[index]);
            if let Some(window) =
                window_manager_find_window(window_manager, source_node_window_list[index])
            {
                window_manager_add_managed_window(
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
            window_manager_remove_managed_window(
                window_manager,
                destination_node_window_list[index],
            );
            if let Some(window) =
                window_manager_find_window(window_manager, destination_node_window_list[index])
            {
                window_manager_add_managed_window(
                    window_manager,
                    window,
                    space_manager,
                    destination_space_id,
                );
            }
        }
    }

    let mut window_list: Vec<WindowCapture> = Vec::new();
    window_node_capture_windows(
        source_space_id,
        source_node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    window_node_capture_windows(
        destination_space_id,
        destination_node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    window_manager_animate_window_list(&window_list, window_manager);
}

pub(crate) fn mouse_drop_action_warp(
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
            mouse_drop_action_swap(
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

    let source_node_remove = view_remove_window_node(
        space_manager,
        source_space_id,
        source_window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
    window_manager_remove_managed_window(window_manager, source_window_id);
    window_manager_purify_window(window_manager, source_window_id);

    let source_node_add = view_add_window_node_with_insertion_point(
        space_manager,
        destination_space_id,
        source_window_id,
        destination_window_id,
        display_manager,
        window_manager,
    );
    window_manager_add_managed_window(
        window_manager,
        source_window_id,
        space_manager,
        destination_space_id,
    );

    let mut window_list: Vec<WindowCapture> = Vec::new();

    if let Some(source_node_remove) = source_node_remove {
        window_node_capture_windows(
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
            window_node_capture_windows(
                destination_space_id,
                source_node_add,
                &mut window_list,
                window_manager,
                space_manager,
            );
        }
    }

    window_manager_animate_window_list(&window_list, window_manager);
}

pub(crate) fn mouse_drop_no_target(
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
        window_node_flush(source_space_id, node_id, window_manager, space_manager);
    } else {
        space_manager_untile_window(
            space_manager,
            source_space_id,
            window_id,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, window_id);
        window_manager_purify_window(window_manager, window_id);

        let view = space_manager_tile_window_on_space(
            space_manager,
            window_id,
            destination_space_id,
            display_manager,
            window_manager,
        );
        window_manager_add_managed_window(window_manager, window_id, space_manager, view);
    }
}

pub(crate) fn mouse_drop_try_adjust_bsp_grid(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    info: &MouseWindowInfo,
    display_manager: &mut DisplayManager,
) {
    let mut success = true;

    'end: {
        let view_layout = space_manager.view.find(&space_id).map(|view| view.layout);
        if view_layout != Some(ViewType::Bsp) {
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
            if window_manager_resize_window_relative(
                window_manager,
                window_id,
                direction as i32,
                info.delta_x,
                info.delta_y,
                true,
                display_manager,
                space_manager,
            ) == WindowOpError::InvalidDstNode
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
            if window_manager_resize_window_relative(
                window_manager,
                window_id,
                direction as i32,
                info.delta_width,
                info.delta_height,
                true,
                display_manager,
                space_manager,
            ) == WindowOpError::InvalidDstNode
            {
                success = false;
            }
        }
    }

    if !success {
        let node = view_find_window_node(space_manager, space_id, window_id);
        if let Some(node) = node {
            window_node_flush(space_id, node, window_manager, space_manager);
        }
    }
}
