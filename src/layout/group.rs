use crate::display::manager::DisplayManager;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::{
    MOST_WINDOWS_A_NODE_CAN_HOLD, WindowNode, add_window_to_view_tree, first_leaf_below_node,
    leaf_holding_window, next_leaf_in_tree_order, stack_window_in_node_as_its_front_window,
};
use crate::layout::view::View;
use crate::space::manager::SpaceManager;
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::manager::WindowManager;

pub(crate) struct RememberedGroup {
    pub(crate) members_in_stack_order: Vec<WindowId>,
    pub(crate) front_window: WindowId,
}

pub(crate) struct RejoinedGroup {
    pub(crate) front_window: WindowId,
    pub(crate) other_members: Vec<WindowId>,
}

pub(crate) fn is_node_a_group(
    view: &View,
    node: &WindowNode,
    window_manager: &WindowManager,
) -> bool {
    match view.layout {
        ViewLayout::Stack => node.window_count > 0,
        ViewLayout::BinarySpacePartitioning => {
            node.window_count > 1
                || (node.window_count == 1
                    && does_window_stay_a_group_on_its_own(window_manager, node.window_list[0]))
        }
        ViewLayout::Float | ViewLayout::Default => false,
    }
}

pub(crate) fn is_window_in_a_group(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    window_manager: &WindowManager,
) -> bool {
    let Some(node_id) = leaf_holding_window(space_manager, space_id, window_id) else {
        return false;
    };
    space_manager
        .view
        .get(&space_id)
        .is_some_and(|view| is_node_a_group(view, view.node(node_id), window_manager))
}

pub(crate) fn set_whether_window_stays_a_group_on_its_own(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    stays_a_group_on_its_own: bool,
) {
    if let Some(window) = window_manager.window.get_mut(&window_id) {
        window.stays_a_group_on_its_own = stays_a_group_on_its_own;
    }
}

pub(crate) fn keep_the_group_a_window_left_even_with_one_window_remaining(
    window_manager: &mut WindowManager,
    leaving_window: WindowId,
    window_left_alone_in_the_group: Option<WindowId>,
    layout: ViewLayout,
) {
    set_whether_window_stays_a_group_on_its_own(window_manager, leaving_window, false);
    if layout == ViewLayout::BinarySpacePartitioning
        && let Some(window_left_alone_in_the_group) = window_left_alone_in_the_group
    {
        set_whether_window_stays_a_group_on_its_own(
            window_manager,
            window_left_alone_in_the_group,
            true,
        );
    }
}

pub(crate) fn remember_the_groups_of_view_as_it_leaves_bsp(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) {
    let mut remembered_groups = Vec::new();
    let mut node = Some(first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager));
    while let Some(node_id) = node {
        if let Some(view) = space_manager.view.get(&space_id) {
            let leaf = view.node(node_id);
            if leaf.window_count > 1 {
                remembered_groups.push(RememberedGroup {
                    members_in_stack_order: leaf.window_list[..leaf.window_count as usize].to_vec(),
                    front_window: leaf.window_order[0],
                });
            }
        }
        node = next_leaf_in_tree_order(space_id, node_id, space_manager);
    }

    if let Some(view) = space_manager.view.get_mut(&space_id) {
        view.groups_remembered_outside_bsp = remembered_groups;
    }
}

pub(crate) fn add_window_to_view_tree_rejoining_its_remembered_group(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    if let Some(leaf) =
        leaf_holding_another_member_of_the_remembered_group_of(space_manager, space_id, window_id)
    {
        stack_window_in_node_as_its_front_window(space_id, leaf, window_id, space_manager);
        return Some(leaf);
    }
    add_window_to_view_tree(
        space_manager,
        space_id,
        window_id,
        display_manager,
        window_manager,
    )
}

pub(crate) fn put_the_rejoined_groups_back_in_order_and_forget_them(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
) -> Vec<RejoinedGroup> {
    let remembered_groups = match space_manager.view.get_mut(&space_id) {
        Some(view) if view.layout == ViewLayout::BinarySpacePartitioning => {
            std::mem::take(&mut view.groups_remembered_outside_bsp)
        }
        _ => return Vec::new(),
    };

    let mut rejoined_groups = Vec::new();
    for remembered_group in remembered_groups {
        let Some(leaf) = remembered_group
            .members_in_stack_order
            .iter()
            .find_map(|member| leaf_holding_window(space_manager, space_id, *member))
        else {
            continue;
        };
        let Some(node) = space_manager.find_node_mut_in_view_of_space(space_id, leaf) else {
            continue;
        };

        if node.window_count == 1 {
            let window_left_alone = node.window_list[0];
            set_whether_window_stays_a_group_on_its_own(window_manager, window_left_alone, true);
            continue;
        }

        rejoined_groups.push(restore_the_stack_order_and_front_window_of_node(
            node,
            &remembered_group,
        ));
    }
    rejoined_groups
}

fn restore_the_stack_order_and_front_window_of_node(
    node: &mut WindowNode,
    remembered_group: &RememberedGroup,
) -> RejoinedGroup {
    let window_count = node.window_count as usize;
    let position_in_the_remembered_group = |window_id: &WindowId| {
        remembered_group
            .members_in_stack_order
            .iter()
            .position(|member| member == window_id)
            .unwrap_or(usize::MAX)
    };

    let mut window_list = node.window_list[..window_count].to_vec();
    window_list.sort_by_key(position_in_the_remembered_group);
    node.window_list[..window_count].copy_from_slice(&window_list);

    let front_window = if window_list.contains(&remembered_group.front_window) {
        remembered_group.front_window
    } else {
        node.window_order[0]
    };
    let mut window_order: Vec<WindowId> = node.window_order[..window_count]
        .iter()
        .copied()
        .filter(|window_id| *window_id != front_window)
        .collect();
    window_order.insert(0, front_window);
    node.window_order[..window_count].copy_from_slice(&window_order);

    RejoinedGroup {
        front_window,
        other_members: window_order[1..].to_vec(),
    }
}

fn leaf_holding_another_member_of_the_remembered_group_of(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
) -> Option<NodeId> {
    let view = space_manager.view.get(&space_id)?;
    if view.layout != ViewLayout::BinarySpacePartitioning {
        return None;
    }
    let other_members: Vec<WindowId> = view
        .groups_remembered_outside_bsp
        .iter()
        .find(|group| group.members_in_stack_order.contains(&window_id))?
        .members_in_stack_order
        .iter()
        .copied()
        .filter(|member| *member != window_id)
        .collect();

    let leaf = other_members
        .into_iter()
        .find_map(|member| leaf_holding_window(space_manager, space_id, member))?;
    let leaf_has_room = space_manager
        .find_node_in_view_of_space(space_id, leaf)
        .is_some_and(|node| (node.window_count as usize) < MOST_WINDOWS_A_NODE_CAN_HOLD);
    leaf_has_room.then_some(leaf)
}

fn does_window_stay_a_group_on_its_own(
    window_manager: &WindowManager,
    window_id: WindowId,
) -> bool {
    window_manager
        .window
        .get(&window_id)
        .is_some_and(|window| window.stays_a_group_on_its_own)
}

#[cfg(test)]
mod tests {
    use super::{RememberedGroup, restore_the_stack_order_and_front_window_of_node};
    use crate::layout::tree::WindowNode;
    use crate::support::handles::WindowId;

    fn node_holding_windows(window_list: &[u32], window_order: &[u32]) -> WindowNode {
        let mut node = WindowNode::default();
        for (index, window_id) in window_list.iter().enumerate() {
            node.window_list[index] = WindowId(*window_id);
        }
        for (index, window_id) in window_order.iter().enumerate() {
            node.window_order[index] = WindowId(*window_id);
        }
        node.window_count = window_list.len() as i32;
        node
    }

    fn remembered_group(members_in_stack_order: &[u32], front_window: u32) -> RememberedGroup {
        RememberedGroup {
            members_in_stack_order: members_in_stack_order
                .iter()
                .map(|id| WindowId(*id))
                .collect(),
            front_window: WindowId(front_window),
        }
    }

    fn ids(window_ids: &[WindowId]) -> Vec<u32> {
        window_ids.iter().map(|window_id| window_id.0).collect()
    }

    #[test]
    fn a_rejoined_group_gets_its_remembered_stack_order_and_front_window_back() {
        let mut node = node_holding_windows(&[30, 10, 20], &[30, 20, 10]);

        let rejoined_group = restore_the_stack_order_and_front_window_of_node(
            &mut node,
            &remembered_group(&[10, 20, 30], 20),
        );

        assert_eq!(ids(&node.window_list[..3]), vec![10, 20, 30]);
        assert_eq!(ids(&node.window_order[..3]), vec![20, 30, 10]);
        assert_eq!(rejoined_group.front_window.0, 20);
        assert_eq!(ids(&rejoined_group.other_members), vec![30, 10]);
    }

    #[test]
    fn a_rejoined_group_whose_front_window_is_gone_keeps_its_current_front_window() {
        let mut node = node_holding_windows(&[20, 10], &[10, 20]);

        let rejoined_group = restore_the_stack_order_and_front_window_of_node(
            &mut node,
            &remembered_group(&[10, 20, 30], 30),
        );

        assert_eq!(ids(&node.window_list[..2]), vec![10, 20]);
        assert_eq!(ids(&node.window_order[..2]), vec![10, 20]);
        assert_eq!(rejoined_group.front_window.0, 10);
        assert_eq!(ids(&rejoined_group.other_members), vec![20]);
    }
}
