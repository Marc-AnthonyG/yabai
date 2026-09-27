use std::collections::VecDeque;

use crate::display::manager::DisplayManager;
use crate::layout::area::{
    Area, area_distance_in_direction, area_is_in_direction, area_make_pair, area_max_point,
};
use crate::layout::insertion::{
    FeedbackWindow, WindowInsertionPoint, insert_feedback_destroy, insert_feedback_show,
};
use crate::layout::settings::{
    ViewType, window_node_get_child, window_node_get_gap, window_node_get_ratio,
    window_node_get_split,
};
use crate::layout::view::{View, view_update};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::space_window_list;
use crate::space::manager::SpaceManager;
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, STACK};
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::animation::{WindowCapture, window_manager_animate_window_list};
use crate::window::manager::{
    WindowManager, window_manager_find_window, window_manager_remove_managed_window,
};
use crate::window::screen_lookup::window_manager_find_rank_of_window_in_list;

#[derive(Clone, Copy)]
pub(crate) struct BalanceNode {
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

pub(crate) static WINDOW_NODE_CHILD_STR: [&str; 3] = ["none", "second_child", "first_child"];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub(crate) enum WindowNodeSplit {
    #[default]
    None = 0,
    Y = 1,
    X = 2,
    Auto = 3,
}

pub(crate) static WINDOW_NODE_SPLIT_STR: [&str; 4] = ["none", "vertical", "horizontal", "auto"];

pub(crate) const NODE_MAX_WINDOW_COUNT: usize = 32;

#[derive(Default)]
pub(crate) struct WindowNode {
    pub(crate) area: Area,
    pub(crate) parent: Option<NodeId>,
    pub(crate) left: Option<NodeId>,
    pub(crate) right: Option<NodeId>,
    pub(crate) zoom: Option<NodeId>,
    pub(crate) window_list: [WindowId; NODE_MAX_WINDOW_COUNT],
    pub(crate) window_order: [WindowId; NODE_MAX_WINDOW_COUNT],
    pub(crate) window_count: i32,
    pub(crate) ratio: f32,
    pub(crate) split: WindowNodeSplit,
    pub(crate) child: WindowNodeChild,
    pub(crate) insert_direction: i32,
    pub(crate) feedback_window: Option<FeedbackWindow>,
}

pub(crate) fn area_make_pair_for_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
) {
    let split = window_node_get_split(space_manager, space_id, node_id);
    let ratio = window_node_get_ratio(space_id, node_id, space_manager);
    let gap = window_node_get_gap(space_manager, space_id);

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node(node_id);
    let parent_area = node.area;
    let left = node.left;
    let right = node.right;

    let (left_area, right_area) = area_make_pair(split, gap, ratio, parent_area);

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

pub(crate) fn window_node_is_occupied(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    view.node(node_id).window_count != 0
}

pub(crate) fn window_node_is_intermediate(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    view.node(node_id).parent.is_some()
}

pub(crate) fn window_node_is_leaf(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    let node = view.node(node_id);

    node.left.is_none() && node.right.is_none()
}

pub(crate) fn window_node_is_left_child(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    match view.node(node_id).parent {
        Some(parent) => view.node(parent).left == Some(node_id),
        None => false,
    }
}

pub(crate) fn window_node_is_right_child(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    match view.node(node_id).parent {
        Some(parent) => view.node(parent).right == Some(node_id),
        None => false,
    }
}

pub(crate) fn window_node_equalize(
    space_id: SpaceId,
    node_id: NodeId,
    axis_flag: u32,
    space_manager: &mut SpaceManager,
) {
    let (left, right) = match space_manager.view.find(&space_id) {
        Some(view) => {
            let node = view.node(node_id);
            (node.left, node.right)
        }
        None => return,
    };

    if let Some(left) = left {
        window_node_equalize(space_id, left, axis_flag, space_manager);
    }
    if let Some(right) = right {
        window_node_equalize(space_id, right, axis_flag, space_manager);
    }

    let split_ratio = space_manager.split_ratio;
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    if (axis_flag & WindowNodeSplit::Y as u32) != 0 && node.split == WindowNodeSplit::Y {
        node.ratio = split_ratio;
    }

    if (axis_flag & WindowNodeSplit::X as u32) != 0 && node.split == WindowNodeSplit::X {
        node.ratio = split_ratio;
    }
}

pub(crate) fn balance_node_add(first: BalanceNode, second: BalanceNode) -> BalanceNode {
    BalanceNode {
        y_count: first.y_count + second.y_count,
        x_count: first.x_count + second.x_count,
    }
}

pub(crate) fn window_node_balance(
    space_id: SpaceId,
    node_id: NodeId,
    axis_flag: u32,
    space_manager: &mut SpaceManager,
) -> BalanceNode {
    if window_node_is_leaf(space_id, node_id, space_manager) {
        let Some(view) = space_manager.view.find(&space_id) else {
            return BalanceNode {
                y_count: 0,
                x_count: 0,
            };
        };
        let parent = view.node(node_id).parent;
        return BalanceNode {
            y_count: match parent {
                Some(parent) => (view.node(parent).split == WindowNodeSplit::Y) as i32,
                None => 0,
            },
            x_count: match parent {
                Some(parent) => (view.node(parent).split == WindowNodeSplit::X) as i32,
                None => 0,
            },
        };
    }

    let (left, right) = match space_manager.view.find(&space_id) {
        Some(view) => {
            let node = view.node(node_id);
            (node.left, node.right)
        }
        None => {
            return BalanceNode {
                y_count: 0,
                x_count: 0,
            };
        }
    };

    let left_leafs = match left {
        Some(left) => window_node_balance(space_id, left, axis_flag, space_manager),
        None => BalanceNode {
            y_count: 0,
            x_count: 0,
        },
    };
    let right_leafs = match right {
        Some(right) => window_node_balance(space_id, right, axis_flag, space_manager),
        None => BalanceNode {
            y_count: 0,
            x_count: 0,
        },
    };
    let mut total_leafs = balance_node_add(left_leafs, right_leafs);

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return total_leafs;
    };
    let node = view.node_mut(node_id);

    if (axis_flag & WindowNodeSplit::Y as u32) != 0 {
        if node.split == WindowNodeSplit::Y {
            node.ratio = left_leafs.y_count as f32 / total_leafs.y_count as f32;
            total_leafs.y_count -= 1;
        }
    }

    if (axis_flag & WindowNodeSplit::X as u32) != 0 {
        if node.split == WindowNodeSplit::X {
            node.ratio = left_leafs.x_count as f32 / total_leafs.x_count as f32;
            total_leafs.x_count -= 1;
        }
    }

    let parent = node.parent;
    if let Some(parent) = parent {
        let parent_split = view.node(parent).split;
        total_leafs.y_count += (parent_split == WindowNodeSplit::Y) as i32;
        total_leafs.x_count += (parent_split == WindowNodeSplit::X) as i32;
    }

    total_leafs
}

pub(crate) fn window_node_split(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
) {
    let window_zoom_persist = space_manager.window_zoom_persist;

    let (left, right, zoom) = {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let left = view.allocate_node();
        let right = view.allocate_node();

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

    if window_node_get_child(space_id, node_id, space_manager) == WindowNodeChild::Second {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
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
        let Some(view) = space_manager.view.find_mut(&space_id) else {
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
        let Some(view) = space_manager.view.find_mut(&space_id) else {
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

    area_make_pair_for_node(space_manager, space_id, node_id);
}

pub(crate) fn window_node_update(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
) {
    if window_node_is_leaf(space_id, node_id, space_manager) {
        let insert_direction = match space_manager.view.find(&space_id) {
            Some(view) => view.node(node_id).insert_direction,
            None => return,
        };
        if insert_direction != 0 {
            insert_feedback_show(space_id, node_id, window_manager, space_manager);
        }
    } else {
        area_make_pair_for_node(space_manager, space_id, node_id);

        let (left, right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                (node.left, node.right)
            }
            None => return,
        };

        if let Some(left) = left {
            window_node_update(space_manager, space_id, left, window_manager);
        }
        if let Some(right) = right {
            window_node_update(space_manager, space_id, right, window_manager);
        }
    }
}

pub(crate) fn window_node_destroy(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut node_ids = Vec::new();
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    window_node_collect_subtree_post_order(view, node_id, &mut node_ids);

    for node_id in node_ids {
        let window_ids = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                node.window_list[..node.window_count as usize].to_vec()
            }
            None => return,
        };

        for window_id in window_ids {
            window_manager_remove_managed_window(window_manager, window_id);
        }

        insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
        view_free_node(space_id, node_id, window_manager, space_manager, mouse_drag_state);
    }
}

pub(crate) fn window_node_clear_zoom(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);
    node.zoom = None;
    let left = node.left;
    let right = node.right;

    if !window_node_is_leaf(space_id, node_id, space_manager) {
        if let Some(left) = left {
            window_node_clear_zoom(space_id, left, space_manager);
        }
        if let Some(right) = right {
            window_node_clear_zoom(space_id, right, space_manager);
        }
    }
}

pub(crate) fn window_node_capture_windows(
    space_id: SpaceId,
    node_id: NodeId,
    window_list: &mut Vec<WindowCapture>,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    if window_node_is_leaf(space_id, node_id, space_manager) {
        let (window_count, node_window_list, area) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                let area = match node.zoom {
                    Some(zoom) => view.node(zoom).area,
                    None => node.area,
                };
                (node.window_count, node.window_list, area)
            }
            None => return,
        };

        for index in 0..window_count as usize {
            if window_manager_find_window(window_manager, node_window_list[index]).is_some() {
                window_list.push(WindowCapture {
                    window_id: node_window_list[index],
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: area.height,
                });
            }
        }
    } else {
        let (left, right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                (node.left, node.right)
            }
            None => return,
        };

        if let Some(left) = left {
            window_node_capture_windows(space_id, left, window_list, window_manager, space_manager);
        }
        if let Some(right) = right {
            window_node_capture_windows(
                space_id,
                right,
                window_list,
                window_manager,
                space_manager,
            );
        }
    }
}

pub(crate) fn window_node_flush(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let mut window_list: Vec<WindowCapture> = Vec::new();
    window_node_capture_windows(
        space_id,
        node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    if !window_list.is_empty() {
        window_manager_animate_window_list(&window_list, window_manager);
    }
}

pub(crate) fn window_node_contains_window(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
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

pub(crate) fn window_node_index_of_window(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> i32 {
    let Some(view) = space_manager.view.find(&space_id) else {
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

pub(crate) fn window_node_swap_window_list(
    a_space_id: SpaceId,
    a_node_id: NodeId,
    b_space_id: SpaceId,
    b_node_id: NodeId,
    space_manager: &mut SpaceManager,
) {
    let (a_window_list, a_window_order, a_window_count) = match space_manager.view.find(&a_space_id)
    {
        Some(view) => {
            let node = view.node(a_node_id);
            (node.window_list, node.window_order, node.window_count)
        }
        None => return,
    };
    let (b_window_list, b_window_order, b_window_count) = match space_manager.view.find(&b_space_id)
    {
        Some(view) => {
            let node = view.node(b_node_id);
            (node.window_list, node.window_order, node.window_count)
        }
        None => return,
    };

    {
        let Some(view) = space_manager.view.find_mut(&a_space_id) else {
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
        let Some(view) = space_manager.view.find_mut(&b_space_id) else {
            return;
        };
        let node = view.node_mut(b_node_id);
        node.window_list[..a_window_count as usize]
            .copy_from_slice(&a_window_list[..a_window_count as usize]);
        node.window_order[..a_window_count as usize]
            .copy_from_slice(&a_window_order[..a_window_count as usize]);
        node.window_count = a_window_count;
    }

    if let Some(view) = space_manager.view.find_mut(&a_space_id) {
        view.node_mut(a_node_id).zoom = None;
    }
    if let Some(view) = space_manager.view.find_mut(&b_space_id) {
        view.node_mut(b_node_id).zoom = None;
    }
}

pub(crate) fn window_node_find_first_leaf(
    space_id: SpaceId,
    root_node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> NodeId {
    let mut node_id = root_node_id;
    while !window_node_is_leaf(space_id, node_id, space_manager) {
        node_id = match space_manager.view.find(&space_id) {
            Some(view) => match view.node(node_id).left {
                Some(left) => left,
                None => return node_id,
            },
            None => return node_id,
        };
    }
    node_id
}

pub(crate) fn window_node_find_last_leaf(
    space_id: SpaceId,
    root_node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> NodeId {
    let mut node_id = root_node_id;
    while !window_node_is_leaf(space_id, node_id, space_manager) {
        node_id = match space_manager.view.find(&space_id) {
            Some(view) => match view.node(node_id).right {
                Some(right) => right,
                None => return node_id,
            },
            None => return node_id,
        };
    }
    node_id
}

pub(crate) fn window_node_find_prev_leaf(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let parent = match space_manager.view.find(&space_id) {
        Some(view) => view.node(node_id).parent,
        None => return None,
    };
    let Some(parent) = parent else {
        return None;
    };

    if window_node_is_left_child(space_id, node_id, space_manager) {
        return window_node_find_prev_leaf(space_id, parent, space_manager);
    }

    let parent_left = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent).left,
        None => return None,
    };
    let Some(parent_left) = parent_left else {
        return None;
    };

    if window_node_is_leaf(space_id, parent_left, space_manager) {
        return Some(parent_left);
    }

    let parent_left_right = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent_left).right,
        None => return None,
    };
    let Some(parent_left_right) = parent_left_right else {
        return None;
    };

    Some(window_node_find_last_leaf(
        space_id,
        parent_left_right,
        space_manager,
    ))
}

pub(crate) fn window_node_find_next_leaf(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let parent = match space_manager.view.find(&space_id) {
        Some(view) => view.node(node_id).parent,
        None => return None,
    };
    let Some(parent) = parent else {
        return None;
    };

    if window_node_is_right_child(space_id, node_id, space_manager) {
        return window_node_find_next_leaf(space_id, parent, space_manager);
    }

    let parent_right = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent).right,
        None => return None,
    };
    let Some(parent_right) = parent_right else {
        return None;
    };

    if window_node_is_leaf(space_id, parent_right, space_manager) {
        return Some(parent_right);
    }

    let parent_right_left = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent_right).left,
        None => return None,
    };
    let Some(parent_right_left) = parent_right_left else {
        return None;
    };

    Some(window_node_find_first_leaf(
        space_id,
        parent_right_left,
        space_manager,
    ))
}

pub(crate) fn window_node_rotate(
    space_id: SpaceId,
    node_id: NodeId,
    degrees: i32,
    space_manager: &mut SpaceManager,
) {
    let (left, right) = {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let node = view.node_mut(node_id);

        if (degrees == 90 && node.split == WindowNodeSplit::Y)
            || (degrees == 270 && node.split == WindowNodeSplit::X)
            || (degrees == 180)
        {
            let temporary = node.left;
            node.left = node.right;
            node.right = temporary;
            node.ratio = 1.0f32 - node.ratio;
        }

        if degrees != 180 {
            if node.split == WindowNodeSplit::X {
                node.split = WindowNodeSplit::Y;
            } else if node.split == WindowNodeSplit::Y {
                node.split = WindowNodeSplit::X;
            }
        }

        (node.left, node.right)
    };

    if !window_node_is_leaf(space_id, node_id, space_manager) {
        if let Some(left) = left {
            window_node_rotate(space_id, left, degrees, space_manager);
        }
        if let Some(right) = right {
            window_node_rotate(space_id, right, degrees, space_manager);
        }
    }
}

pub(crate) fn window_node_mirror(
    space_id: SpaceId,
    node_id: NodeId,
    axis: WindowNodeSplit,
    space_manager: &mut SpaceManager,
) -> NodeId {
    if !window_node_is_leaf(space_id, node_id, space_manager) {
        let (node_left, node_right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                (node.left, node.right)
            }
            None => return node_id,
        };

        let left = node_left.map(|left| window_node_mirror(space_id, left, axis, space_manager));
        let right =
            node_right.map(|right| window_node_mirror(space_id, right, axis, space_manager));

        let Some(view) = space_manager.view.find_mut(&space_id) else {
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

pub(crate) fn window_node_fence(
    space_id: SpaceId,
    node_id: NodeId,
    direction: i32,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let (node_area, mut parent_id) = match space_manager.view.find(&space_id) {
        Some(view) => {
            let node = view.node(node_id);
            (node.area, node.parent)
        }
        None => return None,
    };

    while let Some(parent) = parent_id {
        let Some(view) = space_manager.view.find(&space_id) else {
            return None;
        };
        let parent_node = view.node(parent);

        if (direction == DIR_NORTH
            && parent_node.split == WindowNodeSplit::X
            && parent_node.area.y < node_area.y)
            || (direction == DIR_WEST
                && parent_node.split == WindowNodeSplit::Y
                && parent_node.area.x < node_area.x)
            || (direction == DIR_SOUTH
                && parent_node.split == WindowNodeSplit::X
                && (parent_node.area.y + parent_node.area.height)
                    > (node_area.y + node_area.height))
            || (direction == DIR_EAST
                && parent_node.split == WindowNodeSplit::Y
                && (parent_node.area.x + parent_node.area.width) > (node_area.x + node_area.width))
        {
            return Some(parent);
        }

        parent_id = parent_node.parent;
    }

    None
}

pub(crate) fn view_find_min_depth_leaf_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let mut list: VecDeque<NodeId> = VecDeque::from([node_id]);

    while let Some(current_node_id) = list.pop_front() {
        if window_node_is_leaf(space_id, current_node_id, space_manager) {
            return Some(current_node_id);
        }

        let (left, right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(current_node_id);
                (node.left, node.right)
            }
            None => return None,
        };

        if let Some(left) = left {
            list.push_back(left);
        }
        if let Some(right) = right {
            list.push_back(right);
        }
    }

    None
}

pub(crate) fn view_find_window_node_in_direction(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    source_node_id: NodeId,
    direction: i32,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    let window_list = space_window_list(space_id, false, window_manager)?;

    let mut best_distance = i32::MAX;
    let mut best_rank = i32::MAX;
    let mut best_node: Option<NodeId> = None;
    let source_area = space_manager.view.find(&space_id)?.node(source_node_id).area;
    let source_area_max = area_max_point(source_area);

    let mut target = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(target_node_id) = target {
        if source_node_id != target_node_id {
            let (target_area, target_first_window_id) = {
                let target_node = space_manager.view.find(&space_id)?.node(target_node_id);
                (target_node.area, target_node.window_order[0])
            };

            let target_area_max = area_max_point(target_area);
            if area_is_in_direction(
                &source_area,
                source_area_max,
                &target_area,
                target_area_max,
                direction,
            ) {
                let distance = area_distance_in_direction(
                    &source_area,
                    source_area_max,
                    &target_area,
                    target_area_max,
                    direction,
                );
                let rank =
                    window_manager_find_rank_of_window_in_list(target_first_window_id, &window_list);
                if (distance < best_distance) || (distance == best_distance && rank < best_rank) {
                    best_node = Some(target_node_id);
                    best_distance = distance;
                    best_rank = rank;
                }
            }
        }

        target = window_node_find_next_leaf(space_id, target_node_id, space_manager);
    }

    best_node
}

pub(crate) fn view_find_window_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
) -> Option<NodeId> {
    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        if window_node_contains_window(space_id, node_id, window_id, space_manager) {
            return Some(node_id);
        }

        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    None
}

pub(crate) fn view_remove_window_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) -> Option<NodeId> {
    let node_id = view_find_window_node(space_manager, space_id, window_id)?;

    if space_manager.view.find(&space_id)?.node(node_id).window_count > 1 {
        let view = space_manager.view.find_mut(&space_id)?;
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
                node.window_order.copy_within(index + 1..window_count, index);
                removed_order = true;
            }
        }

        debug_assert!(removed_entry);
        debug_assert!(removed_order);
        node.window_count -= 1;
        let node_first_window_id = node.window_order[0];

        if view.insertion_point == window_id {
            view.insertion_point = node_first_window_id;
        }

        return None;
    }

    if node_id == ROOT_NODE_ID {
        space_manager.view.find_mut(&space_id)?.insertion_point = WindowId(0);
        insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
        *space_manager.view.find_mut(&space_id)?.node_mut(node_id) = WindowNode::default();
        view_update(space_manager, space_id, display_manager, window_manager);
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
        let view = space_manager.view.find(&space_id)?;
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
        let view = space_manager.view.find_mut(&space_id)?;
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
            let view = space_manager.view.find_mut(&space_id)?;
            let feedback_window = view.node_mut(child_id).feedback_window.take();
            let parent = view.node_mut(parent_id);
            parent.feedback_window = feedback_window;
            parent.insert_direction = child_insert_direction;
            parent.split = child_split;
            parent.child = child_child;
            parent.window_order[0]
        };
        window_manager.insert_feedback.remove(&parent_first_window_id);
        window_manager
            .insert_feedback
            .add(parent_first_window_id, (space_id, parent_id));
        insert_feedback_show(space_id, parent_id, window_manager, space_manager);
    }

    if child_parent.is_some() && !(child_left.is_none() && child_right.is_none()) {
        {
            let view = space_manager.view.find_mut(&space_id)?;

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
            window_node_clear_zoom(space_id, parent_id, space_manager);
        }

        window_node_update(space_manager, space_id, parent_id, window_manager);
    }

    insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
    view_free_node(space_id, child_id, window_manager, space_manager, mouse_drag_state);
    view_free_node(space_id, node_id, window_manager, space_manager, mouse_drag_state);

    let auto_balance = space_manager.view.find(&space_id)?.auto_balance;
    if auto_balance != WindowNodeSplit::None as u32 {
        window_node_balance(space_id, ROOT_NODE_ID, auto_balance, space_manager);
        view_update(space_manager, space_id, display_manager, window_manager);
        return Some(ROOT_NODE_ID);
    }

    Some(parent_id)
}

pub(crate) fn view_stack_window_node(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    if node.window_count as usize >= NODE_MAX_WINDOW_COUNT {
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

pub(crate) fn view_add_window_node_with_insertion_point(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    insertion_point: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    if !window_node_is_occupied(space_id, ROOT_NODE_ID, space_manager)
        && window_node_is_leaf(space_id, ROOT_NODE_ID, space_manager)
    {
        let root = space_manager.view.find_mut(&space_id)?.node_mut(ROOT_NODE_ID);
        root.window_list[0] = window_id;
        root.window_order[0] = window_id;
        root.window_count = 1;
        return Some(ROOT_NODE_ID);
    }

    let layout = space_manager.view.find(&space_id)?.layout;

    if layout == ViewType::Bsp {
        let mut previous_insertion_point = WindowId(0);
        let mut leaf: Option<NodeId> = None;

        if insertion_point != WindowId(0) {
            let view = space_manager.view.find_mut(&space_id)?;
            previous_insertion_point = view.insertion_point;
            view.insertion_point = insertion_point;
        }

        let view_insertion_point = space_manager.view.find(&space_id)?.insertion_point;
        if view_insertion_point != WindowId(0) {
            leaf = view_find_window_node(space_manager, space_id, view_insertion_point);
            space_manager.view.find_mut(&space_id)?.insertion_point = previous_insertion_point;

            if let Some(leaf) = leaf {
                let do_stack = {
                    let leaf_node = space_manager.view.find_mut(&space_id)?.node_mut(leaf);
                    let do_stack = leaf_node.insert_direction == STACK;

                    leaf_node.insert_direction = 0;
                    do_stack
                };
                insert_feedback_destroy(space_id, leaf, window_manager, space_manager);

                if do_stack {
                    view_stack_window_node(space_id, leaf, window_id, space_manager);
                    return Some(leaf);
                }
            }
        }

        if leaf.is_none() {
            if space_manager.window_insertion_point == WindowInsertionPoint::Focused {
                leaf = view_find_window_node(
                    space_manager,
                    space_id,
                    window_manager.focused_window_id,
                );
            } else if space_manager.window_insertion_point == WindowInsertionPoint::First {
                leaf = Some(window_node_find_first_leaf(
                    space_id,
                    ROOT_NODE_ID,
                    space_manager,
                ));
            } else if space_manager.window_insertion_point == WindowInsertionPoint::Last {
                leaf = Some(window_node_find_last_leaf(
                    space_id,
                    ROOT_NODE_ID,
                    space_manager,
                ));
            }

            if leaf.is_none() {
                leaf = view_find_min_depth_leaf_node(space_id, ROOT_NODE_ID, space_manager);
            }
        }

        let leaf = leaf?;

        window_node_split(space_manager, space_id, leaf, window_id);

        let auto_balance = space_manager.view.find(&space_id)?.auto_balance;
        if auto_balance != WindowNodeSplit::None as u32 {
            window_node_balance(space_id, ROOT_NODE_ID, auto_balance, space_manager);
            view_update(space_manager, space_id, display_manager, window_manager);
            return Some(ROOT_NODE_ID);
        }

        return Some(leaf);
    } else if layout == ViewType::Stack {
        view_stack_window_node(space_id, ROOT_NODE_ID, window_id, space_manager);
        return Some(ROOT_NODE_ID);
    }

    None
}

pub(crate) fn view_add_window_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    view_add_window_node_with_insertion_point(
        space_manager,
        space_id,
        window_id,
        WindowId(0),
        display_manager,
        window_manager,
    )
}

pub(crate) fn view_find_window_list(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> Vec<WindowId> {
    let mut window_list: Vec<WindowId> = Vec::new();

    if space_manager.view.find(&space_id).is_none() {
        return window_list;
    }

    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        if let Some(view) = space_manager.view.find(&space_id) {
            let leaf = view.node(node_id);
            for index in 0..leaf.window_count {
                window_list.push(leaf.window_list[index as usize]);
            }
        }

        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    window_list
}

pub(crate) fn view_free_node(
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

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    view.nodes[node_id.0 as usize] = None;
    view.free_node_ids.push(node_id);
}

pub(crate) fn window_node_collect_subtree_post_order(
    view: &View,
    node_id: NodeId,
    out: &mut Vec<NodeId>,
) {
    let node = view.node(node_id);
    if let Some(left) = node.left {
        window_node_collect_subtree_post_order(view, left, out);
    }
    if let Some(right) = node.right {
        window_node_collect_subtree_post_order(view, right, out);
    }
    out.push(node_id);
}
