use crate::display::manager::DisplayManager;
use crate::ffi::core_foundation::CGPoint;
use crate::ffi::skylight::_SLPSSetFrontProcessWithOptions;
use crate::layout::area::area_make_pair;
use crate::layout::insertion::{insert_feedback_destroy, insert_feedback_show};
use crate::layout::settings::{
    ViewFlag, ViewType, window_node_get_gap, window_node_get_ratio, window_node_get_split,
};
use crate::layout::tree::{
    NODE_MAX_WINDOW_COUNT, WindowNodeChild, WindowNodeSplit,
    view_add_window_node_with_insertion_point, view_find_window_node, view_remove_window_node,
    view_stack_window_node, window_node_capture_windows, window_node_contains_window,
    window_node_is_left_child, window_node_swap_window_list,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::scripting_addition::client::scripting_addition_order_window;
use crate::space::managed_space::space_is_visible;
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::space::moving_windows::space_manager_move_window_to_space;
use crate::space::tiling::{
    space_manager_tile_window_on_space_with_insertion_point, space_manager_untile_window,
};
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST};
use crate::support::handles::WindowId;
use crate::support::layer::LAYER_BELOW;
use crate::window::animation::{
    WindowCapture, window_manager_animate_window, window_manager_animate_window_list,
};
use crate::window::floating_and_sticky::window_manager_make_window_sticky;
use crate::window::focus::{
    kCPSNoWindows, window_manager_focus_window_with_raise_resolving_its_application,
};
use crate::window::layer::window_manager_adjust_layer;
use crate::window::manager::{
    WindowManager, WindowOpError, window_manager_add_managed_window,
    window_manager_find_managed_window, window_manager_find_window,
    window_manager_is_window_eligible, window_manager_remove_managed_window,
};
use crate::window::model::{WindowFlag, window_check_flag, window_clear_flag, window_space};
use crate::window::screen_lookup::window_manager_find_window_on_space_by_rank_filtering_window;
use crate::window::shadow::window_manager_purify_window;

pub(crate) fn window_manager_set_window_insertion(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    direction: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> WindowOpError {
    let space_id = window_space(window_id);
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return WindowOpError::InvalidSrcView;
    };
    if view.layout != ViewType::Bsp {
        return WindowOpError::InvalidSrcView;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return WindowOpError::InvalidSrcNode;
    };

    let insertion_point = space_manager
        .view
        .find(&space_id)
        .map_or(WindowId(0), |view| view.insertion_point);
    if insertion_point.0 != 0 && insertion_point != window_id {
        let insert_node = view_find_window_node(space_manager, space_id, insertion_point);
        if let Some(insert_node_id) = insert_node {
            insert_feedback_destroy(space_id, insert_node_id, window_manager, space_manager);
            if let Some(view) = space_manager.view.find_mut(&space_id)
                && let Some(insert_node) = view.find_node_mut(insert_node_id)
            {
                insert_node.split = WindowNodeSplit::None;
                insert_node.child = WindowNodeChild::None;
                insert_node.insert_direction = 0;
            }
        }
    }

    let Some(node_insert_direction) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
        .map(|node| node.insert_direction)
    else {
        return WindowOpError::InvalidSrcNode;
    };
    if direction == node_insert_direction {
        insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&space_id) {
            if let Some(node) = view.find_node_mut(node_id) {
                node.split = WindowNodeSplit::None;
                node.child = WindowNodeChild::None;
                node.insert_direction = 0;
            }
            view.insertion_point = WindowId(0);
        }
        return WindowOpError::Success;
    }

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return WindowOpError::InvalidSrcView;
    };
    let Some(node) = view.find_node_mut(node_id) else {
        return WindowOpError::InvalidSrcNode;
    };

    if direction == DIR_NORTH {
        node.split = WindowNodeSplit::X;
        node.child = WindowNodeChild::First;
    } else if direction == DIR_EAST {
        node.split = WindowNodeSplit::Y;
        node.child = WindowNodeChild::Second;
    } else if direction == DIR_SOUTH {
        node.split = WindowNodeSplit::X;
        node.child = WindowNodeChild::Second;
    } else if direction == DIR_WEST {
        node.split = WindowNodeSplit::Y;
        node.child = WindowNodeChild::First;
    }

    node.insert_direction = direction;
    let node_first_window_id = node.window_order[0];
    view.insertion_point = node_first_window_id;
    insert_feedback_show(space_id, node_id, window_manager, space_manager);

    WindowOpError::Success
}

pub(crate) fn window_manager_stack_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    a_window: WindowId,
    b_window: WindowId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) -> WindowOpError {
    if a_window == b_window {
        return WindowOpError::SameWindow;
    }

    let Some(a_view) = window_manager_find_managed_window(window_manager, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };

    let b_view = window_manager_find_managed_window(window_manager, b_window);
    if let Some(b_view) = b_view {
        space_manager_untile_window(
            space_manager,
            b_view,
            b_window,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, b_window);
        window_manager_purify_window(window_manager, b_window);
    } else if window_manager
        .window
        .find(&b_window)
        .is_some_and(|window| window_check_flag(window, WindowFlag::FLOAT))
    {
        if !window_manager_is_window_eligible(b_window, window_manager) {
            return WindowOpError::InvalidSrcNode;
        }
        let Some(window) = window_manager.window.find_mut(&b_window) else {
            return WindowOpError::InvalidSrcNode;
        };
        window_clear_flag(window, WindowFlag::FLOAT);
        if window_check_flag(window, WindowFlag::STICKY) {
            window_manager_make_window_sticky(
                space_manager,
                window_manager,
                b_window,
                false,
                display_manager,
                mouse_drag_state,
            );
        }
    }

    let Some(a_node) = view_find_window_node(space_manager, a_view, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };
    let Some(a_node_window_count) = space_manager
        .view
        .find(&a_view)
        .and_then(|view| view.find_node(a_node))
        .map(|node| node.window_count)
    else {
        return WindowOpError::InvalidSrcNode;
    };
    if a_node_window_count + 1 >= NODE_MAX_WINDOW_COUNT as i32 {
        return WindowOpError::MaxStack;
    }

    view_stack_window_node(a_view, a_node, b_window, space_manager);
    window_manager_add_managed_window(window_manager, b_window, space_manager, a_view);
    window_manager_adjust_layer(b_window, LAYER_BELOW, window_manager);
    let Some(a_node_second_window_in_order) = space_manager
        .view
        .find(&a_view)
        .and_then(|view| view.find_node(a_node))
        .map(|node| node.window_order[1])
    else {
        return WindowOpError::InvalidSrcNode;
    };
    scripting_addition_order_window(b_window, 1, a_node_second_window_in_order);

    let Some(area) = space_manager.view.find(&a_view).and_then(|view| {
        let node = view.find_node(a_node)?;
        match node.zoom {
            Some(zoom) => view.find_node(zoom).map(|zoom_node| zoom_node.area),
            None => Some(node.area),
        }
    }) else {
        return WindowOpError::InvalidSrcNode;
    };
    window_manager_animate_window(
        WindowCapture {
            window_id: b_window,
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        },
        window_manager,
    );
    WindowOpError::Success
}

pub(crate) fn window_manager_warp_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    a_window: WindowId,
    b_window: WindowId,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) -> WindowOpError {
    if a_window == b_window {
        return WindowOpError::SameWindow;
    }

    let a_space_id = window_space(a_window);
    let a_view =
        space_manager_find_view(space_manager, a_space_id, display_manager, window_manager);
    let Some(a_view_layout) = space_manager.view.find(&a_view).map(|view| view.layout) else {
        return WindowOpError::InvalidSrcView;
    };
    if a_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidSrcView;
    }

    let b_space_id = window_space(b_window);
    let b_view =
        space_manager_find_view(space_manager, b_space_id, display_manager, window_manager);
    let Some(b_view_layout) = space_manager.view.find(&b_view).map(|view| view.layout) else {
        return WindowOpError::InvalidDstView;
    };
    if b_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidDstView;
    }

    let Some(a_node) = view_find_window_node(space_manager, a_view, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };

    let Some(b_node) = view_find_window_node(space_manager, b_view, b_window) else {
        return WindowOpError::InvalidDstNode;
    };

    if (a_view, a_node) == (b_view, b_node) {
        return WindowOpError::SameStack;
    }

    let Some((a_node_parent, a_node_window_count)) = space_manager
        .view
        .find(&a_view)
        .and_then(|view| view.find_node(a_node))
        .map(|node| (node.parent, node.window_count))
    else {
        return WindowOpError::InvalidSrcNode;
    };
    let Some(b_node_parent) = space_manager
        .view
        .find(&b_view)
        .and_then(|view| view.find_node(b_node))
        .map(|node| node.parent)
    else {
        return WindowOpError::InvalidDstNode;
    };

    if a_node_parent.is_some()
        && b_node_parent.is_some()
        && (a_view, a_node_parent) == (b_view, b_node_parent)
        && a_node_window_count == 1
    {
        let b_view_insertion_point = space_manager
            .view
            .find(&b_view)
            .map_or(WindowId(0), |view| view.insertion_point);
        if window_node_contains_window(b_view, b_node, b_view_insertion_point, space_manager) {
            if let Some(b_node_parent) = b_node_parent
                && let Some(view) = space_manager.view.find_mut(&b_view)
                && let Some((b_node_split, b_node_child)) =
                    view.find_node(b_node).map(|node| (node.split, node.child))
                && let Some(parent) = view.find_node_mut(b_node_parent)
            {
                parent.split = b_node_split;
                parent.child = b_node_child;
            }

            view_remove_window_node(
                space_manager,
                a_view,
                a_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, a_window);
            window_manager_add_managed_window(window_manager, a_window, space_manager, b_view);
            let a_node_add = view_add_window_node_with_insertion_point(
                space_manager,
                b_view,
                a_window,
                b_window,
                display_manager,
                window_manager,
            );

            let mut window_list: Vec<WindowCapture> = Vec::new();
            if let Some(a_node_add) = a_node_add {
                window_node_capture_windows(
                    b_view,
                    a_node_add,
                    &mut window_list,
                    window_manager,
                    space_manager,
                );
            }
            window_manager_animate_window_list(&window_list, window_manager);
        } else {
            let a_view_insertion_point = space_manager
                .view
                .find(&a_view)
                .map_or(WindowId(0), |view| view.insertion_point);
            if window_node_contains_window(a_view, a_node, a_view_insertion_point, space_manager)
                && let Some(view) = space_manager.view.find_mut(&a_view)
            {
                view.insertion_point = b_window;
            }

            window_node_swap_window_list(a_view, a_node, b_view, b_node, space_manager);

            let mut window_list: Vec<WindowCapture> = Vec::new();
            window_node_capture_windows(
                a_view,
                a_node,
                &mut window_list,
                window_manager,
                space_manager,
            );
            window_node_capture_windows(
                b_view,
                b_node,
                &mut window_list,
                window_manager,
                space_manager,
            );
            window_manager_animate_window_list(&window_list, window_manager);
        }
    } else {
        if a_view == b_view {
            //
            // :NaturalWarp
            //
            // NOTE(asmvik): Precalculate both target areas and select the one that has the closest distance to the source area.
            // This allows the warp to feel more natural in terms of where the window is placed on screen, however, this is only utilized
            // for warp operations where both operands belong to the same space. There may be a better system to handle this if/when multiple
            // monitors should be supported.
            //

            let Some(b_node_area) = space_manager
                .view
                .find(&b_view)
                .and_then(|view| view.find_node(b_node))
                .map(|node| node.area)
            else {
                return WindowOpError::InvalidDstNode;
            };
            let (candidate_first_child_area, candidate_second_child_area) = area_make_pair(
                window_node_get_split(space_manager, b_view, b_node),
                window_node_get_gap(space_manager, b_view),
                window_node_get_ratio(b_view, b_node, space_manager),
                b_node_area,
            );

            let Some(a_node_area) = space_manager
                .view
                .find(&a_view)
                .and_then(|view| view.find_node(a_node))
                .map(|node| node.area)
            else {
                return WindowOpError::InvalidSrcNode;
            };
            let source_node_center_point = CGPoint::new(
                (0.5f32 + a_node_area.x + a_node_area.width / 2.0f32) as i32 as f64,
                (0.5f32 + a_node_area.y + a_node_area.height / 2.0f32) as i32 as f64,
            );
            let distance_to_candidate_first_child = ((source_node_center_point.x
                - ((0.5f32
                    + candidate_first_child_area.x
                    + candidate_first_child_area.width / 2.0f32) as i32) as f64)
                as f32)
                .powf(2.0f32)
                + ((source_node_center_point.y
                    - ((0.5f32
                        + candidate_first_child_area.y
                        + candidate_first_child_area.height / 2.0f32) as i32)
                        as f64) as f32)
                    .powf(2.0f32);
            let distance_to_candidate_second_child = ((source_node_center_point.x
                - ((0.5f32
                    + candidate_second_child_area.x
                    + candidate_second_child_area.width / 2.0f32) as i32) as f64)
                as f32)
                .powf(2.0f32)
                + ((source_node_center_point.y
                    - ((0.5f32
                        + candidate_second_child_area.y
                        + candidate_second_child_area.height / 2.0f32)
                        as i32) as f64) as f32)
                    .powf(2.0f32);

            let b_node_child =
                if distance_to_candidate_first_child < distance_to_candidate_second_child {
                    WindowNodeChild::First
                } else if distance_to_candidate_first_child > distance_to_candidate_second_child {
                    WindowNodeChild::Second
                } else if window_node_is_left_child(a_view, a_node, space_manager) {
                    WindowNodeChild::First
                } else {
                    WindowNodeChild::Second
                };
            if let Some(view) = space_manager.view.find_mut(&b_view)
                && let Some(node) = view.find_node_mut(b_node)
            {
                node.child = b_node_child;
            }

            let a_node_remove = view_remove_window_node(
                space_manager,
                a_view,
                a_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            let a_node_add = view_add_window_node_with_insertion_point(
                space_manager,
                b_view,
                a_window,
                b_window,
                display_manager,
                window_manager,
            );

            let mut window_list: Vec<WindowCapture> = Vec::new();
            if let Some(a_node_remove) = a_node_remove {
                window_node_capture_windows(
                    a_view,
                    a_node_remove,
                    &mut window_list,
                    window_manager,
                    space_manager,
                );
            }

            if let Some(a_node_add) = a_node_add {
                let a_node_add_parent = space_manager
                    .view
                    .find(&b_view)
                    .and_then(|view| view.find_node(a_node_add))
                    .and_then(|node| node.parent);
                if a_node_remove != Some(a_node_add) && a_node_remove != a_node_add_parent {
                    window_node_capture_windows(
                        b_view,
                        a_node_add,
                        &mut window_list,
                        window_manager,
                        space_manager,
                    );
                }
            }

            window_manager_animate_window_list(&window_list, window_manager);
        } else {
            if window_manager.focused_window_id == a_window {
                let next = window_manager_find_window_on_space_by_rank_filtering_window(
                    window_manager,
                    a_view,
                    1,
                    a_window,
                );
                if let Some(next) = next {
                    window_manager_focus_window_with_raise_resolving_its_application(
                        window_manager,
                        next,
                    );
                } else {
                    unsafe {
                        _SLPSSetFrontProcessWithOptions(
                            &mut process_manager.finder_process_serial_number,
                            0,
                            kCPSNoWindows,
                        )
                    };
                }
            }

            //
            // :NaturalWarp
            //
            // TODO(asmvik): Warp operations with operands that belong to different monitors does not yet implement a heuristic to select
            // the target area that feels the most natural in terms of where the window is placed on screen. Is it possible to do better when
            // warping between spaces that belong to the same monitor as well??
            //

            space_manager_untile_window(
                space_manager,
                a_view,
                a_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, a_window);
            window_manager_add_managed_window(window_manager, a_window, space_manager, b_view);
            space_manager_move_window_to_space(b_view, a_window);
            space_manager_tile_window_on_space_with_insertion_point(
                space_manager,
                a_window,
                b_view,
                b_window,
                display_manager,
                window_manager,
            );
        }
    }

    WindowOpError::Success
}

pub(crate) fn window_manager_swap_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    a_window: WindowId,
    b_window: WindowId,
    display_manager: &mut DisplayManager,
) -> WindowOpError {
    if a_window == b_window {
        return WindowOpError::SameWindow;
    }

    let a_space_id = window_space(a_window);
    let a_view =
        space_manager_find_view(space_manager, a_space_id, display_manager, window_manager);

    let b_space_id = window_space(b_window);
    let b_view =
        space_manager_find_view(space_manager, b_space_id, display_manager, window_manager);

    let Some(a_node) = view_find_window_node(space_manager, a_view, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };

    let Some(b_node) = view_find_window_node(space_manager, b_view, b_window) else {
        return WindowOpError::InvalidDstNode;
    };

    if (a_view, a_node) == (b_view, b_node) {
        let mut a_list_index = 0;
        let mut a_order_index = 0;

        let mut b_list_index = 0;
        let mut b_order_index = 0;

        let Some(view) = space_manager.view.find_mut(&a_view) else {
            return WindowOpError::InvalidSrcNode;
        };
        let Some(node) = view.find_node_mut(a_node) else {
            return WindowOpError::InvalidSrcNode;
        };

        for index in 0..node.window_count as usize {
            if node.window_list[index] == a_window {
                a_list_index = index;
            } else if node.window_list[index] == b_window {
                b_list_index = index;
            }

            if node.window_order[index] == a_window {
                a_order_index = index;
            } else if node.window_order[index] == b_window {
                b_order_index = index;
            }
        }

        node.window_list[a_list_index] = b_window;
        node.window_order[a_order_index] = b_window;

        node.window_list[b_list_index] = a_window;
        node.window_order[b_order_index] = a_window;

        if a_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                b_window,
            );
        } else if b_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                a_window,
            );
        }

        return WindowOpError::Success;
    }

    let Some(a_view_layout) = space_manager.view.find(&a_view).map(|view| view.layout) else {
        return WindowOpError::InvalidSrcView;
    };
    if a_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidSrcView;
    }
    let Some(b_view_layout) = space_manager.view.find(&b_view).map(|view| view.layout) else {
        return WindowOpError::InvalidDstView;
    };
    if b_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidDstView;
    }

    let a_view_insertion_point = space_manager
        .view
        .find(&a_view)
        .map_or(WindowId(0), |view| view.insertion_point);
    let b_view_insertion_point = space_manager
        .view
        .find(&b_view)
        .map_or(WindowId(0), |view| view.insertion_point);
    if window_node_contains_window(a_view, a_node, a_view_insertion_point, space_manager) {
        if let Some(view) = space_manager.view.find_mut(&a_view) {
            view.insertion_point = b_window;
        }
    } else if window_node_contains_window(b_view, b_node, b_view_insertion_point, space_manager)
        && let Some(view) = space_manager.view.find_mut(&b_view)
    {
        view.insertion_point = a_window;
    }

    let a_visible = space_is_visible(a_view);
    let b_visible = space_is_visible(b_view);

    if a_view != b_view {
        let Some((a_node_window_list, a_node_window_count)) = space_manager
            .view
            .find(&a_view)
            .and_then(|view| view.find_node(a_node))
            .map(|node| (node.window_list, node.window_count))
        else {
            return WindowOpError::InvalidSrcNode;
        };
        for index in 0..a_node_window_count as usize {
            let window = window_manager_find_window(window_manager, a_node_window_list[index]);
            window_manager_remove_managed_window(window_manager, a_node_window_list[index]);
            if let Some(window) = window {
                space_manager_move_window_to_space(b_view, window);
                window_manager_add_managed_window(window_manager, window, space_manager, b_view);
            }
        }

        let Some((b_node_window_list, b_node_window_count)) = space_manager
            .view
            .find(&b_view)
            .and_then(|view| view.find_node(b_node))
            .map(|node| (node.window_list, node.window_count))
        else {
            return WindowOpError::InvalidDstNode;
        };
        for index in 0..b_node_window_count as usize {
            let window = window_manager_find_window(window_manager, b_node_window_list[index]);
            window_manager_remove_managed_window(window_manager, b_node_window_list[index]);
            if let Some(window) = window {
                space_manager_move_window_to_space(a_view, window);
                window_manager_add_managed_window(window_manager, window, space_manager, a_view);
            }
        }

        if a_visible && !b_visible && a_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                b_window,
            );
        } else if b_visible && !a_visible && b_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                a_window,
            );
        }
    }

    window_node_swap_window_list(a_view, a_node, b_view, b_node, space_manager);
    let mut window_list: Vec<WindowCapture> = Vec::new();

    if a_visible {
        window_node_capture_windows(
            a_view,
            a_node,
            &mut window_list,
            window_manager,
            space_manager,
        );
    } else if let Some(view) = space_manager.view.find_mut(&a_view) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    if b_visible {
        window_node_capture_windows(
            b_view,
            b_node,
            &mut window_list,
            window_manager,
            space_manager,
        );
    } else if let Some(view) = space_manager.view.find_mut(&b_view) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    window_manager_animate_window_list(&window_list, window_manager);
    WindowOpError::Success
}
