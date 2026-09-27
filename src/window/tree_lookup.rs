use crate::display::manager::DisplayManager;
use crate::layout::tree::{
    view_find_window_node, view_find_window_node_in_direction, window_node_find_first_leaf,
    window_node_find_last_leaf, window_node_find_next_leaf, window_node_find_prev_leaf,
    window_node_is_leaf, window_node_is_left_child,
};
use crate::space::focus::space_manager_active_space;
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::manager::{
    WindowManager, window_manager_find_managed_window, window_manager_find_window,
};

pub(crate) fn window_manager_find_closest_managed_window_in_direction(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    direction: i32,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let Some(closest_node_id) = view_find_window_node_in_direction(
        space_manager,
        space_id,
        node_id,
        direction,
        window_manager,
    ) else {
        return None;
    };

    let closest_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(closest_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, closest_window_id)
}

pub(crate) fn window_manager_find_prev_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let Some(previous_node_id) = window_node_find_prev_leaf(space_id, node_id, space_manager)
    else {
        return None;
    };

    let previous_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(previous_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, previous_window_id)
}

pub(crate) fn window_manager_find_next_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let Some(next_node_id) = window_node_find_next_leaf(space_id, node_id, space_manager) else {
        return None;
    };

    let next_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(next_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, next_window_id)
}

pub(crate) fn window_manager_find_first_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let first_node_id = window_node_find_first_leaf(space_id, ROOT_NODE_ID, space_manager);

    let first_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(first_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, first_window_id)
}

pub(crate) fn window_manager_find_last_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let last_node_id = window_node_find_last_leaf(space_id, ROOT_NODE_ID, space_manager);

    let last_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(last_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, last_window_id)
}

pub(crate) fn window_manager_find_recent_managed_window(
    window_manager: &mut WindowManager,
) -> Option<WindowId> {
    let Some(window_id) = window_manager_find_window(window_manager, window_manager.last_window_id)
    else {
        return None;
    };

    let Some(_space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    Some(window_id)
}

pub(crate) fn window_manager_find_largest_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let mut best_id: u32 = 0;
    let mut best_area: u32 = 0;

    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        let (node_area, node_first_window_id) = {
            let leaf_node = space_manager.view.find(&space_id)?.find_node(node_id)?;
            (leaf_node.area, leaf_node.window_order[0])
        };
        let area = (node_area.width * node_area.height) as u32;
        if area > best_area {
            best_id = node_first_window_id.0;
            best_area = area;
        }
        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    if best_id != 0 {
        window_manager_find_window(window_manager, WindowId(best_id))
    } else {
        None
    }
}

pub(crate) fn window_manager_find_smallest_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let mut best_id: u32 = 0;
    let mut best_area: u32 = u32::MAX;

    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        let (node_area, node_first_window_id) = {
            let leaf_node = space_manager.view.find(&space_id)?.find_node(node_id)?;
            (leaf_node.area, leaf_node.window_order[0])
        };
        let area = (node_area.width * node_area.height) as u32;
        if area <= best_area {
            best_id = node_first_window_id.0;
            best_area = area;
        }
        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    if best_id != 0 {
        window_manager_find_window(window_manager, WindowId(best_id))
    } else {
        None
    }
}

fn window_node_sibling(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    parent_node_id: NodeId,
) -> Option<NodeId> {
    let node_is_left_child = window_node_is_left_child(space_id, node_id, space_manager);
    let parent_node = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?;
    if node_is_left_child {
        parent_node.right
    } else {
        parent_node.left
    }
}

fn window_node_first_window_in_order(
    space_manager: &SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
) -> Option<WindowId> {
    Some(
        space_manager
            .view
            .find(&space_id)?
            .find_node(node_id)?
            .window_order[0],
    )
}

pub(crate) fn window_manager_find_sibling_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id = window_node_sibling(space_manager, space_id, node_id, parent_node_id)?;
    if !window_node_is_leaf(space_id, sibling_node_id, space_manager) {
        return None;
    }

    let sibling_window_id =
        window_node_first_window_in_order(space_manager, space_id, sibling_node_id)?;
    window_manager_find_window(window_manager, sibling_window_id)
}

pub(crate) fn window_manager_find_first_nephew_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id = window_node_sibling(space_manager, space_id, node_id, parent_node_id)?;
    let sibling_left_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(sibling_node_id)?
        .left;
    if window_node_is_leaf(space_id, sibling_node_id, space_manager)
        || !sibling_left_node_id
            .is_some_and(|left| window_node_is_leaf(space_id, left, space_manager))
    {
        return None;
    }

    let nephew_window_id =
        window_node_first_window_in_order(space_manager, space_id, sibling_left_node_id?)?;
    window_manager_find_window(window_manager, nephew_window_id)
}

pub(crate) fn window_manager_find_second_nephew_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id = window_node_sibling(space_manager, space_id, node_id, parent_node_id)?;
    let sibling_right_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(sibling_node_id)?
        .right;
    if window_node_is_leaf(space_id, sibling_node_id, space_manager)
        || !sibling_right_node_id
            .is_some_and(|right| window_node_is_leaf(space_id, right, space_manager))
    {
        return None;
    }

    let nephew_window_id =
        window_node_first_window_in_order(space_manager, space_id, sibling_right_node_id?)?;
    window_manager_find_window(window_manager, nephew_window_id)
}

pub(crate) fn window_manager_find_uncle_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let Some(grandparent_node_id) = grandparent else {
        return None;
    };

    let uncle_node_id =
        window_node_sibling(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    if !window_node_is_leaf(space_id, uncle_node_id, space_manager) {
        return None;
    }

    let uncle_window_id =
        window_node_first_window_in_order(space_manager, space_id, uncle_node_id)?;
    window_manager_find_window(window_manager, uncle_window_id)
}

pub(crate) fn window_manager_find_first_cousin_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let Some(grandparent_node_id) = grandparent else {
        return None;
    };

    let uncle_node_id =
        window_node_sibling(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    let uncle_left_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(uncle_node_id)?
        .left;
    if window_node_is_leaf(space_id, uncle_node_id, space_manager)
        || !uncle_left_node_id
            .is_some_and(|left| window_node_is_leaf(space_id, left, space_manager))
    {
        return None;
    }

    let cousin_window_id =
        window_node_first_window_in_order(space_manager, space_id, uncle_left_node_id?)?;
    window_manager_find_window(window_manager, cousin_window_id)
}

pub(crate) fn window_manager_find_second_cousin_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let Some(grandparent_node_id) = grandparent else {
        return None;
    };

    let uncle_node_id =
        window_node_sibling(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    let uncle_right_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(uncle_node_id)?
        .right;
    if window_node_is_leaf(space_id, uncle_node_id, space_manager)
        || !uncle_right_node_id
            .is_some_and(|right| window_node_is_leaf(space_id, right, space_manager))
    {
        return None;
    }

    let cousin_window_id =
        window_node_first_window_in_order(space_manager, space_id, uncle_right_node_id?)?;
    window_manager_find_window(window_manager, cousin_window_id)
}
