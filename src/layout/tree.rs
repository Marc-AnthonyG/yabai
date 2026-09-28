use std::collections::VecDeque;

use crate::display::manager::DisplayManager;
use crate::layout::area::{
    Area, bottom_right_pixel_inside_area, distance_from_source_area_to_target_area_in_direction,
    divide_area_into_two_by_split_ratio_and_gap,
    is_target_area_in_direction_of_source_area_and_facing_it,
};
use crate::layout::feedback_window::FeedbackWindow;
use crate::layout::group::keep_the_group_a_window_left_even_with_one_window_remaining;
use crate::layout::group_area::area_given_to_the_windows_of_node;
use crate::layout::group_header::refresh_the_group_headers_of_view;
use crate::layout::group_header_window::GroupHeaderWindow;
use crate::layout::insertion::{
    WindowInsertionPoint, destroy_insert_feedback_of_node, show_insert_feedback_of_node,
};
use crate::layout::settings::{
    ViewLayout, effective_child_for_new_window_in_node, effective_ratio_of_node,
    effective_split_of_node, effective_window_gap_of_view,
};
use crate::layout::view::{View, recompute_view_areas_from_display_bounds_and_padding};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::query_windows_on_space;
use crate::space::manager::SpaceManager;
use crate::support::direction::{
    DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_STACK_INSTEAD_OF_SPLIT,
    DIRECTION_WEST,
};
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::animation::{
    WindowWithTargetFrame, move_windows_to_their_target_frames_animating_if_enabled,
};
use crate::window::manager::{WindowManager, forget_managed_window, tracked_window_with_id};
use crate::window::screen_lookup::rank_of_window_in_list;

#[derive(Clone, Copy)]
pub(crate) struct LeafCountsPerSplitAxis {
    pub(crate) y_count: i32,
    pub(crate) x_count: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum WindowNodeChild {
    #[default]
    None = 0,
    Second = 1,
    First = 2,
}

pub(crate) static WINDOW_NODE_CHILD_NAMES: [&str; 3] = ["none", "second_child", "first_child"];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub(crate) enum WindowNodeSplit {
    #[default]
    None = 0,
    Vertical = 1,
    Horizontal = 2,
    Auto = 3,
}

pub(crate) static WINDOW_NODE_SPLIT_NAMES: [&str; 4] = ["none", "vertical", "horizontal", "auto"];

pub(crate) fn window_node_split_and_child_placing_a_window_inserted_in_direction(
    insert_direction: i32,
) -> Option<(WindowNodeSplit, WindowNodeChild)> {
    match insert_direction {
        DIRECTION_NORTH => Some((WindowNodeSplit::Horizontal, WindowNodeChild::First)),
        DIRECTION_EAST => Some((WindowNodeSplit::Vertical, WindowNodeChild::Second)),
        DIRECTION_SOUTH => Some((WindowNodeSplit::Horizontal, WindowNodeChild::Second)),
        DIRECTION_WEST => Some((WindowNodeSplit::Vertical, WindowNodeChild::First)),
        _ => None,
    }
}

pub(crate) const MOST_WINDOWS_A_NODE_CAN_HOLD: usize = 32;

#[derive(Default)]
pub(crate) struct WindowNode {
    pub(crate) area: Area,
    pub(crate) parent: Option<NodeId>,
    pub(crate) left: Option<NodeId>,
    pub(crate) right: Option<NodeId>,
    pub(crate) zoom: Option<NodeId>,
    pub(crate) window_list: [WindowId; MOST_WINDOWS_A_NODE_CAN_HOLD],
    pub(crate) window_order: [WindowId; MOST_WINDOWS_A_NODE_CAN_HOLD],
    pub(crate) window_count: i32,
    pub(crate) ratio: f32,
    pub(crate) split: WindowNodeSplit,
    pub(crate) child: WindowNodeChild,
    pub(crate) insert_direction: i32,
    pub(crate) feedback_window: Option<FeedbackWindow>,
    pub(crate) group_header: Option<GroupHeaderWindow>,
}

pub(crate) fn divide_node_area_between_its_children(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
) {
    let split = effective_split_of_node(space_manager, space_id, node_id);
    let ratio = effective_ratio_of_node(space_id, node_id, space_manager);
    let gap = effective_window_gap_of_view(space_manager, space_id);

    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };
    let node = view.node(node_id);
    let parent_area = node.area;
    let left = node.left;
    let right = node.right;

    let (left_area, right_area) =
        divide_area_into_two_by_split_ratio_and_gap(split, gap, ratio, parent_area);

    if let Some(left) = left {
        view.node_mut(left).area = left_area;
    }
    if let Some(right) = right {
        view.node_mut(right).area = right_area;
    }

    let node = view.node_mut(node_id);
    node.split = split;
    node.ratio = ratio;
}

pub(crate) fn is_node_holding_any_window(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };

    view.node(node_id).window_count != 0
}

pub(crate) fn is_node_below_the_root(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };

    view.node(node_id).parent.is_some()
}

pub(crate) fn is_leaf_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };
    let node = view.node(node_id);

    node.left.is_none() && node.right.is_none()
}

pub(crate) fn is_node_the_left_child_of_its_parent(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };

    match view.node(node_id).parent {
        Some(parent) => view.node(parent).left == Some(node_id),
        None => false,
    }
}

pub(crate) fn is_node_the_right_child_of_its_parent(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };

    match view.node(node_id).parent {
        Some(parent) => view.node(parent).right == Some(node_id),
        None => false,
    }
}

pub(crate) fn reset_split_ratios_below_node_to_the_global_ratio(
    space_id: SpaceId,
    node_id: NodeId,
    axis_flag: u32,
    space_manager: &mut SpaceManager,
) {
    let Some(node) = space_manager.find_node_in_view_of_space(space_id, node_id) else {
        return;
    };
    let (left, right) = (node.left, node.right);

    if let Some(left) = left {
        reset_split_ratios_below_node_to_the_global_ratio(space_id, left, axis_flag, space_manager);
    }
    if let Some(right) = right {
        reset_split_ratios_below_node_to_the_global_ratio(
            space_id,
            right,
            axis_flag,
            space_manager,
        );
    }

    let split_ratio = space_manager.split_ratio;
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    if (axis_flag & WindowNodeSplit::Vertical as u32) != 0
        && node.split == WindowNodeSplit::Vertical
    {
        node.ratio = split_ratio;
    }

    if (axis_flag & WindowNodeSplit::Horizontal as u32) != 0
        && node.split == WindowNodeSplit::Horizontal
    {
        node.ratio = split_ratio;
    }
}

pub(crate) fn add_leaf_counts_per_split_axis(
    first: LeafCountsPerSplitAxis,
    second: LeafCountsPerSplitAxis,
) -> LeafCountsPerSplitAxis {
    LeafCountsPerSplitAxis {
        y_count: first.y_count + second.y_count,
        x_count: first.x_count + second.x_count,
    }
}

pub(crate) fn balance_split_ratios_below_node_giving_each_leaf_an_equal_share(
    space_id: SpaceId,
    node_id: NodeId,
    axis_flag: u32,
    space_manager: &mut SpaceManager,
) -> LeafCountsPerSplitAxis {
    if is_leaf_node(space_id, node_id, space_manager) {
        let Some(view) = space_manager.view.get(&space_id) else {
            return LeafCountsPerSplitAxis {
                y_count: 0,
                x_count: 0,
            };
        };
        let parent = view.node(node_id).parent;
        return LeafCountsPerSplitAxis {
            y_count: match parent {
                Some(parent) => (view.node(parent).split == WindowNodeSplit::Vertical) as i32,
                None => 0,
            },
            x_count: match parent {
                Some(parent) => (view.node(parent).split == WindowNodeSplit::Horizontal) as i32,
                None => 0,
            },
        };
    }

    let (left, right) = match space_manager.view.get(&space_id) {
        Some(view) => {
            let node = view.node(node_id);
            (node.left, node.right)
        }
        None => {
            return LeafCountsPerSplitAxis {
                y_count: 0,
                x_count: 0,
            };
        }
    };

    let left_leafs = match left {
        Some(left) => balance_split_ratios_below_node_giving_each_leaf_an_equal_share(
            space_id,
            left,
            axis_flag,
            space_manager,
        ),
        None => LeafCountsPerSplitAxis {
            y_count: 0,
            x_count: 0,
        },
    };
    let right_leafs = match right {
        Some(right) => balance_split_ratios_below_node_giving_each_leaf_an_equal_share(
            space_id,
            right,
            axis_flag,
            space_manager,
        ),
        None => LeafCountsPerSplitAxis {
            y_count: 0,
            x_count: 0,
        },
    };
    let mut total_leafs = add_leaf_counts_per_split_axis(left_leafs, right_leafs);

    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return total_leafs;
    };
    let node = view.node_mut(node_id);

    if (axis_flag & WindowNodeSplit::Vertical as u32) != 0 {
        if node.split == WindowNodeSplit::Vertical {
            node.ratio = left_leafs.y_count as f32 / total_leafs.y_count as f32;
            total_leafs.y_count -= 1;
        }
    }

    if (axis_flag & WindowNodeSplit::Horizontal as u32) != 0 {
        if node.split == WindowNodeSplit::Horizontal {
            node.ratio = left_leafs.x_count as f32 / total_leafs.x_count as f32;
            total_leafs.x_count -= 1;
        }
    }

    let parent = node.parent;
    if let Some(parent) = parent {
        let parent_split = view.node(parent).split;
        total_leafs.y_count += (parent_split == WindowNodeSplit::Vertical) as i32;
        total_leafs.x_count += (parent_split == WindowNodeSplit::Horizontal) as i32;
    }

    total_leafs
}

pub(crate) fn split_leaf_node_to_hold_a_new_window(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
) {
    let window_zoom_persist = space_manager.window_zoom_persist;

    let (left, right, zoom) = {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            return;
        };
        let left = view.allocate_empty_node_reusing_a_freed_id();
        let right = view.allocate_empty_node_reusing_a_freed_id();

        let node = view.node(node_id);
        let zoom = if !window_zoom_persist {
            None
        } else if node.zoom.is_none() {
            None
        } else if node.zoom == node.parent {
            Some(node_id)
        } else {
            Some(ROOT_NODE_ID)
        };

        (left, right, zoom)
    };

    if effective_child_for_new_window_in_node(space_id, node_id, space_manager)
        == WindowNodeChild::Second
    {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            return;
        };
        let node = view.node(node_id);
        let window_count = node.window_count;
        let window_list = node.window_list;
        let window_order = node.window_order;

        let left_node = view.node_mut(left);
        left_node.window_list[..window_count as usize]
            .copy_from_slice(&window_list[..window_count as usize]);
        left_node.window_order[..window_count as usize]
            .copy_from_slice(&window_order[..window_count as usize]);
        left_node.window_count = window_count;
        left_node.zoom = zoom;

        let right_node = view.node_mut(right);
        right_node.window_list[0] = window_id;
        right_node.window_order[0] = window_id;
        right_node.window_count = 1;
    } else {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            return;
        };
        let node = view.node(node_id);
        let window_count = node.window_count;
        let window_list = node.window_list;
        let window_order = node.window_order;

        let right_node = view.node_mut(right);
        right_node.window_list[..window_count as usize]
            .copy_from_slice(&window_list[..window_count as usize]);
        right_node.window_order[..window_count as usize]
            .copy_from_slice(&window_order[..window_count as usize]);
        right_node.window_count = window_count;
        right_node.zoom = zoom;

        let left_node = view.node_mut(left);
        left_node.window_list[0] = window_id;
        left_node.window_order[0] = window_id;
        left_node.window_count = 1;
    }

    {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            return;
        };
        view.node_mut(left).parent = Some(node_id);
        view.node_mut(right).parent = Some(node_id);

        let node = view.node_mut(node_id);
        node.window_count = 0;
        node.left = Some(left);
        node.right = Some(right);
        node.zoom = None;
    }

    divide_node_area_between_its_children(space_manager, space_id, node_id);
}

pub(crate) fn recompute_areas_below_node_redrawing_insert_feedback(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
) {
    if is_leaf_node(space_id, node_id, space_manager) {
        let Some(insert_direction) = space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .map(|node| node.insert_direction)
        else {
            return;
        };
        if insert_direction != 0 {
            show_insert_feedback_of_node(space_id, node_id, window_manager, space_manager);
        }
    } else {
        divide_node_area_between_its_children(space_manager, space_id, node_id);

        let Some(node) = space_manager.find_node_in_view_of_space(space_id, node_id) else {
            return;
        };
        let (left, right) = (node.left, node.right);

        if let Some(left) = left {
            recompute_areas_below_node_redrawing_insert_feedback(
                space_manager,
                space_id,
                left,
                window_manager,
            );
        }
        if let Some(right) = right {
            recompute_areas_below_node_redrawing_insert_feedback(
                space_manager,
                space_id,
                right,
                window_manager,
            );
        }
    }
}

pub(crate) fn free_node_subtree_unmanaging_its_windows(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut node_ids = Vec::new();
    let Some(view) = space_manager.view.get(&space_id) else {
        return;
    };
    collect_node_subtree_in_post_order(view, node_id, &mut node_ids);

    for node_id in node_ids {
        let Some(node) = space_manager.find_node_in_view_of_space(space_id, node_id) else {
            return;
        };
        let window_ids = node.window_list[..node.window_count as usize].to_vec();

        for window_id in window_ids {
            forget_managed_window(window_manager, window_id);
        }

        destroy_insert_feedback_of_node(space_id, node_id, window_manager, space_manager);
        free_node_scrubbing_every_reference_to_it(
            space_id,
            node_id,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    }
}

pub(crate) fn clear_zoom_of_node_subtree(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);
    node.zoom = None;
    let left = node.left;
    let right = node.right;

    if !is_leaf_node(space_id, node_id, space_manager) {
        if let Some(left) = left {
            clear_zoom_of_node_subtree(space_id, left, space_manager);
        }
        if let Some(right) = right {
            clear_zoom_of_node_subtree(space_id, right, space_manager);
        }
    }
}

pub(crate) fn collect_windows_below_node_with_their_target_areas(
    space_id: SpaceId,
    node_id: NodeId,
    window_list: &mut Vec<WindowWithTargetFrame>,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    if is_leaf_node(space_id, node_id, space_manager) {
        let (window_count, node_window_list, area) = match space_manager.view.get(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                let area = area_given_to_the_windows_of_node(view, node, window_manager);
                (node.window_count, node.window_list, area)
            }
            None => return,
        };

        for index in 0..window_count as usize {
            if tracked_window_with_id(window_manager, node_window_list[index]).is_some() {
                window_list.push(WindowWithTargetFrame {
                    window_id: node_window_list[index],
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: area.height,
                });
            }
        }
    } else {
        let Some(node) = space_manager.find_node_in_view_of_space(space_id, node_id) else {
            return;
        };
        let (left, right) = (node.left, node.right);

        if let Some(left) = left {
            collect_windows_below_node_with_their_target_areas(
                space_id,
                left,
                window_list,
                window_manager,
                space_manager,
            );
        }
        if let Some(right) = right {
            collect_windows_below_node_with_their_target_areas(
                space_id,
                right,
                window_list,
                window_manager,
                space_manager,
            );
        }
    }
}

pub(crate) fn move_windows_below_node_into_their_areas(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let mut window_list: Vec<WindowWithTargetFrame> = Vec::new();
    collect_windows_below_node_with_their_target_areas(
        space_id,
        node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    if !window_list.is_empty() {
        move_windows_to_their_target_frames_animating_if_enabled(&window_list, window_manager);
    }
    refresh_the_group_headers_of_view(space_id, space_manager, window_manager);
}

pub(crate) fn is_window_in_node(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };
    let node = view.node(node_id);

    for index in 0..node.window_count {
        if node.window_list[index as usize] == window_id {
            return true;
        }
    }

    false
}

pub(crate) fn stack_index_of_window_in_node(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> i32 {
    let Some(view) = space_manager.view.get(&space_id) else {
        return 0;
    };
    let node = view.node(node_id);

    for index in 0..node.window_count {
        if node.window_list[index as usize] == window_id {
            return index;
        }
    }

    0
}

pub(crate) fn swap_windows_between_nodes_clearing_their_zoom(
    a_space_id: SpaceId,
    a_node_id: NodeId,
    b_space_id: SpaceId,
    b_node_id: NodeId,
    space_manager: &mut SpaceManager,
) {
    let Some(a_node) = space_manager.find_node_in_view_of_space(a_space_id, a_node_id) else {
        return;
    };
    let (a_window_list, a_window_order, a_window_count) =
        (a_node.window_list, a_node.window_order, a_node.window_count);
    let Some(b_node) = space_manager.find_node_in_view_of_space(b_space_id, b_node_id) else {
        return;
    };
    let (b_window_list, b_window_order, b_window_count) =
        (b_node.window_list, b_node.window_order, b_node.window_count);

    {
        let Some(view) = space_manager.view.get_mut(&a_space_id) else {
            return;
        };
        let node = view.node_mut(a_node_id);
        node.window_list[..b_window_count as usize]
            .copy_from_slice(&b_window_list[..b_window_count as usize]);
        node.window_order[..b_window_count as usize]
            .copy_from_slice(&b_window_order[..b_window_count as usize]);
        node.window_count = b_window_count;
    }

    {
        let Some(view) = space_manager.view.get_mut(&b_space_id) else {
            return;
        };
        let node = view.node_mut(b_node_id);
        node.window_list[..a_window_count as usize]
            .copy_from_slice(&a_window_list[..a_window_count as usize]);
        node.window_order[..a_window_count as usize]
            .copy_from_slice(&a_window_order[..a_window_count as usize]);
        node.window_count = a_window_count;
    }

    if let Some(view) = space_manager.view.get_mut(&a_space_id) {
        view.node_mut(a_node_id).zoom = None;
    }
    if let Some(view) = space_manager.view.get_mut(&b_space_id) {
        view.node_mut(b_node_id).zoom = None;
    }
}

pub(crate) fn first_leaf_below_node(
    space_id: SpaceId,
    root_node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> NodeId {
    let mut node_id = root_node_id;
    while !is_leaf_node(space_id, node_id, space_manager) {
        node_id = match space_manager.view.get(&space_id) {
            Some(view) => match view.node(node_id).left {
                Some(left) => left,
                None => return node_id,
            },
            None => return node_id,
        };
    }
    node_id
}

pub(crate) fn last_leaf_below_node(
    space_id: SpaceId,
    root_node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> NodeId {
    let mut node_id = root_node_id;
    while !is_leaf_node(space_id, node_id, space_manager) {
        node_id = match space_manager.view.get(&space_id) {
            Some(view) => match view.node(node_id).right {
                Some(right) => right,
                None => return node_id,
            },
            None => return node_id,
        };
    }
    node_id
}

pub(crate) fn previous_leaf_in_tree_order(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let parent = space_manager.view.get(&space_id)?.node(node_id).parent?;

    if is_node_the_left_child_of_its_parent(space_id, node_id, space_manager) {
        return previous_leaf_in_tree_order(space_id, parent, space_manager);
    }

    let parent_left = space_manager.view.get(&space_id)?.node(parent).left?;

    if is_leaf_node(space_id, parent_left, space_manager) {
        return Some(parent_left);
    }

    let parent_left_right = space_manager.view.get(&space_id)?.node(parent_left).right?;

    Some(last_leaf_below_node(
        space_id,
        parent_left_right,
        space_manager,
    ))
}

pub(crate) fn next_leaf_in_tree_order(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let parent = space_manager.view.get(&space_id)?.node(node_id).parent?;

    if is_node_the_right_child_of_its_parent(space_id, node_id, space_manager) {
        return next_leaf_in_tree_order(space_id, parent, space_manager);
    }

    let parent_right = space_manager.view.get(&space_id)?.node(parent).right?;

    if is_leaf_node(space_id, parent_right, space_manager) {
        return Some(parent_right);
    }

    let parent_right_left = space_manager.view.get(&space_id)?.node(parent_right).left?;

    Some(first_leaf_below_node(
        space_id,
        parent_right_left,
        space_manager,
    ))
}

pub(crate) fn rotate_node_subtree_by_degrees(
    space_id: SpaceId,
    node_id: NodeId,
    degrees: i32,
    space_manager: &mut SpaceManager,
) {
    let (left, right) = {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            return;
        };
        let node = view.node_mut(node_id);

        if (degrees == 90 && node.split == WindowNodeSplit::Vertical)
            || (degrees == 270 && node.split == WindowNodeSplit::Horizontal)
            || (degrees == 180)
        {
            let temporary = node.left;
            node.left = node.right;
            node.right = temporary;
            node.ratio = 1.0f32 - node.ratio;
        }

        if degrees != 180 {
            if node.split == WindowNodeSplit::Horizontal {
                node.split = WindowNodeSplit::Vertical;
            } else if node.split == WindowNodeSplit::Vertical {
                node.split = WindowNodeSplit::Horizontal;
            }
        }

        (node.left, node.right)
    };

    if !is_leaf_node(space_id, node_id, space_manager) {
        if let Some(left) = left {
            rotate_node_subtree_by_degrees(space_id, left, degrees, space_manager);
        }
        if let Some(right) = right {
            rotate_node_subtree_by_degrees(space_id, right, degrees, space_manager);
        }
    }
}

pub(crate) fn mirror_node_subtree_along_axis(
    space_id: SpaceId,
    node_id: NodeId,
    axis: WindowNodeSplit,
    space_manager: &mut SpaceManager,
) -> NodeId {
    if !is_leaf_node(space_id, node_id, space_manager) {
        let Some(node) = space_manager.find_node_in_view_of_space(space_id, node_id) else {
            return node_id;
        };
        let (node_left, node_right) = (node.left, node.right);

        let left = node_left
            .map(|left| mirror_node_subtree_along_axis(space_id, left, axis, space_manager));
        let right = node_right
            .map(|right| mirror_node_subtree_along_axis(space_id, right, axis, space_manager));

        let Some(view) = space_manager.view.get_mut(&space_id) else {
            return node_id;
        };
        let node = view.node_mut(node_id);
        if node.split == axis {
            node.left = right;
            node.right = left;
        }
    }

    node_id
}

pub(crate) fn ancestor_whose_split_borders_node_in_direction(
    space_id: SpaceId,
    node_id: NodeId,
    direction: i32,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let view = space_manager.view.get(&space_id)?;
    let node_area = view.node(node_id).area;
    let mut parent_id = view.node(node_id).parent;

    while let Some(parent) = parent_id {
        let parent_node = view.node(parent);

        if (direction == DIRECTION_NORTH
            && parent_node.split == WindowNodeSplit::Horizontal
            && parent_node.area.y < node_area.y)
            || (direction == DIRECTION_WEST
                && parent_node.split == WindowNodeSplit::Vertical
                && parent_node.area.x < node_area.x)
            || (direction == DIRECTION_SOUTH
                && parent_node.split == WindowNodeSplit::Horizontal
                && (parent_node.area.y + parent_node.area.height)
                    > (node_area.y + node_area.height))
            || (direction == DIRECTION_EAST
                && parent_node.split == WindowNodeSplit::Vertical
                && (parent_node.area.x + parent_node.area.width) > (node_area.x + node_area.width))
        {
            return Some(parent);
        }

        parent_id = parent_node.parent;
    }

    None
}

pub(crate) fn shallowest_leaf_below_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let mut list: VecDeque<NodeId> = VecDeque::from([node_id]);

    while let Some(current_node_id) = list.pop_front() {
        if is_leaf_node(space_id, current_node_id, space_manager) {
            return Some(current_node_id);
        }

        let node = space_manager.view.get(&space_id)?.node(current_node_id);
        let (left, right) = (node.left, node.right);

        if let Some(left) = left {
            list.push_back(left);
        }
        if let Some(right) = right {
            list.push_back(right);
        }
    }

    None
}

pub(crate) fn closest_leaf_in_direction_of_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    source_node_id: NodeId,
    direction: i32,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    let window_list = query_windows_on_space(space_id, false, window_manager)?;

    let mut best_distance = i32::MAX;
    let mut best_rank = i32::MAX;
    let mut best_node: Option<NodeId> = None;
    let source_area = space_manager.view.get(&space_id)?.node(source_node_id).area;
    let source_area_max = bottom_right_pixel_inside_area(source_area);

    let mut target = Some(first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
    while let Some(target_node_id) = target {
        if source_node_id != target_node_id {
            let (target_area, target_first_window_id) = {
                let target_node = space_manager.view.get(&space_id)?.node(target_node_id);
                (target_node.area, target_node.window_order[0])
            };

            let target_area_max = bottom_right_pixel_inside_area(target_area);
            if is_target_area_in_direction_of_source_area_and_facing_it(
                &source_area,
                source_area_max,
                &target_area,
                target_area_max,
                direction,
            ) {
                let distance = distance_from_source_area_to_target_area_in_direction(
                    &source_area,
                    source_area_max,
                    &target_area,
                    target_area_max,
                    direction,
                );
                let rank = rank_of_window_in_list(target_first_window_id, &window_list);
                if (distance < best_distance) || (distance == best_distance && rank < best_rank) {
                    best_node = Some(target_node_id);
                    best_distance = distance;
                    best_rank = rank;
                }
            }
        }

        target = next_leaf_in_tree_order(space_id, target_node_id, space_manager);
    }

    best_node
}

pub(crate) fn leaf_holding_window(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
) -> Option<NodeId> {
    let mut node = Some(first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
    while let Some(node_id) = node {
        if is_window_in_node(space_id, node_id, window_id, space_manager) {
            return Some(node_id);
        }

        node = next_leaf_in_tree_order(space_id, node_id, space_manager);
    }

    None
}

pub(crate) fn remove_window_from_view_tree(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) -> Option<NodeId> {
    let node_id = leaf_holding_window(space_manager, space_id, window_id)?;

    if space_manager
        .view
        .get(&space_id)?
        .node(node_id)
        .window_count
        > 1
    {
        let view = space_manager.view.get_mut(&space_id)?;
        let node = view.node_mut(node_id);
        let mut removed_entry = false;
        let mut removed_order = false;

        for index in 0..node.window_count {
            let index = index as usize;
            let window_count = node.window_count as usize;

            if !removed_entry && node.window_list[index] == window_id {
                node.window_list.copy_within(index + 1..window_count, index);
                removed_entry = true;
            }

            if !removed_order && node.window_order[index] == window_id {
                node.window_order
                    .copy_within(index + 1..window_count, index);
                removed_order = true;
            }
        }

        debug_assert!(removed_entry);
        debug_assert!(removed_order);
        node.window_count -= 1;
        let node_first_window_id = node.window_order[0];
        let window_left_alone_in_the_group =
            (node.window_count == 1).then_some(node_first_window_id);

        if view.insertion_point == window_id {
            view.insertion_point = node_first_window_id;
        }

        let layout = view.layout;
        keep_the_group_a_window_left_even_with_one_window_remaining(
            window_manager,
            window_id,
            window_left_alone_in_the_group,
            layout,
        );
        return None;
    }

    if node_id == ROOT_NODE_ID {
        space_manager.view.get_mut(&space_id)?.insertion_point = WindowId(0);
        destroy_insert_feedback_of_node(space_id, node_id, window_manager, space_manager);
        *space_manager.view.get_mut(&space_id)?.node_mut(node_id) = WindowNode::default();
        recompute_view_areas_from_display_bounds_and_padding(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        );
        return None;
    }

    let window_zoom_persist = space_manager.window_zoom_persist;

    let (
        parent_id,
        parent_parent,
        child_id,
        child_window_list,
        child_window_order,
        child_window_count,
        child_parent,
        child_zoom,
        child_left,
        child_right,
        child_left_zoom,
        child_right_zoom,
        child_insert_direction,
        child_split,
        child_child,
    ) = {
        let view = space_manager.view.get(&space_id)?;
        let parent_id = view.node(node_id).parent?;
        let parent = view.node(parent_id);
        let child_id = if parent.right == Some(node_id) {
            parent.left?
        } else {
            parent.right?
        };
        let child = view.node(child_id);
        (
            parent_id,
            parent.parent,
            child_id,
            child.window_list,
            child.window_order,
            child.window_count,
            child.parent,
            child.zoom,
            child.left,
            child.right,
            child.left.and_then(|left| view.node(left).zoom),
            child.right.and_then(|right| view.node(right).zoom),
            child.insert_direction,
            child.split,
            child.child,
        )
    };

    let parent_zoom = if !window_zoom_persist {
        None
    } else if child_zoom.is_none() {
        None
    } else if child_zoom == Some(parent_id) {
        parent_parent
    } else {
        Some(ROOT_NODE_ID)
    };

    {
        let view = space_manager.view.get_mut(&space_id)?;
        let parent = view.node_mut(parent_id);
        parent.window_list[..child_window_count as usize]
            .copy_from_slice(&child_window_list[..child_window_count as usize]);
        parent.window_order[..child_window_count as usize]
            .copy_from_slice(&child_window_order[..child_window_count as usize]);
        parent.window_count = child_window_count;

        parent.left = None;
        parent.right = None;
        parent.zoom = parent_zoom;
    }

    if child_insert_direction != 0 {
        let parent_first_window_id = {
            let view = space_manager.view.get_mut(&space_id)?;
            let feedback_window = view.node_mut(child_id).feedback_window.take();
            let parent = view.node_mut(parent_id);
            parent.feedback_window = feedback_window;
            parent.insert_direction = child_insert_direction;
            parent.split = child_split;
            parent.child = child_child;
            parent.window_order[0]
        };
        window_manager
            .insert_feedback
            .insert(parent_first_window_id, (space_id, parent_id));
        if mouse_drag_state.feedback_node == Some((space_id, child_id)) {
            mouse_drag_state.feedback_node = Some((space_id, parent_id));
        }
        show_insert_feedback_of_node(space_id, parent_id, window_manager, space_manager);
    }

    if child_parent.is_some() && !(child_left.is_none() && child_right.is_none()) {
        {
            let view = space_manager.view.get_mut(&space_id)?;

            view.node_mut(parent_id).left = child_left;
            if let Some(left) = child_left {
                let left_node = view.node_mut(left);
                left_node.parent = Some(parent_id);
                left_node.zoom = if !window_zoom_persist {
                    None
                } else if child_left_zoom.is_none() {
                    None
                } else if child_left_zoom == Some(child_id) {
                    Some(parent_id)
                } else {
                    Some(ROOT_NODE_ID)
                };
            }

            view.node_mut(parent_id).right = child_right;
            if let Some(right) = child_right {
                let right_node = view.node_mut(right);
                right_node.parent = Some(parent_id);
                right_node.zoom = if !window_zoom_persist {
                    None
                } else if child_right_zoom.is_none() {
                    None
                } else if child_right_zoom == Some(child_id) {
                    Some(parent_id)
                } else {
                    Some(ROOT_NODE_ID)
                };
            }
        }

        if !window_zoom_persist {
            clear_zoom_of_node_subtree(space_id, parent_id, space_manager);
        }

        recompute_areas_below_node_redrawing_insert_feedback(
            space_manager,
            space_id,
            parent_id,
            window_manager,
        );
    }

    destroy_insert_feedback_of_node(space_id, node_id, window_manager, space_manager);
    free_node_scrubbing_every_reference_to_it(
        space_id,
        child_id,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
    free_node_scrubbing_every_reference_to_it(
        space_id,
        node_id,
        window_manager,
        space_manager,
        mouse_drag_state,
    );

    let auto_balance = space_manager.view.get(&space_id)?.auto_balance;
    if auto_balance != WindowNodeSplit::None as u32 {
        balance_split_ratios_below_node_giving_each_leaf_an_equal_share(
            space_id,
            ROOT_NODE_ID,
            auto_balance,
            space_manager,
        );
        recompute_view_areas_from_display_bounds_and_padding(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        );
        return Some(ROOT_NODE_ID);
    }

    Some(parent_id)
}

pub(crate) fn stack_window_in_node_as_its_front_window(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    if node.window_count as usize >= MOST_WINDOWS_A_NODE_CAN_HOLD {
        return;
    }

    let mut insert_index = node.window_count;

    for index in 0..node.window_count {
        if node.window_list[index as usize] == node.window_order[0] {
            insert_index = index + 1;
            break;
        }
    }

    if insert_index < node.window_count {
        node.window_list.copy_within(
            insert_index as usize..node.window_count as usize,
            insert_index as usize + 1,
        );
    }

    node.window_list[insert_index as usize] = window_id;
    node.window_order
        .copy_within(0..node.window_count as usize, 1);
    node.window_order[0] = window_id;
    node.window_count += 1;
}

pub(crate) fn add_window_to_view_tree_preferring_insertion_point(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    insertion_point: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    if !is_node_holding_any_window(space_id, ROOT_NODE_ID, space_manager)
        && is_leaf_node(space_id, ROOT_NODE_ID, space_manager)
    {
        let root = space_manager
            .view
            .get_mut(&space_id)?
            .node_mut(ROOT_NODE_ID);
        root.window_list[0] = window_id;
        root.window_order[0] = window_id;
        root.window_count = 1;
        return Some(ROOT_NODE_ID);
    }

    let layout = space_manager.view.get(&space_id)?.layout;

    if layout == ViewLayout::BinarySpacePartitioning {
        let mut previous_insertion_point = WindowId(0);
        let mut leaf: Option<NodeId> = None;

        if insertion_point != WindowId(0) {
            let view = space_manager.view.get_mut(&space_id)?;
            previous_insertion_point = view.insertion_point;
            view.insertion_point = insertion_point;
        }

        let view_insertion_point = space_manager.view.get(&space_id)?.insertion_point;
        if view_insertion_point != WindowId(0) {
            leaf = leaf_holding_window(space_manager, space_id, view_insertion_point);
            space_manager.view.get_mut(&space_id)?.insertion_point = previous_insertion_point;

            if let Some(leaf) = leaf {
                let do_stack = {
                    let leaf_node = space_manager.view.get_mut(&space_id)?.node_mut(leaf);
                    let do_stack = leaf_node.insert_direction == DIRECTION_STACK_INSTEAD_OF_SPLIT;

                    leaf_node.insert_direction = 0;
                    do_stack
                };
                destroy_insert_feedback_of_node(space_id, leaf, window_manager, space_manager);

                if do_stack {
                    stack_window_in_node_as_its_front_window(
                        space_id,
                        leaf,
                        window_id,
                        space_manager,
                    );
                    return Some(leaf);
                }
            }
        }

        if leaf.is_none() {
            if space_manager.window_insertion_point == WindowInsertionPoint::Focused {
                leaf =
                    leaf_holding_window(space_manager, space_id, window_manager.focused_window_id);
            } else if space_manager.window_insertion_point == WindowInsertionPoint::First {
                leaf = Some(first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
            } else if space_manager.window_insertion_point == WindowInsertionPoint::Last {
                leaf = Some(last_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
            }

            if leaf.is_none() {
                leaf = shallowest_leaf_below_node(space_id, ROOT_NODE_ID, space_manager);
            }
        }

        let leaf = leaf?;

        split_leaf_node_to_hold_a_new_window(space_manager, space_id, leaf, window_id);

        let auto_balance = space_manager.view.get(&space_id)?.auto_balance;
        if auto_balance != WindowNodeSplit::None as u32 {
            balance_split_ratios_below_node_giving_each_leaf_an_equal_share(
                space_id,
                ROOT_NODE_ID,
                auto_balance,
                space_manager,
            );
            recompute_view_areas_from_display_bounds_and_padding(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
            return Some(ROOT_NODE_ID);
        }

        return Some(leaf);
    } else if layout == ViewLayout::Stack {
        stack_window_in_node_as_its_front_window(space_id, ROOT_NODE_ID, window_id, space_manager);
        return Some(ROOT_NODE_ID);
    }

    None
}

pub(crate) fn add_window_to_view_tree(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    add_window_to_view_tree_preferring_insertion_point(
        space_manager,
        space_id,
        window_id,
        WindowId(0),
        display_manager,
        window_manager,
    )
}

pub(crate) fn collect_windows_of_view_in_tree_order(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> Vec<WindowId> {
    let mut window_list: Vec<WindowId> = Vec::new();

    if !space_manager.view.contains_key(&space_id) {
        return window_list;
    }

    let mut node = Some(first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
    while let Some(node_id) = node {
        if let Some(view) = space_manager.view.get(&space_id) {
            let leaf = view.node(node_id);
            for index in 0..leaf.window_count {
                window_list.push(leaf.window_list[index as usize]);
            }
        }

        node = next_leaf_in_tree_order(space_id, node_id, space_manager);
    }

    window_list
}

pub(crate) fn free_node_scrubbing_every_reference_to_it(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if mouse_drag_state.feedback_node == Some((space_id, node_id)) {
        mouse_drag_state.feedback_node = None;
    }

    let window_ids_whose_insert_feedback_names_the_freed_node: Vec<WindowId> = window_manager
        .insert_feedback
        .iter()
        .filter(|(_, feedback_node)| **feedback_node == (space_id, node_id))
        .map(|(window_id, _)| *window_id)
        .collect();
    for window_id in window_ids_whose_insert_feedback_names_the_freed_node {
        window_manager.insert_feedback.remove(&window_id);
    }

    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };
    view.nodes[node_id.0 as usize] = None;
    view.free_node_ids.push(node_id);
}

pub(crate) fn collect_node_subtree_in_post_order(
    view: &View,
    node_id: NodeId,
    out: &mut Vec<NodeId>,
) {
    let node = view.node(node_id);
    if let Some(left) = node.left {
        collect_node_subtree_in_post_order(view, left, out);
    }
    if let Some(right) = node.right {
        collect_node_subtree_in_post_order(view, right, out);
    }
    out.push(node_id);
}
