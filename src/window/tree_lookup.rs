use crate::display::manager::DisplayManager;
use crate::layout::tree::{
    closest_leaf_in_direction_of_node, first_leaf_below_node, is_leaf_node,
    is_node_the_left_child_of_its_parent, last_leaf_below_node, leaf_holding_window,
    next_leaf_in_tree_order, previous_leaf_in_tree_order,
};
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::manager::{WindowManager, space_managing_window, tracked_window_with_id};

pub(crate) fn closest_managed_window_in_direction(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    direction: i32,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let space_id = space_managing_window(window_manager, window_id)?;

    let node_id = leaf_holding_window(space_manager, space_id, window_id)?;

    let closest_node_id = closest_leaf_in_direction_of_node(
        space_manager,
        space_id,
        node_id,
        direction,
        window_manager,
    )?;

    let closest_window_id = space_manager
        .view
        .get(&space_id)?
        .find_node(closest_node_id)?
        .window_order[0];
    tracked_window_with_id(window_manager, closest_window_id)
}

pub(crate) fn managed_window_before_window_in_active_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_the_focused_display(window_manager),
        display_manager,
        window_manager,
    );
    if !space_manager.view.contains_key(&space_id) {
        return None;
    }

    let node_id = leaf_holding_window(space_manager, space_id, window_id)?;

    let previous_node_id = previous_leaf_in_tree_order(space_id, node_id, space_manager)?;

    let previous_window_id = space_manager
        .view
        .get(&space_id)?
        .find_node(previous_node_id)?
        .window_order[0];
    tracked_window_with_id(window_manager, previous_window_id)
}

pub(crate) fn managed_window_after_window_in_active_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_the_focused_display(window_manager),
        display_manager,
        window_manager,
    );
    if !space_manager.view.contains_key(&space_id) {
        return None;
    }

    let node_id = leaf_holding_window(space_manager, space_id, window_id)?;

    let next_node_id = next_leaf_in_tree_order(space_id, node_id, space_manager)?;

    let next_window_id = space_manager
        .view
        .get(&space_id)?
        .find_node(next_node_id)?
        .window_order[0];
    tracked_window_with_id(window_manager, next_window_id)
}

pub(crate) fn first_managed_window_in_active_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_the_focused_display(window_manager),
        display_manager,
        window_manager,
    );
    if !space_manager.view.contains_key(&space_id) {
        return None;
    }

    let first_node_id = first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager);

    let first_window_id = space_manager
        .view
        .get(&space_id)?
        .find_node(first_node_id)?
        .window_order[0];
    tracked_window_with_id(window_manager, first_window_id)
}

pub(crate) fn last_managed_window_in_active_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_the_focused_display(window_manager),
        display_manager,
        window_manager,
    );
    if !space_manager.view.contains_key(&space_id) {
        return None;
    }

    let last_node_id = last_leaf_below_node(space_id, ROOT_NODE_ID, space_manager);

    let last_window_id = space_manager
        .view
        .get(&space_id)?
        .find_node(last_node_id)?
        .window_order[0];
    tracked_window_with_id(window_manager, last_window_id)
}

pub(crate) fn previously_focused_window_if_managed(
    window_manager: &mut WindowManager,
) -> Option<WindowId> {
    let window_id = tracked_window_with_id(window_manager, window_manager.last_window_id)?;

    space_managing_window(window_manager, window_id)?;

    Some(window_id)
}

pub(crate) fn largest_managed_window_in_active_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_the_focused_display(window_manager),
        display_manager,
        window_manager,
    );
    if !space_manager.view.contains_key(&space_id) {
        return None;
    }

    let mut best_id: u32 = 0;
    let mut best_area: u32 = 0;

    let mut node = Some(first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
    while let Some(node_id) = node {
        let (node_area, node_first_window_id) = {
            let leaf_node = space_manager.view.get(&space_id)?.find_node(node_id)?;
            (leaf_node.area, leaf_node.window_order[0])
        };
        let area = (node_area.width * node_area.height) as u32;
        if area > best_area {
            best_id = node_first_window_id.0;
            best_area = area;
        }
        node = next_leaf_in_tree_order(space_id, node_id, space_manager);
    }

    if best_id != 0 {
        tracked_window_with_id(window_manager, WindowId(best_id))
    } else {
        None
    }
}

pub(crate) fn smallest_managed_window_in_active_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_the_focused_display(window_manager),
        display_manager,
        window_manager,
    );
    if !space_manager.view.contains_key(&space_id) {
        return None;
    }

    let mut best_id: u32 = 0;
    let mut best_area: u32 = u32::MAX;

    let mut node = Some(first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
    while let Some(node_id) = node {
        let (node_area, node_first_window_id) = {
            let leaf_node = space_manager.view.get(&space_id)?.find_node(node_id)?;
            (leaf_node.area, leaf_node.window_order[0])
        };
        let area = (node_area.width * node_area.height) as u32;
        if area <= best_area {
            best_id = node_first_window_id.0;
            best_area = area;
        }
        node = next_leaf_in_tree_order(space_id, node_id, space_manager);
    }

    if best_id != 0 {
        tracked_window_with_id(window_manager, WindowId(best_id))
    } else {
        None
    }
}

fn sibling_of_node_under_parent(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    parent_node_id: NodeId,
) -> Option<NodeId> {
    let node_is_left_child = is_node_the_left_child_of_its_parent(space_id, node_id, space_manager);
    let parent_node = space_manager
        .view
        .get(&space_id)?
        .find_node(parent_node_id)?;
    if node_is_left_child {
        parent_node.right
    } else {
        parent_node.left
    }
}

fn most_recently_focused_window_of_node(
    space_manager: &SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
) -> Option<WindowId> {
    Some(
        space_manager
            .view
            .get(&space_id)?
            .find_node(node_id)?
            .window_order[0],
    )
}

pub(crate) fn sibling_window_of_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let space_id = space_managing_window(window_manager, window_id)?;

    let node = leaf_holding_window(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id =
        sibling_of_node_under_parent(space_manager, space_id, node_id, parent_node_id)?;
    if !is_leaf_node(space_id, sibling_node_id, space_manager) {
        return None;
    }

    let sibling_window_id =
        most_recently_focused_window_of_node(space_manager, space_id, sibling_node_id)?;
    tracked_window_with_id(window_manager, sibling_window_id)
}

pub(crate) fn first_nephew_window_of_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let space_id = space_managing_window(window_manager, window_id)?;

    let node = leaf_holding_window(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id =
        sibling_of_node_under_parent(space_manager, space_id, node_id, parent_node_id)?;
    let sibling_left_node_id = space_manager
        .view
        .get(&space_id)?
        .find_node(sibling_node_id)?
        .left;
    if is_leaf_node(space_id, sibling_node_id, space_manager)
        || !sibling_left_node_id.is_some_and(|left| is_leaf_node(space_id, left, space_manager))
    {
        return None;
    }

    let nephew_window_id =
        most_recently_focused_window_of_node(space_manager, space_id, sibling_left_node_id?)?;
    tracked_window_with_id(window_manager, nephew_window_id)
}

pub(crate) fn second_nephew_window_of_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let space_id = space_managing_window(window_manager, window_id)?;

    let node = leaf_holding_window(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id =
        sibling_of_node_under_parent(space_manager, space_id, node_id, parent_node_id)?;
    let sibling_right_node_id = space_manager
        .view
        .get(&space_id)?
        .find_node(sibling_node_id)?
        .right;
    if is_leaf_node(space_id, sibling_node_id, space_manager)
        || !sibling_right_node_id.is_some_and(|right| is_leaf_node(space_id, right, space_manager))
    {
        return None;
    }

    let nephew_window_id =
        most_recently_focused_window_of_node(space_manager, space_id, sibling_right_node_id?)?;
    tracked_window_with_id(window_manager, nephew_window_id)
}

pub(crate) fn uncle_window_of_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let space_id = space_managing_window(window_manager, window_id)?;

    let node = leaf_holding_window(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .get(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let grandparent_node_id = grandparent?;

    let uncle_node_id =
        sibling_of_node_under_parent(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    if !is_leaf_node(space_id, uncle_node_id, space_manager) {
        return None;
    }

    let uncle_window_id =
        most_recently_focused_window_of_node(space_manager, space_id, uncle_node_id)?;
    tracked_window_with_id(window_manager, uncle_window_id)
}

pub(crate) fn first_cousin_window_of_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let space_id = space_managing_window(window_manager, window_id)?;

    let node = leaf_holding_window(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .get(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let grandparent_node_id = grandparent?;

    let uncle_node_id =
        sibling_of_node_under_parent(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    let uncle_left_node_id = space_manager
        .view
        .get(&space_id)?
        .find_node(uncle_node_id)?
        .left;
    if is_leaf_node(space_id, uncle_node_id, space_manager)
        || !uncle_left_node_id.is_some_and(|left| is_leaf_node(space_id, left, space_manager))
    {
        return None;
    }

    let cousin_window_id =
        most_recently_focused_window_of_node(space_manager, space_id, uncle_left_node_id?)?;
    tracked_window_with_id(window_manager, cousin_window_id)
}

pub(crate) fn second_cousin_window_of_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let space_id = space_managing_window(window_manager, window_id)?;

    let node = leaf_holding_window(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .get(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let grandparent_node_id = grandparent?;

    let uncle_node_id =
        sibling_of_node_under_parent(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    let uncle_right_node_id = space_manager
        .view
        .get(&space_id)?
        .find_node(uncle_node_id)?
        .right;
    if is_leaf_node(space_id, uncle_node_id, space_manager)
        || !uncle_right_node_id.is_some_and(|right| is_leaf_node(space_id, right, space_manager))
    {
        return None;
    }

    let cousin_window_id =
        most_recently_focused_window_of_node(space_manager, space_id, uncle_right_node_id?)?;
    tracked_window_with_id(window_manager, cousin_window_id)
}
