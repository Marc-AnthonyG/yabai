use crate::display::manager::DisplayManager;
use crate::layout::group_header::refresh_the_group_headers_of_view;
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::tree::{
    WindowNodeSplit, add_window_to_view_tree_preferring_insertion_point,
    balance_split_ratios_below_node_giving_each_leaf_an_equal_share, is_node_below_the_root,
    leaf_holding_window, mirror_node_subtree_along_axis, move_windows_below_node_into_their_areas,
    recompute_areas_below_node_redrawing_insert_feedback, remove_window_from_view_tree,
    reset_split_ratios_below_node_to_the_global_ratio, rotate_node_subtree_by_degrees,
};
use crate::layout::view::{
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::is_space_visible_on_its_display;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::handles::{ROOT_NODE_ID, SpaceId, WindowId};
use crate::support::layer::{LAYER_BELOW, LAYER_NORMAL};
use crate::window::layer::set_window_layer_unless_explicitly_set;
use crate::window::manager::WindowManager;
use crate::window::model::query_space_holding_window;

pub(crate) fn untile_window_from_view_of_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout == ViewLayout::Float {
        return;
    }

    set_window_layer_unless_explicitly_set(window_id, LAYER_NORMAL, window_manager);
    let Some(node_id) = remove_window_from_view_tree(
        space_manager,
        space_id,
        window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    ) else {
        refresh_the_group_headers_of_view(space_id, space_manager, window_manager);
        return;
    };

    if is_space_visible_on_its_display(space_id) {
        move_windows_below_node_into_their_areas(space_id, node_id, window_manager, space_manager);
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
    }
}

pub(crate) fn rotate_view_of_space_by_degrees(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    degrees: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewLayout::BinarySpacePartitioning {
        return false;
    }

    rotate_node_subtree_by_degrees(space_id, ROOT_NODE_ID, degrees, space_manager);
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

    true
}

pub(crate) fn mirror_view_of_space_along_axis(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis: WindowNodeSplit,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewLayout::BinarySpacePartitioning {
        return false;
    }

    mirror_node_subtree_along_axis(space_id, ROOT_NODE_ID, axis, space_manager);
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

    true
}

pub(crate) fn reset_split_ratios_in_view_of_space_to_the_global_ratio(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis_flag: u32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewLayout::BinarySpacePartitioning {
        return false;
    }

    reset_split_ratios_below_node_to_the_global_ratio(
        space_id,
        ROOT_NODE_ID,
        axis_flag,
        space_manager,
    );
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

    true
}

pub(crate) fn balance_split_ratios_in_view_of_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis_flag: u32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewLayout::BinarySpacePartitioning {
        return false;
    }

    balance_split_ratios_below_node_giving_each_leaf_an_equal_share(
        space_id,
        ROOT_NODE_ID,
        axis_flag,
        space_manager,
    );
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

    true
}

pub(crate) fn tile_window_on_space_preferring_insertion_point(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    space_id: SpaceId,
    insertion_point: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return space_id;
    };
    if view.layout == ViewLayout::Float {
        return space_id;
    }

    set_window_layer_unless_explicitly_set(window_id, LAYER_BELOW, window_manager);
    let node_id = add_window_to_view_tree_preferring_insertion_point(
        space_manager,
        space_id,
        window_id,
        insertion_point,
        display_manager,
        window_manager,
    );
    debug_assert!(node_id.is_some());

    if is_space_visible_on_its_display(space_id) {
        if let Some(node_id) = node_id {
            move_windows_below_node_into_their_areas(
                space_id,
                node_id,
                window_manager,
                space_manager,
            );
        }
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
    }

    space_id
}

pub(crate) fn tile_window_on_space(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    tile_window_on_space_preferring_insertion_point(
        space_manager,
        window_id,
        space_id,
        WindowId(0),
        display_manager,
        window_manager,
    )
}

pub(crate) fn toggle_split_direction_of_the_parent_of_window_leaf(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_space_holding_window(window_id),
        display_manager,
        window_manager,
    );
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout != ViewLayout::BinarySpacePartitioning {
        return;
    }

    let node_id = leaf_holding_window(space_manager, space_id, window_id);
    if let Some(node_id) = node_id
        && is_node_below_the_root(space_id, node_id, space_manager)
    {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let Some(parent_node_id) = view.node(node_id).parent else {
            return;
        };
        let parent_split = view.node(parent_node_id).split;
        view.node_mut(parent_node_id).split = if parent_split == WindowNodeSplit::Vertical {
            WindowNodeSplit::Horizontal
        } else {
            WindowNodeSplit::Vertical
        };

        let auto_balance = view.auto_balance;
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
            move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                space_manager,
                space_id,
                window_manager,
            );
        } else {
            recompute_areas_below_node_redrawing_insert_feedback(
                space_manager,
                space_id,
                parent_node_id,
                window_manager,
            );
            if is_space_visible_on_its_display(space_id) {
                move_windows_below_node_into_their_areas(
                    space_id,
                    parent_node_id,
                    window_manager,
                    space_manager,
                );
            } else if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
            }
        }
    }
}
