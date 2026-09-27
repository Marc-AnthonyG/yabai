use crate::display::manager::DisplayManager;
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::{
    WindowNodeSplit, view_add_window_node_with_insertion_point, view_find_window_node,
    view_remove_window_node, window_node_balance, window_node_equalize, window_node_flush,
    window_node_is_intermediate, window_node_mirror, window_node_rotate, window_node_update,
};
use crate::layout::view::{view_flush, view_update};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::space_is_visible;
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::support::handles::{ROOT_NODE_ID, SpaceId, WindowId};
use crate::support::layer::{LAYER_BELOW, LAYER_NORMAL};
use crate::window::layer::window_manager_adjust_layer;
use crate::window::manager::WindowManager;
use crate::window::model::window_space;

pub(crate) fn space_manager_untile_window(
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
    if view.layout == ViewType::Float {
        return;
    }

    window_manager_adjust_layer(window_id, LAYER_NORMAL, window_manager);
    let Some(node_id) = view_remove_window_node(
        space_manager,
        space_id,
        window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    ) else {
        return;
    };

    if space_is_visible(space_id) {
        window_node_flush(space_id, node_id, window_manager, space_manager);
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }
}

pub(crate) fn space_manager_rotate_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    degrees: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_rotate(space_id, ROOT_NODE_ID, degrees, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_mirror_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis: WindowNodeSplit,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_mirror(space_id, ROOT_NODE_ID, axis, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_equalize_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis_flag: u32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_equalize(space_id, ROOT_NODE_ID, axis_flag, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_balance_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis_flag: u32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_balance(space_id, ROOT_NODE_ID, axis_flag, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_tile_window_on_space_with_insertion_point(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    space_id: SpaceId,
    insertion_point: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return space_id;
    };
    if view.layout == ViewType::Float {
        return space_id;
    }

    window_manager_adjust_layer(window_id, LAYER_BELOW, window_manager);
    let node_id = view_add_window_node_with_insertion_point(
        space_manager,
        space_id,
        window_id,
        insertion_point,
        display_manager,
        window_manager,
    );
    debug_assert!(node_id.is_some());

    if space_is_visible(space_id) {
        if let Some(node_id) = node_id {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        }
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    space_id
}

pub(crate) fn space_manager_tile_window_on_space(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    space_manager_tile_window_on_space_with_insertion_point(
        space_manager,
        window_id,
        space_id,
        WindowId(0),
        display_manager,
        window_manager,
    )
}

pub(crate) fn space_manager_toggle_window_split(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id = space_manager_find_view(
        space_manager,
        window_space(window_id),
        display_manager,
        window_manager,
    );
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout != ViewType::Bsp {
        return;
    }

    let node_id = view_find_window_node(space_manager, space_id, window_id);
    if let Some(node_id) = node_id
        && window_node_is_intermediate(space_id, node_id, space_manager)
    {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let Some(parent_node_id) = view.node(node_id).parent else {
            return;
        };
        let parent_split = view.node(parent_node_id).split;
        view.node_mut(parent_node_id).split = if parent_split == WindowNodeSplit::Y {
            WindowNodeSplit::X
        } else {
            WindowNodeSplit::Y
        };

        let auto_balance = view.auto_balance;
        if auto_balance != WindowNodeSplit::None as u32 {
            window_node_balance(space_id, ROOT_NODE_ID, auto_balance, space_manager);
            view_update(space_manager, space_id, display_manager, window_manager);
            view_flush(space_manager, space_id, window_manager);
        } else {
            window_node_update(space_manager, space_id, parent_node_id, window_manager);
            if space_is_visible(space_id) {
                window_node_flush(space_id, parent_node_id, window_manager, space_manager);
            } else if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }
    }
}
