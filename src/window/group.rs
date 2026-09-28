use crate::display::manager::DisplayManager;
use crate::layout::group::{is_node_a_group, set_whether_window_stays_a_group_on_its_own};
use crate::layout::settings::ViewLayout;
use crate::layout::tree::{
    WindowNodeSplit, add_window_to_view_tree,
    balance_split_ratios_below_node_giving_each_leaf_an_equal_share, leaf_holding_window,
    move_windows_below_node_into_their_areas, remove_window_from_view_tree,
    split_leaf_node_to_hold_a_new_window,
};
use crate::layout::view::{
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::space::manager::SpaceManager;
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::focus_follows_mouse::schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles;
use crate::window::manager::{WindowManager, WindowOperationOutcome, space_managing_window};

pub(crate) fn toggle_group_of_window(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) -> WindowOperationOutcome {
    let Some(space_id) = space_managing_window(window_manager, window_id) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };
    if space_manager
        .view
        .get(&space_id)
        .is_none_or(|view| view.layout != ViewLayout::BinarySpacePartitioning)
    {
        return WindowOperationOutcome::InvalidSourceView;
    }
    let Some(node_id) = leaf_holding_window(space_manager, space_id, window_id) else {
        return WindowOperationOutcome::InvalidSourceNode;
    };

    let is_already_a_group = space_manager
        .view
        .get(&space_id)
        .is_some_and(|view| is_node_a_group(view, view.node(node_id), window_manager));
    if is_already_a_group {
        ungroup_node_giving_each_window_its_own_tile(
            space_manager,
            space_id,
            node_id,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
    } else {
        set_whether_window_stays_a_group_on_its_own(window_manager, window_id, true);
        move_windows_below_node_into_their_areas(space_id, node_id, window_manager, space_manager);
    }
    schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles(window_manager);

    WindowOperationOutcome::Success
}

fn ungroup_node_giving_each_window_its_own_tile(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some((front_window, windows_leaving_the_group)) =
        space_manager.view.get(&space_id).map(|view| {
            let node = view.node(node_id);
            let front_window = node.window_order[0];
            let windows_leaving_the_group: Vec<WindowId> = node.window_list
                [..node.window_count as usize]
                .iter()
                .copied()
                .filter(|window_id| *window_id != front_window)
                .collect();
            (front_window, windows_leaving_the_group)
        })
    else {
        return;
    };

    let mut window_to_split_beside = front_window;
    for window_leaving_the_group in windows_leaving_the_group {
        remove_window_from_view_tree(
            space_manager,
            space_id,
            window_leaving_the_group,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        match leaf_holding_window(space_manager, space_id, window_to_split_beside) {
            Some(leaf) => split_leaf_node_to_hold_a_new_window(
                space_manager,
                space_id,
                leaf,
                window_leaving_the_group,
            ),
            None => {
                add_window_to_view_tree(
                    space_manager,
                    space_id,
                    window_leaving_the_group,
                    display_manager,
                    window_manager,
                );
            }
        }
        window_to_split_beside = window_leaving_the_group;
    }
    set_whether_window_stays_a_group_on_its_own(window_manager, front_window, false);

    let auto_balance = space_manager
        .view
        .get(&space_id)
        .map_or(WindowNodeSplit::None as u32, |view| view.auto_balance);
    if auto_balance != WindowNodeSplit::None as u32 {
        balance_split_ratios_below_node_giving_each_leaf_an_equal_share(
            space_id,
            ROOT_NODE_ID,
            auto_balance,
            space_manager,
        );
    }
    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    );
    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        space_id,
        window_manager,
    );
}
