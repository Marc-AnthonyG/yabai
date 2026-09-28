use crate::display::manager::DisplayManager;
use crate::ffi::core_foundation::CGPoint;
use crate::ffi::skylight::_SLPSSetFrontProcessWithOptions;
use crate::layout::area::divide_area_into_two_by_split_ratio_and_gap;
use crate::layout::insertion::{destroy_insert_feedback_of_node, show_insert_feedback_of_node};
use crate::layout::settings::{
    ViewFlag, ViewLayout, effective_ratio_of_node, effective_split_of_node,
    effective_window_gap_of_view,
};
use crate::layout::tree::{
    MOST_WINDOWS_A_NODE_CAN_HOLD, WindowNodeChild, WindowNodeSplit,
    add_window_to_view_tree_preferring_insertion_point,
    collect_windows_below_node_with_their_target_areas, is_node_the_left_child_of_its_parent,
    is_window_in_node, leaf_holding_window, remove_window_from_view_tree,
    stack_window_in_node_as_its_front_window, swap_windows_between_nodes_clearing_their_zoom,
    window_node_split_and_child_placing_a_window_inserted_in_direction,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::scripting_addition::client::order_window_relative_to_other_window_through_scripting_addition;
use crate::space::managed_space::is_space_visible_on_its_display;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::space::moving_windows::move_window_to_space_by_whichever_mechanism_this_macos_supports;
use crate::space::tiling::{
    tile_window_on_space_preferring_insertion_point, untile_window_from_view_of_space,
};
use crate::support::handles::WindowId;
use crate::support::layer::LAYER_BELOW;
use crate::window::animation::{
    WindowWithTargetFrame, move_window_to_its_target_frame_animating_if_enabled,
    move_windows_to_their_target_frames_animating_if_enabled,
};
use crate::window::floating_and_sticky::set_whether_window_is_sticky;
use crate::window::focus::{focus_and_raise_tracked_window, kCPSNoWindows};
use crate::window::layer::set_window_layer_unless_explicitly_set;
use crate::window::manager::{
    WindowManager, WindowOperationOutcome, forget_managed_window,
    is_window_eligible_for_management, record_managed_window_on_space_updating_its_shadow,
    space_managing_window, tracked_window_with_id,
};
use crate::window::model::{
    WindowFlag, clear_window_flag, is_window_flag_set, query_space_holding_window,
};
use crate::window::screen_lookup::query_tracked_window_at_rank_on_space_skipping_window;
use crate::window::shadow::apply_shadow_removal_mode_to_window;

pub(crate) fn toggle_insertion_point_at_window_in_direction(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    direction: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> WindowOperationOutcome {
    let space_id = query_space_holding_window(window_id);
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return WindowOperationOutcome::InvalidSourceView;
    };
    if view.layout != ViewLayout::BinarySpacePartitioning {
        return WindowOperationOutcome::InvalidSourceView;
    }

    let Some(node_id) = leaf_holding_window(space_manager, space_id, window_id) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };

    let insertion_point = space_manager
        .view
        .find(&space_id)
        .map_or(WindowId(0), |view| view.insertion_point);
    if insertion_point.0 != 0 && insertion_point != window_id {
        let insert_node = leaf_holding_window(space_manager, space_id, insertion_point);
        if let Some(insert_node_id) = insert_node {
            destroy_insert_feedback_of_node(
                space_id,
                insert_node_id,
                window_manager,
                space_manager,
            );
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
        return WindowOperationOutcome::InvalidSourceNode;
    };
    if direction == node_insert_direction {
        destroy_insert_feedback_of_node(space_id, node_id, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&space_id) {
            if let Some(node) = view.find_node_mut(node_id) {
                node.split = WindowNodeSplit::None;
                node.child = WindowNodeChild::None;
                node.insert_direction = 0;
            }
            view.insertion_point = WindowId(0);
        }
        return WindowOperationOutcome::Success;
    }

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return WindowOperationOutcome::InvalidSourceView;
    };
    let Some(node) = view.find_node_mut(node_id) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };

    if let Some((split, child)) =
        window_node_split_and_child_placing_a_window_inserted_in_direction(direction)
    {
        node.split = split;
        node.child = child;
    }

    node.insert_direction = direction;
    let node_first_window_id = node.window_order[0];
    view.insertion_point = node_first_window_id;
    show_insert_feedback_of_node(space_id, node_id, window_manager, space_manager);

    WindowOperationOutcome::Success
}

pub(crate) fn stack_second_window_onto_the_node_of_first_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    receiving_window: WindowId,
    stacked_window: WindowId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) -> WindowOperationOutcome {
    if receiving_window == stacked_window {
        return WindowOperationOutcome::SameWindow;
    }

    let Some(receiving_view) = space_managing_window(window_manager, receiving_window) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };

    let stacked_view = space_managing_window(window_manager, stacked_window);
    if let Some(stacked_view) = stacked_view {
        untile_window_from_view_of_space(
            space_manager,
            stacked_view,
            stacked_window,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        forget_managed_window(window_manager, stacked_window);
        apply_shadow_removal_mode_to_window(window_manager, stacked_window);
    } else if window_manager
        .window
        .find(&stacked_window)
        .is_some_and(|window| is_window_flag_set(window, WindowFlag::FLOATING))
    {
        if !is_window_eligible_for_management(stacked_window, window_manager) {
            return WindowOperationOutcome::InvalidSourceNode;
        }
        let Some(window) = window_manager.window.find_mut(&stacked_window) else {
            return WindowOperationOutcome::InvalidSourceNode;
        };
        clear_window_flag(window, WindowFlag::FLOATING);
        if is_window_flag_set(window, WindowFlag::STICKY) {
            set_whether_window_is_sticky(
                space_manager,
                window_manager,
                stacked_window,
                false,
                display_manager,
                mouse_drag_state,
            );
        }
    }

    let Some(receiving_node) = leaf_holding_window(space_manager, receiving_view, receiving_window)
    else {
        return WindowOperationOutcome::InvalidSourceNode;
    };
    let Some(receiving_node_window_count) = space_manager
        .view
        .find(&receiving_view)
        .and_then(|view| view.find_node(receiving_node))
        .map(|node| node.window_count)
    else {
        return WindowOperationOutcome::InvalidSourceNode;
    };
    if receiving_node_window_count + 1 >= MOST_WINDOWS_A_NODE_CAN_HOLD as i32 {
        return WindowOperationOutcome::StackIsFull;
    }

    stack_window_in_node_as_its_front_window(
        receiving_view,
        receiving_node,
        stacked_window,
        space_manager,
    );
    record_managed_window_on_space_updating_its_shadow(
        window_manager,
        stacked_window,
        space_manager,
        receiving_view,
    );
    set_window_layer_unless_explicitly_set(stacked_window, LAYER_BELOW, window_manager);
    let Some(receiving_node_second_window_in_order) = space_manager
        .view
        .find(&receiving_view)
        .and_then(|view| view.find_node(receiving_node))
        .map(|node| node.window_order[1])
    else {
        return WindowOperationOutcome::InvalidSourceNode;
    };
    order_window_relative_to_other_window_through_scripting_addition(
        stacked_window,
        1,
        receiving_node_second_window_in_order,
    );

    let Some(area) = space_manager.view.find(&receiving_view).and_then(|view| {
        let node = view.find_node(receiving_node)?;
        match node.zoom {
            Some(zoom) => view.find_node(zoom).map(|zoom_node| zoom_node.area),
            None => Some(node.area),
        }
    }) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };
    move_window_to_its_target_frame_animating_if_enabled(
        WindowWithTargetFrame {
            window_id: stacked_window,
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        },
        window_manager,
    );
    WindowOperationOutcome::Success
}

pub(crate) fn warp_first_window_into_the_node_of_second_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    warped_window: WindowId,
    target_window: WindowId,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) -> WindowOperationOutcome {
    if warped_window == target_window {
        return WindowOperationOutcome::SameWindow;
    }

    let warped_space_id = query_space_holding_window(warped_window);
    let warped_view = find_or_create_view_for_space(
        space_manager,
        warped_space_id,
        display_manager,
        window_manager,
    );
    let Some(warped_view_layout) = space_manager
        .view
        .find(&warped_view)
        .map(|view| view.layout)
    else {
        return WindowOperationOutcome::InvalidSourceView;
    };
    if warped_view_layout != ViewLayout::BinarySpacePartitioning {
        return WindowOperationOutcome::InvalidSourceView;
    }

    let target_space_id = query_space_holding_window(target_window);
    let target_view = find_or_create_view_for_space(
        space_manager,
        target_space_id,
        display_manager,
        window_manager,
    );
    let Some(target_view_layout) = space_manager
        .view
        .find(&target_view)
        .map(|view| view.layout)
    else {
        return WindowOperationOutcome::InvalidDestinationView;
    };
    if target_view_layout != ViewLayout::BinarySpacePartitioning {
        return WindowOperationOutcome::InvalidDestinationView;
    }

    let Some(warped_node) = leaf_holding_window(space_manager, warped_view, warped_window) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };

    let Some(target_node) = leaf_holding_window(space_manager, target_view, target_window) else {
        return WindowOperationOutcome::InvalidDestinationNode;
    };

    if (warped_view, warped_node) == (target_view, target_node) {
        return WindowOperationOutcome::SameStack;
    }

    let Some((warped_node_parent, warped_node_window_count)) = space_manager
        .view
        .find(&warped_view)
        .and_then(|view| view.find_node(warped_node))
        .map(|node| (node.parent, node.window_count))
    else {
        return WindowOperationOutcome::InvalidSourceNode;
    };
    let Some(target_node_parent) = space_manager
        .view
        .find(&target_view)
        .and_then(|view| view.find_node(target_node))
        .map(|node| node.parent)
    else {
        return WindowOperationOutcome::InvalidDestinationNode;
    };

    if warped_node_parent.is_some()
        && target_node_parent.is_some()
        && (warped_view, warped_node_parent) == (target_view, target_node_parent)
        && warped_node_window_count == 1
    {
        let target_view_insertion_point = space_manager
            .view
            .find(&target_view)
            .map_or(WindowId(0), |view| view.insertion_point);
        if is_window_in_node(
            target_view,
            target_node,
            target_view_insertion_point,
            space_manager,
        ) {
            if let Some(target_node_parent) = target_node_parent
                && let Some(view) = space_manager.view.find_mut(&target_view)
                && let Some((target_node_split, target_node_child)) = view
                    .find_node(target_node)
                    .map(|node| (node.split, node.child))
                && let Some(parent) = view.find_node_mut(target_node_parent)
            {
                parent.split = target_node_split;
                parent.child = target_node_child;
            }

            remove_window_from_view_tree(
                space_manager,
                warped_view,
                warped_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            forget_managed_window(window_manager, warped_window);
            record_managed_window_on_space_updating_its_shadow(
                window_manager,
                warped_window,
                space_manager,
                target_view,
            );
            let warped_node_add = add_window_to_view_tree_preferring_insertion_point(
                space_manager,
                target_view,
                warped_window,
                target_window,
                display_manager,
                window_manager,
            );

            let mut window_list: Vec<WindowWithTargetFrame> = Vec::new();
            if let Some(warped_node_add) = warped_node_add {
                collect_windows_below_node_with_their_target_areas(
                    target_view,
                    warped_node_add,
                    &mut window_list,
                    window_manager,
                    space_manager,
                );
            }
            move_windows_to_their_target_frames_animating_if_enabled(&window_list, window_manager);
        } else {
            let warped_view_insertion_point = space_manager
                .view
                .find(&warped_view)
                .map_or(WindowId(0), |view| view.insertion_point);
            if is_window_in_node(
                warped_view,
                warped_node,
                warped_view_insertion_point,
                space_manager,
            ) && let Some(view) = space_manager.view.find_mut(&warped_view)
            {
                view.insertion_point = target_window;
            }

            swap_windows_between_nodes_clearing_their_zoom(
                warped_view,
                warped_node,
                target_view,
                target_node,
                space_manager,
            );

            let mut window_list: Vec<WindowWithTargetFrame> = Vec::new();
            collect_windows_below_node_with_their_target_areas(
                warped_view,
                warped_node,
                &mut window_list,
                window_manager,
                space_manager,
            );
            collect_windows_below_node_with_their_target_areas(
                target_view,
                target_node,
                &mut window_list,
                window_manager,
                space_manager,
            );
            move_windows_to_their_target_frames_animating_if_enabled(&window_list, window_manager);
        }
    } else {
        if warped_view == target_view {
            //
            // :NaturalWarp
            //
            // NOTE(asmvik): Precalculate both target areas and select the one that has the closest distance to the source area.
            // This allows the warp to feel more natural in terms of where the window is placed on screen, however, this is only utilized
            // for warp operations where both operands belong to the same space. There may be a better system to handle this if/when multiple
            // monitors should be supported.
            //

            let Some(target_node_area) = space_manager
                .view
                .find(&target_view)
                .and_then(|view| view.find_node(target_node))
                .map(|node| node.area)
            else {
                return WindowOperationOutcome::InvalidDestinationNode;
            };
            let (candidate_first_child_area, candidate_second_child_area) =
                divide_area_into_two_by_split_ratio_and_gap(
                    effective_split_of_node(space_manager, target_view, target_node),
                    effective_window_gap_of_view(space_manager, target_view),
                    effective_ratio_of_node(target_view, target_node, space_manager),
                    target_node_area,
                );

            let Some(warped_node_area) = space_manager
                .view
                .find(&warped_view)
                .and_then(|view| view.find_node(warped_node))
                .map(|node| node.area)
            else {
                return WindowOperationOutcome::InvalidSourceNode;
            };
            let source_node_center_point = CGPoint::new(
                (0.5f32 + warped_node_area.x + warped_node_area.width / 2.0f32) as i32 as f64,
                (0.5f32 + warped_node_area.y + warped_node_area.height / 2.0f32) as i32 as f64,
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

            let target_node_child = if distance_to_candidate_first_child
                < distance_to_candidate_second_child
            {
                WindowNodeChild::First
            } else if distance_to_candidate_first_child > distance_to_candidate_second_child {
                WindowNodeChild::Second
            } else if is_node_the_left_child_of_its_parent(warped_view, warped_node, space_manager)
            {
                WindowNodeChild::First
            } else {
                WindowNodeChild::Second
            };
            if let Some(view) = space_manager.view.find_mut(&target_view)
                && let Some(node) = view.find_node_mut(target_node)
            {
                node.child = target_node_child;
            }

            let warped_node_remove = remove_window_from_view_tree(
                space_manager,
                warped_view,
                warped_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            let warped_node_add = add_window_to_view_tree_preferring_insertion_point(
                space_manager,
                target_view,
                warped_window,
                target_window,
                display_manager,
                window_manager,
            );

            let mut window_list: Vec<WindowWithTargetFrame> = Vec::new();
            if let Some(warped_node_remove) = warped_node_remove {
                collect_windows_below_node_with_their_target_areas(
                    warped_view,
                    warped_node_remove,
                    &mut window_list,
                    window_manager,
                    space_manager,
                );
            }

            if let Some(warped_node_add) = warped_node_add {
                let warped_node_add_parent = space_manager
                    .view
                    .find(&target_view)
                    .and_then(|view| view.find_node(warped_node_add))
                    .and_then(|node| node.parent);
                if warped_node_remove != Some(warped_node_add)
                    && warped_node_remove != warped_node_add_parent
                {
                    collect_windows_below_node_with_their_target_areas(
                        target_view,
                        warped_node_add,
                        &mut window_list,
                        window_manager,
                        space_manager,
                    );
                }
            }

            move_windows_to_their_target_frames_animating_if_enabled(&window_list, window_manager);
        } else {
            if window_manager.focused_window_id == warped_window {
                let next = query_tracked_window_at_rank_on_space_skipping_window(
                    window_manager,
                    warped_view,
                    1,
                    warped_window,
                );
                if let Some(next) = next {
                    focus_and_raise_tracked_window(window_manager, next);
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

            untile_window_from_view_of_space(
                space_manager,
                warped_view,
                warped_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            forget_managed_window(window_manager, warped_window);
            record_managed_window_on_space_updating_its_shadow(
                window_manager,
                warped_window,
                space_manager,
                target_view,
            );
            move_window_to_space_by_whichever_mechanism_this_macos_supports(
                target_view,
                warped_window,
            );
            tile_window_on_space_preferring_insertion_point(
                space_manager,
                warped_window,
                target_view,
                target_window,
                display_manager,
                window_manager,
            );
        }
    }

    WindowOperationOutcome::Success
}

pub(crate) fn swap_managed_windows(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    a_window: WindowId,
    b_window: WindowId,
    display_manager: &mut DisplayManager,
) -> WindowOperationOutcome {
    if a_window == b_window {
        return WindowOperationOutcome::SameWindow;
    }

    let a_space_id = query_space_holding_window(a_window);
    let a_view =
        find_or_create_view_for_space(space_manager, a_space_id, display_manager, window_manager);

    let b_space_id = query_space_holding_window(b_window);
    let b_view =
        find_or_create_view_for_space(space_manager, b_space_id, display_manager, window_manager);

    let Some(a_node) = leaf_holding_window(space_manager, a_view, a_window) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };

    let Some(b_node) = leaf_holding_window(space_manager, b_view, b_window) else {
        return WindowOperationOutcome::InvalidDestinationNode;
    };

    if (a_view, a_node) == (b_view, b_node) {
        let mut a_list_index = 0;
        let mut a_order_index = 0;

        let mut b_list_index = 0;
        let mut b_order_index = 0;

        let Some(view) = space_manager.view.find_mut(&a_view) else {
            return WindowOperationOutcome::InvalidSourceNode;
        };
        let Some(node) = view.find_node_mut(a_node) else {
            return WindowOperationOutcome::InvalidSourceNode;
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
            focus_and_raise_tracked_window(window_manager, b_window);
        } else if b_window == window_manager.focused_window_id {
            focus_and_raise_tracked_window(window_manager, a_window);
        }

        return WindowOperationOutcome::Success;
    }

    let Some(a_view_layout) = space_manager.view.find(&a_view).map(|view| view.layout) else {
        return WindowOperationOutcome::InvalidSourceView;
    };
    if a_view_layout != ViewLayout::BinarySpacePartitioning {
        return WindowOperationOutcome::InvalidSourceView;
    }
    let Some(b_view_layout) = space_manager.view.find(&b_view).map(|view| view.layout) else {
        return WindowOperationOutcome::InvalidDestinationView;
    };
    if b_view_layout != ViewLayout::BinarySpacePartitioning {
        return WindowOperationOutcome::InvalidDestinationView;
    }

    let a_view_insertion_point = space_manager
        .view
        .find(&a_view)
        .map_or(WindowId(0), |view| view.insertion_point);
    let b_view_insertion_point = space_manager
        .view
        .find(&b_view)
        .map_or(WindowId(0), |view| view.insertion_point);
    if is_window_in_node(a_view, a_node, a_view_insertion_point, space_manager) {
        if let Some(view) = space_manager.view.find_mut(&a_view) {
            view.insertion_point = b_window;
        }
    } else if is_window_in_node(b_view, b_node, b_view_insertion_point, space_manager)
        && let Some(view) = space_manager.view.find_mut(&b_view)
    {
        view.insertion_point = a_window;
    }

    let a_visible = is_space_visible_on_its_display(a_view);
    let b_visible = is_space_visible_on_its_display(b_view);

    if a_view != b_view {
        let Some((a_node_window_list, a_node_window_count)) = space_manager
            .view
            .find(&a_view)
            .and_then(|view| view.find_node(a_node))
            .map(|node| (node.window_list, node.window_count))
        else {
            return WindowOperationOutcome::InvalidSourceNode;
        };
        for index in 0..a_node_window_count as usize {
            let window = tracked_window_with_id(window_manager, a_node_window_list[index]);
            forget_managed_window(window_manager, a_node_window_list[index]);
            if let Some(window) = window {
                move_window_to_space_by_whichever_mechanism_this_macos_supports(b_view, window);
                record_managed_window_on_space_updating_its_shadow(
                    window_manager,
                    window,
                    space_manager,
                    b_view,
                );
            }
        }

        let Some((b_node_window_list, b_node_window_count)) = space_manager
            .view
            .find(&b_view)
            .and_then(|view| view.find_node(b_node))
            .map(|node| (node.window_list, node.window_count))
        else {
            return WindowOperationOutcome::InvalidDestinationNode;
        };
        for index in 0..b_node_window_count as usize {
            let window = tracked_window_with_id(window_manager, b_node_window_list[index]);
            forget_managed_window(window_manager, b_node_window_list[index]);
            if let Some(window) = window {
                move_window_to_space_by_whichever_mechanism_this_macos_supports(a_view, window);
                record_managed_window_on_space_updating_its_shadow(
                    window_manager,
                    window,
                    space_manager,
                    a_view,
                );
            }
        }

        if a_visible && !b_visible && a_window == window_manager.focused_window_id {
            focus_and_raise_tracked_window(window_manager, b_window);
        } else if b_visible && !a_visible && b_window == window_manager.focused_window_id {
            focus_and_raise_tracked_window(window_manager, a_window);
        }
    }

    swap_windows_between_nodes_clearing_their_zoom(a_view, a_node, b_view, b_node, space_manager);
    let mut window_list: Vec<WindowWithTargetFrame> = Vec::new();

    if a_visible {
        collect_windows_below_node_with_their_target_areas(
            a_view,
            a_node,
            &mut window_list,
            window_manager,
            space_manager,
        );
    } else if let Some(view) = space_manager.view.find_mut(&a_view) {
        view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
    }

    if b_visible {
        collect_windows_below_node_with_their_target_areas(
            b_view,
            b_node,
            &mut window_list,
            window_manager,
            space_manager,
        );
    } else if let Some(view) = space_manager.view.find_mut(&b_view) {
        view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
    }

    move_windows_to_their_target_frames_animating_if_enabled(&window_list, window_manager);
    WindowOperationOutcome::Success
}
